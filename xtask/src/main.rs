//! `cargo xtask lint` — the repo's spec/code invariant lints (vv-guide §6), in Rust.
//!
//! Replaces `scripts/lint/*.sh`. Each check is a pure function over already-read source
//! files, so it is unit-tested with both a *passing* and a *known-bad* fixture that asserts
//! the lint actually fires — closing the "vacuous pass" class the old awk `vv_matrix_gate`
//! fell into. Zero dependencies (std only); cross-platform (no grep/awk/sed divergence).
//!
//! The lints here are text/structure checks. Invariant 1 (no floats in certified paths) is
//! **not** among them — it is enforced solely by the type-aware `no_float` dylint lint
//! (`lints/no_float/`), which catches float literals *and* `f32`/`f64` types with none of a
//! text scan's comment/string false positives; see `docs/engineering-log.md`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Milestones that have shipped — the `vv_matrix` gate only enforces ★ rows whose milestone
/// has landed. Extend as each ships (mirrors the former `vv_matrix_gate.sh` `landed=` var).
const LANDED: &[&str] = &["M0", "M1", "M2", "M3a", "M3c", "M3d", "M3e", "AUTH.1"];

/// The repo root, embedded at compile time (this crate lives at `<root>/xtask`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a child of the repo root")
        .to_path_buf()
}

/// A source file read into memory.
struct SrcFile {
    /// Repo-relative path (forward slashes), e.g. `crates/lattice/src/rat.rs`.
    rel: String,
    text: String,
}

/// One lint hit.
struct Finding {
    rel: String,
    line: usize,
    msg: String,
}

/// Recursively collect `*.rs` files under `<root>/<sub>`, as repo-relative `SrcFile`s.
fn collect_rs(root: &Path, sub: &str) -> Vec<SrcFile> {
    let mut out = Vec::new();
    collect_ext(root, &root.join(sub), "rs", &mut out);
    out
}

fn collect_ext(root: &Path, dir: &Path, ext: &str, out: &mut Vec<SrcFile>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ext(root, &path, ext, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(SrcFile { rel, text });
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Matching helpers
// ---------------------------------------------------------------------------------------

/// Is `line` a Rust doc-comment (`///` or `//!`, after leading whitespace)?
fn is_doc_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("///") || t.starts_with("//!")
}

// ---------------------------------------------------------------------------------------
// The checks
// ---------------------------------------------------------------------------------------

/// **tuple-predicate** (spec §8.2 / glossary): the adjective "proportional" is banned in
/// doc-comments — predicates on multi-component objects name the tuple.
fn tuple_predicate(files: &[SrcFile]) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in files {
        for (i, line) in f.text.lines().enumerate() {
            if is_doc_comment(line) && line.contains("proportional") {
                out.push(Finding {
                    rel: f.rel.clone(),
                    line: i + 1,
                    msg: "banned adjective \"proportional\" (name the tuple)".into(),
                });
            }
        }
    }
    out
}

/// **:= census** (spec §8.2 commit protocol): every `NAME :=` defines exactly once. A name
/// defined twice in the scanned code is a lint failure. Returns one finding per duplicate.
fn census(files: &[SrcFile]) -> Vec<Finding> {
    use std::collections::BTreeMap;
    // name -> (count, first (rel, line))
    let mut seen: BTreeMap<String, (usize, String, usize)> = BTreeMap::new();
    for f in files {
        for (i, line) in f.text.lines().enumerate() {
            for name in defined_names(line) {
                let e = seen.entry(name).or_insert((0, f.rel.clone(), i + 1));
                e.0 += 1;
            }
        }
    }
    seen.into_iter()
        .filter(|(_, (count, _, _))| *count > 1)
        .map(|(name, (count, rel, line))| Finding {
            rel,
            line,
            msg: format!("name `{name}` defined {count}× (`:=` census)"),
        })
        .collect()
}

/// Names introduced by `NAME :=` on a line — `NAME` is `[A-Za-z_][A-Za-z0-9_()-]*` directly
/// (modulo whitespace) before `:=` (mirrors the former `census.sh` regex).
fn defined_names(line: &str) -> Vec<String> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b':' && b[i + 1] == b'=' {
            // walk back over whitespace, then over the identifier chars
            let mut j = i;
            while j > 0 && (b[j - 1] == b' ' || b[j - 1] == b'\t') {
                j -= 1;
            }
            let end = j;
            while j > 0 && {
                let c = b[j - 1];
                c.is_ascii_alphanumeric() || matches!(c, b'_' | b'(' | b')' | b'-')
            } {
                j -= 1;
            }
            if end > j && (b[j].is_ascii_alphabetic() || b[j] == b'_') {
                out.push(line[j..end].to_string());
            }
        }
        i += 1;
    }
    out
}

/// **vv-matrix gate** (vv-guide §6/§8): a **landed** soundness-critical (★) row must carry a
/// `{Kani ∨ Lean ∨ rc-hyp}` proof cell. Parses the markdown table (`|`-delimited columns:
/// col 2 = Item, col 7 = Kani, col 8 = Lean).
fn vv_matrix(matrix: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, line) in matrix.lines().enumerate() {
        let t = line.trim_start();
        if !t.starts_with('|') {
            continue;
        }
        // separator row: only `|`, `-`, `:`, space
        if t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
            continue;
        }
        let cols: Vec<&str> = line.split('|').collect();
        // cols[0] = "" (before first |); Item = cols[1], Kani = cols[6], Lean = cols[7].
        let Some(item) = cols.get(1) else { continue };
        if !item.contains('★') {
            continue;
        }
        let Some(ms) = milestone_tag(item) else {
            continue;
        };
        if !LANDED.contains(&ms.as_str()) {
            continue;
        }
        let kani = cols.get(6).copied().unwrap_or("");
        let lean = cols.get(7).copied().unwrap_or("");
        let ok = kani.contains('✅') || lean.contains('✅') || line.contains("rc-hyp ✅");
        if !ok {
            out.push(Finding {
                rel: "vv-matrix.md".into(),
                line: i + 1,
                msg: format!(
                    "landed ★ row lacks {{Kani ∨ Lean ∨ rc-hyp}}: {}",
                    item.trim()
                ),
            });
        }
    }
    out
}

/// The `[Mx]` milestone tag inside an item cell, e.g. `[M3d]` -> `M3d`.
fn milestone_tag(item: &str) -> Option<String> {
    let start = item.find('[')?;
    let end = item[start..].find(']')? + start;
    let inner = &item[start + 1..end];
    if inner.starts_with('M') && inner[1..].chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(inner.to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------------------

fn print_findings(findings: &[Finding]) {
    for f in findings {
        println!("  {}:{}: {}", f.rel, f.line, f.msg);
    }
}

fn run_lint() -> bool {
    let root = repo_root();
    let crate_rs = collect_rs(&root, "crates");
    let matrix = std::fs::read_to_string(root.join("vv-matrix.md")).unwrap_or_default();

    let mut ok = true;
    let mut report = |name: &str, findings: Vec<Finding>| {
        if findings.is_empty() {
            println!("{name}: OK");
        } else {
            println!("{name}: FAIL ({} hit(s))", findings.len());
            print_findings(&findings);
            ok = false;
        }
    };

    report("tuple-predicate", tuple_predicate(&crate_rs));
    report(":= census", census(&crate_rs));
    report("vv-matrix gate", vv_matrix(&matrix));
    report(
        "panic-freedom discharge",
        panic_freedom_discharge(&crate_rs),
    );
    ok
}

/// **panic-freedom discharge** (`docs/trusted-invariants.md`): in the pure tier (`lattice`,
/// `certify-core`), every `#[allow(clippy::…)]` for a panic-capable lint must carry a nearby
/// `// PANIC-FREEDOM:` justification — so a bare `#[allow]` cannot silently defeat the
/// crate-root `deny`. The `deny` itself is enforced by clippy; this guards its exceptions.
fn panic_freedom_discharge(files: &[SrcFile]) -> Vec<Finding> {
    const PANIC_LINTS: &[&str] = &[
        "unwrap_used",
        "expect_used",
        "panic",
        "unreachable",
        "todo",
        "unimplemented",
    ];
    let mut out = Vec::new();
    for f in files {
        let pure = f.rel.starts_with("crates/lattice/src/")
            || f.rel.starts_with("crates/certify-core/src/");
        if !pure {
            continue;
        }
        let lines: Vec<&str> = f.text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let is_panic_allow = line.contains("#[allow(clippy::")
                && PANIC_LINTS
                    .iter()
                    .any(|l| line.contains(&format!("clippy::{l}")));
            if !is_panic_allow {
                continue;
            }
            // require a `// PANIC-FREEDOM:` within the 4 lines up to and including this one
            let start = i.saturating_sub(4);
            let discharged = lines[start..=i]
                .iter()
                .any(|l| l.contains("PANIC-FREEDOM:"));
            if !discharged {
                out.push(Finding {
                    rel: f.rel.clone(),
                    line: i + 1,
                    msg: "pure-tier #[allow(clippy::…)] for a panic lint lacks a nearby `// PANIC-FREEDOM:` discharge (docs/trusted-invariants.md)".into(),
                });
            }
        }
    }
    out
}

fn main() -> ExitCode {
    let cmd = std::env::args().nth(1);
    match cmd.as_deref() {
        Some("lint") | None => {
            if run_lint() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some(other) => {
            eprintln!("xtask: unknown command `{other}` (expected `lint`)");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------------------
// Tests — each check gets a passing fixture AND a known-bad one that must fire.
// ---------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn file(rel: &str, text: &str) -> SrcFile {
        SrcFile {
            rel: rel.into(),
            text: text.into(),
        }
    }

    #[test]
    fn tuple_predicate_fires_only_in_doc_comments() {
        let bad = vec![file("crates/x/src/a.rs", "/// the proportional minor")];
        assert_eq!(tuple_predicate(&bad).len(), 1);
        let clean = vec![file(
            "crates/x/src/a.rs",
            "let proportional = 1; // proportional in plain code, not a doc-comment",
        )];
        assert!(tuple_predicate(&clean).is_empty());
    }

    #[test]
    fn census_flags_a_duplicated_definition() {
        let dup = vec![
            file("crates/x/src/a.rs", "FOO := the first"),
            file("crates/y/src/b.rs", "  FOO := the second"),
        ];
        assert_eq!(census(&dup).len(), 1);
        let unique = vec![file("crates/x/src/a.rs", "FOO := once\nBAR := once")];
        assert!(census(&unique).is_empty());
    }

    #[test]
    fn defined_names_extracts_the_lhs() {
        assert_eq!(defined_names("CAP-OUT := ..."), vec!["CAP-OUT".to_string()]);
        assert_eq!(defined_names("  x := y"), vec!["x".to_string()]);
        assert!(defined_names("no assignment here").is_empty());
        assert!(defined_names("a == b").is_empty());
    }

    #[test]
    fn vv_matrix_gate_fires_on_a_landed_star_row_without_proof() {
        let header = "| Item | crate | unit | property | differential | Kani | Lean | validation |\n|---|---|---|---|---|---|---|---|\n";
        // landed ★ row with empty Kani + Lean ⇒ FAIL
        let bad = format!("{header}| foo ★ [M0] | c | ✅ | ✅ | — | ⬜ | ⬜ | — |\n");
        assert_eq!(vv_matrix(&bad).len(), 1);
        // same row, Kani ✅ ⇒ OK
        let ok = format!("{header}| foo ★ [M0] | c | ✅ | ✅ | — | ✅ (harness) | ⬜ | — |\n");
        assert!(vv_matrix(&ok).is_empty());
        // not-yet-landed ★ row is out of scope even with empty cells
        let deferred = format!("{header}| foo ★ [M4] | c | ⬜ | ⬜ | — | ⬜ | ⬜ | — |\n");
        assert!(vv_matrix(&deferred).is_empty());
        // non-★ landed row is out of scope
        let nonstar = format!("{header}| foo [M0] | c | ✅ | — | — | — | — | — |\n");
        assert!(vv_matrix(&nonstar).is_empty());
    }

    #[test]
    fn panic_freedom_requires_a_discharge_tag() {
        // bad: a pure-tier allow for a panic lint with no PANIC-FREEDOM tag
        let bad = vec![file(
            "crates/lattice/src/x.rs",
            "#[allow(clippy::unwrap_used)]\nfn f() {}",
        )];
        assert_eq!(panic_freedom_discharge(&bad).len(), 1);
        // ok: tagged nearby; and a non-pure-tier allow is out of scope
        let clean = vec![
            file(
                "crates/lattice/src/x.rs",
                "// PANIC-FREEDOM: guarded, see docs/trusted-invariants.md\n#[allow(clippy::unwrap_used)]\nfn f() {}",
            ),
            file(
                "crates/geom/src/x.rs",
                "#[allow(clippy::unwrap_used)]\nfn g() {}",
            ),
        ];
        assert!(panic_freedom_discharge(&clean).is_empty());
    }

    #[test]
    fn milestone_tag_parses() {
        assert_eq!(milestone_tag("foo ★ [M3d]").as_deref(), Some("M3d"));
        assert_eq!(milestone_tag("no tag"), None);
    }
}
