//! Transverse/tangent classification (M3a Phase 4) under the most-degenerate-first
//! guard `d² > 0 ∨ ¬COINCIDENT`. Tangency by exact A-identity (line/circle
//! `dist² = r²`; circle/circle `d² = (r₁ ± r²)²`), transversality by the sign of
//! `det(ċ_A, ċ_B)`. Emits touch vertices with sidedness bits (raw crossing data;
//! the ℤ₂² face-flip encoding is deferred to slice 3d).
//!
//! Both A-identities are captured exactly by `det(ċ_A, ċ_B) = 0`: the carriers'
//! tangents at the point are parallel precisely when the line is tangent to the
//! circle (or the two circles are tangent). So the determinant's sign is the whole
//! decision — nonzero is transverse (and the sign is the sidedness datum), zero is
//! the tangency identity. The guard is enforced upstream by the spine's step
//! order (this runs only on non-coincident carriers).

use crate::event::TouchKind;
use geom::content::{Edge, Point2};
use lattice::{Backend, Surd};

/// The carrier's tangent direction `ċ` at `p`: a line's is the constant `(b, −a)`;
/// a circle's is the radius rotated a quarter turn, `(−(p_y − cy), p_x − cx)`.
fn tangent_vec<B: Backend>(edge: &Edge<B>, p: &Point2<B>) -> (Surd<B>, Surd<B>) {
    match edge {
        Edge::Seg(s) => (
            Surd::from_rat(s.line.b.clone()),
            Surd::from_rat(s.line.a.neg()),
        ),
        Edge::Arc(a) => {
            let dx = p.x.sub(&Surd::from_rat(a.circle.cx.clone())).unwrap_surd();
            let dy = p.y.sub(&Surd::from_rat(a.circle.cy.clone())).unwrap_surd();
            (dy.neg(), dx)
        }
    }
}

/// `u_x·v_y − u_y·v_x`. Both tangents are built from the shared point `p` and
/// rational carrier data, so the products stay in one radical and never escalate.
fn cross<B: Backend>(u: &(Surd<B>, Surd<B>), v: &(Surd<B>, Surd<B>)) -> Surd<B> {
    u.0.mul(&v.1)
        .unwrap_surd()
        .sub(&u.1.mul(&v.0).unwrap_surd())
        .unwrap_surd()
}

/// The classification determinant `det(ċ_A, ċ_B)` at `p`; its sign is the whole
/// transverse/tangent decision, and `= 0` is the exact tangency A-identity.
pub fn det_at<B: Backend>(p: &Point2<B>, a: &Edge<B>, b: &Edge<B>) -> Surd<B> {
    cross(&tangent_vec(a, p), &tangent_vec(b, p))
}

/// Classify a retained touch from its determinant: a nonzero sign is transverse
/// (the sign is the sidedness datum), zero is tangent.
pub fn kind_of<B: Backend>(d: &Surd<B>) -> TouchKind {
    match d.sign() {
        0 => TouchKind::Tangent,
        s => TouchKind::Transverse { det_sign: s },
    }
}
