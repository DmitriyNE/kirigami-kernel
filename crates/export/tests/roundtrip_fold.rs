//! DD.1 — the certified **3D↔2D round-trip** on the cone gore (γ = 0): author an interior ECAD
//! feature on the *flat pattern*, fold it back onto the cone, and recover the original 3-D geometry
//! — the fold-back leg the Stage-1 demo skips (it only lifts cuts *forward*).
//!
//! The core round-trip closure runs under default features (no float in the certificate; the
//! `f64` audit uses `Rat::numer_denom_decimal`, tests being exempt from the no-float rule). The
//! `develop_cone` float-oracle corroboration is gated behind `diagnostics` (that module is where
//! the float diagnostics live), like the sibling `mesh3d` corroboration tests.

use certify_core::Verdict;
use develop::cone::{ConeDevelopment, DevConfig};
use develop::fold::fold_outline;
use fixtures::devices::cone;
use lattice::{Bignum, Interval, Rat};

type Q = Rat<Bignum>;

/// `f64` of an exact rational (audit only — never in the certificate).
fn to_f64(r: &Q) -> f64 {
    let (n, d) = r.numer_denom_decimal();
    n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
}

fn tag<T, E: core::fmt::Debug, M>(v: &Verdict<T, E, M>) -> String {
    match v {
        Verdict::Verified(_) => "Verified".into(),
        Verdict::Refuted(w) => format!("Refuted({w:?})"),
        Verdict::Unresolved(_) => "Unresolved".into(),
    }
}

/// The interior ECAD feature, as authored on the FLAT pattern: a rectangle. Its 3-D preimage is the
/// chart-space rectangle σ ∈ [0, 1/2], μ̂ ∈ [−3/2, −1] (the −μ̂ band side), developed forward here so
/// the test self-locates; folding it back is the leg under test.
fn feature_corners() -> [(Q, Q); 4] {
    [
        (Q::from_i128(0), Q::new(-3, 2)),
        (Q::new(1, 2), Q::new(-3, 2)),
        (Q::new(1, 2), Q::from_i128(-1)),
        (Q::from_i128(0), Q::from_i128(-1)),
    ]
}

/// Author a rectangle on the flat pattern, fold it back onto the cone, and recover the 3-D
/// geometry to `< ε` — the DD.1 round-trip (develop ∘ fold ≈ identity), audited against the
/// independent float 3-D positions.
#[test]
fn flat_authored_feature_folds_back_and_recovers_the_3d_geometry() {
    let chart = cone();
    let dev = ConeDevelopment::new(&chart).expect("the device cone is a canonical arctan cone");
    let cfg = DevConfig::tight();
    // One-sided gore σ ∈ [0, 1] — the fold's signed-area bisection is faithful (span < π).
    let domain = Interval {
        lo: Q::from_i128(0),
        hi: Q::from_i128(1),
    };
    let w0 = Q::from_i128(0);

    let mut flat: Vec<[Q; 2]> = Vec::new();
    let mut truth3d: Vec<[f64; 3]> = Vec::new(); // independent float 3-D positions (the audit oracle)
    for (s, m) in &feature_corners() {
        let (x, y) = dev.point(s, m, &cfg).center();
        flat.push([x, y]);
        let p = chart.surface(m, &w0).eval(s).expect("surface eval");
        truth3d.push([to_f64(&p[0]), to_f64(&p[1]), to_f64(&p[2])]);
    }

    // FOLD the flat-authored feature back onto the cone → a certified 3-D wire.
    let wire = match fold_outline(
        &chart,
        &flat,
        &w0,
        &domain,
        60,
        true,
        &cfg,
        &Q::from_i128(1),
    ) {
        Verdict::Verified(w) => w,
        other => panic!(
            "the flat-authored feature must fold back and certify: {}",
            tag(&other)
        ),
    };
    assert_eq!(
        wire.points.len(),
        4,
        "one folded 3-D vertex per authored corner"
    );
    assert!(
        wire.eps.cmp(&Q::new(1, 2)) == core::cmp::Ordering::Less,
        "the wire round-trip backward error must clear the DRC (ε < clearance/2)"
    );

    // ROUND-TRIP CLOSURE (the DD.1 assertion): each folded 3-D vertex recovers its original 3-D
    // point — develop ∘ fold ≈ identity — against the independent float oracle `truth3d`.
    let mut max_recover = 0f64;
    for (b, t) in wire.points.iter().zip(&truth3d) {
        let d = ((to_f64(&b[0].mid()) - t[0]).powi(2)
            + (to_f64(&b[1].mid()) - t[1]).powi(2)
            + (to_f64(&b[2].mid()) - t[2]).powi(2))
        .sqrt();
        max_recover = max_recover.max(d);
    }
    assert!(
        max_recover < 1e-6,
        "round-trip recovery residual {max_recover:e} (folded 3-D must recover the authored preimage)"
    );
}

/// A flat-authored feature whose direction angle exceeds the gore's angular range is refused
/// `OutOfGore` — the fold is fail-closed (a feature outside the developed panel cannot be folded
/// onto the cone), never a wrong `Verified`.
#[test]
fn a_flat_feature_outside_the_gore_is_refused() {
    use develop::fold::FoldFault;
    let chart = cone();
    let dev = ConeDevelopment::new(&chart).unwrap();
    let cfg = DevConfig::tight();
    // Develop a point at σ = 5 (well past the domain [0,1] whose max angle is ψ(1)), then try to
    // fold it back over [0,1]: its flat angle exceeds the gore → no σ ∈ [0,1] reaches it.
    let (x, y) = dev
        .point(&Q::from_i128(5), &Q::from_i128(-1), &cfg)
        .center();
    let v = fold_outline(
        &chart,
        &[[x, y]],
        &Q::from_i128(0),
        &Interval {
            lo: Q::from_i128(0),
            hi: Q::from_i128(1),
        },
        40,
        true,
        &cfg,
        &Q::from_i128(1),
    );
    assert!(matches!(v, Verdict::Refuted(FoldFault::OutOfGore)));
}

/// ORACLE ∧ AUDIT: the independent float `develop_cone` reproduces the authored flat feature —
/// corroborating that the flat rectangle is the genuine developed image (the develop leg the fold
/// inverts). A fine strip over σ ∈ [0, 1] with the two μ̂ columns; the feature corners sit at σ = 0
/// (row 0) and σ = 1/2 (row K), so the flat sides fall on grid rows.
#[cfg(feature = "diagnostics")]
#[test]
fn the_flat_feature_corroborates_the_develop_cone_oracle() {
    use export::approx::rat_to_f64;
    use export::mesh3d::develop_cone;

    let chart = cone();
    let dev = ConeDevelopment::new(&chart).unwrap();
    let cfg = DevConfig::tight();
    let w0 = Q::from_i128(0);

    let flat: Vec<[Q; 2]> = feature_corners()
        .iter()
        .map(|(s, m)| {
            let (x, y) = dev.point(s, m, &cfg).center();
            [x, y]
        })
        .collect();

    const K: usize = 200;
    let nrows = 2 * K + 1; // 401 rows; σ = 1/2 → row K
    let cols = [Q::new(-3, 2), Q::from_i128(-1)];
    let mut positions = Vec::with_capacity(nrows * 2);
    for i in 0..nrows {
        let s = Q::new(i as i128, (nrows - 1) as i128);
        for m in &cols {
            let p = chart.surface(m, &w0).eval(&s).expect("surface eval");
            positions.push([rat_to_f64(&p[0]), rat_to_f64(&p[1]), rat_to_f64(&p[2])]);
        }
    }
    let dc = develop_cone(&positions, nrows, 2);
    // Corner → (grid row, column) into the develop_cone flat map.
    let rows_cols = [(0usize, 0usize), (K, 0), (K, 1), (0, 1)];
    let mut max_oracle = 0f64;
    for (corner, &(row, col)) in flat.iter().zip(&rows_cols) {
        let f = &dc[row * 2 + col];
        let d = ((rat_to_f64(&corner[0]) - f[0]).powi(2) + (rat_to_f64(&corner[1]) - f[1]).powi(2))
            .sqrt();
        max_oracle = max_oracle.max(d);
    }
    assert!(
        max_oracle < 1e-5,
        "develop_cone oracle residual {max_oracle:e} (the flat feature must be the developed image)"
    );
}
