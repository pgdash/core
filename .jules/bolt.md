# Bolt's Journal

## Critical Learnings

* The standard library's `HashMap::entry` API forces an unconditional heap allocation when taking an owned `String` as a key. For instances where keys rarely miss, doing a simple `contains_key` check and conditional insert avoids string allocations significantly, turning $O(N)$ allocations into $O(S)$ allocations where $S$ is the number of unique schemas. Applying `#[allow(clippy::map_entry)]` right above the `if` statement correctly suppresses clippy's warning for this specific scenario.
