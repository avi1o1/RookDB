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

    // Calculate initial column widths based on column names
    let mut col_widths: Vec<usize> = columns.iter().map(|col| col.name.len()).collect();

    // First pass: scan all tuples to find max width needed for each column
    for page_num in 1..total_pages {
        let mut page = Page::new();
        read_page(file, &mut page, page_num)?;
        
        let lower = u32::from_le_bytes(page.data[0..4].try_into().unwrap());
        let num_items = (lower - PAGE_HEADER_SIZE) / ITEM_ID_SIZE;
        
        for i in 0..num_items {
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
                    _ => {
                        13 // "<unsupported>"
                    }
                };
                col_widths[col_idx] = col_widths[col_idx].max(value_len);
            }
        }
    }

    let total_width: usize = col_widths.iter().sum::<usize>() + (col_widths.len() * 3) + 1;

    // 3. Second pass: display the tuples with calculated widths
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

        // Print table header
        println!("{}", format!("╔{}╗", "═".repeat(total_width - 2)).cyan());
        print!("{}", "║".cyan());
        for (i, col) in columns.iter().enumerate() {
            print!(" {:<width$} {}", col.name.bold().white(), "║".cyan(), width = col_widths[i]);
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
                    _ => {
                        "<unsupported>".to_string()
                    }
                };
                
                print!(" {:<width$} {}", value.green(), "║".cyan(), width = col_widths[col_idx]);
            }
            println!();
        }
        
        // Print table footer
        println!("{}", format!("╚{}╝", "═".repeat(total_width - 2)).cyan());
    }
    
    println!("{}", "--- End of table tuples ---".bold().purple());
    Ok(())
}