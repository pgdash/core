use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferentialAction {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: PostgresDataType,
    pub is_nullable: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub name: String,
    pub is_unique: bool,
    pub is_primary_key: bool,
    pub columns: Vec<String>,
    pub index_type: String,                // e.g., btree, gin, hash
    pub partial_condition: Option<String>, // the WHERE clause for partial indexes
    pub definition: String,                // Full SQL definition from using pg_get_indexdef
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constraint {
    pub name: String,
    pub columns: Vec<String>, // local columns affected by the constraint
    pub constraint_type: ConstraintType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trigger {
    pub name: String,
    pub event_manipulation: String, // can be INSERT, UPDATE, DELETE
    pub action_statement: String,
    pub action_timing: String, // can be BEFORE, AFTER, INSTEAD OF
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        let pk_columns: Vec<String> = self
            .constraints
            .iter()
            .filter(|c| matches!(c.constraint_type, ConstraintType::PrimaryKey))
            .flat_map(|c| c.columns.clone())
            .collect();

        self.columns
            .iter()
            .filter(|col| pk_columns.contains(&col.name))
            .collect()
    }

    pub fn is_foreign_key(&self, column_name: &str) -> bool {
        self.constraints.iter().any(|c| {
            matches!(c.constraint_type, ConstraintType::ForeignKey { .. })
                && c.columns.contains(&column_name.to_string())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    pub name: String,
    pub schema_name: String,
    pub definition: String,
    pub is_updatable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumType {
    pub name: String,
    pub schema_name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sequence {
    pub name: String,
    pub schema_name: String,
    pub start_value: i64,
    pub increment_by: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub cycle: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub schema_name: String,
    pub argument_types: Vec<String>,
    pub return_type: String,
    pub definition: String,
    pub language: String,
    pub is_procedure: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub tables: Vec<Table>,
    pub views: Vec<View>,
    pub enums: Vec<EnumType>,
    pub functions: Vec<Function>,
    pub sequences: Vec<Sequence>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Database {
    pub name: String,
    pub schemas: HashMap<String, Schema>,
}
