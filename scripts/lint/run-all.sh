#!/usr/bin/env bash
# Runs the spec/code invariant lints CI gates on from the skeleton
# (vv-guide §6; spec §8.2). Any nonzero exit fails the gate.
set -eu
here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
rc=0
bash "$here/no_float_certified.sh" || rc=1
bash "$here/tuple_predicate.sh" || rc=1
bash "$here/census.sh" || rc=1
exit "$rc"
