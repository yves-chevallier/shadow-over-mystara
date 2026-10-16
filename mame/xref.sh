#!/usr/bin/env bash
# Références croisées dynamiques sur une plage RAM/E/S pendant un scénario scripté.
# usage : mame/xref.sh scenario.txt debut fin sortie.txt   (adresses en hex, ex. ff85e6 ff85e9)
# La couverture (mame/coverage.bin) est mise à jour au passage.
set -euo pipefail
scenario=$1 start=$2 end=$3 out=$4
SCENARIO="$scenario" XREF="$start $end" XREF_OUT="$out" \
	mame ddsom -rompath /home/ycr/som -video none -sound none -nothrottle -skip_gameinfo \
	-autoboot_script /home/ycr/som/mame/session.lua > /dev/null 2>&1
sort -k4 -n -r "$out"
