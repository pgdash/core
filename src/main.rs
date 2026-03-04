use pgdash_lib::scanner::PostgresScanner;
use postgres::{Client, NoTls};

fn main() {
    let db_url = "postgres://postgres:postgres@localhost/dvdrental?sslmode=disable";

    println!("Connecting to {}...", db_url);
    let mut client = match Client::connect(db_url, NoTls) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect to database: {}", e);
            return;
        }
    };

    let mut scanner = PostgresScanner::new(&mut client);

    match scanner.scan("dvdrental") {
        Ok(database) => {
            println!("Successfully scanned database: {}", database.name);
            for (name, schema) in &database.schemas {
                println!("Schema: {}", name);
                println!("  Tables: {} found", schema.tables.len());
                for table in &schema.tables {
                    println!(
                        "    - Table: {} ({} columns)",
                        table.name,
                        table.columns.len()
                    );
                    for column in &table.columns {
                        println!(
                            "      - Column: {} ({:?}) {}",
                            column.name,
                            column.data_type,
                            if column.is_nullable {
                                "NULL"
                            } else {
                                "NOT NULL"
                            }
                        );
                    }
                }
                println!("  Views: {} found", schema.views.len());
                for view in &schema.views {
                    println!("    - View: {}", view.name);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to scan database: {}", e);
        }
    }
}
