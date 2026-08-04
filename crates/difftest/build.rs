// Builds the CGAL C++ FFI shim — only under `--features cgal`, so the default
// workspace build needs no system C++ libraries.
//
// Toolchain consistency (the darwin trap): the `cxx` crate compiles its own C++
// runtime with the environment's default compiler, which in `nix develop` is
// g++/libstdc++. So this shim must ALSO be g++/libstdc++ (don't force clang++ —
// that mixes libc++ and libstdc++ and the C++ stdlib symbols won't resolve). The
// only darwin gap is that nix's libstdc++ lives in the store, off the linker's
// default search path, so we add it below.
//
// Header discovery: the nix cc-wrapper injects CGAL/boost include paths via
// NIX_CFLAGS_COMPILE (inherited by the g++ cxx-build invokes). gmp/mpfr are
// linked via pkg-config. For a non-nix build, point CGAL_INCLUDE_DIR /
// BOOST_INCLUDE_DIR at the headers.
fn main() {
    if std::env::var_os("CARGO_FEATURE_CGAL").is_none() {
        return;
    }

    let mut build = cxx_build::bridge("src/cgal.rs");
    build
        .file("src/cgal_shim.cc")
        .std("c++17")
        .define("CGAL_HEADER_ONLY", "1")
        // CGAL's headers flood the log with benign GCC ABI notes (`-Wpsabi`).
        .flag_if_supported("-Wno-psabi")
        // No debug info on the oracle shim: recent gcc/glibc (the flake floats
        // nixos-unstable) emit a `.debug_gdb_scripts` section that `rust-lld` rejects
        // ("string is not null terminated") when linking the C++ object. Debug info buys
        // nothing on a differential-test oracle.
        .debug(false);

    for var in ["CGAL_INCLUDE_DIR", "BOOST_INCLUDE_DIR", "GMP_INCLUDE_DIR"] {
        if let Some(dir) = std::env::var_os(var) {
            build.include(dir);
        }
    }

    build.compile("cgal_shim");

    // gmp/mpfr have pkg-config (.pc); nix supplies -L + rpath, pkg-config -l.
    pkg_config::probe_library("gmp").expect("gmp (pkg-config) — run inside `nix develop`");
    pkg_config::probe_library("mpfr").expect("mpfr (pkg-config) — run inside `nix develop`");

    // C++ standard library: the cxx runtime + this shim are libstdc++ (nix's g++
    // default). On darwin libstdc++.dylib is in the nix store, not on the linker's
    // default path — add its directory so the final rustc/clang link resolves the
    // C++ stdlib symbols (`std::ios_base::Init`, the rust::String ctors, …). On
    // Linux libstdc++ is already on the default path (the probe finds nothing to
    // add), and `-lstdc++` is harmlessly redundant with the cc crate's own.
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
    for lib in ["libstdc++.dylib", "libstdc++.so"] {
        if let Ok(o) = std::process::Command::new(&cxx)
            .arg(format!("-print-file-name={lib}"))
            .output()
        {
            let p = String::from_utf8_lossy(&o.stdout);
            let path = std::path::Path::new(p.trim());
            if path.is_absolute() {
                if let Some(dir) = path.parent() {
                    println!("cargo:rustc-link-search=native={}", dir.display());
                }
            }
        }
    }
    println!("cargo:rustc-link-lib=dylib=stdc++");

    println!("cargo:rerun-if-changed=src/cgal.rs");
    println!("cargo:rerun-if-changed=src/cgal_shim.h");
    println!("cargo:rerun-if-changed=src/cgal_shim.cc");
}
