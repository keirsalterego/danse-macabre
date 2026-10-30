#!/usr/bin/env bash
# Correctness gate: run a track binary over full seeds 1-4 and every tiny seed, and compare the
# sha256 of its one output file with the organizers' reference output.
# Usage: gate.sh <a|b|c|d> <binary>     The binary is called as: <binary> <data_dir> <out_dir>
# Expected hashes: data/<t>/full-seed<n>.sha256 (from the reference runs) and data/<t>/tiny/seed<n>/ref.sha256.
set -uo pipefail
t=$1 bin=$(realpath "$2") data=~/speed-demon/data/$1
work=$(mktemp -d) && trap 'rm -rf "$work"' EXIT
pass=0 fail=0

check() { # <data_dir> <expected_sha256>
  rm -rf "$work/out" && mkdir "$work/out"
  if ! "$bin" "$1" "$work/out" 2>"$work/err"; then
    echo "FAIL exit: $1"; sed 's/^/  /' "$work/err" | head -5; fail=$((fail + 1)); return
  fi
  local files=("$work/out"/*)
  if [ "${#files[@]}" -ne 1 ]; then echo "FAIL $1: ${#files[@]} files in out"; fail=$((fail + 1)); return; fi
  if [ "$(sha256sum "${files[0]}" | cut -d' ' -f1)" != "$2" ]; then
    echo "FAIL hash: $1"; fail=$((fail + 1)); return
  fi
  pass=$((pass + 1))
}

start=$(date +%s.%N)
for f in "$data"/full-seed[0-9]*.sha256; do check "${f%.sha256}" "$(cat "$f")"; done
for d in "$data"/tiny/seed*/; do check "$d/in" "$(cat "$d/ref.sha256")"; done
printf 'gate %s: %d pass, %d fail, %.1f s\n' "$t" "$pass" "$fail" "$(echo "$(date +%s.%N) - $start" | bc)"
[ "$fail" -eq 0 ] && [ "$pass" -gt 0 ]
