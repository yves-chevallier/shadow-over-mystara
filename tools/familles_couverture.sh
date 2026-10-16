#!/usr/bin/env bash
# Combien de fois chaque gestionnaire ESHL / SET / SOBJ est appelé par les scénarios (code d'origine
# seul) : tools/familles_couverture.sh [images] > couverture.txt
# Sortie : « adresse appels » par gestionnaire (cumul des 8 scénarios) ; à rapprocher de
# `python3 tools/familles_tables.py` (adresse -> type).
set -euo pipefail
cd "$(dirname "$0")/.."
frames=${1:-30000}
roms=${DDSOM_ROMS:-rom}
eeprom=${DDSOM_EEPROM:-testdata/eeprom_ddsom.bin}
cargo build --release -q --manifest-path port/Cargo.toml -p ddsom-game --examples
out=$(mktemp -d)
(cd "$out" && python3 "$OLDPWD/tools/familles_tables.py" tsv > /dev/null)
for s in scenarios/*.txt; do
	./port/target/release/examples/couverture "$roms" "$eeprom" "$s" "$frames" "$out/handlers.tsv" > "$out/$(basename "$s" .txt).cov" &
done
wait
cat "$out"/*.cov | awk '{c[$1]+=$2} END {for (k in c) print k, c[k]}' | sort
rm -rf "$out"
