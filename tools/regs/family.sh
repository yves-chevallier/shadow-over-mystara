#!/bin/bash
# family.sh <file.rs:function> <cleanup pattern>: makes the function and what it calls generic, cleans the
# registers in the pattern's files; on failure, restores src from the snapshot.
O=$(cd "$(dirname "$0")" && pwd)
cd ${W:?worktree}/port
snap=${SCRATCH:?temporary directory}/${P:-locales}_snap
rm -rf $snap; cp -r ddsom-game/src $snap
for i in 1 2 3 4; do
  out=$(python3 $O/generic.py . $([ $i = 1 ] && echo $1) 2>&1)
  if echo "$out" | grep -q "^compile"; then echo "COMPILE ($1)"; exit 0; fi
  n=$(python3 $O/clean.py . $2 2>&1)
  echo "$n" | grep -v "removed\|left:" 
  if echo "$n" | grep -q "TO CHECK"; then break; fi
  if echo "$n" | grep -q "^0 other errors" && ! echo "$n" | grep -q "removed\|left:"; then break; fi
done
echo "FAILED ($1): restored"
echo "$out" | grep -v "^  ddsom" | grep -E "^error|^ *-->|^[0-9 ]+\|" | head -40
rm -rf ddsom-game/src; cp -r $snap ddsom-game/src
exit 1
