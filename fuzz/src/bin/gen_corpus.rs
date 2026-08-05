//! Regenerate the `int_chain` seed corpus from the authoritative encoder in
//! `lattice::ratfuzz::corpus_seeds()` (so the byte format can never drift from the decoder).
//! Run from `fuzz/`:  `cargo run --bin gen_corpus`
use std::fs;
use std::io::Write;

fn main() {
    let dir = "corpus/int_chain";
    fs::create_dir_all(dir).expect("create corpus dir");
    for (i, seed) in lattice::ratfuzz::corpus_seeds().into_iter().enumerate() {
        let path = format!("{dir}/seed_{i:03}.bin");
        fs::File::create(&path)
            .and_then(|mut f| f.write_all(&seed))
            .expect("write corpus seed");
        println!("wrote {path} ({} bytes)", seed.len());
    }
}
