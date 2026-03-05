use pgdash_lib::scanner::PostgresScanner;
use postgres::{Client, NoTls};
use std::fs::File;
use std::io::Write;

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

            // Serialize to JSON
            let json = serde_json::to_string_pretty(&database)
                .expect("Failed to serialize database to JSON");

            // Write to file
            let filename = format!("{}_schema.json", database.name);
            let mut file = File::create(&filename).expect("Failed to create JSON file");
            file.write_all(json.as_bytes())
                .expect("Failed to write to JSON file");

            println!("Schema written to {}", filename);

            for (name, schema) in &database.schemas {
                println!("Schema: {}", name);
                println!("  Tables: {} found", schema.tables.len());
                println!("  Views: {} found", schema.views.len());
            }
        }
        Err(e) => {
            eprintln!("Failed to scan database: {}", e);
        }
    }
}
