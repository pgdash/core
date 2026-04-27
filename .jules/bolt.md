# Bolt's Journal

## Zero-Cost Abstractions inside Iterators
When doing operations inside iterators (like `.filter()`, `.find()`, or `.any()`), it is critical to look out for operations that cause heap allocations such as `.to_string()`, `.clone()`, or collecting into intermediate collections like `Vec<String>`.

A prime example is doing `.any(|c| c.columns.contains(&column_name.to_string()))`. For every element checked in the iterator, a new String is allocated on the heap!

Instead, by utilizing references, iterators, and zero-cost abstraction techniques, we can completely eliminate these heap allocations:
- Replace `c.columns.contains(&column_name.to_string())` with `c.columns.iter().any(|col| col == column_name)`. This compares string references instead of allocating new Strings.
- When creating an intermediate collection of strings to check against, collect string references (`Vec<&String>`) instead of owned strings (`Vec<String>`). Use `.flat_map(|c| c.columns.iter())` rather than `.flat_map(|c| c.columns.clone())`.

These small changes can yield massive performance gains, especially inside loops or hot paths. In our test, replacing these allocations inside `is_foreign_key` improved performance from ~29.1s to ~7.3s for 10 million iterations.

## Zero-Copy Deserialization with tokio-postgres
The `tokio-postgres` library's `Row::get` and `Row::try_get` methods allow fetching `TEXT` or `VARCHAR` fields as borrowed string slices (`&str`). This is a zero-copy operation that avoids heap-allocating `String`s. By avoiding allocations for intermediate variables (like type definitions or flags in information_schema queries) that only need to be parsed or evaluated (e.g., checking `== "YES"` or mapping to an enum via `&str`), we can significantly speed up the schema scanning process and minimize heap allocations.

## HashMap Entry Allocation Avoidance
When inserting into or querying a `HashMap` where the key is a `String` constructed dynamically (e.g., extracted from a database row), the common anti-pattern `.entry(schema_name.clone()).or_insert_with(...)` forces a heap allocation for `schema_name` on *every single check*, even if the key is already in the map (a cache hit).

To achieve a zero-cost lookup for cache hits, use the `Entry::or_insert_with_key` API:
```rust
let schema = schemas_map
    .entry(schema_name) // consumes the original string, avoiding cloning
    .or_insert_with_key(|k| Schema {
        name: k.clone(), // clone is deferred until an actual insert happens
        // ...
    });
```
This defers the allocation entirely to the cache-miss path. If the key already exists, `entry()` consumes the owned string, but no new `String` is allocated.
Note: Since the original variable (e.g., `schema_name`) is moved into `entry()`, subsequent code must borrow it back from the returned entry (e.g., `&schema.name`) to satisfy the borrow checker.
