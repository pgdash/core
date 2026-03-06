use crate::schema::{
    Column, Constraint, ConstraintType, Database, Function, Index, PostgresDataType,
    ReferentialAction, Schema, Table, Trigger, View,
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

            println!("Scanning Triggers for Table {}", table_name);
            let triggers = self.scan_triggers(&schema_name, &table_name)?;

            schema.tables.push(Table {
                name: table_name,
                schema_name: schema_name.clone(),
                columns,
                indexes,
                constraints,
                triggers,
                comment: None,
            });
        }

        let views_query = "
            SELECT table_schema, table_name, view_definition, is_updatable
            FROM information_schema.views
            WHERE table_schema NOT IN ('information_schema', 'pg_catalog')
        ";

        for row in self.client.query(views_query, &[])? {
            let schema_name: String = row.get("table_schema");
            let view_name: String = row.get("table_name");
            let definition: String = row.get("view_definition");
            let is_updatable_str: &str = row.get("is_updatable");

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

        self.scan_enums(&mut database)?;
        self.scan_sequences(&mut database)?;
        self.scan_functions(&mut database)?;

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
            let data_type_str: &str = col_row.get("data_type");
            let is_nullable_str: &str = col_row.get("is_nullable");
            let column_default: Option<String> = col_row.get("column_default");
            let char_len: Option<i32> = col_row.get("character_maximum_length");

            let data_type = map_data_type(data_type_str, char_len);

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
                    update_action: update_rule.map(|r| map_referential_action(&r)),
                    delete_action: delete_rule.map(|r| map_referential_action(&r)),
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
                am.amname AS index_type,
                idx.indisunique AS is_unique,
                idx.indisprimary AS is_primary,
                pg_get_indexdef(idx.indexrelid) AS index_definition,
                pg_get_expr(idx.indpred, idx.indrelid) AS partial_condition,
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
                pg_am AS am ON i.relam = am.oid
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
            let index_type: String = row.get("index_type");
            let is_unique: bool = row.get("is_unique");
            let is_primary: bool = row.get("is_primary");
            let definition: String = row.get("index_definition");
            let partial_condition: Option<String> = row.get("partial_condition");
            let columns: Vec<String> = row.get("index_columns");

            println!("Adding Index {}", name);
            indexes.push(Index {
                name,
                index_type,
                is_unique,
                is_primary_key: is_primary,
                columns,
                partial_condition,
                definition,
            });
        }

        Ok(indexes)
    }

    fn scan_triggers(
        &mut self,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Vec<Trigger>, postgres::Error> {
        let trigger_query = "
            SELECT
                trigger_name,
                event_manipulation,
                action_statement,
                action_timing,
                action_condition
            FROM information_schema.triggers
            WHERE event_object_schema = $1 AND event_object_table = $2
        ";

        let rows = self
            .client
            .query(trigger_query, &[&schema_name, &table_name])?;
        let mut triggers = Vec::new();

        for row in rows {
            triggers.push(Trigger {
                name: row.get("trigger_name"),
                event_manipulation: row.get("event_manipulation"),
                action_statement: row.get("action_statement"),
                action_timing: row.get("action_timing"),
                action_condition: row.get("action_condition"),
            });
        }

        Ok(triggers)
    }

    fn scan_enums(&mut self, database: &mut Database) -> Result<(), postgres::Error> {
        let enum_query = "
            SELECT
                n.nspname AS schema_name,
                t.typname AS enum_name,
                array_agg(e.enumlabel ORDER BY e.enumsortorder) AS variants
            FROM
                pg_type t
            JOIN
                pg_enum e ON t.oid = e.enumtypid
            JOIN
                pg_namespace n ON n.oid = t.typnamespace
            WHERE
                n.nspname NOT IN ('information_schema', 'pg_catalog')
            GROUP BY
                n.nspname, t.typname;
        ";

        println!("Querying user-defined enums");
        let rows = self.client.query(enum_query, &[])?;

        for row in rows {
            let schema_name: String = row.get("schema_name");
            let enum_name: String = row.get("enum_name");
            let variants: Vec<String> = row.get("variants");

            println!("Adding Enum {} in schema {}", enum_name, schema_name);
            let schema = database
                .schemas
                .entry(schema_name.clone())
                .or_insert_with(|| Schema {
                    name: schema_name.clone(),
                    ..Default::default()
                });

            schema.enums.push(crate::schema::EnumType {
                name: enum_name,
                schema_name,
                variants,
            });
        }

        Ok(())
    }

    fn scan_sequences(&mut self, database: &mut Database) -> Result<(), postgres::Error> {
        let seq_query = "
            SELECT
                sequence_schema,
                sequence_name,
                start_value::bigint,
                increment::bigint,
                minimum_value::bigint,
                maximum_value::bigint,
                cycle_option
            FROM
                information_schema.sequences
            WHERE
                sequence_schema NOT IN ('information_schema', 'pg_catalog');
        ";

        let rows = self.client.query(seq_query, &[])?;

        for row in rows {
            let schema_name: String = row.get("sequence_schema");
            let name: String = row.get("sequence_name");
            let start_value: i64 = row.get("start_value");
            let increment: i64 = row.get("increment");
            let min_value: i64 = row.get("minimum_value");
            let max_value: i64 = row.get("maximum_value");
            let cycle_option: String = row.get("cycle_option");

            let schema = database
                .schemas
                .entry(schema_name.clone())
                .or_insert_with(|| Schema {
                    name: schema_name.clone(),
                    ..Default::default()
                });

            schema.sequences.push(crate::schema::Sequence {
                name,
                schema_name,
                start_value,
                increment_by: increment,
                min_value,
                max_value,
                cycle: cycle_option == "YES",
            });
        }

        Ok(())
    }

    fn scan_functions(&mut self, database: &mut Database) -> Result<(), postgres::Error> {
        let routine_query = "
            SELECT
                routine_schema,
                routine_name,
                routine_type,
                data_type AS return_type,
                routine_definition,
                external_language
            FROM information_schema.routines
            WHERE routine_schema NOT IN ('information_schema', 'pg_catalog')
        ";

        let rows = self.client.query(routine_query, &[])?;

        for row in rows {
            let schema_name: Option<String> = row.try_get("routine_schema").ok();
            let routine_name: Option<String> = row.try_get("routine_name").ok();
            let routine_type: Option<&str> = row.try_get("routine_type").ok();
            let return_type: Option<String> = row.try_get("return_type").ok();
            let definition: Option<String> = row.try_get("routine_definition").ok();
            let language: Option<String> = row.try_get("external_language").ok();

            if let (Some(s_name), Some(r_name)) = (schema_name, routine_name) {
                let schema = database
                    .schemas
                    .entry(s_name.clone())
                    .or_insert_with(|| Schema {
                        name: s_name.clone(),
                        ..Default::default()
                    });

                let param_query = "
                    SELECT data_type
                    FROM information_schema.parameters
                    WHERE specific_schema = $1 AND specific_name = (
                        SELECT specific_name
                        FROM information_schema.routines
                        WHERE routine_schema = $1 AND routine_name = $2
                        LIMIT 1
                    )
                    ORDER BY ordinal_position
                ";

                let param_rows = self.client.query(param_query, &[&s_name, &r_name])?;
                let argument_types = param_rows.iter().map(|r| r.get("data_type")).collect();

                schema.functions.push(Function {
                    name: r_name,
                    schema_name: s_name,
                    argument_types,
                    return_type: return_type.unwrap_or_else(|| "void".to_string()),
                    definition: definition.unwrap_or_default(),
                    language: language.unwrap_or_else(|| "sql".to_string()),
                    is_procedure: routine_type.map(|t| t == "PROCEDURE").unwrap_or(false),
                });
            }
        }

        Ok(())
    }
}

fn map_data_type(dt: &str, char_len: Option<i32>) -> PostgresDataType {
    match dt {
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
        _ => PostgresDataType::Custom(dt.to_string()),
    }
}

fn map_referential_action(action: &str) -> ReferentialAction {
    match action {
        "CASCADE" => ReferentialAction::Cascade,
        "SET NULL" => ReferentialAction::SetNull,
        "SET DEFAULT" => ReferentialAction::SetDefault,
        "RESTRICT" => ReferentialAction::Restrict,
        _ => ReferentialAction::NoAction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_data_type() {
        assert_eq!(map_data_type("boolean", None), PostgresDataType::Boolean);
        assert_eq!(map_data_type("integer", None), PostgresDataType::Integer);
        assert_eq!(
            map_data_type("character varying", Some(255)),
            PostgresDataType::Varchar(Some(255))
        );
        assert_eq!(
            map_data_type("timestamp with time zone", None),
            PostgresDataType::Timestamp(true)
        );
        assert_eq!(
            map_data_type("unknown_type", None),
            PostgresDataType::Custom("unknown_type".to_string())
        );
    }

    #[test]
    fn test_map_referential_action() {
        assert_eq!(
            map_referential_action("CASCADE"),
            ReferentialAction::Cascade
        );
        assert_eq!(
            map_referential_action("SET NULL"),
            ReferentialAction::SetNull
        );
        assert_eq!(
            map_referential_action("RESTRICT"),
            ReferentialAction::Restrict
        );
        assert_eq!(
            map_referential_action("NO ACTION"),
            ReferentialAction::NoAction
        );
    }
}
