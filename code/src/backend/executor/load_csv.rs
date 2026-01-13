///! This file is to test load CSV file without using Buffer Manager.
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use colored::Colorize;

use crate::catalog::types::Catalog;
use crate::heap::insert_tuple;

pub fn load_csv(
    catalog: &Catalog,
    db_name: &str,
    table_name: &str,
    file: &mut File,
    csv_path: &str,
) -> io::Result<()> {
    // --- 1. Fetch table schema from catalog ---
    let db = catalog
        .databases
        .get(db_name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("Database '{}' not found", db_name)))?;

    let table = db
        .tables
        .get(table_name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("Table '{}' not found", table_name)))?;

    let columns = &table.columns;
    if columns.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Table has no columns"));
    }

    // --- 2. Open and read the CSV file ---
    let csv_file = File::open(csv_path)?;
    let reader = BufReader::new(csv_file);

    let mut lines = reader.lines();

    // Skip header line
    if let Some(Ok(_header)) = lines.next() {
        // println!("Header: {}", header);
    }

    // --- 3. Iterate through rows ---
    let mut inserted = 0;
    for (i, line) in lines.enumerate() {
        let row = line?;
        if row.trim().is_empty() {
            continue;
        }

        // Split CSV fields by comma
        let values: Vec<&str> = row.split(',').map(|v| v.trim()).collect();

        // Validate number of columns
        if values.len() != columns.len() {
            println!(
                "{}",
                format!("Skipping row {}: expected {} columns, found {}", i + 1, columns.len(), values.len()).yellow()
            );
            continue;
        }

        // --- 4. Serialize row based on schema ---
        let mut tuple_bytes: Vec<u8> = Vec::new();

        for (val, col) in values.iter().zip(columns.iter()) {
            match col.data_type.as_str() {
                "INT" => {
                    let num: i32 = val.parse().unwrap_or_default();
                    tuple_bytes.extend_from_slice(&num.to_le_bytes());
                }
                "FLOAT" => {
                    let num: f32 = val.parse().unwrap_or_default();
                    tuple_bytes.extend_from_slice(&num.to_le_bytes());
                }
                "TEXT" => {
                    let mut text_bytes = val.as_bytes().to_vec();
                    if text_bytes.len() > 10 {
                        text_bytes.truncate(10);
                    } else if text_bytes.len() < 10 {
                        text_bytes.extend(vec![b' '; 10 - text_bytes.len()]);
                    }
                    tuple_bytes.extend_from_slice(&text_bytes);
                }
                "BOOLEAN" => {
                    let bool_val = match val.to_lowercase().as_str() {
                        "true" | "t" | "1" | "yes" | "y" => 1u8,
                        _ => 0u8,
                    };
                    tuple_bytes.push(bool_val);
                }
                "DATE" => {
                    // Parse YYYY-MM-DD format to days since epoch (1970-01-01)
                    let days = parse_date(val).unwrap_or(0);
                    tuple_bytes.extend_from_slice(&days.to_le_bytes());
                }
                "TIME" => {
                    // Parse HH:MM:SS format to seconds since midnight
                    let seconds = parse_time(val).unwrap_or(0);
                    tuple_bytes.extend_from_slice(&seconds.to_le_bytes());
                }
                "DATETIME" => {
                    // Parse YYYY-MM-DD HH:MM:SS format to Unix timestamp
                    let timestamp = parse_datetime(val).unwrap_or(0);
                    tuple_bytes.extend_from_slice(&timestamp.to_le_bytes());
                }
                _ => {
                    println!(
                        "{}",
                        format!("Unsupported column type '{}' in column '{}'", col.data_type, col.name).red()
                    );
                    continue;
                }
            }
        }

        // --- 5. Insert tuple into page system ---
        if let Err(e) = insert_tuple(file, &tuple_bytes) {
            println!("{}", format!("Failed to insert row {}: {}", i + 1, e).red());
        } else {
            inserted += 1;
        }
    }
    println!("{} {}", "✓".green(), format!("Total number of rows inserted: {}", inserted).green());
    Ok(())
}

/// Parse date string (YYYY-MM-DD) to days since Unix epoch (1970-01-01)
fn parse_date(s: &str) -> Option<i32> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: i32 = parts[1].parse().ok()?;
    let day: i32 = parts[2].parse().ok()?;
    
    // Simple calculation (doesn't account for all leap years perfectly, but works for basic use)
    let days_since_epoch = (year - 1970) * 365 + (year - 1969) / 4
        + match month {
            1 => 0,
            2 => 31,
            3 => 59,
            4 => 90,
            5 => 120,
            6 => 151,
            7 => 181,
            8 => 212,
            9 => 243,
            10 => 273,
            11 => 304,
            12 => 334,
            _ => return None,
        }
        + day - 1;
    Some(days_since_epoch)
}

/// Parse time string (HH:MM:SS) to seconds since midnight
fn parse_time(s: &str) -> Option<i32> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours: i32 = parts[0].parse().ok()?;
    let minutes: i32 = parts[1].parse().ok()?;
    let seconds: i32 = parts[2].parse().ok()?;
    Some(hours * 3600 + minutes * 60 + seconds)
}

/// Parse datetime string (YYYY-MM-DD HH:MM:SS) to Unix timestamp
fn parse_datetime(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    let days = parse_date(parts[0])? as i64;
    let time_seconds = parse_time(parts[1])? as i64;
    Some(days * 86400 + time_seconds)
}
