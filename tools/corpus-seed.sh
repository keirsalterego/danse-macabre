#!/usr/bin/env bash
# One tiny-seed oracle case: the organizers' generator writes the input, the organizers'
# reference writes the expected output, and its sha256 goes next to it.
# Usage: corpus-seed.sh <a|b|c|d> <seed>      (fan out with: seq 1 300 | xargs -P 8 -n1 corpus-seed.sh a)
set -euo pipefail
declare -A MOD=([a]=a_ingest [b]=b_index [c]=c_join [d]=d_traverse)
H=~/speed-demon/harness-repo
t=$1 s=$2 m=${MOD[$1]}
d=~/speed-demon/data/$t/tiny/seed$s
[ -s "$d/ref.sha256" ] && exit 0   # already built; makes reruns cheap
rm -rf "$d" && mkdir -p "$d/out"
python3 -I "$H/generators/$m.py" --seed "$s" --scale tiny --out "$d/in" >/dev/null
python3 -I "$H/reference/$m.py" --data "$d/in" --out "$d/out"
sha256sum "$d/out"/* | cut -d' ' -f1 >"$d/ref.sha256"
