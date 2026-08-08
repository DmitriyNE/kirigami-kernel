//! Emit `cone.html` — the certified device cone strip, rendered in 3D with Three.js.
//!
//! Samples [`fixtures::devices::certified_cone`]'s mid-surface (`w = 0`) through the quarantined
//! exact→`f64` bridge into a triangle mesh + ruling generators + its flat isometric development,
//! then writes a self-contained viewer page (lit surface, orbit controls, a flat↔rolled morph
//! slider, a `(μ,w)` panel with the certified sub-box marked).
//!
//! Run it with the `diagnostics` feature (the only place floats are allowed):
//!
//! ```text
//! cargo run --example cone --features diagnostics
//! ```
//!
//! By default Three.js loads from a pinned CDN (needs the network). To render fully offline,
//! point `--vendor` at a directory holding `three.module.js` and `OrbitControls.js` (the module
//! sources from the `three` npm package) — they are inlined as `data:` URLs:
//!
//! ```text
//! cargo run --example cone --features diagnostics -- --vendor path/to/three
//! ```
//!
//! Other flags: `--out <path>` (default `cone.html`), `--nsig <n>` / `--nmu <n>` (grid density).

use export::mesh3d::{DEFAULT_NMU, DEFAULT_NSIG, ThreeSrc, cone_html, sample_cone_strip};
use fixtures::devices::certified_cone;
use std::path::Path;

/// Read the first of `rels` that exists under `dir`, or exit with a clear message.
fn read_vendor(dir: &str, rels: &[&str]) -> String {
    for rel in rels {
        if let Ok(s) = std::fs::read_to_string(Path::new(dir).join(rel)) {
            return s;
        }
    }
    eprintln!("--vendor: none of {rels:?} found under `{dir}`");
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut vendor: Option<String> = None;
    let mut out = "cone.html".to_string();
    let mut nsig = DEFAULT_NSIG;
    let mut nmu = DEFAULT_NMU;

    let mut i = 0;
    while i < args.len() {
        let next = |i: &mut usize| {
            *i += 1;
            args.get(*i).cloned().unwrap_or_else(|| {
                eprintln!("missing value for `{}`", args[*i - 1]);
                std::process::exit(1);
            })
        };
        match args[i].as_str() {
            "--vendor" => vendor = Some(next(&mut i)),
            "--out" => out = next(&mut i),
            "--nsig" => nsig = next(&mut i).parse().expect("--nsig expects an integer"),
            "--nmu" => nmu = next(&mut i).parse().expect("--nmu expects an integer"),
            other => {
                eprintln!("unknown argument `{other}`");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let cert = certified_cone();
    let mesh = sample_cone_strip(&cert, nsig, nmu);

    let three = match &vendor {
        Some(dir) => ThreeSrc::Inline {
            three_module: read_vendor(dir, &["three.module.js", "build/three.module.js"]),
            orbit_controls: read_vendor(
                dir,
                &[
                    "OrbitControls.js",
                    "controls/OrbitControls.js",
                    "examples/jsm/controls/OrbitControls.js",
                ],
            ),
        },
        None => ThreeSrc::Cdn,
    };

    let html = cone_html("kirigami-kernel — certified cone strip", &mesh, &three);
    std::fs::write(&out, &html).expect("write cone.html");
    println!(
        "wrote {out} — {} verts, {} tris, {} rulings, {} bytes ({})",
        mesh.positions.len(),
        mesh.tris.len(),
        mesh.ruling_rows.len(),
        html.len(),
        if vendor.is_some() {
            "Three.js inlined"
        } else {
            "Three.js via CDN"
        },
    );
}
