use std::io::{self, Write};
use std::fs::OpenOptions;
use colored::Colorize;

use storage_manager::catalog::load_catalog;
use storage_manager::buffer_manager::BufferManager;
use storage_manager::table::page_count;
use storage_manager::executor::show_tuples;

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