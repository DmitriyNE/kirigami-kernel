#!/usr/bin/env bash
#
# The `#print axioms` gate: every theorem a `certify-core` checker leans on must
# have a KNOWN axiom footprint.
#
# Clean is `[propext, Classical.choice, Quot.sound]` — Lean's own three. Anything
# else is either a `sorryAx` (an unfinished proof) or a named `axiom` (a trust
# hole). Both must fail.
#
# ONE kind of exception is legitimate, and it is PER-THEOREM rather than global:
# the runtime-checked-hypothesis pattern (`docs/vv-guide.md §0`) cites a theorem
# the literature has but Mathlib does not, as a single labelled `axiom` — the 📌
# rows of `docs/proofs/ledger.md`. Today that is exactly one: `sturm_root_count`
# on `verify_chain_sound`.
#
# Why per-theorem: a global allowlist would say "this axiom is fine anywhere",
# so the same name appearing under an unrelated proof — the actual leak this
# gate exists to catch — would pass silently. Declaring the citation against its
# one theorem keeps the footprint of every OTHER theorem strictly clean.
#
# The check is two-sided on purpose. A declared citation that STOPS appearing is
# also an error: it means the obligation was discharged and the ledger's 📌 row
# is now stale, which is a docs update we want to be told about rather than
# discover months later.
#
# Assumes elan/lake on PATH (the nix devShell); does the `lake` build itself so
# the whole Lean gate is one command locally as well as in CI.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null \
  || { cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd; })"

# One entry per audited theorem: `<name>` for a clean footprint, or
# `<name> <cited-axiom>` for a 📌 obligation. Keep in step with
# `docs/proofs/ledger.md` — that table is the human-readable half of this list.
THEOREMS='
signVariationsImp_eq_signVariations
verify_chain_sound sturm_root_count
lifted_sign_variations_eq_loop
sign_variations_spec
sliceIter_next_spec
gcd_u128_spec
reduce_spec
verify_common_factor_sound
link_ok_rejects_pinch
v_boundary_iff_boundary
v_boundary_imp_link_ok
cyclic_true_runs_loop_spec
cyclic_true_runs_spec
link_ok_spec
ClipSigma.clip_sigma_branch_eq
ClipSigma.corner_range_eq
ClipSigma.clipSigma_sound_positive
ClipSigma.clipSigma_sound_negative
ClipSigma.clipSigma_rejects_straddle
RefBackend.is_zero_eq
RefBackend.cmp_eq
RefBackend.add_eq
RefBackend.sub_eq
RefBackend.mul_eq
RefBackend.shl1_eq
RefBackend.testbit_eq
RefBackend.bit_len_spec
RefBackend.divrem_eq
RefBackend.gcd_eq
RefBackend.refBackend_eq_ZQ
'

cd "$ROOT/certify-check"
# `git`-in-lake needs these unset under the darwin C toolchain
# (docs/spike-extraction-report.md §3).
unset DEVELOPER_DIR SDKROOT

if [ "${SKIP_LAKE_BUILD:-0}" != "1" ]; then
  lake exe cache get
  lake build
fi

tmpdir="$(mktemp -d)"
audit_lean="$tmpdir/axiom_audit.lean"
audit_out="$tmpdir/axiom_audit.out"
trap 'rm -rf "$tmpdir"' EXIT

# Drop the blank lines the quoted list is padded with, so every loop below sees
# only real entries (a trailing empty line would otherwise make a `while` exit
# non-zero and, under `pipefail`, kill the script before it can say why).
THEOREMS="$(echo "$THEOREMS" | grep '[^[:space:]]')"

{
  echo 'import CertifyCheck'
  echo 'open CertifyCheck'
  echo "$THEOREMS" | while read -r name _cited; do
    echo "#print axioms $name"
  done
} > "$audit_lean"

# Capture the exit status FIRST: an audit file that does not COMPILE (a renamed
# or deleted theorem) must fail here, not slip through because its error output
# happens to lack a forbidden axiom name.
if ! lake env lean "$audit_lean" > "$audit_out" 2>&1; then
  echo "AXIOM AUDIT FAILED: the audit file did not compile" >&2
  cat "$audit_out" >&2
  exit 1
fi

# `#print axioms` wraps long footprints across lines, so join each record from
# the line naming the theorem through the line closing its `[...]`. Emitting one
# `name|axiom,axiom,...` record per theorem is what makes the per-theorem rule
# below expressible at all — the old audit flattened every footprint into a
# single anonymous stream, which is why it could only ever be a global grep AND
# why a wrapped continuation line escaped it entirely.
# `q` carries a literal single quote (octal 47) so the awk program itself needs
# none — macOS's awk is BWK awk, which has no `\x` escape.
records="$(awk -v q="$(printf '\047')" '
  {
    if (acc)                              { rec = rec " " $0 }
    else if ($0 ~ /depends on axioms:/)   { rec = $0; acc = 1 }
    if (acc && rec ~ /\]/) {
      name = rec; sub(/ depends on axioms:.*$/, "", name); gsub(q, "", name)
      ax = rec; sub(/^.*axioms:[[:space:]]*\[/, "", ax); sub(/\].*$/, "", ax)
      gsub(/[[:space:]]/, "", ax)
      print name "|" ax
      acc = 0; rec = ""
    }
  }
' "$audit_out")"

want=$(echo "$THEOREMS" | wc -l | tr -d ' ')
got=$(echo "$records" | grep -c '[^[:space:]]' || true)
if [ "$want" != "$got" ]; then
  echo "AXIOM AUDIT FAILED: expected $want footprints, parsed $got" >&2
  cat "$audit_out" >&2
  exit 1
fi

status=0
echo "$THEOREMS" | while read -r name cited; do
  rec="$(echo "$records" | grep -E "(^|\.)${name//./\\.}\|" || true)"
  if [ -z "$rec" ]; then
    echo "AXIOM AUDIT FAILED: no footprint parsed for '$name'" >&2
    exit 1
  fi
  # Everything beyond Lean's own three axioms.
  extra="$(echo "${rec#*|}" | tr ',' '\n' \
    | grep -vE '^(propext|Classical\.choice|Quot\.sound)$' | grep -v '^$' || true)"
  expect="${cited:-}"
  actual="$(echo "$extra" | tr '\n' ' ' | sed 's/ *$//')"
  if [ "$actual" != "$expect" ]; then
    if [ -z "$expect" ]; then
      echo "AXIOM AUDIT FAILED: '$name' depends on non-allowlisted axiom(s): $actual" >&2
    elif [ -z "$actual" ]; then
      echo "AXIOM AUDIT FAILED: '$name' no longer needs its cited axiom '$expect'." >&2
      echo "  The obligation was discharged — promote its row in docs/proofs/ledger.md" >&2
      echo "  from 📌 to ✅ and drop the citation from THEOREMS in this script." >&2
    else
      echo "AXIOM AUDIT FAILED: '$name' cites '$expect' but depends on: $actual" >&2
    fi
    exit 1
  fi
  if [ -n "$expect" ]; then
    echo "  📌 $name — cited axiom '$expect' (docs/proofs/ledger.md)"
  fi
done || status=1

[ "$status" = 0 ] || exit 1
echo "axiom audit OK: $want theorems, footprints as declared"
