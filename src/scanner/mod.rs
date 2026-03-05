use crate::schema::{
    Column, Constraint, ConstraintType, Database, Index, PostgresDataType, ReferentialAction,
    Schema, Table, View,
};
use postgres::Client;
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
        println!("Initiating Scan");

        // 1. Fetch all tables
        let tables_query = "
            SELECT table_schema, table_name
            FROM information_schema.tables
            WHERE table_schema NOT IN ('information_schema', 'pg_catalog')
            AND table_type = 'BASE TABLE'
            ORDER BY table_schema, table_name
        ";

        println!("Querying table names");
        let table_rows = self.client.query(tables_query, &[])?;

        let mut schemas_found = Vec::new();
        for row in table_rows {
            let schema_name: String = row.get("table_schema");
            let table_name: String = row.get("table_name");
            schemas_found.push((schema_name, table_name));
        }

        for (schema_name, table_name) in schemas_found {
            let schema = database
                .schemas
                .entry(schema_name.clone())
                .or_insert_with(|| Schema {
                    name: schema_name.clone(),
                    ..Default::default()
                });

            println!("Scanning Columns for Table {}", table_name);
            let columns = self.scan_columns(&schema_name, &table_name)?;

            println!("Scanning Constraints for Table {}", table_name);
            let constraints = self.scan_constraints(&schema_name, &table_name)?;

            println!("Scanning Indexes for Table {}", table_name);
            let indexes = self.scan_indexes(&schema_name, &table_name)?;

            schema.tables.push(Table {
                name: table_name,
                schema_name: schema_name.clone(),
                columns,
                indexes,
                constraints,
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

            let schema = database
                .schemas
                .entry(schema_name.clone())
                .or_insert_with(|| Schema {
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

    fn scan_columns(
        &mut self,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Vec<Column>, postgres::Error> {
        let columns_query = "
            SELECT column_name, data_type, is_nullable, column_default, character_maximum_length
            FROM information_schema.columns
            WHERE table_schema = $1 AND table_name = $2
            ORDER BY ordinal_position
        ";

        let col_rows = self
            .client
            .query(columns_query, &[&schema_name, &table_name])?;
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

            println!("Adding Column {}", col_name);
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

    fn scan_constraints(
        &mut self,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Vec<Constraint>, postgres::Error> {
        let mut constraints = Vec::new();

        // 1. Scan PRIMARY KEY, UNIQUE, and FOREIGN KEY constraints
        let constraints_query = "
            SELECT
                tc.constraint_name,
                tc.constraint_type,
                rc.update_rule,
                rc.delete_rule,
                ccu.table_name AS foreign_table,
                ccu.column_name AS foreign_column,
                kcu.column_name AS local_column
            FROM
                information_schema.table_constraints AS tc
                JOIN information_schema.key_column_usage AS kcu
                  ON tc.constraint_name = kcu.constraint_name
                  AND tc.table_schema = kcu.table_schema
                LEFT JOIN information_schema.referential_constraints AS rc
                  ON tc.constraint_name = rc.constraint_name
                  AND tc.table_schema = rc.constraint_schema
                LEFT JOIN information_schema.constraint_column_usage AS ccu
                  ON rc.unique_constraint_name = ccu.constraint_name
                  AND rc.unique_constraint_schema = ccu.table_schema
            WHERE tc.table_schema = $1 AND tc.table_name = $2
            ORDER BY tc.constraint_name, kcu.ordinal_position;
        ";

        let rows = self
            .client
            .query(constraints_query, &[&schema_name, &table_name])?;

        struct ConstraintGroup {
            ctype: String,
            local_cols: Vec<String>,
            foreign_table: Option<String>,
            update_action: Option<ReferentialAction>,
            delete_action: Option<ReferentialAction>,
            foreign_cols: Vec<String>,
        }

        let mut constraint_map: HashMap<String, ConstraintGroup> = HashMap::new();

        for row in rows {
            let name: String = row.get("constraint_name");
            let ctype: String = row.get("constraint_type");
            let local_col: String = row.get("local_column");
            let foreign_table: Option<String> = row.get("foreign_table");
            let foreign_col: Option<String> = row.get("foreign_column");
            let update_rule: Option<String> = row.get("update_rule");
            let delete_rule: Option<String> = row.get("delete_rule");

            let entry = constraint_map
                .entry(name.clone())
                .or_insert_with(|| ConstraintGroup {
                    ctype,
                    local_cols: Vec::new(),
                    foreign_table,
                    update_action: update_rule.map(|r| self.map_referential_action(&r)),
                    delete_action: delete_rule.map(|r| self.map_referential_action(&r)),
                    foreign_cols: Vec::new(),
                });

            println!("Adding constraint {}", name);
            entry.local_cols.push(local_col);
            if let Some(fcol) = foreign_col {
                entry.foreign_cols.push(fcol);
            }
        }

        for (name, group) in constraint_map {
            let constraint_type = match group.ctype.as_str() {
                "PRIMARY KEY" => ConstraintType::PrimaryKey,
                "UNIQUE" => ConstraintType::Unique,
                "FOREIGN KEY" => ConstraintType::ForeignKey {
                    foreign_table: group.foreign_table.unwrap_or_default(),
                    foreign_columns: group.foreign_cols,
                    on_delete: group.delete_action.unwrap_or(ReferentialAction::NoAction),
                    on_update: group.update_action.unwrap_or(ReferentialAction::NoAction),
                },
                _ => continue,
            };

            println!("Adding Constraint {}", name);
            constraints.push(Constraint {
                name,
                columns: group.local_cols,
                constraint_type,
            });
        }

        // 2. Scan CHECK constraints
        let check_query = "
            SELECT tc.constraint_name, cc.check_clause, ccu.column_name
            FROM information_schema.table_constraints AS tc
            JOIN information_schema.check_constraints AS cc
              ON tc.constraint_name = cc.constraint_name
              AND tc.table_schema = cc.constraint_schema
            LEFT JOIN information_schema.constraint_column_usage AS ccu
              ON tc.constraint_name = ccu.constraint_name
              AND tc.table_schema = ccu.table_schema
            WHERE tc.table_schema = $1 AND tc.table_name = $2;
        ";

        let mut check_map: HashMap<String, (String, Vec<String>)> = HashMap::new();
        for row in self
            .client
            .query(check_query, &[&schema_name, &table_name])?
        {
            let name: String = row.get("constraint_name");
            let clause: String = row.get("check_clause");
            let column: Option<String> = row.get("column_name");

            let entry = check_map.entry(name).or_insert((clause, Vec::new()));
            if let Some(col) = column {
                entry.1.push(col);
            }
        }

        for (name, (clause, columns)) in check_map {
            println!("Adding Check Constraint {}", name);
            constraints.push(Constraint {
                name,
                columns,
                constraint_type: ConstraintType::Check(clause),
            });
        }

        Ok(constraints)
    }

    fn scan_indexes(
        &mut self,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Vec<Index>, postgres::Error> {
        let mut indexes = Vec::new();

        let index_query = "
            SELECT
                i.relname AS index_name,
                idx.indisunique AS is_unique,
                idx.indisprimary AS is_primary,
                pg_get_indexdef(idx.indexrelid) AS index_definition,
                (
	                SELECT array_agg(pg_get_indexdef(idx.indexrelid, k + 1, true) ORDER BY k)
	                FROM generate_subscripts(idx.indkey, 1) AS k
                ) AS index_columns
            FROM
                pg_index AS idx
            JOIN
                pg_class AS t ON t.oid = idx.indrelid
            JOIN
                pg_class AS i ON i.oid = idx.indexrelid
            JOIN
                pg_namespace AS n ON n.oid = t.relnamespace
            WHERE
                n.nspname = $1
                AND t.relname = $2;
        ";

        println!("Querying indexes for {}.{}", schema_name, table_name);
        let rows = self
            .client
            .query(index_query, &[&schema_name, &table_name])?;

        println!("Done querying indexes");

        for row in rows {
            let name: String = row.get("index_name");
            let is_unique: bool = row.get("is_unique");
            let is_primary: bool = row.get("is_primary");
            let definition: String = row.get("index_definition");
            let columns: Vec<String> = row.get("index_columns");

            println!("Adding Index {}", name);
            indexes.push(Index {
                name,
                is_unique,
                is_primary_key: is_primary,
                columns,
                definition,
            });
        }

        Ok(indexes)
    }

    fn map_referential_action(&self, action: &str) -> ReferentialAction {
        match action {
            "CASCADE" => ReferentialAction::Cascade,
            "SET NULL" => ReferentialAction::SetNull,
            "SET DEFAULT" => ReferentialAction::SetDefault,
            "RESTRICT" => ReferentialAction::Restrict,
            _ => ReferentialAction::NoAction,
        }
    }
}
