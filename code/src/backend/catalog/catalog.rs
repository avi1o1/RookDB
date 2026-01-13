//! Manages catalog metadata including databases, tables, and columns.
//! Handles persistence of catalog state and creation of physical
//! database and table structures on disk.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::path::Path;
use colored::Colorize;

use crate::catalog::types::*;

use crate::heap::init_table;
use crate::layout::*;

/// Initializes the catalog and required directory structure on disk.
/// Creates the catalog file if it does not already exist.
pub fn init_catalog() {
    let catalog_path = Path::new(CATALOG_FILE);

    // Create directory if not exist
    if let Some(parent) = catalog_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).expect("Failed to create catalog directory");
        }
    }

    // Ensure base database directory exists
    let base_dir = Path::new(DATABASE_DIR);
    if !base_dir.exists() {
        fs::create_dir_all(base_dir).expect("Failed to create base data directory");
    }

    // Create an empty catalog file if missing
    if !catalog_path.exists() {
        let empty_catalog = Catalog {
            databases: HashMap::new(),
        };
        let json = serde_json::to_string_pretty(&empty_catalog)
            .expect("Failed to serialize empty catalog");
        fs::write(catalog_path, json).expect("Failed to write catalog file");
        println!(
            "{}",
            format!("Created new catalog file at {}", catalog_path.display()).cyan()
        );
    } else {
        println!("{}", format!("Catalog file already exists at {}", catalog_path.display()).cyan());
    }
}

/// Loads the catalog from disk into memory.
/// Returns an empty catalog if the file is missing or invalid.
pub fn load_catalog() -> Catalog {
    let catalog_path = Path::new(CATALOG_FILE);

    // Check if catalog file exists
    if !catalog_path.exists() {
        eprintln!("{}", format!("Catalog file does not exist at {}.", catalog_path.display()).red());
        return Catalog {
            databases: HashMap::new(),
        };
    }

    // Read the catalog file
    let data = fs::read_to_string(catalog_path);
    let data = match data {
        Ok(content) => content,
        Err(err) => {
            eprintln!("{}", format!("Failed to read catalog file: {}", err).red());
            return Catalog {
                databases: HashMap::new(),
            };
        }
    };

    // Deserialize JSON into Catalog struct
    match serde_json::from_str::<Catalog>(&data) {
        Ok(catalog) => {
            catalog
        }
        Err(err) => {
            eprintln!("{}", format!("Failed to parse catalog JSON: {}", err).red());
            Catalog {
                databases: HashMap::new(),
            }
        }
    }
}

// Persists the in-memory catalog state to disk.
pub fn save_catalog(catalog: &Catalog) {
    let catalog_path = Path::new(CATALOG_FILE);

    // Convert catalog to formatted JSON
    let json = serde_json::to_string_pretty(catalog).expect("Failed to serialize catalog to JSON");

    // Write catalog to disk
    fs::write(catalog_path, json).expect("Failed to write catalog file to disk");

    println!("{}", format!("Catalog file updated at {}", catalog_path.display()).cyan());
}

// Prints all databases present in the catalog.
pub fn show_databases(catalog: &Catalog) {
    
    if catalog.databases.is_empty() {
        println!("{}", "No databases found.".purple().bold());
        return;
    }
    
    println!("{}", "Databases in Catalog:".bold().purple());
    for db_name in catalog.databases.keys() {
        println!("{} {}", "→".cyan(), db_name.bold().white());
    }
}

// Creates a new database entry in the catalog and its directory on disk.
pub fn create_database(catalog: &mut Catalog, db_name: &str) -> bool {
     // Validate database name
    if db_name.is_empty() {
        println!("{}", "Database name cannot be empty.".red());
        return false;
    }

    if catalog.databases.contains_key(db_name) {
        println!("{}", format!("Database '{}' already exists.", db_name).red());
        return false;
    }

    // Insert database into in-memory catalog
    catalog.databases.insert(
        db_name.to_string(),
        Database {
            tables: HashMap::new(),
        },
    );

     // Persist updated catalog
    let json = match serde_json::to_string_pretty(&catalog) {
        Ok(j) => j,
        Err(e) => {
            println!("{}", format!("Failed to serialize catalog: {}", e).red());
            return false;
        }
    };

    if let Err(e) = fs::write(CATALOG_FILE, json) {
        println!("{}", format!("Failed to write catalog file: {}", e).red());
        return false;
    }

    // Create database directory on disk
    let db_path_str = TABLE_DIR_TEMPLATE.replace("{database}", db_name);
    let db_path = Path::new(&db_path_str);

    if !db_path.exists() {
        if let Err(e) = fs::create_dir_all(db_path) {
            println!("{}", format!("Failed to create database directory: {}", e).red());
            return false;
        }
        // println!("Created new database directory at {}", db_path.display());
    } else {
        println!("{} {}", "→".yellow(), format!("Database directory already exists at {}", db_path.display()).cyan());
    }

    // println!("Database '{}' created successfully", db_name);
    true
}


// Creates a new table, updates the catalog, and initializes its data file.
pub fn create_table(catalog: &mut Catalog, db_name: &str, table_name: &str, columns: Vec<Column>) {
    // Step 1: Validate database existence
    if !catalog.databases.contains_key(db_name) {
        println!(
            "{}",
            format!("Database '{}' does not exist. Cannot create table '{}'.", db_name, table_name).red()
        );
        return;
    }

    let database = catalog.databases.get_mut(db_name).unwrap();

    // Prevent overwriting existing table
    if database.tables.contains_key(table_name) {
        println!(
            "{}",
            format!("Table '{}' already exists in database '{}'. Skipping creation.", table_name, db_name).yellow()
        );
        return;
    }

    // Insert table metadata into catalog
    let new_table = Table { columns };
    database.tables.insert(table_name.to_string(), new_table);

   // Persist catalog changes
    save_catalog(catalog);

    // Construct table file path
    let table_file_path = TABLE_FILE_TEMPLATE
        .replace("{database}", db_name)
        .replace("{table}", table_name);

    // Create and initialize table file
    let table_path = Path::new(&table_file_path);
    if !table_path.exists() {
        match OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .truncate(true)
            .open(&table_file_path)
        {
            Ok(mut file) => {
                println!("{} {}", "→".yellow(), format!("Table data file created at '{}'.", table_file_path).cyan());

                if let Err(e) = init_table(&mut file) {
                    eprintln!("{}", format!("Failed to initialize table '{}': {}", table_name, e).red());
                } else {
                    println!("{} {}", "✓".green(), format!("Table '{}' initialized successfully.", table_name).green());
                }
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    format!("Failed to create table data file '{}': {}", table_file_path, e).red()
                );
                return;
            }
        }
    } else {
        println!("{} {}", "→".yellow(), format!("Table data file '{}' already exists.", table_file_path).cyan());
    }

    println!(
        "{} {}",
        "✓".green(),
        format!("Table '{}' created successfully in database '{}' and saved to catalog.", table_name, db_name).green()
    );
}

/// Lists all tables in the specified database.
pub fn show_tables(catalog: &Catalog, db_name: &str) {
    if let Some(database) = catalog.databases.get(db_name) {
        if database.tables.is_empty() {
            println!("{}", format!("No tables found in the database '{}'.", db_name).purple().bold());
            return;
        }
        
        println!("{}", format!("Tables in Database: {}", db_name).bold().purple());
        for table_name in database.tables.keys() {
            println!("{} {}", "→".cyan(), table_name.bold().white());
        }

    } else {
        println!("{}", format!("Database '{}' not found.", db_name).red());
    }
}

/// Deletes a table from the catalog and removes its data file from disk.
pub fn delete_table(catalog: &mut Catalog, db_name: &str, table_name: &str) -> bool {
    // Check if database exists
    if !catalog.databases.contains_key(db_name) {
        println!(
            "{}",
            format!("Database '{}' does not exist. Cannot delete table '{}'.", db_name, table_name).red()
        );
        return false;
    }

    let database = catalog.databases.get_mut(db_name).unwrap();

    // Check if table exists
    if !database.tables.contains_key(table_name) {
        println!(
            "{}",
            format!("Table '{}' does not exist in database '{}'.", table_name, db_name).red()
        );
        return false;
    }

    // Remove table from catalog
    database.tables.remove(table_name);

    // Persist catalog changes
    save_catalog(catalog);

    // Delete table file from disk
    let table_file_path = TABLE_FILE_TEMPLATE
        .replace("{database}", db_name)
        .replace("{table}", table_name);

    let table_path = Path::new(&table_file_path);
    if table_path.exists() {
        match fs::remove_file(table_path) {
            Ok(_) => {
                println!(
                    "{} {}",
                    "✓".green(),
                    format!("Table file '{}' deleted from disk.", table_file_path).green()
                );
            }
            Err(e) => {
                println!(
                    "{}",
                    format!("Failed to delete table file '{}': {}", table_file_path, e).red()
                );
                return false;
            }
        }
    }

    println!(
        "{} {}",
        "✓".green(),
        format!("Table '{}' deleted successfully from database '{}'.", table_name, db_name).green()
    );
    true
}

/// Deletes a database from the catalog and removes its directory from disk.
pub fn delete_database(catalog: &mut Catalog, db_name: &str) -> bool {
    // Check if database exists
    if !catalog.databases.contains_key(db_name) {
        println!(
            "{}",
            format!("Database '{}' does not exist.", db_name).red()
        );
        return false;
    }

    // Remove database from catalog
    catalog.databases.remove(db_name);

    // Persist catalog changes
    save_catalog(catalog);

    // Delete database directory from disk
    let db_path_str = TABLE_DIR_TEMPLATE.replace("{database}", db_name);
    let db_path = Path::new(&db_path_str);

    if db_path.exists() {
        match fs::remove_dir_all(db_path) {
            Ok(_) => {
                println!(
                    "{} {}",
                    "✓".green(),
                    format!("Database directory '{}' deleted from disk.", db_path_str).green()
                );
            }
            Err(e) => {
                println!(
                    "{}",
                    format!("Failed to delete database directory '{}': {}", db_path_str, e).red()
                );
                return false;
            }
        }
    }

    println!(
        "{} {}",
        "✓".green(),
        format!("Database '{}' deleted successfully.", db_name).green()
    );
    true
}
