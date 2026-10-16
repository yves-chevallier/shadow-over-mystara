# Enregistrement joueur

4 enregistrements de 0x200 octets : P1 = FF85DE, P2 = FF87DE, P3 = FF89DE, P4 = FF8BDE.
Le code y accède via `lea (0x5de,A5),An` puis `lea (0x200,An),An` (A5 = FF8000).

| Offset | Taille | Rôle | Source |
|---|---|---|---|
| +0x00 | b | actif (0 = slot libre) | boucles `tst.b (A2)` |
| +0x01 | b | drapeaux (bit 1 testé pour exclure le joueur du ciblage) | 0x429c |
| +0x02 | b | numéro de joueur 0..3 (index dans FF8000) | get_player_input |
| +0x03 | b | personnage (index des tables de vitesses, 0..7) | player_walk_speed |
| +0x04 | b | état principal (< 4 = vivant/ciblable ?) | 0x42a6 |
| +0x08 | l | X monde, 16.16 | mesuré + obj_move_xy |
| +0x10 | l | Y profondeur, 16.16 (augmente vers le haut de l'écran) | mesuré + obj_move_xy |
| +0x14 | w | vx, 8.8 px/image | mesuré + obj_move_xy |
| +0x16 | w | ax, 8.8 | obj_move_xy |
| +0x1c | w | vy, 8.8 | mesuré + obj_move_xy |
| +0x1e | w | ay, 8.8 | obj_move_xy |
| +0x62 | w | testé non nul avant certaines actions (PV ?) | 0x291a |
| +0x66 | b | drapeau : force la marche lente (1.0 px/f) | player_walk_speed |
| +0x6c | b | entrées maintenues (masquées) | copy_inputs_to_players |
| +0x6d | b | entrées image précédente | idem |
| +0x6e | b | entrées nouvellement appuyées | idem |
| +0x8a | b | drapeau : marche lente (1.0 px/f) | player_walk_speed |
| +0x162 | b | drapeau : marche normale 2.0 px/f (sinon 3.0) — à identifier | player_walk_speed |

Bits d'entrée (actifs à 1 après inversion) : 0 droite, 1 gauche, 2 bas, 3 haut, 4..7 boutons 1..4.

## Déplacement : chaîne complète

1. `read_inputs` (0x80c, chaque image) : lit 804000/804010/804020, inverse, calcule les fronts.
2. `copy_inputs_to_players` (0x2aec) : `entrée & in_mask` vers +0x6c..+0x6e de chaque joueur.
3. `player_walk_speed` (0x39260) : choisit une table selon les drapeaux, puis
   `vx = table[perso][dir].x ; vy = table[perso][dir].y` avec `dir = (+0x6c) & 0xF`.
   Les diagonales sont pré-normalisées (4.5 → 3.18 = 4.5/√2).
4. `obj_move_xy` (0x1d62) : intégration accélération → vitesse → position.

## Méthode (pour reproduire)

- Positions/vitesses : `mame/scenario.lua` + `RAMDUMP`, puis `tools/ramdiff.py` / `tools/ramcorr.py`.
- Lecteurs/écrivains : `mame/xref.sh scenario.txt ff85e6 ff85e9 out.txt`.
- Appelants : `CALLERS="2bfe"` avec `mame/session.lua`.
