#!/usr/bin/env bash
# Inventaire du 68000 restant : tools/reste68k.sh [images]  → docs/reste68k.md
#
# Scénarios : ceux de validate_all.sh (scenarios/, extra, boss, enemy, modes ; scene et scene_end avec
# DDSOM_SCENES=1) et scenarios/cov/ (toujours). `images` = images des scénarios de premier niveau et
# de cov (30000 par défaut). Les comptes bruts vont dans $DDSOM_CNT (répertoire temporaire par défaut).
set -euo pipefail
cd "$(dirname "$0")/.."
frames=${1:-30000}
roms=${DDSOM_ROMS:-rom}
eeprom=${DDSOM_EEPROM:-testdata/eeprom_ddsom.bin}
decomp=${DDSOM_DECOMP:-/home/ycr/som/decomp/index.tsv}
jobs=${DDSOM_JOBS:-$(nproc)}
if [[ -z ${DDSOM_BIN:-} ]]; then
	cargo build --release -q --manifest-path port/Cargo.toml -p ddsom-game --example reste68k
fi
# DDSOM_BIN : exécutable figé (pour reconstruire pendant la mesure)
bin=${DDSOM_BIN:-port/target/release/examples/reste68k}
out=${DDSOM_CNT:-$(mktemp -d)}
mkdir -p "$out"

list() {
	local dir=$1 n=$2 s
	for s in "$dir"/*.txt; do
		[[ -e $s ]] && echo "$s $n"
	done
	return 0
}
# DDSOM_LISTE : fichier « scénario images » à utiliser à la place de la liste complète
if [[ -n ${DDSOM_LISTE:-} ]]; then
	cp "$DDSOM_LISTE" "$out/liste"
else {
	list scenarios "$frames"
	list scenarios/extra 12000
	list scenarios/boss 12000
	list scenarios/enemy 12000
	list scenarios/modes 12000
	if [[ ${DDSOM_SCENES:-0} == 1 ]]; then
		list scenarios/scene 6500
		list scenarios/scene_end 9000
	fi
	list scenarios/cov "$frames"
} > "$out/liste"
fi

export bin roms eeprom out
xargs -P "$jobs" -L 1 sh -c \
	'[ -s "$out/$(echo "$0" | tr / _).cnt" ] || "$bin" run "$roms" "$eeprom" "$0" "$1" "$out/$(echo "$0" | tr / _).cnt"' \
	< "$out/liste"
"$bin" report "$decomp" docs/atlas.md docs/reste68k.md "$out"/*.cnt
