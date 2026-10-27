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

### À vérifier : entrées de tables d'aiguillage sans gestionnaire Rust ni native

Relevé statique (tables de sauts du code des joueurs, longueur estimée par le premier gestionnaire qui suit la table,
donc peut-être surestimée) : entrées qui ne mènent ni à un gestionnaire de `HANDLERS`/`Steps`, ni à une native. Si
elles sont atteignables, le jeu sans 68000 s'y arrêterait ; aucun scénario ne les exerce. Non traité au lot D :

| Table | Entrées sans gestionnaire |
|---|---|
| 0x23b98 (résurrection, `scripted`, pas `+6` = 8) | 0x23c7a (résurrection devant l'objet suivi par la caméra) |
| 0x23d66 (attente, `scripted`) | 0x23d68 |
| 0x3a584, 0x3a89c, 0x3a9e6, 0x3aa04, 0x3af50, 0x3b57c, 0x3b738, 0x3b7c6, 0x3b8b2, 0x3b8d0 (réactions, `hurt`) | 0x3a59a, 0x3a5a4, 0x3a296, 0x3aa16, 0x3aa5e, 0x3aff8, 0x3b592, 0x3b5b2, 0x3b656, 0x3b740, 0x3b78a, 0x3b7a4, 0x3b7aa, 0x3b7ca, 0x3b7e2, 0x3aa2e, 0x3b8d6, 0x3b8e0 |

## Lot B : effets, objets, coffres

### Replis `_ => deleguer(cible)` / `_ => x.call(cible)` des tables d'états (remplacés par `hors_table`)

* **Sites** : 68 aiguillages des gestionnaires EFFCT (`effects.rs`, 24), ITEM (`items.rs`, 29 ; `items_b.rs`, 3) et
  BOX (`boxes.rs`, 12) : état `+4`, sous-état `+5`, `2 × variante`, octet d'action `+0x3d` des bulles, tables de
  longs (`0xdafb8`, `0xdb392`).
* **Ce que faisait le code** : pour un indice sans bras Rust, saut dans le code d'origine à la cible lue dans la table
  (`move.w (table,PC,D0.w),D1.w ; jmp (table,PC,D1.w)`).
* **Pourquoi jamais atteint** : relevé de chaque table (`table.py` sur `prog_dec.bin`, étendue vérifiée par le label
  de code ou la table qui suit dans `listing.txt`). 56 aiguillages ont un bras Rust pour chaque entrée de la table :
  un indice hors table ferait sauter l'original dans des données ou au milieu d'une instruction (rien ne l'écrit).
  Les 12 autres (`effects.rs` : halo, aura, scintillement, traînée, poussière, nuage, balayage, fumée, scintillement
  de classe, trace — `trois_etats` —, bandeau, éclat, interface 1, 2 et 4, éclat unique, marque, gerbe, éclats de
  débris, accessoire, déclencheur) n'avaient pas de bras pour l'état 6 (et 4 pour l'interface 4) : la cible est un
  simple `jmp 0x2218` (libération EFFCT), que `deleguer` appelait déjà directement. État 6 posé par l'objet parent
  qui détruit ses effets (54 écritures de 6 / `0x06000000` en `(4,An)` dans la ROM, non attribuées type par type) et
  par l'accessoire lui-même (`accessoire_suit`, propriétaire nul).
* **Décision** : l'état 6 est un bras explicite (`etat::DETRUIT => free`, comportement réel) ; le reste devient
  `hors_table(cible)` (`effects.rs`, `unreachable!` avec l'adresse que l'original atteindrait). Les bulles
  (`bulle_selon_action`) : une action hors de la petite table ferait sauter l'original dans le code qui la suit ;
  les actions produites par les joueurs n'y mènent pas sur les scénarios (non prouvé pour toutes les actions :
  `hors_table` le signalera).
* **Nature** : repli défensif de la conversion.
* **Preuves** : audit des 68 tables (lot B, 2026-10-27) ; validation complète.

### Remarque : variante 3 de la barrière (`boxes.rs`, type 1)

L'initialisation accepte la variante 3 (le mot lu en 0xda5ac retombe sur le code de la variante 0), mais l'état actif
(`0xda5f4`, deux entrées) la ferait sauter en 0xdb61c, au milieu d'une instruction : l'original planterait aussi.
Aucune donnée de scène connue ne crée de barrière de variante 3 ; `hors_table` le signalerait.

### Pool plein menant à une erreur d'adresse (0xaf2bc, 0xb778c ; `erreur_d_adresse`)

* **Adresses** : 0xaedde, 0xaeeaa, 0xaeef8, 0xaef2c (bourse de la boutique, `effects_c::purse_init` : pools ITEM,
  PEFF, SOBJ, PEFF) → 0xaf2bc ; 0xb7560, 0xb7584 (créature de l'introduction, `swimmer`, deux SET) → 0xb778c.
* **Ce que fait l'original** : `bcs` vers `tst.w (0x1,A5)` : lecture d'un mot à une adresse impaire, erreur d'adresse
  du 68000, le jeu s'arrête sur l'écran d'exception. Le Rust rappelait l'adresse d'origine (`x.call`), sans native
  (le jeu sans 68000 s'y serait arrêté).
* **Pourquoi jamais atteint** : il faudrait un pool plein à la création de la bourse ou de la créature ; aucun
  scénario ne le produit.
* **Nature** : bug d'origine (branchement vers une instruction de diagnostic, sans doute un `illegal` de débogage
  devenu `tst.w`) ; comportement réel mais fatal. **Remplacé par** `panic!` documenté (`effects::erreur_d_adresse`) :
  le moteur s'arrête comme le jeu, sans reproduire l'écran d'exception.

## Lot A : ennemis et boss (`families/boss_*`, `enemy*`, `reactions.rs`)

Rien n'a été supprimé : les constantes retirées étaient des adresses recopiées devenues inutiles (les appels
passent par des fonctions), et les aides retirées des doublons renommés (`boss_common::play` → `lib::start`...).

### Routines d'origine sans native, appelées par les familles

Ces routines étaient appelées par adresse depuis des gestionnaires réécrits mais n'avaient pas de native : le jeu
sans 68000 se serait arrêté en les atteignant. Aucun des 269 scénarios ne les atteint (la validation complète,
qui s'arrête sur une adresse sans native, passe). Elles sont maintenant transcrites en Rust dans
`families/enemy_lib.rs` (section « routines d'origine sans native ») d'après `listing.txt`, registres de sortie
compris, et appelées directement. **Non validées par un scénario** : à couvrir (poke, fuzz) quand le chemin sera
atteignable.

| Adresse | Appelant | Ce qu'elle fait | Pourquoi jamais atteinte (h) |
|---|---|---|---|
| 0x10298 | boss 16 (`moves.rs`, réveil) | script de palette : 8 longs de la table 0x102ba copiés en 0x91c380, luminosité maximale | séquence de réveil du boss 16 non jouée par les scénarios (`boss16_poke` démarre après) |
| 0xa2d22 | boss 3 (`attacks.rs`) | six tirs ESHL 0x26 (`+0x2f` = 5..0) liés par `+0x46` | attaque du boss 3 rarement tirée ; pas dans les scénarios |
| 0xffda | boss 3 (`fight.rs`, riposte 0x80a5c) | tirage d'un bit dans un masque (table 0xffd8 par compteur d'images / 8) : attaque 16 ou 20 | riposte non déclenchée dans les scénarios |
| 0xa09ca | Gnoll (`arrival.rs`, lâche sa hache) ; Harpie (`enemy_harpy.rs`) | tir ESHL 0x12 à 0x30 pixels de haut et 0x60 devant ; glitch : la retenue testée par la harpie est celle de l'addition en X | aucune scène de Gnoll armé ni de ce geste de Harpie dans les scénarios |
| 0xa114a | Gnoll (`arrival.rs`, `hit.rs`) | tir ESHL 0x16 de variante D0 (arme reprise / perdue) | idem |
| 0xa0ae0 | Harpie | tir ESHL 0x13 visant la cible (A2), lancé par l'objet | geste de Harpie non joué |
| 0x9f346 | Troll (`attacks.rs`, lancer) | tir ESHL 0xa orienté à l'opposé, lié par `+0x50` | lancer du Troll non joué |
| 0xad7c | réactions (`reactions.rs`, reprise d'état sauvé 0x205) | tirage d'un bit dans la ligne `+0x78` (table 0xadb0) ; faux si `+0x78` < 0 | état sauvé 0x205 jamais rencontré |

### Reste en code d'origine

* 0x818e6, 0x819e2 (formes du corps d'un boss, table 0x81ad4, appelées par `boss_common::push_out_of_boss`) : sans
  native. `+0xa4` (forme) n'est non nul pour aucun boss des scénarios (h). À transcrire avec le boss qui les pose.
