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
    println!("{}", format!("║ {} {:<width$} ║", "1.".green(), "Show Databases", width = WIDTH - 7).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "2.".green(), "Create Database", width = WIDTH - 7).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "3.".green(), "Select Database", width = WIDTH - 7).cyan());
    println!("{}", format!("╠{}╣", "─".repeat(WIDTH - 2)).cyan());
    println!("{}", format!("║ {:<width$} ║", "Table Operations:".bold().blue(), width = WIDTH - 4).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "4.".green(), "Show Tables", width = WIDTH - 7).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "5.".green(), "Create Table", width = WIDTH - 7).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "6.".green(), "Show Table Statistics", width = WIDTH - 7).cyan());
    println!("{}", format!("╠{}╣", "─".repeat(WIDTH - 2)).cyan());
    println!("{}", format!("║ {:<width$} ║", "Data Operations:".bold().blue(), width = WIDTH - 4).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "7.".green(), "Load CSV", width = WIDTH - 7).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "8.".green(), "Show Tuples", width = WIDTH - 7).cyan());
    println!("{}", format!("╠{}╣", "─".repeat(WIDTH - 2)).cyan());
    println!("{}", format!("║ {} {:<width$} ║", "0.".red(), "Exit", width = WIDTH - 7).cyan());
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
        let choice = choice.trim();

        // Dispatch command based on user selection
        match choice {
            "1" => database_cmd::show_databases_cmd(&catalog),
            "2" => database_cmd::create_database_cmd(&mut catalog)?,
            "3" => database_cmd::select_database_cmd(&catalog, &mut current_db)?,
            "4" => table_cmd::show_tables_cmd(&catalog, &current_db),
            "5" => table_cmd::create_table_cmd(
                &mut catalog,
                &mut buffer_manager,
                &current_db,
            )?,
            "6" => table_cmd::show_table_statistics_cmd(&current_db)?,
            "7" => data_cmd::load_csv_cmd(
                &mut buffer_manager,
                &current_db,
            )?,
            "8" => data_cmd::show_tuples_cmd(&current_db)?,
            "0" => {
                println!("{}", "Exiting RookDB. Goodbye!".purple().bold());
                break;
            }
            _ => println!("{}", "✗ Invalid option.".red()),
        }
    }

    Ok(())
}
