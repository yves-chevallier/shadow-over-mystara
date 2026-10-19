# BOSS (C04) : les boss (`families/boss_*.rs`)

Pool de 3 objets de 0xC0 octets (FFBA1E, compteur de places libres inutilisé), table de gestionnaires 0x39b12 (23
entrées, dispatch 0x39aec). Même cadre que les ennemis (`docs/familles/enemy.md` §2, `Cx`, `handlers!`, contrat A0/D7 :
`docs/familles-eshl-set-sobj.md`) : `+4` état (0 création, 2 actif, 4 touché, 6 libération), `+5`, `+6`, `+7` sous-états,
octets pairs qui indexent des tables de sauts. `boss_common.rs` réexporte `enemy_common` (champs, états, `alloc`,
`suspended`, `random`...) et ajoute les variables de scène des boss (`var::*`, `leader`, `scroll_x`).
Les noms de rôle (h) sont des hypothèses tirées du comportement et des noms de debug du jeu (atlas, annexe A).

## 1. Qui apparaît où

Relevé par `examples/boss_scan.rs` (saute de scène en scène sur la machine d'origine) et `examples/boss_seen.rs`
(rejoue un scénario et note la première image de chaque boss) :

| Type | Handler | Octets | Nom (h) | Scènes |
|---|---|---|---|---|
| 13 | 0x8f59c | 1 352 | dragon noir en vol | 7, 9 (visible aussi pendant 6 et 12 : l'objet survit au changement de scène) |
| 17 | 0x952b4 | 9 016 | Tel'Arin (h) | 13, 44 (variante 1) |
| 6 | 0x8430e | 5 866 | Manticore | 24 |
| 18 | 0x975ec | 7 822 | Lich (h) | 26 |
| 19 | = 14 (0x8fae4) | 6 732 | dragon vert (h) | 32, 65-66 (variante 1) |
| 12 / 11 | 0x8d59e | 8 190 | salamandres de feu / de glace | 40 / 42 |
| 21 | 0x9947a | 6 704 | ? | 44 |
| 15 | 0x91530 | 7 470 | dragon rouge | 46 |
| 5 | 0x8341c | 3 826 | Ezerhorden | 58 |
| 8 | 0x87fc0 | 10 140 | DarkWarrior 2 | 64 |
| 10 | 0x8ca4a | 2 900 | Nagpa | 65, 66 |

Les types 0 à 4 (Ogre, Displacer Beast, Man Scorpion, Harpy, Chimera) n'ont été vus dans aucune scène 1-68 ni dans les
scénarios `cov_*` : le jeu les joue comme ENEMY (types 4, 19, 20, 21, 22...). Les types 14 et 19, 15 et 20, 11 et 12 partagent un handler.

## 2. Réécrits

### Type 13 (0x8f59c) : dragon noir en vol — `boss_dragon.rs`

Ce n'est pas un adversaire : il accompagne la caméra (`A5+0x506`) à `scroll - 0x80`, flotte (`bob` : tous les 16 images,
quatre vitesses tirées à `4 × (tirage & 7)` octets dans la table 0x8faa2, qui se chevauchent comme dans l'original,
puis leurs opposées), et déclenche des étapes quand la caméra dépasse les seuils 0xf00, 0x1e00, 0x2d00, 0x3c00 de la
table 0x8f97a (`next_stop`, compteur `+0xa7` ; la lecture au-delà du 4e seuil lit des opcodes). Création : au moins
deux places ESHL libres et un meneur de la caméra (`A5+0x46`), deux tirs ESHL 0x14 (variantes 1 puis 0, `+0x50` = le
dragon) et le jeu d'animations 0x17d7d2. Sous-états `+6` de l'état actif (`+5` = 0 ; `+5` = 2 libère) :

| `+6` | Rôle |
|---|---|
| 0 | prépare l'étape (efface les bits `+0x42`, appelle 0xdf4bc) |
| 2 | attend le seuil suivant ; si le bit 0 de `+0x42` est levé : fige le jeu (`A5+0x3d/0x3c/0x33` = 1, `+0x16a` des joueurs), ouvre un message (0xc8312, type 0xe, lien `+0xb0`) et lance la cinématique (0xe0d8c) |
| 4 | attend la fin du message (octet `+0x22` du message), rend la main aux joueurs, repart vers le meneur |
| 6 | s'approche du meneur (`+0x58` du meneur ajouté à X ; 0x1e92 teste l'arrivée) |
| 8, 12 | suit le meneur dans la fenêtre `[x - 0xe0, x - 0x60]` pendant `+0xa2` images |
| 10 | idem puis tient la position tant que le bit 0 de `+0xa5` |
| 14 | sort : retient la vitesse du meneur moins le défilement, ou -8, ou -0x200 si `0xff0ec6` ; hors de ±0x200 de la caméra : fin de cinématique (0xe0c5c), retour à l'état 2/0 |

L'octet `0xff0ec6` (`A5-0x713a`) force le départ. Les états 4 et 6 de `+4` appellent la libération d'ENEMY `0x21f4` (l'original
ne libère pas le boss par `0x2234` : ils ne sont jamais atteints). Couverture : fuzz `fuzz_boss` (300 instructions sur 301 en
300 tours, toutes en 600), `DDSOM_CHECK=1 DDSOM_CHECK_ALL=1` sur le scénario (12 000 images : VALIDE).

## 3. Validation

* `scenarios/boss/` : un scénario par boss (entrée directe dans la scène par `poke8 ff8034`, bot `autobot`, PV du boss
  intacts), générés par `tools/gen_boss.py` ; `tools/validate_all.sh` les lance sur 12 000 images.
* `scenarios/ko/boss13_dragon.txt` : le dragon diverge dès l'image 3100 (fraction de X du joueur 1), avec ou sans son natif
  (`DDSOM_SANS=boss_13`) ; chaque native seule, et toutes les paires, sont VALIDES ; toutes natives ensemble divergent. Hypothèse :
  la machine d'origine B finit en retard certaines images de cette scène chargée (tirs, sprites) alors que A, plus rapide,
  les finit à temps. Contrôle appel par appel (`DDSOM_CHECK=1 DDSOM_CHECK_ALL=1`) : VALIDE.
* `examples/fuzz_boss.rs <roms> <eeprom> <scénario> <chauffe> <tours>` (modèle `fuzz_enemy.rs`) : objet neuf dans le pool BOSS,
  états tirés dans les tables de sauts (`DDSOM_ETATS=1`), meneur de la caméra, joueurs, caméra et drapeaux perturbés à chaque pas,
  chaque appel comparé à l'original ; `DDSOM_COUVERTURE=fichier` écrit les instructions vues (`DDSOM_GRAINE`, `DDSOM_ENTREE`, `DDSOM_PAS`).
  Exemple : `DDSOM_ETATS=1 fuzz_boss $DDSOM_ROMS $DDSOM_EEPROM scenarios/bot_char0.txt 4500 300`.
