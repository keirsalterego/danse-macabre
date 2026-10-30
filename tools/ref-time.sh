#!/usr/bin/env bash
# Time an image on full seed 1 with the official harness (sd-timer, memory.peak), turbo off, one
# track at a time: the organizers' reference by default, or our track folder when a context is given.
# Needs root for the cgroup reads, so it runs under sudo.
# Usage: sudo tools/ref-time.sh [tracks=abcd] [runs=5] [context dir, e.g. repo/track-a]
set -euo pipefail
H=/home/manish/speed-demon/harness-repo
DATA=/home/manish/speed-demon/data
# The box keeps /out on a tmpfs (setup-box.sh), so output pages are shmem charged to M and never
# reach a disk. /tmp is a tmpfs here too; an ext4 work dir would time the output path wrongly.
WORK=/tmp/speed-demon-work
CTX=${3:-$H/reference} TAG=speeddemon/reference:2026
[ -n "${3:-}" ] && TAG=speeddemon/ours-$(basename "$3"):latest
declare -A OUT=([a]=summary.txt [b]=results.txt [c]=aggregates.txt [d]=distances.txt)
declare -A TIMEOUT=([a]=320.5 [b]=563.5 [c]=144.4 [d]=726.9)
mkdir -p "$WORK"
turbo=/sys/devices/system/cpu/intel_pstate/no_turbo
old=$(cat $turbo) && echo 1 >$turbo && trap 'echo "$old" >$turbo' EXIT
for t in $(echo "${1:-abcd}" | grep -o .); do
  python3 "$H/harness/run_submission.py" --context "$CTX" --track "$t" \
    --data "$DATA/$t/full-seed1" --output "${OUT[$t]}" --timeout-s "${TIMEOUT[$t]}" \
    --timer "$H/harness/timer/sd-timer" --work "$WORK" --tag "$TAG" \
    --runs "${2:-5}" --results "$WORK/ref-$t.json"
  python3 -c "import json,sys; r=json.load(open(sys.argv[1])); b=r['batches'][-1]
print(sys.argv[2], 'T', r.get('T_s'), 'M', r.get('M_bytes'), 'gates', r['gates'], 'sha', sorted({x.get('sha256') for x in b}), 'walls', [x.get('wall_s') for x in b])" "$WORK/ref-$t.json" "$t"
done
