//! Handles database-related user commands such as listing, creating,
//! and selecting databases from the catalog.

use std::io::{self, Write};
use colored::Colorize;
use storage_manager::catalog::{Catalog, create_database, show_databases};

/// Displays all available databases
pub fn show_databases_cmd(catalog: &Catalog) {
    show_databases(catalog);
}

/// Creates a new database based on user input
pub fn create_database_cmd(catalog: &mut Catalog) -> io::Result<()> {
    let mut db_name = String::new();
    
    // Prompt for database name
    print!("{}", "Enter new database name: ".yellow().bold());
    io::stdout().flush()?;
    io::stdin().read_line(&mut db_name)?;

    // Validate and create database
    let db_name = db_name.trim();
    if db_name.is_empty() {
        println!("{}", "Database name cannot be empty.".red());
    } else if create_database(catalog, db_name) {
        println!("{} {}", "✓".green(), format!("Database '{}' created successfully.", db_name).green());
    } else {
        println!("{} {}", "✗".red(), format!("Failed to create database '{}'.", db_name).red());
    }
    Ok(())
}

/// Selects an existing database and updates the current context
pub fn select_database_cmd(
    catalog: &Catalog,
    current_db: &mut Option<String>,
) -> io::Result<()> {

    // Check if any databases exist
    if catalog.databases.is_empty() {
        println!("{}", "No databases found.".purple().bold());
        return Ok(());
    }

    // Display available databases
    println!("{}", "Available Databases:".bold().purple());
    for db in catalog.databases.keys() {
        println!("{} {}", "→".cyan(), db.bold().white());
    }

    // Read database name from user
    let mut db_name = String::new();
    print!("{}", "Enter database name: ".yellow().bold());
    io::stdout().flush()?;
    io::stdin().read_line(&mut db_name)?;


    let db_name = db_name.trim().to_string();
    // Update selected database
    if catalog.databases.contains_key(&db_name) {
        *current_db = Some(db_name.clone());
        println!("{} {}", "✓".green(), format!("Database '{}' selected.", db_name).green());
    } else {
        println!("{} {}", "✗".red(), format!("Database '{}' does not exist.", db_name).red());
    }

    Ok(())
}

/// Deletes an existing database from the catalog
pub fn delete_database_cmd(
    catalog: &mut Catalog,
    current_db: &mut Option<String>,
) -> io::Result<()> {

    // Check if any databases exist
    if catalog.databases.is_empty() {
        println!("{}", "No databases found.".purple().bold());
        return Ok(());
    }

    // Display available databases
    println!("{}", "Available Databases:".bold().purple());
    for db in catalog.databases.keys() {
        println!("{} {}", "→".cyan(), db.bold().white());
    }

    // Read database name from user
    let mut db_name = String::new();
    print!("{}", "Enter database name to delete (Enter '/cancel' to cancel): ".yellow().bold());
    io::stdout().flush()?;
    io::stdin().read_line(&mut db_name)?;

    if db_name.trim() == "/cancel" {
        println!("{}", "Database deletion cancelled.".green());
        return Ok(());
    }

    let db_name = db_name.trim().to_string();

    if db_name.is_empty() {
        println!("{}", "Database name cannot be empty.".red());
        return Ok(());
    }

    // Check if database exists
    if !catalog.databases.contains_key(&db_name) {
        println!("{} {}", "✗".red(), format!("Database '{}' does not exist.", db_name).red());
        return Ok(());
    }

    // Confirmation prompt
    print!("{}", format!("Are you sure you want to delete database '{}'? This will delete all tables. (y/n): ", db_name).yellow().bold());
    io::stdout().flush()?;
    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm)?;

    if confirm.trim().to_lowercase() != "y" {
        println!("{}", "Database deletion cancelled.".yellow());
        return Ok(());
    }

    // Clear current database if it's being deleted
    if let Some(current) = current_db.as_ref() {
        if current == &db_name {
            *current_db = None;
            println!("{}", "Current database selection cleared.".yellow());
        }
    }

    // Import delete_database function
    use storage_manager::catalog::delete_database;
    delete_database(catalog, &db_name);

    Ok(())
}
