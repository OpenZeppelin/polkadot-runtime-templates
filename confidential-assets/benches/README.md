# Benchmarking Cryptographic Backends for Confidential Arithmetic

FHE vs `u128` arithmetic:
```
(amar-tfhe)⚡ % cargo bench
   Compiling benches v0.0.1
    Finished `bench` profile [optimized] target(s) in 3.73s
     Running benches/fhe_vs_u128.rs
Benchmarking fhe_add (one op): Warming up for 3.0000 s
Warning: Unable to complete 100 samples in 5.0s. You may wish to increase target time to 99.0s, or reduce sample count to 10.
fhe_add (one op)        time:   [962.93 ms 969.41 ms 977.06 ms]
Found 7 outliers among 100 measurements (7.00%)
  2 (2.00%) high mild
  5 (5.00%) high severe

Benchmarking fhe_sub (one op): Warming up for 3.0000 s
Warning: Unable to complete 100 samples in 5.0s. You may wish to increase target time to 98.4s, or reduce sample count to 10.
fhe_sub (one op)        time:   [994.60 ms 1.0120 s 1.0321 s]
Found 5 outliers among 100 measurements (5.00%)
  2 (2.00%) high mild
  3 (3.00%) high severe

Benchmarking fhe_select_true (one op): Warming up for 3.0000 s
Warning: Unable to complete 100 samples in 5.0s. You may wish to increase target time to 88.2s, or reduce sample count to 10.
fhe_select_true (one op)
                        time:   [969.47 ms 993.36 ms 1.0186 s]
Found 1 outliers among 100 measurements (1.00%)
  1 (1.00%) high mild

u128_add (1000 iters)   time:   [571.22 ns 578.43 ns 587.08 ns]

u128_sub (1000 iters)   time:   [492.47 ns 500.89 ns 509.73 ns]
Found 6 outliers among 100 measurements (6.00%)
  4 (4.00%) high mild
  2 (2.00%) high severe
```