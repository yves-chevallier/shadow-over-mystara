# Refactorisation : du Rust « en forme de 68000 » au Rust ordinaire

La conversion est finie : `port/ddsom-game` fait tourner tout le jeu en Rust, 269/269 scénarios VALIDE sans 68000 (le
jeu, mode par défaut depuis le lot 0a, [noyau.md](noyau.md)) et depuis les seuls assets ([sans-rom.md](sans-rom.md)). Mais le code
garde la forme de l'original. Cette phase le rend lisible, en fonctions réutilisables, puis rapide, **sans changer le
comportement** : effets de bord, glitches, bugs d'origine, tirages du hasard, ordre des écritures en RAM. La validation
compare la RAM image par image avec l'original ; elle reste le filet, et l'état du jeu reste donc dans la RAM émulée.

Règles pratiques, exemples et check-list : [guide-refactorisation.md](guide-refactorisation.md). Mesure :
`tools/measure_refactor.py` (forme 68000 par zone, `--doublons 6`, `--consts`), `examples/bench` (vitesse).

## 1. État des lieux (main 8b2612f)

150 000 lignes de code (177 000 avec les commentaires), dont 101 000 dans `families/` ; 2 046 natives (dont 536
enveloppes son générées). `tools/measure_refactor.py` compte les marques de la convention 68000 :

| Zone | lignes | regs | drap | adr | pile | duree | brut | a5 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| families | 101 033 | 3 656 | 341 | 3 742 | 19 | 377 | 1 104 | 1 003 |
| player | 14 262 | 199 | 15 | 338 | 0 | 30 | 295 | 64 |
| scene | 6 856 | 299 | 10 | 288 | 168 | 34 | 480 | 4 |
| modes | 4 667 | 361 | 0 | 227 | 139 | 18 | 847 | 134 |
| objects | 2 045 | 105 | 88 | 1 | 0 | 88 | 83 | 1 |
| reste (40 fichiers) | 21 028 | 1 359 | 241 | 180 | 135 | 489 | 1 832 | 105 |
| **total** | **149 891** | **5 979** | **695** | **4 776** | **461** | **1 036** | **4 641** | **1 307** |

(regs : `.d[n]`/`.a[n]` ; drap : `SR_C`, `cond()`, `flags_logic`... ; adr : appels par adresse `call`, `x.call`, `.jsr`,
`.rom`, `deleguer`, `tail` ; pile : `a[7]`, `Unwound`, `resume!`... ; duree : `cost`, `Chemin`, `walk_cycles`, `.pris(`... ;
brut : `rd*/wr*` ; a5 : décalages `A5 + 0x…` écrits en clair.)

### Ce qui rend le code difficile

1. **La convention d'appel du 68000.** Une native est `fn(&mut Cpu, &mut Cps2Bus)` : arguments dans D0-D7/A0-A6, résultat
   dans un registre et la retenue (`SR_C` : « pool plein », « script fini »), `rts` final. Un appel Rust → Rust passe par
   l'adresse d'origine : `x.call(0x2348)` empile une adresse de retour, remet les cadres de la tâche sur la pile 68000,
   cherche la native dans l'index, l'exécute, dépile (`call_at`, `tasks::table::mettre_sur_la_pile`). 4 776 appels ainsi.
   L'appelant relit ensuite `x.c.a[1]` et `x.c.sr & SR_C` : le type de retour est implicite et le compilateur ne voit
   rien.
2. **Dix contextes qui se ressemblent.** `families::cx::Cx`, `families::Ctx`, `player::Ctx`, `scene::Ctx`,
   `scene::exit::Ex`, `combat::World`, `ai::Mach`, `scroll::Exec`, `terrain::Push`, `objects::lists::Reinsert` : chacun
   redéfinit ses accès (`a5_u8`, `a5u8`, `ram8`, `v8`, `data16`, `prog16`, `random`...). Deux types `Obj`
   (`objects::Obj`, `combat::Obj`).
3. **Duplication.** 11 % des lignes de code (16 600) sont dans une fenêtre de 6 lignes présente dans un autre fichier
   (`families` 15 %, `player` 10 %) ; 299 adresses d'origine ou décalages sont déclarés en constante dans plusieurs
   fichiers, 1 491 déclarations (`0xe76c` ×26, `MOVE_TABLE` ×24, `SPEED_TABLE` ×23, `REINSERT` ×22, `ALLOC_SET` ×16,
   `FREE_BOSS` ×18...), parfois sous des noms faux (`ALLOC_BOX = 0x23a8` désignait l'allocation des BOSS) ; une vingtaine
   de copies de `alloc`, `free`, `random`, `players()`.
4. **Constantes magiques.** 1 307 décalages `A5 + 0x…` en clair, 4 641 accès bruts `rd16(b, …)`.
5. **Les durées du mode hybride** (supprimées au lot 0a, § 6). En mode normal, une native qui déborderait sur l'interruption suivante est remplacée par
   son original : chaque native annonce sa durée (`cost`, `exact_cost`), parfois exacte, calculée en suivant le chemin de
   l'original (`Chemin`, `walk_cycles`, `path_cycles`, exécution « à blanc » sur une copie de la machine :
   `families::Ctx::a_blanc`, `view::View::simulee`, `objects::cost`, `scroll::Exec::simulated`, `events::plafonnements`).
   1 036 marques, sans aucun effet sur le jeu sans 68000, qui ne les lit pas. Pire, elles couplent le code : une logique
   qui cesse d'appeler une routine par son adresse fait paniquer le calcul du chemin (« branchement sans décision »), vu
   pendant le pilote.
6. **Les replis vers l'original.** `deleguer` (59), bras `_ =>` qui appellent une adresse (68), `g.rom` des joueurs (301),
   `x.call(0x…)` vers du code qui est de toute façon une native. Sans 68000, une adresse sans native arrête le jeu : chacun
   est soit un appel de native déguisé, soit du code jamais atteint (à lister, pas à garder en repli).
7. **La pile et les cessions.** Une tâche cède en posant le PC sur une native de reprise (`resume!`, 349), les appels en
   cours sont des cadres (`tasks::table::Frame`) remis sur la pile 68000 quand du « code d'origine » reprend la main,
   `Unwound` abandonne les fonctions Rust quand un appelé cède ou revient plus haut (`epilogue_etranger` des bulles).
   Tout cela imite une pile qui n'a plus de raison d'être quand appelant et appelé sont en Rust.

### Ce qui doit rester

* **La machine de référence** (`cps2`, `m68k`, `ddsom-rc`, `recomp`) : l'oracle, hors du chemin du jeu.
* **`validate`** et ses diagnostics (`DDSOM_CHECK`, `DDSOM_ADOPT_REGS`, `examples/cessions`, `reste68k`, `watch`...).
* **L'état dans la RAM émulée**, aux mêmes adresses, avec le même ordre d'écriture, tant que la validation compare la RAM.
  Les vues typées (`Obj`, `Player`, `ram::*`) nomment cet état sans le déplacer.
* **La table des tâches Rust** et son contrôle contre FF0064, le calendrier de l'image du jeu sans 68000 (`no68k.rs`).
* **Les glitches** : allocation non testée qui écrit dans le premier emplacement ou après le pool, boucles qui insistent,
  registres résiduels relus. Ils sont marqués (`// Glitch d'original : …`), jamais corrigés.

## 2. Architecture cible

### Une routine = une fonction Rust ordinaire

```rust
// cœur : arguments et résultat Rust, état dans la RAM par des vues typées
impl Pool { pub fn alloc(&self, b: &mut Cps2Bus) -> Option<Obj> { … } }
```

Trois couches pendant la migration, une seule à la fin :

| Couche | Rôle | Disparaît quand |
|---|---|---|
| cœur `alloc(b) -> Option<Obj>` | la logique, sans registre | jamais |
| adaptateur `alloc_68k(c, b) -> Option<Obj>` | appelle le cœur et laisse les registres de l'original (A1, D6, drapeaux) ; sert aux appelants Rust pas encore prouvés indépendants des registres | `DDSOM_POISON` prouve les registres morts (voir le guide) : les appelants passent au cœur |
| native `Native { entry: 0x2348, run: \|c, b\| { adaptateur ; rts } }` | sert les appels par adresse qui restent (tables de la ROM, dispatch, autres natives) | plus aucun appel par adresse ni table ne la désigne |

Un appelant converti écrit `let Some(o) = x.alloc(&pools::SET) else { return };` au lieu de
`x.call(ALLOC_SET); if x.c.sr & SR_C != 0 { return } let o = Obj(x.c.a[1]);`. Les adaptateurs portent le suffixe
**`_68k`** : leur nombre mesure ce qui reste à prouver.

### Nommer l'état

* Champs d'objet : constantes associées de la vue (`Obj::X`, `Player::HP`) ou module `f` du type ; accesseurs typés quand
  un champ est lu partout (`o.depth(b)`).
* Variables de la RAM de travail : `ram::Byte/Word/Long/Table` (`src/ram.rs`). Les variables partagées sont dans `ram.rs`
  (scène, phase, caméra, joueurs, germe du hasard...), celles d'un sous-système dans son module (`hud::var`,
  `objects::ram`) avec les mêmes types.
* Adresses d'origine : une seule déclaration, dans le module qui possède la routine (`Pool::alloc_routine`), jamais
  recopiée ; elles disparaissent des appelants à mesure que les appels deviennent des fonctions.

### Les tâches qui cèdent la main

Aujourd'hui une reprise est une native posée à l'adresse que l'original reprend (`resume!`), et les appels en cours sont
des cadres. Cible : chaque tâche est une machine à états Rust,

```rust
enum Etape { Entree, Attente, Tour, … }               // une variante par point de reprise d'aujourd'hui
fn pas(t: &mut EtatTache, j: &mut Jeu) -> Attente    // Images(n) | Prete | Suspendue | Terminee
```

et le noyau appelle `step` au lieu de poser `c.pc`. Chemin : (1) un module de tâche range ses reprises en `match` sur une
`enum` (l'adresse d'origine devient `Step::address()`, gardée pour la comparaison de la table des tâches avec FF0064) ;
(2) `table::Resume` gagne une variante Rust (`Step`) à côté de `Cession(Contexte)` ; (3) les sous-programmes qui cèdent
(fondus, écran noir) deviennent des sous-machines appelées par `step`, leurs cadres des champs de l'état ; (4) quand plus
aucune reprise n'est une adresse, `Frame`, `mettre_sur_la_pile`, `Unwound` et `resume!` disparaissent. Le contexte de
registres d'une tâche (sens d'un fondu dans D1...) devient un champ nommé de son état.

### Quand le `Cpu` quitte le chemin du jeu

1. **D1 — un seul mode de jeu** (décision préalable, lot 0 ; fait au lot 0a, § 6) : le jeu sans 68000 devient le jeu ; le mode normal (natives
   et 68000 de repli, durées) ne sert plus qu'au diagnostic `DDSOM_CHECK`, avec une durée constante par native. Les
   durées exactes, `Chemin`, l'exécution à blanc et `View` disparaissent (1 036 marques).
2. **J1 — plus d'appel par adresse entre natives** (`adr` → 0 hors tables de la ROM et noyau) : les lots.
3. **J2 — plus de registre hors frontière** (`regs`, `drap` → 0 hors adaptateurs `_68k` restants) : `DDSOM_POISON` par
   routine, puis suppression des adaptateurs.
4. **J3 — tâches en machines à états** : le noyau n'écrit plus `c.pc` ; `no68k::step` devient un appel direct.
5. Alors `Native::run` perd son `Cpu` (`fn(&mut Jeu)`), les contextes perdent `c`, et `m68k::Cpu` ne sert plus qu'à la
   machine de référence. Les natives restantes ne sont que la table « adresse → fonction » des tables de la ROM
   (gestionnaires par type) et le contrôle `DDSOM_CHECK`.

L'état lui-même ne quitte la RAM émulée qu'après J3, structure par structure, avec une projection comparée par image
(hors de cette phase).

## 3. Découpage en lots

Les conflits de fusion viennent des fichiers communs : `lib.rs` (listes de natives), `families/mod.rs`, `families/cx.rs`,
`objects/`, `mem.rs`, `ram.rs`, `tasks/table.rs`, `kernel.rs`. Règle : **un seul agent touche les API communes** (lot 0),
les autres convertissent leurs appelants dans leurs propres répertoires ; un besoin d'API nouvelle se met d'abord dans le
module du lot (privé), puis remonte dans le socle à la fusion suivante.

| Lot | Périmètre | Dépend de | Taille |
|---|---|---|---|
| **0a** (seul, d'abord) | Supprimer les durées du mode hybride : champ `cost` réduit à une constante, `exact_cost`, `Chemin`, à-blanc de `families::Ctx`, `view.rs`, `objects/cost.rs`, `timing.rs`, chemins de `events`/`credits`/`config`/`palette`/`scroll`. Valider le mode normal avec une durée constante | — | ~1 000 marques, tous les fichiers (mécanique) |
| **0b** (seul, d'abord) | API du moteur d'objets comme le pilote `pools` : `anim` (start, step), `motion` (move_*, speed_table, move_by_table), `lists` (reinsert, unlink), `cull`, `rank` : cœur + `_68k` + méthodes de `Cx`/`Ctx` qui appellent l'adaptateur directement ; `random` unique ; `combat::Obj` fusionné dans `objects::Obj` ; contexte commun (trait d'accès mémoire ou base `Jeu`) | 0a | `objects/`, `families/cx.rs`, `families/mod.rs`, `rng.rs`, `ram.rs` |
| A | Familles d'ennemis et de boss : `families/boss_*`, `boss_common`, `enemy*`, `reactions` | 0b | ~30 000 lignes |
| B | Effets, objets, coffres : `families/effects*`, `items*`, `boxes`, `small`, `creators`, `effects_creators`, `effects_aids.rs` | 0b | ~30 000 lignes |
| C | Tirs et décors : `families/eshl*`, `set*`, `sobj`, `shell`, `player/`, `eshl_creators` | 0b | ~35 000 lignes |
| D | Joueurs et monde : `player/`, `combat/`, `combat.rs`, `ai.rs`, `ai_aids.rs`, `terrain.rs`, `scroll.rs`, `camera_modes.rs`, `placement.rs` | 0b | ~22 000 lignes |
| E | Tâches et écrans : `tasks/` (machines à états, J3), `kernel.rs`, `vblank.rs`, `scene/`, `modes/`, `hud/`, `credits_task.rs`, `credits_screen.rs`, `service/`, `eeprom*`, `game.rs`, `dispatch.rs` | 0a | ~20 000 lignes |
| F | Performance (après A-E, ou en parallèle hors `ddsom-game`) : rendu, QSound, bus | — | voir § 4 |

Ordre conseillé : 0a puis 0b (un agent, l'un après l'autre) ; puis trois agents en parallèle sur A, C, E ; puis B et D.
Chaque lot est disjoint par répertoire ; les seules lignes communes sont les listes `NATIVES` (une entrée par ligne :
chacun ne retire que les siennes). Dans chaque lot : suivre la check-list du guide, valider, committer par sous-ensemble.

## 4. Performance : base de mesure

`examples/bench <roms> <eeprom> <scénario> <images>` (jeu seul, sans machine de référence), main 8b2612f, 12 000 images,
temps de chargement des ROMs compris (~0,5 ms rapporté à une image) :

| Mode | `bot30k` | `cov_4p_a` | `attract` |
|---|---:|---:|---:|
| sans 68000, rendu de l'image compris (comme l'application) | 364 im/s | 357 | 377 |
| sans 68000, sans rendu, Z80 émulé | 2 810 im/s (356 µs) | 2 372 (422 µs) | 2 766 (362 µs) |
| sans 68000, sans rendu, pilote son Rust (`DDSOM_SON_RUST=1`) | 5 470 im/s (183 µs) | 5 438 (184 µs) | 5 895 (170 µs) |
| normal (68000 de repli, ×16), sans rendu | 2 654 im/s | | |

Profil (`perf record` sur `bench`, symboles par `addr2line` : le décalage du segment de code du binaire est de 0x1000) :

* **Avec rendu : `cps2_gfx::render` fait 92 % du temps** (`Layer::sample`, `pixel` : échantillonnage des couches pixel
  par pixel). `validate` rendait les deux machines à chaque image sans regarder l'image : `Machine::rendering = false` dans
  `validate` et `bench` (socle, sans effet sur l'état) divise le temps de validation (mesure dans § 5).
* **Sans rendu, pilote son Rust** : `qsound` (filtres `Fir::apply`, `update_sample`, voix) 72 % ; le jeu lui-même ~15 %
  (~25 µs par image) : accès au bus `Cps2Bus::read_word`/`write_word` (décodage d'adresse à chaque accès, 7 %),
  `latch_palette` 2 %, `Scenario::apply` 1,4 % (outil), `lookup` des natives (table de hachage SipHash) 1 %, gestion de
  la pile 68000 des tâches (`mettre_sur_la_pile`, `reprendre_de_la_pile`, `pop_return`, `type_id` de la table des
  tâches rangée en `Any`) ~1,2 %, `dispatch::Class::sweep`, `idle`.
* **Z80 émulé** : le programme du Z80 coûte autant que tout le reste (356 contre 183 µs).

Pistes pour le lot F, par gain attendu : rendu (couches en lignes de tuiles plutôt qu'en pixels, palette précalculée) ;
QSound (FIR vectorisé, ou mixage à la fréquence de sortie) ; dans le jeu, l'accès direct à la RAM de travail (tableau,
sans décodage d'adresse) puis la disparition du chemin `call`/cadres (J1, J3) et de `lookup`. Le jeu en Rust n'est pas le
goulot : l'optimiser avant les lots n'aurait pas de sens.

### Lot F : résultats (branche wt-perf)

Contrôle bit à bit : `examples/render_check <roms> <eeprom> <scénario> <images> [exit]` imprime, par image, le
hachage des pixels et celui des échantillons audio (jeu sans 68000, rendu allumé, Z80 émulé ou `DDSOM_SON_RUST=1`).
Référence enregistrée avec le code d'avant le lot, puis comparée avec `cmp` après chaque optimisation : attract,
bot30k, boss10_scene65, cov_4p_a, 4 000 images, deux modes de son, soit 8 séries, toutes identiques à chaque étape.
(`validate` ne calculant plus l'image, il ne vérifie pas le rendu.)

`bench` bot30k, sans 68000, pilote son Rust (12 000 images, une seule instance) :

| Étape | avec rendu | sans rendu |
|---|---:|---:|
| avant (socle) | 364 im/s (le rendu seul : 92 %) | 5 470 im/s |
| rendu par portions de tuile (une tuile lue par ligne) | 798 | |
| huit pixels décodés d'un coup (table d'éclatement des bits) | 1 285 | |
| ligne de tuile décodée puis recopiée en tranche | 1 626 | |
| FIR QSound en produits scalaires contigus | 1 862 | 11 408 |
| voix muettes, palette en tranches | 1 862 | 11 325 (bruit) |

Restent : couches 35 % du temps avec rendu (lecture des tuiles, `pixels8`), sprites 7 %, QSound ~45 % sans rendu
(110 ns par échantillon, la boucle des 16 voix domine), le jeu ~25 µs par image. Le bus (`read_word`/`write_word`)
et le `lookup` des natives pèsent moins de 5 % chacun : non traités, le gain ne vaut pas la complexité avant la
disparition du chemin `call`.

Validation complète (mode par défaut, 30 000 images) : 269/269 VALIDE.

## 5. Pilote : les pools d'objets, de bout en bout

Chaîne choisie : l'allocation et la libération des objets (`0x225e`-`0x2408`, `0x21d8`-`0x2250`), appelées de partout
(606 références dans 120 fichiers, 16 copies de `ALLOC_SET`, une vingtaine d'aides `alloc`/`try_alloc`/`spawn`).

1. **Cœur et adaptateurs** (`objects/pools.rs`) : `Pool::alloc`, `alloc_unchecked`, `alloc_or_leftover`, `release`
   (purs) ; `alloc_68k`, `alloc_unchecked_68k`, `release_68k` (registres de l'original, `diag::dead_regs`) ;
   natives inchangées pour les appels par adresse ; `Pool::alloc_routine`/`free_routine` (adresses déclarées une fois) et
   `pool_of_alloc_routine` pour les tables de la ROM qui désignent un pool par sa routine.
2. **Appelants** (scripts puis relecture) : tous passent par `x.alloc(&pools::SET)`, `x.alloc_unchecked(...)`,
   `x.free(&pools::SET)` ou `pools::SET.alloc_68k(c, b)`. 120 constantes recopiées et ~20 aides supprimées. Un nom faux
   corrigé (`ALLOC_BOX = 0x23a8` allouait un BOSS). Validation complète : 269/269 dans les deux modes (e1f8159).
3. **Preuve des registres** : `DDSOM_POISON=alloc,free` fait tout échouer à l'image 389 : la native `SOBJ_VARIANT` de
   la machine à modes rend l'objet créé dans A1 à son appelant (`modes/title.rs`) ; ses registres font partie de son
   contrat (guide, « Exemple réel »). Les sites des gestionnaires seuls (`DDSOM_POISON=gestionnaires`) : 265/269 ; les 4
   échecs (`ending`, `ending_end`, `cov_tour4_c3`, `cov_tour8_c1`) désignent un seul lecteur, l'orbe ESHL 0x34
   (`eshl_c.rs`) qui recopie la position depuis A1 après avoir créé ses trois éléments — glitch d'origine, rendu
   explicite par `alloc_or_leftover`. Ensuite 269/269 avec le poison, dans les deux modes.
4. **Fonctions pures** : les méthodes des gestionnaires (`Cx`, `families::Ctx`) appellent le cœur, sans registre. Les
   34 appels `_68k` restants sont dans des natives appelées par adresse (`(c, b)` des créateurs, `scene::Ctx`,
   `player::Ctx`, `combat`) : leur contrat de registres est à prouver par leurs lots.

Effet mesurable : `adr` 4 776 → 4 581, `drap` 695 → 643, `regs` 5 979 → 5 907, adresses déclarées plusieurs fois
299 → 288 (1 491 → 1 378 déclarations). Le temps de la validation complète passe d'environ 1 h 30 à 13 min (mode
normal) et 9 min 20 s (sans 68000) avec 10 tâches, grâce au rendu éteint.

## 6. Lot 0a : plus de durées, le jeu sans 68000 par défaut

D1 est appliquée. Le mode sans 68000 est le mode par défaut de l'application `ddsom`, de `validate`, de
`tools/validate_all.sh`, de `bench` et de `rom_reads` ; `DDSOM_HYBRIDE=1` choisit le mode hybride (natives et 68000
de repli), qui ne sert plus qu'au diagnostic et n'a plus à finir VALIDE (`DDSOM_SANS_68K` n'existe plus).

* `Native` perd `cost` et `exact_cost` ; en mode hybride chaque native dure `NATIVE_DURATION` (100 cycles), et sans 68000
  rien (`lib.rs`, `execute`). `Stats` perd `original_cycles`/`native_cycles` (contrôle `DDSOM_CHECK` : mémoire et
  registres seulement).
* Supprimés : `chemin.rs`, `timing.rs`, `view.rs`, `objects/cost.rs` ; l'exécution à blanc et le relevé de chemin de
  `families::Ctx` (`a_blanc`, `at`, `pris`, `externe`, `sans_chemin`, `jsr_indirect`, `cost`, couverture des
  décisions), de `ai::Mach` (`dry`, `seg`, `span`, tirage prédit, `refund`), de `scroll::Exec` (`Sim`, `mark`,
  `repeat`, `helper`) ; les fonctions de coût de chaque module (palette, vidéo, crédits, configuration, EEPROM,
  événements, son, entrées, objets, combat, texte, démarrage, service) et les arguments de coût des macros
  (`handlers!`, `entry!`, `native!`, `handler`, `fixed`...). Les fonctions écrites sur `View` (réelle ou simulée)
  prennent le bus.
* Le comportement ne change pas : seules des lectures sans effet et des décisions recopiées disparaissent (`ctx.pris(c)`
  devient `c`). Les exemples `families`, `fuzz_families`, `fuzz*`, `recensement` perdent la comparaison des cycles.
* Mesure (`tools/measure_refactor.py`, total) : `duration` 1 036 → 11 (restent des faux positifs : le prix d'un crédit,
  `cost`, dans `hud/slot.rs`, `sprites.rs`, `credits_task.rs`) ; lignes de code 149 834 → 147 861 ; au passage `regs`
  5 907 → 5 865, `drap` 643 → 642, `adr` 4 581 → 4 584 (`call` explicites qui remplacent `View::call`).

## 7. Lot E : les tâches en machines à états (J3)

Modèle et marche à suivre : [guide-refactorisation.md](guide-refactorisation.md), « Exemple : une tâche qui cède » ;
détail du noyau : [noyau.md](noyau.md), « Tâches Rust ».

* **Pas 1-2 (étapes, `Resume::Step`).** Chaque point de reprise est une variante d'une `enum Step` de son module
  (`steps!`), réunies dans `tasks::Step` ; `table::Next` (étape ou adresse d'une native) désigne la suite d'une
  cession et d'un appel en cours. L'ordonnanceur remet le contexte puis appelle le pas (`tasks::execute`) au lieu de
  poser `c.pc` ; une tâche neuve dont l'entrée est une étape démarre de même. Converties : gel, invites, écran des
  scores, menu TEST, interface des joueurs, crédits, fondus, partie (entrée, début de tour), portraits de la boutique,
  machine à modes (démarrage, attract, titre, choix, classement, démonstration, fin, fin de partie) et séquences de
  scène (sortie de scène, fondu au noir, transitions, bonus, boutique, personnages, comptes) : 349 `resume!` → 4.
* **Pas 3 (sous-machines).** Fondus et écran noir sont des fonctions Rust appelées directement
  (`Sequence::subroutine`, appel terminal) ; ce qu'ils gardaient sur la pile (sens d'un fondu dans D1, registres
  d'un `movem`, D0 d'une impulsion de verrouillage) est un champ de leur étape. Un `Frame` n'est plus qu'une suite :
  `Contenu::Regs`, `Value`, `garder`, `resume_step` ont disparu.
* **J1 dans le périmètre.** Plus d'appel par adresse aux enveloppes du noyau (`0x9b6`, `0xa14`, `0xb64`, `0xa40`,
  `0x9f8`, `0xae4` : `kernel::create_task`, `kill_task`, `tasks::one_frame`, `wait`, `finish`,
  `restart`), ni aux fondus et à l'écran noir.
* **Pas 4, ce qui reste (et pourquoi).** `resume!` : le pas de jeu `0x1c4b0` (la boucle d'image teste le plafond de
  pas, `cps2::StepCap`, sur le PC), les retours des phases de la démonstration `0x1f98c`/`0x20a98` (adresse de retour
  d'un `call_at` vers une phase qui peut céder), le fondu au noir `0x1460` (appelé par `call_at` par les objets SET,
  `families/set.rs`, `set_b.rs`) ; plus depuis le lot C, § 8). `Frame`, `mettre_sur_la_pile`, `Unwound` et `Resume::Cession` restent pour les
  appelants hors du lot : gestionnaires d'objets qui cèdent par `call_at` (fondu des SET, reprises `game.rs`
  `WORLD_RESUMES`/`PHASE_12_RESUME` et `dispatch.rs` `RESUME_*`, adresses de retour de leurs `call_at`), épilogue
  étranger des bulles (`families/mod.rs`, qui dépile des cadres remis sur la pile ; l'étape `Tour` qu'il atteint est
  retrouvée par `table::etape_depilee`), `families/effects_g.rs` (`table::call`). Ils disparaîtront quand les
  familles (lots A-C) ne céderont plus par `call_at`.

Mesure (`tools/measure_refactor.py`, zones du lot : `scene`, `modes`, `hud`, `tasks`, `service`, `eeprom`,
`credits_task`, `game`, `kernel`, `vblank`, `credits_screen`, `dispatch`) : lignes 18 724 → 18 693, `adr` 625 → 560,
`stack` 407 → 67, natives 389 → 49 ; `regs` (886), `drap` (23), `raw` (1 837 → 1 836) inchangés : le lot a porté sur
J3, pas sur J2.

## 8. Lot 0b : l'API du moteur d'objets

Fait sur le modèle du pilote (§ 5), décrit pour les lots suivants dans le guide (« L'API commune (lot 0b) »).

* **Cœurs, adaptateurs, natives** pour `anim` (start, start_entry, start_by_character, step), `motion` (ballistic,
  move_by_table, select_speed, suspended, tick_counters), `lists` (reinsert, unlink, unlink_all), `cull`
  (despawn_if_offscreen, in_arena, update_visibility), `rank` (reset, load_preset, tick, raise, lower) et `rng`
  (random, seed). Les natives n'appellent plus que la forme « registres » de l'adaptateur, plus `rts`.
* **Contexte commun** : trait `machine::Machine` (choisi plutôt qu'une base `Jeu` : les accès disjoints `x.c`/`x.b`
  ne survivraient pas à un `Deref`), implémenté par `Cx`, `families::Ctx`, `player::Ctx`, `scene::Ctx`,
  `combat::World` ; `random`, `prog*`, `data*`. Trait `objects::Current` pour les gestionnaires (`x.move_xz()`,
  `x.anim_step()`, `x.reinsert()`...), qui remplace les méthodes inhérentes de `Cx` et `Ctx` (et `Ctx::native`).
* **Un seul `random`** (`rng::random`) : copies de `camera_modes`, `player::Ctx`, `ai_aids`, `effects_b` supprimées ;
  ~90 sites `x.call(RNG)` convertis. **`combat::Obj`** fusionné dans `objects::Obj` (`from_word`, `word`, `long_ref`).
* **Appelants** : ~1 400 appels par adresse convertis par scripts (familles, joueurs, scènes, modes ; appels de lot E
  limités à une ligne). Lectures immédiates de drapeaux et de D0 repliées dans les valeurs rendues (`if x.anim_step()`,
  `x.move_xz().gt()` via `motion::LastAdd`, `let rng = x.random()`). Constantes d'adresse recopiées supprimées
  (dont des noms faux : `MOVE_Z = 0x1da6` déplaçait en X et Z, `MOVE_Y = 0x1dbc` en Z, `PLAYER_ARRIVES = 0x1f300`
  baissait le rang).
* **Glitches mis au jour** : durée d'animation nulle qui repart de 0xFFFF en levant la retenue (`anim::Step::Wrapped`,
  vu par `ending`) ; registres de la réinsertion relus par le gestionnaire suivant (`Current::reinsert` garde
  l'adaptateur).
* **Preuve** : `DDSOM_POISON` gagne les sites (`étiquette@pattern`, `!étiquette@pattern`, `#[track_caller]` sur
  adaptateurs et méthodes). Validation complète empoisonnée (`anim,motion,lists,cull,rank,random`), lecteurs isolés
  site par site, puis méthodes passées au cœur. Restent 88 appels `_68k` hors modules du socle (liste dans le guide) :
  natives appelées par adresse, aides à contrat de registres, lecteurs documentés.

Mesure (`tools/measure_refactor.py`, avant : b53a8b1 ; après : lot 0b fusionné avec le lot E de main, dont les chiffres
`stack` et `natives` de `scene`/`modes` viennent) :

| | regs | drap | adr | brut | a5 |
|---|---:|---:|---:|---:|---:|
| families avant / après | 3 608 / 3 453 | 305 / 190 | 3 573 / 2 064 | 1 104 / 1 101 | 1 003 / 1 001 |
| total avant / après | 5 865 / 5 714 | 642 / 530 | 4 584 / 2 976 | 4 579 / 4 567 | 1 307 / 1 300 |

Adresses déclarées dans plusieurs fichiers : 288 → 251 (1 378 → 1 101 déclarations) ; aucune adresse du moteur
d'objets n'y figure plus.

## 9. Lot D : joueurs et monde

Périmètre : `player/`, `combat/`, `combat.rs`, `ai.rs`, `ai_aids.rs`, `terrain.rs`, `scroll.rs`, `camera_modes.rs`,
`placement.rs`. Code mort et trous : [code-mort.md](code-mort.md), section du lot D.

* **Accesseurs propres supprimés.** `player::Ctx` perd `a5u8/a5u16/set_a5u8/set_a5u16` et ses `data*`/`prog*` (trait
  `Machine`) ; les variables de la RAM sont typées dans `player::ram` (`ram::Byte/Word`, celles de `crate::ram`
  réexportées ; nommées : `SPELL_ROW`, `CURSE_COUNT`, `SCROLL1_Y`, `FADE_*`, `CAMERA_FOCUS`, `BOSS_SIGNAL`, `DSW`...).
  `combat::World` perd `u8/u16/u32/set8/16/32`, `a5_8/a5_16`, `pc8/pc16` et son `random` à registres : vues `Obj`,
  variables partagées avec `player::ram`, `Machine`, files de collision prises dans `objects::pools`
  (`QUEUE_*`).
* **J1 dans le périmètre.** Aiguillage des actions des personnages en Rust (`characters::routine`, tables
  0x35918-0x35a48 relues dans la ROM ; `OVERRIDES` vide supprimée) ; décor (`terrain::push_68k`), placement et
  palettes d'effet (`placement`), secousse de caméra (`camera_modes::start_shake`, cœur + natives), créateurs
  d'effets joueur (`spawn::swap`, `weapon_trail`, `thrown`, `lightning_target`, `wall_target`), animations par
  variante : appels de fonctions Rust au lieu de `g.rom(address)`. Restent par adresse (`Ctx::rom`) : les sons, les
  palettes de `palette.rs`, les effets de `families`, `video::terrain` (0x24b0), déclarés une seule fois
  (`player::external`) en attendant leur API.
* **J2.** `DDSOM_POISON` (scénarios du lot, 90) a prouvé morts et fait passer au cœur : rang, animations,
  allocation (combat, ai_aids), vitesses, zone de jeu, tirage du combat ; registres du décor (`terrain`), du
  placement et des palettes (`placement`) ; D0/D1/A1 après les créateurs, secousses et animations convertis
  (étiquette temporaire). Le pas d'animation des joueurs suit maintenant la retenue de l'original (durée nulle :
  `Step::finished`, glitch d'origine).
* **Replis et trous.** 13 replis `_ => g.rom(address clean)` des automates (jamais atteints, parfois récursifs)
  deviennent `unreachable!`. Dix routines atteignables **sans native** (le jeu sans 68000 s'y serait arrêté) sont
  écrites en Rust et validées par `fuzz_player` contre l'original : actions 0x26, 0x2c, 0x30 (`moves::curse_drop`,
  `dust_puffs`, `box_recoil`) et sept créateurs (`spawn::cursed_weapon`, `dying_shards`, `storm_cloud`,
  `portal_glow`, `portal_sparks`, `portal_burst`, `rain`). D'autres entrées de tables des réactions restent à
  vérifier (code-mort.md).
* **Restent en `_68k` (et pourquoi).** `spawn::create` (A1 et retenue de l'allocation : contrat des créateurs
  appelés par adresse, relus par `items::lightning`/`wall`) ; `common::step_keeping_regs` (D0.b du pas d'animation
  relu comme variante par le créateur suivant) ; `terrain::push_68k`, `placement::place_on_free_ground_68k`
  (registres prouvés morts, mais ils appellent `video::terrain` par ses registres) ; `ai::Mach::random` (machine à
  registres des IA : D0 relu, `DDSOM_POISON=random@ai.rs` échoue à l'image 2742 d'`attract`).
* **Mesure** (`tools/measure_refactor.py`, zones du lot, avant c8107db / après) : lignes 20 722 / 20 878 (dix routines
  ajoutées), `regs` 756 / 755, `drap` 104 / 110, `adr` 354 / 285, `raw` 917 / 895, `a5` 138 / 78 ; adresses
  déclarées dans plusieurs fichiers du lot 43 / 0 (112 / 0 déclarations) ; lignes dupliquées (`--doublons 6`) de
  `player` 1 464 / 1 497, `combat` 38 / 30, `ai` 8 / 0 ; appels `_68k` 17 / 13 (les 13 restants sont décrits
  ci-dessus).
* **Pas faits** : `ai.rs`/`ai_aids.rs` (natives à contrat de registres appelées par les familles), `scroll.rs`,
  `combat` (`call_ao` vers effets et sons hors lot), `--doublons` des personnages.

## 10. Lot C : tirs et décors (`eshl*`, `set*`, `sobj`, `shell`, `player/`)

* **Fondu au noir des portes sans `call_at`.** Les portes SET 7 (`set.rs`, `exit_gate`) et SET 29 (`set_b.rs`,
  `door_b`) appellent `tasks::fading::descent` comme un sous-programme de tâche (`set::exit_fading`) : la suite
  est une étape `exit_fading::Step` (`tasks::Step::DoorSet`) qui porte la porte ; à la fin du fondu la tâche
  y reprend, puis revient dans la boucle de dispatch par l'adresse de retour du gestionnaire. Les fonctions Rust de la
  boucle sont abandonnées par `Unwound`, comme le faisait `call_at`. Disparaissent : `resume!(FONDU_DESCENTE, 0x1460)`
  (`tasks/fading.rs`) et les natives de reprise 0xcb896, 0xcb9c0, 0xd8f06. `fuzz_door` suit maintenant le fondu
  jusqu'au bout (étapes de la tâche) et couvre aussi la porte 29, que les scénarios n'atteignent pas
  (`DDSOM_PORTE=29`).
* **J1.** Plus d'appel par adresse vers les natives du lot : créateurs ESHL (`eshl_creators::call`, chaque
  créateur est une fonction publique `(c, b)`, la native n'ajoute que `rts`), routines de vague (`set_residue`),
  interprète du dialogue de la boutique (`peff_shop::talk_a/b`, natives 0x514ac/0x514cc retirées), création de
  la tâche de fondu (`kernel::create_task` au lieu de 0x9b6). Les adresses des natives d'autres modules appelées par
  le lot ne sont plus recopiées : `crate::camera_modes::SHAKE_A.entry`, `terrain::SHADOW`, `palette::*`,
  `text::CLEAR_BOX`, `aids::*`, `video::*`, `ai::KEEP_ON_SCREEN`, `scene::*` (60 constantes locales, dont des noms
  faux : `SAY_A`/`SAY_B`/`SAY_C`, `SOUND_THUD`, `AREA_DAMAGE`, `CAMERA_LOCK` désignaient les secousses de caméra
  0x2824/0x2860/0x289c ; `RUN_PALETTE_SCRIPT`/`PLAY_SCRIPT` l'effacement d'un rectangle de texte 0x1324).
* **J2, preuve à la sortie des gestionnaires.** Diagnostic (non gardé) : à la sortie de chaque gestionnaire du lot,
  les registres qu'il a changés sont empoisonnés. Validation complète : tous morts sauf trois lecteurs d'origine,
  documentés : D4 laissé par la copie de palette du SET 0 (`set.rs`, relu par 0xad7b0), D2 des effets PEFF 12-14
  (`peff_e.rs`, rebond du suivant), D3 de la gerbe de flammes PSHL 5 (`pshl.rs`, relu par un gestionnaire suivant,
  `boss11_scene42_mort` image 4447). Les écritures résiduelles des gestionnaires sans appel lisant des registres
  ont été retirées (`peff_ring`, `peff_b`, `peff_g`, `peff_l`, `peff_o`, `peff` ; aides `rand_mask` et `linked`
  sans registre).
* **Ce qui bloque le reste de J2.** (1) Les **appels de son** (153 sites) : chaque wrapper recopie D0-D7 et A0-A4
  sur la pile privée du son (`sound::sauvegarde_fantome`, copie morte restée en RAM, que `validate` compare) ; tout
  registre vivant au moment d'un son doit donc rester celui de l'original, et un gestionnaire qui joue un son garde
  ses écritures de registres. Il faudrait que `validate` ignore cette zone, ou une API de son qui prenne ces valeurs
  en argument. (2) Les **créateurs** (`eshl_creators`, `set_residue`, tremplins) : leurs registres de sortie (A1,
  retenue, compteurs) sont relus par les gestionnaires d'ennemis et de boss (lot A) et par les sons qui suivent ;
  empoisonnés, une trentaine de scénarios divergent. Ils gardent leur contrat.
## 11. Lot B : effets, objets, coffres

Périmètre : `families/effects*.rs`, `effects_creators.rs`, `items*.rs`, `boxes.rs`, `small.rs`, `creators.rs`,
`effects_aids.rs`. Code mort et replis : [code-mort.md](code-mort.md), section du lot B.

* **Créateurs écrits une fois** (`effects_creators::leftovers::Regs`). Chaque créateur (allouer, poser type,
  variante, réglage, lien) est une fonction générique sur un `Regs` : `Pure` (le cœur, aucun registre) ou le
  processeur (natives appelées par adresse et adaptateurs `_68k` : l'allocation passe par `Pool::alloc_68k`, les
  drapeaux du dernier `move`, les compteurs de boucle et les tirages vont dans les registres de l'original). Plus
  de seconde implémentation « à registres » à tenir à jour. Convertis : tous les créateurs d'EFFCT
  (`effects_creators.rs`), les étincelles et chocs (`effects_aids.rs`), les créateurs de `creators.rs`/`small.rs`
  appelés par le lot ou demandés par le lot D.
* **Cœurs pour le lot D** (appelants par adresse de `player/external.rs`, à brancher) : `effects_aids::shock`
  (0xb10c8, `DUST`), `effects_creators::halo` (0xab708), `blow` (0xb378a, `STARS`), `creators::set_a` (0xcc352,
  `WAND`), `creators::sparks` (0x4dc9a), `bolt` (0x4b7a0), `quake` (0x4b340), `small::quake_b` (0x4f132), chacun
  avec son adaptateur `_68k` (registres de l'original, étiquette `effects`). `HEALTH_BAR` (0x19dce) est dans
  `events.rs`, hors lot.
* **J1.** Plus d'appel par adresse vers les créateurs du lot, le décor (`terrain::push_68k`), la secousse
  (`camera_modes::start_shake`), la création de tâche (`kernel::create_task`), les éclairs et pages de palette
  du lot (`small`), la remise à zéro de scène (`creators::restart_scene`). Restent par adresse (pas d'API Rust) :
  les sons (enveloppes de `sound.rs`), `events` (0x19ae6, 0x19b08), `ai` (0xfbae, 0xfbb0, 0x103d6, 0x137e,
  0x1172e), palettes de `palette.rs`, vidéo, scène (0x33aa6, 0xf2e56, 0x20de6), 0x3403c (second décor). Les
  créateurs ESHL du lot C (fusionné) sont appelés comme fonctions Rust, y compris par la table des effets spéciaux
  des coffres (`match` sur l'adresse lue).
* **J2.** `DDSOM_POISON=effects` (registres des créateurs après les appels convertis) puis `DDSOM_POISON=leftovers`
  (registres laissés par les gestionnaires : restes des aiguillages `saut_table`, pointeurs de lecture, compteurs
  de boucle) : 270/270, écritures supprimées. Un lecteur trouvé : le nuage (type 26) relisait le mot haut de D0 de
  l'aiguillage (nul) après la remise en liste ; écrit en clair. Restent des lectures de registres réels : D2 hérité
  par `items_b::drop_old` (compteur de coups de la pièce rejetée, glitch d'origine), A2 de `guide_target` (aucun
  joueur vivant), A1 laissé par la boucle de dispatch (`effects_c`, adresse du gestionnaire), D0 des tirages
  (`effects_c`, `effects_d::random`), D4 hérité du radeau (`effects_e`), D6 de l'allocation dans la boucle des
  éclats (`effects_f`, glitch d'origine documenté).
* **Replis.** 68 aiguillages `_ => deleguer(target)` relevés table par table : état 6 (libération) explicite là où
  la table l'a, `outside_table` ailleurs ; deux branchements « pool plein » vers une erreur d'adresse de l'original
  (`address_error`).
* **Cession (lot E).** Le feu d'artifice (type 80) fait attendre la tâche de partie au milieu de la boucle de
  dispatch : ses reprises sont des étapes (`effects_g::Step`, ligne ajoutée à `tasks::task_steps!`) au lieu
  de natives à adresse et de `call_at` ; l'abandon de la boucle reste un `Unwound` (la boucle de dispatch n'est pas
  une machine à états). Nouveau scénario `cov/cov_effct_feu` (le chemin n'était exercé par aucun) ; ses lectures de
  la ROM manquaient aux assets sans ROM : cinq zones ajoutées à `port/extract/native_zones.txt` (fichier local
  généré) et `natives.json` des assets régénéré.
* **Nommer.** Variables de la caméra, de la scène et de la phase par `ram.rs` (85 accès) ; constantes d'adresses
  recopiées supprimées avec leurs appels.
* **Contrats des natives restantes.** `fuzz creators` (`examples/fuzz.rs`, mode hybride, contrôle) compare chaque
  créateur du lot à l'original sur des états tirés : 0 écart après correction de drapeaux que l'ancienne écriture
  oubliait (X du `lsr` du choc, `moveq #0,D0` de la colonne ESHL au pool plein, X de la moitié droite, `moveq` des
  douze blocs de palette, `move.b D1` du décor 11, table des décalages du joueur scripté lue en ROM).
* **Mesure** (`tools/measure_refactor.py`, fichiers du lot, avant 1f5ae77 / après) : lignes 16 022 / 16 104 ; `regs`
  729 / 425 ; `drap` 40 / 38 ; `adr` 266 / 124 (les sons, `events`, `ai`, palettes et vidéo hors lot) ; `stack`
  1 / 2 (`Unwound` explicite du feu d'artifice) ; `raw` 490 / 451 ; `a5` 263 / 174 ; adresses déclarées dans
  plusieurs fichiers touchant le lot 51 / 39 (72 / 47 déclarations du lot) ; lignes dupliquées (`--doublons 6`)
  221 / 223.
* **Restent en `_68k` (et pourquoi).** Les natives appelées par adresse (créateurs de `creators.rs`, `small.rs`,
  `effects_creators.rs`, `effects_aids.rs` : leur contrat de registres sert aux appelants hors lot), `regs_68k` de
  la boucle des éclats (`effects_f`, D6 de l'allocation relu, glitch d'origine), `terrain::push_68k` (pas de cœur,
  il appelle `video::terrain` par ses registres), `random_68k` des tirages dont D0 est relu (`effects_c`),
  `small::flash_68k` (drapeaux non prouvés morts), `start_by_character_68k` (`effects_f`, D6 sauvé autour).

## 12. Lot S : services communs (son, palettes, texte, terrain, aides, événements)

Périmètre : `sound.rs`, `palette.rs`, `text.rs`, `video.rs`, `aids.rs`, `events.rs`, `terrain.rs`,
`placement.rs`, `ai_aids.rs` (et `ai.rs`, `scroll.rs`, `camera_modes.rs`, laissés par le lot D), plus les
appelants hors des familles des lots A et B. API et mode d'emploi : [guide-refactorisation.md](guide-refactorisation.md),
« Les services communs (lot S) ».

* **La sauvegarde fantôme du son disparaît** (écart assumé, [son-68000.md](son-68000.md)). Trace des lectures sur la
  machine d'origine, 269 scénarios : la copie D0-D7/A0-A4 que chaque wrapper laisse sous le pointeur de sa pile
  privée (FF297E-FF2A19, trois niveaux d'aiguillage au plus) n'est relue que par les restaurations des wrappers et
  par le test de la RAM du menu TEST (0x2d096, qui remet la valeur lue). Elle était déjà dans la zone ignorée par
  `validate` (FF2900-FF2A19) : le blocage signalé par le lot C venait des registres que les sons **lisent** (A0,
  D4, A1), pas de la copie. `validate` n'ignore plus que la sauvegarde (`GHOST_SAVE`) : la fin de la file
  des sons, ses têtes et l'alternance des voix (FF2900-FF2961) sont maintenant comparées.
* **Cœurs.** Son : `play(b, sound, object)`, `play_global`, `play_with(b, sound, Source)` (valeurs `Option` : un son
  qui lit une valeur absente s'arrête), `service(b, Service::…)`, `emit` ; aiguillages et entrées « selon la
  version » dans le cœur ; la liste `sound::NATIVES` (inutilisée, en double) supprimée. Palettes : luminosité,
  éclairs (`Flash`), couleur d'éclair, palette du personnage. Texte : `print`, `erase`, `print_over` (un seul
  moteur `text`), `print_tall(_at)`, blocs, `clear_box`, qui rendent la fin du descripteur. Vidéo :
  `terrain(b, x, y) -> Terrain`, `terrain_of`, `terrain_68k`. Aides : `direction*`, `grabbed -> NameEntry`, `game_over`
  (sans appel par adresse : `palette::character_palette`, `player::actions::to_stand`). Événements : files de
  points, crédit, pop-ups (`Popup`). Terrain et placement : `terrain::push(b, o)`, `placement::place_on_free_ground(b,
  o)` (plus d'adaptateur `_68k` : regs de 0x24b0 pris below `Terrain`). IA : `ai_aids::stance`, `in_front`,
  `box_around -> InHitbox` (boîte : `Hitbox`). `ram` : `PLAYERS_ALIVE` (0x35, ex-`DIFFICULTE`), `SCROLL_Y_PLANE1`
  (0x4ba), `DSW` (0x7fd0), réexportés par `player::ram`. Les natives restent pour les appels par adresse et
  reproduisent les registres de l'original.
* **Appelants convertis** (hors familles des lots A et B) : ~330 appels de son (script `tools/convert_sounds.py`,
  puis à la main ceux qui lisent D4/A1 ou prennent une adresse calculée : la valeur posée dans D4 devient l'argument),
  ~110 appels de palettes, textes, directions, terrain, événements, fin de partie. Noms faux corrigés : les
  « scripts d'animation de palette » de la phase 0 (`game.rs`, 0x1324) effacent des rectangles de texte
  (`TEXT_BOXES`) ; `DIFFICULTE`.
* **Registres gardés, et pourquoi.** `pshl::stopped_by_terrain` (et, par prudence, `eshl.rs`, `shell.rs`) gardent les
  registres de 0x24a8 (`terrain_68k`) : D3 est relu par un gestionnaire suivant (`boss11_scene42_mort`, image 4447,
  la divergence est apparue en passant au cœur) ; `peff_e.rs` : D2 du terrain relu par le rebond du suivant
  (lot C). `combat.rs` (`play_with` des aiguillages 0xe662a/0xe823a : D4 posé par l'appelant pour la cible, peut-être
  un `ByD4`). Les tours de `peff_m`, `peff_n`, `pshl_d` lisent encore leur cible dans A1 (posé par l'appelant).
* **Glitch mis au jour.** La barre de vie (0x19dce) laisse A2 sur la file des pop-ups, et `player_hits_enemies`
  garde A2 comme boîte d'attaque du joueur pour les ennemis suivants : après un coup qui affiche une barre, les
  ennemis suivants de la liste sont testés contre la file des pop-ups (`combat::show_enemy_hp`, marqué ; vu en
  passant au cœur `events::create_popup` : une dizaine de scénarios divergeaient).
* **Fuzz.** `fuzz sound` (wrappers, aiguillages hors sélecteurs aléatoires, services) et `fuzz video` sans écart ;
  `fuzz u1` sans écart sur les natives du lot (un écart antérieur, hors lot : D6 de `popups_clear_all`,
  `popups.rs`, 0x0000ffff attendu) ; `fuzz_world ia_` (nouveau : 0xa470, 0x7f0c, 0x7ef4, 0x7f19c) : mêmes écarts
  avant et après (drapeau X seulement, que ces natives n'ont jamais reproduit).
* **Mesure** (`tools/measure_refactor.py`, avant 44d7794 / après) : total `adr` 2 896 / 2 347, `regs` 5 596 / 5 365,
  `drap` 536 / 522, lignes 148 020 / 148 078 ; `player` `adr` 263 / 71, `scene` 257 / 173, `modes` 185 / 158,
  `families` (lot C seulement) 2 053 / 1 839 ; modules du lot : `sound` `regs` 54 / 24, `aids` 46 / 28,
  `events` 43 / 35 (`drap` 30 / 18), `placement` 16 / 8, `ai_aids` 273 / 249 ; adresses déclarées dans plusieurs
  fichiers 214 / 196 (940 / 872 déclarations). Les `_68k(` passent de 116 à 133 : les adaptateurs du lot portent
  maintenant ce nom (`terrain_68k`, `enqueue_68k`...), alors que les natives les contenaient sans le dire.
* **Pas faits.** `ai.rs` (machine à registres des IA), le reste de `ai_aids.rs` (une quarantaine d'aides à contrat
  D0/Z appelées par les familles des lots A et B : tirages, butin, débris, saisie...), `scroll.rs`,
  `camera_modes.rs` ; les appels de services dans les familles des lots A et B (à faire par ces lots avec l'API).

## 13. Lot A : ennemis et boss

Périmètre : `families/boss_*`, `boss_common.rs`, `boss_dragon.rs`, `enemy*`, `reactions.rs` (~56 000 lignes de code).

* **Façade `enemy_lib`** (alias `lib`, accessible partout par `enemy_common`) : chaque routine d'un autre module que
  les familles appelaient par adresse n'est plus appelée qu'à un seul endroit, une fonction Rust nommée : entrées en
  arguments, résultat en valeur Rust (`lib::target_in_front -> InBox`, `lib::attack_hits_a_player -> Option<Obj>`,
  `lib::turn_decision -> Turn`, `lib::maybe_draw_6e -> bool`, `lib::decision(x, même_côté)`...). Les appelants ne
  lisent plus ni Z, ni la retenue, ni D0. Après la fusion des lots S, B et D, la façade appelle leurs cœurs (son,
  événements, `ai_aids::stance/in_front/box_around`, `effects_creators::attack/debris`, `aids::direction*`,
  `aids::grabbed`, `video::terrain_of`) ; restent par adresse dans `enemy_lib` les routines sans cœur (le reste de
  `ai.rs`/`ai_aids.rs` : tirages 0x788c/0x79ce/0x79f6/0xac3a/0xf118, PV, immunités, cibles, saisie 0xfc44/0xfd64/
  0xfd8c/0xfda2, butin, débris 0x1172e/0x11a36, poussée ; secousses de `camera_modes.rs` ; quelques créateurs).
* **Noms faux corrigés** (vérifiés par valeur) : `FACE_TARGET = 0xa470` tirait l'attitude (`lib::draw_stance`),
  `TURN_TO_TARGET = 0x788c` un octet de table (`draw_6b`), `SOUND`/`SHOCK = 0x2824` secouait la caméra
  (`shake_54`), `SHADOW = 0x33fe4` est la collision avec le décor (`collide_terrain`), `boss_common::facing_target`
  testait le sens de la vitesse (`moves_forward`), `target_in_box_far = 0x7f0c` teste une cible en garde
  (`guard_in_front_strict`), `SPEED_Y = +0x16` est l'accélération en X (`Obj::AX`), `ANGLE = +0x2d` l'index de
  la table de vitesses, `DIFFICULTY = A5+0x7f92` le rang dynamique (`var::RANK`).
* **État nommé** : champs génériques par `Obj::X`, `Obj::VX`... et `f::SUB3`, `f::KEY_FRAME` (15 copies des mêmes
  modules de champs supprimées) ; variables de la RAM de travail typées (`enemy_common::var`, `ram::Byte/Word`).
* **Natives du périmètre en fonctions Rust** : 0x27ee (détachement de l'hôte), 0x81aa4, 0x833b6, 0x87048
  (`boss_common`), 0x81ae0 (riposte du boss 3), réactions communes (`reactions::run_common`, appelées directement
  par `enemy_common::react`, `react_with` pour la routine propre d'un boss) ; leurs natives ne servent plus qu'aux
  appels par adresse restants.
* **Routines d'origine sans native** appelées par les familles, jamais atteintes (le jeu sans 68000 s'y serait
  arrêté) : transcrites dans `enemy_lib` ([code-mort.md](code-mort.md), lot A).
* **J2 : registres résiduels.** Les écritures de registres que plus rien ne relit en Rust (copies des registres de
  l'original, ~800) ont été instrumentées une à une (`dead_regs`, une étiquette par site). La validation
  empoisonnée, puis des bissections automatiques sur les sites (outil de travail : `DDSOM_POISON=lota@file:line:`),
  ont désigné les écritures relues par la suite : un gestionnaire suivant (A1 de `load_target`, D1-D4 du boss 15,
  D5 de l'archer, D2 du boss 6...) ou un compteur de boucle relu au tour suivant. Elles restent, marquées
  `// relu : …` ; 725 autres sont supprimées. `boss_1` et `boss_9` (lecteurs nombreux) gardent leurs registres.
  Piège : une écriture dans une boucle peut être relue au tour suivant (lecture placée avant dans le texte).
* **Besoins d'API** (signalés) : cœurs pour le reste de `ai_aids.rs` et `ai.rs` (tirages, cibles, PV, immunités,
  saisie, butin, débris), les secousses de `camera_modes.rs`, les créateurs 0xd5f30/0xd6952/0xd5ef8 (SET de la barre
  de vie), `popups::clear_all` (0x19eb0), 0x57c66/0x5cdc6 (`creators.rs`, privés) ; `ram::DIFFICULTE` était le
  nombre de joueurs en vie (corrigé au lot S).

Mesure (périmètre du lot, `tools/measure_refactor.py` restreint aux fichiers du lot ; avant c8107db / après) :

| | lignes | regs | drap | adr | brut | a5 | `_68k(` | doublons (6 lignes) | adresses déclarées aussi ailleurs |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| avant | 56 744 | 1 729 | 111 | 1 423 | 213 | 426 | 7 | 13 464 | 70 (294 décl.) |
| après | 55 330 | 973 | 21 | 161 | 191 | 191 | 12 | 12 579 | 11 (18 décl.) |

Des 161 `adr` restants, ~100 sont les appels de la façade `enemy_lib` (un par routine d'un autre lot) ; les autres :
tables de la ROM (réactions propres à un type, créateurs par table), quelques créateurs appelés une fois, natives
`(c, b)` du périmètre. Les `_68k(` gardés : `random_68k` (D0 relu), `terrain_68k` (registres de 0x24a8 relus),
`shock_68k` (A1, D1, retenue relus), `in_arena_68k` (`bhi` de `reactions.rs`).

## 14. Lot « noyau final » : J3 achevé, l'ordonnanceur en Rust

Périmètre : `dispatch.rs`, `game.rs`, `tasks/`, `kernel.rs`, `vblank.rs`, `no68k.rs`, `startup.rs`, `lib.rs`,
`families/mod.rs`, `families/cx.rs`, et les reprises restantes de `modes/` (boucle, démarrage, démonstration),
`families/set.rs` et `effects_g.rs` (cessions des gestionnaires). Détail : [noyau.md](noyau.md), « Ordonnanceur et
traps », « Appels en cours en Rust », « Le jeu sans 68000 ».

* **Plus de cession par la pile 68000.** Les appels en cours de la boucle de partie sont des étapes : fin d'image
  (`game::Step::FrameEnd`, ex-native `0x1c4c4`), fin de la phase 12 (`game::Step::AfterWorld`, ex-`PHASE_12_RESUME`),
  mise à jour du monde (`game::Step::World { order, step }`, ex-`WORLD_RESUMES`, 46 natives), boucles d'aiguillage
  (`dispatch::Step::Dispatch`, ex-`RESUME_*`, 10 natives), retours des phases de la démonstration
  (`demo::Step::AfterPhaseA/B`, ex-`resume!` `0x1f98c`/`0x20a98`, et `AfterWorld` `0x1fa4a` pour sa phase 12), boucle
  de la machine à modes (`modes::Step`, ex-natives `0x133fe`, `0x1341a`, `0x13422`), entrée de la tâche de démarrage
  (`boot::Step::Entry`, ex-native `0x11b16`), pas de jeu (`game::Step::Step`, ex-`resume!` `0x1c4b0`).
* **Céder au milieu d'une boucle Rust** sans abandon par panique : une boucle qui appelle un gestionnaire puis
  continue ouvre un *appel attendu* (`table::call_expected`, `crate::call_expected`) dont le `rts` lui rend la
  main ; si le gestionnaire cède (porte : `exit_fading::fade_to_black` ne diverge plus ; feu d'artifice), la
  cession est notée (`tasks::has_ceded`, remise à faux à chaque point de premier niveau), les `rts` suivants ne font
  rien, et chaque niveau rend `Flow::Yield` ; la tâche reprendra par les cadres. L'épilogue étranger des bulles
  dépile quatre cadres (`table::pop_calls`, A1-A4 = leurs adresses d'origine) et revient au cinquième ; observé
  aussi dans la démonstration de l'attract (`modes/attract`, images 16 857 et suivantes), d'où l'étape `AfterWorld`
  de sa phase 12.
* **Disparaissent** : `Unwound` (et le `catch_unwind` de chaque native), `call_at` (`Cx::call_at` ; `Cx::tail` est un
  `call`), `resume!`, `Resume::Cession`, `table::mettre_sur_la_pile`, `relever`, `etape_depilee`,
  `reprendre_de_la_pile`, `vers_le_68000` et la liste `remis`. `Frame` reste, comme pile des suites d'une tâche (sa
  hauteur de pile 68000 ne sert plus qu'à distinguer un appel par adresse, qui empile une sentinelle) ; il gagne
  l'attente (`expected`). `crate::call` n'admet plus de cession ni de retour plus haut : une panique nomme le site
  (aucun cas dans les 272 scénarios).
* **Le noyau ne pose plus de PC.** L'ordonnanceur (`0x928`-`0x984`) et le démarrage (`0x350`-`0x50a`) sont une machine
  à états (`kernel::Point`, rangé dans la table des tâches) que `no68k::line` fait avancer d'un point à la fois ;
  l'interruption de VBLANK est une fonction Rust prise entre deux points (`vblank::interruption`). Une tâche est
  lancée en appelant le pas de son étape ; toutes les reprises et entrées sont des étapes (l'écran mort `0x19068`
  n'en a pas). Le plafond des pas est compté par le jeu (`no68k::game_step`, `Point::StepKept`) au lieu du test
  du PC (`cps2::StepCap` reste pour la machine de référence). Natives retirées : les cinq de l'ordonnanceur, les dix
  des handlers de traps et de leur retour (`trap #12` change directement le masque), le VBLANK, les cinq du démarrage.
* **Mode hybride retiré** (D1 jusqu'au bout) : avec des tâches lancées par un ordonnanceur Rust et des appels en cours
  hors de la pile 68000, le code d'origine ne peut plus prendre la suite d'une native. `install` = jeu sans 68000,
  `DDSOM_HYBRIDE=1` arrête les outils avec un message (`refuse_hybrid`). La machine de référence (`install_reference`)
  garde son 68000 et n'a pas changé ; `DDSOM_CHECK` et `DDSOM_ADOPT_REGS` (original sur une copie) marchent toujours.
  `examples/cessions` et `reste68k` (68000 restant du mode hybride) n'ont plus d'objet.
* **J1 dans le périmètre** : la mise à jour du monde et la fin de phase appellent directement `player::update::update_all`,
  `player::overlays::update`, `combat::combat_update`, `rank::tick_68k`, `aids::game_over` ; la fin de la phase 12 est
  commune au jeu et à la démonstration (`game::after_world`). Restent par adresse (corps privés d'autres modules) :
  jauge `0xb3e`, sprites `0x1de96`, caméras `0x33e20`, pop-ups `0x19e20`, script `0xf2e5c`, entrées `0x2aec`, et les
  quatre services du VBLANK (`0x80c`, `0x68c`, `0x143da`, `0x2dcf6`).

**Sortie du `Cpu` (étape 3) : pas atteinte, ce qui bloque.** `Native::run` garde `(c, b)` et les contextes leur `c` :
(1) le **PC** sert encore aux sauts d'une native à une autre au sein d'une tâche (`Sequence::jump`/`jump_to`/`tail`,
~25 sites de `modes/`, `scene/`, `tasks/` ; `tasks::return_to_caller` vers une `Next::Address`, retour d'un `crate::call` ;
`ai_aids::landing_by_kind`, `jmp (A1)`) et aux appels par adresse (`crate::call`, 929 `adr`, dont ~430 dans
`families`) ; (2) les **registres** : 4 269 accès (`regs`) dont 2 195 dans `families`, 349 `modes`, 282 `scene`, 249
`ai_aids`, 186 `player`, 172 `ai` ; les registres résiduels relus d'une routine à l'autre (bilans des lots C, B, S, A :
boucle de dispatch A0/D7/D0/A1, `reinsert`, `random_68k`, `terrain_68k`, créateurs, boss 1 et 9...) et ceux que le
noyau transmet aux tâches neuves et que les cessions rangent dans le contexte ; (3) la **pile 68000** des appels par
adresse (sentinelle) et de l'interruption (FF0E66 et piles des tâches, ignorées par `validate`). Une structure
`Regs` portée par les contextes n'a de sens qu'une fois (1) fait : elle remplacerait le banc `Cpu` à l'identique.

Mesure (`tools/measure_refactor.py`, avant b30905d / après) : `stack` total 117 / 81 (`tasks` 22 / 14, `kernel` 17 / 3,
`no68k` 10 / 3, `families` 18 / 6) ; natives déclarées 330 / 317 (sans compter les 46 + 10 natives de reprise
générées) ; `regs` `kernel` 47 / 33, `no68k` 9 / 3 ; `adr` total 943 / 929 (`game` 15 / 14 : les appels directs
remplacent `call` mais le compte inclut les commentaires d'adresse) ; lignes 146 821 / 146 702. Écritures de `c.pc` :
`kernel` 11 / 0, `no68k` 2 / 0, `startup` 5 / 0, `vblank` 1 / 0, `families/mod.rs` 3 / 2 (repli hors jeu de
`jsr_origine`), `tasks` 2 / 2 (`return_to_caller` vers une adresse ; `execute` pose l'adresse d'origine de l'étape :
un pas qui rendrait la main sans céder ni sauter s'arrêterait sur « pas de native »).

## 15. Lot IA : aides d'IA et derniers services

Périmètre : `ai.rs`, `ai_aids.rs`, `camera_modes.rs` (secousses), `scroll.rs`, `popups.rs`, créateurs privés (SET
0xd5f30, 0xd6952, 0xd5ef8 ; 0x57c66, 0x5cdc6), initialisation des pools (0x2062, 0x2134, 0x2150, 0x21bc), appels
des événements (0x19ae6, 0x19b08, 0x19dce) et des palettes (0x1686, 0x3af8), plus leurs appelants dans les familles,
`player/`, `scene/`, `modes/`, `combat.rs`.

* **Cœurs.** Chaque aide d'IA est une fonction Rust sur l'objet (`ai::pick_target(b, o, Rule) -> Option<Obj>`,
  `target_by`, `random_player`, `initial_hp`, `set_immunities -> u16`, `hp_rank`, `decision_a/b -> bool`,
  `pick_byte`, `target_in_front(b, o, Hitbox) -> InHitbox`, `facing_target`, `target_faces_me`, `back_to_target`,
  `turn_decision -> Turn`, `moving_forward`, `keep_on_screen/keep_in_zone -> bool`, `turn_toward`, `drift_heading` ;
  `ai_aids::` points de marche, prises, tirages par tables, parades (`Parry`), débris, butins, éclair et secousse,
  `landing_routine`). La machine à registres `ai::Mach` (opérations à drapeaux exacts) et les ~60 natives des deux
  modules sont supprimées : plus aucun appel par adresse ne les vise (code-mort.md, lot IA). Autres cœurs :
  `creators::set_14`, `set_15_b`, `in_right_half`, `chance_10_16`, `set_residue::attach_life_bar`,
  `popups::clear_all`, `popups::animer`, `effects_creators::impact_spark`, `terrain::push_collides`,
  `camera_modes::shake::of_routine`, `camera_modes::update_camera_1/2` et `scroll::update_cameras` (appelables
  directement, encore sur le processeur : les caméras appellent des natives vidéo par adresse).
* **Appelants.** La façade `enemy_lib` n'appelle plus que des cœurs ; ses doublons Rust du lot A (`outside`,
  `facing_target`...) sont fondus avec eux. Familles (`eshl*`, `items*`, `effects*`, `small`, `player/`), joueurs,
  scènes, modes, combat : secousses par `start_shake`, zone et écran par `ai::keep_*`, animations de l'IA par
  `enemy_lib::start/step`, événements par `events::enfiler_*`/`create_popup`, palettes par `palette::full_brightness`
  et `placement::load_effect_palette`, pools par `Pool::reset`.
* **Preuve (J2).** Les appels par adresse ont d'abord été sondés (`ai::sonde`, diagnostic retiré depuis) : après
  chaque native, les registres qu'elle avait changés et ceux que l'appelant avait posés pour elle étaient
  empoisonnés (`DDSOM_POISON=ia_<address>`, un registre à la fois : `ia_<address>.d2`), validations complètes puis
  bissections par étiquette et par site. Lecteurs trouvés, écrits en clair (glitches d'origine) : index de réaction
  des boss 7, 8, 19 et 3 (octet haut de D0 laissé par 0x10112, 0xb648, 0xfe5a), `+0x8e` de la harpie (D0 de 0xfe5a),
  D5 de la zone et du décor relu par le squelette (`zone_and_terrain -> bool`), côté de la boîte relu dans D0
  (squelette, golem), D2 de 0xb5e0 relu par le boss 11 (restes des tests de boîte, voir plus bas), D2/D3 de
  `drop_rewards` donnant leurs vitesses à l'étincelle de 0x103ba (`flash_and_shake`), A1 des cibles (boss 5, 11,
  harpie : les cœurs rendent la cible), D2 de 0x43ae (boss 10), D0/A1 de la routine d'atterrissage (0x3e480).
* **Après fusion du lot « trous » (scénarios longs).** Trois scénarios nouveaux ont montré des registres que la
  preuve sur 272 scénarios n'avait pas atteints : D5 traverse les gestionnaires jusqu'à la rangée d'éclats d'une
  caisse (0xa17d0, glitch de `row_1a`) : tests de zone (`ai::keep_on_screen_d5`), cibles (D5 = 0 / 2), débris
  (D5.w = -1) et trésor (compteur) l'écrivent en clair ; les restes D1-D3 des tests de boîte
  (`target_in_front_68k`, `box_around_68k`, `in_front_68k`) et les registres de la collision avec le décor
  (`terrain::push_68k`) donnent ses vitesses à l'étincelle de 0x103ba (`long_scene_20_2p`). Ces tests de boîte et la
  collision gardent donc les registres de l'original dans la façade. Outil : `examples/register_written` compilé sans
  ROM (interprété, sinon les blocs recompilés ne passent pas par le crochet).
* **Fuzz.** `examples/fuzz_ia.rs` compare chaque cœur à la routine d'origine (interpréteur seul) sur des états tirés
  d'une partie réelle : 0 écart sur les 70 entrées (skeleton, kobold, hellhound, boss 11, boss 19). Il a corrigé
  deux routines du lot A (0xffda, 0x10298) et mon propre `in_right_half` (sens de la soustraction).
* **Restent.** `scroll.rs`/`camera_modes.rs` appellent par adresse les natives vidéo (0x26d4, 0x2766, 0xbc6, 0x3525c)
  et 0xa8ae8 ; les adaptateurs `_68k` des autres lots (`random_68k`, `terrain_68k`, `push_68k`, `shock_68k`,
  `in_arena_68k`, `reinsert_68k`) n'ont pas été réexaminés.

## 16. Lot « sortie du Cpu », partie 1 : PC, vidéo, adaptateurs, forme cible des natives

But : lever, hors de `families/`, ce qui empêche le `Cpu` de quitter le chemin du jeu (§ 14, « Sortie du `Cpu` »).

* **Le PC.** Plus aucune fonction du jeu n'écrit `c.pc`. `Sequence::jump`, `jump_to`, `tail` et `Cx::tail` sont
  supprimés : les phases de la partie et de la démonstration sont appelées directement (`game::handler`,
  `select::phase_4`, `ending::phase_28`, `gameover::phase_20`...), un saut terminal vers une native connue passe
  par `Sequence::jump` (`crate::jump` : la native s'exécute aussitôt, son `rts` revient à l'appel en cours), la
  cession de la sortie de scène appelle `kernel::wait`, les deux blocages définitifs (`bra.b *` du MEMORY CHECK
  et de l'EEPROM) sont un point du noyau (`Point::Blocked`), et la fin d'étape du tableau des scores une étape
  (`fixed_screen::Step::AfterStep`). Les replis vers une adresse sans native (entrées de tables jamais produites)
  sont des `unreachable!` (code-mort.md). `Next` n'est plus convertible depuis une adresse ; `tasks::execute`
  ne pose plus de PC ; une tâche qui rendrait la main sans céder arrête le jeu (`Point::Task` n'est plus qu'un état
  transitoire). Dans le jeu, **un appel par adresse n'écrit plus ni le PC ni la pile 68000** : `crate::call` et
  `crate::call_expected` exécutent la native aussitôt ; le retour d'un appel par adresse est une entrée de
  `tasks::table::Table::calls` (A7 descend de 4 comme si l'adresse de retour était empilée, sans écriture), et
  un retour sans appel en cours arrête le jeu. Restent : le chemin des outils (machine sans table des tâches) et
  les correctifs (`cps2::Patch`) de la machine de référence. Les natives partagées par plusieurs entrées (536
  enveloppes son, services son) reçoivent leur adresse (`Body::CpuIn`) au lieu de lire le PC.
* **Vidéo et entrées.** Cœurs Rust : `video::gauge`, `turn_layers_on` (0xbc6), `clear_layer1_start`
  (0x3525c), `load_grid(b, Grid::{Blocks4x4, Layer2, Layer3}, n, destination) -> GridEnd` (0x260c,
  0x26d4, 0x2766), `sprites::build(b, d4) -> Option<Leftovers>` (0x1de96), `sprites::clear_lists` (0x1de7c),
  `inputs::read_panel` (0x80c), `inputs::copy` (0x2aec), `sound::send` (0x68c),
  `credits::read_coin_slots` (0x143da, et la comptabilité `eeprom::count_coin`, 0x2e448, sans `Cpu`),
  `eeprom::vblank` (0x2dcf6) ; `eeprom.rs` n'utilise plus le processeur que pour les registres de ses natives.
  Le VBLANK n'appelle plus que des cœurs (`vblank::vblank(b)`). `scroll::update_cameras(b)`,
  `camera_modes::update_camera_1/2(b)`, `scroll::cameras_start`, `cameras_return` ne prennent plus de `Cpu` :
  les scripts de caméra ont leur contexte (`camera_modes::Cam`, bus et caméra), les parallaxes et le défilement
  pas à pas sont des cœurs, 0xa8ae8 est `effects_creators::set_list`. La mise à jour du monde et la fin de la phase
  12 appellent directement jauge, caméras, sprites, entrées, pop-ups et script (`game.rs` : `adr` 11 → 2).
* **Glitches d'origine trouvés (registres relus, écrits en clair).** D4 laissé par la recopie du décor des caméras
  (0x3f/0x40, 0x1f/0xe0) : relu par un gestionnaire d'objet (`boss13_dragon`, image 7241) — `update_cameras` le
  rend (`[D4Leftovers; 2]`), `update_cameras_68k` le pose. D4 de la liste de sprites (attributs du dernier morceau,
  relu par 0xad7b0) — `sprites::Leftovers::d4`. D2.w = 0x400 laissé par la jauge masquée (test de DSW), écrit par un
  gestionnaire dans un champ d'objet (`long_nain_57_86`, image 13398, `ram ffd692`) — `video::gauge_68k`.
* **Preuve.** Sonde générique `DDSOM_POISON=call_<address>` dans `crate::call` (registres changés par l'appelé,
  inversés au retour, au site d'appel) et étiquettes des adaptateurs restreintes hors des familles
  (`alloc,!alloc@families/`...). Faux positifs écartés : enchaînement des blocs de tuiles (A1 = fin du bloc
  précédent, `modes/title.rs` : un argument, pas un reste), A1 des créateurs appelés par adresse
  (`effects_aids.rs`, `player/spawn.rs` : contrat de leur native).
* **Adaptateurs `_68k` hors des familles (55 sites au départ).** Registres morts (sonde et étiquettes sur les 286
  scénarios), passés au cœur : allocations des modes, scènes, boutique, tableau des scores, crédits et phase 0 (16),
  `rank::reset/lower/tick` et `load_current_preset` (12), `lists::unlink_all` (3), `rng::seed` (3), pop-ups et
  effacement de la couche 1 de la phase 12, A0 des caméras. Registres relus, gardés et documentés (glitches
  d'origine) : copie des entrées 0x2aec (`inputs::copy_68k`, D0-D4 relus : `long_elfe_objets`, image 13133),
  pas d'animation des personnages (`step_keeping_regs`, D0.b relu : `long_3p_mortels`, image 22018), jauge (D2),
  sprites (D4), caméras (D4). Contrats de natives appelées par adresse (restent tant que leurs appelants le
  sont) : créateurs (`effects_aids.rs`, `player/spawn.rs` : A1 relu par `modes/title.rs`), terrain
  (`terrain.rs`, `placement.rs`, `video.rs`), files d'événements (`events.rs`), adaptateurs des natives de ce
  lot (`scroll`, `video`, `eeprom`, `sprites`, `inputs`, `popups`). Familles : 65 sites (liste dans le rapport du
  lot : `random_68k` 18, `push_68k` 14, `terrain_68k` 9, `alloc_68k` 5, `alloc_unchecked_68k` 4, `start_68k` 2,
  `reinsert_68k` 2, `in_front_68k` 2, `flash_68k` 2, et un site de `target_in_front_68k`, `start_by_character_68k`,
  `shock_68k`, `release_68k`, `regs_68k`, `in_arena_68k`, `box_around_68k`).
* **Forme cible des natives.** `Native::run` est un `Body` : `Processeur(fn(&mut Cpu, &mut Cps2Bus))`,
  `CpuIn(fn(&mut Cpu, &mut Cps2Bus, u32))` (corps partagé, reçoit son entrée) ou `Memoire(fn(&mut
  Cps2Bus))` (without register ; le `rts` est done pour lui, `Native::executer`). Converties en `Memoire` : 0x1de7c,
  0xc58, 0x143da, 0xf2e56, 0xf7ee0, 0xe03e4. Bilan : 956 `Processeur`, 532 `CpuIn`, 6 `Memory`.
* **Ce qui bloque encore le retrait du `Cpu`** (mesure `tools/measure_refactor.py` après le lot : `regs` 3 676, `adr`
  749, `stack` 80). (1) Les **appels par adresse** : 749 `adr`, dont 301 dans `families`, 154 `modes`, 169 `scene`,
  28 `credits_task` ; environ 490 `call`/`x.call` hors des familles (`modes/ending` 56, `select` 46, `scene/entry`
  44, `title` 41, `credits_task` 28, `scene/exit` 28, `demo` 28, `transition` 38...) : leurs cibles sont des natives
  `(c, b)` dont le contrat de registres est à prouver une par une (la sonde `appel_<address>` le fait). (2) Les
  **registres** : 3 676 `regs`, dont 2 077 `families`, 348 `modes`, 275 `scene`, 56 `tasks`, 53 `video`, 38
  `scroll` (natives des caméras et parallaxes appelées par adresse), et les glitches relus listés plus haut, qui
  devront devenir des champs nommés d'un état (`Regs`) quand le banc `Cpu` disparaîtra. (3) La **pile** :
  80 `stack` (cadres des tâches et appels attendus, `Table::calls`, contexte des cessions, prise du VBLANK qui
  empile PC et SR) : plus aucune écriture de la pile 68000 par un appel du jeu, mais A7 sert encore de clé aux
  retours. (4) Les **natives** : 956 corps `Processeur` et 532 `CpuIn` (enveloppes et services son, qui lisent
  A0, A1, D4 ou D0/D1) ; un corps `Memory` n'est possible que pour une routine qui ne lit ni ne laisse aucun registre.

## 17. Lot « sortie du Cpu », partie 2 : les écrans

But : plus d'appel par adresse ni de registre miroir dans les écrans (`modes/`, `scene/`, `credits_task.rs`,
`credits_screen.rs`, `hud/`, `service/`), contrats prouvés par poison, état nommé.

* **Appels par adresse.** Les quelque 490 `call`/`x.call` des écrans sont des appels Rust : cœurs nouveaux ou rendus
  publics dans les services (`video::{verrouiller_compteurs, vider_objets, ecran_noir, effacer_couche(Couche),
  desactiver_couches, copier_ecrans_fixes}`, `palette::{charger_palettes_de_scene, charger_ensemble, charger_page,
  charger_tout}`, `eeprom::{compter_partie, compter_depart, stat_personnage_choisi, stat_duree_de_partie}`,
  `credits::{read_starts, write_digit, initialize}`, `inputs::player_inputs`, `player::init::{initialiser,
  rejoindre, effacer_joueurs}`, `config::initialiser`, `sound::emettre`), créateurs des familles en fonctions pures
  (`creators::set_13_pure`... `-> Result<Obj, Obj>`, l'`Err` est l'A1 laissé par un échec : conversion mécanique
  seulement). `adr` : `scene` 169 → 1 (`script.rs` : appels `CALL` des scripts, lus dans la ROM), `modes` 154 → 0,
  `credits_task` 28 → 0, `hud` 8 → 0, `service` 9 → 0, `credits_screen` 3 → 0, `tasks` 8 → 0. `Ctx::call`,
  `Block::call_d0/ok` et les natives qui ne servaient qu'aux appels par adresse (textes de l'écran des crédits,
  boutons de départ, verrous, chiffre des crédits) sont retirés.
* **Registres.** Les miroirs (`c.d[0] = ...` pour un appelé qui n'en lit rien, A0 rechargé après chaque appel) sont
  retirés ; le contexte d'une tâche relu après une cession devient un champ nommé de son `Step`
  (`credits_task::GameStart {coin_slot, player, record, cost}`, D2/D3/A3 de l'original ;
  `characters::Player {record, byte}`, A6/D1). `regs` : `scene` 275 → 67, `modes` 348 → 33,
  `credits_task` 42 → 8, `credits_screen` 30 → 0, `hud` 6 → 3, `service` 10 → 3, `tasks` 56 → 43.
* **État nommé.** `scene::fading` (pages, pas, délai et les six tâches de fondu : `RISE`, `DESCENT`,
  `DESCENT_STEP`, `RISE_STEP`, `TO_WHITE`, `FROM_WHITE`) remplace les copies par écran (`FADE_*`) ;
  `credits_screen` en fonctions pures ; `tasks::prompt::Print`.
* **Natives.** `Memory` : `video::copy_fixed_screens`, `hud::FORGET_DISPLAYED` (0x1507a), `RECORDER_STOP`
  (0xe05d6), remises à zéro des blocs (0xf3130, 0xf7916, 0xfc562) et effacement du texte (0xf683c), qui n'ont plus
  d'appelant par adresse. Bilan du lot (après fusion de main 044c685) : 944 `Processeur`, 532 `CpuIn`, 14 `Memory` (6 avant le
  lot) ; total `regs` 3 676 → 3 045, `adr` 749 → 368 (299 dans `families`). Après fusion de la partie 3 (a911566) :
  862 `Processeur`, 532 `CpuIn`, 14 `Memory`, 82 `Handler` ; `regs` 2 811, `adr` 163.
* **Glitches d'origine (registres relus, écrits en clair).** A1 laissé par une allocation qui échoue (créateurs,
  messages, objets système : l'échec n'est pas testé) ; D0 du chargement des palettes (premier long de la dernière
  page) relu par les compteurs de répétition du menu des réglages ; D1.w = -1 de l'effacement du texte (0xf683c) relu
  comme réglage de l'effet du compte à rebours du bonus ; A0 du gestionnaire de phase (`jsr (A0)`) relu par l'entrée
  de scène ; A0 laissé par la boucle de partie lié aux effets d'ouverture (`effects_13`) et à l'objet de la finale ;
  A0 du déliement des listes à travers la cession de l'écran noir ; D0 du rendu qui fixe la durée d'une cession du
  générique ; reprise de la transition 32 qui rouvre le message 1 (D0 de l'attente) ; octet haut de D0.w de la
  statistique du personnage choisi ; contexte des cessions (D2/D3/A3 du départ d'un joueur, A6/D1 de l'intermède
  des personnages) ; A0 sur le bloc de la sortie de scène gardé par les cessions de la boutique et de l'intermède
  des personnages (constante `BLOCK`), et relu par l'aiguillage en fin d'état partagé (octet `+2` du bloc désigné par
  A0 : `x.c.a[0] = BLOCK` posé en clair au retour de la boutique).
* **Preuve.** Sondes `cession`, `ret` et `rendering` (guide, « Sondes des appels, des cessions et des retours »),
  validation empoisonnée du code d'origine avec `all@` les fichiers du lot et les cessions/retours de ses tâches,
  bissection de chaque échec (`tools/bisect_poison.py`), puis la même liste sur le code converti (`rendering` compris) : 46 échecs au premier passage (A0 du bloc relu après
  les cessions de la boutique et de l'intermède, contexte D1/A6 déjà en champs), 0 après correction, une seule
  lecture gardée (`ret@Shop(LeaveWaited)#a0`, A0 vers l'aiguillage). Les restes de l'effacement de la couche 1
  (0x3525c) avant une mise à jour du monde, notés « glitch possible », sont morts (rendu empoisonné VALIDE) : appel du
  cœur.
* **Ce qui reste.** Les appels `CALL` des scripts d'apparition (adresse lue dans la ROM) ; les natives des phases et
  des écrans gardent un corps `Processeur` tant que le noyau leur passe la main par adresse ; des adaptateurs `_68k`
  dont les restes sont relus (glitches d'origine) : listes de sprites (`build_68k`, D4, 17 sites), caméras
  (`update_cameras_68k`, D4, 3), copie des entrées (`copy_68k`, D0-D4, 2), déliement des listes de l'écran
  noir (A0) ; le passage d'A0 d'un état partagé à l'aiguillage de la sortie de scène (`a0_clobbered`).

## 18. Lot « sortie du Cpu », partie 3 : familles d'objets

Périmètre : `families/` (et les créateurs appelés par `player/spawn.rs`, `effects_aids.rs`). But : que les gestionnaires
et les créateurs ne passent plus par le banc de registres du `Cpu`.

* **Sonde de sortie des gestionnaires.** `DDSOM_POISON=exit` (`diag::probe_exit`, appelée par la boucle
  d'aiguillage `dispatch.rs`) inverse au retour de chaque gestionnaire les registres qu'il a changés (sauf A0 et D7) ;
  `exit.d4@ba310` vise un registre d'un gestionnaire, `!…` en exclut (la même syntaxe vaut pour `appel_<address>`,
  et `call@families/` vise tous les appels par adresse d'un dossier). Bissection automatique (`tools/regs_bisect.py` : classe, gestionnaire,
  registre) sur chaque scénario qui diverge. Résultat sur les 286 scénarios, complété sur les 296 : **les registres que laisse un
  gestionnaire ne sont relus par personne, sauf pour une douzaine de gestionnaires** (tableau ci-dessous). Tout ce que
  les autres écrivent dans les registres (copies des registres de l'original, restes de `reinsert`, de la collision
  avec le décor, des tests de terrain, des tirages…) n'a d'effet qu'à l'intérieur du gestionnaire.

  | Gestionnaire | Registre relu par la suite | Lecteur |
  |---|---|---|
  | SET 0 (0xba310, palette) | D4 (dernier long copié) | le radeau sans pilote (0xad7b0) |
  | EFFCT radeau (0xad63e) | D4 (déplacement) | lui-même à l'image suivante |
  | PSHL 2 (0x5204e) | D3 (case du terrain) | `flash_and_shake` (boss 11, scène 42) |
  | PEFF 0, 1, 12 (0x49e36, 0x4a21e, 0x4bc66) | D2 | le rebond de l'éclat de gel (0x4bda8) |
  | BOSS 0 (0x794ae) | D2, D3 (collision, tests de boîte) | `flash_and_shake` |
  | ITEM 5 (0xdc382) | D2 | (`covb_long3p_10`) |
  | ENEMY 16 (0x6f1c0), ENEMY 4 (0x5d87a) | D5 | la rangée d'éclats d'une caisse (0xa17d0) |
  | PEFF 41 (0x50348), et la collision des PEFF 39/41 | D2 (registres de 0x33fe4) | l'éclat de gel (`sorts_*`, lot « sorts ») |

  Les dix scénarios longs du lot « sorts » ont montré, après coup, deux de ces lecteurs (PEFF 41, éclat de gel : D2 et
  d'autres registres ensemble) : la preuve ne vaut que pour les chemins joués. Les gestionnaires de ce tableau restent sur
  le processeur.
* **Restes nommés de la boucle.** Ces registres deviennent les champs de `dispatch::Leftovers` (`d4`, `d5`), passés au
  gestionnaire converti dans `dispatch::Dispatch` (objet A0, compteur D7.w, restes) et rendus au processeur après lui
  (les routines encore sur le processeur, liste des sprites, caméras, gestionnaires non convertis, les lisent et les
  écrivent toujours dans le banc). SET 0 et le radeau écrivent `leftovers.d4` ; la zone de jeu et la collision avec le
  décor écrivent `leftovers.d5` (`Cx::keep_in_zone`, `keep_on_screen`, `push_terrain`).
* **Forme des gestionnaires.** `Cx<'a, R>` est générique sur le banc de registres : `&mut Cpu` (par défaut, inchangé)
  ou `&mut Dispatch` (`Gx`, sans registres : `x.c.d[0]` n'existe pas, le compilateur le garantit). Le trait
  `cx::Bench` porte ce qui dépend du banc : la remise en liste (adaptateur `reinsert_68k` sur le processeur, cœur sans
  registres) et l'accès au reste D5. `Machine` n'exige plus que le bus (`Processeur` donne `machine()` aux contextes
  qui ont un `Cpu`). Un gestionnaire `Gx` est déclaré `0x… "name" #[without] chemin` dans `handlers!` : sa native est
  `Body::Handler(fn(&mut Cps2Bus, &mut Dispatch))`.
* **Généralisation par point fixe** (`tools/regs_generic.py`) : toutes les fonctions
  `fn f(x: &mut Cx, …)` sont rendues génériques (`<R: Bench>`), puis celles où le compilateur trouve une erreur reprennent
  leur signature, jusqu'à ce que tout compile ; un contrôle par entrée (`tools/regs_without.py`) marque `#[without]` les gestionnaires
  dont la fonction est générique. Résultat : **1 870 fonctions des familles prouvées sans registres** (2 796 restent
  sur le processeur, dont 728 l'utilisent directement : 1 212 accès à Dn, 304 à An, 76 appels par adresse), **82
  gestionnaires `Body::Handler`** (SET, SOBJ, EFFCT, ESHL…).
* **Adaptateurs `_68k` (65 au départ).** Retirés : les 18 `random_68k` (valeur rendue ; `rand_and_b` garde l'octet
  haut du tirage dans D0, relu par l'éclat de gel), 10 des 14 `push_68k` et 6 des 9 `terrain_68k`, dans les gestionnaires dont
  la sortie n'est pas relue (`push_terrain`, `terrain_of`), `alloc*_68k` des créateurs (portés par `Regs`),
  `start_68k`, `start_by_character_68k`, `release_68k`, `regs_68k` (boucle d'éclats de caméra), `flash_68k`
  (`palette::set_flash_color`), `in_arena_68k` (le `bhi` de `reactions.rs` est `cull::beyond_arena`). Restent (22 sites)
  : les implémentations `Regs` et `Bench` du processeur (natives et gestionnaires non convertis), `terrain_68k` de
  PSHL 2 et de l'éclat de gel (D2, D3 relus), la façade `enemy_lib` (tests de boîte, collision, choc : D1-D3, A1 relus
  par `flash_and_shake` et les ennemis), `items.rs` (contexte `families::Ctx`), les créateurs de paires d'ennemis.
* **Appels par adresse (301 → 97 `adr`).** Sons (`convert_sounds.py` puis à la main : 70 sites), éclairs de palette
  (`palette::set_flash_color`, `flash`, `restore_flash`), directions (`aids::direction_from`), créateurs d'effets
  (cœurs `effects_creators::scenery_*`, `spark_at`, `spell*`, `shouts`… sur `Pure`), créateurs ESHL (`eshl_creators`
  réécrit sur `Regs` : 33 créateurs, cœur sans registres et native ; tirs lancés passés en `fn(&mut Cps2Bus, Obj)`),
  SET de scène (bulles de dialogue), écrans de fin, vote, SET 0xa, tirs 0x17/0x18, effets 5, tir d'arme, chocs.
  Restent : tables par type de la ROM (réactions, routines de chute, de mort, de vague : 42 sites), routines de scène
  (`scene/`, autre lot), palettes par page, et une vingtaine de créateurs de `creators.rs`/`small.rs` (paires
  d'ennemis, SOBJ, pierres, éclair du joueur).
* **Glitches d'origine écrits en clair** : D4 du radeau, D5 de la rangée d'éclats (`row_1a(…, kind, d5)`), réglage de
  0xb1a7a pris dans D1 laissé (`set_c_finale`), variante des débris prise dans le D1 laissé par le choc 0xb10c8
  (`eshl_b`), D0 = 0 de 0x54872 qui empêche de retenir le tir créé (`eshl_c`), A1 « reste » de l'allocation où
  0x9dda8 / 0x9ed78 écrivent l'orientation quand le pool est plein (`eshl_creators::copy_flip`).
* **Factorisation** : le décor repris (0xcc0ca) avait quatre copies (`creators`, porte de zone annexe SET 7 à l'aller
  et au retour, passage EFFCT 14) : `creators::resume_scenery` ; tirs 0x18 (trois natives, une fonction) ; éclairs de
  couleur (`small` sur `palette`).
* **Écart trouvé par `fuzz_couv`** (ESHL, zone C08, avant/après sur les mêmes états) : la remise en liste en clair
  (`SortedList::reinsert` : plan puis application) ne refaisait pas les écritures de 0x1ee1a dans le même ordre ; sur une
  liste dont des liens se recouvrent, la mémoire différait (ESHL 44). Le cœur est maintenant la transcription exacte de
  `Reinsert`, sans registres (`objects/lists.rs`).

**Ce qu'il faut encore pour que `m68k::Cpu` disparaisse du jeu.**
1. *Registres portés.* Familles : 2 796 fonctions sur le processeur, dont 728 l'utilisent directement (`boss_1`,
   `boss_9`, `boss_11/chain`, `reactions`, `set_b`, `effects_c`, `player/peff_*`, `enemy_lib`, `enemy_common`…) ; ce
   sont presque toujours des copies de registres que rien ne relit hors du gestionnaire (sonde `exit`) : à remplacer
   par des variables locales, fonction par fonction, en relançant `tools/regs_generic.py` et `tools/regs_without.py` (chaque fonction convertie
   en libère d'autres). Les flux réels sont connus : restes de la boucle (tableau ci-dessus : D2, D3 à ajouter à
   `Leftovers`, avec les valeurs que laissent la collision et les tests de boîte), et à l'intérieur d'un gestionnaire les
   registres d'entrée des tables par type (D0 = index, A1 = routine). Hors familles : `modes/`, `scene/` (autre lot),
   `player/` (189), `palette` (86), `video` (53), `scroll` (38), `events` (35), `aids` (30), `credits_task`,
   `credits`, `config`, `kernel` (33).
2. *Appels par adresse.* Familles : 97 `adr` (tables par type, scène, palettes par page, créateurs restants) ; il faut
   une table Rust (adresse → fonction) pour les tables par type de la ROM, comme `boxes::effect_special`.
3. *La pile A7 comme clé des retours.* `tasks::table::Table::calls` et les cadres (`Frame::stack`) identifient un retour
   par la valeur d'A7 ; `crate::call` décrémente A7. Sans `Cpu`, il faut une profondeur d'appel portée par la table des
   tâches (compteur par tâche) à la place d'A7, et le contexte d'une tâche (ses registres sauvés à la cession) réduit à
   ses restes nommés.
4. *Prise du VBLANK.* `vblank::interruption` empile PC et SR sur la pile de la tâche interrompue (et le noyau les
   reprend) : à remplacer par un état du noyau (« interrompu au point P ») sans pile 68000, ce qui suppose 3.
5. *Restes des routines hors familles.* La liste des sprites (D4), les caméras (D4), la jauge (D2), la copie des
   entrées (D0-D4) écrivent encore leurs restes dans le banc : quand le banc disparaît, `dispatch::Leftovers` devient un
   état de la partie (dans la table des tâches) que ces routines et les gestionnaires lisent et écrivent par leur nom.

Mesure (`tools/measure_refactor.py`, avant 1ead669 / après) : `families` regs 2 079 / 1 841, adr 301 / 97, drap 83 / 64 ;
total regs 3 686 / 3 455, adr 749 / 545. Natives : 82 `Body::Handler`.

## 19. Lot « sortie du Cpu », partie 7 : noyau et écrans (wt-cpu-core)

Périmètre : noyau (`kernel`, `no68k`, `startup`), `tasks/`, `scene/`, `modes/`, `hud/`, `service/`, `credits*`,
`dispatch`, `game`, `config`, et les services `palette`, `video`, `sound` côté jeu. But : que le jeu ne passe plus par
le banc de registres du `Cpu` dans ce périmètre (le type reste, pour la machine de référence, les outils et le code
encore sur le processeur des autres périmètres).

* **Contexte des étapes sans processeur.** `scene::Ctx` n'a plus que le bus (`Ctx { b, phase_handler }`) : toutes les
  étapes des tâches, des écrans et des séquences de scène s'exécutent sans registres ; `tasks::execute(step, b)`,
  `tasks::return_to_caller(b)`, `table::pop_return(b)` (les variantes `_68k` ne servent plus qu'aux natives appelées
  par le code d'origine dans les outils : le retour vers le code d'origine est noté et appliqué par `Native::execute`).
* **Noyau sans registres.** L'ordonnanceur ne tient plus A0/D0/D1 : le créneau examiné est `Table::exam` ; le SR devient
  le masque d'interruption de la table (`kernel::interrupt_mask`, 7 à la mise sous tension, 6 pendant l'examen d'un
  créneau, 0 dans une tâche ; `trap #12` le change). `kernel::wait(b, images, reprise)`, `cede_ready`, `finish`,
  `restart(b, entrée)` sans registres ; les enveloppes 0x9b6... restent des natives (arguments en registres) pour les
  appels par adresse et les outils. `no68k::line` n'utilise plus le `Cpu` de la machine (sauf son compteur de
  cycles) ; le démarrage appelle directement les effacements (0xc58, 0xca8, 0xcc8, 0xce8), `config::initialize`
  (0x2d30a) et `kernel::create_task` (0x9b6).
* **Boucle d'aiguillage sans registres.** `dispatch::Class::update(b)` appelle chaque gestionnaire (`Body::Handler`,
  tous depuis la partie 5) avec l'objet, le compteur et les restes de la partie ; D0 (type × 4) et A1 (le
  gestionnaire) ne sont plus posés. Un gestionnaire qui a cédé reprend par `dispatch::resume_handler` (portes SET 7 et
  29, feu d'artifice : plus de `Cpu` temporaire). `update_last_kind` rend le type du dernier objet traité (glitch du
  générique de fin, ci-dessous).
* **Restes nommés.** `dispatch::Leftovers` (table des tâches) est l'état unique des restes D2-D5 : la jauge
  (`video::gauge_in_world`, D2.w = 0x400), la liste des sprites (`sprites::build_in_game`, D4), les caméras
  (`scroll::update_cameras_in_game`, D4), la copie des entrées (`inputs::copy_in_game`, octets bas de D2-D4), le rang
  d'arrivée de la sortie de scène (D5), les fondus par composante (restes sauvés et rendus comme le `movem`).
* **Pont vers le code encore sur le processeur** (`bench::on_bench`) : mise à jour et initialisation des joueurs
  (`player/`), résolution des coups (`combat`), fin de partie (`aids::game_over`), routines des scripts de scène
  (`families/`) reçoivent un banc neuf (A5, A7 sur le haut de la pile du superviseur, restes D2-D5 en entrée et en
  sortie ; rien d'autre ne passe d'un appel à l'autre ; sonde `DDSOM_POISON=bench`). Il disparaîtra avec ces modules.
* **Glitches d'origine écrits en clair.** A0 du gestionnaire de phase (`jsr (A0)`) relu comme objet par l'entrée de
  scène (éclair 0x13, objet de la finale) : `Ctx::phase_handler`, passé par la boucle de partie et la démo ; A0 relu par
  l'aiguillage d'une séquence de sortie de scène en fin d'état partagé : `Table::scene_exit_a0` (le bloc, ou le dernier
  objet délié par l'écran noir, qui survit à la cession), et l'A0 que laisse la mise à jour du monde (au-delà du
  dernier créneau SOBJ) ; durée de la cession du générique (0xfbf00) = D0 laissé par le rendu (type × 4 du dernier objet
  traité). Approximations sur des chemins jamais joués : code-mort.md (« partie 7 »).
* **Natives.** Sans registres (`Body::Memory`) : effacements de couches, écran noir, 0x1cf3c, `config_*`, envoi au Z80
  (0x68c ; l'affichage des verrous du Z80, code de mise au point, est gardé compilé : `sound::show_locks`,
  `hex_byte` = 0x29d78), phase 24, mise à jour du monde, phases 0 et 12, lecteur de script. Retirée : `config_variant`.
  Supprimés : `crate::jump`, `jump_to`, `call_expected`, sondes `exit`, `return`, `new`/`resume`.

## Avancement

| Étape | Commit | Validation |
|---|---|---|
| Socle : mesure, bench, rendu facultatif, `ram.rs`, `diag.rs`, adaptateurs `_68k` des pools | 688b54c | ciblée |
| Pilote, appelants par les adaptateurs ; documents | e1f8159 | complète, 269/269 normal et sans 68000 |
| Pilote, poison, glitch de l'orbe, gestionnaires en fonctions pures | voir `git log` | complète, 269/269 normal (15 min 40 s) et sans 68000 (15 min 19 s), en parallèle à 7 tâches chacune |
| Lot 0a : plus de durées du mode hybride, jeu sans 68000 par défaut (`DDSOM_HYBRIDE=1` : diagnostic) | edef15d, documentation | complète, 269/269 sans 68000 (14 min 46 s, 10 tâches) et 269/269 depuis les assets (`DDSOM_ASSETS`) ; hybride : bot30k 5 000 images VALIDE (non exigé) |
| Lot E : tâches et écrans en machines à états (étapes, sous-machines des fondus et de l'écran noir) | voir `git log` (wt-taches) | complète, 269/269 sans 68000 et 269/269 depuis les assets (`DDSOM_ASSETS`) |
| Lot 0b : API du moteur d'objets, appelants convertis (adaptateurs) | f60da0a | complète, 269/269 sans 68000 |
| Lot 0b : fusion du lot E, méthodes au cœur après `DDSOM_POISON`, documentation | voir `git log` | complète, 269/269 sans 68000 et 269/269 depuis les assets (`DDSOM_ASSETS`) ; empoisonnée : lecteurs isolés (guide) |
| Lot D : joueurs et monde (accesseurs, appels Rust directs, poison, trous réécrits) | voir `git log` (wt-joueur) | complète, 269/269 sans 68000 et 269/269 depuis les assets (`DDSOM_ASSETS`), JOBS=8 ; poison du lot sur 90 scénarios ; fuzz_player des dix routines ajoutées |
| Lot C : fondu des portes en sous-programme de tâche, J1 dans le lot, registres morts des gestionnaires sans son | e3a49b7 (wt-decors) | complète, 269/269 sans 68000 (30 min 49 s, 8 tâches) et 269/269 depuis les assets (`DDSOM_ASSETS`) ; `fuzz_door` (portes 7 et 29), `fuzz shop`, `fuzz small` sans écart |
| Lot S : services communs sans registres (sauvegarde fantôme du son, cœurs son/palette/texte/terrain/aides/événements, appelants hors lots A-B) | voir `git log` (wt-services) | complète, 269/269 sans 68000 et 269/269 depuis les assets (`DDSOM_ASSETS`), JOBS=8 ; `fuzz sound`, `fuzz video`, `fuzz u1`, `fuzz_world ia_` |
| Lot B : créateurs en fonctions Rust, appels directs, replis, registres prouvés morts, étape du feu d'artifice | voir `git log` (wt-effets) | complète, 270/270 sans 68000 et 270/270 depuis les assets ; `DDSOM_POISON=effects` et `leftovers` 270/270 |
| Lot A : ennemis et boss (façade `enemy_lib`, noms, variables typées, natives du lot en Rust, routines sans native, registres résiduels prouvés par poison) | voir `git log` (wt-boss) | complète après fusion de main (lots S, B, 9b27479), 272/272 sans 68000 et 272/272 depuis les assets (`DDSOM_ASSETS`, assets réextraits : `native_zones.txt` de main), JOBS=8 ; poison par site et bissections (boss/, enemy/, cov) |
| Lot « noyau final » : J3 achevé (appels en cours en étapes, appel attendu, cession notée au lieu de `Unwound`), ordonnanceur, démarrage et VBLANK en machine à états Rust, mode hybride retiré, appels directs de la fin de phase | voir `git log` (wt-noyau) | complète après fusion de main (f8f67af), 272/272 sans 68000 (14 min 55 s, 8 tâches) et 272/272 (10 min 53 s) depuis les assets (`DDSOM_ASSETS`, `data` réextrait par l'`extract` de main : `reste.json`) |
| Lot IA : cœurs des aides d'IA et services restants, natives retirées, glitches de registres en clair, fuzz_ia | voir `git log` (wt-ia) | complète, 272/272 puis, fusionné avec le lot « trous », 286/286 (DDSOM_LONG=1) sans 68000 et depuis les assets (`trous_assets`), JOBS=8 ; poison par adresse et par registre (sondes), `fuzz_ia` 0 écart |
| Lot « sortie du Cpu », partie 2 : écrans sans appel par adresse ni registre miroir, contexte des tâches en champs d'étape, `scene::fading`, sondes `cession`/`ret`/`rendering` | voir `git log` (wt-ecrans2) | complète après fusion de main a911566 : 296/296 (DDSOM_LONG=1) sans 68000 et 296/296 depuis les assets (`sorts_assets`), JOBS=8 ; empoisonnée 296/296 après cette fusion aussi. Avant, après fusion de main (044c685), 296/296 (DDSOM_LONG=1) sans 68000 et 296/296 depuis les assets (`sorts_assets`), JOBS=8 ; empoisonnée (`all@` les fichiers du lot, cessions et retours de ses tâches, une lecture gardée) 296/296. Étape suivante : prouver le reste des `_68k` des écrans (`build_68k`, `update_cameras_68k`) avec la sonde `exit` et `tools/regs_*.py` |
| Sortie du Cpu, partie 3 : familles (sonde de sortie des gestionnaires, `Cx<R>`/`Bench`/`Leftovers`, 1 870 fonctions génériques, 82 gestionnaires `Body::Handler`, créateurs ESHL et d'effets en Rust, `random_68k` retiré, remise en liste exacte) | voir `git log` (wt-familles) | complète après fusion de main (044c685) : 296/296 sans 68000 et 296/296 depuis les assets (`sorts_assets`), JOBS=7/6 ; sonde `exit` 285/286 puis exclusions bissectées ; `fuzz_couv` C08, C10, C0e, C14 avant/après : aucun écart nouveau |
| Sortie du Cpu, partie 5 : registres des familles en variables locales (wt-locales) — EN COURS (dernier état validé, fusion de main b6ef83e comprise) | 0e2ccf8 (étape 1), 1a3aff7 (étape 2), e4a30ac, 169e328, 4eaba13, fusion b6ef83e (validés 296/296 dans les deux modes) | **Suite (wt-familles2, depuis 82f6d91)** : toutes les familles d'ennemis sont en gestionnaires `#[without]` (`lecteurs.py --famille enemy` : 0) : squelette (1), trog (2), gargouille (8), golem (11 ; glitch d'origine : boutons relus dans le D0 que 0x1a56 écrase par `+0x32`, drapeau « a bougé » en D2), bête de l'ombre (19 ; Z laissé par 0x1a32 après l'orphelinat), scarabée (9 ; effet d'attaque testé par la valeur rendue et non par une retenue jamais posée), scorpion (10), harpie (21 ; créateurs 0xa036e/0xa0566 en cœurs sur `Regs`), Riyakaa (16), tentacule (7 ; tir 0x9ef20 en cœur), doubles (22-24, 26 ; réaction du maître par `enemy_common::run_reaction`), bras du guerrier (25 ; étincelles sur `lib::shock`, A1 en scène 41 = l'hôte). Boss `#[without]` : 0, 1, 2/22, 4, 6, 7, 10, 13, 16, 17, 18, 21 (réactions communes par `run_reaction`, créateurs 0x57c8c/0x5f764 en cœurs, D2/D5 des boucles `dbf` laissés en restes ; `boss_common::addq8/sub8/sub16` n'écrivent plus SR). EFFCT `effct_double_b` (0xb7248). Validé 296/296 dans les deux modes (DDSOM_LONG=1) : a11f35e, 8505fff, ef8fe90 (fusion de main 0e04e48), cf7baf5, c5734b0. `fuzz_couv` C02/C04 (fuzz.sh) contre main 82f6d91 : aucune différence. **Reste** : boss 3 (forme du corps, A1/D6 de 0x81aa4), 5 (A1 hérité de l'image précédente en saisie, sous-état 0xc : il faudrait un reste A1), 8, 9 (registres de boucle A2/D6 partout), 11 (chaîne), 15 (D1 relu, 0x168e), 19 (octet haut de D0 de l'aiguillage dans l'index des réactions, 0x63ae0) ; puis `player/`, `set_b`, `effects_c` (`lecteurs.py`). Outil cassé signalé : `fuzz_enemy families` (sans 68000 par défaut) : l'original contrôlé ne revient jamais (`check`), aucun appel comparé. **4eaba13 (validé 296/296 dans les deux modes, puis de nouveau après fusion de main b6ef83e)** : six familles de plus en `#[without]` : elfe (12), gnoll (13), ogre (18), hibours (4), goule (5), molosse (6) ; 0x81aa4 (corps du boss 3) générique (`boss_3::body::pousser` au lieu du `call` par adresse ; registres rendus par l'adaptateur de la native) ; `active_loop`/`hit_loop` génériques ; tables de programme lues par `enemy_common::rom32` (et non `data32` : `Cx::rom32` du processeur lit l'espace programme). **Reste (familles d'ennemis)** : squelette (D1 relu, attacks.rs:336), trog (A1 de table, mod.rs:136), gargouille (D0 de visée relu, attacks.rs:409/436), golem (D0 « boutons », control.rs:50), bête de l'ombre (D0 d'angle, parts.rs:234), scarabée, scorpion, harpie, Riyakaa, guerrier, double, part ; puis boss, player/, set_b, effects_c (`lecteurs.py`). Outils `generique.py`/`nettoie.py`/`famille.sh` : `<scratchpad>/locales_outils/` (à recopier, le scratchpad ne survit pas). **Étapes 1 et 2 validées (e4a30ac, après fusion de main 29f0087) : 296/296 VALIDE en mode par défaut et avec `DDSOM_ASSETS` (DDSOM_LONG=1).** Cause de l'écart de `sorts_noms_b` (image 21225, `+0xa0` du Bow Gnoll) : `enemy_bow::decide` lisait la cible dans l'A1 laissé par le test de boîte, que les façades génériques n'écrivent plus ; elle prend désormais la cible `+0x50` (c'est la même). L'étape 2 cassait 13 scénarios (boss15_scene46, scènes 46 et 105, ending, cov_2p_c...) : `enemy_warrior::sparks` appelait le `shock` sans registres puis relisait A1 ; il garde `shock_68k`. **`fuzz_couv` C02/C04 contre main (29f0087)** : 10 scénarios d'ennemis et 10 de boss, mêmes graines : aucun écart plausible ni panique de plus que main ; seule différence, 58 écarts REG (D0) de moins pour `enemy_shake_b` (0x71680). **Étape 3, familles sans registres (169e328, validé 296/296 dans les deux modes)** : `reactions.rs` entièrement générique (`Cx<'_, R: Bench>`), avec les relevés par type qu'elle appelle ; `load_target` ne pose plus A1 (scarabée et scorpion prennent la cible rendue) ; riposte du boss 3 : le tirage et le masque sortent par l'adaptateur de la native 0x81ae0 ; cinq familles en gestionnaires `#[without]` : kobold (0), Bow Gnoll (3), troll (14), gobelin (15), machine de guerre (27). Méthode (outils du scratchpad `locales_outils/` : `generique.py` rend générique une fonction et, d'après `cargo check`, tout ce qu'elle appelle ; `nettoie.py` retire les écritures seules de D0/D1/D6/An et fait passer D2-D5 par le banc ; `lecteurs.py` liste les lecteurs de registres) : la fonction générique est la preuve que la famille ne lit plus de registre ; entre gestionnaires, seuls D2-D5 sont relus (`DDSOM_POISON=exit`, `dispatch::Leftovers`). **Étape 1 (0e2ccf8)** : `dispatch::Leftovers` reçoit D2/D3 (pris et rendus par `Dispatch`), le trait `cx::Bench` donne `d2()`..`d5()` et `counter()` ; `terrain::push_leftovers`, `ai_aids::{target_in_front_leftovers, box_around_leftovers, in_front_with_leftovers, HitboxLeftovers}` ; façades `enemy_lib`/`enemy_common` génériques (collision, tests de boîte, butin, `flash_and_shake` lisent/écrivent les restes D2-D5 ; visée, tirages, créateurs sans registres ; appelants de `aim*` reposent D0 en attendant leur conversion). Validation complète de 0e2ccf8 en mode par défaut : 295/296, **ÉCHEC de `sorts_noms_b` (première différence à l'image 21225)**, à bissecter (candidats : restes D2-D5 désormais écrits par les façades génériques, D0/A1/D1 retirés de `collide_terrain`, des tests de boîte, de `palette_script`, de `roll_*`) ; la validation `DDSOM_ASSETS` n'a pas démarré (construction tombée sur l'arbre en cours d'édition) : à relancer. **Étape 2 (non validée, compile)** : `reactions.rs` sans registres (pas en macro `step!` à appel direct au lieu de tableaux de pointeurs, sons par type via `sound::play`, relevés par type 0x3e000 en table Rust `get_up_by_kind`, `kept_down` au lieu d'un pointeur de suite, D2/D3 gardés en restes dans `face_attacker`/`seized_hits`, glitch scène 41 de `zapped_burn` écrit en clair, `burnt` : animation 0 = D0 laissé par l'aiguillage) ; `lib::shock` rend `Choc` (Cree/Refuse(A1 laissé)/Aucun), l'ancien `shock_68k` reste pour `enemy_warrior::sparks` ; `react` : table Rust des réactions propres (`small::own_reaction` : 0x78ad2, 0x71680, et 0x65236 de la gargouille, jamais atteinte, réécrite) ; `react_with` remplacé par `reaction_routine`. **Suite** : valider l'étape 2 (DDSOM_LONG=1, deux modes) et fuzz_couv C02/C04 contre `base/` (scratchpad locales_/fuzz.sh) ; puis convertir les 228 « lecteurs de registres d'entrée » (outil scratchpad locales_/lecteurs.py) famille par famille (ennemis, boss, player/, set_b, effects_c…), garder D2-D5 en restes dans le code ennemis/boss (lecteurs : `flash_and_shake`, roll du boss 11, rangée d'éclats), supprimer les écritures mortes, puis `tools/regs_generic.py` et `regs_without.py`. Méthode : la fonction générique est la preuve (compilateur) ; une écriture de registre ne se retire que si aucun lecteur non générique ne suit. |
| Sortie du Cpu, partie 5, suite joueur2 (wt-joueur2) : `player/`, `set_b`, `effects_c` en gestionnaires sans registres — FAIT (non fusionné) | 1645d98 (player/, set_b), 23917cd (effects_c), 93a30bf (fusion de main 3bec3f5) : **296/296 VALIDE en mode par défaut et avec `DDSOM_ASSETS` (DDSOM_LONG=1), après fusion de main** | **player/ et set_b** : les 70 gestionnaires PSHL/PEFF de `families/player/` et ceux de `set_b` (sauf la porte 29, `door_b`, dont le fondu cède la main : `run`/`watch`/`after_fade`/`exit_zone` restent sur le processeur) sont `#[without]` ; `lecteurs.py` : joueur 38 → 0, set_b 9 → 1 (`run_on_68k`, adaptateur des portes sur le processeur). Méthode : toutes les fonctions rendues génériques d'un coup (`perl` puis `nettoie.py`), lectures restantes converties à la main en arguments et retours. `pshell` sans registres (`link_a1` → `linked`, `state`, `rand_and`, `rand_and_b` ; `rand_and_b_word` rend le D0.w relu par l'éclat de gel ; `offset` garde D2 en reste ; `leftovers_terrain` pour les restes de 0x24a8/0x24b0) ; boutique (PEFF 42/43) : client et position du script en arguments (`talk::Script`), octet haut de D0 de l'appelant écrit en clair (nul : étendues des zones < 0x100) ; familiers (PEFF 34, 41) et chaîne (PSHL 19) : la cible de `turn`/`steer` en argument (glitch d'origine gardé : après une recherche, le dernier objet examiné et non la cible) ; foudre (PEFF 26) : `(0x168,A1)` avec A1 = l'adresse du gestionnaire (0x4e038), écrit en clair ; créateurs appelés par adresse remplacés par des cœurs (`creators::bolt_shot`, `shot_5`, `small::pshl_stones` sur `Regs`, natives 0x53e48/0x52578/0x53746 retirées), palette 0x3afe par `placement::load_effect_palette` (D2-D5 en restes). set_b : vagues (créateurs = allocations ENEMY/BOSS/SET), `effects_aids::living_players` (0xcc18e), `set_residue::begin_wave`/`finish_wave` (sons 0xe0c5c/0xe0d8c), SET de scène (`scene_set_via` sur `OnBench`), décors 6 et 11, liste de décors 7, dialogue : `load` rend le D6 que laisse la copie de palette (relu par `shrink`) ; caméras, palettes par page, placement des joueurs, 0x1ad2a, 0xa66c0 sur un banc de travail (`effects_creators::leftovers::sur_banc_de_travail` : seuls D2-D5 entrent et sortent). **Écart trouvé et corrigé (API commune)** : la remise en liste sans registres n'écrivait pas le reste D2 de 0x1ee1a (lien quitté lors d'un déplacement), relu par le rebond de l'éclat de gel (`sorts_magicien` image 4874) : `SortedList::reinsert_leftovers`, utilisé par `Bench for &mut Dispatch`. Nouveau : `effects_creators::leftovers::OnBench` (cœur `Regs` qui ne garde que les restes D2-D5). **effects_c** : 40 gestionnaires `#[without]`, `lecteurs.py` 5 → 0 (total du jeu après fusion : 180 fonctions) ; `fuzz_couv` C06/C12/C10/C0e contre main bba79f5 (sorts_*, long_boutiques, bot_2p, bot_char3) : aucune panique de plus, écarts du même ordre que main ; boutique EFFCT (case de l'acheteur en argument), état 4 du minuteur du vote en `unreachable!` (code-mort.md). **Reste** : `effects_creators` (natives d'appel par adresse, arguments en registres : 4 lecteurs, appelants hors périmètre), appels par adresse sur banc de travail (caméras, palettes, 0x930a4, 0x3403c), `regs_generic.py`/`regs_without.py`. |
| Sortie du Cpu, partie 5, suite familles3 (wt-familles3) : ESHL, SOBJ, SET, ITEM, BOX, EFFCT restants en gestionnaires sans registres — FAIT (non fusionné) | 9b2e7b1 (étape 1), 4afecd3 (étape 2) : 296/296 VALIDE dans les deux modes ; étape 3 et fusion de main 440e988 : **308/308 VALIDE en mode par défaut et avec `DDSOM_ASSETS` (DDSOM_LONG=1)** | **Toutes les entrées des tables d'aiguillage sont des gestionnaires `#[without]`** (`Body::Handler`) : ESHL (`eshl*.rs`, 52 de plus), SOBJ (5), SET (`set.rs`, `set_c*` : contrôleur de scène, fenêtre, écrans de fin ; portes SET 7 et 29 comprises), ITEM (`items.rs`, `items_b`), BOX (`boxes.rs`), EFFCT (`effects.rs`, `effects_b/d/e/f/g`, feu d'artifice compris), ENEMY 20 et 28. Natives (`examples/names`) : `Body::Processeur` 715 → 594, `Handler` 227 → 344 ; `lecteurs.py` : 180 → 143 fonctions (familles 69 → 32). **`families::Ctx` supprimé** : `effects.rs`, `items.rs`, `boxes.rs` écrits sur `cx::Cx` générique (`ctx.rom*` de l'ancien contexte = espace données → `data*`, `prog*` par `Machine`) ; `handler!` accepte `#[without]`. Bulle (EFFCT 4) : l'épilogue étranger 0xaa9fe ne fait que dépiler les quatre appels (`effects::foreign_epilogue`, A1-A4 jamais relus). Cessions sans processeur dans le gestionnaire : fondu des portes (`set::exit_fading::fade_to_black` générique, pas du fondu sur un banc de travail qui porte A0 et les restes) et attente de quatre images du feu d'artifice (`kernel::wait` sur un banc de travail) ; plus de `run_on_68k`. **Appels par adresse remplacés** : nouveau module `families/scenery.rs` (décor repris 0xcc0ca/0x33aa6/0x33b24 + palettes 0x1a360/0x1a678/0x1a8ac, placement des joueurs 0x20de6, page de palettes 1 0x1a608, sauvegarde 0x1ad2a, restitution 0x1ad6a, salve 0xa66c0 ; chacun écrit les restes D2-D5 de l'original), `boss_15::attacks::approach_toward` (cœur de 0x930a4, native retirée), `terrain::push_second_leftovers` (0x3403c), `small::four_palette_blocks` (0x10158/0x101da), `small::clear_scroll1_start` (0x3525c), `creators::play_scene_music`/`run_scene_exit` (0xd9778/0xd981a), libération par classe (0xcc2b8 → `pools::POOLS`), cadre de dialogue `creators::sobj_12_pure`, `video::load_grid` (0x26d4/0x2766), `effects_aids::scenery_6`, `effects_creators::tile_block` (0xb230e), `player::init::init_weapon_stats` (0x3552) ; `sur_banc_de_travail` supprimé (plus d'utilisateur). Doublons retirés : natives des aides de la barrière (0xb9ee6, 0xb9eba, 0xba094, code-mort.md). Glitches écrits en clair : rangée de placement de l'ESHL 10 lue dans le D5 hérité ; A1 = adresse du gestionnaire (0x9f67c) de l'ESHL 13 sans propriétaire ; marge du SOBJ 2 dans le D2 laissé par 0xddbfc ; couleurs du `movem` du guide 3 en D2-D5 ; secousse des écrans de fin (D1 → réglage de 0xb1a7a, D2 en reste). Code mort : guide 1 sans joueur vivant (`unreachable!`, code-mort.md). **Écart trouvé par `fuzz_couv`** (branche contre main a00e694, 14 scénarios, cibles `eshl_`, `sobj_`, `set_`, `effct_`, `item_`, `box_`, ENEMY 20/28) : la version « banc de travail » du passage (EFFCT 14, 0xad866) écrivait FFFFFE sur un chemin non joué (registres du banc nuls) ; disparu avec `scenery::resume_scenery_lit`. Aucune autre différence. **Étape 2** : créateurs d'effets appelés par les joueurs (`player/`) par des fonctions à arguments (`effects_creators::for_players` : étoiles 0xb378a, halo 0xab708, gerbe 0xab7d8, type 57 0xb6ba8, flammèche 0xb9c6a, éclat posé 0xb97ba ; A0, A1 et drapeaux écrits comme la native) ; 0xdabba et 0xdb6c0 en fonctions de `items.rs` appelées par `game::leave` et `player::hurt` (natives retirées). **Écart `effct_attack` (fuzz_couv 122 contre 63) : bruit.** Même graine, cible `name:effct_attack` seule, 13 scénarios (sorts_*, long_boutiques, bot_2p, bot_char3) : bba79f5 et a00e694 donnent exactement les mêmes essais (9 093), écarts (63) et plausibles (19) ; les écarts viennent d'états `+4` hors table (0x0a, jamais produits) ; les chiffres de wt-joueur2 venaient de zones différentes (l'échantillonnage dépend de l'ensemble des natives ciblées). **Étape 3** (appels par adresse hors familles qui visaient des natives des familles) : étincelles de la résolution des coups (`combat::World::spark`, 0xaa96c/0xaaa04/0xaaa84), réaction de l'ennemi frappé (`player::hit_reactions::enemy_reacts_68k`, native 0xf232e retirée), chocs des joueurs (`player::Ctx::shock`, 0xb10c8), créateurs des joueurs (`creators::for_players` : éclair 0x4b7a0, étincelles 0x4dc9a, tourbillon 0x4b8d0, effets 8 et 0x2f ; `set_a_68k`, `quake_68k`, `small::quake_b_68k`, `small::revive_68k` 0x4c930, `effects_creators::for_players::thrown_shard_68k` 0xb96c0), renvoi des morts-vivants du clerc (cinq appels écrits en clair), octet de table 0x3baea, luminosité 0x173c. **Scripts de scène** (`scene::script::script_routine`) : les entrées « appel » de tous les scripts de la ROM (table 0xf2f62, relevé exhaustif) n'appellent que 0x2a24, 0x2a2a, 0x2a30 (caméra), 0xa8ae8 (liste de décors), 0xd40e8 (tremplins), 0xdd12c (verrou des objets) : appels typés avec l'argument D0 ; natives retirées. Natives `Body::Processeur` : 594 → 584 ; `lecteurs.py` : 141 fonctions (familles 30). **Reste avant de retirer `m68k::Cpu`** : natives sur le processeur encore exécutées dans les 308 scénarios : `CREATE_TASK` (0x9b6, lancements de tâches par `crate::call`), `config_init`, `palette_pages_blanches` (0x1cf3c, `game::leave`), baguettes du magicien `spawn_rod_*` (table de tirs, `g.rom(shot, …)`), effacements de couches 0xca8/0xcc8/0xce8 (démarrage, modes) ; appels par adresse restants (23 sites) : tables du module `player/` (actions, pas, sous-états : `run_at`, `handler`, `routine`), son 0x4c58e/HEX_BYTE, `combat` (gestionnaires de tirs inconnus), `dispatch` (boucle d'aiguillage sur processeur), `startup`, `set_residue` (grilles), `creators` (0xcc0ca et ses appels, plus appelé que par sa native) ; puis le processeur comme banc des modules `player/`, `scene/`, `modes/`, `palette`, `video`, `kernel` (`lecteurs.py` : 141 fonctions, dont 30 dans `families/`, surtout des natives d'appel par adresse aux arguments en registres). Les 2 fonctions des familles encore sur `Cx<'_, Cpu>` sont des adaptateurs (`boss_common::push_out_of_boss_68k`, `enemy_common::set_dw`) ; `tools/regs_generic.py`/`regs_without.py` n'ont plus rien à convertir dans les tables d'aiguillage (toutes `#[without]`). |
| Sortie du Cpu, partie 5 (suite, boss restants : wt-boss2) | 4478a09 (boss 3, 5, 8, 9 ; 296/296 sans 68000), 07d1f81 (15, 19), 0eaa248 (11/12 ; 296/296 VALIDE dans les deux modes, DDSOM_LONG=1) | **Tous les boss sont en gestionnaires `#[without]`** (`lecteurs.py --famille boss` : plus que les adaptateurs des natives 0x818e6/0x819e2 et 0x930a4, appelée par `effects_c`). Boss 3 : formes du corps sur (bus, boss, objet), adaptateurs réduits à D0/CCR ; réaction : l'octet haut de D0 est nul (aiguillage, index de 0xfe5a). Boss 5 : saisie `+6` = 0xc (glitch d'origine : table de trois entrées, le saut tombe sur `moveq #0,D0 ; jsr 0xfd64` et « saisit » A1 = l'adresse du gestionnaire laissée par la boucle d'aiguillage, d'où une écriture en ROM et `+0x50` = 0x341c) : constante `HANDLER`, pas de reste A1 ; 0x2a42 (`small::background_1_lit_a`), 0x9b6 (`scene::fading::launch`). Boss 8 : comme le 7. Boss 9 : secousse à amplitude en argument. Boss 11/12 : recherches d'anneaux par valeur, D2 de 0x8f4c2 et de `life_rank` en restes ; glitch d'origine écrit en clair (le test du type relit le D0 mis à zéro par 0x19e10 : les deux types prennent les réglages du type 11). Boss 15 : `set_anim` sans registres (le réglage de 0xb3fb0 est le D1.b posé par 0x93182 : 0), 0x168e par `palette::or_brightness`, 0x76ec4/0xa7234/0xa727c par des cœurs de `small` sur `Regs` (`enemy_pair_kind_via`, `roar_via`), poussée des joueurs : 0x1386 par `aids::compute` (rendu `pub(crate)`), ses registres de travail D2-D4 en restes. Boss 19 : index des réactions = le code (`moveq` de l'aiguillage 0x90948), 0x63ae0 par `creators::enemy_7_pair_created`, 0x9b6 par `kernel::create_task`. Aucune modification de `Leftovers`. **`fuzz_enemy families` réparé** : sans 68000, `lib::check` faisait partir l'original du PC du banc (jamais posé par `call`) ; il part maintenant de l'entrée de la native (`stack_for_original`) ; l'outil résume les paniques et échoue si rien n'est comparé. Types ENEMY 0 à 28 (`DDSOM_ETATS=1`, 40 tours, kobold_scene7) : tous VALIDE (golem 11, troll 14...), sauf 28 (0x79300 cède la main dès le pas 0 : limite de l'outil, l'appel est abandonné) ; une panique « l'original ne revient pas » pour 26 (état tiré hors des rails). `fuzz_couv` C04 (9 scénarios de boss) contre main bba79f5 : aucune différence. **Reste** : `player/`, `set_b`, `effects_c` (autre agent), puis `regs_generic.py` et `regs_without.py`. |
| Sortie du Cpu, partie 4 : pile et interruption (wt-pile fusionnée ; suite wt-restes, validée, non fusionnée) | voir `git log` (wt-pile) | **Fait** (après fusion de main 29f0087) : (point 3) les retours ne sont plus identifiés par A7 : `Frame::calls` (profondeur des appels hors cadre à l'ouverture) et `Table::calls: Vec<OutsideFrame>` ; `crate::call` ne touche plus A7 ; `call`/`call_expected` sans `Cpu`. (Point 4) VBLANK = point du noyau `Point::Interruption`, point interrompu dans `Table::interrupted`, plus de PC/SR empilés. **Contexte réduit** (`tasks::table::Contexte`) : SR (masque du `trap #12`), A0, D4, D7, nommés et documentés ; le noyau ne gère plus ni A7 ni USP (plus de pile du superviseur, plus d'A1 = pile laissé par la cession) ; une tâche neuve ne reçoit plus A1/A4/D1 (personne ne les lit). Lecteurs trouvés par bissection (`tools/bisect_poison.py`, sondes `resume`/`neuve` de `diag::probe_launch`, site sans espaces ni virgules) : D4 relu par la phase du pas de jeu suivant (`Game(Step)`) ; A0 relu au retour d'un sous-programme qui cède (fondu de luminosité `Fading(Brightness)`, `Bonus(SetUp)`, `Exit(Both)`, `Exit(DispatchTail(18))`, `Transition(SpritesAfterFrame)`, `TransitionEarly(S4FadingOutNext)`, `TransitionLate(S32End)`) ; D7 (et A0) de la boucle d'aiguillage interrompue par une cession (`Firework(AfterWait)`, fondu appelé par un gestionnaire). Aucun registre hérité par une tâche neuve n'est lu. Sonde `stack` (`no68k::poison_stacks`) : FF0166-FF0E65 rempli d'une valeur changeante avant chaque point hors démarrage (FF0E66 et au-delà sont des variables du jeu relatives à A5 : `placement::LAST_X`...). **Validation empoisonnée** `DDSOM_POISON=resume,new,stack,=reprise#a0+d4+d7` : 296/296 VALIDE (DDSOM_LONG=1). **Validation complète** (640d635, DDSOM_LONG=1, JOBS=8) : 296/296 VALIDE sans 68000 et 296/296 depuis les assets (`DDSOM_ASSETS`, assets frais de main). **Resserrement de `kernel_ram` (wt-restes)** : n'ignore plus que FF0052-FF005D, FF0064-FF0163 et FF0166-FF0E65 (piles) ; FF0E66-FF0FFF (variables du jeu relatives à A5) est comparé : 296/296 VALIDE dans les deux modes (DDSOM_LONG=1), aucun écart. **Suite (wt-restes)** : (point 5) `dispatch::Leftovers` est un état de la partie, `tasks::table::Table::leftovers` ; le code encore sur le processeur les tient dans le banc (D2-D5) : `Native::execute` les passe du banc à la table à l'entrée d'un gestionnaire converti (`Body::Handler`) et les y rend à sa sortie (`Leftovers::from_bench`/`to_bench`) ; API de `Leftovers`, `Dispatch` et `cx::Bench` inchangée. **`Contexte` supprimé** : `Resume::Step(Step)` sans registres. Ce qui était relu est nommé : `dispatch::Step::Dispatch { class, object, counter }` (A0/D7 notés à l'appel du gestionnaire, remis dans le banc à la reprise de la boucle) ; `tasks::game::Step::Step { d4 }` (D4 du pas précédent, aussi dans `kernel::Point::StepKept { d4 }`) ; `tasks::fading::Step::Brightness { fading, a0 }` (A0 de l'appelant du sous-programme : bloc de la sortie de scène, objet d'une porte SET) ; `tasks::resume_step`/`resume_a0` : les étapes des séquences de sortie de scène (Exit, Bonus, Tally, Characters, Transition, TransitionEarly, TransitionLate) retrouvent le bloc `scene::screen::blocks::EXIT`, le feu d'artifice (`Firework`) l'objet de la boucle d'aiguillage en cours (`dispatch::current_object`). SR : une tâche reprend en mode utilisateur, interruptions permises (aucune ne cède en les masquant : `assert!` dans `kernel::cede` ; le seul `trap #12` du jeu, test de la mémoire du menu TEST, ne cède pas) ; les drapeaux ne sont pas relus. Sonde `cession` retirée (plus rien n'est sauvé à la cession). **Preuve** : avant le retrait, une validation complète où chaque valeur nommée était comparée (panique sinon) au registre rendu par le contexte : 296/296 VALIDE, aucun écart ; après le retrait (4764679, DDSOM_LONG=1, JOBS=8) : 296/296 VALIDE sans 68000, 296/296 depuis les assets (`DDSOM_ASSETS`, assets frais), et **`DDSOM_POISON=resume,new,stack` sans aucune exemption : 296/296 VALIDE** (plus aucun registre relu à la reprise d'une tâche). **Reste** : les restes D2-D5 ne quittent le banc qu'autour des gestionnaires convertis ; ils deviendront l'état unique quand la liste des sprites, les caméras, la jauge et les gestionnaires encore sur le processeur les écriront par leur nom (partie 5). |
| Sortie du Cpu, partie 7 : noyau et écrans (wt-cpu-core) — EN COURS | 55e0743 (étape 1 : 308/308 VALIDE en mode par défaut, DDSOM_LONG=1) | Voir § 19. Le jeu n'a plus de banc de registres dans ce périmètre : `scene::Ctx` sans `Cpu`, ordonnanceur sans registres (`Table::exam`, `Table::interrupt_mask`), boucle d'aiguillage qui appelle les gestionnaires sans registres, restes D2-D5 nommés (`dispatch::Leftovers` : jauge, sprites, caméras, copie des entrées, rang d'arrivée), pont `bench::on_bench` (16 sites : joueurs, combat, fin de partie, routines des scripts) vers le code des autres périmètres. Écarts trouvés à la première validation (end_06/07/09/41, scene_41, long_28_scenes) : l'A0 relu par l'aiguillage de la sortie de scène au retour d'une cession est le bloc (`Table::scene_exit_a0` posé à chaque image de la phase 16). Mesures (lignes avec `Cpu` / accès aux registres, `grep`) dans le périmètre : 166 / 391 avant, 83 / 161 après ; ce qui reste est fait de natives d'entrée pour le code d'origine (arguments en registres : `palette`, `video`, `sound`, enveloppes du noyau, phases et états de scène appelables par les outils), des correctifs de la machine de référence et du pont. |
