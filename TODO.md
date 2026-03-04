# pgdash Core - Roadmap

## 1. Schema Module Enhancements (`src/schema/mod.rs`)
- [x] **Serde Support**: Add `#[derive(Serialize, Deserialize)]` to all structs and enums.
- [ ] **Constraint Details**: Expand `ConstraintType` to include full metadata for Foreign Keys (on_delete, on_update actions).
- [ ] **Index Details**: Capture index types (B-tree, GIN, etc.) and partial index conditions.
- [ ] **Trigger Logic**: Parse trigger functions and execution conditions.
- [ ] **Full Type Coverage**: Complete the `PostgresDataType` mapping (including Range types, Geometry, etc.).
- [ ] **JSON/YAML Export**: Add methods to serialize the `Database` struct to common formats.

## 2. Scanner Module Enhancements (`src/scanner/mod.rs`)
- [ ] **Primary & Foreign Keys**: Implement scanning logic for `information_schema.table_constraints` and `key_column_usage`.
- [ ] **Indexes**: Query `pg_indexes` and `pg_get_indexdef` to populate index metadata.
- [ ] **Enums**: Scan `pg_type` and `pg_enum` to extract user-defined enum variants.
- [ ] **Sequences**: Query `information_schema.sequences`.
- [ ] **Functions/Procedures**: Query `information_schema.routines` to extract SQL/Plpgsql definitions.
- [ ] **Async Support**: Explore `tokio-postgres` for non-blocking scanning.
- [ ] **Error Handling**: Implement a custom `ScannerError` type instead of passing through `postgres::Error`.

## 3. Infrastructure & Testing
- [ ] **Unit Tests**: Add tests for `PostgresDataType` mapping.
- [ ] **Integration Tests**: Set up a test container (Docker) with a sample schema to verify scanning accuracy.
