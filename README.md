# Moshiki

A pattern detection based (CLP like) search engine in Rust.

# TODO

- [ ] Ass support for term id grouping
   - e.g. `ABCD-DEFG` could be stored as `ABCD-DEFC` as term id X. To search for `ABCD` we could store `ABCD` and `DEFG` and reference `X` in the index.
- [X] Limit number of tokens?
