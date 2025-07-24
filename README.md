# Moshiki

A pattern detection based (CLP like) search engine in Rust.

```bash
Indexing file: datasets_small/Android_v2_logs.txt, datasets_small/BGL_logs.txt, datasets_small/Spark_logs.txt, datasets_small/Windows_logs.txt, datasets_small/hdfs-logs-full 
Dataset              Throughput   Input (MB)   Output (MB)  Comp Ratio   Zstd (MB)    FB Dict (MB)    Dict (MB)      
Android_v2_logs.txt  100.82       559.75       33.69        17           45.36        0.00            3.19           
BGL_logs.txt         131.38       708.76       29.06        24           67.30        0.00            5.93           
Spark_logs.txt       151.52       372.16       14.26        26           26.39        0.00            0.62           
Windows_logs.txt     420.41       1154.87      1.42         811          8.18         0.00            0.44           
hdfs-logs-full       199.32       3286.67      110.93       30           198.51       0.00            10.82          
```

# TODO

- [ ] Add support for term id grouping?
   - e.g. `ABCD-DEFG` could be stored as `ABCD-DEFC` as term id X. To search for `ABCD` we could store `ABCD` and `DEFG` and reference `X` in the index.
- [X] Limit number of tokens?
   - feature flag: `token_limit` (Update: REMOVED FF). Afte N Tokens, the rest of the text is put into a special catch-all dict.
   - Added complexity probably not worth it, logs are often truncated after some length.
   - Compression is higher, but it requires a full scan of catch-all dict. 
- [ ] Improved filtering heuristic

# Indexing Improvements
- [ ] Batch documents
- [X] Partition by num tokens
    - Probably the right approach, since it avoids breaking collisions
