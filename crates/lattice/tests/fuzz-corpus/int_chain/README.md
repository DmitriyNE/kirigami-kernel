# `int_chain` fuzz regression corpus

Checked-in, minimized crash inputs for the `int_chain` differential fuzz target
(`fuzz/fuzz_targets/int_chain.rs`). Every `*.bin` here is replayed on **every PR** by the
stable, libFuzzer-free `tests/fuzz_replay.rs` gate — so a divergence the nightly fuzzer finds
stays fixed once pinned here.

**Adding a regression** (when the nightly cron reports a crash):

```sh
# minimize the crashing input the cron uploaded as an artifact
cargo +nightly fuzz tmin int_chain path/to/crash-<hash>
# drop the minimized input here
cp fuzz/artifacts/int_chain/minimized-from-crash-<hash> \
   crates/lattice/tests/fuzz-corpus/int_chain/<short-description>.bin
```

The seed corpus (threshold-straddling programs) is **not** stored here — it is regenerated
deterministically by `fuzz/src/bin/gen_corpus.rs` from `lattice::ratfuzz::corpus_seeds()`, and
also replayed per-PR by the `replay_seed_corpus` unit test. This directory is only for
*discovered* crashes.
