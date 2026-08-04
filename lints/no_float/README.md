# `no_float` — dylint lint

A [dylint](https://github.com/trailofbits/dylint) lint for **invariant 1** (no floats in
certified paths), covering the case a text scan cannot: **float literals**.

`cargo xtask lint`'s `no-float` check scans for `f32`/`f64` *tokens*. But a float can enter a
certified path as a literal — `let x = 1.5;` — with no such token, and text can't even tell the
literal `1.0` from tuple field access `x.1.0`. This `rustc_private` `LateLintPass` resolves the
HIR and flags float literals in `lattice` / `certify_core` / `arrange2d` (`testgen.rs`
excepted).

## Run

```sh
cargo dylint --all -- -p lattice -p certify-core -p arrange2d
```

A standalone, workspace-excluded crate pinned to a nightly (`rust-toolchain`) with `rustc-dev`
+ `llvm-tools`; registered in the root `Cargo.toml` under `[workspace.metadata.dylint]`. The UI
test (`ui/lattice.rs` + `ui/lattice.stderr`) proves the lint fires.
