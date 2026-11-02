#!/usr/bin/env bash
# How many times each ESHL / SET / SOBJ handler is called by the scenarios (original code
# only): tools/families_coverage.sh [frames] > coverage.txt
# Output: "address calls" per handler (sum over the 8 scenarios); match it against
# `python3 tools/families_tables.py` (address -> type).
set -euo pipefail
cd "$(dirname "$0")/.."
frames=${1:-30000}
roms=${DDSOM_ROMS:-rom}
eeprom=${DDSOM_EEPROM:-testdata/eeprom_ddsom.bin}
cargo build --release -q --manifest-path port/Cargo.toml -p ddsom-game --examples
out=$(mktemp -d)
(cd "$out" && python3 "$OLDPWD/tools/families_tables.py" tsv > /dev/null)
for s in scenarios/*.txt; do
	./port/target/release/examples/couverture "$roms" "$eeprom" "$s" "$frames" "$out/handlers.tsv" > "$out/$(basename "$s" .txt).cov" &
done
wait
cat "$out"/*.cov | awk '{c[$1]+=$2} END {for (k in c) print k, c[k]}' | sort
rm -rf "$out"
