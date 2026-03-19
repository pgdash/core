use crate::schema::{
    Column, Constraint, ConstraintType, Database, EnumType, Function, Index, PostgresDataType,
    ReferentialAction, Schema, Table, Trigger, View,
};
use std::collections::HashMap;
use tracing::{info, warn};
use traits::{DatabaseClient, DatabaseRow};

#[cfg(test)]
pub mod mock;
pub mod traits;

pub struct PostgresScanner<'a, C: DatabaseClient> {
    client: &'a C,
}

impl<'a, C: DatabaseClient> PostgresScanner<'a, C> {
    pub fn new(client: &'a C) -> Self {
        Self { client }
    }

    pub async fn scan(&self, database_name: &str) -> Result<Database, String> {
        let mut schemas_map: HashMap<String, Schema> = HashMap::new();

        let schema_query = "SELECT oid, nspname FROM pg_namespace WHERE nspname NOT IN ('information_schema', 'pg_catalog', 'pg_toast')";

        info!("Scanning schemas");
        for row in self.client.query(schema_query, &[]).await? {
            let oid: u32 = row.get_u32("oid");
            let name: String = row.get_string("nspname");
            schemas_map.insert(
                name.clone(),
                Schema {
                    oid,
                    name,
                    ..Default::default()
                },
            );
        }

        let tables_query = "
            SELECT t.table_schema, t.table_name, c.oid
            FROM information_schema.tables t
            JOIN pg_namespace n ON n.nspname = t.table_schema
            JOIN pg_class c ON c.relname = t.table_name AND c.relnamespace = n.oid
            WHERE t.table_schema NOT IN ('information_schema', 'pg_catalog')
            AND t.table_type = 'BASE TABLE'
            ORDER BY table_schema, table_name
        ";

        info!("Scanning tables");
        let table_rows = self.client.query(tables_query, &[]).await?;

        let mut schemas_found = Vec::new();
        for row in table_rows {
            let schema_name: String = row.get_string("table_schema");
            let table_name: String = row.get_string("table_name");
            let oid: u32 = row.get_u32("oid");
            schemas_found.push((schema_name, table_name, oid));
        }

        for (schema_name, table_name, oid) in schemas_found {
            let schema = schemas_map
                .entry(schema_name.clone())
                .or_insert_with(|| Schema {
                    name: schema_name.clone(),
                    ..Default::default()
                });

            info!("running scan tasks for table: {}", table_name);
            let (col_res, con_res, idx_res, trig_res) = tokio::join!(
                self.scan_columns(&schema_name, &table_name),
                self.scan_constraints(&schema_name, &table_name),
                self.scan_indexes(&schema_name, &table_name),
                self.scan_triggers(&schema_name, &table_name),
            );

            let columns = col_res.unwrap_or_else(|e| {
                warn!(
                    "error scanning columns for {}.{}: {:?}",
                    schema_name, table_name, e
                );
                vec![]
            });
            let constraints = con_res.unwrap_or_else(|e| {
                warn!(
                    "error scanning constraints for {}.{}: {:?}",
                    schema_name, table_name, e
                );
                vec![]
            });
            let indexes = idx_res.unwrap_or_else(|e| {
                warn!(
                    "error scanning indexes for {}.{}: {:?}",
                    schema_name, table_name, e
                );
                vec![]
            });
            let triggers = trig_res.unwrap_or_else(|e| {
                warn!(
                    "error scanning triggers for {}.{}: {:?}",
                    schema_name, table_name, e
                );
                vec![]
            });

            schema.tables.push(Table {
                oid,
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
            SELECT v.table_schema, v.table_name, v.view_definition, v.is_updatable, c.oid
            FROM information_schema.views v
            JOIN pg_namespace n ON n.nspname = v.table_schema
            JOIN pg_class c ON c.relname = v.table_name AND c.relnamespace = n.oid
            WHERE v.table_schema NOT IN ('information_schema', 'pg_catalog')
        ";

        info!("scanning views");
        match self.client.query(views_query, &[]).await {
            Ok(view_rows) => {
                for row in view_rows {
                    let schema_name: String = row.get_string("table_schema");
                    let view_name: String = row.get_string("table_name");
                    let definition: Option<String> = row.get_opt_string("view_definition");
                    let is_updatable = row.get_str("is_updatable") == "YES";
                    let oid: u32 = row.get_u32("oid");

                    let schema = schemas_map
                        .entry(schema_name.clone())
                        .or_insert_with(|| Schema {
                            name: schema_name.clone(),
                            ..Default::default()
                        });

                    schema.views.push(View {
                        oid,
                        name: view_name,
                        schema_name: schema_name.clone(),
                        definition: definition.unwrap_or_default(),
                        is_updatable,
                    });
                }
            }
            Err(e) => {
                warn!("error scanning views: {:?}", e);
            }
        }

        info!("scanning enums");
        if let Err(e) = self.scan_enums(&mut schemas_map).await {
            warn!("error scanning enums {:#?}", e);
        }
        info!("scanning sequences");
        if let Err(e) = self.scan_sequences(&mut schemas_map).await {
            warn!("error scanning sequences {:#?}", e)
        }
        info!("scanning functions");
        if let Err(e) = self.scan_functions(&mut schemas_map).await {
            warn!("error scanning functions {:#?}", e)
        }

        let mut schemas: Vec<_> = schemas_map.into_values().collect();
        schemas.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(Database {
            name: database_name.to_string(),
            schemas,
        })
    }

    async fn scan_columns(
        &self,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Vec<Column>, String> {
        let columns_query = "
            SELECT column_name, data_type, is_nullable, column_default, character_maximum_length
            FROM information_schema.columns
            WHERE table_schema = $1 AND table_name = $2
            ORDER BY ordinal_position
        ";

        let col_rows = self
            .client
            .query(columns_query, &[&schema_name, &table_name])
            .await?;
        let mut columns = Vec::new();

        for col_row in col_rows {
            let col_name: String = col_row.get_string("column_name");
            let data_type = map_data_type(
                col_row.get_str("data_type"),
                col_row.get_opt_i32("character_maximum_length"),
            );
            let is_nullable = col_row.get_str("is_nullable") == "YES";
            let column_default: Option<String> = col_row.get_opt_string("column_default");

            columns.push(Column {
                name: col_name,
                data_type,
                is_nullable,
                default_value: column_default,
                comment: None,
            });
        }

        Ok(columns)
    }

    async fn scan_constraints(
        &self,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Vec<Constraint>, String> {
        let mut constraints = Vec::new();

        let constraints_query = "
            SELECT
                tc.constraint_name,
                tc.constraint_type,
                rc.update_rule,
                rc.delete_rule,
                ccu.table_schema AS foreign_schema,
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
            .query(constraints_query, &[&schema_name, &table_name])
            .await?;

        struct ConstraintGroup {
            ctype: String,
            local_cols: Vec<String>,
            foreign_schema: Option<String>,
            foreign_table: Option<String>,
            update_action: Option<ReferentialAction>,
            delete_action: Option<ReferentialAction>,
            foreign_cols: Vec<String>,
        }

        let mut constraint_map: HashMap<String, ConstraintGroup> = HashMap::new();

        for row in rows {
            let name: String = row.get_string("constraint_name");
            let ctype: String = row.get_string("constraint_type");
            let local_col: String = row.get_string("local_column");
            let foreign_schema: Option<String> = row.get_opt_string("foreign_schema");
            let foreign_table: Option<String> = row.get_opt_string("foreign_table");
            let foreign_col: Option<String> = row.get_opt_string("foreign_column");
            let update_rule: Option<String> = row.get_opt_string("update_rule");
            let delete_rule: Option<String> = row.get_opt_string("delete_rule");

            let entry = constraint_map
                .entry(name.clone())
                .or_insert_with(|| ConstraintGroup {
                    ctype,
                    local_cols: Vec::new(),
                    foreign_schema,
                    foreign_table,
                    update_action: update_rule.map(map_referential_action),
                    delete_action: delete_rule.map(map_referential_action),
                    foreign_cols: Vec::new(),
                });

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
                    foreign_schema: group.foreign_schema.unwrap_or_default(),
                    foreign_table: group.foreign_table.unwrap_or_default(),
                    foreign_columns: group.foreign_cols,
                    on_delete: group.delete_action.unwrap_or(ReferentialAction::NoAction),
                    on_update: group.update_action.unwrap_or(ReferentialAction::NoAction),
                },
                _ => continue,
            };

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
            .query(check_query, &[&schema_name, &table_name])
            .await?
        {
            let name: String = row.get_string("constraint_name");
            let clause: String = row.get_string("check_clause");
            let column: Option<String> = row.get_opt_string("column_name");

            let entry = check_map.entry(name).or_insert((clause, Vec::new()));
            if let Some(col) = column {
                entry.1.push(col);
            }
        }

        for (name, (clause, columns)) in check_map {
            constraints.push(Constraint {
                name,
                columns,
                constraint_type: ConstraintType::Check(clause),
            });
        }

        Ok(constraints)
    }

    async fn scan_indexes(
        &self,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Vec<Index>, String> {
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

        let rows = self
            .client
            .query(index_query, &[&schema_name, &table_name])
            .await?;

        for row in rows {
            let name: String = row.get_string("index_name");
            let index_type: String = row.get_string("index_type");
            let is_unique: bool = row.get_bool("is_unique");
            let is_primary: bool = row.get_bool("is_primary");
            let definition: String = row.get_string("index_definition");
            let partial_condition: Option<String> = row.get_opt_string("partial_condition");
            let columns: Vec<String> = row.get_vec_string("index_columns");

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

    async fn scan_triggers(
        &self,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Vec<Trigger>, String> {
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
            .query(trigger_query, &[&schema_name, &table_name])
            .await?;
        let mut triggers = Vec::new();

        for row in rows {
            triggers.push(Trigger {
                name: row.get_string("trigger_name"),
                event_manipulation: row.get_string("event_manipulation"),
                action_statement: row.get_string("action_statement"),
                action_timing: row.get_string("action_timing"),
                action_condition: row.get_opt_string("action_condition"),
            });
        }

        Ok(triggers)
    }

    async fn scan_enums(&self, schemas_map: &mut HashMap<String, Schema>) -> Result<(), String> {
        let enum_query = "
            SELECT
                n.nspname AS schema_name,
                t.typname AS enum_name,
                t.oid AS enum_oid,
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
                n.nspname, t.typname, t.oid;
        ";

        let rows = self.client.query(enum_query, &[]).await?;

        for row in rows {
            let schema_name: String = row.get_string("schema_name");
            let enum_name: String = row.get_string("enum_name");
            let variants: Vec<String> = row.get_vec_string("variants");
            let oid: u32 = row.get_u32("enum_oid");

            let schema = schemas_map
                .entry(schema_name.clone())
                .or_insert_with(|| Schema {
                    name: schema_name.clone(),
                    ..Default::default()
                });

            schema.enums.push(EnumType {
                oid,
                name: enum_name,
                schema_name,
                variants,
            });
        }

        Ok(())
    }

    async fn scan_sequences(
        &self,
        schemas_map: &mut HashMap<String, Schema>,
    ) -> Result<(), String> {
        let seq_query = "
            SELECT
                sequence_schema,
                sequence_name,
                start_value::bigint,
                increment::bigint,
                minimum_value::bigint,
                maximum_value::bigint,
                cycle_option,
                c.oid
            FROM
                information_schema.sequences s
            JOIN pg_namespace n ON n.nspname = s.sequence_schema
            JOIN pg_class c ON c.relname = s.sequence_name AND c.relnamespace = n.oid
            WHERE
                s.sequence_schema NOT IN ('information_schema', 'pg_catalog');
        ";

        let rows = self.client.query(seq_query, &[]).await?;

        for row in rows {
            let schema_name: String = row.get_string("sequence_schema");
            let name: String = row.get_string("sequence_name");
            let start_value: i64 = row.get_i64("start_value");
            let increment: i64 = row.get_i64("increment");
            let min_value: i64 = row.get_i64("minimum_value");
            let max_value: i64 = row.get_i64("maximum_value");
            let cycle = row.get_str("cycle_option") == "YES";
            let oid: u32 = row.get_u32("oid");

            let schema = schemas_map
                .entry(schema_name.clone())
                .or_insert_with(|| Schema {
                    name: schema_name.clone(),
                    ..Default::default()
                });

            schema.sequences.push(crate::schema::Sequence {
                oid,
                name,
                schema_name,
                start_value,
                increment_by: increment,
                min_value,
                max_value,
                cycle,
            });
        }

        Ok(())
    }

    async fn scan_functions(
        &self,
        schemas_map: &mut HashMap<String, Schema>,
    ) -> Result<(), String> {
        let routine_query = "
            SELECT
                routine_schema,
                routine_name,
                routine_type,
                data_type AS return_type,
                routine_definition,
                external_language,
                (SELECT p.oid FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace WHERE n.nspname = routine_schema AND p.proname = routine_name LIMIT 1) as oid
            FROM information_schema.routines
            WHERE routine_schema NOT IN ('information_schema', 'pg_catalog')
        ";

        let rows = self.client.query(routine_query, &[]).await?;

        for row in rows {
            let schema_name: Option<String> = row.try_get_string("routine_schema").ok();
            let routine_name: Option<String> = row.try_get_string("routine_name").ok();
            let is_procedure = row
                .try_get_str("routine_type")
                .is_ok_and(|t| t == "PROCEDURE");
            let return_type: Option<String> = row.try_get_string("return_type").ok();
            let definition: Option<String> = row.try_get_string("routine_definition").ok();
            let language = row
                .try_get_str("external_language")
                .unwrap_or("sql")
                .to_string();
            let oid: Option<u32> = row.try_get_u32("oid").ok();

            if let (Some(s_name), Some(r_name)) = (schema_name, routine_name) {
                let schema = schemas_map.entry(s_name.clone()).or_insert_with(|| Schema {
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

                let param_rows = self.client.query(param_query, &[&s_name, &r_name]).await?;
                let argument_types = param_rows
                    .iter()
                    .map(|r| r.get_string("data_type"))
                    .collect();

                schema.functions.push(Function {
                    oid: oid.unwrap_or(0),
                    name: r_name,
                    schema_name: s_name,
                    argument_types,
                    return_type: return_type.unwrap_or_else(|| "void".to_string()),
                    definition: definition.unwrap_or_default(),
                    language,
                    is_procedure,
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
        "character varying" => PostgresDataType::Varchar {
            length: char_len.map(|l| l as u32),
        },
        "character" => PostgresDataType::Character {
            length: char_len.map(|l| l as u32),
        },
        "timestamp without time zone" => PostgresDataType::Timestamp {
            with_time_zone: false,
        },
        "timestamp with time zone" => PostgresDataType::Timestamp {
            with_time_zone: true,
        },
        "date" => PostgresDataType::Date,
        "json" => PostgresDataType::Json,
        "jsonb" => PostgresDataType::Jsonb,
        "uuid" => PostgresDataType::Uuid,
        _ => PostgresDataType::Custom {
            name: dt.to_string(),
        },
    }
}

fn map_referential_action(action: String) -> ReferentialAction {
    match action.as_str() {
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
    use crate::scanner::mock::mock_client::MockClient;
    use serde_json::json;

    #[tokio::test]
    async fn test_mock_scanner() {
        let mut mock = MockClient::new();

        mock.add_response(
            "SELECT oid, nspname",
            json!([
                { "oid": 1000, "nspname": "public" }
            ]),
        );

        mock.add_response(
            "information_schema.tables",
            json!([
                { "table_schema": "public", "table_name": "users", "oid": 2000 }
            ]),
        );

        mock.add_response(
            "information_schema.columns",
            json!([
                {
                    "column_name": "id",
                    "data_type": "uuid",
                    "is_nullable": "NO",
                    "column_default": null,
                    "character_maximum_length": null
                }
            ]),
        );

        let scanner = PostgresScanner::new(&mock);
        let database = scanner.scan("test_db").await.expect("scan failed");

        assert_eq!(database.schemas.len(), 1);
        assert_eq!(database.schemas[0].name, "public");
        assert_eq!(database.schemas[0].tables.len(), 1);
        assert_eq!(database.schemas[0].tables[0].name, "users");
        assert_eq!(database.schemas[0].tables[0].columns.len(), 1);
        assert_eq!(database.schemas[0].tables[0].columns[0].name, "id");
    }

    #[tokio::test]
    async fn test_scan_columns() {
        let mut mock = MockClient::new();
        mock.add_response(
            "information_schema.columns",
            json!([
                {
                    "column_name": "id",
                    "data_type": "uuid",
                    "is_nullable": "NO",
                    "column_default": null,
                    "character_maximum_length": null
                },
                {
                    "column_name": "name",
                    "data_type": "character varying",
                    "is_nullable": "YES",
                    "column_default": "'default_name'",
                    "character_maximum_length": 255
                }
            ]),
        );

        let scanner = PostgresScanner::new(&mock);
        let columns = scanner
            .scan_columns("public", "users")
            .await
            .expect("scan_columns failed");

        assert_eq!(columns.len(), 2);

        let id_col = &columns[0];
        assert_eq!(id_col.name, "id");
        assert!(matches!(id_col.data_type, PostgresDataType::Uuid));
        assert!(!id_col.is_nullable);
        assert_eq!(id_col.default_value, None);

        let name_col = &columns[1];
        assert_eq!(name_col.name, "name");
        assert!(matches!(
            name_col.data_type,
            PostgresDataType::Varchar { length: Some(255) }
        ));
        assert!(name_col.is_nullable);
        assert_eq!(name_col.default_value, Some("'default_name'".to_string()));
    }

    #[tokio::test]
    async fn test_scan_constraints() {
        let mut mock = MockClient::new();

        mock.add_response(
            "ccu.table_schema AS foreign_schema",
            json!([
                {
                    "constraint_name": "users_pkey",
                    "constraint_type": "PRIMARY KEY",
                    "update_rule": null,
                    "delete_rule": null,
                    "foreign_schema": null,
                    "foreign_table": null,
                    "foreign_column": null,
                    "local_column": "id"
                },
                {
                    "constraint_name": "fk_role",
                    "constraint_type": "FOREIGN KEY",
                    "update_rule": "NO ACTION",
                    "delete_rule": "CASCADE",
                    "foreign_schema": "public",
                    "foreign_table": "roles",
                    "foreign_column": "role_id",
                    "local_column": "role_id"
                }
            ]),
        );

        mock.add_response(
            "information_schema.check_constraints",
            json!([
                {
                    "constraint_name": "age_check",
                    "check_clause": "age > 18",
                    "column_name": "age"
                }
            ]),
        );

        let scanner = PostgresScanner::new(&mock);
        let constraints = scanner
            .scan_constraints("public", "users")
            .await
            .expect("scan_constraints failed");

        assert_eq!(constraints.len(), 3);

        let pkey = constraints.iter().find(|c| c.name == "users_pkey").unwrap();
        assert_eq!(pkey.columns, vec!["id"]);
        assert!(matches!(pkey.constraint_type, ConstraintType::PrimaryKey));

        let fkey = constraints.iter().find(|c| c.name == "fk_role").unwrap();
        assert_eq!(fkey.columns, vec!["role_id"]);
        if let ConstraintType::ForeignKey {
            foreign_schema,
            foreign_table,
            foreign_columns,
            on_delete,
            ..
        } = &fkey.constraint_type
        {
            assert_eq!(foreign_schema, "public");
            assert_eq!(foreign_table, "roles");
            assert_eq!(foreign_columns, &vec!["role_id"]);
            assert!(matches!(on_delete, ReferentialAction::Cascade));
        } else {
            panic!("Expected ForeignKey");
        }

        let check = constraints.iter().find(|c| c.name == "age_check").unwrap();
        assert_eq!(check.columns, vec!["age"]);
        if let ConstraintType::Check(clause) = &check.constraint_type {
            assert_eq!(clause, "age > 18");
        } else {
            panic!("Expected Check constraint");
        }
    }

    #[tokio::test]
    async fn test_scan_indexes() {
        let mut mock = MockClient::new();
        mock.add_response(
            "pg_index AS idx",
            json!([
                {
                    "index_name": "idx_users_email",
                    "index_type": "btree",
                    "is_unique": true,
                    "is_primary": false,
                    "index_definition": "CREATE UNIQUE INDEX idx_users_email ON public.users USING btree (email)",
                    "partial_condition": null,
                    "index_columns": ["email"]
                }
            ]),
        );

        let scanner = PostgresScanner::new(&mock);
        let indexes = scanner
            .scan_indexes("public", "users")
            .await
            .expect("scan_indexes failed");

        assert_eq!(indexes.len(), 1);
        let idx = &indexes[0];
        assert_eq!(idx.name, "idx_users_email");
        assert_eq!(idx.index_type, "btree");
        assert!(idx.is_unique);
        assert!(!idx.is_primary_key);
        assert_eq!(idx.columns, vec!["email"]);
        assert_eq!(idx.partial_condition, None);
    }

    #[tokio::test]
    async fn test_scan_triggers() {
        let mut mock = MockClient::new();
        mock.add_response(
            "information_schema.triggers",
            json!([
                {
                    "trigger_name": "update_updated_at",
                    "event_manipulation": "UPDATE",
                    "action_statement": "EXECUTE FUNCTION update_timestamp()",
                    "action_timing": "BEFORE",
                    "action_condition": null
                }
            ]),
        );

        let scanner = PostgresScanner::new(&mock);
        let triggers = scanner
            .scan_triggers("public", "users")
            .await
            .expect("scan_triggers failed");

        assert_eq!(triggers.len(), 1);
        let tg = &triggers[0];
        assert_eq!(tg.name, "update_updated_at");
        assert_eq!(tg.event_manipulation, "UPDATE");
        assert_eq!(tg.action_statement, "EXECUTE FUNCTION update_timestamp()");
        assert_eq!(tg.action_timing, "BEFORE");
    }

    #[tokio::test]
    async fn test_scan_enums() {
        let mut mock = MockClient::new();
        mock.add_response(
            "pg_enum e",
            json!([
                {
                    "schema_name": "public",
                    "enum_name": "user_status",
                    "enum_oid": 3000,
                    "variants": ["active", "inactive", "banned"]
                }
            ]),
        );

        let scanner = PostgresScanner::new(&mock);
        let mut schemas_map = std::collections::HashMap::new();
        scanner
            .scan_enums(&mut schemas_map)
            .await
            .expect("scan_enums failed");

        assert_eq!(schemas_map.len(), 1);
        let schema = schemas_map.get("public").unwrap();
        assert_eq!(schema.enums.len(), 1);

        let e = &schema.enums[0];
        assert_eq!(e.name, "user_status");
        assert_eq!(e.oid, 3000);
        assert_eq!(e.variants, vec!["active", "inactive", "banned"]);
    }

    #[tokio::test]
    async fn test_scan_sequences() {
        let mut mock = MockClient::new();
        mock.add_response(
            "information_schema.sequences",
            json!([
                {
                    "sequence_schema": "public",
                    "sequence_name": "users_id_seq",
                    "start_value": 1,
                    "increment": 1,
                    "minimum_value": 1,
                    "maximum_value": 9223372036854775807_i64,
                    "cycle_option": "NO",
                    "oid": 4000
                }
            ]),
        );

        let scanner = PostgresScanner::new(&mock);
        let mut schemas_map = std::collections::HashMap::new();
        scanner
            .scan_sequences(&mut schemas_map)
            .await
            .expect("scan_sequences failed");

        assert_eq!(schemas_map.len(), 1);
        let schema = schemas_map.get("public").unwrap();
        assert_eq!(schema.sequences.len(), 1);

        let seq = &schema.sequences[0];
        assert_eq!(seq.name, "users_id_seq");
        assert_eq!(seq.start_value, 1);
        assert_eq!(seq.increment_by, 1);
        assert_eq!(seq.min_value, 1);
        assert_eq!(seq.max_value, 9223372036854775807);
        assert!(!seq.cycle);
    }

    #[tokio::test]
    async fn test_scan_functions() {
        let mut mock = MockClient::new();

        mock.add_response(
            "WHERE routine_schema NOT IN",
            json!([
                {
                    "routine_schema": "public",
                    "routine_name": "calculate_tax",
                    "routine_type": "FUNCTION",
                    "return_type": "numeric",
                    "routine_definition": "BEGIN RETURN amount * 0.2; END;",
                    "external_language": "plpgsql",
                    "oid": 5000
                }
            ]),
        );

        mock.add_response(
            "information_schema.parameters",
            json!([
                { "data_type": "numeric" }
            ]),
        );

        let scanner = PostgresScanner::new(&mock);
        let mut schemas_map = std::collections::HashMap::new();
        scanner
            .scan_functions(&mut schemas_map)
            .await
            .expect("scan_functions failed");

        assert_eq!(schemas_map.len(), 1);
        let schema = schemas_map.get("public").unwrap();
        assert_eq!(schema.functions.len(), 1);

        let func = &schema.functions[0];
        assert_eq!(func.name, "calculate_tax");
        assert_eq!(func.oid, 5000);
        assert_eq!(func.return_type, "numeric");
        assert_eq!(func.argument_types, vec!["numeric"]);
        assert_eq!(func.language, "plpgsql");
        assert!(!func.is_procedure);
    }
}
