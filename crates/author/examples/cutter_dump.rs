//! **The diagnostic dump (IO.3)** — the part, the sketch it was cut with, and the body the cut
//! actually swept, as one compound you can open and look at.
//!
//! The certificates say a cut resolved and how tightly. What they cannot say is whether the plane
//! the sketch was picked on is the plane the author meant, or whether the file that supplied the
//! outline was read as the shape it draws. Both are questions about *intent*, and a picture is the
//! only instrument for either. So this driver emits three things at once, in their true relative
//! positions:
//!
//! - the **folded sheet** — the certified solid, exactly as `sketch_cutter` writes it;
//! - the **sketch face** — the authored profile, at the frame it was drawn in. Skew to the sheet
//!   it cut, and the pick is wrong; the wrong shape, and the outline was read wrong;
//! - the **cutter body** — the resolver's own certified footprint lifted back to the sheet, cast
//!   back down its generatrices to the sketch plane, and ruled between. Where the near cap does
//!   *not* trace the sketch face, the tracer and the author disagree.
//!
//! Everything here goes out through the raw `write_brep`, never `emit_certified_step`: a picture
//! that arrived with a certificate attached would be a lie about what was checked. That the cutter
//! body's shell happens to close is a fact about the tracer — a footprint is a simple closed curve
//! — and not a warrant for any geometry inside it.
//!
//! ```text
//! cargo run --example cutter_dump                                   # counts + residuals
//! nix develop -c cargo run --example cutter_dump --features step    # + the .step compound
//! ```
//!
//! Flags: `--out-dir <dir>` (default `generated-demos/`).

use author::dump;
use author::part::Part;
use certify_core::Verdict;
use develop::extrude::Apex;
use export::approx::{rat_to_f64, surd_to_f64};
use export::brep::Brep;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The footprint chord budget — the number the solid path itself certifies at, so the picture is
/// the geometry the part was built from rather than a finer one drawn alongside it.
const BODY_SEGMENTS: usize = 16;

/// The dyadic precision the sketch face's in-plane samples snap to. Ample for a picture: it costs
/// the outline ~10⁻⁶ and the plane exactly nothing (see `author::dump`).
const SKETCH_BITS: u32 = 20;

/// The L-slot through the Stage-1 device gore — the AUTH.2 traced-footprint fixture, whose
/// non-convexity is the whole reason the body is triangulated rather than fanned.
fn device() -> Part<Bignum> {
    let parallel = Apex::direction([qi(0), qi(0), qi(1)]).expect("a real direction");
    acceptance::sketch_panel(Some((parallel, acceptance::ell_slot())))
}

/// The distance from `p` to the segment `a b`.
fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

/// OCCT's own view of a shell — the differential oracle, never the certificate.
#[cfg(feature = "step")]
fn audit(name: &str, brep: &Brep<Bignum>) {
    match export::step::audit_brep(brep) {
        Ok(a) => println!(
            "{name:<9} OCCT      {} faces · {} edges · free {} · non-manifold {} · closed {} · \
             BRepCheck valid {}",
            a.faces, a.edges, a.free_edges, a.nonmanifold_edges, a.closed, a.brepcheck_valid,
        ),
        Err(e) => println!("{name:<9} OCCT      audit failed: {e}"),
    }
}

#[cfg(not(feature = "step"))]
fn audit(_name: &str, _brep: &Brep<Bignum>) {}

#[cfg(feature = "step")]
fn write(out_dir: &str, name: &str, brep: &Brep<Bignum>) {
    let path = format!("{out_dir}/{name}.step");
    // **Raw, deliberately.** `emit_certified_step` would certify first and report a verdict, and a
    // diagnostic must not carry one.
    let report = export::step::write_brep(&path, brep);
    println!("compound  wrote     {path}   {report}");
}

#[cfg(not(feature = "step"))]
fn write(_out_dir: &str, _name: &str, _brep: &Brep<Bignum>) {
    println!(
        "compound  skipped   no `.step` written — build under \
         `nix develop -c cargo run --example cutter_dump --features step`"
    );
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut out_dir = "generated-demos".to_string();
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--out-dir" => {
                out_dir = argv[i + 1].clone();
                i += 2;
            }
            other => panic!("unknown flag {other}"),
        }
    }
    std::fs::create_dir_all(&out_dir).expect("create --out-dir");

    println!("diagnostic dump — L-slot through the device gore (β≈42°, σ∈[−7/2,7/2] ≈ 296°)\n");
    let part = device();
    let mut compound = Brep::<Bignum>::new();

    // ── the folded sheet ────────────────────────────────────────────────────────────────────────
    let t = std::time::Instant::now();
    match part.solid() {
        Verdict::Verified(solid) => {
            println!(
                "sheet     Verified  ε {:.3e}   {} faces · free {} · non-manifold {}   [{:.1}s]",
                rat_to_f64(solid.eps()),
                solid.brep().faces().len(),
                solid.brep().free_edges(),
                solid.brep().nonmanifold_edges(),
                t.elapsed().as_secs_f64(),
            );
            compound.absorb(solid.into_brep());
        }
        Verdict::Refuted(f) => println!("sheet     Refuted({f:?})"),
        Verdict::Unresolved(e) => {
            println!("sheet     Unresolved at ε {:.3e}", rat_to_f64(&e))
        }
    }

    // ── the authored sketch, at the plane it cuts from ───────────────────────────────────────────
    let sketch = dump::sketch_faces(&part, SKETCH_BITS);
    println!("sketch    {}", sketch.summary());
    // Its wire, in ring order, kept for the differential below.
    let outline: Vec<[f64; 2]> = sketch
        .brep
        .verts()
        .iter()
        .map(|v| [surd_to_f64(&v[0]), surd_to_f64(&v[1])])
        .collect();
    compound.absorb(sketch.brep);

    // ── the body the cut actually swept ──────────────────────────────────────────────────────────
    let t = std::time::Instant::now();
    match dump::cutter_bodies(&part, BODY_SEGMENTS) {
        Verdict::Verified(d) => {
            println!(
                "body      {}   [{:.1}s]",
                d.summary(),
                t.elapsed().as_secs_f64()
            );
            // **The differential.** The near cap and the sketch face are the same closed curve
            // reached two ways — one from the authored profile edges, one from the traced footprint
            // pulled back through the chart and cast down its generatrices. Neither computation
            // knows about the other, and no certified ε would report their disagreeing.
            let mut worst = 0.0f64;
            let mut near = 0usize;
            for v in d.brep.verts() {
                if surd_to_f64(&v[2]).abs() > 1e-12 {
                    continue; // a far-cap vertex — on the sheet, not in the sketch plane
                }
                near += 1;
                let p = [surd_to_f64(&v[0]), surd_to_f64(&v[1])];
                let n = outline.len();
                worst = worst.max(
                    (0..n)
                        .map(|i| seg_dist(p, outline[i], outline[(i + 1) % n]))
                        .fold(f64::INFINITY, f64::min),
                );
            }
            if near > 0 {
                println!(
                    "body      faithful  {near} near-cap vertices, worst distance to the *authored* \
                     sketch outline {worst:.3e}   (the two routes share no code)",
                );
            }
            for b in &d.bodies {
                println!(
                    "body      op {:<2} region {}   {} footprint vertices   {}",
                    b.op,
                    b.region,
                    b.vertices,
                    if b.solid {
                        "near cap + walls + far cap (closed)"
                    } else {
                        "far cap only — a metric cutter has no sketch plane to cast back to"
                    },
                );
            }
            println!(
                "body      closure   free {} · non-manifold {}   (a footprint is a simple closed \
                 curve, so its body closes — a fact about the tracer, not a certificate)",
                d.brep.free_edges(),
                d.brep.nonmanifold_edges(),
            );
            audit("body", &d.brep);
            compound.absorb(d.brep);
        }
        Verdict::Refuted(f) => println!("body      Refuted({f:?})"),
        Verdict::Unresolved(e) => println!("body      Unresolved at ε {:.3e}", rat_to_f64(&e)),
    }

    // ── one compound ────────────────────────────────────────────────────────────────────────────
    println!(
        "\ncompound  {} faces · {} vertices · free {}   (open, by the sketch faces — a lone planar \
         face is all boundary, so the compound can never pass a closed-shell check)",
        compound.faces().len(),
        compound.verts().len(),
        compound.free_edges(),
    );
    write(&out_dir, "cutter_dump", &compound);
}
