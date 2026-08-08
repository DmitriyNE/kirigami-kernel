#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::LitKind;
use rustc_hir::def::Res;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::{AmbigArg, Expr, ExprKind, PrimTy, QPath, Ty, TyKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Flags floating-point **literals** (`1.5`, `1e-3`) and **types** (`f32`/`f64` in a
    /// signature, field, cast, or generic argument) in the *compiled* certified crates
    /// (`lattice`, `certify_core`, `arrange2d`). Test code — `#[cfg(test)]` modules and
    /// `tests/` dirs — is not compiled by the default lib target, so floats are permitted
    /// there (independent oracles, input generators, readable expectations); see
    /// `docs/engineering-log.md`.
    ///
    /// ### Why is this bad?
    /// Invariant 1: no floats in the certified predicate path — a float that reaches a
    /// predicate is a bug. This single, type-aware lint is the sole enforcement (it replaced
    /// the former `cargo xtask lint` text scan): it catches float *literals* (`1.5` carries
    /// no `f32`/`f64` token, and text can't even tell `1.0` from tuple access `.0`) **and**
    /// float *types* (which a literal-only check misses), with none of a text scan's
    /// comment/string false positives.
    ///
    /// ### Example
    /// ```rust,ignore
    /// let x = 1.5;              // flagged: an f64 literal
    /// fn f(w: f64) -> f64 { w } // flagged: f64 types
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// let x = Rat::new(3, 2);   // exact (lattice number)
    /// ```
    pub NO_FLOAT,
    Deny,
    "floating-point literal or type in a certified path (invariant 1)"
}

/// The certified crates (crate names use underscores: `certify-core` -> `certify_core`).
const CERTIFIED: &[&str] = &["lattice", "certify_core", "arrange2d"];

/// Is the crate currently being linted one of the certified crates?
fn in_certified(cx: &LateContext<'_>) -> bool {
    let name = cx.tcx.crate_name(LOCAL_CRATE);
    CERTIFIED.contains(&name.as_str())
}

const HELP: &str =
    "certified paths are exact (lattice numbers); floats live behind the `diagnostics` feature";

impl<'tcx> LateLintPass<'tcx> for NoFloat {
    /// Float **literals** — `1.5`, `2.0`, `1e-3` (and literals whose type is inferred to a
    /// float). No `f32`/`f64` token, so invisible to any text scan.
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &Expr<'tcx>) {
        if !in_certified(cx) {
            return;
        }
        if let ExprKind::Lit(lit) = &expr.kind
            && matches!(lit.node, LitKind::Float(..))
        {
            span_lint_and_help(
                cx,
                NO_FLOAT,
                expr.span,
                "floating-point literal in a certified path",
                None,
                HELP,
            );
        }
    }

    /// Float **types** — `f16`/`f32`/`f64`/`f128` wherever a `Ty` node appears: fn params
    /// and returns, `let` annotations, struct/enum fields, type aliases, generic arguments
    /// (`Vec<f64>`), and `as f64` cast targets.
    fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &'tcx Ty<'tcx, AmbigArg>) {
        if !in_certified(cx) {
            return;
        }
        if let TyKind::Path(QPath::Resolved(_, path)) = ty.kind
            && matches!(path.res, Res::PrimTy(PrimTy::Float(_)))
        {
            span_lint_and_help(
                cx,
                NO_FLOAT,
                ty.span,
                "floating-point type in a certified path",
                None,
                HELP,
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
