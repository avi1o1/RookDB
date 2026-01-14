use std::fs::File;
use std::io::{self};
use colored::Colorize;

use crate::catalog::types::Catalog;
use crate::heap::insert_tuple;

/// Insert a new tuple with user-provided values
pub fn insert_tuple_manual(
    catalog: &Catalog,
    db_name: &str,
    table_name: &str,
    file: &mut File,
    values: &[String],
) -> io::Result<()> {
    // 1. Get schema from catalog
    let db = catalog
        .databases
        .get(db_name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("Database '{}' not found", db_name)))?;

    let table = db
        .tables
        .get(table_name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("Table '{}' not found", table_name)))?;

    let columns = &table.columns;

    // 2. Validate number of values matches number of columns
    if values.len() != columns.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Column count mismatch: Expected {} values but got {}",
                columns.len(),
                values.len()
            ),
        ));
    }

    // 3. Validate and encode each value based on column type
    let mut tuple_bytes: Vec<u8> = Vec::new();

    for (idx, col) in columns.iter().enumerate() {
        let value = &values[idx];

        match col.data_type.as_str() {
            "INT" => {
                let val = value
                    .parse::<i32>()
                    .map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!(
                                "Invalid INT value for column '{}': '{}'. Expected an integer.",
                                col.name, value
                            ),
                        )
                    })?;
                tuple_bytes.extend_from_slice(&val.to_le_bytes());
            }
            "FLOAT" => {
                let val = value
                    .parse::<f32>()
                    .map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!(
                                "Invalid FLOAT value for column '{}': '{}'. Expected a decimal number.",
                                col.name, value
                            ),
                        )
                    })?;
                tuple_bytes.extend_from_slice(&val.to_le_bytes());
            }
            "TEXT" => {
                if value.len() > 10 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "TEXT value too long for column '{}': '{}'. Maximum length is 10 characters.",
                            col.name, value
                        ),
                    ));
                }
                let mut text_bytes = vec![0u8; 10];
                let bytes = value.as_bytes();
                text_bytes[..bytes.len()].copy_from_slice(bytes);
                tuple_bytes.extend_from_slice(&text_bytes);
            }
            "BOOLEAN" => {
                let val = match value.to_lowercase().as_str() {
                    "true" | "1" | "yes" => true,
                    "false" | "0" | "no" => false,
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!(
                                "Invalid BOOLEAN value for column '{}': '{}'. Expected true/false, 1/0, or yes/no.",
                                col.name, value
                            ),
                        ))
                    }
                };
                tuple_bytes.push(if val { 1u8 } else { 0u8 });
            }
            "DATE" => {
                // Expected format: YYYY-MM-DD
                let parts: Vec<&str> = value.split('-').collect();
                if parts.len() != 3 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "Invalid DATE format for column '{}': '{}'. Expected YYYY-MM-DD.",
                            col.name, value
                        ),
                    ));
                }
                let year: i32 = parts[0].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid year in DATE for column '{}'", col.name),
                    )
                })?;
                let month: i32 = parts[1].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid month in DATE for column '{}'", col.name),
                    )
                })?;
                let day: i32 = parts[2].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid day in DATE for column '{}'", col.name),
                    )
                })?;

                if month < 1 || month > 12 || day < 1 || day > 31 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Date out of range for column '{}'", col.name),
                    ));
                }

                // Convert to days since epoch (1970-01-01)
                let mut days = 0;
                for y in 1970..year {
                    days += if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                        366
                    } else {
                        365
                    };
                }
                let month_days = [
                    31,
                    if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                        29
                    } else {
                        28
                    },
                    31,
                    30,
                    31,
                    30,
                    31,
                    31,
                    30,
                    31,
                    30,
                    31,
                ];
                for m in 0..(month - 1) {
                    days += month_days[m as usize];
                }
                days += day - 1;

                tuple_bytes.extend_from_slice(&days.to_le_bytes());
            }
            "TIME" => {
                // Expected format: HH:MM:SS
                let parts: Vec<&str> = value.split(':').collect();
                if parts.len() != 3 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "Invalid TIME format for column '{}': '{}'. Expected HH:MM:SS.",
                            col.name, value
                        ),
                    ));
                }
                let hours: i32 = parts[0].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid hours in TIME for column '{}'", col.name),
                    )
                })?;
                let minutes: i32 = parts[1].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid minutes in TIME for column '{}'", col.name),
                    )
                })?;
                let seconds: i32 = parts[2].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid seconds in TIME for column '{}'", col.name),
                    )
                })?;

                if hours < 0 || hours > 23 || minutes < 0 || minutes > 59 || seconds < 0 || seconds > 59 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Time out of range for column '{}'", col.name),
                    ));
                }

                let total_seconds = hours * 3600 + minutes * 60 + seconds;
                tuple_bytes.extend_from_slice(&total_seconds.to_le_bytes());
            }
            "DATETIME" => {
                // Expected format: YYYY-MM-DD HH:MM:SS
                let parts: Vec<&str> = value.split(' ').collect();
                if parts.len() != 2 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "Invalid DATETIME format for column '{}': '{}'. Expected YYYY-MM-DD HH:MM:SS.",
                            col.name, value
                        ),
                    ));
                }

                // Parse date part
                let date_parts: Vec<&str> = parts[0].split('-').collect();
                if date_parts.len() != 3 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid date in DATETIME for column '{}'", col.name),
                    ));
                }
                let year: i32 = date_parts[0].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid year in DATETIME for column '{}'", col.name),
                    )
                })?;
                let month: i32 = date_parts[1].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid month in DATETIME for column '{}'", col.name),
                    )
                })?;
                let day: i32 = date_parts[2].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid day in DATETIME for column '{}'", col.name),
                    )
                })?;

                // Parse time part
                let time_parts: Vec<&str> = parts[1].split(':').collect();
                if time_parts.len() != 3 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid time in DATETIME for column '{}'", col.name),
                    ));
                }
                let hours: i32 = time_parts[0].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid hours in DATETIME for column '{}'", col.name),
                    )
                })?;
                let minutes: i32 = time_parts[1].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid minutes in DATETIME for column '{}'", col.name),
                    )
                })?;
                let seconds: i32 = time_parts[2].parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid seconds in DATETIME for column '{}'", col.name),
                    )
                })?;

                // Convert date to days
                let mut days: i64 = 0;
                for y in 1970..year {
                    days += if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                        366
                    } else {
                        365
                    };
                }
                let month_days = [
                    31,
                    if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                        29
                    } else {
                        28
                    },
                    31,
                    30,
                    31,
                    30,
                    31,
                    31,
                    30,
                    31,
                    30,
                    31,
                ];
                for m in 0..(month - 1) {
                    days += month_days[m as usize] as i64;
                }
                days += (day - 1) as i64;

                // Convert time to seconds
                let total_seconds = (hours * 3600 + minutes * 60 + seconds) as i64;

                // Combine into timestamp
                let timestamp = days * 86400 + total_seconds;
                tuple_bytes.extend_from_slice(&timestamp.to_le_bytes());
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unsupported data type: {} for column '{}'", col.data_type, col.name),
                ));
            }
        }
    }

    // 4. Insert the tuple using the existing low-level insert function
    insert_tuple(file, &tuple_bytes)?;

    println!(
        "{}",
        format!("✓ Successfully inserted new tuple into '{}.{}'", db_name, table_name)
            .green()
            .bold()
    );

    Ok(())
}
