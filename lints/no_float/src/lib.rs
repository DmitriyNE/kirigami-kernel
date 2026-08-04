#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::LitKind;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Flags floating-point **literals** (`1.5`, `2.0`, `1e-3`) in the certified crates
    /// (`lattice`, `certify_core`, `arrange2d`; `testgen.rs` excepted).
    ///
    /// ### Why is this bad?
    /// Invariant 1: no floats in certified paths — a float that reaches a predicate is a
    /// bug. The `cargo xtask lint` token scan catches `f32`/`f64` *types*, but a float can
    /// enter as a *literal* with no float token, invisible to any text scan (and text can't
    /// even tell `1.0` from tuple field access `.0`). This type-aware lint closes that gap.
    ///
    /// ### Example
    /// ```rust,ignore
    /// let x = 1.5;            // flagged: an f64 literal, no `f64` token
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// let x = Rat::new(3, 2); // exact
    /// ```
    pub NO_FLOAT,
    Deny,
    "floating-point literal in a certified path (invariant 1)"
}

/// The certified crates (crate names use underscores: `certify-core` -> `certify_core`).
const CERTIFIED: &[&str] = &["lattice", "certify_core", "arrange2d"];

impl<'tcx> LateLintPass<'tcx> for NoFloat {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &Expr<'tcx>) {
        if !CERTIFIED.contains(&cx.tcx.crate_name(LOCAL_CRATE).as_str()) {
            return;
        }
        // `testgen.rs` holds test-only generators, excepted like the token scan.
        let filename = cx.sess().source_map().span_to_filename(expr.span);
        if format!("{filename:?}").contains("testgen.rs") {
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
                "certified paths are exact (lattice numbers); floats live behind the `diagnostics` feature",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
