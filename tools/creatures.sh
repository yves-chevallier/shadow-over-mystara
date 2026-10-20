#!/bin/sh
# Génère les images des créatures dans assets/creatures/ (dossier ignoré par git : images tirées des ROMs).
#   DDSOM_ROMS=rom tools/creatures.sh [scan|spawn|build]     (sans argument : les trois étapes)
# scan : rejoue les scénarios de couverture et échantillonne les créatures rencontrées ;
# spawn : fait apparaître à la main les types que ces scénarios ne rencontrent jamais ;
# build : choisit les poses, écrit les PNG, les planches et docs/creatures.md. Voir docs/creatures.md.
# Au plus 4 processus en parallèle.
set -e
ROOT=$(cd "$(dirname "$0")/.." && pwd)
ROMS=${DDSOM_ROMS:-$ROOT/rom}
EEPROM=${DDSOM_EEPROM:-$ROOT/testdata/eeprom_ddsom.bin}
OUT=${DDSOM_CREATURES_OUT:-$ROOT/assets/creatures}
BIN=$ROOT/port/target/release/examples/creatures
FRAMES=${CREATURES_FRAMES:-30000}
SCENARIOS="cov_tour1_c0 cov_tour2_c1 cov_tour3_c2 cov_tour4_c3 cov_tour5_c4 cov_tour6_c5 cov_tour7_c0 cov_tour8_c1 cov_natural_c0 ${CREATURES_EXTRA:-}"
# scène famille type variante : types jamais vus en partie (la palette est celle de la scène choisie)
SPAWNS=${CREATURES_SPAWNS:-"1 enemy 3 0  1 enemy 11 0  1 enemy 13 0  1 enemy 14 0  1 enemy 15 0  1 enemy 27 0  64 enemy 26 0  66 enemy 28 0  42 boss 11 0"}
STEP=${1:-all}

(cd "$ROOT/port" && cargo build --release -p ddsom-game --example creatures)
mkdir -p "$OUT/cache"

if [ "$STEP" = all ] || [ "$STEP" = scan ]; then
  n=0
  for s in $SCENARIOS; do
    "$BIN" scan "$ROMS" "$EEPROM" "$ROOT/scenarios/cov/$s.txt" "$FRAMES" "$OUT/cache" "$s" &
    n=$((n + 1))
    if [ $((n % 4)) -eq 0 ]; then wait; fi
  done
  wait
fi
if [ "$STEP" = all ] || [ "$STEP" = spawn ]; then
  # shellcheck disable=SC2086
  DDSOM_CREATURES_OUT=$OUT "$ROOT/tools/creatures_spawn.sh" $SPAWNS
fi
if [ "$STEP" = all ] || [ "$STEP" = build ]; then
  "$BIN" build "$OUT/cache" "$OUT" "$ROOT/docs/creatures.md"
fi
