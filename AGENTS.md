<INSTRUCTIONS>
## Invariants over Options
- When a value is guaranteed by an internal invariant (e.g., an ID created from a vector index), do not wrap access in `Option` checks.
- Prefer direct indexing or `expect` with a clear message if the invariant could be violated by future changes.
## Invariants over Errors
- If an error case is impossible given validated inputs and internal invariants, use `panic!()` (or `expect`) instead of propagating or handling the error.
- Otherwise, return/propagate errors and check invariants early at the top of the function.
## Naming clarity
- Use descriptive identifiers like `schema_id_1` instead of terse names like `id1`.
## Validation
- Run tests after changes that affect behavior or public APIs.
## Performance
- Avoid unnecessary allocations in hot paths (for example, avoid `to_string()` before hashmap lookups when borrowed lookup is possible).
</INSTRUCTIONS>
