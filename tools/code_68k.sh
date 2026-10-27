#!/usr/bin/env bash
# Mesure le code 68000 exécuté par la machine de référence sur tous les scénarios (mêmes listes et durées que
# tools/validate_all.sh avec DDSOM_SCENES=1 et DDSOM_COV=1) et écrit port/extract/code_68k.txt : les plages d'octets
# d'instructions exécutées, seuls octets de la région programme que `extract` ne range pas dans data/reste.json
# (docs/sans-rom.md « Couverture complète des données »).
#
#   DDSOM_ROMS=<roms> DDSOM_EEPROM=<eeprom> DDSOM_JOBS=10 tools/code_68k.sh [dossier de travail]
#
# Le dossier de travail (par défaut ~/.cache/ddsom/code_68k) garde la mesure de chaque scénario : une mesure
# interrompue se reprend. Le résultat est l'union de toutes les mesures du dossier.
set -euo pipefail
cd "$(dirname "$0")/.."
roms=${DDSOM_ROMS:-rom}
eeprom=${DDSOM_EEPROM:-testdata/eeprom_ddsom.bin}
jobs=${DDSOM_JOBS:-$(nproc)}
travail=${1:-$HOME/.cache/ddsom/code_68k}
mkdir -p "$travail"
DDSOM_ROMS=$roms cargo build --release -q --manifest-path port/Cargo.toml -p ddsom-game --example code_execute
bin=port/target/release/examples/code_execute
list() {
	local dir=$1 n=$2 s
	for s in "$dir"/*.txt; do
		[[ -e $s ]] && echo "$s $n"
	done
	return 0
}
{
	list scenarios 30000
	list scenarios/extra 12000
	list scenarios/boss 12000
	list scenarios/enemy 12000
	list scenarios/modes 12000
	list scenarios/scene 6500
	list scenarios/scene_end 9000
	list scenarios/cov "${COV_FRAMES:-30000}"
} > "$travail/liste"
export roms eeprom travail bin
xargs -P "$jobs" -L 1 sh -c '
	cle="$travail/$(basename "$(dirname "$0")")_$(basename "$0" .txt)-$(sha256sum "$0" | cut -c1-16)-$1.code"
	[ -f "$cle" ] || { "$bin" "$roms" "$eeprom" "$cle.$$" "$0:$1" 2> /dev/null && mv "$cle.$$" "$cle"; }' \
	< "$travail/liste"
python3 - "$travail" > port/extract/code_68k.txt <<'EOF'
import glob, sys
octets = bytearray(0x400000)
for f in glob.glob(sys.argv[1] + '/*.code'):
    for l in open(f):
        a, b = (int(v, 16) for v in l.split())
        octets[a:b] = b'\1' * (b - a)
print('# début fin(exclue) : octets d\'instructions 68000 exécutées par la référence sur tous les scénarios')
print('# généré par tools/code_68k.sh, ne pas éditer (docs/sans-rom.md « Couverture complète des données »)')
a = None
for i, x in enumerate(bytes(octets) + b'\0'):
    if x and a is None:
        a = i
    elif not x and a is not None:
        print(f'{a:06x} {i:06x}')
        a = None
EOF
echo "port/extract/code_68k.txt : $(grep -vc '^#' port/extract/code_68k.txt) plages"
