use std::fs::File;
use std::io::{self};
use colored::Colorize;

use crate::catalog::types::Catalog;
use crate::disk::{read_page, write_page};
use crate::page::{Page, PAGE_HEADER_SIZE, ITEM_ID_SIZE};
use crate::table::page_count;

/// Update a specific column in a tuple identified by tuple_id (row_id)
pub fn update_tuple(
    catalog: &Catalog,
    db_name: &str,
    table_name: &str,
    file: &mut File,
    tuple_id: u32,
    column_name: &str,
    new_value: &str,
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

    // 2. Find the column index and data type
    let (col_idx, col_type) = columns
        .iter()
        .enumerate()
        .find(|(_, col)| col.name == column_name)
        .map(|(idx, col)| (idx, col.data_type.as_str()))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("Column '{}' not found", column_name)))?;

    // 3. Validate and parse the new value based on column type
    let new_bytes = match col_type {
        "INT" => {
            let val = new_value.parse::<i32>()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, 
                    format!("Invalid INT value: '{}'. Expected an integer.", new_value)))?;
            val.to_le_bytes().to_vec()
        }
        "FLOAT" => {
            let val = new_value.parse::<f32>()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, 
                    format!("Invalid FLOAT value: '{}'. Expected a decimal number.", new_value)))?;
            val.to_le_bytes().to_vec()
        }
        "TEXT" => {
            if new_value.len() > 10 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, 
                    format!("TEXT value too long: '{}'. Maximum length is 10 characters.", new_value)));
            }
            let mut text_bytes = vec![0u8; 10];
            let bytes = new_value.as_bytes();
            text_bytes[..bytes.len()].copy_from_slice(bytes);
            text_bytes
        }
        "BOOLEAN" => {
            let val = match new_value.to_lowercase().as_str() {
                "true" | "1" | "yes" => true,
                "false" | "0" | "no" => false,
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, 
                    format!("Invalid BOOLEAN value: '{}'. Expected true/false, 1/0, or yes/no.", new_value))),
            };
            vec![if val { 1u8 } else { 0u8 }]
        }
        "DATE" => {
            // Expected format: YYYY-MM-DD
            let parts: Vec<&str> = new_value.split('-').collect();
            if parts.len() != 3 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, 
                    format!("Invalid DATE format: '{}'. Expected YYYY-MM-DD.", new_value)));
            }
            let year: i32 = parts[0].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid year"))?;
            let month: i32 = parts[1].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid month"))?;
            let day: i32 = parts[2].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid day"))?;
            
            if month < 1 || month > 12 || day < 1 || day > 31 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Date out of range"));
            }
            
            // Convert to days since epoch (1970-01-01)
            let mut days = 0;
            for y in 1970..year {
                days += if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
            }
            let month_days = [31, if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            for m in 0..(month - 1) {
                days += month_days[m as usize];
            }
            days += day - 1;
            
            days.to_le_bytes().to_vec()
        }
        "TIME" => {
            // Expected format: HH:MM:SS
            let parts: Vec<&str> = new_value.split(':').collect();
            if parts.len() != 3 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, 
                    format!("Invalid TIME format: '{}'. Expected HH:MM:SS.", new_value)));
            }
            let hours: i32 = parts[0].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid hours"))?;
            let minutes: i32 = parts[1].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid minutes"))?;
            let seconds: i32 = parts[2].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid seconds"))?;
            
            if hours < 0 || hours > 23 || minutes < 0 || minutes > 59 || seconds < 0 || seconds > 59 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Time out of range"));
            }
            
            let total_seconds = hours * 3600 + minutes * 60 + seconds;
            total_seconds.to_le_bytes().to_vec()
        }
        "DATETIME" => {
            // Expected format: YYYY-MM-DD HH:MM:SS
            let parts: Vec<&str> = new_value.split(' ').collect();
            if parts.len() != 2 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, 
                    format!("Invalid DATETIME format: '{}'. Expected YYYY-MM-DD HH:MM:SS.", new_value)));
            }
            
            // Parse date part
            let date_parts: Vec<&str> = parts[0].split('-').collect();
            if date_parts.len() != 3 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid date in DATETIME"));
            }
            let year: i32 = date_parts[0].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid year"))?;
            let month: i32 = date_parts[1].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid month"))?;
            let day: i32 = date_parts[2].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid day"))?;
            
            // Parse time part
            let time_parts: Vec<&str> = parts[1].split(':').collect();
            if time_parts.len() != 3 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid time in DATETIME"));
            }
            let hours: i32 = time_parts[0].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid hours"))?;
            let minutes: i32 = time_parts[1].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid minutes"))?;
            let seconds: i32 = time_parts[2].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid seconds"))?;
            
            // Convert date to days
            let mut days: i64 = 0;
            for y in 1970..year {
                days += if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
            }
            let month_days = [31, if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            for m in 0..(month - 1) {
                days += month_days[m as usize] as i64;
            }
            days += (day - 1) as i64;
            
            // Convert time to seconds
            let total_seconds = (hours * 3600 + minutes * 60 + seconds) as i64;
            
            // Combine into timestamp
            let timestamp = days * 86400 + total_seconds;
            timestamp.to_le_bytes().to_vec()
        }
        _ => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, 
                format!("Unsupported data type: {}", col_type)));
        }
    };

    // 4. Read total number of pages
    let total_pages = page_count(file)?;

    // 5. Locate the tuple by iterating through pages
    let mut current_row_id = 0u32;
    let mut found = false;
    
    for page_num in 1..total_pages {
        let mut page = Page::new();
        read_page(file, &mut page, page_num)?;
        
        let lower = u32::from_le_bytes(page.data[0..4].try_into().unwrap());
        let num_items = (lower - PAGE_HEADER_SIZE) / ITEM_ID_SIZE;
        
        for i in 0..num_items {
            if current_row_id == tuple_id {
                // Found the tuple! Now update it
                let base = (PAGE_HEADER_SIZE + i * ITEM_ID_SIZE) as usize;
                let offset = u32::from_le_bytes(page.data[base..base + 4].try_into().unwrap()) as usize;
                let length = u32::from_le_bytes(page.data[base + 4..base + 8].try_into().unwrap()) as usize;
                
                // Calculate the offset within the tuple for the target column
                let mut tuple_offset = 0usize;
                for (idx, col) in columns.iter().enumerate() {
                    if idx == col_idx {
                        // Update the value at this position
                        let col_offset = offset + tuple_offset;
                        
                        // Ensure we have enough space
                        if col_offset + new_bytes.len() > page.data.len() {
                            return Err(io::Error::new(io::ErrorKind::InvalidInput, 
                                "Column update would exceed page boundaries"));
                        }
                        
                        page.data[col_offset..col_offset + new_bytes.len()].copy_from_slice(&new_bytes);
                        
                        // Write the updated page back to disk
                        write_page(file, &mut page, page_num)?;
                        
                        println!("{}", format!("✓ Updated tuple {} in '{}.{}'", tuple_id, db_name, table_name).green().bold());
                        
                        // Now print the entire updated tuple
                        print_updated_tuple(&page, offset, length, columns, tuple_id)?;
                        
                        found = true;
                        break;
                    }
                    
                    // Advance tuple_offset by the size of this column
                    tuple_offset += match col.data_type.as_str() {
                        "INT" | "FLOAT" | "DATE" | "TIME" => 4,
                        "BOOLEAN" => 1,
                        "TEXT" => 10,
                        "DATETIME" => 8,
                        _ => 0,
                    };
                }
                
                if found {
                    break;
                }
            }
            
            current_row_id += 1;
        }
        
        if found {
            break;
        }
    }
    
    if !found {
        return Err(io::Error::new(io::ErrorKind::NotFound, 
            format!("Entry with tuple_id {} not found", tuple_id)));
    }
    
    Ok(())
}

/// Print a single updated tuple in a formatted table
fn print_updated_tuple(
    page: &Page,
    offset: usize,
    length: usize,
    columns: &[crate::catalog::types::Column],
    tuple_id: u32,
) -> io::Result<()> {
    let tuple_data = &page.data[offset..offset + length];
    
    // Calculate column widths
    let mut col_widths: Vec<usize> = vec![6]; // "row_id"
    col_widths.extend(columns.iter().map(|col| col.name.len()));
    
    // Decode tuple and update widths
    let mut cursor = 0usize;
    let mut values: Vec<String> = Vec::new();
    
    for (col_idx, col) in columns.iter().enumerate() {
        let value = match col.data_type.as_str() {
            "INT" => {
                if cursor + 4 <= tuple_data.len() {
                    let val = i32::from_le_bytes(tuple_data[cursor..cursor + 4].try_into().unwrap());
                    cursor += 4;
                    val.to_string()
                } else {
                    cursor += 4;
                    "NULL".to_string()
                }
            }
            "FLOAT" => {
                if cursor + 4 <= tuple_data.len() {
                    let val = f32::from_le_bytes(tuple_data[cursor..cursor + 4].try_into().unwrap());
                    cursor += 4;
                    format!("{:.2}", val)
                } else {
                    cursor += 4;
                    "NULL".to_string()
                }
            }
            "TEXT" => {
                if cursor + 10 <= tuple_data.len() {
                    let text_bytes = &tuple_data[cursor..cursor + 10];
                    let text = String::from_utf8_lossy(text_bytes).trim_end_matches('\0').to_string();
                    cursor += 10;
                    text
                } else {
                    cursor += 10;
                    "NULL".to_string()
                }
            }
            "BOOLEAN" => {
                if cursor + 1 <= tuple_data.len() {
                    let bool_val = tuple_data[cursor] != 0;
                    cursor += 1;
                    bool_val.to_string()
                } else {
                    cursor += 1;
                    "NULL".to_string()
                }
            }
            "DATE" => {
                if cursor + 4 <= tuple_data.len() {
                    let days = i32::from_le_bytes(tuple_data[cursor..cursor + 4].try_into().unwrap());
                    cursor += 4;
                    format_date(days)
                } else {
                    cursor += 4;
                    "NULL".to_string()
                }
            }
            "TIME" => {
                if cursor + 4 <= tuple_data.len() {
                    let seconds = i32::from_le_bytes(tuple_data[cursor..cursor + 4].try_into().unwrap());
                    cursor += 4;
                    format_time(seconds)
                } else {
                    cursor += 4;
                    "NULL".to_string()
                }
            }
            "DATETIME" => {
                if cursor + 8 <= tuple_data.len() {
                    let timestamp = i64::from_le_bytes(tuple_data[cursor..cursor + 8].try_into().unwrap());
                    cursor += 8;
                    format_datetime(timestamp)
                } else {
                    cursor += 8;
                    "NULL".to_string()
                }
            }
            _ => "<unsupported>".to_string(),
        };
        
        col_widths[col_idx + 1] = col_widths[col_idx + 1].max(value.len());
        values.push(value);
    }
    
    // Update row_id width
    col_widths[0] = col_widths[0].max(tuple_id.to_string().len());
    
    let total_width: usize = col_widths.iter().sum::<usize>() + (col_widths.len() * 3) + 1;
    
    println!();
    println!("{}", "Updated Tuple:".bold().cyan());
    println!("{}", format!("╔{}╗", "═".repeat(total_width - 2)).cyan());
    
    // Print header
    print!("{}", "║".cyan());
    print!(" {:<width$} {}", "row_id".bold().yellow(), "║".cyan(), width = col_widths[0]);
    for (i, col) in columns.iter().enumerate() {
        print!(" {:<width$} {}", col.name.bold().white(), "║".cyan(), width = col_widths[i + 1]);
    }
    println!();
    println!("{}", format!("╠{}╣", "═".repeat(total_width - 2)).cyan());
    
    // Print tuple data
    print!("{}", "║".cyan());
    print!(" {:<width$} {}", tuple_id.to_string().yellow(), "║".cyan(), width = col_widths[0]);
    for (i, value) in values.iter().enumerate() {
        print!(" {:<width$} {}", value.green(), "║".cyan(), width = col_widths[i + 1]);
    }
    println!();
    println!("{}", format!("╚{}╝", "═".repeat(total_width - 2)).cyan());
    
    Ok(())
}

/// Format days since epoch to YYYY-MM-DD
fn format_date(days: i32) -> String {
    let mut year = 1970;
    let mut remaining_days = days;
    
    loop {
        let year_days = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 366 } else { 365 };
        if remaining_days < year_days {
            break;
        }
        remaining_days -= year_days;
        year += 1;
    }
    
    let month_days = [31, if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1;
    for &days_in_month in &month_days {
        if remaining_days < days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }
    
    let day = remaining_days + 1;
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// Format seconds since midnight to HH:MM:SS
fn format_time(seconds: i32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
}

/// Format Unix timestamp to YYYY-MM-DD HH:MM:SS
fn format_datetime(timestamp: i64) -> String {
    let days = (timestamp / 86400) as i32;
    let time_seconds = (timestamp % 86400) as i32;
    format!("{} {}", format_date(days), format_time(time_seconds))
}
