use std::collections::HashMap;

/// Represents a Postgres Data Type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    Timestamp(bool), // with/without time zone
    Date,
    Time(bool), // with/without time zone
    Interval,
    Json,
    Jsonb,
    Uuid,
    Inet,
    Cidr,
    MacAddr,
    Array(Box<PostgresDataType>),
    Custom(String), // For user-defined types/enums
}

/// Represents the action to take on a foreign key during updates or deletions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferentialAction {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

/// Represents the type of a table-level constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Represents a column within a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub data_type: PostgresDataType,
    pub is_nullable: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

/// Represents an index on a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    pub name: String,
    pub is_unique: bool,
    pub is_primary_key: bool,
    pub columns: Vec<String>,
    pub definition: String, // Full SQL definition from pg_get_indexdef
}

/// Represents a table-level or column-level constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub name: String,
    pub columns: Vec<String>, // Local columns affected by the constraint
    pub constraint_type: ConstraintType,
}

/// Represents a trigger on a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub name: String,
    pub event_manipulation: String, // INSERT, UPDATE, DELETE
    pub action_statement: String,
    pub action_timing: String,      // BEFORE, AFTER, INSTEAD OF
}

/// Represents a table in a Postgres schema.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        let pk_columns: Vec<String> = self.constraints.iter()
            .filter(|c| matches!(c.constraint_type, ConstraintType::PrimaryKey))
            .flat_map(|c| c.columns.clone())
            .collect();
        
        self.columns.iter()
            .filter(|col| pk_columns.contains(&col.name))
            .collect()
    }

    pub fn is_foreign_key(&self, column_name: &str) -> bool {
        self.constraints.iter().any(|c| {
            matches!(c.constraint_type, ConstraintType::ForeignKey { .. }) &&
            c.columns.contains(&column_name.to_string())
        })
    }
}

/// Represents a database view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub name: String,
    pub schema_name: String,
    pub definition: String,
    pub is_updatable: bool,
}

/// Represents a user-defined Enum type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumType {
    pub name: String,
    pub schema_name: String,
    pub variants: Vec<String>,
}

/// Represents a Postgres sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sequence {
    pub name: String,
    pub schema_name: String,
    pub start_value: i64,
    pub increment_by: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub cycle: bool,
}

/// Represents a Postgres function or procedure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub schema_name: String,
    pub argument_types: Vec<String>,
    pub return_type: String,
    pub definition: String,
    pub language: String,
    pub is_procedure: bool,
}

/// Represents a Postgres Schema (e.g., 'public', 'auth').
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schema {
    pub name: String,
    pub tables: Vec<Table>,
    pub views: Vec<View>,
    pub enums: Vec<EnumType>,
    pub functions: Vec<Function>,
    pub sequences: Vec<Sequence>,
}

/// Represents a full Postgres Database.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Database {
    pub name: String,
    pub schemas: HashMap<String, Schema>,
}
