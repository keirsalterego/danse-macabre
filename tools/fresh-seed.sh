#!/usr/bin/env bash
# Holdout insurance: generate an unseen full seed, run the organizers' reference on it, and check
# that our binary writes the same bytes. Final scoring uses a seed nobody has seen, so passing
# seeds 1-4 is not enough on its own.
# Usage: fresh-seed.sh <a|b|c|d> <seed> <binary>     (binary takes <data_dir> <out_dir>)
set -euo pipefail
declare -A MOD=([a]=a_ingest [b]=b_index [c]=c_join [d]=d_traverse)
H=~/speed-demon/harness-repo
t=$1 s=$2 bin=$(realpath "$3") m=${MOD[$1]}
d=~/speed-demon/data/$t/full-seed$s
if [ ! -s "$d.sha256" ]; then
  python3 -I "$H/generators/$m.py" --seed "$s" --scale full --out "$d" >/dev/null
  rm -rf "$d.ref" && mkdir -p "$d.ref"
  python3 -I "$H/reference/$m.py" --data "$d" --out "$d.ref" >/dev/null
  sha256sum "$d.ref"/* | cut -d' ' -f1 >"$d.sha256"
fi
out=$(mktemp -d) && trap 'rm -rf "$out"' EXIT
"$bin" "$d" "$out"
if [ "$(sha256sum "$out"/* | cut -d' ' -f1)" = "$(cat "$d.sha256")" ]; then
  echo "fresh seed $s ($t): PASS"
else
  echo "fresh seed $s ($t): FAIL; first differing lines:"
  diff <(cat "$d.ref"/*) <(cat "$out"/*) | head -20
  exit 1
fi
