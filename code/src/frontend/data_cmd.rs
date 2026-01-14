use std::io::{self, Write};
use std::fs::OpenOptions;
use colored::Colorize;

use storage_manager::catalog::load_catalog;
use storage_manager::buffer_manager::BufferManager;
use storage_manager::table::page_count;
use storage_manager::executor::{show_tuples, update_tuple};

pub fn load_csv_cmd(
    buffer_manager: &mut BufferManager,
    current_db: &Option<String>,
) -> io::Result<()> {
    let db = match current_db {
        Some(db) => db.clone(),
        None => {
            println!("{}", "No database selected. Please select a database first.".purple().bold());
            return Ok(());
        }
    };

    let mut table = String::new();
    print!("{}", "Enter table name: ".yellow().bold());
    io::stdout().flush()?;
    io::stdin().read_line(&mut table)?;
    let table = table.trim();
    
    let catalog = load_catalog();
    if let Some(database) = catalog.databases.get(&db) {
        if !database.tables.contains_key(table) {
            println!("{}", format!("✗ Table '{}' does not exist in database '{}'.", table, db).red());
            return Ok(());
        }
    } else {
        println!("{}", format!("✗ Database '{}' not found in catalog.", db).red());
        return Ok(());
    }

    let mut csv_path = String::new();
    print!("{}", "Enter CSV path: ".yellow().bold());
    io::stdout().flush()?;
    io::stdin().read_line(&mut csv_path)?;
    let csv_path = csv_path.trim();

    // check if file exists
    if !std::path::Path::new(csv_path).exists() {
        println!("{}", format!("✗ CSV file not found at path: {}", csv_path).red());
        return Ok(());
    }

    let catalog = load_catalog();
    buffer_manager.load_csv_to_buffer(&catalog, &db, table, csv_path)?;

    let path = format!("database/base/{}/{}.dat", db, table);
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    println!("{} {}", "Page Count:".bold().cyan(), page_count(&mut file)?.to_string().bold().white());

    Ok(())
}

pub fn show_tuples_cmd(current_db: &Option<String>) -> io::Result<()> {
    let db = match current_db {
        Some(db) => db.clone(),
        None => {
            println!("{}", "No database selected. Please select a database first.".purple().bold());
            return Ok(());
        }
    };

    let mut table = String::new();
    print!("{}", "Enter table name: ".yellow().bold());
    io::stdout().flush()?;
    io::stdin().read_line(&mut table)?;
    let table = table.trim();

    // Check if table exists in catalog
    let catalog = load_catalog();
    
    if let Some(database) = catalog.databases.get(&db) {
        if !database.tables.contains_key(table) {
            println!("{}", format!("✗ Table '{}' does not exist in database '{}'.", table, db).red());
            return Ok(());
        }
    } else {
        println!("{}", format!("Database '{}' not found in catalog.", db).red());
        return Ok(());
    }

    let path = format!("database/base/{}/{}.dat", db, table);
    
    // Check if the file exists
    if !std::path::Path::new(&path).exists() {
        println!("{}", format!("Table file not found at path: {}", path).red());
        return Ok(());
    }
    
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;

    show_tuples(&catalog, &db, table, &mut file)?;

    Ok(())
}

pub fn update_tuple_cmd(current_db: &Option<String>) -> io::Result<()> {
    let db = match current_db {
        Some(db) => db.clone(),
        None => {
            println!("{}", "No database selected. Please select a database first.".purple().bold());
            return Ok(());
        }
    };

    let mut table = String::new();
    print!("{}", "Enter table name: ".yellow().bold());
    io::stdout().flush()?;
    io::stdin().read_line(&mut table)?;
    let table = table.trim().to_string();

    // Check if table exists in catalog
    let catalog = load_catalog();
    
    if let Some(database) = catalog.databases.get(&db) {
        if !database.tables.contains_key(&table) {
            println!("{}", format!("✗ Table '{}' does not exist in database '{}'.", table, db).red());
            return Ok(());
        }
    } else {
        println!("{}", format!("✗ Database '{}' not found in catalog.", db).red());
        return Ok(());
    }

    let path = format!("database/base/{}/{}.dat", db, table);
    
    // Check if the file exists
    if !std::path::Path::new(&path).exists() {
        println!("{}", format!("✗ Table file not found at path: {}", path).red());
        return Ok(());
    }

    // Get tuple_id (row_id)
    let mut tuple_id_str = String::new();
    print!("{}", "Enter tuple_id (row_id): ".yellow().bold());
    io::stdout().flush()?;
    io::stdin().read_line(&mut tuple_id_str)?;
    let tuple_id = match tuple_id_str.trim().parse::<u32>() {
        Ok(id) => id,
        Err(_) => {
            println!("{}", "✗ Invalid tuple_id. Must be a non-negative integer.".red());
            return Ok(());
        }
    };

    // Get column name
    let mut column_name = String::new();
    print!("{}", "Enter column name to update: ".yellow().bold());
    io::stdout().flush()?;
    io::stdin().read_line(&mut column_name)?;
    let column_name = column_name.trim().to_string();

    // Verify column exists and get its type
    let db_ref = catalog.databases.get(&db).unwrap();
    let table_ref = db_ref.tables.get(&table).unwrap();
    
    let column_info = table_ref.columns.iter().find(|col| col.name == column_name);
    
    if column_info.is_none() {
        println!("{}", format!("✗ Column '{}' does not exist in table '{}'.", column_name, table).red());
        return Ok(());
    }
    
    let col_type = &column_info.unwrap().data_type;
    
    // Get new value with data type hint
    let mut new_value = String::new();
    print!("{}", format!("Enter new value for column '{}' (type: {}): ", column_name, col_type).yellow().bold());
    io::stdout().flush()?;
    io::stdin().read_line(&mut new_value)?;
    let new_value = new_value.trim().to_string();

    // Open the table file
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;

    // Call the update_tuple function
    match update_tuple(&catalog, &db, &table, &mut file, tuple_id, &column_name, &new_value) {
        Ok(_) => {
            // Success message is printed by the update_tuple function
        }
        Err(e) => {
            println!("{}", format!("✗ Failed to update tuple: {}", e).red());
        }
    }

    Ok(())
}