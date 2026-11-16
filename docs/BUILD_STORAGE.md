# Build storage policy

Dev and test builds use `debug = "line-tables-only"` and `incremental = false`.
Stack locations remain available; full variable/type inspection needs an explicit
temporary debug override. Assertions and overflow checks retain Cargo defaults.
Incremental recompilation is disabled to avoid accumulating dependency graphs and
object caches. Rebuilds after source changes can take longer.
Cargo also changes the default codegen-unit count from256 to16 when incremental
compilation is disabled. Generated dev/test code and timings can therefore differ;
unchanged optimization levels are not a promise of identical performance.

The application test profile uses opt-level0 consistently. Do not add the previous
CLI profile override: it is now in Cargo.toml. Interactive dev playback remains
opt-level1 and codec dependencies remain opt-level2; release settings are unchanged.
Test-profile performance probes do not qualify release/native performance.

Use one target directory for this active checkout and serialize Cargo commands.
Run focused tests/checks during a slice, then the appropriate final regression.
Avoid rebuilding every feature/profile combination after every edit. Release
builds are deliberate final checks, not the inner development loop.

Before broad builds inspect free space and target size. Keep at least10GiB free;
pause broad builds below that margin and reclaim only identified build artifacts
after all Cargo processes stop. Never delete another worktree's files or source.
No automatic cache deletion or machine-specific target path is configured.
These settings reduce artifacts; they impose no hard disk quota on Cargo.

Deleting target is safe for source and commits but forces dependency compilation.
If old full-symbol/incremental variants still exist, changed profiles do not remove
them automatically. Here the user already removed the previous target directory.

Reference: https://doc.rust-lang.org/cargo/reference/profiles.html
