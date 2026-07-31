//! The CGAL FFI bridge (feature `cgal`). One function, strings only — no CGAL
//! type crosses the boundary; all `unsafe` is confined to the cxx glue.

#[cxx::bridge(namespace = "kirigami")]
mod ffi {
    unsafe extern "C++" {
        include!("difftest/src/cgal_shim.h");

        /// Phase-0 build smoke: exercises a heavy CGAL header + its exact number
        /// type and gmp linking. Returns an exact rational as a string ("3/4").
        fn cgal_smoke() -> String;
    }
}

pub use ffi::cgal_smoke;

#[cfg(test)]
mod tests {
    /// Runtime link smoke: the FFI resolves and CGAL exact arithmetic runs.
    #[test]
    fn cgal_smoke_exact_rational() {
        assert_eq!(super::cgal_smoke(), "3/4");
    }
}
