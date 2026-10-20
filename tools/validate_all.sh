#!/usr/bin/env bash
# Valide la réécriture sur tous les scénarios, image par image : tools/validate_all.sh [images]
#
# Jeux de scénarios (nombre d'images) :
#   scenarios/*.txt         bots des six personnages et à deux joueurs ([images], 30000 par défaut)
#   scenarios/extra/        entrées aléatoires (make_bot.py --mash), révèlent les cas rares (12000)
#   scenarios/boss/         entrée directe dans la scène d'un boss (gen_boss.py) (12000)
#   scenarios/enemy/        entrée directe dans une scène où un type d'ennemi apparaît (gen_enemy.py) (12000)
#   scenarios/modes/        attract, fin de partie et classement, générique de fin (machine à modes, docs/modes.md) (12000)
#   scenarios/scene/        entrée de scène, DDSOM_SCENES=1 (6500)
#   scenarios/scene_end/    sortie de scène, DDSOM_SCENES=1 (9000)
#   scenarios/cov/          couverture de tout le jeu, DDSOM_COV=1 (COV_FRAMES, 30000 par défaut)
# Au plus DDSOM_JOBS validations simultanées (par défaut : nombre de cœurs).
set -euo pipefail
cd "$(dirname "$0")/.."
frames=${1:-30000}
# Depuis un worktree : DDSOM_ROMS et DDSOM_EEPROM pointent vers le dépôt principal.
roms=${DDSOM_ROMS:-rom}
eeprom=${DDSOM_EEPROM:-testdata/eeprom_ddsom.bin}
jobs=${DDSOM_JOBS:-$(nproc)}
cargo build --release -q --manifest-path port/Cargo.toml -p ddsom-game --examples
out=$(mktemp -d)

# liste « scénario images »
list() {
	local dir=$1 n=$2 s
	for s in "$dir"/*.txt; do
		[[ -e $s ]] && echo "$s $n"
	done
	return 0
}
{
	list scenarios "$frames"
	list scenarios/extra 12000
	list scenarios/boss 12000
	list scenarios/enemy 12000
	list scenarios/modes 12000
	if [[ ${DDSOM_SCENES:-0} == 1 ]]; then
		list scenarios/scene 6500
		list scenarios/scene_end 9000
	fi
	if [[ ${DDSOM_COV:-0} == 1 ]]; then
		list scenarios/cov "${COV_FRAMES:-30000}"
	fi
} > "$out/liste"

export roms eeprom out
xargs -P "$jobs" -L 1 sh -c \
	'./port/target/release/examples/validate "$roms" "$eeprom" "$0" "$1" > "$out/$(basename "$0" .txt).log" 2>&1' \
	< "$out/liste" || true

status=0
for log in "$out"/*.log; do
	name=$(basename "$log" .log)
	verdict=$(tail -1 "$log")
	printf '%-20s %s\n' "$name" "$verdict"
	[[ $verdict == VALIDE* ]] || status=1
done
echo "--- appels par routine (cumul) ---"
grep -h -E '^  \S+ +[0-9]+ appels' "$out"/*.log | awk '{c[$1]+=$2} END {for (k in c) printf "  %-28s %8d appels\n", k, c[k]}' | sort
rm -rf "$out"
exit $status
