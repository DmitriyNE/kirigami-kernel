// Builds the OCCT STEP-writer C++ FFI shim — only under `--features step`, so
// the default workspace build (and CI clippy) needs no system OpenCASCADE.
//
// Toolchain consistency (the difftest recipe): the `cxx` crate compiles its own
// C++ runtime with the environment's default compiler, which in `nix develop`
// is g++/libstdc++. So this shim is ALSO g++/libstdc++ (the default — don't
// force clang++, which would compile the shim as libc++ and fail to link
// against the libstdc++ cxx runtime). nix's libstdc++ lives in the store, off
// the linker's default path on darwin, so we add it below.
//
// OCCT itself is a prebuilt *libc++* library — but the shim only crosses the
// OCCT boundary via `const char*` / `double` / OCCT's own types; no `std::`
// object crosses, so libc++ (OCCT) and libstdc++ (the shim + cxx runtime)
// coexist at load time without a shared-symbol clash.
//
// Header discovery: OCCT nests its headers in `<occt>/include/opencascade/`,
// and they #include each other unqualified, so that dir must be on the include
// path directly (the nix cc-wrapper only injects `<occt>/include`). Libs live
// in `<occt>/lib`. Both are derived from the `-isystem` path the nix cc-wrapper
// leaves in NIX_CFLAGS_COMPILE; for a non-nix build, point OCCT_INCLUDE_DIR /
// OCCT_LIB_DIR at the `opencascade` header dir and the lib dir.
fn main() {
    if std::env::var_os("CARGO_FEATURE_STEP").is_none() {
        return;
    }

    let (occt_include, occt_lib) = occt_dirs();

    let mut build = cxx_build::bridge("src/step.rs");
    build
        .file("src/occt_shim.cc")
        .std("c++17")
        // No debug info on the shim: recent gcc/glibc emit a `.debug_gdb_scripts`
        // section `rust-lld` rejects when linking the C++ object; it buys nothing here.
        .debug(false);
    build.include(&occt_include);
    build.compile("occt_shim");

    // OCCT is prebuilt dylibs with no pkg-config; add its lib dir + the STEP
    // dependency closure (leaf → core). TKDESTEP holds the STEPControl_*
    // reader/writer in OCCT 7.8+ (formerly TKSTEP).
    println!("cargo:rustc-link-search=native={occt_lib}");
    for lib in [
        "TKDESTEP",
        "TKDE",
        "TKXSBase",
        "TKShHealing",
        "TKPrim",
        "TKBRep",
        "TKTopAlgo",
        "TKGeomAlgo",
        "TKGeomBase",
        "TKG3d",
        "TKG2d",
        "TKMath",
        "TKernel",
    ] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }

    // C++ standard library for the shim + cxx runtime (both libstdc++). On darwin
    // libstdc++.dylib is in the nix store, not on the linker's default path — add
    // its directory so the final rustc/clang link resolves the C++ stdlib symbols
    // (`std::ios_base::Init`, the rust::String ctors, …). On Linux libstdc++ is
    // already on the default path. (Copied from difftest's build.rs.)
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

    println!("cargo:rerun-if-changed=src/step.rs");
    println!("cargo:rerun-if-changed=src/occt_shim.h");
    println!("cargo:rerun-if-changed=src/occt_shim.cc");
    println!("cargo:rerun-if-env-changed=OCCT_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=OCCT_LIB_DIR");
}

/// The OCCT `opencascade` header dir and the lib dir. Prefers explicit
/// `OCCT_INCLUDE_DIR` / `OCCT_LIB_DIR`; otherwise derives both from the
/// `-isystem <occt>/include` token the nix cc-wrapper leaves in
/// NIX_CFLAGS_COMPILE.
fn occt_dirs() -> (String, String) {
    let env_inc = std::env::var("OCCT_INCLUDE_DIR").ok();
    let env_lib = std::env::var("OCCT_LIB_DIR").ok();
    if let (Some(i), Some(l)) = (&env_inc, &env_lib) {
        return (i.clone(), l.clone());
    }

    // Find `<store>/opencascade-occt-<ver>/include` among the -isystem tokens.
    let cflags = std::env::var("NIX_CFLAGS_COMPILE").unwrap_or_default();
    let root = cflags
        .split_whitespace()
        .find(|t| t.contains("opencascade-occt") && t.ends_with("/include"))
        .map(|t| t.trim_end_matches("/include").to_string())
        .expect(
            "OCCT not found: set OCCT_INCLUDE_DIR + OCCT_LIB_DIR, or run inside `nix develop` \
             (which puts opencascade-occt on NIX_CFLAGS_COMPILE)",
        );

    let include = env_inc.unwrap_or_else(|| format!("{root}/include/opencascade"));
    let lib = env_lib.unwrap_or_else(|| format!("{root}/lib"));
    (include, lib)
}
