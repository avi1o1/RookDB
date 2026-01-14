use std::fs::File;
use std::io::{self};
use colored::Colorize;

use crate::catalog::types::Catalog;
use crate::disk::{read_page, write_page};
use crate::page::{Page, PAGE_HEADER_SIZE, ITEM_ID_SIZE};
use crate::table::page_count;

/// Delete a tuple identified by row_id and return its data for display
pub fn delete_tuple(
    catalog: &Catalog,
    db_name: &str,
    table_name: &str,
    file: &mut File,
    tuple_id: u32,
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

    // 3. Locate the tuple by iterating through pages
    let mut current_row_id = 0u32;
    let mut found = false;
    
    for page_num in 1..total_pages {
        let mut page = Page::new();
        read_page(file, &mut page, page_num)?;
        
        let lower = u32::from_le_bytes(page.data[0..4].try_into().unwrap());
        let num_items = (lower - PAGE_HEADER_SIZE) / ITEM_ID_SIZE;
        
        for i in 0..num_items {
            if current_row_id == tuple_id {
                // Found the tuple! Get its data for display
                let base = (PAGE_HEADER_SIZE + i * ITEM_ID_SIZE) as usize;
                let offset = u32::from_le_bytes(page.data[base..base + 4].try_into().unwrap()) as usize;
                let length = u32::from_le_bytes(page.data[base + 4..base + 8].try_into().unwrap()) as usize;
                
                // Display the tuple before deletion
                print_tuple_for_deletion(&page, offset, length, columns, tuple_id)?;
                
                // 1. Compact tuple data region (defragmentation)
                let mut upper = u32::from_le_bytes(page.data[4..8].try_into().unwrap());
                let deleted_offset = offset as u32;
                let deleted_length = length as u32;
                
                // Shift all higher tuples down to reclaim the space
                let mut tuples_to_shift: Vec<(usize, u32, u32)> = Vec::new(); // (slot_index, offset, length)
                
                for j in 0..num_items {
                    if j == i {
                        continue; // Skip the tuple we're deleting
                    }
                    let slot_base = (PAGE_HEADER_SIZE + j * ITEM_ID_SIZE) as usize;
                    let tuple_offset = u32::from_le_bytes(page.data[slot_base..slot_base + 4].try_into().unwrap());
                    let tuple_length = u32::from_le_bytes(page.data[slot_base + 4..slot_base + 8].try_into().unwrap());
                    
                    // If this tuple is stored at or above the deleted tuple, it needs to shift down
                    if tuple_offset >= deleted_offset + deleted_length {
                        tuples_to_shift.push((j as usize, tuple_offset, tuple_length));
                    }
                }
                
                // Sort by offset (ascending) to shift in correct order
                tuples_to_shift.sort_by_key(|&(_, offset, _)| offset);
                
                // Shift each tuple's data down by deleted_length bytes
                for (_slot_idx, tuple_offset, tuple_length) in &tuples_to_shift {
                    let old_start = *tuple_offset as usize;
                    let new_start = (tuple_offset - deleted_length) as usize;
                    let len = *tuple_length as usize;
                    
                    // Use a temporary buffer to avoid overlap issues during copy
                    let tuple_data: Vec<u8> = page.data[old_start..old_start + len].to_vec();
                    page.data[new_start..new_start + len].copy_from_slice(&tuple_data);
                }
                
                // Update upper pointer
                upper += deleted_length;
                page.data[4..8].copy_from_slice(&upper.to_le_bytes());
                
                // 2. Update item slot offsets for shifted tuples
                for (slot_idx, old_offset, _) in tuples_to_shift {
                    let slot_base = (PAGE_HEADER_SIZE as usize + slot_idx * ITEM_ID_SIZE as usize);
                    let new_offset = old_offset - deleted_length;
                    page.data[slot_base..slot_base + 4].copy_from_slice(&new_offset.to_le_bytes());
                }
                
                // 3. Shift all item slots after the deleted one down
                let mut new_lower = lower;
                let item_slots = num_items;
                
                for j in (i + 1)..item_slots {
                    let src_base = (PAGE_HEADER_SIZE + j * ITEM_ID_SIZE) as usize;
                    let dst_base = (PAGE_HEADER_SIZE + (j - 1) * ITEM_ID_SIZE) as usize;
                    
                    // Copy the item slot
                    page.data.copy_within(src_base..src_base + ITEM_ID_SIZE as usize, dst_base);
                }
                
                // Update lower pointer (decrease by one item slot)
                new_lower -= ITEM_ID_SIZE;
                page.data[0..4].copy_from_slice(&new_lower.to_le_bytes());
                
                // Write the compacted page back to disk
                write_page(file, &mut page, page_num)?;
                
                println!("{}", format!("✓ Deleted tuple with 'row_id' {} from '{}.{}'", tuple_id, db_name, table_name).green().bold());
                // println!("{}", format!("Reclaimed {} bytes through page compaction", deleted_length).cyan());
                
                found = true;
                break;
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

/// Get tuple data for confirmation display
pub fn get_tuple_for_confirmation(
    catalog: &Catalog,
    db_name: &str,
    table_name: &str,
    file: &mut File,
    tuple_id: u32,
) -> io::Result<String> {
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

    // 3. Locate the tuple by iterating through pages
    let mut current_row_id = 0u32;
    
    for page_num in 1..total_pages {
        let mut page = Page::new();
        read_page(file, &mut page, page_num)?;
        
        let lower = u32::from_le_bytes(page.data[0..4].try_into().unwrap());
        let num_items = (lower - PAGE_HEADER_SIZE) / ITEM_ID_SIZE;
        
        for i in 0..num_items {
            if current_row_id == tuple_id {
                // Found the tuple!
                let base = (PAGE_HEADER_SIZE + i * ITEM_ID_SIZE) as usize;
                let offset = u32::from_le_bytes(page.data[base..base + 4].try_into().unwrap()) as usize;
                let length = u32::from_le_bytes(page.data[base + 4..base + 8].try_into().unwrap()) as usize;
                
                return format_tuple_display(&page, offset, length, columns, tuple_id);
            }
            
            current_row_id += 1;
        }
    }
    
    Err(io::Error::new(io::ErrorKind::NotFound, 
        format!("Entry with tuple_id {} not found", tuple_id)))
}

/// Format tuple data for display
fn format_tuple_display(
    page: &Page,
    offset: usize,
    length: usize,
    columns: &[crate::catalog::types::Column],
    tuple_id: u32,
) -> io::Result<String> {
    let tuple_data = &page.data[offset..offset + length];
    
    let mut output = String::new();
    
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
    
    output.push_str(&format!("╔{}╗\n", "═".repeat(total_width - 2)));
    
    // Print header
    output.push_str("║");
    output.push_str(&format!(" {:<width$} ", "row_id", width = col_widths[0]));
    output.push_str("║");
    for (i, col) in columns.iter().enumerate() {
        output.push_str(&format!(" {:<width$} ", col.name, width = col_widths[i + 1]));
        output.push_str("║");
    }
    output.push('\n');
    output.push_str(&format!("╠{}╣\n", "═".repeat(total_width - 2)));
    
    // Print tuple data
    output.push_str("║");
    output.push_str(&format!(" {:<width$} ", tuple_id, width = col_widths[0]));
    output.push_str("║");
    for (i, value) in values.iter().enumerate() {
        output.push_str(&format!(" {:<width$} ", value, width = col_widths[i + 1]));
        output.push_str("║");
    }
    output.push('\n');
    output.push_str(&format!("╚{}╝\n", "═".repeat(total_width - 2)));
    
    Ok(output)
}

/// Print a tuple before deletion
fn print_tuple_for_deletion(
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
    
    // let total_width: usize = col_widths.iter().sum::<usize>() + (col_widths.len() * 3) + 1;
    
    // println!("{}", "Tuple to be deleted:".bold().red());
    // println!("{}", format!("╔{}╗", "═".repeat(total_width - 2)).cyan());
    
    // // Print header
    // print!("{}", "║".cyan());
    // print!(" {:<width$} {}", "row_id".bold().yellow(), "║".cyan(), width = col_widths[0]);
    // for (i, col) in columns.iter().enumerate() {
    //     print!(" {:<width$} {}", col.name.bold().white(), "║".cyan(), width = col_widths[i + 1]);
    // }
    // println!();
    // println!("{}", format!("╠{}╣", "═".repeat(total_width - 2)).cyan());
    
    // // Print tuple data
    // print!("{}", "║".cyan());
    // print!(" {:<width$} {}", tuple_id.to_string().yellow(), "║".cyan(), width = col_widths[0]);
    // for (i, value) in values.iter().enumerate() {
    //     print!(" {:<width$} {}", value.red(), "║".cyan(), width = col_widths[i + 1]);
    // }
    // println!();
    // println!("{}", format!("╚{}╝", "═".repeat(total_width - 2)).cyan());
    
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
