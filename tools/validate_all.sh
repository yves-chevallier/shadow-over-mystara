#!/usr/bin/env bash
# Validates the rewrite on all scenarios, frame by frame: tools/validate_all.sh [frames]
#
# Scenario sets (number of frames):
#   scenarios/*.txt         bots of the six characters and two-player bots ([frames], 30000 by default)
#   scenarios/extra/        random inputs (make_bot.py --mash), reveal rare cases (12000)
#   scenarios/boss/         direct entry into a boss scene (gen_boss.py) (12000)
#   scenarios/enemy/        direct entry into a scene where an enemy type appears (gen_enemy.py) (12000)
#   scenarios/modes/        attract, game over and ranking, end credits (mode machine, docs/modes.md) (12000)
#   scenarios/scene/        scene entry, DDSOM_SCENES=1 (6500)
#   scenarios/scene_end/    scene exit, DDSOM_SCENES=1 (9000)
#   scenarios/cov/          coverage of the whole game, DDSOM_COV=1 (COV_FRAMES, 30000 by default)
#   scenarios/long/         long bots (autobot, several players, routes, shops), DDSOM_LONG=1 (each with its own
#                           number of frames, in <name>.images; ~3 min each)
# At most DDSOM_JOBS simultaneous validations (default: number of cores). The validated game is the game without the
# 68000 (the hybrid mode, DDSOM_HYBRIDE=1, is gone).
#
# Resume: each result is stored in $DDSOM_RESULTATS (~/.cache/ddsom/validate by default), under the
# hash of the `validate` binary, and a scenario already VALID for this binary, this file and this number
# of frames is not replayed. An interrupted validation can thus be restarted without losing what is done. The binary
# is copied under its hash: one can rebuild while a validation runs.
set -euo pipefail
cd "$(dirname "$0")/.."
frames=${1:-30000}
# From a worktree: DDSOM_ROMS and DDSOM_EEPROM point to the main repository.
roms=${DDSOM_ROMS:-rom}
eeprom=${DDSOM_EEPROM:-testdata/eeprom_ddsom.bin}
jobs=${DDSOM_JOBS:-$(nproc)}
cargo build --release -q --manifest-path port/Cargo.toml -p ddsom-game --examples
bin=port/target/release/examples/validate
# hash: the binary and the DDSOM_* variables that change its behaviour (not those of this script)
settings=$(env | grep '^DDSOM_' | grep -vE '^DDSOM_(JOBS|RESULTATS|SCENES|COV|LONG|ROMS|EEPROM)=' | sort || true)
hash=$({ sha256sum < "$bin"; echo "$settings"; } | sha256sum | cut -c1-16)
results=${DDSOM_RESULTATS:-$HOME/.cache/ddsom/validate}/$hash
mkdir -p "$results"
[[ -x $results/validate ]] || { cp "$bin" "$results/validate.tmp" && mv "$results/validate.tmp" "$results/validate"; }
out=$(mktemp -d)

# "scenario frames" list
list() {
	local dir=$1 n=$2 s
	for s in "$dir"/*.txt; do
		[[ -e $s ]] && echo "$s $n"
	done
	return 0
}
# same, with a number of frames specific to each scenario (file <name>.images next to it)
list_own() {
	local dir=$1 s
	for s in "$dir"/*.txt; do
		[[ -e $s ]] && echo "$s $(cat "${s%.txt}.images")"
	done
	return 0
}
{
	# the longest first: they set the total duration
	if [[ ${DDSOM_LONG:-0} == 1 ]]; then
		list_own scenarios/long
	fi
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
} > "$out/list"

export roms eeprom out results
# one scenario: result reused if already VALID, otherwise replayed (log written aside then renamed)
xargs -P "$jobs" -L 1 sh -c '
	name=$(basename "$0" .txt)
	key="$results/$name-$(sha256sum "$0" | cut -c1-16)-$1.log"
	if ! { [ -f "$key" ] && tail -1 "$key" | grep -q "^VALID"; }; then
		"$results/validate" "$roms" "$eeprom" "$0" "$1" > "$key.$$" 2>&1
		mv "$key.$$" "$key"
	fi
	cp "$key" "$out/$name.log"' \
	< "$out/list" || true

status=0
for log in "$out"/*.log; do
	name=$(basename "$log" .log)
	verdict=$(tail -1 "$log")
	printf '%-20s %s\n' "$name" "$verdict"
	[[ $verdict == VALID* ]] || status=1
done
echo "--- calls per routine (total) ---"
grep -h -E '^  \S+ +[0-9]+ (appels|calls)' "$out"/*.log | awk '{c[$1]+=$2} END {for (k in c) printf "  %-28s %8d calls\n", k, c[k]}' | sort
rm -rf "$out"
exit $status
