# Code mort et replis jamais atteints

Une entrée par cas retiré ou gardé (règle : `docs/guide-refactorisation.md`, « Code mort et replis »). Chaque lot a sa
section.

## Lot D : joueurs et monde

### Replis `_ => g.rom(adresse de la routine)` des automates des joueurs (supprimés)

* **Adresses** : 0x3573e (course), 0x35c26 et 0x35c38 (porter / lâcher), 0x35e56 (escalade), 0x3632c (sortie),
  0x36be0 (fin du ralentissement), 0x36c12 (saut sur place), 0x36b26 (garde), 0x36d68 (pousser), 0x35a78 (attaque
  sautée), 0x35b96 (saut arrière du magicien), 0x39ff0 (parade), 0x36abe (saisi).
* **Ce que faisait le code** : quand le pas (`+7`, `+6` ou `+0x8b`) ne correspondait à aucun bras réécrit, la
  fonction Rust rappelait la routine d'origine à sa propre adresse, pour que le 68000 fasse le travail.
* **Pourquoi jamais atteint** : chaque routine d'origine aiguille par une table de sauts `move.w (table,PC,D0.w)` dont
  les entrées sont exactement les bras réécrits ; un pas hors table ferait sauter l'original dans des données, et
  rien n'écrit de telles valeurs (recherche des écritures de `+7`/`+6`/`+0x8b` dans `player/`). Sans 68000, le repli
  menait à la native elle-même (0x35a78, 0x35b96 : récursion sans fin) ou à une adresse sans native.
* **Nature** : repli défensif de la conversion. Remplacé par `unreachable!` (message : adresse et valeur).
* **Preuves** : validation des scénarios du lot (90 scénarios, dont `bot*`, `mash_*`, `covb_*`, boss) sans passage ;
  tables lues dans la ROM (`joueur_tmp`).

### Repli de l'aiguillage des actions 0x35586 (`actions::run_action`, remplacé)

* **Ce que faisait le code** : pour une action sans bras Rust, saut par adresse dans la table d'origine.
* **Constat** : trois entrées n'avaient pas de native : 0x26 (0x36b88, l'arme maudite tombe), 0x2c (0x36cb8, bouffées
  de poussière) et 0x30 (0x36f1a, recul après une caisse). Elles sont atteignables (posées par `checks::item_curse`,
  `common::curse_check`, `combat/contact.rs`, `families/effects_f.rs`, `families/boxes.rs`) mais aucun scénario ne
  les exerce : le jeu sans 68000 s'y serait arrêté. **Comportement réel : réécrit en Rust** (`moves::curse_drop`,
  `dust_puffs`, `box_recoil`), validé par `fuzz_player player_action` (1 300 appels, 0 écart). Le bras restant
  (action impaire ou > 0x32) est `unreachable!` : la table n'a que 26 actions paires.

### Aiguillage des actions des personnages (`characters::run`)

* **Ce que faisait le code** : lecture de l'adresse dans les tables 0x35918-0x35a48 puis appel par adresse ; la liste
  `OVERRIDES` (vide) permettait de remplacer une routine par une fonction Rust.
* **Remplacé** par `characters::routine` (table Rust équivalente, lue dans la ROM : les deux variantes d'un personnage
  ont le même code). `OVERRIDES` (vide, jamais utilisée) supprimée. Les natives des actions restent pour
  `fuzz_player` et `DDSOM_CHECK`.

### Créateurs d'effets joueur sans native (réécrits, pas supprimés)

0x4fefe (arme maudite qui s'échappe), 0x5009e (éclats du joueur mourant), 0x51aa4 (nuée de l'orage), 0x51872
(lueur du portail), 0x4cfee (étincelles du portail), 0x4d064 (éclat du portail), 0x54274 (pluie de traits) : appelés
par le code des joueurs, jamais exercés par les scénarios, sans native (arrêt du jeu sans 68000). **Comportement
réel : réécrits** dans `player/spawn.rs`, validés par `fuzz_player spawn` (`DDSOM_ENTREE`, 300 appels chacun, 0 écart).
