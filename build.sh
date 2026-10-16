#!/usr/bin/env bash
# Chaîne complète : ROMs -> image programme -> opcodes déchiffrés -> projet Ghidra + image fusionnée + listing.
set -euo pipefail
cd "$(dirname "$0")"
GHIDRA=${GHIDRA:-$HOME/tools/ghidra_12.1.4_PUBLIC}
HEADLESS="$GHIDRA/support/analyzeHeadless"
COV=$( [ -f mame/coverage.bin ] && echo "$PWD/mame/coverage.bin" || true )

mkdir -p rom ghidra_proj
[ -f rom/ddsom.key ] || unzip -oq ddsom.zip -d rom

python3 -I tools/build_prog.py rom prog_plain.bin
cargo build --release -q --manifest-path cps2dec/Cargo.toml
./cps2dec/target/release/cps2dec prog_plain.bin rom/ddsom.key prog_dec.bin

# Fin de la plage chiffrée affichée par cps2dec (0x100000 pour ddsom).
"$HEADLESS" ghidra_proj ddsom -import prog_dec.bin -overwrite \
	-loader BinaryLoader -processor 68000:BE:32:default -noanalysis \
	-scriptPath ghidra_scripts -postScript Cps2Setup.java prog_plain.bin 0x100000 prog_merged.bin $COV \
	> ghidra_run.log 2>&1
grep -E 'couverture|passe|fusionn|ERROR' ghidra_run.log | sed 's/ (GhidraScript).*//'

"$HEADLESS" ghidra_proj ddsom -process prog_dec.bin -noanalysis -scriptPath ghidra_scripts \
	-postScript ApplySymbols.java "$PWD/symbols.txt" 2>&1 | grep -E 'symboles|ERROR' | sed 's/.*> //; s/ (GhidraScript).*//'

"$HEADLESS" ghidra_proj ddsom -process prog_dec.bin -noanalysis -readOnly -scriptPath ghidra_scripts \
	-postScript Report.java -postScript DumpListing.java "$PWD/listing.txt" 2>&1 \
	| grep 'Report.java>' | sed 's/.*Report.java> //; s/ (GhidraScript).*//'
