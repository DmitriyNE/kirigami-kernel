//! Emit `cone_flat.svg` — the **certified flat pattern** of the device cone, unrolled by the
//! Milestone-E development tier (`develop::unroll`, product direction ①).
//!
//! Takes the device cone's free-boundary μ-band (`fixtures::devices::cone`, half-angle ≈ 42°),
//! develops its boundary loop to a flat polyline via
//! [`unroll_freeboundary`](develop::unroll::unroll_freeboundary) — each rail edge certified
//! within `ε` of the true continuous developed rail (DEV.2c/2d) — and draws the resulting flat
//! "gore" as an SVG: the developed band (filled), its two rails, the ruling fan converging to the
//! apex, and a caption reporting the *certified* backward error `ε` and the DRC verdict.
//!
//! The certificate is rational and float-free; floats appear only here, at the display boundary,
//! through the quarantined [`approx`](export::approx) bridge — so this is gated on `diagnostics`
//! like the other renderers:
//!
//! ```text
//! cargo run --example cone_flat --features diagnostics
//! ```
//!
//! Flags: `--out <path>` (default `cone_flat.svg`), `--segments <n>` (rail discretization).

use certify_core::Verdict;
use develop::cone::{ConeDevelopment, DevConfig};
use develop::unroll::{FlatOutline, unroll_freeboundary};
use export::approx::rat_to_f64;
use fixtures::devices::cone;
use lattice::{Bignum, Interval, Poly, Rat, RatFunc};

/// SVG canvas size and inner margin, in pixels.
const W: f64 = 640.0;
const H: f64 = 600.0;
const MARGIN: f64 = 48.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = "cone_flat.svg".to_string();
    let mut segments = 48usize;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out = args.get(i).cloned().expect("--out expects a path");
            }
            "--segments" => {
                i += 1;
                segments = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .expect("--segments expects an integer");
            }
            other => {
                eprintln!("unknown argument `{other}`");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let chart = cone();
    let dev = ConeDevelopment::new(&chart).expect("the device cone is a canonical arctan cone");
    // The retained band: outer rail μ⁻ ≡ −1, inner rail μ⁺ ≡ −1/2, over the certified gore σ∈[0,1].
    let mu_lo = RatFunc::<Bignum>::from_poly(Poly::constant(Rat::from_i128(-1)));
    let mu_hi = RatFunc::from_poly(Poly::constant(Rat::new(-1, 2)));
    let span = Interval {
        lo: Rat::from_i128(0),
        hi: Rat::from_i128(1),
    };
    // A generous fab clearance (1 unit on a ~1.5-unit part) so the demo certifies; the caption
    // reports the *achieved* ε, which is far tighter.
    let clearance = Rat::from_i128(1);

    let outline = match unroll_freeboundary(
        &dev,
        &span,
        &mu_lo,
        &mu_hi,
        segments,
        &DevConfig::tight(),
        &clearance,
    ) {
        Verdict::Verified(o) => o,
        Verdict::Unresolved(e) => {
            eprintln!(
                "unroll Unresolved: ε ≈ {:.3e} ≥ clearance/2 — raise --segments or the clearance",
                rat_to_f64(&e)
            );
            std::process::exit(1);
        }
        Verdict::Refuted(f) => {
            eprintln!("unroll Refuted: {f:?}");
            std::process::exit(1);
        }
    };

    let eps = rat_to_f64(&outline.eps);
    let svg = render_svg(&outline, eps, "Verified", segments);
    std::fs::write(&out, &svg).expect("write cone_flat.svg");
    println!(
        "wrote {out} — {} flat vertices, certified backward error ε ≈ {eps:.3e} (Verified vs clearance/2), {} bytes",
        outline.vertices.len(),
        svg.len(),
    );
}

/// Flatten a rational [`FlatOutline`] into an SVG string: the developed band (filled polygon),
/// its two rails, the ruling fan to the apex, and a caption with the certified numbers.
fn render_svg(outline: &FlatOutline<Bignum>, eps: f64, verdict: &str, segments: usize) -> String {
    // Vertex centers as display f64 (order: μ⁻ rail σ_lo→σ_hi, then μ⁺ rail σ_hi→σ_lo).
    let pts: Vec<[f64; 2]> = outline
        .vertices
        .iter()
        .map(|b| {
            let (x, y) = b.center();
            [rat_to_f64(&x), rat_to_f64(&y)]
        })
        .collect();

    // Fit the flat pattern *and* the apex (origin) into the canvas; flip y (SVG y points down).
    let (mut xmin, mut ymin, mut xmax, mut ymax) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for p in &pts {
        xmin = xmin.min(p[0]);
        ymin = ymin.min(p[1]);
        xmax = xmax.max(p[0]);
        ymax = ymax.max(p[1]);
    }
    let (dx, dy) = ((xmax - xmin).max(1e-9), (ymax - ymin).max(1e-9));
    let scale = ((W - 2.0 * MARGIN) / dx).min((H - 2.0 * MARGIN) / dy);
    let tx = |x: f64| MARGIN + (x - xmin) * scale;
    let ty = |y: f64| H - MARGIN - (y - ymin) * scale;
    let (apex_x, apex_y) = (tx(0.0), ty(0.0));

    // The band boundary as one closed polygon (outer rail forward + inner rail back).
    let poly: String = pts
        .iter()
        .map(|p| format!("{:.2},{:.2}", tx(p[0]), ty(p[1])))
        .collect::<Vec<_>>()
        .join(" ");

    // The two rails, split at the μ⁻/μ⁺ boundary (each rail has `segments + 1` vertices).
    let n = segments + 1;
    let rail = |slice: &[[f64; 2]]| -> String {
        slice
            .iter()
            .map(|p| format!("{:.2},{:.2}", tx(p[0]), ty(p[1])))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let outer = rail(&pts[..n]); // μ⁻
    let inner = rail(&pts[n..]); // μ⁺ (reversed order)

    // A decimated ruling fan: at station k the ruling joins the inner-rail point (σ_k) to the
    // outer-rail point (σ_k); every ruling extended passes through the apex.
    let mut rulings = String::new();
    let step = (segments / 12).max(1);
    for k in (0..n).step_by(step) {
        let o = pts[k]; // outer rail at σ_k
        let inr = pts[(n) + (segments - k)]; // inner rail at σ_k (reversed second half)
        rulings.push_str(&format!(
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#94a3b8\" stroke-width=\"0.7\"/>",
            tx(inr[0]), ty(inr[1]), tx(o[0]), ty(o[1]),
        ));
    }

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{W:.0}\" height=\"{H:.0}\" viewBox=\"0 0 {W:.0} {H:.0}\" font-family=\"system-ui,sans-serif\">\n\
         <rect width=\"{W:.0}\" height=\"{H:.0}\" fill=\"#ffffff\"/>\n\
         <polygon points=\"{poly}\" fill=\"#dbeafe\" stroke=\"none\"/>\n\
         {rulings}\n\
         <polyline points=\"{outer}\" fill=\"none\" stroke=\"#2563eb\" stroke-width=\"2.2\"/>\n\
         <polyline points=\"{inner}\" fill=\"none\" stroke=\"#ea580c\" stroke-width=\"2.2\"/>\n\
         <circle cx=\"{apex_x:.1}\" cy=\"{apex_y:.1}\" r=\"3.5\" fill=\"#111827\"/>\n\
         <text x=\"16\" y=\"28\" font-size=\"15\" font-weight=\"600\" fill=\"#111827\">Certified flat pattern — device cone (β ≈ 42°), gore σ∈[0,1]</text>\n\
         <text x=\"16\" y=\"{cap_y:.0}\" font-size=\"12\" fill=\"#374151\">outer rail μ⁻ (blue) · inner rail μ⁺ (orange) · rulings to apex (gray)</text>\n\
         <text x=\"16\" y=\"{cap_y2:.0}\" font-size=\"12\" fill=\"#374151\">certified backward error ε ≈ {eps:.2e} — {verdict} (ε &lt; clearance/2) · float-free certificate, {segments} rail segments</text>\n\
         </svg>\n",
        cap_y = H - 30.0,
        cap_y2 = H - 14.0,
    )
}
