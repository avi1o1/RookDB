//! Handles table-related user commands such as listing tables,
//! creating tables, and displaying table statistics.

use std::io::{self, Write};
use colored::Colorize;

use storage_manager::buffer_manager::BufferManager;
use storage_manager::catalog::{Catalog, Column, create_table, show_tables, VALID_TYPES};
use storage_manager::statistics::print_table_page_count;

/// Displays tables in the currently selected database
pub fn show_tables_cmd(catalog: &Catalog, current_db: &Option<String>) {
    let db = match current_db {
        Some(db) => db,
        None => {
            println!("{}", "No database selected. Please select a database first.".purple().bold());
            return;
        }
    };
    show_tables(catalog, db);
}

pub fn create_table_cmd(
    catalog: &mut Catalog,
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

    let mut table_name = String::new();
    print!("{}", "Enter table name: ".yellow().bold());
    io::stdout().flush()?;
    io::stdin().read_line(&mut table_name)?;
    let table_name = table_name.trim().to_string();

    // check if table already exists
    if let Some(database) = catalog.databases.get(&db) {
        if database.tables.contains_key(&table_name) {
            println!("{}", format!("✗ Table '{}' already exists in database '{}'.", table_name, db).red());
            return Ok(());
        }
    } else {
        println!("{}", format!("✗ Database '{}' not found in catalog.", db).red());
        return Ok(());
    }

    println!("{}", "Enter column details (Press Enter on an empty line to finish)".yellow().bold());

    let mut columns = Vec::new();
    loop {
        let mut input = String::new();
        print!("{}", "Enter column (column_name:data_type): ".yellow().bold());
        io::stdout().flush()?;
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            break;
        }

        let parts: Vec<&str> = input.split(':').collect();
        if parts.len() != 2 {
            println!("{}", "Invalid format. Please use column_name:data_type (e.g. name:TEXT)".red());
            continue;
        }

        // check if data type is valid
        if !VALID_TYPES.contains(&parts[1].to_uppercase().as_str()) {
            println!("{}", format!("Invalid data type '{}'. Supported types are: {}", parts[1], VALID_TYPES.join(", ")).red());
            continue;
        }

        columns.push(Column {
            name: parts[0].to_string(),
            data_type: parts[1].to_string(),
        });
    }

    create_table(catalog, &db, &table_name, columns);
    buffer_manager.load_table_from_disk(&db, &table_name)?;

    Ok(())
}

pub fn show_table_statistics_cmd(
    current_db: &Option<String>,
) -> io::Result<()> {
    let db_name = match current_db {
        Some(db) => db,
        None => {
            println!("{}", "No database selected. Please select a database first.".purple().bold());
            return Ok(());
        }
    };

    print!("{}", "Enter table name: ".yellow().bold());
    io::stdout().flush()?;

    let mut table_name = String::new();
    io::stdin().read_line(&mut table_name)?;
    let table_name = table_name.trim();

    print_table_page_count(db_name, table_name)?;

    Ok(())
}