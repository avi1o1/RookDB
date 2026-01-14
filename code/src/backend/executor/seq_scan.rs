use std::fs::File;
use std::io::{self};
use colored::Colorize;

use crate::catalog::types::Catalog;
use crate::disk::read_page;
use crate::page::{Page, PAGE_HEADER_SIZE, ITEM_ID_SIZE};
use crate::table::page_count;

pub fn show_tuples(
    catalog: &Catalog,
    db_name: &str,
    table_name: &str,
    file: &mut File,
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

    // 2. Read total number of pages
    let total_pages = page_count(file)?;

    println!("{}", format!("Tuples in '{}.{}': ({} total pages)", db_name, table_name, total_pages).bold().purple());

    // Calculate initial column widths
    let mut col_widths: Vec<usize> = vec![6]; // Start with "row_id" width
    col_widths.extend(columns.iter().map(|col| col.name.len()));

    // First pass: scan all tuples to find max width needed for each column
    let mut global_row_id = 0u32;
    for page_num in 1..total_pages {
        let mut page = Page::new();
        read_page(file, &mut page, page_num)?;
        
        let lower = u32::from_le_bytes(page.data[0..4].try_into().unwrap());
        let num_items = (lower - PAGE_HEADER_SIZE) / ITEM_ID_SIZE;
        
        for i in 0..num_items {
            // Calculate row_id width
            let row_id_str = global_row_id.to_string();
            col_widths[0] = col_widths[0].max(row_id_str.len());
            global_row_id += 1;
            
            let base = (PAGE_HEADER_SIZE + i * ITEM_ID_SIZE) as usize;
            let offset = u32::from_le_bytes(page.data[base..base + 4].try_into().unwrap());
            let length = u32::from_le_bytes(page.data[base + 4..base + 8].try_into().unwrap());
            let tuple_data = &page.data[offset as usize..(offset + length) as usize];
            
            let mut cursor = 0usize;
            for (col_idx, col) in columns.iter().enumerate() {
                let value_len = match col.data_type.as_str() {
                    "INT" => {
                        if cursor + 4 <= tuple_data.len() {
                            let val = i32::from_le_bytes(tuple_data[cursor..cursor + 4].try_into().unwrap());
                            cursor += 4;
                            val.to_string().len()
                        } else {
                            cursor += 4;
                            4 // "NULL"
                        }
                    }
                    "FLOAT" => {
                        if cursor + 4 <= tuple_data.len() {
                            let val = f32::from_le_bytes(tuple_data[cursor..cursor + 4].try_into().unwrap());
                            cursor += 4;
                            format!("{:.2}", val).len()
                        } else {
                            cursor += 4;
                            4 // "NULL"
                        }
                    }
                    "TEXT" => {
                        if cursor + 10 <= tuple_data.len() {
                            let text_bytes = &tuple_data[cursor..cursor + 10];
                            let text = String::from_utf8_lossy(text_bytes).trim_end_matches('\0').to_string();
                            cursor += 10;
                            text.len()
                        } else {
                            cursor += 10;
                            4 // "NULL"
                        }
                    }
                    "BOOLEAN" => {
                        if cursor + 1 <= tuple_data.len() {
                            let bool_val = tuple_data[cursor] != 0;
                            cursor += 1;
                            bool_val.to_string().len() // 4 or 5 ("true" or "false")
                        } else {
                            cursor += 1;
                            4 // "NULL"
                        }
                    }
                    "DATE" => {
                        cursor += 4;
                        10 // "YYYY-MM-DD"
                    }
                    "TIME" => {
                        cursor += 4;
                        8 // "HH:MM:SS"
                    }
                    "DATETIME" => {
                        cursor += 8;
                        19 // "YYYY-MM-DD HH:MM:SS"
                    }
                    _ => {
                        13 // "<unsupported>"
                    }
                };
                col_widths[col_idx + 1] = col_widths[col_idx + 1].max(value_len);
            }
        }
    }

    let total_width: usize = col_widths.iter().sum::<usize>() + (col_widths.len() * 3) + 1;

    // 3. Second pass: display the tuples with calculated widths
    let mut global_row_id = 0u32;
    for page_num in 1..total_pages {
        let mut page = Page::new();
        read_page(file, &mut page, page_num)?;
        
        let lower = u32::from_le_bytes(page.data[0..4].try_into().unwrap());
        let upper = u32::from_le_bytes(page.data[4..8].try_into().unwrap());
        let num_items = (lower - PAGE_HEADER_SIZE) / ITEM_ID_SIZE;
        
        println!("{}", format!("Page {} (Tuples: {}, Lower: {}, Upper: {})", page_num, num_items, lower, upper).bold().blue());
        
        if num_items == 0 {
            continue;
        }

        // Print table header with row_id column
        println!("{}", format!("╔{}╗", "═".repeat(total_width - 2)).cyan());
        print!("{}", "║".cyan());
        print!(" {:<width$} {}", "row_id".bold().yellow(), "║".cyan(), width = col_widths[0]);
        for (i, col) in columns.iter().enumerate() {
            print!(" {:<width$} {}", col.name.bold().white(), "║".cyan(), width = col_widths[i + 1]);
        }
        println!();
        println!("{}", format!("╠{}╣", "═".repeat(total_width - 2)).cyan());

        // 4. For each tuple
        for i in 0..num_items {
            let base = (PAGE_HEADER_SIZE + i * ITEM_ID_SIZE) as usize;
            let offset = u32::from_le_bytes(page.data[base..base + 4].try_into().unwrap());
            let length = u32::from_le_bytes(page.data[base + 4..base + 8].try_into().unwrap());
            let tuple_data = &page.data[offset as usize..(offset + length) as usize];

            print!("{}", "║".cyan());
            
            // Print row_id (sequential integer across all pages)
            print!(" {:<width$} {}", global_row_id.to_string().yellow(), "║".cyan(), width = col_widths[0]);
            global_row_id += 1;

            // 5. Decode each column
            let mut cursor = 0usize;
            for (col_idx, col) in columns.iter().enumerate() {
                let value = match col.data_type.as_str() {
                    "INT" => {
                        if cursor + 4 <= tuple_data.len() {
                            let val = i32::from_le_bytes(tuple_data[cursor..cursor + 4].try_into().unwrap());
                            cursor += 4;
                            format!("{}", val)
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
                            format!("{}", text)
                        } else {
                            cursor += 10;
                            "NULL".to_string()
                        }
                    }
                    "BOOLEAN" => {
                        if cursor + 1 <= tuple_data.len() {
                            let bool_val = tuple_data[cursor] != 0;
                            cursor += 1;
                            format!("{}", bool_val)
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
                    _ => {
                        "<unsupported>".to_string()
                    }
                };
                
                print!(" {:<width$} {}", value.green(), "║".cyan(), width = col_widths[col_idx + 1]);
            }
            println!();
        }
        
        // Print table footer
        println!("{}", format!("╚{}╝", "═".repeat(total_width - 2)).cyan());
    }
    
    println!("{}", "--- End of table tuples ---".bold().purple());
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