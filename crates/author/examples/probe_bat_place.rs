//! Probe (#311): where the `bat-cutout.dxf` window lands on the two-ramp device, and which
//! placements the engine can carry.
//!
//! Reads the σ ↔ azimuth convention off the kernel rather than off a doc comment, then walks the
//! device's own chart asking the cutter's `Cast` which `(σ, µ̂)` are inside the imported rectangle —
//! so the footprint's σ-window, its pass count and whether it reaches a band end are all *measured*
//! before a certificate is asked for, and every op's derived [`OpRole`](author::part::OpRole) is
//! printed after, so a cut that silently resolved `Inactive` cannot read as a success.
//!
//! Measured 2026-08-20 at `--segments 8`:
//!
//! ```text
//! as drawn, +y (4.6 mm on the seam)   reaches a band end   → Refuted(CutUnresolved{op:2})  619 s
//! narrowed to ±0.6, +y (lap wedge)    end-clear, 2 passes  → Verified ε 3.490, 2 holes     536 s
//! as drawn, −y (plain body)           end-clear, 1 pass    → Verified ε 3.490, 1 hole      519 s
//! ```
//!
//! The narrow and wide `+y` cuts differ in nothing but width, so what the engine cannot carry is
//! precisely a cut that **opens onto the part's own boundary** — the [`#291`] frontier. One cutter
//! piercing both lapped sheets derives two certified holes, which is the harder-looking case and
//! works.
//!
//! `BAT_ONLY_NEW=1` skips the walk and the two refusing variants.

use develop::extrude::Cast;
use export::approx::{rat_to_f64, surd_to_f64};
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

fn main() {
    let lap = acceptance::lapped::lapped_cone(&acceptance::two_ramp_spec()).expect("recipe");
    println!("-- the two-ramp device's regions --");
    for (i, (band, h)) in lap.regions.iter().enumerate() {
        println!(
            "  {i}: σ ∈ [{:+.4}, {:+.4}]  h {:+.4}",
            rat_to_f64(&band.lo),
            rat_to_f64(&band.hi),
            rat_to_f64(h)
        );
    }
    let chart = &lap.chart;

    // The cutter: the imported rectangle in the tilted sketch plane, swept along that plane's own
    // normal (a straight prism — "through all", no draft).
    let cast = Cast::new(acceptance::bat_plane(), acceptance::bat_sweep())
        .expect("the sweep direction is off the sketch plane");
    let (mut xlo, mut xhi, mut ylo, mut yhi) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for e in &acceptance::bat_cutout_profile() {
        if let geom::content::Edge::Seg(s) = e {
            for p in [&s.start, &s.end] {
                let (x, y) = (surd_to_f64(&p.x), surd_to_f64(&p.y));
                xlo = xlo.min(x);
                xhi = xhi.max(x);
                ylo = ylo.min(y);
                yhi = yhi.max(y);
            }
        }
    }
    println!(
        "\n-- the window: x ∈ [{xlo:+.4}, {xhi:+.4}], y ∈ [{ylo:+.4}, {yhi:+.4}]  ({:.3} × {:.3}) --",
        xhi - xlo,
        yhi - ylo
    );

    let inside = |mu: &Q, sigma: &Q| -> Option<(f64, f64)> {
        let p = chart.surface(mu, &qi(0)).eval(sigma)?;
        let (a, b) = cast.coords(&p)?;
        let (a, b) = (rat_to_f64(&a), rat_to_f64(&b));
        (a >= xlo && a <= xhi && b >= ylo && b <= yhi).then_some((a, b))
    };

    // ρ is linear in µ̂ along a ruling, so one evaluation per σ gives the material's µ̂ window (the
    // annulus ρ ∈ [4, 43/4]) on the kept nappe (µ̂ < 0). `BAT_ONLY_NEW` skips the walk and the two
    // already-measured variants, so a rerun costs only the cases being added.
    let only_new = std::env::var("BAT_ONLY_NEW").is_ok();
    if !only_new {
        println!("\n-- the footprint, walked over σ (µ̂ < 0 is the kept nappe) --");
        let steps = 900i128;
        let mut runs: Vec<(f64, Vec<(f64, f64)>)> = Vec::new();
        for i in 0..=steps {
            let sigma = Q::new(-9, 8).add(&Q::new(9 * i, 4 * steps));
            let p1 = match chart.surface(&qi(-1), &qi(0)).eval(&sigma) {
                Some(p) => p,
                None => continue,
            };
            let s = rat_to_f64(&p1[0]).hypot(rat_to_f64(&p1[1]));
            let (mu_lo, mu_hi) = (-10.75 / s, -4.0 / s); // ρ = |µ̂|·s
            let n = 260;
            let mut hits: Vec<(f64, f64)> = Vec::new();
            let mut open: Option<f64> = None;
            for k in 0..=n {
                let mu = mu_lo + (mu_hi - mu_lo) * (k as f64) / (n as f64);
                let is = inside(&Q::new((mu * 100_000.0) as i128, 100_000), &sigma).is_some();
                match (is, open) {
                    (true, None) => open = Some(mu),
                    (false, Some(a)) => {
                        hits.push((a, mu));
                        open = None;
                    }
                    _ => {}
                }
            }
            if let Some(a) = open {
                hits.push((a, mu_hi));
            }
            if !hits.is_empty() {
                runs.push((rat_to_f64(&sigma), hits));
            }
        }
        if runs.is_empty() {
            println!("  the cutter meets no material at all");
            return;
        }
        let mut last_n = 0usize;
        for (sigma, hits) in &runs {
            if hits.len() != last_n {
                let spans: Vec<String> = hits
                    .iter()
                    .map(|(a, b)| format!("[{a:+.3}, {b:+.3}]"))
                    .collect();
                println!(
                    "  σ {sigma:+.4}  → {} µ̂-interval(s)  {}",
                    hits.len(),
                    spans.join(" ∪ ")
                );
                last_n = hits.len();
            }
        }
        println!(
            "  σ-extent of the whole footprint: [{:+.4}, {:+.4}]",
            runs[0].0,
            runs[runs.len() - 1].0
        );
        for w in runs.windows(2) {
            if w[1].0 - w[0].0 > 4.0 * 2.25 / steps as f64 {
                println!(
                    "  …but empty over σ ∈ ({:+.4}, {:+.4}) — separate passes",
                    w[0].0, w[1].0
                );
            }
        }
        println!(
            "  most µ̂-intervals at one σ: {}",
            runs.iter().map(|(_, h)| h.len()).max().unwrap_or(0)
        );
        for (band, h) in &lap.regions {
            let (lo, hi) = (rat_to_f64(&band.lo), rat_to_f64(&band.hi));
            if runs.iter().any(|(s, _)| *s > lo && *s < hi) {
                println!(
                    "  touches region σ ∈ [{lo:+.4}, {hi:+.4}] (h {:+.4})",
                    rat_to_f64(h)
                );
            }
        }

        // — and now the kernel's own verdict on the same cut, twice —
        //
        // `as drawn` is the device as it stands. `late ramp` moves the ccw ramp's end from σ = 7/8 down
        // to 3/4, ahead of the footprint's own σ = 0.7675 start, so the whole +σ pass sits in ONE
        // support region: it separates the two obstacles the walk above measured — the region crossing
        // and the footprint's overhang past the sheet's radial ends — instead of reading the first
        // refusal as if it were the only one.
        // Three variants, to separate which model assumption each refusal is about:
        //   as drawn  — the device as it stands (ccw ramp joins the plateau at σ = 7/8).
        //   late ramp — the join moved to 3/4, ahead of the footprint's own σ = 0.7675 start.
        //   no ramps  — both ends flat, so the part has ONE support region and
        //               `HoleCrossesRegions` cannot fire at all; whatever refuses then is the
        //               footprint reaching the sheet's free σ-ends, and nothing else.
        for (name, ramp_end, flat) in [
            ("as drawn", Q::new(7, 8), false),
            ("late ramp", Q::new(3, 4), false),
            ("no ramps", Q::new(7, 8), true),
        ] {
            let mut spec = acceptance::two_ramp_spec();
            spec.ccw.ramp_end = acceptance::lapped::Azimuth::Sigma(ramp_end);
            if flat {
                spec.ccw = acceptance::lapped::SideAngles::flat(
                    acceptance::lapped::Azimuth::Sigma(Q::new(9, 8)),
                );
                spec.seam_offset = qi(0);
            }
            let v = match acceptance::lapped::lapped_cone(&spec) {
                Ok(v) => v,
                Err(f) => {
                    println!("\n  {name}: the recipe itself refuses — {f:?}");
                    continue;
                }
            };
            println!("\n  {name}: regions as the recipe derives them");
            for (i, (band, h)) in v.regions.iter().enumerate() {
                println!(
                    "    {i}: σ ∈ [{:+.5}, {:+.5}]  h {:+.4}",
                    rat_to_f64(&band.lo),
                    rat_to_f64(&band.hi),
                    rat_to_f64(h)
                );
            }
            let part = acceptance::self_lapping_cone_from(
                &spec,
                8,
                8,
                false,
                Some(acceptance::bat_cutter()),
            );
            let clock = std::time::Instant::now();
            print!("\n-- develop(), window subtracted, {name} (segments 8) --\n  ");
            match part.develop() {
                certify_core::Verdict::Verified(flat) => println!(
                    "Verified   ε {:.3e}   {} hole(s)   [{:.1}s]",
                    rat_to_f64(flat.eps()),
                    flat.holes().len(),
                    clock.elapsed().as_secs_f64()
                ),
                certify_core::Verdict::Refuted(f) => {
                    println!("Refuted({f:?})   [{:.1}s]", clock.elapsed().as_secs_f64())
                }
                certify_core::Verdict::Unresolved(e) => println!(
                    "Unresolved at ε {:.3e}   [{:.1}s]",
                    rat_to_f64(&e),
                    clock.elapsed().as_secs_f64()
                ),
            }
        }
    } // end of the walk + the two measured variants
    // — two experiments that separate "the model cannot spell an edge-merging cut" from "the
    //   engine cannot do this cut at all" —
    //
    //   narrowed  — the same window, the same tilted plane, the same +y side, x scaled ±2.3 → ±1.0
    //               so the footprint fits INSIDE the 27° lap wedge and stops short of the sheet's
    //               free σ-ends. If this develops, reaching the end is the whole cause.
    //   −y side   — the drawing intact, the plane's v mirrored to (0, −1428, −1475)/2053, which
    //               lands it on plain single-sheet body material.
    let ylo_q = Q::new(6_419_782_418_195_965, 1_000_000_000_000_000);
    let yhi_q = Q::new(9_119_782_418_195_967, 1_000_000_000_000_000);
    let narrowed = |half: Q| -> Vec<geom::content::Edge<Bignum>> {
        arrange2d::profile::Profile::new()
            .polygon(&[
                [half.neg(), ylo_q.clone()],
                [half.clone(), ylo_q.clone()],
                [half.clone(), yhi_q.clone()],
                [half.neg(), yhi_q.clone()],
            ])
            .into_edges()
    };
    // Both placements from one sign: v = (0, ±1428, −1475)/2053 and the plane's OWN normal
    // u × v = (0, −v_z, v_y) ∝ (0, 1475, ±1428). Reusing `bat_sweep` for the mirrored plane would
    // sweep the profile obliquely across a plane it is not perpendicular to — a different cutter,
    // and one whose prism can miss the sheet entirely.
    let frame_of = |sign: i128| {
        develop::extrude::Frame::new(
            [qi(0), qi(0), qi(0)],
            [qi(1), qi(0), qi(0)],
            [qi(0), Q::new(sign * 1428, 2053), Q::new(-1475, 2053)],
        )
        .expect("independent axes")
    };
    let sweep_of = |sign: i128| {
        develop::extrude::Apex::direction([qi(0), qi(1475), qi(sign * 1428)]).expect("a direction")
    };
    let cases: Vec<(&str, i128, Vec<geom::content::Edge<Bignum>>)> = vec![
        (
            "narrowed to ±0.6 (+y, clear of the free edges)",
            1,
            narrowed(Q::new(3, 5)),
        ),
        (
            "as drawn, −y side (plain body), plane's own normal",
            -1,
            acceptance::bat_cutout_profile(),
        ),
    ];
    for (name, sign, profile) in cases {
        println!("\n== {name} ==");
        // Measure the footprint FIRST — coarse, but enough to see the σ-extent, the pass count and
        // whether it reaches a band end. An experiment whose geometry is assumed rather than
        // measured proves nothing: ±1.0 was picked by arithmetic and landed its corners on the free
        // edges, which is why it read like a model limit.
        let cast = Cast::new(frame_of(sign), sweep_of(sign)).expect("off-plane sweep");
        let (mut plo, mut phi, mut pylo, mut pyhi) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for e in &profile {
            if let geom::content::Edge::Seg(s) = e {
                for p in [&s.start, &s.end] {
                    let (x, y) = (surd_to_f64(&p.x), surd_to_f64(&p.y));
                    plo = plo.min(x);
                    phi = phi.max(x);
                    pylo = pylo.min(y);
                    pyhi = pyhi.max(y);
                }
            }
        }
        let steps = 360i128;
        let mut hit: Vec<f64> = Vec::new();
        let mut rho = (f64::MAX, f64::MIN);
        for i in 0..=steps {
            let sigma = Q::new(-9, 8).add(&Q::new(9 * i, 4 * steps));
            let p1 = match chart.surface(&qi(-1), &qi(0)).eval(&sigma) {
                Some(p) => p,
                None => continue,
            };
            let s = rat_to_f64(&p1[0]).hypot(rat_to_f64(&p1[1]));
            for k in 0..=90 {
                let mu = -10.75 / s + (10.75 - 4.0) / s * (k as f64) / 90.0;
                let q = Q::new((mu * 100_000.0) as i128, 100_000);
                let inside = chart
                    .surface(&q, &qi(0))
                    .eval(&sigma)
                    .and_then(|p| cast.coords(&p))
                    .map(|(a, b)| {
                        let (a, b) = (rat_to_f64(&a), rat_to_f64(&b));
                        a >= plo && a <= phi && b >= pylo && b <= pyhi
                    })
                    .unwrap_or(false);
                if inside {
                    hit.push(rat_to_f64(&sigma));
                    let r = mu.abs() * s;
                    rho = (rho.0.min(r), rho.1.max(r));
                    break;
                }
            }
        }
        if hit.is_empty() {
            println!(
                "  footprint: EMPTY — this cutter meets no material (the cut would be a no-op)"
            );
        } else {
            let (lo, hi) = (hit[0], hit[hit.len() - 1]);
            let gap = hit
                .windows(2)
                .filter(|w| w[1] - w[0] > 3.0 * 2.25 / steps as f64)
                .count();
            println!(
                "  footprint: σ ∈ [{lo:+.4}, {hi:+.4}], {} pass(es), ρ ∈ [{:.3}, {:.3}]; \
                 reaches a band end: {}",
                gap + 1,
                rho.0,
                rho.1,
                lo <= -1.1249 || hi >= 1.1249
            );
        }
        let part = acceptance::self_lapping_cone_from(
            &acceptance::two_ramp_spec(),
            8,
            8,
            false,
            Some(author::part::Cutter::extrude(
                frame_of(sign),
                sweep_of(sign),
                profile,
            )),
        );
        let clock = std::time::Instant::now();
        print!("  develop(): ");
        match part.develop() {
            certify_core::Verdict::Verified(flat) => {
                println!(
                    "Verified   ε {:.3e}   {} hole(s)   [{:.1}s]",
                    rat_to_f64(flat.eps()),
                    flat.holes().len(),
                    clock.elapsed().as_secs_f64()
                );
                // A green certificate says nothing about whether the cut HAPPENED. Print the
                // derived role of every op, so an `Inactive` cut cannot pass for a success.
                for (i, o) in flat.report().ops.iter().enumerate() {
                    println!(
                        "    op {i}: {} → {:?}",
                        if o.subtract { "subtract" } else { "intersect" },
                        o.role
                    );
                }
            }
            certify_core::Verdict::Refuted(f) => {
                println!("Refuted({f:?})   [{:.1}s]", clock.elapsed().as_secs_f64())
            }
            certify_core::Verdict::Unresolved(e) => println!(
                "Unresolved at ε {:.3e}   [{:.1}s]",
                rat_to_f64(&e),
                clock.elapsed().as_secs_f64()
            ),
        }
    }
}
