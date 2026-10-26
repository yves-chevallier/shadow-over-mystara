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
#
# Reprise : chaque résultat est rangé dans $DDSOM_RESULTATS (~/.cache/ddsom/validate par défaut), sous
# l'empreinte du binaire `validate`, et un scénario déjà VALIDE pour ce binaire, ce fichier et ce nombre
# d'images n'est pas rejoué. Une validation interrompue se relance donc sans perdre ce qui est fait. Le binaire
# est copié sous son empreinte : on peut reconstruire pendant qu'une validation tourne.
set -euo pipefail
cd "$(dirname "$0")/.."
frames=${1:-30000}
# Depuis un worktree : DDSOM_ROMS et DDSOM_EEPROM pointent vers le dépôt principal.
roms=${DDSOM_ROMS:-rom}
eeprom=${DDSOM_EEPROM:-testdata/eeprom_ddsom.bin}
jobs=${DDSOM_JOBS:-$(nproc)}
cargo build --release -q --manifest-path port/Cargo.toml -p ddsom-game --examples
bin=port/target/release/examples/validate
# empreinte : le binaire et les variables DDSOM_* qui changent son comportement (pas celles de ce script)
reglages=$(env | grep '^DDSOM_' | grep -vE '^DDSOM_(JOBS|RESULTATS|SCENES|COV|ROMS|EEPROM)=' | sort)
empreinte=$({ sha256sum < "$bin"; echo "$reglages"; } | sha256sum | cut -c1-16)
resultats=${DDSOM_RESULTATS:-$HOME/.cache/ddsom/validate}/$empreinte
mkdir -p "$resultats"
[[ -x $resultats/validate ]] || { cp "$bin" "$resultats/validate.tmp" && mv "$resultats/validate.tmp" "$resultats/validate"; }
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

export roms eeprom out resultats
# un scénario : résultat repris s'il est déjà VALIDE, sinon rejoué (journal écrit à part puis renommé)
xargs -P "$jobs" -L 1 sh -c '
	nom=$(basename "$0" .txt)
	cle="$resultats/$nom-$(sha256sum "$0" | cut -c1-16)-$1.log"
	if ! { [ -f "$cle" ] && tail -1 "$cle" | grep -q "^VALIDE"; }; then
		"$resultats/validate" "$roms" "$eeprom" "$0" "$1" > "$cle.$$" 2>&1
		mv "$cle.$$" "$cle"
	fi
	cp "$cle" "$out/$nom.log"' \
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
