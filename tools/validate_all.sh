#!/usr/bin/env bash
# Valide les routines natives sur tous les scénarios (en parallèle) : tools/validate_all.sh [images]
set -euo pipefail
cd "$(dirname "$0")/.."
frames=${1:-30000}
# Depuis un worktree : DDSOM_ROMS et DDSOM_EEPROM pointent vers le dépôt principal.
roms=${DDSOM_ROMS:-rom}
eeprom=${DDSOM_EEPROM:-testdata/eeprom_ddsom.bin}
cargo build --release -q --manifest-path port/Cargo.toml -p ddsom-game --examples
out=$(mktemp -d)
for s in scenarios/*.txt; do
	./port/target/release/examples/validate "$roms" "$eeprom" "$s" "$frames" > "$out/$(basename "$s" .txt).log" 2>&1 &
done
wait
status=0
for log in "$out"/*.log; do
	name=$(basename "$log" .log)
	verdict=$(tail -1 "$log")
	printf '%-12s %s\n' "$name" "$verdict"
	[[ $verdict == VALIDE* ]] || status=1
done
echo "--- appels par routine (cumul) ---"
grep -h -E '^  \S+ +[0-9]+ appels' "$out"/*.log | awk '{c[$1]+=$2; e[$1]+=$4} END {for (k in c) printf "  %-28s %8d appels %5d écarts\n", k, c[k], e[k]}' | sort
rm -rf "$out"
exit $status
