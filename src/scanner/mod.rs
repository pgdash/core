use postgres::Client;
use crate::schema::{Database, Schema, Table, Column, PostgresDataType, View};
use std::collections::HashMap;

pub struct PostgresScanner<'a> {
    client: &'a mut Client,
}

impl<'a> PostgresScanner<'a> {
    pub fn new(client: &'a mut Client) -> Self {
        Self { client }
    }

    pub fn scan(&mut self, database_name: &str) -> Result<Database, postgres::Error> {
        let mut database = Database {
            name: database_name.to_string(),
            schemas: HashMap::new(),
        };

        // 1. Fetch all tables
        let tables_query = "
            SELECT table_schema, table_name 
            FROM information_schema.tables 
            WHERE table_schema NOT IN ('information_schema', 'pg_catalog')
            AND table_type = 'BASE TABLE'
            ORDER BY table_schema, table_name
        ";

        let table_rows = self.client.query(tables_query, &[])?;
        
        for row in table_rows {
            let schema_name: String = row.get("table_schema");
            let table_name: String = row.get("table_name");

            let schema = database.schemas.entry(schema_name.clone()).or_insert_with(|| Schema {
                name: schema_name.clone(),
                ..Default::default()
            });

            let columns = self.scan_columns(&schema_name, &table_name)?;

            schema.tables.push(Table {
                name: table_name,
                schema_name: schema_name.clone(),
                columns,
                indexes: Vec::new(),
                constraints: Vec::new(),
                triggers: Vec::new(),
                comment: None,
            });
        }

        // 2. Fetch Views
        let views_query = "
            SELECT table_schema, table_name, view_definition, is_updatable
            FROM information_schema.views
            WHERE table_schema NOT IN ('information_schema', 'pg_catalog')
        ";

        for row in self.client.query(views_query, &[])? {
            let schema_name: String = row.get("table_schema");
            let view_name: String = row.get("table_name");
            let definition: String = row.get("view_definition");
            let is_updatable_str: String = row.get("is_updatable");

            let schema = database.schemas.entry(schema_name.clone()).or_insert_with(|| Schema {
                name: schema_name.clone(),
                ..Default::default()
            });

            schema.views.push(View {
                name: view_name,
                schema_name: schema_name.clone(),
                definition,
                is_updatable: is_updatable_str == "YES",
            });
        }

        Ok(database)
    }

    fn scan_columns(&mut self, schema_name: &str, table_name: &str) -> Result<Vec<Column>, postgres::Error> {
        let columns_query = "
            SELECT column_name, data_type, is_nullable, column_default, character_maximum_length
            FROM information_schema.columns
            WHERE table_schema = $1 AND table_name = $2
            ORDER BY ordinal_position
        ";

        let col_rows = self.client.query(columns_query, &[&schema_name, &table_name])?;
        let mut columns = Vec::new();

        for col_row in col_rows {
            let col_name: String = col_row.get("column_name");
            let data_type_str: String = col_row.get("data_type");
            let is_nullable_str: String = col_row.get("is_nullable");
            let column_default: Option<String> = col_row.get("column_default");
            let char_len: Option<i32> = col_row.get("character_maximum_length");

            let data_type = match data_type_str.as_ref() {
                "boolean" => PostgresDataType::Boolean,
                "smallint" => PostgresDataType::SmallInt,
                "integer" => PostgresDataType::Integer,
                "bigint" => PostgresDataType::BigInt,
                "real" => PostgresDataType::Real,
                "double precision" => PostgresDataType::DoublePrecision,
                "text" => PostgresDataType::Text,
                "character varying" => PostgresDataType::Varchar(char_len.map(|l| l as u32)),
                "character" => PostgresDataType::Character(char_len.map(|l| l as u32)),
                "timestamp without time zone" => PostgresDataType::Timestamp(false),
                "timestamp with time zone" => PostgresDataType::Timestamp(true),
                "date" => PostgresDataType::Date,
                "json" => PostgresDataType::Json,
                "jsonb" => PostgresDataType::Jsonb,
                "uuid" => PostgresDataType::Uuid,
                _ => PostgresDataType::Custom(data_type_str),
            };

            columns.push(Column {
                name: col_name,
                data_type,
                is_nullable: is_nullable_str == "YES",
                default_value: column_default,
                comment: None,
            });
        }

        Ok(columns)
    }
}
