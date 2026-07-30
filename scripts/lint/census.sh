#!/usr/bin/env bash
# := census (spec §8.2 commit protocol): every defined name defines exactly once
# per commit — composites in the gate section, certificates in §8.5, geometry in
# its home section; a same-commit twin definition is a lint failure.
#
# STARTER SCOPE: Rust source (a duplicated `NAME :=` in code). Extending this to
# spec/ needs the section-scoping rules (which `:=` belongs to which home) plus
# an allow-list for the spec's own meta-discussion of the census; that lands
# with the next spec edit. Kept green-by-construction until then.
set -eu
root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
dups="$(grep -rhoE '[A-Za-z_][A-Za-z0-9_()-]*[[:space:]]*:=' "$root/crates" 2>/dev/null \
  | sed -E 's/[[:space:]]*:=$//' \
  | sort | uniq -d || true)"
if [ -n "$dups" ]; then
  printf ':= census: FAIL — names defined more than once:\n%s\n' "$dups"
  exit 1
fi
echo ":= census: OK (code; spec-text scoping is a TODO)."
