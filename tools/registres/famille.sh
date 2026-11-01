#!/bin/bash
# famille.sh <fichier.rs:fonction> <motif de nettoyage> : rend générique la fonction et ce qu'elle appelle, nettoie les
# registres dans les fichiers du motif ; en cas d'échec, restaure src depuis l'instantané.
O=$(cd "$(dirname "$0")" && pwd)
cd ${W:?worktree}/port
snap=${SCRATCH:?dossier temporaire}/${P:-locales}_snap
rm -rf $snap; cp -r ddsom-game/src $snap
for i in 1 2 3 4; do
  out=$(python3 $O/generique.py . $([ $i = 1 ] && echo $1) 2>&1)
  if echo "$out" | grep -q "^compile"; then echo "COMPILE ($1)"; exit 0; fi
  n=$(python3 $O/nettoie.py . $2 2>&1)
  echo "$n" | grep -v "retiré\|reste :" 
  if echo "$n" | grep -q "À VOIR"; then break; fi
  if echo "$n" | grep -q "^0 autres erreurs" && ! echo "$n" | grep -q "retiré\|reste :"; then break; fi
done
echo "ÉCHEC ($1) : restauré"
echo "$out" | grep -v "^  ddsom" | grep -E "^error|^ *-->|^[0-9 ]+\|" | head -40
rm -rf ddsom-game/src; cp -r $snap ddsom-game/src
exit 1
