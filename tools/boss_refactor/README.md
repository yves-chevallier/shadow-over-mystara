# Outils du lot L9 (boss)

Scripts qui ont servi au lot L9a ([docs/fin.md](../../docs/fin.md), « Résultat L9a », « Pour L9b »). Ils ne changent
aucune valeur : ils remplacent un littéral par un nom de même valeur. Lancer `brec.py`, `common_fields.py`,
`romaddr.py` et `jt.py` depuis `port/ddsom-game/src/families/`, `check_tables.py` depuis la racine du dépôt.

* `brec.py CHEMIN Struct [1]` : le `mod g` d'un boss devient un `record!` `Struct` ; un champ de même nom et de même
  décalage que dans `Boss` (avec `1` : aussi le gabarit `+0xa0..`) ou `Enemy` prend le nom commun.
* `BOSS_FIELDS=1 common_fields.py apply FICHIERS` : décalages littéraux communs vers `Enemy::`/`Obj::`/`Boss::` ; affiche
  ce qui reste. Attention : `x.set8(off, 0)` est pris pour un décalage (vérifier les boucles `for off in`).
* `romaddr.py CHEMIN` : constantes `u32` de la ROM en `RomAddr` (et leurs usages en `.0`) ; vérifier les noms qui
  entrent en collision (`pools::SET`) et les décalages A5 pris pour des adresses.
* `jt.py CHAMP FICHIERS` : `match dispatch(x, 0xT, F)` / `match step(x, 0xT)` en `match x.u8(F)` sur les index, d'après
  le programme déchiffré (`PROG_DEC`, par défaut `/home/ycr/som/prog_dec.bin`) ; un bras qui reçoit l'adresse (`t =>`)
  est à reprendre à la main.
* `check_tables.py` : tables lues mais non déclarées dans `extract/src/bosses.rs`, et l'inverse.
