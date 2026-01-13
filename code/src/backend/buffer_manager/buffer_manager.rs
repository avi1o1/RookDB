use crate::catalog::types::Catalog;
use crate::disk::{read_page, write_page};
use crate::page::{PAGE_SIZE, Page, init_page, page_free_space, ITEM_ID_SIZE};
use colored::Colorize;

use std::fs::File;
use std::io::{self, BufRead, BufReader, ErrorKind, Read, Seek, SeekFrom};

pub struct BufferManager {
    pub pages: Vec<Page>, // In-memory pages (header + data)
}

impl BufferManager {
    pub fn new() -> Self {
        // Start with ONLY header page
        let mut pages = Vec::new();

        let mut header = Page::new();
        init_page(&mut header);
        pages.push(header);

        println!("{}", "Buffer Manager initialized with header page only.".cyan());

        Self { pages }
    }

    /// Allocate ONE new data page
    pub fn allocate_page(&mut self) {
        let mut page = Page::new();
        init_page(&mut page);
        self.pages.push(page);
    }

    /// Loads table from disk into buffer (opens an existing table)
    pub fn load_table_from_disk(
        &mut self,
        db_name: &str,
        table_name: &str,
    ) -> io::Result<()> {
        let table_path = format!("database/base/{}/{}.dat", db_name, table_name);
        let mut file = File::open(&table_path)?;

        let metadata = file.metadata()?;
        let file_size = metadata.len();
        let total_pages = (file_size as usize) / PAGE_SIZE;

        println!(
            "{} {}",
            "→".yellow(),
            format!("Loading table '{}' ({} bytes, {} pages)...", table_name, file_size, total_pages).cyan()
        );

        // Reset in-memory buffer
        self.pages.clear();

        // Read header (page 0)
        let mut header_page = Page::new();
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut header_page.data)?;
        self.pages.push(header_page);

        // Read data pages
        for page_num in 1..total_pages {
            let mut page = Page::new();
            match read_page(&mut file, &mut page, page_num as u32) {
                Ok(_) => self.pages.push(page),
                Err(e) => {
                    if e.kind() == ErrorKind::UnexpectedEof {
                        break;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        println!(
            "{} {}",
            "✓".green(),
            format!("Loaded {} pages (1 header + {} data).", self.pages.len(), self.pages.len().saturating_sub(1)).green()
        );

        Ok(())
    }

    /// Load CSV into memory using page-based allocation
    pub fn load_csv_into_pages(
        &mut self,
        catalog: &Catalog,
        db_name: &str,
        table_name: &str,
        csv_path: &str,
    ) -> io::Result<usize> {
        // --- schema ---
        let db = catalog.databases.get(db_name).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("Database '{}' not found", db_name))
        })?;
        let table = db.tables.get(table_name).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("Table '{}' not found", table_name))
        })?;
        let columns = &table.columns;

        if columns.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Table has no columns"));
        }

        // --- read CSV ---
        let csv_file = File::open(csv_path)?;
        let reader = BufReader::new(csv_file);
        let mut lines = reader.lines();
        if let Some(Ok(_)) = lines.next() {} // skip header

        let mut inserted_rows = 0usize;
        let mut current_page_index = self.pages.len() - 1;// DATA pages start at index 1

        // Ensure first data page exists
        if self.pages.len() == 1 {
            self.allocate_page();
        }

        for (i, line) in lines.enumerate() {
            let row = line?;
            if row.trim().is_empty() {
                continue;
            }

            let values: Vec<&str> = row.split(',').map(|v| v.trim()).collect();
            if values.len() != columns.len() {
                println!(
                    "{}",
                    format!("Skipping row {}: Expected {} columns, got {}", i + 1, columns.len(), values.len()).red()
                );
                continue;
            }

            // Serialize tuple
            let mut tuple_bytes: Vec<u8> = Vec::new();
            for (val, col) in values.iter().zip(columns.iter()) {
                match col.data_type.as_str() {
                    "INT" => {
                        let num: i32 = val.parse().unwrap_or_default();
                        tuple_bytes.extend_from_slice(&num.to_le_bytes());
                    }
                    "FLOAT" => {
                        let num: f32 = val.parse().unwrap_or_default();
                        tuple_bytes.extend_from_slice(&num.to_le_bytes());
                    }
                    "TEXT" => {
                        let mut t = val.as_bytes().to_vec();
                        if t.len() > 10 {
                            t.truncate(10);
                        } else if t.len() < 10 {
                            t.extend(vec![b' '; 10 - t.len()]);
                        }
                        tuple_bytes.extend_from_slice(&t);
                    }
                    "BOOLEAN" => {
                        let bool_val = match val.to_lowercase().as_str() {
                            "true" | "t" | "1" | "yes" | "y" => 1u8,
                            _ => 0u8,
                        };
                        tuple_bytes.push(bool_val);
                    }
                    "DATE" => {
                        let days = parse_date_buf(val).unwrap_or(0);
                        tuple_bytes.extend_from_slice(&days.to_le_bytes());
                    }
                    "TIME" => {
                        let seconds = parse_time_buf(val).unwrap_or(0);
                        tuple_bytes.extend_from_slice(&seconds.to_le_bytes());
                    }
                    "DATETIME" => {
                        let timestamp = parse_datetime_buf(val).unwrap_or(0);
                        tuple_bytes.extend_from_slice(&timestamp.to_le_bytes());
                    }
                    _ => continue,
                }
            }

            let tuple_len = tuple_bytes.len() as u32;
            let required = tuple_len + ITEM_ID_SIZE;

            loop {
                if current_page_index >= self.pages.len() {
                    self.allocate_page();
                }

                let page = &mut self.pages[current_page_index];
                let free = page_free_space(page)?;

                if free < required {
                    current_page_index += 1;
                    continue;
                }

                let mut lower =
                    u32::from_le_bytes(page.data[0..4].try_into().unwrap());
                let mut upper =
                    u32::from_le_bytes(page.data[4..8].try_into().unwrap());

                let start = upper - tuple_len;

                page.data[start as usize..upper as usize]
                    .copy_from_slice(&tuple_bytes);

                let item_id_pos = lower as usize;
                page.data[item_id_pos..item_id_pos + 4]
                    .copy_from_slice(&start.to_le_bytes());
                page.data[item_id_pos + 4..item_id_pos + 8]
                    .copy_from_slice(&tuple_len.to_le_bytes());

                lower += ITEM_ID_SIZE;
                upper = start;

                page.data[0..4].copy_from_slice(&lower.to_le_bytes());
                page.data[4..8].copy_from_slice(&upper.to_le_bytes());
                inserted_rows += 1;
                break;
            }
        }

        let used_pages = self.pages.len();
        self.pages[0].data[0..4]
            .copy_from_slice(&(used_pages as u32).to_le_bytes());

        if inserted_rows == 0 {
            println!(
                "{} {}",
                "✗".red(),
                "No rows were inserted from the CSV file.".red()
            );
            return Ok(used_pages);
        }

        println!(
            "{} {}",
            "✓".green(),
            format!("Loaded {} rows into {} data page(s).", inserted_rows, used_pages - 1).green()
        );

        Ok(used_pages)
    }

    pub fn flush_to_disk(
        &mut self,
        db_name: &str,
        table_name: &str,
        used_pages: usize,
    ) -> io::Result<()> {
        let path = format!("database/base/{}/{}.dat", db_name, table_name);
        let mut file = File::options().write(true).open(&path)?;

        for (i, page) in self.pages.iter_mut().take(used_pages).enumerate() {
            write_page(&mut file, page, i as u32)?;
        }

        Ok(())
    }

    pub fn load_csv_to_buffer(
        &mut self,
        catalog: &Catalog,
        db_name: &str,
        table_name: &str,
        csv_path: &str,
    ) -> io::Result<()> {
        let used = self.load_csv_into_pages(catalog, db_name, table_name, csv_path)?;
        self.flush_to_disk(db_name, table_name, used)?;
        Ok(())
    }
}

/// Parse date string (YYYY-MM-DD) to days since Unix epoch (1970-01-01)
fn parse_date_buf(s: &str) -> Option<i32> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: i32 = parts[1].parse().ok()?;
    let day: i32 = parts[2].parse().ok()?;
    
    let days_since_epoch = (year - 1970) * 365 + (year - 1969) / 4
        + match month {
            1 => 0,
            2 => 31,
            3 => 59,
            4 => 90,
            5 => 120,
            6 => 151,
            7 => 181,
            8 => 212,
            9 => 243,
            10 => 273,
            11 => 304,
            12 => 334,
            _ => return None,
        }
        + day - 1;
    Some(days_since_epoch)
}

/// Parse time string (HH:MM:SS) to seconds since midnight
fn parse_time_buf(s: &str) -> Option<i32> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours: i32 = parts[0].parse().ok()?;
    let minutes: i32 = parts[1].parse().ok()?;
    let seconds: i32 = parts[2].parse().ok()?;
    Some(hours * 3600 + minutes * 60 + seconds)
}

/// Parse datetime string (YYYY-MM-DD HH:MM:SS) to Unix timestamp
fn parse_datetime_buf(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    let days = parse_date_buf(parts[0])? as i64;
    let time_seconds = parse_time_buf(parts[1])? as i64;
    Some(days * 86400 + time_seconds)
}
