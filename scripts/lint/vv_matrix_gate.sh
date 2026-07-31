#!/usr/bin/env bash
# The milestone gate (vv-guide §6/§8; spec invariant 2). CI fails if a
# soundness-critical row (marked ★) whose milestone has LANDED has an empty
# {Kani ∨ Lean ∨ runtime-checked-hypothesis} cell — i.e. a shipped soundness-
# critical operation with no proof. Not-yet-landed ★ rows are out of scope until
# their milestone ships (so the deferred M3d/M3e/… ★ rows do not trip the gate).
#
# A ★ row is SATISFIED when its Kani or Lean cell holds a ✅, or an "rc-hyp ✅"
# marker appears (the runtime-checked-hypothesis disjunct, tracked in the ledger).
set -eu
root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
matrix="$root/vv-matrix.md"

# Landed milestones — extend as each ships.
landed="M0 M3a M3c"

awk -v landed="$landed" '
  BEGIN { n = split(landed, a, " "); for (i = 1; i <= n; i++) L[a[i]] = 1; fail = 0 }
  /^\|/ {
    if ($0 ~ /^\|[- :|]*$/) next;      # separator row
    item = $2;
    if (item !~ /★/) next;             # only soundness-critical rows
    ms = "";
    if (match(item, /\[M[0-9A-Za-z]+\]/)) ms = substr(item, RSTART + 1, RLENGTH - 2);
    if (!(ms in L)) next;              # milestone not landed → not gated yet
    kani = $7; lean = $8;
    ok = (kani ~ /✅/) || (lean ~ /✅/) || ($0 ~ /rc-hyp ✅/);
    if (!ok) { printf("vv-matrix gate: FAIL — landed ★ row lacks {Kani ∨ Lean ∨ rc-hyp}:%s\n", item); fail = 1 }
  }
  END { if (fail) exit 1 }
' "$matrix" || exit 1

echo "vv-matrix gate: OK (landed soundness-critical rows [$landed] all carry a proof cell)."
