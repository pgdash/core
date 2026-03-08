use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[schema(no_recursion)]
pub enum PostgresDataType {
    Boolean,
    SmallInt,
    Integer,
    BigInt,
    Real,
    DoublePrecision,
    Numeric(Option<u32>, Option<u32>), // precision, scale
    Character(Option<u32>),
    Varchar(Option<u32>),
    Text,
    Bytea,
    Date,
    Timestamp(bool), // bool in team stands for with/without time zone
    Time(bool),
    Interval,
    Json,
    Jsonb,
    Uuid,
    Inet,
    Cidr,
    MacAddr,
    Array(Box<PostgresDataType>),
    Custom(String), // for user-defined types or enums
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub enum ReferentialAction {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub enum ConstraintType {
    PrimaryKey,
    Unique,
    Check(String), // constraint definition
    ForeignKey {
        foreign_table: String,
        foreign_columns: Vec<String>,
        on_delete: ReferentialAction,
        on_update: ReferentialAction,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Column {
    pub name: String,
    pub data_type: PostgresDataType,
    pub is_nullable: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Index {
    pub name: String,
    pub is_unique: bool,
    pub is_primary_key: bool,
    pub columns: Vec<String>,
    pub index_type: String,                // e.g., btree, gin, hash
    pub partial_condition: Option<String>, // the WHERE clause for partial indexes
    pub definition: String,                // Full SQL definition from using pg_get_indexdef
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Constraint {
    pub name: String,
    pub columns: Vec<String>, // local columns affected by the constraint
    pub constraint_type: ConstraintType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Trigger {
    pub name: String,
    pub event_manipulation: String, // can be INSERT, UPDATE, DELETE
    pub action_statement: String,
    pub action_timing: String, // can be BEFORE, AFTER, INSTEAD OF
    pub action_condition: Option<String>, // the WHEN clause
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Table {
    pub name: String,
    pub schema_name: String,
    pub columns: Vec<Column>,
    pub indexes: Vec<Index>,
    pub constraints: Vec<Constraint>,
    pub triggers: Vec<Trigger>,
    pub comment: Option<String>,
}

impl Table {
    pub fn get_primary_key_columns(&self) -> Vec<&Column> {
        // Zero-cost abstraction: Collect references rather than cloning Strings into a new Vec
        let pk_columns: Vec<&String> = self
            .constraints
            .iter()
            .filter(|c| matches!(c.constraint_type, ConstraintType::PrimaryKey))
            .flat_map(|c| c.columns.iter())
            .collect();

        self.columns
            .iter()
            .filter(|col| pk_columns.contains(&&col.name))
            .collect()
    }

    pub fn is_foreign_key(&self, column_name: &str) -> bool {
        self.constraints.iter().any(|c| {
            matches!(c.constraint_type, ConstraintType::ForeignKey { .. })
                // Zero-cost abstraction: Use .iter().any to avoid heap allocating with .to_string()
                && c.columns.iter().any(|col| col == column_name)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct View {
    pub name: String,
    pub schema_name: String,
    pub definition: String,
    pub is_updatable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EnumType {
    pub name: String,
    pub schema_name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Sequence {
    pub name: String,
    pub schema_name: String,
    pub start_value: i64,
    pub increment_by: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub cycle: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Function {
    pub name: String,
    pub schema_name: String,
    pub argument_types: Vec<String>,
    pub return_type: String,
    pub definition: String,
    pub language: String,
    pub is_procedure: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Schema {
    pub name: String,
    pub tables: Vec<Table>,
    pub views: Vec<View>,
    pub enums: Vec<EnumType>,
    pub functions: Vec<Function>,
    pub sequences: Vec<Sequence>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Database {
    pub name: String,
    pub schemas: HashMap<String, Schema>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_get_primary_key_columns() {
        let col1 = Column {
            name: "id".to_string(),
            data_type: PostgresDataType::Integer,
            is_nullable: false,
            default_value: None,
            comment: None,
        };
        let col2 = Column {
            name: "name".to_string(),
            data_type: PostgresDataType::Text,
            is_nullable: true,
            default_value: None,
            comment: None,
        };

        let table = Table {
            name: "users".to_string(),
            schema_name: "public".to_string(),
            columns: vec![col1.clone(), col2.clone()],
            indexes: vec![],
            constraints: vec![Constraint {
                name: "pk_users".to_string(),
                columns: vec!["id".to_string()],
                constraint_type: ConstraintType::PrimaryKey,
            }],
            triggers: vec![],
            comment: None,
        };

        let pk_cols = table.get_primary_key_columns();
        assert_eq!(pk_cols.len(), 1);
        assert_eq!(pk_cols[0].name, "id");
    }

    #[test]
    fn test_table_is_foreign_key() {
        let table = Table {
            name: "posts".to_string(),
            schema_name: "public".to_string(),
            columns: vec![],
            indexes: vec![],
            constraints: vec![Constraint {
                name: "fk_user".to_string(),
                columns: vec!["user_id".to_string()],
                constraint_type: ConstraintType::ForeignKey {
                    foreign_table: "users".to_string(),
                    foreign_columns: vec!["id".to_string()],
                    on_delete: ReferentialAction::Cascade,
                    on_update: ReferentialAction::NoAction,
                },
            }],
            triggers: vec![],
            comment: None,
        };

        assert!(table.is_foreign_key("user_id"));
        assert!(!table.is_foreign_key("title"));
    }
}
