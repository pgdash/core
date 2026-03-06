# pgdash-core

A high-performance Postgres management and introspection API server.

## Project Overview

`pgdash-core` is an API-driven engine designed to manage and introspect Postgres databases. It serves as the backend for a separate UI, providing a comprehensive set of capabilities including:

- **Schema Discovery & Introspection**: Deep scanning of database metadata (tables, columns, constraints, indexes, views, enums, functions, sequences, and triggers).
- **Schema Management (DDL)**: APIs to edit and manage database structures (tables, columns, types, etc.).
- **Data Management (DML)**: Handling data queries, CRUD operations, and managing table content.
- **Query Execution**: Providing an interface to run arbitrary SQL queries safely.
- **Observability**: Monitoring and observing Postgres metrics and performance data.
- **Extension Management**: Managing Postgres extensions.

## Architecture

- **`src/schema`**: Defines the structured, serializable data model for database objects.
- **`src/scanner`**: Implements the `PostgresScanner` for deep, bulk-fetching metadata discovery.

## Building and Running

### Prerequisites
- Rust (stable)
- `libpq-dev` (Postgres development headers)

### Key Commands
- **Build**: `cargo build`
- **Run**: `cargo run`
- **Test**: `cargo test`
- **Format**: `cargo fmt` (Enforced via a git pre-commit hook)
- **Check**: `cargo check`

## Development Conventions

- **Strict Commenting Rule**: Never add "obvious" comments. Comments are strictly reserved for `TODO`, `WARNING`, `BUG`, or complex logic/field expectations that are not self-expressed by the code.
- **Type-Driven Design**: Use strong enums and structured types (e.g., `PostgresDataType`, `ReferentialAction`) to model Postgres primitives accurately.
- **Efficiency**: Favor bulk-fetching and single-pass queries over per-object lookups to minimize database round-trips.
- **Robust Error Handling**: Favor `match` or `if let` over `unwrap`/`expect` for all production paths.

## Infrastructure
- **Git Hook**: A `pre-commit` hook ensures all staged `.rs` files are formatted with `rustfmt`.
- **CI/CD**: GitHub Actions handle build/test validation (`ci.yml`) and automated binary releases (`release.yml`) on version tags.
