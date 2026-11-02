#!/bin/sh
# Spawns by hand object types that the playthroughs never meet (see `creatures spawn`).
#   DDSOM_ROMS=rom tools/creatures_spawn.sh <scene> <enemy|boss> <type> <variant> [<scene> <family> <type> <variant> ...]
# At most 4 processes in parallel; the samples go to assets/creatures/cache.
set -e${CREATURES_TRACE:+x}
ROOT=$(cd "$(dirname "$0")/.." && pwd)
ROMS=${DDSOM_ROMS:-$ROOT/rom}
EEPROM=${DDSOM_EEPROM:-$ROOT/testdata/eeprom_ddsom.bin}
OUT=${DDSOM_CREATURES_OUT:-$ROOT/assets/creatures}
BIN=$ROOT/port/target/release/examples/creatures
mkdir -p "$OUT/cache"
n=0
while [ $# -ge 4 ]; do
  "$BIN" spawn "$ROMS" "$EEPROM" "$OUT/cache" "$1" "$2" "$3" "$4" &
  shift 4
  n=$((n + 1))
  if [ $((n % 4)) -eq 0 ]; then wait; fi
done
wait
