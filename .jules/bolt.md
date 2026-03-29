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

* **HashMap `.entry` API with Owned Keys:** When iterating over owned `String` keys and updating a `HashMap`, passing `key.clone()` to `.entry()` incurs a heap allocation on every loop iteration, even on cache hits. Instead, pass the consumed key by value directly using `.entry(key)`, and defer any necessary cloning to the `.or_insert_with_key(|k| ...)` closure. If the key already exists, the Map drops the consumed `String` with zero allocations. For this to work in `async` blocks, ensure the Map update happens *after* any `.await` points to avoid compiler lifetime/move errors.
