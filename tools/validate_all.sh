#!/usr/bin/env bash
# Valide la réécriture sur tous les scénarios (en parallèle), image par image : tools/validate_all.sh [images]
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
# entrées aléatoires (make_bot.py --mash) : courts, mais révèlent les cas rares
for s in scenarios/extra/*.txt; do
	./port/target/release/examples/validate "$roms" "$eeprom" "$s" 12000 > "$out/$(basename "$s" .txt).log" 2>&1 &
done
# scénarios des boss (tools/gen_boss.py) : entrée directe dans la scène du boss, qui est combattu sans affaiblissement
for s in scenarios/boss/*.txt; do
	[[ -e $s ]] || continue
	./port/target/release/examples/validate "$roms" "$eeprom" "$s" 12000 > "$out/$(basename "$s" .txt).log" 2>&1 &
done
# scénarios des ennemis (tools/gen_enemy.py) : entrée directe dans une scène où le type apparaît, joueur invincible
for s in scenarios/enemy/*.txt; do
	[[ -e $s ]] || continue
	./port/target/release/examples/validate "$roms" "$eeprom" "$s" 12000 > "$out/$(basename "$s" .txt).log" 2>&1 &
done
# scénarios de couverture de tout le jeu (docs/couverture.md) : longs (jusqu'à 30000 images à 1-4 joueurs, plus d'une
# heure sous charge) ; activés par DDSOM_COV=1, nombre d'images par COV_FRAMES (30000 par défaut)
if [[ ${DDSOM_COV:-0} == 1 ]]; then
	for s in scenarios/cov/*.txt; do
		./port/target/release/examples/validate "$roms" "$eeprom" "$s" "${COV_FRAMES:-30000}" > "$out/$(basename "$s" .txt).log" 2>&1 &
	done
fi
wait
status=0
for log in "$out"/*.log; do
	name=$(basename "$log" .log)
	verdict=$(tail -1 "$log")
	printf '%-12s %s\n' "$name" "$verdict"
	[[ $verdict == VALIDE* ]] || status=1
done
echo "--- appels par routine (cumul) ---"
grep -h -E '^  \S+ +[0-9]+ appels' "$out"/*.log | awk '{c[$1]+=$2} END {for (k in c) printf "  %-28s %8d appels\n", k, c[k]}' | sort
rm -rf "$out"
exit $status
