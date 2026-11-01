#!/bin/bash
# fuzz.sh : fuzz_couv C02 (scénarios d'ennemis) et C04 (boss), branche et main, même graine ; sorties dans fuzz/
S=${SCRATCH:?dossier temporaire}
W=${W:?worktree}
out=$S/locales_fuzz; mkdir -p $out
cd $W
{
for s in kobold_scene7 skeleton_scene58 gargoyle_scene55 hellhound_scene55 ogre_scene66 scorpion_scene36 trog_scene101 elf_scene96 beetle_scene18 ghoul_scene26; do
  for v in branche main; do echo "$v C02 scenarios/enemy/$s.txt"; done
done
for s in boss0_poke boss2_poke3p boss5_scene58 boss8_scene64 boss10_scene65 boss11_scene42 boss15_scene46 boss16_poke boss19_scene32 boss21_scene44; do
  for v in branche main; do echo "$v C04 scenarios/boss/$s.txt"; done
done
} | xargs -P ${JOBS:-8} -L 1 sh -c '
  n=$(basename $2 .txt)
  FUZZ_PERIODE=8 FUZZ_ESSAIS=32 '"$S"'/locales_fuzz_$0 /home/ycr/som/rom /home/ycr/som/testdata/eeprom_ddsom.bin $2 12000 $1 '"$out"'/$0_$1_$n.bin > '"$out"'/$0_$1_$n.txt 2> '"$out"'/$0_$1_$n.err
  echo "$0 $1 $n : $(tail -1 '"$out"'/$0_$1_$n.txt)"
'
