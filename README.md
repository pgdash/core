# pgdash-core

A high-performance Postgres management and introspection API server. `pgdash-core` is the API-driven backend engine designed to manage and introspect Postgres databases.

## Project Overview

`pgdash-core` acts as the middleware between the client and the database. It handles the core responsibilities of:
* Scanning a database and writing metadata/info about each database to its own database.
* Processing user queries from the frontend.
* Editing data and schema from the frontend.
* Managing database extensions and other administrative tasks on the user's behalf.

## Setup

Getting started is incredibly simple: All you have to do is provide your Postgres connection string!

### Prerequisites
* Rust (stable)
* `libpq-dev` (Postgres development headers)

### Building and Running
* **Build**: `cargo build`
* **Run**: `cargo run`
* **Test**: `cargo test -- --test-threads=1`

## Configuration

`pgdash-core` can be configured using either a YAML config file, environment variables, or both. Environment variables take precedence over config file values.

### Config File

By default, the application looks for `config.yaml` in the current directory. You can specify a custom path using the `PGDASH_CONFIG` environment variable.

Example `config.yaml`:

```yaml
server:
  port: 5000
  log_level: info

database:
  url: postgres://postgres:postgres@localhost/postgres

admin:
  username: admin
  password: admin
```

### Configuration Options

| Option | Description | Default |
|--------|-------------|---------|
| `server.port` | HTTP server port | `5000` |
| `server.log_level` | Log level (trace, debug, info, warn, error) | `info` |
| `database.url` | PostgreSQL connection URL | `postgres://postgres:postgres@localhost/postgres` |
| `admin.username` | Default admin username | `admin` |
| `admin.password` | Default admin password | `admin` |

### Environment Variables

All configuration options can be overridden via environment variables with the `PGDASH_` prefix:

| Variable | Config Option |
|----------|--------------|
| `PGDASH_SERVER_PORT` | `server.port` |
| `PGDASH_SERVER_LOG_LEVEL` | `server.log_level` |
| `PGDASH_DATABASE_URL` | `database.url` |
| `PGDASH_ADMIN_USERNAME` | `admin.username` |
| `PGDASH_ADMIN_PASSWORD` | `admin.password` |
| `PGDASH_CONFIG` | Path to config file (default: `config.yaml`) |

### Examples

Using only environment variables:
```bash
PGDASH_SERVER_PORT=3000 PGDASH_DATABASE_URL=postgres://user:pass@host/db cargo run
```

Using a custom config file:
```bash
PGDASH_CONFIG=/etc/pgdash/config.yaml cargo run
```

## Features and Capabilities

The platform offers a full-feature UI to manage your database, including tables, triggers, data, stored procedures, and nearly all Postgres features.

### Core Features
* **Table Explorer**: Introspect and manage database tables and schema.
* **Data Explorer**: Powerful CRUD operations and data management.
* **Query Runner and Management**: Interface to run arbitrary SQL queries safely.
* **Schema Visualizer**: Automatically generated entity-relationship (tables) diagrams.

### Observability & Metrics
* **Auto Metric Collection**: Automatically collects metrics to display on a comprehensive dashboard.
* **Customizable Alerting**: Set up custom alerts based on collected metrics to monitor database health.

### AI-Powered Assistance (Planned)
AI Agents are integrated deeply into the platform to assist in general DB management tasks, analysis, and query writing:
* **Context-Aware Prompting**: Select a row, cell, or database object, fill out a prompt, and the AI understands your intention to generate the corresponding query or procedure.
* **Procedure Simulation**: When generating a procedure/trigger, a dedicated view pops up demonstrating an example trigger in action (e.g., against an inserted row) along with the after-effects (e.g., the affected table with a diff view). You can further prompt the agent to refine or accept the suggestion.
* **Issue Detection**: AI agents can automatically detect issues from the collected metrics.

## Planned Features (Beyond Release)

* **Predictive Issue Detection**: AI agents will predict potential future issues based on historical metric collection.
* **Automated Performance Analysis**: The AI will proactively analyze query metrics and performance to provide actionable suggestions, such as indexing or partitioning.
* **Dry Run Procedure Simulation**: The procedure simulation will utilize a safe dry-run approach: it opens a transaction (`BEGIN`), injects the new function and trigger, performs the triggering action, captures the side effects, and then rolls back (`ROLLBACK`).
