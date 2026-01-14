//! Handles the interactive command-line menu and routes user input
//! to the appropriate operations. 

use std::io::{self, Write};
use colored::Colorize;

// Core storage manager components
use storage_manager::buffer_manager::BufferManager;
use storage_manager::catalog::{init_catalog, load_catalog};

// Frontend command handlers
use crate::frontend::{
    database_cmd,
    table_cmd,
    data_cmd,
    WIDTH,
};

// Prints the welcome message
pub fn print_welcome_message() {
    let title = "Welcome to RookDB";
    let padding = (WIDTH - title.len() - 4) / 2; // 4 for "║ " and " ║"
    let remaining = WIDTH - title.len() - 4 - padding;
    
    println!("{}", format!("╔{}╗", "═".repeat(WIDTH - 2)).cyan());
    println!("{} {}{}{} {}", 
        "║".cyan(), 
        " ".repeat(padding),
        title.bold().magenta(),
        " ".repeat(remaining),
        "║".cyan()
    );
    println!("{}", format!("╚{}╝", "═".repeat(WIDTH - 2)).cyan());
}

// Prints the interactive menu
pub fn print_menu() {
    println!("{}", format!("╔{}╗", "═".repeat(WIDTH - 2)).cyan());
    println!("{}", format!("║ {:<width$} ║", "Choose an option:".bold().white(), width = WIDTH - 4).cyan());
    println!("{}", format!("╠{}╣", "═".repeat(WIDTH - 2)).cyan());
    println!("{}", format!("║ {:<width$} ║", "Database Operations:".bold().blue(), width = WIDTH - 4).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "DB1.".green(), "Show Databases", width = WIDTH - 9).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "DB2.".green(), "Create Database", width = WIDTH - 9).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "DB3.".green(), "Select Database", width = WIDTH - 9).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "DB4.".green(), "Delete Database", width = WIDTH - 9).cyan());
    println!("{}", format!("╠{}╣", "─".repeat(WIDTH - 2)).cyan());
    println!("{}", format!("║ {:<width$} ║", "Table Operations:".bold().blue(), width = WIDTH - 4).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "T1.".green(), "Show Tables", width = WIDTH - 8).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "T2.".green(), "Create Table", width = WIDTH - 8).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "T3.".green(), "Table Statistics", width = WIDTH - 8).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "T4.".green(), "Delete Table", width = WIDTH - 8).cyan());
    println!("{}", format!("╠{}╣", "─".repeat(WIDTH - 2)).cyan());
    println!("{}", format!("║ {:<width$} ║", "Data Operations:".bold().blue(), width = WIDTH - 4).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "D1.".green(), "Load CSV", width = WIDTH - 8).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "D2.".green(), "Show Tuples", width = WIDTH - 8).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "D3.".green(), "Update Tuple", width = WIDTH - 8).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "D4.".green(), "Insert Tuple", width = WIDTH - 8).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "D5.".green(), "Delete Tuple", width = WIDTH - 8).cyan());
    println!("{}", format!("╠{}╣", "─".repeat(WIDTH - 2)).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "X0.".red(), "Exit", width = WIDTH - 8).cyan());
    println!("{}", format!("╚{}╝", "═".repeat(WIDTH - 2)).cyan());
}

/// Runs the main interactive menu loop
pub fn run() -> io::Result<()> {
    print_welcome_message();

    // Ensure catalog file exists
    println!("{} Initializing Catalog File...", "→".yellow());
    init_catalog();

    // Load catalog metadata into memory
    println!("{} Loading Catalog...", "→".yellow());
    let mut catalog = load_catalog();

    // Initialize buffer manager
    let mut buffer_manager = BufferManager::new();

    // Tracks the currently selected database
    let mut current_db: Option<String> = None;

    loop {
        print_menu();

        // Read user input
        print!("{} ", "Enter your choice:".bold().yellow());
        io::stdout().flush()?;

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;
        let choice = choice.trim().to_uppercase();

        // Dispatch command based on user selection
        match choice.as_str() {
            "DB1" => database_cmd::show_databases_cmd(&catalog),
            "DB2" => database_cmd::create_database_cmd(&mut catalog)?,
            "DB3" => database_cmd::select_database_cmd(&catalog, &mut current_db)?,
            "DB4" => database_cmd::delete_database_cmd(&mut catalog, &mut current_db)?,
            "T1" => table_cmd::show_tables_cmd(&catalog, &current_db),
            "T2" => table_cmd::create_table_cmd(
                &mut catalog,
                &mut buffer_manager,
                &current_db,
            )?,
            "T3" => table_cmd::show_table_statistics_cmd(&current_db)?,
            "T4" => table_cmd::delete_table_cmd(&mut catalog, &current_db)?,
            "D1" => data_cmd::load_csv_cmd(
                &mut buffer_manager,
                &current_db,
            )?,
            "D2" => data_cmd::show_tuples_cmd(&current_db)?,
            "D3" => data_cmd::update_tuple_cmd(&current_db)?,
            "D4" => data_cmd::insert_tuple_cmd(&current_db)?,
            "D5" => data_cmd::delete_tuple_cmd(&current_db)?,
            "X0" => {
                println!("{}", "Exiting RookDB. Goodbye!".purple().bold());
                break;
            }
            _ => println!("{}", "✗ Invalid option.".red()),
        }
    }

    Ok(())
}