use crate::schema::{
    Column, Constraint, ConstraintType, Database, EnumType, Function, Index, PostgresDataType,
    ReferentialAction, Schema, Table, Trigger, View,
};
use std::collections::HashMap;
use tokio_postgres::{Client, Error};
use tracing::{info, warn};

pub struct PostgresScanner<'a> {
    client: &'a Client,
}

impl<'a> PostgresScanner<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn scan(&self, database_name: &str) -> Result<Database, Error> {
        let mut schemas_map: HashMap<String, Schema> = HashMap::new();

        let schema_query = "SELECT oid, nspname FROM pg_namespace WHERE nspname NOT IN ('information_schema', 'pg_catalog', 'pg_toast')";

        info!("Scanning schemas");
        for row in self.client.query(schema_query, &[]).await? {
            let oid: u32 = row.get("oid");
            let name: String = row.get("nspname");
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
            let schema_name: String = row.get("table_schema");
            let table_name: String = row.get("table_name");
            let oid: u32 = row.get("oid");
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
                warn!("error scanning columns for {}.{}: {:?}", schema_name, table_name, e);
                vec![]
            });
            let constraints = con_res.unwrap_or_else(|e| {
                warn!("error scanning constraints for {}.{}: {:?}", schema_name, table_name, e);
                vec![]
            });
            let indexes = idx_res.unwrap_or_else(|e| {
                warn!("error scanning indexes for {}.{}: {:?}", schema_name, table_name, e);
                vec![]
            });
            let triggers = trig_res.unwrap_or_else(|e| {
                warn!("error scanning triggers for {}.{}: {:?}", schema_name, table_name, e);
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
                    let schema_name: String = row.get("table_schema");
                    let view_name: String = row.get("table_name");
                    let definition: Option<String> = row.get("view_definition");
                    let is_updatable_str: String = row.get("is_updatable");
                    let oid: u32 = row.get("oid");

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
                        is_updatable: is_updatable_str == "YES",
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
    ) -> Result<Vec<Column>, Error> {
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
            let col_name: String = col_row.get("column_name");
            let data_type_str: String = col_row.get("data_type");
            let is_nullable_str: String = col_row.get("is_nullable");
            let column_default: Option<String> = col_row.get("column_default");
            let char_len: Option<i32> = col_row.get("character_maximum_length");

            let data_type = map_data_type(data_type_str.as_ref(), char_len);

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

    async fn scan_constraints(
        &self,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Vec<Constraint>, Error> {
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
            let name: String = row.get("constraint_name");
            let ctype: String = row.get("constraint_type");
            let local_col: String = row.get("local_column");
            let foreign_schema: Option<String> = row.get("foreign_schema");
            let foreign_table: Option<String> = row.get("foreign_table");
            let foreign_col: Option<String> = row.get("foreign_column");
            let update_rule: Option<String> = row.get("update_rule");
            let delete_rule: Option<String> = row.get("delete_rule");

            let entry = constraint_map
                .entry(name.clone())
                .or_insert_with(|| ConstraintGroup {
                    ctype,
                    local_cols: Vec::new(),
                    foreign_schema,
                    foreign_table,
                    update_action: update_rule.map(|r| map_referential_action(&r)),
                    delete_action: delete_rule.map(|r| map_referential_action(&r)),
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
            let name: String = row.get("constraint_name");
            let clause: String = row.get("check_clause");
            let column: Option<String> = row.get("column_name");

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

    async fn scan_indexes(&self, schema_name: &str, table_name: &str) -> Result<Vec<Index>, Error> {
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
            let name: String = row.get("index_name");
            let index_type: String = row.get("index_type");
            let is_unique: bool = row.get("is_unique");
            let is_primary: bool = row.get("is_primary");
            let definition: String = row.get("index_definition");
            let partial_condition: Option<String> = row.get("partial_condition");
            let columns: Vec<String> = row.get("index_columns");

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
    ) -> Result<Vec<Trigger>, Error> {
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
                name: row.get("trigger_name"),
                event_manipulation: row.get("event_manipulation"),
                action_statement: row.get("action_statement"),
                action_timing: row.get("action_timing"),
                action_condition: row.get("action_condition"),
            });
        }

        Ok(triggers)
    }

    async fn scan_enums(&self, schemas_map: &mut HashMap<String, Schema>) -> Result<(), Error> {
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
            let schema_name: String = row.get("schema_name");
            let enum_name: String = row.get("enum_name");
            let variants: Vec<String> = row.get("variants");
            let oid: u32 = row.get("enum_oid");

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

    async fn scan_sequences(&self, schemas_map: &mut HashMap<String, Schema>) -> Result<(), Error> {
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
            let schema_name: String = row.get("sequence_schema");
            let name: String = row.get("sequence_name");
            let start_value: i64 = row.get("start_value");
            let increment: i64 = row.get("increment");
            let min_value: i64 = row.get("minimum_value");
            let max_value: i64 = row.get("maximum_value");
            let cycle_option: String = row.get("cycle_option");
            let oid: u32 = row.get("oid");

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
                cycle: cycle_option == "YES",
            });
        }

        Ok(())
    }

    async fn scan_functions(&self, schemas_map: &mut HashMap<String, Schema>) -> Result<(), Error> {
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
            let schema_name: Option<String> = row.try_get("routine_schema").ok();
            let routine_name: Option<String> = row.try_get("routine_name").ok();
            let routine_type: Option<String> = row.try_get("routine_type").ok();
            let return_type: Option<String> = row.try_get("return_type").ok();
            let definition: Option<String> = row.try_get("routine_definition").ok();
            let language: Option<String> = row.try_get("external_language").ok();
            let oid: Option<u32> = row.try_get("oid").ok();

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
                let argument_types = param_rows.iter().map(|r| r.get("data_type")).collect();

                schema.functions.push(Function {
                    oid: oid.unwrap_or(0),
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

fn map_referential_action(action: &str) -> ReferentialAction {
    match action {
        "CASCADE" => ReferentialAction::Cascade,
        "SET NULL" => ReferentialAction::SetNull,
        "SET DEFAULT" => ReferentialAction::SetDefault,
        "RESTRICT" => ReferentialAction::Restrict,
        _ => ReferentialAction::NoAction,
    }
}
