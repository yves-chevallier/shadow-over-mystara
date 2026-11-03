# Code mort et replis jamais atteints

Une entrée par cas retiré ou gardé (règle : `docs/guide-refactorisation.md`, « Code mort et replis »). Chaque lot a sa
section.

## Lot D : joueurs et monde

### Replis `_ => g.rom(address de la routine)` des automates des joueurs (supprimés)

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

### Replis `_ => deleguer(target)` / `_ => x.call(target)` des tables d'états (remplacés par `outside_table`)

* **Sites** : 68 aiguillages des gestionnaires EFFCT (`effects.rs`, 24), ITEM (`items.rs`, 29 ; `items_b.rs`, 3) et
  BOX (`boxes.rs`, 12) : état `+4`, sous-état `+5`, `2 × variant`, octet d'action `+0x3d` des bulles, tables de
  longs (`0xdafb8`, `0xdb392`).
* **Ce que faisait le code** : pour un indice sans bras Rust, saut dans le code d'origine à la cible lue dans la table
  (`move.w (table,PC,D0.w),D1.w ; jmp (table,PC,D1.w)`).
* **Pourquoi jamais atteint** : relevé de chaque table (`table.py` sur `prog_dec.bin`, étendue vérifiée par le label
  de code ou la table qui suit dans `listing.txt`). 56 aiguillages ont un bras Rust pour chaque entrée de la table :
  un indice hors table ferait sauter l'original dans des données ou au milieu d'une instruction (rien ne l'écrit).
  Les 12 autres (`effects.rs` : halo, aura, scintillement, traînée, poussière, nuage, balayage, fumée, scintillement
  de classe, trace — `three_states` —, bandeau, éclat, interface 1, 2 et 4, éclat unique, marque, gerbe, éclats de
  débris, accessoire, déclencheur) n'avaient pas de bras pour l'état 6 (et 4 pour l'interface 4) : la cible est un
  simple `jmp 0x2218` (libération EFFCT), que `deleguer` appelait déjà directement. État 6 posé par l'objet parent
  qui détruit ses effets (54 écritures de 6 / `0x06000000` en `(4,An)` dans la ROM, non attribuées type par type) et
  par l'accessoire lui-même (`accessoire_suit`, propriétaire nul).
* **Décision** : l'état 6 est un bras explicite (`state::DETRUIT => free`, comportement réel) ; le reste devient
  `outside_table(target)` (`effects.rs`, `unreachable!` avec l'adresse que l'original atteindrait). Les bulles
  (`bubble_by_action`) : une action hors de la petite table ferait sauter l'original dans le code qui la suit ;
  les actions produites par les joueurs n'y mènent pas sur les scénarios (non prouvé pour toutes les actions :
  `outside_table` le signalera).
* **Nature** : repli défensif de la conversion.
* **Preuves** : audit des 68 tables (lot B, 2026-10-27) ; validation complète.

### Remarque : variante 3 de la barrière (`boxes.rs`, type 1)

L'initialisation accepte la variante 3 (le mot lu en 0xda5ac retombe sur le code de la variante 0), mais l'état actif
(`0xda5f4`, deux entrées) la ferait sauter en 0xdb61c, au milieu d'une instruction : l'original planterait aussi.
Aucune donnée de scène connue ne crée de barrière de variante 3 ; `outside_table` le signalerait.

### Pool plein menant à une erreur d'adresse (0xaf2bc, 0xb778c ; `address_error`)

* **Adresses** : 0xaedde, 0xaeeaa, 0xaeef8, 0xaef2c (bourse de la boutique, `effects_c::purse_init` : pools ITEM,
  PEFF, SOBJ, PEFF) → 0xaf2bc ; 0xb7560, 0xb7584 (créature de l'introduction, `swimmer`, deux SET) → 0xb778c.
* **Ce que fait l'original** : `bcs` vers `tst.w (0x1,A5)` : lecture d'un mot à une adresse impaire, erreur d'adresse
  du 68000, le jeu s'arrête sur l'écran d'exception. Le Rust rappelait l'adresse d'origine (`x.call`), sans native
  (le jeu sans 68000 s'y serait arrêté).
* **Pourquoi jamais atteint** : il faudrait un pool plein à la création de la bourse ou de la créature ; aucun
  scénario ne le produit.
* **Nature** : bug d'origine (branchement vers une instruction de diagnostic, sans doute un `illegal` de débogage
  devenu `tst.w`) ; comportement réel mais fatal. **Remplacé par** `panic!` documenté (`effects::address_error`) :
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

## Lot « noyau final » (`kernel.rs`, `vblank.rs`, `startup.rs`, `no68k.rs`)

### Handlers des traps 0 à 7 et 12 (0x9c2, 0x9fa, 0xa20, 0xa52, 0xa92, 0xab8, 0xad4, 0xae6, 0xafa ; natives retirées)

Ce qu'ils font : le travail des enveloppes de l'ordonnanceur (créer, finir, tuer, céder, suspendre, réveiller une
tâche, recommencer en A0, masquer les interruptions). Pourquoi ils ne sont pas atteints : le jeu ne prend ces traps que
par leurs enveloppes (0x9b6... 0xb64, natives qui font le travail sur la table des tâches Rust sans prendre le trap) ;
seul `trap #7` est aussi exécuté directement, par le menu de mise au point 0x2c3e0, que le jeu réécrit n'exécute pas
(carte de développement, `docs/service.md`). Les natives de ces adresses ne servaient qu'au mode hybride (le 68000 y
aurait été) : 0x9c2-0xad4 paniquaient, 0xae6 recommençait la tâche, 0xafa/0xaf8 changeaient le masque puis revenaient.
Hypothèse : chemin normal du noyau d'origine, rendu inutile par la réécriture (pas du code mort de l'original). Repli
défensif supprimé ; une adresse sans native arrête le jeu avec un diagnostic.

### Tâche de l'écran 0x19068 (emplacement 0xf0)

Déjà signalée (`docs/noyau.md`, « Code mort de la machine à modes ») : sa tâche n'a pas d'étape d'entrée Rust ; le noyau
s'arrête sur `entrée 19068 without étape Rust` si elle était créée (jamais dans les 272 scénarios).

## Lot IA : aides d'IA et derniers services

### Routines du lot A sans native (section précédente) : validées par fuzz, deux erreurs corrigées

`examples/fuzz_ia.rs` compare chaque routine transcrite à l'original (interpréteur 68000 seul, aucune native), sur des
états tirés à partir d'une partie réelle (un ennemi ou un boss actif sert d'objet) : 0xa09ca, 0xa0ae0, 0xa114a,
0x9f346, 0xa2d22, 0xad7c sans écart ; deux écarts corrigés :

* **0xffda** (`enemy_lib::roll_ffda`) : la table des masques est en 0xfff4 (`move.w (0xfff4,PC,D0w)`, juste après le
  `rts` de la routine), pas en 0xffd8 ; 112 tirages sur 200 différaient.
* **0x10298** (`enemy_lib::palette_script`) : le `movem.l (A2)` lit l'espace **données** (A2 vient d'un `lea`), pas
  l'espace programme.

Les dix routines ajoutées par le lot D (`moves::curse_drop`, `dust_puffs`, `box_recoil`, sept créateurs de
`player/spawn.rs`) étaient déjà validées par `fuzz_player` (section du lot D).

### Natives des aides d'IA retirées (aucun appel par adresse)

Après la conversion de leurs appelants (façade `enemy_lib`, familles, combat), plus aucun code Rust ni table de la
ROM n'appelle par adresse les routines de `ai.rs` et `ai_aids.rs` : leurs natives (0x424e, 0x43ae, 0x43bc, 0x44ee,
0x453c, 0x4544, 0xa4b4, 0xa4c2, 0xac3a, 0xfe5a, 0x1059e, 0xb5e0, 0x103d6, 0x1040c, 0x1050c, 0x10560, 0xfbae, 0xfbb0,
0xe76c, 0xf0e8, 0xfffc, 0x103ba, 0xe886, 0xe914, 0xeff2, 0xf084, 0xfda2, 0xff54, 0x104a2, 0x10574, 0x105b6, 0xfc44,
0xfd64, 0xfd8c, 0x103e0, 0x10428, 0x10450, 0x10474, 0x1047a, 0x104e0, 0xa470, 0x7f0c, 0x7ef4, 0x7f19c, 0x11654,
0x1161c, 0x115e2, 0x67452, 0x788c, 0x79ce, 0x79f6, 0xb048, 0xb068, 0xf118, 0x10112, 0x1172e, 0x11a36, 0xb648,
0xb780, 0xbad6, 0xe2f6, 0xe7fa, 0x3e480) et celles des secousses (0x2824, 0x2860, 0x289c : la table de 0x4b172 de
`peff_c` passe par `camera_modes::shake::of_routine`) sont supprimées ; les cœurs restent. Doublons supprimés :
0x67452 (`nearest_player_in_box`, déjà réécrite en Rust par le lot A : `enemy_common::find_player`), 0x11654 et
0x1161c (aides internes de 0x115e2). Les versions Rust du lot A qui doublaient des natives (`outside`,
`facing_target`, `moving_forward`, `jitter_angle`, `init_hp_rank`, `hp_threshold`, `ai_gate`, `any_player_alive`)
sont fondues avec les cœurs.

Routines jamais atteintes par les scénarios mais réécrites et validées par `fuzz_ia` (300 tirages chacune, 0 écart) :
0xe914 (point de marche atteint), 0xe7fa (point proche numéroté), 0xb068 (seconde table de parade), 0x103e0
(animation de la variante), 0xf084 seule.

## Lot « sortie du Cpu » (sauts vers des natives, partie 1)

Les replis qui sautaient (`jump`, `jump_to`, pose du PC) vers une adresse d'origine sans native s'arrêtaient de toute
façon (« pas de native ») dans le jeu sans 68000. Chacun vise une entrée de table que le jeu ne produit pas ; ils sont
remplacés par `unreachable!` (message qui nomme la table) ou par l'arrêt documenté de l'original. Preuves : tables lues
dans la ROM (`modes`, `tasks`, `scene/exit`), 286 scénarios (`DDSOM_LONG=1`) sans arrêt.

| Site | Table (ROM) | Entrées | Pourquoi le repli n'est pas atteint | Nature |
|---|---|---|---|---|
| `modes::turn` (mode inconnu) | 0x13424 | modes 0 et 2 | seul `init` écrit le mode (0 → 2) | repli défensif |
| `modes::dispatch` (sous-mode inconnu) | 0x1345c | 11 (0 à 0x14), toutes réécrites | les écrans n'écrivent que ces sous-modes | repli défensif |
| `title::title_step`, `second_title_step` | 0xfa2a4 (16), 0xfaf00 (5) | toutes réécrites | les étapes avancent de 2 jusqu'à la dernière | repli défensif |
| `demo::interpreter_a`, étape 4 et au-delà | 0x1f340 (4) | l'étape 4 (0x1fb20, fin de scène) n'est jamais écrite | la boucle A y mène directement (`scene_end_a`, même code) | entrée de table jamais choisie |
| `demo::presentation_step` | 0x1fc72 (4) | toutes réécrites | sous-étapes 0 à 6 | repli défensif |
| `demo::run_phase` (phases autres que 8 et 12) | 0x1f990, 0x20a9c | phases 8 et 12, les autres sont nulles (saut en 0) | la démonstration ne pose que ces deux phases | entrées nulles |
| `game::handler` (phase hors table) | 0x1c502 (8) | phases 0 à 0x1c | la phase avance de 4 et revient | repli défensif |
| `fixed_screen::tour` | 0x18386 (2), 0x1853c (3) | toutes réécrites | la sous-étape 4 (`end`) termine la tâche | repli défensif |
| `scene::exit` (`delegate` : séquence ou étape non réécrite) | 0xf3170 (séquence par scène) | séquences 0 à 0x46 (indices 0 à 35), toutes réécrites | les étapes restent dans leurs états | repli défensif |
| `service::menu` (« Secret Test », 0xee16e) | mot 0xc0 du programme | nul dans cette version | voir `docs/service.md` | fonction cachée, non réécrite |

**Boutique (0xf7dfa).** L'ouverture alloue un SOBJ et un EFFCT par joueur juste après avoir vidé tous les pools ; en
cas d'échec l'original saute en 0xf7dfa (`tst.w 1(A5)` : erreur d'adresse, comme 0xaf2bc). Jamais atteint (pools vides) :
`effects::address_error(0xf7dfa)`.

**Blocages (0x2d112, 0x2d7fa).** « NG » du MEMORY CHECK et « EEPROM ERROR » finissent sur `bra.b *` ; la native vide
posée à l'adresse est remplacée par le point `kernel::Point::Blocked` (l'image finit, les interruptions continuent).
Jamais atteints (`docs/service.md` §7).

**Affichage des verrous du Z80 (fin de 0x68c, appel de 0x29d78).** Écran de débogage : chaque octet des verrous
0x618001... en hexadécimal dans la couche de texte, seulement si le mot 0xc0 du programme (nul dans cette version) et
`A5+0x7fc6` sont non nuls ; 0x29d78 n'a pas de native. Nature : mode de mise au point. Depuis la partie 7 de la
sortie du Cpu, le comportement est gardé compilé et sans registres : `sound::send` appelle `show_locks`, qui écrit
chaque octet par `hex_byte` (transcription de 0x29d78, attribut 2), au lieu de l'`unreachable!` d'avant.

## Lot fuzz : branches non jouées, validées ou écartées par fuzz différentiel

Aucun code retiré. Le fuzz différentiel `examples/fuzz_couv` (bilan et méthode : [couverture-fonctions.md](couverture-fonctions.md),
« Validation par fuzz ») exécute les branches que les scénarios ne prennent pas ; les cas suivants restent des états que
le jeu ne produit pas, gardés tels quels dans le Rust :

* **Index hors des tables de sauts** (états `+4`/`+6`/variante au-delà des entrées : réaction 0x3ddf6 pas 0xc,
  bulle EFFCT 0xaabd8, actions des joueurs 0x35586) : l'original sauterait dans des données ou dans la table
  elle-même ; les natives paniquent avec l'adresse (`unreachable!`/`assert!`). Preuve : aucun écrivain de ces champs
  ne produit ces valeurs (tables relues), et le fuzz ne les atteint qu'en forçant l'octet.
* **Pool plein avec lecture à une adresse impaire** (bourse EFFCT 0xaf2bc) : l'original lèverait une erreur
  d'adresse (écran d'exception) ; panique documentée dans la native.
* **Champs hors domaine** : octet `+0x15e` des joueurs ≥ 0x80 (P5 : 0 à 0x25 dans tous les scénarios et les bots
  longs), orientation `+0x42` des anneaux du boss 11 hors de 0 à 31 (toujours masquée par `& 0x1f` à l'écriture),
  rang `A5+0x7f92` ≥ 0x8000 (0 à 0x1f). Les écarts natif/original n'y apparaissent que par ces valeurs.
* **Échange d'équipement sans propriétaire** (ITEM 3, `+0x50` nul : pointeur 0, lecture des octets de la ROM en 0x3 et
  0x140) : glitch reproduit (index D0.w avec l'octet haut de la classe), bien qu'un objet ramassé ait toujours un
  propriétaire.

## Lot « sortie du Cpu », partie 2 (écrans)

| Site | Adresse(s) | Ce que fait le code | Pourquoi il n'est jamais atteint | Nature | Décision |
|---|---|---|---|---|---|
| `scene::script::pool_of_class` | table 0xf2f38, dixième entrée (0x226e) | allocation de l'apparition par la routine de la classe | 0x226e n'est l'allocateur d'aucun pool (aucune native) ; aucun script d'apparition n'emploie la classe 9 (286 scénarios, et le jeu sans 68000 s'arrêterait sur l'adresse) | entrée de table jamais choisie | `unreachable!` |
| `tasks::prompt::stop`, `print_shown` | 0xf38, 0xf4a (attributs négatifs) | les blocs lisent la palette `+0x2b` de l'objet A0 de l'appelant quand les attributs du texte sont négatifs | les textes gardés par la tâche ont tous l'attribut 3 (tables 0xe048c, 0xe04b0, 0xe04e4, lues dans la ROM) | repli défensif du moteur de texte | objet 0 passé, commenté |
| `modes::select`, étape 6 | table 0x1d0f6 | appel de la routine d'initialisation par l'octet `+2` du joueur | la table ne contient que 0x2e58, 0x2e6e, 0x2e86, 0x2e9e (une par joueur) ; un octet `+2` ≥ 4 n'est jamais produit | repli défensif | `assert!` |

Les natives qui ne servaient plus qu'aux appels par adresse des écrans sont retirées avec leurs appels (le code reste,
en fonctions Rust) : textes et chiffres de l'écran des crédits (0x148be, 0x14652, 0x14b74, 0x14c1e), boutons de départ
et verrous des monnayeurs (0x141e0, 0x14290), chiffre des crédits (0x14e46).
Les remises à zéro des blocs de la sortie de scène, de la boutique et du classement (0xf3130, 0xf7916, 0xfc562) et
l'effacement du texte des écrans de fin (0xf683c) n'ont plus d'appelant par adresse : leurs natives restent (outils,
fuzz), en `Body::Memory`, sans les registres que laissait l'original (D0 = 0, A4/D6 de la boucle, A1/A2/D0-D2 de
l'effacement), qu'aucun appelant ne relit plus (le D1.w = -1 de 0xf683c, relu par le bonus, y est écrit en clair).

## Lot « sortie du Cpu », partie 5 (familles et boss en gestionnaires sans registres, wt-familles2)

### Boss 7 : replis hors table des aiguillages (0x85cc8, 0x86ee6)

`active` (0x85bd0) saute par la table de 8 mots 0x85cc8 indexée par `+5` (groupe 0 à 14) ; `death` (0x86ed8) par
la table de 4 mots 0x86ee6 indexée par `+6` (0 à 6). Au-delà, l'original lirait des mots de code comme décalages et
sauterait au milieu d'instructions : aucun état n'y mène (`+5` et `+6` ne prennent que les valeurs paires des
tables : écritures relevées dans `boss_7/`). Les replis Rust `x.call(t)` (appel par adresse d'une routine sans
native) sont devenus des `unreachable!` : repli défensif, supprimé.

### Golem (type 11, 0x67926) : boutons relus dans D0 après le pas d'animation

Pas du code mort, mais un comportement qu'aucun scénario n'exerce (le golem n'apparaît dans aucune scène et
n'est piloté que par les touches du quatrième joueur) : `enemy_golem::control::walk` relit les boutons dans D0,
que 0x1a56 écrase par l'octet `+0x32` quand l'image en cours se termine, et laisse le drapeau « a bougé » dans D2.
Écrit d'après le désassemblage (0x67926-0x679a0), non validé par un scénario.

## Lot « sortie du Cpu », partie 5 (joueur, set_b, effects_c en gestionnaires sans registres, wt-joueur2)

### Minuteur du vote : état 4 (EFFCT `effct_vote` 0xafe20, variante 2, 0xb0374)

Le minuteur du vote (`effects_c::ballot_timer`) ne prend que les états 0 (initialisation), 2 (décompte, puis fin
du scrutin et libération) et 6 (posé par le retrait hors écran) : aucune écriture de `+4` ne vaut 4 (recherche dans
`effects_c.rs` ; aucun scénario n'y passe). L'entrée 4 de sa table d'états mène à 0xb0374, qui efface le mot `+0x54`
de l'objet désigné par A6 (le registre que laisse le code exécuté avant le gestionnaire, sans rapport avec le vote),
rend la palette du minuteur (0x2531c0) et libère l'objet. Hypothèse : reste d'une version où la fin du scrutin
passait par un état propre, l'objet du scrutin étant tenu dans A6. Sans registres, A6 n'existe plus : l'entrée
est devenue un `unreachable!` (repli défensif ; l'A6 de l'original y désignerait un objet quelconque).

## Lot « couverture » (wt-couv2)

### Boss 15 et 19 : sous-états 8 et 0xa de l'état 4 « touché » (tables 0x929b8 et 0x90970)

* **Ce que fait le code.** L'état 4 de chacun des deux dragons aiguille par `+5` dans une table de six mots :
  0 réaction, 2 recul, 4 relevé (boss 15) ou réaction (boss 19), 6 mort, puis 8 et 0xa. L'entrée 8 est un `rts`
  (0x9152e pour le boss 15, 0x8fae2 pour le 19) ; l'entrée 0xa ramène au combat, marche vers un joueur
  (0x92ce0 → 0x9300c : `+0xa2` effacé, état 0x02020400, animation de repos ; 0x9101a → 0x913e8 pour le boss 19).
* **Pourquoi jamais atteint.** Les écrivains de l'état d'un de ces boss qui mènent à l'état 4 sont : les coups
  (`move.l #0x04000000,(4,A1)`, 0x10074 et les collisions 0x264e4...), la mort du boss 19 (0x9141a : 0x04060000),
  les écritures de `+5` dans l'état 4 lui-même (6 seulement : 0x9290c, 0x92a92 ; 0x90a5e, 0x90b18, 0x90b3e) et
  les retours à un état sauvé : `+0xb0` (0x9292e, seulement si l'octet sauvé vaut 2), `+0xa8` (0x90916, seulement
  0x0204 04xx) et `+0x8a` (0x92b04, 0x90c84), copie de l'état au moment du coup (0x10092, 0x263c2...). Aucun
  `addq.b #2,(5,A0)` dans les routines de l'état 4 (0x928ee-0x92ce0, 0x908ea-0x91010). Par récurrence, `+5` ne vaut
  donc que 0, 2, 4 ou 6 dans l'état 4. (Le constat du lot fuzz citait 0x9101a comme table du boss 19 : c'est
  l'entrée 0xa de la table 0x90970.)
* **Nature.** Entrées de table jamais choisies, mais un vrai comportement de jeu (retour au combat).
* **Décision.** Gardées en Rust comme l'original (`boss_15/hit.rs`, `boss_19/hit.rs` : `8 => {}`, `0xa => chase`),
  au lieu du `_ => {}` qui ignorait aussi 0xa ; au-delà de la table, `unreachable!`. Validées par `fuzz_couv`
  (`boss/boss15_scene46` et `boss/boss19_scene32`, 12 000 images, un appel sur 4, 48 essais par appel : environ
  14 300 essais chacun, les deux entrées sont atteintes par des essais sans écart).

### Interface des joueurs (F3) : 0x15234-0x1523e après le `bra.w` de 0x15230

* **Ce que fait le code.** `tst.b (0x21,A5) ; beq ; lea (0x44e,A5),A4` : choisir le compteur de pièces commun selon
  le nombre de monnayeurs.
* **Pourquoi jamais atteint.** Ces trois instructions suivent le `bra.w 0x1525c` de 0x15230 et aucun saut ni aucune
  table n'y mène (le listing ne les désassemble même pas) ; le choix du compteur passe par la table 0x1526e indexée
  par le mode des monnayeurs (`A5+0x1d`). Reste d'une version antérieure.
* **Décision.** Code mort de l'original, non réécrit (`hud::slot`).

### Corps des tâches K1, K2, F1, F4, F7, O1 : ce que les scénarios n'exécutent toujours pas

Mesure du lot (union des scénarios de la validation complète, docs/couverture-fonctions.md) ; les chemins atteignables
de F1, F4 et F7 sont désormais joués (`cov_demarrage_boutons`, `long_noms_embleme`) :

* **Drapeaux de mise au point de l'en-tête de la ROM** (octets 0xc4, 0xc5 et 0xc7 de l'espace données, nuls dans cette
  ROM ; le mot 0xc0, nul aussi, garde l'affichage des verrous du son) : 0x514 (`boot_init`), 0x5b4-0x5c0 et 0x5d0
  (`irq2_vblank` : attente d'un bit de 0x804021, copie de 0x8040b0), 0x664-0x686 (combinaison de touches lue au
  démarrage), 0x11bd2-0x11be0 (tâche de démarrage : autres textes d'avertissement). Inatteignables : la ROM ne change
  pas. `vblank.rs` les omet (commentaire d'en-tête) ; `startup.rs` et `modes/boot.rs` lisent l'octet 0xc4 comme
  l'original.
* **Registre 0x804030** (`reset`, 0x456-0x48a : lecture de la configuration 0x2dc9a, puis réinitialisation 0x2e8e4
  sur une combinaison de 0x804020, quand le bit 14 est nul) : la carte émulée rend 0xe021 (bit 14 levé ;
  `startup.rs`, `CARD`).
* **Erreurs de RAM** (K2, `ram_test_block`, 0x1940-0x19ae : écran « RAM ERROR » et boucle sans fin) : la RAM émulée
  ne se trompe jamais.
* **Données** prises pour du code par `decomp/index.tsv` : 0x980 (fin de l'ordonnanceur), 0x1fa2-0x1fb6 (table après
  `obj_on_screen`, O1), 0x19aa.
* **Écran 0x19068** (F4, 0x196ae et la fin de 0x19038) : tâche jamais créée, déjà consignée plus haut (« Tâche de
  l'écran 0x19068 »).

## Lot « sortie du Cpu », partie 5 (ESHL, SOBJ, SET, ITEM, BOX, EFFCT restants en gestionnaires sans registres, wt-familles3)

### Aides de la barrière en natives (0xb9ee6, 0xb9eba, 0xba094 ; `effects.rs`, module `barriere`)

Trois feuilles de l'effet « barrière » (EFFCT 0xb9f36) transcrites instruction par instruction sur le processeur :
« l'objet A1 est-il dans la boîte de A0 » (0xb9ee6, D0 = 1/0), « un joueur vivant touche-t-il la boîte » (0xb9eba,
lève `+0x30`), « tous les fils ont-ils fini » (0xba094, drapeau Z). Elles ne servaient qu'au code d'origine de la
barrière, désormais réécrite dans `effects_c` (`barrier`, `in_box`, interrupteurs) : plus aucun appel par adresse
ne les atteint (recherche de 0xb9ee6, 0xb9eba, 0xba094 dans `src/` ; seul `examples/fuzz_world.rs` les citait).
Doublon : supprimées, avec leurs entrées de `fuzz_world`.

### Guide de scène sans joueur vivant (ITEM 6, variante 1, 0xdcddc ; `items::guide_target`)

0xdcddc choisit le joueur vivant le plus proche ; s'il n'y en a aucun, l'original garde l'A2 de son appelant comme
« propriétaire ». Pour le guide 3 (attract), c'est la fin de sa copie de palette (0x91c320), écrite en clair. Pour le
guide 1, c'est l'A2 laissé par les gestionnaires précédents de la boucle (aucun n'est tenu sans registres). Le
guide 1 naît du script de la scène, joueurs présents ; aucun scénario ne le voit naître sans joueur vivant.
Hypothèse : cas impossible en jeu (repli de l'original sans sens). Devenu un `unreachable!`.

## Lot « sortie du Cpu », partie 7 (noyau et écrans, wt-cpu-core)

Le jeu n'a plus de banc de registres : les registres que l'original laisse et que seul un code non joué relirait
sont désormais des valeurs fixées, à noter ici.

* **Retenue X de l'horloge de partie (phase 12, 0x1c6a0).** Le premier `sbcd` du décompte de l'horloge prend comme
  retenue le drapeau X laissé par le code précédent : nul après l'addition du compteur d'images de partie
  (`A5+0x54`), sinon (compteur bloqué à 0xFFFF, après 18 minutes de partie) celui de la dernière instruction
  arithmétique exécutée, que les boucles d'aiguillage laissent nul (`lsl.w #2`). Aucun scénario n'atteint 0xFFFF
  images (les bots longs durent au plus 90 000 images, dont bien moins en partie continue). `game::phase12` prend la
  retenue nulle. Nature : effet de bord de l'original, sans conséquence visible sauf cas limite.
* **Octet haut de la statistique du personnage choisi (0x2e630, `modes::select`).** La routine reçoit D0.w dont
  seul l'octet bas est le choix ; l'octet haut est un reste des appels précédents. Elle n'agit que sur la carte de
  développement (`A5+NO_ACCOUNTING` non nul), jamais présente (bit 14 du port 0x804030 toujours à 1). L'octet haut
  est pris nul.
* **Natives retirées.** `config_variant` (0x2d648 : rend le bloc de valeurs par défaut en A6 et son déplacement en
  D0) : aucun appelant par adresse, les cœurs Rust prennent le bloc (`config::block_defaults`).


## Sortie du Cpu, partie 6 (joueurs, `player/`, `combat`, `creators`, `set_residue` ; wt-cpu-player)

### Replis par adresse des tables du joueur (`player/update.rs`, `scripted.rs`, `cinematics.rs`, `hurt.rs`)

* **Ce que faisait le code.** Quand une entrée des tables d'aiguillage du joueur (sous-états 0x3541c, cinématiques
  par scène 0x23826 et 0x23f4e, attente 0x23d66, réactions 0x3a0f4 et 0x3b820 et leurs tables de pas, pas des
  cinématiques de fin) n'avait pas de fonction Rust, `Ctx::rom` appelait l'adresse par `crate::call`.
* **Pourquoi jamais atteint.** Relevé statique (`cpup_tables`, scratchpad) : toutes les entrées des tables par scène
  et d'attente sont réécrites ; la table 0x3541c n'a que trois entrées (0, 4, 8), les octets suivants sont du code ;
  aucune native n'existe dans les plages 0x23000-0x25200 et 0x3a0f4-0x3bb52, donc le jeu sans 68000 s'y arrêtait déjà
  (`no68k::native`). Aucun de ces appels n'apparaît dans les compteurs des 308 scénarios (seules les baguettes
  `spawn_rod_*` et des sons étaient appelés par adresse depuis `player/`).
* **Nature.** Repli défensif. Devenus des `unreachable!` qui nomment la table.

### Tir de joueur de type inconnu (`combat/shots.rs`, table 0x267c4)

* **Ce que faisait le code.** Pour un type de tir hors 0..0x18, appel par adresse de l'entrée de la table 0x267c4
  (A0 = tir, A1 = victime, A3 = propriétaire, Z = le tir porte).
* **Pourquoi jamais atteint.** Les tirs de joueur (PSHL) n'ont que les types 0 à 0x18 (gestionnaires `pshl_0` à
  `pshl_24`) ; un autre type arrêterait déjà la boucle d'aiguillage. Repli défensif : `unreachable!`.

### Étincelle d'objet et bouffées de poussière dans la scène 41 (`common::item_spark`, `moves::dust_puffs`)

* **Ce que fait l'original.** 0xb10c8 ne crée rien dans la scène 41 et rend la retenue nulle ; l'appelant
  (0x3fb8c, 0x36cb8) ne teste que la retenue et écrit la hauteur à `(0xc,A1)`, A1 étant celui que le code d'avant
  laisse (l'enveloppe du son 0xe2430 sauve et rend tous les registres : c'est l'A1 de l'entrée de l'action).
* **Pourquoi non reproduit.** Aucun scénario ne le joue, et cet A1 vient de bien avant l'action (boucle des joueurs,
  combinaisons) : le Rust ne le connaît pas. L'ancienne version Rust écrivait à l'A1 du banc, qui n'était pas non plus
  celui de l'original. L'écriture est omise (le son est joué comme dans l'original). À reprendre si un scénario
  atteint ce cas (objet utilisé dans la scène 41).

### Pas de réactions jamais joués, réécrits (`player/hurt.rs`)

En rendant les replis par adresse `unreachable!`, `fuzz_couv` (cible `players_update`, états perturbés) est tombé sur
des pas des tables de réactions qui n'avaient ni fonction Rust ni native : le jeu sans 68000 s'y arrêtait déjà
(main compris), sans qu'aucun des 308 scénarios n'y passe. Relevé statique des tables de pas (`cpup_tables`) :
pas 0 des réactions 0x3a578, 0x3a890, 0x3a9da, 0x3a9f8 (0x3a59a, 0x3a5a4, 0x3a296, 0x3aa16) et pas 8 de 0x3a9f8
(0x3aa5e) ; pas 0xa de 0x3af44 (glissade jusqu'à l'arrêt, 0x3aff8) ; réactions entières 0x3b570 (étoiles, perte de
16 PV, 0x3b592/0x3b5b2/0x3b656), 0x3b72c (agenouillé sous six étoiles, 0x3b740-0x3b7aa), 0x3b7ba (0x3b7ca, 0x3b7e2) ;
au sol, 0x3b8a6 (0x3aa2e, 0x3aa5e) et 0x3b8c4 (0x3b8d6, 0x3b8e0). **Trou réel** : un joueur au pas lent tué par un
coup (`hurt_slow`, réaction 0x14) passe par 0x3b8a6 → 0x3aa2e, qui arrêtait le jeu. Tous transcrits de l'original
(glitch écrit en clair : réglage des étoiles de 0x3b7ca = D1.b, l'offset du pas) et validés par `fuzz_couv`
(`nom:players_update`, 10 scénarios, ~59 000 essais : plus aucune panique ni écart plausible ; avant : 163 paniques).
Les entrées au-delà de la fin des tables (0x3b880, 0x3b8a2, 0x3ba0e : milieu d'instruction) ne sont pas des pas.

## Lot « nettoyage » (wt-nettoyage)

### Coléoptère : parité de côté calculée puis ignorée (0x65efe, 0x65f18)

0x65efe calcule un côté (`+0xa4`, inversé selon la parité de `+0x2f`) et le passe en D0 à 0x65f18, qui teste sa
parité puis fait la même chose dans les deux branches. Aucun effet : le paramètre `side` de `cross_turn` (vide depuis
la réécriture) est retiré, le comportement est documenté dans `families/enemy_beetle.rs`. Hypothèse : tour à gauche ou
à droite prévu puis unifié.

## Lot L7 (ESHL, SOBJ, objets, caisses ; wt-l7-objets)

### Tables d'aiguillage lues pour les messages d'erreur (`items/`, `boxes/`, `items_b.rs`)

* **Code** : les gestionnaires ITEM (types 0 à 9), la barrière et le décor inerte (BOX 1 et 2), la caisse (BOX 0) et
  la pièce d'équipement (ITEM 2, `items_b.rs`) lisaient encore leurs tables `move.w (table,PC,Dn.w),D1 ; jmp` (par
  `effects::target` ou un `jump` local) uniquement pour calculer l'adresse qu'atteindrait l'original quand l'index
  n'a pas de bras Rust (`outside_table(target)`). Même chose pour les tables de routines d'effets des consommables
  (0xdafb8 : nourriture 0xdafd0 deux fois, potions 0xdb044 et 0xdb026, rien 0xdb068, compteur 0xdb06a), des effets
  spéciaux des caisses (0xda06a : créateurs ESHL 0xa1360, 0xa1a40, 0xa1670, 0xa17d0, 0xa1c6a, 0xa1360) et des
  gestionnaires d'emplacement de l'équipement (0xdb392, huit routines).
* **Pourquoi jamais atteint** : un index hors table ferait sauter l'original dans des données ou au milieu d'une
  instruction ; aucune donnée ne le produit (audit du lot B, 308 scénarios).
* **Décision** : les tables ne sont plus lues ; le cas reste un `unreachable!` par `effects::bad_index(index)`
  (`effects::target` et `outside_table` retirés). Les tables de routines deviennent un `match` sur l'index (variante,
  effet spécial, emplacement), l'adresse de chaque routine d'origine est rappelée en commentaire.
* **Nature** : repli défensif de la conversion.

### Lectures d'origine hors des tables, gardées

* `eshl_c/debris45` (variante 1, 0xa3dc2) : la hauteur est lue à `table + 2 + dx` ; avec un `dx` négatif, la lecture
  tombe dans le code qui précède la table. La table `debris45_place_offsets` commence donc 0x16 octets plus tôt
  (0xa3dac), comme `beacon_sheets` au lot L5.
* `items/scene` (guide, 0xdcbbc) : le mode 0x1e que donne le niveau 0 (`item_scene_guide_animations`) lit l'octet 0xdcbda, dans le code qui suit la table des animations
  d'arrivée (scénario `scene_end/end_01`) ; la table `item_scene_guide_animations_arrival` va jusqu'à 0xdcbdc.

Les deux cas apparaissent dans le résidu de la validation (une lecture chacun) avant l'extension des tables.

## Lot L11 (écrans ; wt-l11-ecrans)

### Noms d'adresses jamais utilisés (`modes/mod.rs`, `mod sub`)

* **Code** : le module `modes::sub` nommait six routines du moteur appelées par les écrans d'origine (0xc58, 0xca8,
  0xcc8, 0xce8 : effacements de la VRAM ; 0x1a67e : palette ; 0x26d4 : bloc de tuiles). Ce n'étaient que des
  constantes : les écrans appellent les fonctions Rust (`video::clear_objects`, `clear_layer`, `palette::…`,
  `video::load_grid`).
* **Pourquoi jamais atteint** : aucune référence dans le jeu ni les outils (le compilateur ne les signalait pas car
  le module était public).
* **Décision** : retirées ; les adresses restent dans les commentaires des fonctions Rust.
* **Nature** : doublon sans intérêt.

### Enregistreur de démonstrations du kit de développement (`modes/demo/recorder.rs`), gardé

* Avec le bit 15 du réglage `A5+0x7fd0`, la remise à zéro de l'enregistreur (0xe0538) prépare un enregistrement des
  entrées dans la RAM de la carte de développement (0x930000, absente d'une borne : les écritures s'y perdent).
  Comportement de jeu réel mais inatteignable sur une borne ; gardé en Rust, désormais par `hw::io::DevRam`.

## Lot L9a (boss 0 à 9 et `boss_common` ; wt-l9a-boss)

### Tables d'aiguillage des états des boss 7 et 8, lues pour choisir un bras

* **Code** : les deux guerriers noirs (boss 7, 0x859f8, et 8, 0x87fc0) choisissaient leurs états en relisant les tables
  `move.w (table,PC,Dn.w),D1 ; jmp` (une cinquantaine : groupes `+5`, pas `+6`, sous-pas `+7`) et en comparant
  l'adresse atteinte aux adresses des bras Rust. Elles deviennent des `match` sur l'octet d'état (outil
  `jt.py` du scratchpad : chaque bras prend les index dont l'entrée mène à son adresse ; un bras `_` garde les index
  restants comme avant). Deux pas partagent une même routine avec des tables différentes : l'esquive (`group6`,
  tables 0x868c8/0x868de et 0x88f5a/0x88f70, choisies par `+6`) et la ruée (boss 8, 0x88e1c/0x88e5e) : `match` gardé
  par la condition du choix de table. `leap_rest` (pas communs des bonds) prend le pas `+7` (2, 4, 6, 8) au lieu de
  l'adresse.
* **Pourquoi jamais atteint** : un index hors table ou impair ferait sauter l'original dans du code ou des données ;
  avant, ces cas tombaient déjà dans le bras `_` ou un `unreachable!` ; ils y restent (`effects::bad_index` pour les
  groupes de 0x85cc8 et 0x881aa et la mort 0x86ee6).
* **Lecture sans effet retirée** : la pose de l'attract du boss 7 (0x85dd6) relisait la table d'un seul mot 0x85de4
  par `+7` sans utiliser le résultat (l'original saute à son unique entrée) : la lecture est retirée.
* **Nature** : aiguillage de l'original, devenu un `match` (L13 traitera les tables d'aiguillage des classes).

### Tables d'entrée des décisions des boss 0 et 6

* **Code** : boss 0 (0x7aa16, 0x7aa22 : six entrées par moitié) et boss 6 (0x85734 : trois, 0x857ac et 0x8581a : six)
  sautent, selon la phase et le geste de l'IA, vers de petites routines qui tirent chacune dans une table de choix. Le
  port lisait ces tables pour retrouver la routine. Elles deviennent un tableau Rust indexé par `index / 2` (le
  numéro de la table de choix), ou rien pour 0x857ac dont les six routines tirent dans la même table (lecture sans
  effet retirée).
* **Pourquoi jamais atteint** : un index au-delà de la table (phase hors de 0 à 2) ferait sauter l'original au hasard ;
  `bad_index` comme au lot L7 (avant : `panic!` du boss 0, `unreachable!` ou repli du boss 6).
* **Nature** : repli défensif.

### Formes du corps du boss 3 (0x81ad4)

* `boss_common::push_out_of_boss` (0x81aa4) sautait par la table relative 0x81ad4 (formes 1 = 0x818e6, 2 = 0x819e2) ;
  `boss_3::body::push` prend désormais la forme (1 ou 2). Une forme 3 et au-delà (jamais posée : `push_players` ne
  pose que 1 et 2) ferait sauter l'original dans le code qui suit (0x88ad4) : `bad_index`, comme l'`unreachable!` qui
  précédait.

## Lot L9b (boss 10 à 22, dragon ; wt-l9b-boss)

Aucun comportement du jeu retiré. Seuls des restes du port, sans effet, sont supprimés :

* **Lectures d'aiguillage sans effet du boss 11** : `run` (0x8d5ca) et `dispatch` relisaient l'entrée de la table
  relative pour la ranger dans D1 (`setw(x, 1, …)`), mais D1 n'est pas un reste relu (`dispatch::Leftovers` : D2 à
  D5) : l'écriture ne faisait rien, la lecture est retirée ; les aiguillages deviennent des `match` sur l'octet d'état
  (`jt.py`). Un index hors table tombe comme avant dans le bras `_`.
* **Tables relatives à une seule cible utile** : boss 11, orientation des anneaux (0x8ed7a : l'entrée 0 fixe les
  angles, les autres entrées lisibles mènent hors du code des poses ; les lignes n'emploient que 1) et poses
  (0x8efa2, entrée 7 = 0x9031e, hors code) ; boss 19, tremblement (0x90ada, quatre entrées pour `t & 3`) : `match`
  sur l'index, le cas hors table gardant le bras `_` d'avant. Nature : aiguillage de l'original.
* **Lectures au-delà des tables gardées** (pas du code mort, glitches d'origine) : `boss_11/give_up_masks`,
  `boss_22/counter_chances`, `boss_13/stops` sont extraites avec le code qui les suit, que l'original lit comme
  des données ([fin.md](fin.md), « Résultat L9b »).


## Lot L12 (moteur et communs des familles ; wt-l12-moteur)

Aucun comportement du jeu retiré. Restes du port supprimés :

* **Accesseurs de ROM sans appelant** : `Cx::data8/16/32`, `rom8/16`, `rom_i16`, `ram8`, `set_ram8` et
  `Machine::prog8/prog32/data8/16/32` (plus aucun lecteur ; les données passent par les tables typées). Restent
  `Machine::prog16` (table des phases de `tasks::game`, lot L13) et les lectures du résidu (`Missing::pc_*`).
* **Constantes d'adresses de routines sans lecteur** : `families::cx::sub` (0x24a8, 0x24b0) et `families::rom`
  (0xfbae, 0x33fe4).
* **Copies locales** : `alloc` d'`ai_aids` et de `camera_modes` (désormais `Pool::alloc` et `Current::alloc`),
  accesseurs `u8/set8/add16…` et `rom16/data32` de `camera_modes::Cam` (le contexte est un `Cx` : champs de
  `scroll::Camera`), `prog8` local d'`ai_aids`, champs `ai::f`, `ai_aids::f`/`g` (vue `Enemy`).

Lectures de l'original hors des tables, désormais **nommées** dans `data/engine.json` (glitches d'origine gardés) :

* **Palettes depuis l'adresse 0** (`palette/from_zero`, 0x0-0x400) : une entrée nulle des pages de palettes d'une
  scène fait copier à l'original 32 palettes depuis l'adresse 0 (vecteurs de reset et début du programme).
* **Réglages du rang** (`rank/presets`, 9 lignes) : la table a 6 lignes (0x1f1d8-0x1f238) ; un numéro de ligne
  `A5+0x2a` au-delà de 5 lit les octets du code qui suit (relevé par les scénarios jusqu'à 0x1f25e).
* **Seuils de PV** (`ai/hp_thresholds`, 16 blocs de 0x100 octets) : une phase au-delà de la deuxième lit le bloc
  suivant (celui d'un autre ennemi), comme l'original ; les familles passent un numéro de bloc.
* **Boîtes de visibilité et marques au sol au-delà des tables** (`sprites/bounds_beyond`, `sprites/marks_beyond`) :
  un octet d'index au-delà des 128 boîtes et des 97 marques de `sprites/tables.json` lit les octets qui suivent (code
  compris) ; relevé : marques 112 et 224, boîte 158.
* **Grilles de la couche 2 hors des cartes** (`video/scroll2_grids`, `video/scroll2_tail`) : les chargeurs de blocs
  lisent des grilles que nulle carte de scène n'emploie (fonds des écrans), au-delà des bandes de `levels.json`.
* **Textes des sprites** (`sprites/texts@…`) : chaînes affichées en sprites, relevées par lecteur (`native_zones.txt`).

Clés du résidu restantes (toutes antérieures à L12, comptées par `validate`, aucune du moteur) :

* `anim entry` 0x1af22c, 0x1af354, 0x1af49a, 0x1b1fc2, 0x1b21c2, 0x1b21c3, 0x1b21c6, 0x1b21f6, 0x1bf902 et
  `anim set` 0x1b21c2, 0x1b670a, 0x1bf8fe : scripts d'animation incohérents de l'original (un pointeur d'animation
  pris dans des données qui ne sont pas des entrées ; leurs « images » ne sont pas des images) ; lot L1 ;
* `frame` 0x32003200, 0xb200b30, 0xb400b50, 0x23002310, 0x2ab02ac0, 0x7c0208 : pointeurs d'image formés de mots de
  tuiles par ces mêmes entrées incohérentes (l'image n'est jamais une vraie image) ; lot L1 ;
* `speed table` 0x0 : un objet sans table de vitesses lit la table 0 ; lot L1 ;
* `damage_crit_row` 0x3c84 : ligne de critiques lue au-delà de sa table (zone des joueurs, lot L10).

Ces lectures sont des glitches d'origine qu'aucune table ne décrit (adresses arbitraires) : elles restent comptées
par le résidu (fatales avec `DDSOM_STRICT=1`), lues sur le bus en mode ROM et dans `reste.json` sans ROM.
