# Bolt's Journal

## Zero-Cost Abstractions inside Iterators
When doing operations inside iterators (like `.filter()`, `.find()`, or `.any()`), it is critical to look out for operations that cause heap allocations such as `.to_string()`, `.clone()`, or collecting into intermediate collections like `Vec<String>`.

A prime example is doing `.any(|c| c.columns.contains(&column_name.to_string()))`. For every element checked in the iterator, a new String is allocated on the heap!

Instead, by utilizing references, iterators, and zero-cost abstraction techniques, we can completely eliminate these heap allocations:
- Replace `c.columns.contains(&column_name.to_string())` with `c.columns.iter().any(|col| col == column_name)`. This compares string references instead of allocating new Strings.
- When creating an intermediate collection of strings to check against, collect string references (`Vec<&String>`) instead of owned strings (`Vec<String>`). Use `.flat_map(|c| c.columns.iter())` rather than `.flat_map(|c| c.columns.clone())`.

These small changes can yield massive performance gains, especially inside loops or hot paths. In our test, replacing these allocations inside `is_foreign_key` improved performance from ~29.1s to ~7.3s for 10 million iterations.

### Zero-Copy Text Fetching from Postgres Rows

When pulling temporary metadata fields (like schema names, definitions, or type properties) from `postgres::Row` (or `tokio_postgres::Row`), we can map directly to `&str` instead of `String`. Utilizing `let data_type_str: &str = row.get("data_type");` prevents allocating intermediate memory on the heap before we immediately consume or map that string data. This is especially impactful in metadata scanning loops where `Row` data is quickly discarded.
