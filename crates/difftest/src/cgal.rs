//! The CGAL FFI bridge (feature `cgal`). One function, strings only — no CGAL
//! type crosses the boundary; all `unsafe` is confined to the cxx glue.

#[cxx::bridge(namespace = "kirigami")]
mod ffi {
    unsafe extern "C++" {
        include!("difftest/src/cgal_shim.h");

        /// Phase-0 build smoke: exercises a heavy CGAL header + its exact number
        /// type and gmp linking. Returns an exact rational as a string ("3/4").
        fn cgal_smoke() -> String;

        /// Phase-5 circular-kernel `Arrangement_2` smoke: the intersection vertex
        /// of two crossing segments as two `a b d` triples (`a + b·√d`), proving
        /// the circular kernel builds/links and `Root_of_2` extraction works.
        fn cgal_arr_smoke() -> String;

        /// Phase-5 differential oracle: the genuine (degree ≥ 3) intersection
        /// vertices of an arrangement of the input curves (one per line: `S x1 y1
        /// x2 y2` segment / `C cx cy r2` circle, rationals as "num/den"), each
        /// returned as `xa xb xd ya yb yd` (coordinate = a + b·√d).
        fn cgal_arrange(input: &str) -> String;

        /// Phase-5/3c overlap-edge oracle: one line per arrangement edge,
        /// `n xa xb xd ya yb yd ua ub ud va vb vd` — `n` = number of input curves
        /// covering the edge (≥ 2 ⇒ overlap/merged), then the two endpoints. Input
        /// curves carry an id: `S x1 y1 x2 y2 id` / `C cx cy r2 id`.
        fn cgal_arrange_edges(input: &str) -> String;
    }
}

pub use ffi::{cgal_arr_smoke, cgal_arrange, cgal_arrange_edges, cgal_smoke};

#[cfg(test)]
mod tests {
    /// Runtime link smoke: the FFI resolves and CGAL exact arithmetic runs.
    #[test]
    fn cgal_smoke_exact_rational() {
        assert_eq!(super::cgal_smoke(), "3/4");
    }

    /// The circular-kernel arrangement of two crossing segments has its
    /// intersection at the exact rational origin (0 + 0·√0, twice; `Gmpq` streams
    /// `0` as `0/1`).
    #[test]
    fn cgal_arr_smoke_origin_vertex() {
        assert_eq!(super::cgal_arr_smoke(), "0/1 0/1 0/1 ; 0/1 0/1 0/1");
    }
}
