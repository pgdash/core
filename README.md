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
* **Test**: `cargo test`

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
