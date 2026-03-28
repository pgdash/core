1. Modify `scan_functions` to fix the N+1 query issue for fetching routine parameters.
Currently, the method fetches all routines in one query, then executes a separate `param_query` for each routine to get its parameters, causing significant database load when scanning schemas with many functions.
I will combine these into a single query by performing a `LEFT JOIN` or aggregating parameters in the initial `routine_query`.

Alternatively, we can fetch all parameters in a second query and group them in memory, or aggregate them into an array directly in SQL using `array_agg` which is faster and easier to map to `Vec<String>`.

Example aggregate query:
```sql
SELECT
    r.routine_schema,
    r.routine_name,
    r.routine_type,
    r.data_type AS return_type,
    r.routine_definition,
    r.external_language,
    (SELECT p.oid FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace WHERE n.nspname = r.routine_schema AND p.proname = r.routine_name LIMIT 1) as oid,
    ARRAY(
        SELECT p.data_type
        FROM information_schema.parameters p
        WHERE p.specific_schema = r.specific_schema
          AND p.specific_name = r.specific_name
        ORDER BY p.ordinal_position
    ) as argument_types
FROM information_schema.routines r
WHERE r.routine_schema NOT IN ('information_schema', 'pg_catalog')
```

Then we can fetch `argument_types` using `row.get_vec_string("argument_types")`.
