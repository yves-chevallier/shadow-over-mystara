# Refactorisation : du Rust « en forme de 68000 » au Rust ordinaire

La conversion est finie : `port/ddsom-game` fait tourner tout le jeu en Rust, 269/269 scénarios VALIDE sans 68000 (le
jeu, mode par défaut depuis le lot 0a, [noyau.md](noyau.md)) et depuis les seuls assets ([sans-rom.md](sans-rom.md)). Mais le code
garde la forme de l'original. Cette phase le rend lisible, en fonctions réutilisables, puis rapide, **sans changer le
comportement** : effets de bord, glitches, bugs d'origine, tirages du hasard, ordre des écritures en RAM. La validation
compare la RAM image par image avec l'original ; elle reste le filet, et l'état du jeu reste donc dans la RAM émulée.

Règles pratiques, exemples et check-list : [guide-refactorisation.md](guide-refactorisation.md). Mesure :
`tools/mesure_refacto.py` (forme 68000 par zone, `--doublons 6`, `--consts`), `examples/bench` (vitesse).

## 1. État des lieux (main 8b2612f)

150 000 lignes de code (177 000 avec les commentaires), dont 101 000 dans `families/` ; 2 046 natives (dont 536
enveloppes son générées). `tools/mesure_refacto.py` compte les marques de la convention 68000 :

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
   cherche la native dans l'index, l'exécute, dépile (`call_at`, `taches::table::mettre_sur_la_pile`). 4 776 appels ainsi.
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
   `families::Ctx::a_blanc`, `vue::Vue::simulee`, `objects::cost`, `scroll::Exec::simulated`, `evenements::plafonnements`).
   1 036 marques, sans aucun effet sur le jeu sans 68000, qui ne les lit pas. Pire, elles couplent le code : une logique
   qui cesse d'appeler une routine par son adresse fait paniquer le calcul du chemin (« branchement sans décision »), vu
   pendant le pilote.
6. **Les replis vers l'original.** `deleguer` (59), bras `_ =>` qui appellent une adresse (68), `g.rom` des joueurs (301),
   `x.call(0x…)` vers du code qui est de toute façon une native. Sans 68000, une adresse sans native arrête le jeu : chacun
   est soit un appel de native déguisé, soit du code jamais atteint (à lister, pas à garder en repli).
7. **La pile et les cessions.** Une tâche cède en posant le PC sur une native de reprise (`resume!`, 349), les appels en
   cours sont des cadres (`taches::table::Cadre`) remis sur la pile 68000 quand du « code d'origine » reprend la main,
   `Unwound` abandonne les fonctions Rust quand un appelé cède ou revient plus haut (`epilogue_etranger` des bulles).
   Tout cela imite une pile qui n'a plus de raison d'être quand appelant et appelé sont en Rust.

### Ce qui doit rester

* **La machine de référence** (`cps2`, `m68k`, `ddsom-rc`, `recomp`) : l'oracle, hors du chemin du jeu.
* **`validate`** et ses diagnostics (`DDSOM_CHECK`, `DDSOM_ADOPT_REGS`, `examples/cessions`, `reste68k`, `watch`...).
* **L'état dans la RAM émulée**, aux mêmes adresses, avec le même ordre d'écriture, tant que la validation compare la RAM.
  Les vues typées (`Obj`, `Player`, `ram::*`) nomment cet état sans le déplacer.
* **La table des tâches Rust** et son contrôle contre FF0064, le calendrier de l'image du jeu sans 68000 (`sans68k.rs`).
* **Les glitches** : allocation non testée qui écrit dans le premier emplacement ou après le pool, boucles qui insistent,
  registres résiduels relus. Ils sont marqués (`// Glitch d'origine : …`), jamais corrigés.

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
* Variables de la RAM de travail : `ram::Octet/Mot/Long/Table` (`src/ram.rs`). Les variables partagées sont dans `ram.rs`
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

et le noyau appelle `pas` au lieu de poser `c.pc`. Chemin : (1) un module de tâche range ses reprises en `match` sur une
`enum` (l'adresse d'origine devient `Etape::adresse()`, gardée pour la comparaison de la table des tâches avec FF0064) ;
(2) `table::Reprise` gagne une variante Rust (`Etape`) à côté de `Cession(Contexte)` ; (3) les sous-programmes qui cèdent
(fondus, écran noir) deviennent des sous-machines appelées par `pas`, leurs cadres des champs de l'état ; (4) quand plus
aucune reprise n'est une adresse, `Cadre`, `mettre_sur_la_pile`, `Unwound` et `resume!` disparaissent. Le contexte de
registres d'une tâche (sens d'un fondu dans D1...) devient un champ nommé de son état.

### Quand le `Cpu` quitte le chemin du jeu

1. **D1 — un seul mode de jeu** (décision préalable, lot 0 ; fait au lot 0a, § 6) : le jeu sans 68000 devient le jeu ; le mode normal (natives
   et 68000 de repli, durées) ne sert plus qu'au diagnostic `DDSOM_CHECK`, avec une durée constante par native. Les
   durées exactes, `Chemin`, l'exécution à blanc et `Vue` disparaissent (1 036 marques).
2. **J1 — plus d'appel par adresse entre natives** (`adr` → 0 hors tables de la ROM et noyau) : les lots.
3. **J2 — plus de registre hors frontière** (`regs`, `drap` → 0 hors adaptateurs `_68k` restants) : `DDSOM_POISON` par
   routine, puis suppression des adaptateurs.
4. **J3 — tâches en machines à états** : le noyau n'écrit plus `c.pc` ; `sans68k::pas` devient un appel direct.
5. Alors `Native::run` perd son `Cpu` (`fn(&mut Jeu)`), les contextes perdent `c`, et `m68k::Cpu` ne sert plus qu'à la
   machine de référence. Les natives restantes ne sont que la table « adresse → fonction » des tables de la ROM
   (gestionnaires par type) et le contrôle `DDSOM_CHECK`.

L'état lui-même ne quitte la RAM émulée qu'après J3, structure par structure, avec une projection comparée par image
(hors de cette phase).

## 3. Découpage en lots

Les conflits de fusion viennent des fichiers communs : `lib.rs` (listes de natives), `families/mod.rs`, `families/cx.rs`,
`objects/`, `mem.rs`, `ram.rs`, `taches/table.rs`, `kernel.rs`. Règle : **un seul agent touche les API communes** (lot 0),
les autres convertissent leurs appelants dans leurs propres répertoires ; un besoin d'API nouvelle se met d'abord dans le
module du lot (privé), puis remonte dans le socle à la fusion suivante.

| Lot | Périmètre | Dépend de | Taille |
|---|---|---|---|
| **0a** (seul, d'abord) | Supprimer les durées du mode hybride : champ `cost` réduit à une constante, `exact_cost`, `Chemin`, à-blanc de `families::Ctx`, `vue.rs`, `objects/cost.rs`, `timing.rs`, chemins de `evenements`/`credits`/`config`/`palette`/`scroll`. Valider le mode normal avec une durée constante | — | ~1 000 marques, tous les fichiers (mécanique) |
| **0b** (seul, d'abord) | API du moteur d'objets comme le pilote `pools` : `anim` (start, step), `motion` (move_*, speed_table, move_by_table), `lists` (reinsert, unlink), `cull`, `rank` : cœur + `_68k` + méthodes de `Cx`/`Ctx` qui appellent l'adaptateur directement ; `random` unique ; `combat::Obj` fusionné dans `objects::Obj` ; contexte commun (trait d'accès mémoire ou base `Jeu`) | 0a | `objects/`, `families/cx.rs`, `families/mod.rs`, `rng.rs`, `ram.rs` |
| A | Familles d'ennemis et de boss : `families/boss_*`, `boss_common`, `enemy*`, `reactions` | 0b | ~30 000 lignes |
| B | Effets, objets, coffres : `families/effects*`, `items*`, `boxes`, `petites`, `creators`, `effects_creators`, `effects_aids.rs` | 0b | ~30 000 lignes |
| C | Tirs et décors : `families/eshl*`, `set*`, `sobj`, `shell`, `joueur/`, `eshl_creators` | 0b | ~35 000 lignes |
| D | Joueurs et monde : `player/`, `combat/`, `combat.rs`, `ai.rs`, `ai_aids.rs`, `terrain.rs`, `scroll.rs`, `camera_modes.rs`, `placement.rs` | 0b | ~22 000 lignes |
| E | Tâches et écrans : `taches/` (machines à états, J3), `kernel.rs`, `vblank.rs`, `scene/`, `modes/`, `hud/`, `tache_credits.rs`, `ecran_credits.rs`, `service/`, `eeprom*`, `game.rs`, `dispatch.rs` | 0a | ~20 000 lignes |
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
  par pixel). `validate` rendait les deux machines à chaque image sans regarder l'image : `Machine::rendu = false` dans
  `validate` et `bench` (socle, sans effet sur l'état) divise le temps de validation (mesure dans § 5).
* **Sans rendu, pilote son Rust** : `qsound` (filtres `Fir::apply`, `update_sample`, voix) 72 % ; le jeu lui-même ~15 %
  (~25 µs par image) : accès au bus `Cps2Bus::read_word`/`write_word` (décodage d'adresse à chaque accès, 7 %),
  `latch_palette` 2 %, `Scenario::apply` 1,4 % (outil), `lookup` des natives (table de hachage SipHash) 1 %, gestion de
  la pile 68000 des tâches (`mettre_sur_la_pile`, `reprendre_de_la_pile`, `depiler_retour`, `type_id` de la table des
  tâches rangée en `Any`) ~1,2 %, `dispatch::Class::sweep`, `idle`.
* **Z80 émulé** : le programme du Z80 coûte autant que tout le reste (356 contre 183 µs).

Pistes pour le lot F, par gain attendu : rendu (couches en lignes de tuiles plutôt qu'en pixels, palette précalculée) ;
QSound (FIR vectorisé, ou mixage à la fréquence de sortie) ; dans le jeu, l'accès direct à la RAM de travail (tableau,
sans décodage d'adresse) puis la disparition du chemin `call`/cadres (J1, J3) et de `lookup`. Le jeu en Rust n'est pas le
goulot : l'optimiser avant les lots n'aurait pas de sens.

### Lot F : résultats (branche wt-perf)

Contrôle bit à bit : `examples/controle_rendu <roms> <eeprom> <scénario> <images> [sortie]` imprime, par image, le
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
   (purs) ; `alloc_68k`, `alloc_unchecked_68k`, `release_68k` (registres de l'original, `diag::registres_morts`) ;
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
`tools/validate_all.sh`, de `bench` et de `lectures_rom` ; `DDSOM_HYBRIDE=1` choisit le mode hybride (natives et 68000
de repli), qui ne sert plus qu'au diagnostic et n'a plus à finir VALIDE (`DDSOM_SANS_68K` n'existe plus).

* `Native` perd `cost` et `exact_cost` ; en mode hybride chaque native dure `DUREE_NATIVE` (100 cycles), et sans 68000
  rien (`lib.rs`, `executer`). `Stats` perd `original_cycles`/`native_cycles` (contrôle `DDSOM_CHECK` : mémoire et
  registres seulement).
* Supprimés : `chemin.rs`, `timing.rs`, `vue.rs`, `objects/cost.rs` ; l'exécution à blanc et le relevé de chemin de
  `families::Ctx` (`a_blanc`, `at`, `pris`, `externe`, `sans_chemin`, `jsr_indirect`, `cout`, couverture des
  décisions), de `ai::Mach` (`dry`, `seg`, `span`, tirage prédit, `refund`), de `scroll::Exec` (`Sim`, `mark`,
  `repeat`, `helper`) ; les fonctions de coût de chaque module (palette, vidéo, crédits, configuration, EEPROM,
  événements, son, entrées, objets, combat, texte, démarrage, service) et les arguments de coût des macros
  (`handlers!`, `entry!`, `natif!`, `handler`, `fixed`...). Les fonctions écrites sur `Vue` (réelle ou simulée)
  prennent le bus.
* Le comportement ne change pas : seules des lectures sans effet et des décisions recopiées disparaissent (`ctx.pris(c)`
  devient `c`). Les exemples `familles`, `fuzz_familles`, `fuzz*`, `recensement` perdent la comparaison des cycles.
* Mesure (`tools/mesure_refacto.py`, total) : `duree` 1 036 → 11 (restent des faux positifs : le prix d'un crédit,
  `cost`, dans `hud/slot.rs`, `sprites.rs`, `tache_credits.rs`) ; lignes de code 149 834 → 147 861 ; au passage `regs`
  5 907 → 5 865, `drap` 643 → 642, `adr` 4 581 → 4 584 (`call` explicites qui remplacent `Vue::appeler`).

## 7. Lot E : les tâches en machines à états (J3)

Modèle et marche à suivre : [guide-refactorisation.md](guide-refactorisation.md), « Exemple : une tâche qui cède » ;
détail du noyau : [noyau.md](noyau.md), « Tâches Rust ».

* **Pas 1-2 (étapes, `Reprise::Etape`).** Chaque point de reprise est une variante d'une `enum Etape` de son module
  (`etapes!`), réunies dans `taches::Etape` ; `table::Suite` (étape ou adresse d'une native) désigne la suite d'une
  cession et d'un appel en cours. L'ordonnanceur remet le contexte puis appelle le pas (`taches::executer`) au lieu de
  poser `c.pc` ; une tâche neuve dont l'entrée est une étape démarre de même. Converties : gel, invites, écran des
  scores, menu TEST, interface des joueurs, crédits, fondus, partie (entrée, début de tour), portraits de la boutique,
  machine à modes (démarrage, attract, titre, choix, classement, démonstration, fin, fin de partie) et séquences de
  scène (sortie de scène, fondu au noir, transitions, bonus, boutique, personnages, comptes) : 349 `resume!` → 4.
* **Pas 3 (sous-machines).** Fondus et écran noir sont des fonctions Rust appelées directement
  (`Sequence::sous_programme`, appel terminal) ; ce qu'ils gardaient sur la pile (sens d'un fondu dans D1, registres
  d'un `movem`, D0 d'une impulsion de verrouillage) est un champ de leur étape. Un `Cadre` n'est plus qu'une suite :
  `Contenu::Registres`, `Valeur`, `garder`, `reprendre` ont disparu.
* **J1 dans le périmètre.** Plus d'appel par adresse aux enveloppes du noyau (`0x9b6`, `0xa14`, `0xb64`, `0xa40`,
  `0x9f8`, `0xae4` : `kernel::creer_tache`, `tuer_tache`, `taches::une_image`, `attendre`, `terminer`,
  `recommencer`), ni aux fondus et à l'écran noir.
* **Pas 4, ce qui reste (et pourquoi).** `resume!` : le pas de jeu `0x1c4b0` (la boucle d'image teste le plafond de
  pas, `cps2::StepCap`, sur le PC), les retours des phases de la démonstration `0x1f98c`/`0x20a98` (adresse de retour
  d'un `call_at` vers une phase qui peut céder), le fondu au noir `0x1460` (appelé par `call_at` par les objets SET,
  `families/set.rs`, `set_b.rs`) ; plus depuis le lot C, § 8). `Cadre`, `mettre_sur_la_pile`, `Unwound` et `Reprise::Cession` restent pour les
  appelants hors du lot : gestionnaires d'objets qui cèdent par `call_at` (fondu des SET, reprises `game.rs`
  `WORLD_RESUMES`/`PHASE_12_RESUME` et `dispatch.rs` `RESUME_*`, adresses de retour de leurs `call_at`), épilogue
  étranger des bulles (`families/mod.rs`, qui dépile des cadres remis sur la pile ; l'étape `Tour` qu'il atteint est
  retrouvée par `table::etape_depilee`), `families/effects_g.rs` (`table::appeler`). Ils disparaîtront quand les
  familles (lots A-C) ne céderont plus par `call_at`.

Mesure (`tools/mesure_refacto.py`, zones du lot : `scene`, `modes`, `hud`, `taches`, `service`, `eeprom`,
`tache_credits`, `game`, `kernel`, `vblank`, `ecran_credits`, `dispatch`) : lignes 18 724 → 18 693, `adr` 625 → 560,
`pile` 407 → 67, natives 389 → 49 ; `regs` (886), `drap` (23), `brut` (1 837 → 1 836) inchangés : le lot a porté sur
J3, pas sur J2.

## 8. Lot 0b : l'API du moteur d'objets

Fait sur le modèle du pilote (§ 5), décrit pour les lots suivants dans le guide (« L'API commune (lot 0b) »).

* **Cœurs, adaptateurs, natives** pour `anim` (start, start_entry, start_by_character, step), `motion` (ballistic,
  move_by_table, select_speed, suspended, tick_counters), `lists` (reinsert, unlink, unlink_all), `cull`
  (despawn_if_offscreen, in_arena, update_visibility), `rank` (reset, load_preset, tick, raise, lower) et `rng`
  (random, seed). Les natives n'appellent plus que la forme « registres » de l'adaptateur, plus `rts`.
* **Contexte commun** : trait `machine::Machine` (choisi plutôt qu'une base `Jeu` : les accès disjoints `x.c`/`x.b`
  ne survivraient pas à un `Deref`), implémenté par `Cx`, `families::Ctx`, `player::Ctx`, `scene::Ctx`,
  `combat::World` ; `random`, `prog*`, `data*`. Trait `objects::Courant` pour les gestionnaires (`x.move_xz()`,
  `x.anim_step()`, `x.reinsert()`...), qui remplace les méthodes inhérentes de `Cx` et `Ctx` (et `Ctx::natif`).
* **Un seul `random`** (`rng::random`) : copies de `camera_modes`, `player::Ctx`, `ai_aids`, `effects_b` supprimées ;
  ~90 sites `x.call(RNG)` convertis. **`combat::Obj`** fusionné dans `objects::Obj` (`from_word`, `word`, `long_ref`).
* **Appelants** : ~1 400 appels par adresse convertis par scripts (familles, joueurs, scènes, modes ; appels de lot E
  limités à une ligne). Lectures immédiates de drapeaux et de D0 repliées dans les valeurs rendues (`if x.anim_step()`,
  `x.move_xz().gt()` via `motion::LastAdd`, `let alea = x.random()`). Constantes d'adresse recopiées supprimées
  (dont des noms faux : `MOVE_Z = 0x1da6` déplaçait en X et Z, `MOVE_Y = 0x1dbc` en Z, `PLAYER_ARRIVES = 0x1f300`
  baissait le rang).
* **Glitches mis au jour** : durée d'animation nulle qui repart de 0xFFFF en levant la retenue (`anim::Step::Wrapped`,
  vu par `ending`) ; registres de la réinsertion relus par le gestionnaire suivant (`Courant::reinsert` garde
  l'adaptateur).
* **Preuve** : `DDSOM_POISON` gagne les sites (`étiquette@motif`, `!étiquette@motif`, `#[track_caller]` sur
  adaptateurs et méthodes). Validation complète empoisonnée (`anim,motion,lists,cull,rank,random`), lecteurs isolés
  site par site, puis méthodes passées au cœur. Restent 88 appels `_68k` hors modules du socle (liste dans le guide) :
  natives appelées par adresse, aides à contrat de registres, lecteurs documentés.

Mesure (`tools/mesure_refacto.py`, avant : b53a8b1 ; après : lot 0b fusionné avec le lot E de main, dont les chiffres
`pile` et `natives` de `scene`/`modes` viennent) :

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
  `Machine`) ; les variables de la RAM sont typées dans `player::ram` (`ram::Octet/Mot`, celles de `crate::ram`
  réexportées ; nommées : `SPELL_ROW`, `CURSE_COUNT`, `SCROLL1_Y`, `FADE_*`, `CAMERA_FOCUS`, `BOSS_SIGNAL`, `DSW`...).
  `combat::World` perd `u8/u16/u32/set8/16/32`, `a5_8/a5_16`, `pc8/pc16` et son `random` à registres : vues `Obj`,
  variables partagées avec `player::ram`, `Machine`, files de collision prises dans `objects::pools`
  (`QUEUE_*`).
* **J1 dans le périmètre.** Aiguillage des actions des personnages en Rust (`characters::routine`, tables
  0x35918-0x35a48 relues dans la ROM ; `OVERRIDES` vide supprimée) ; décor (`terrain::push_68k`), placement et
  palettes d'effet (`placement`), secousse de caméra (`camera_modes::start_shake`, cœur + natives), créateurs
  d'effets joueur (`spawn::swap`, `weapon_trail`, `thrown`, `lightning_target`, `wall_target`), animations par
  variante : appels de fonctions Rust au lieu de `g.rom(adresse)`. Restent par adresse (`Ctx::rom`) : les sons, les
  palettes de `palette.rs`, les effets de `families`, `video::terrain` (0x24b0), déclarés une seule fois
  (`player::externes`) en attendant leur API.
* **J2.** `DDSOM_POISON` (scénarios du lot, 90) a prouvé morts et fait passer au cœur : rang, animations,
  allocation (combat, ai_aids), vitesses, zone de jeu, tirage du combat ; registres du décor (`terrain`), du
  placement et des palettes (`placement`) ; D0/D1/A1 après les créateurs, secousses et animations convertis
  (étiquette temporaire). Le pas d'animation des joueurs suit maintenant la retenue de l'original (durée nulle :
  `Step::finished`, glitch d'origine).
* **Replis et trous.** 13 replis `_ => g.rom(adresse propre)` des automates (jamais atteints, parfois récursifs)
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
* **Mesure** (`tools/mesure_refacto.py`, zones du lot, avant c8107db / après) : lignes 20 722 / 20 878 (dix routines
  ajoutées), `regs` 756 / 755, `drap` 104 / 110, `adr` 354 / 285, `brut` 917 / 895, `a5` 138 / 78 ; adresses
  déclarées dans plusieurs fichiers du lot 43 / 0 (112 / 0 déclarations) ; lignes dupliquées (`--doublons 6`) de
  `player` 1 464 / 1 497, `combat` 38 / 30, `ai` 8 / 0 ; appels `_68k` 17 / 13 (les 13 restants sont décrits
  ci-dessus).
* **Pas faits** : `ai.rs`/`ai_aids.rs` (natives à contrat de registres appelées par les familles), `scroll.rs`,
  `combat` (`call_ao` vers effets et sons hors lot), `--doublons` des personnages.

## 10. Lot C : tirs et décors (`eshl*`, `set*`, `sobj`, `shell`, `joueur/`)

* **Fondu au noir des portes sans `call_at`.** Les portes SET 7 (`set.rs`, `exit_gate`) et SET 29 (`set_b.rs`,
  `door_b`) appellent `taches::fondu::descente` comme un sous-programme de tâche (`set::fondu_de_sortie`) : la suite
  est une étape `fondu_de_sortie::Etape` (`taches::Etape::PorteSet`) qui porte la porte ; à la fin du fondu la tâche
  y reprend, puis revient dans la boucle de dispatch par l'adresse de retour du gestionnaire. Les fonctions Rust de la
  boucle sont abandonnées par `Unwound`, comme le faisait `call_at`. Disparaissent : `resume!(FONDU_DESCENTE, 0x1460)`
  (`taches/fondu.rs`) et les natives de reprise 0xcb896, 0xcb9c0, 0xd8f06. `fuzz_porte` suit maintenant le fondu
  jusqu'au bout (étapes de la tâche) et couvre aussi la porte 29, que les scénarios n'atteignent pas
  (`DDSOM_PORTE=29`).
* **J1.** Plus d'appel par adresse vers les natives du lot : créateurs ESHL (`eshl_creators::appeler`, chaque
  créateur est une fonction publique `(c, b)`, la native n'ajoute que `rts`), routines de vague (`set_residu`),
  interprète du dialogue de la boutique (`peff_boutique::talk_a/b`, natives 0x514ac/0x514cc retirées), création de
  la tâche de fondu (`kernel::creer_tache` au lieu de 0x9b6). Les adresses des natives d'autres modules appelées par
  le lot ne sont plus recopiées : `crate::camera_modes::SHAKE_A.entry`, `terrain::SHADOW`, `palette::*`,
  `text::CLEAR_BOX`, `aides::*`, `video::*`, `ai::KEEP_ON_SCREEN`, `scene::*` (60 constantes locales, dont des noms
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
  en argument. (2) Les **créateurs** (`eshl_creators`, `set_residu`, tremplins) : leurs registres de sortie (A1,
  retenue, compteurs) sont relus par les gestionnaires d'ennemis et de boss (lot A) et par les sons qui suivent ;
  empoisonnés, une trentaine de scénarios divergent. Ils gardent leur contrat.
## 11. Lot B : effets, objets, coffres

Périmètre : `families/effects*.rs`, `effects_creators.rs`, `items*.rs`, `boxes.rs`, `petites.rs`, `creators.rs`,
`effects_aids.rs`. Code mort et replis : [code-mort.md](code-mort.md), section du lot B.

* **Créateurs écrits une fois** (`effects_creators::restes::Registres`). Chaque créateur (allouer, poser type,
  variante, réglage, lien) est une fonction générique sur un `Registres` : `Pur` (le cœur, aucun registre) ou le
  processeur (natives appelées par adresse et adaptateurs `_68k` : l'allocation passe par `Pool::alloc_68k`, les
  drapeaux du dernier `move`, les compteurs de boucle et les tirages vont dans les registres de l'original). Plus
  de seconde implémentation « à registres » à tenir à jour. Convertis : tous les créateurs d'EFFCT
  (`effects_creators.rs`), les étincelles et chocs (`effects_aids.rs`), les créateurs de `creators.rs`/`petites.rs`
  appelés par le lot ou demandés par le lot D.
* **Cœurs pour le lot D** (appelants par adresse de `player/externes.rs`, à brancher) : `effects_aids::shock`
  (0xb10c8, `DUST`), `effects_creators::halo` (0xab708), `blow` (0xb378a, `STARS`), `creators::set_a` (0xcc352,
  `WAND`), `creators::sparks` (0x4dc9a), `bolt` (0x4b7a0), `quake` (0x4b340), `petites::quake_b` (0x4f132), chacun
  avec son adaptateur `_68k` (registres de l'original, étiquette `effets`). `HEALTH_BAR` (0x19dce) est dans
  `evenements.rs`, hors lot.
* **J1.** Plus d'appel par adresse vers les créateurs du lot, le décor (`terrain::push_68k`), la secousse
  (`camera_modes::start_shake`), la création de tâche (`kernel::creer_tache`), les éclairs et pages de palette
  du lot (`petites`), la remise à zéro de scène (`creators::restart_scene`). Restent par adresse (pas d'API Rust) :
  les sons (enveloppes de `sound.rs`), `evenements` (0x19ae6, 0x19b08), `ai` (0xfbae, 0xfbb0, 0x103d6, 0x137e,
  0x1172e), palettes de `palette.rs`, vidéo, scène (0x33aa6, 0xf2e56, 0x20de6), 0x3403c (second décor). Les
  créateurs ESHL du lot C (fusionné) sont appelés comme fonctions Rust, y compris par la table des effets spéciaux
  des coffres (`match` sur l'adresse lue).
* **J2.** `DDSOM_POISON=effets` (registres des créateurs après les appels convertis) puis `DDSOM_POISON=restes`
  (registres laissés par les gestionnaires : restes des aiguillages `saut_table`, pointeurs de lecture, compteurs
  de boucle) : 270/270, écritures supprimées. Un lecteur trouvé : le nuage (type 26) relisait le mot haut de D0 de
  l'aiguillage (nul) après la remise en liste ; écrit en clair. Restent des lectures de registres réels : D2 hérité
  par `items_b::drop_old` (compteur de coups de la pièce rejetée, glitch d'origine), A2 de `guide_cible` (aucun
  joueur vivant), A1 laissé par la boucle de dispatch (`effects_c`, adresse du gestionnaire), D0 des tirages
  (`effects_c`, `effects_d::random`), D4 hérité du radeau (`effects_e`), D6 de l'allocation dans la boucle des
  éclats (`effects_f`, glitch d'origine documenté).
* **Replis.** 68 aiguillages `_ => deleguer(cible)` relevés table par table : état 6 (libération) explicite là où
  la table l'a, `hors_table` ailleurs ; deux branchements « pool plein » vers une erreur d'adresse de l'original
  (`erreur_d_adresse`).
* **Cession (lot E).** Le feu d'artifice (type 80) fait attendre la tâche de partie au milieu de la boucle de
  dispatch : ses reprises sont des étapes (`effects_g::Etape`, ligne ajoutée à `taches::etapes_des_taches!`) au lieu
  de natives à adresse et de `call_at` ; l'abandon de la boucle reste un `Unwound` (la boucle de dispatch n'est pas
  une machine à états). Nouveau scénario `cov/cov_effct_feu` (le chemin n'était exercé par aucun) ; ses lectures de
  la ROM manquaient aux assets sans ROM : cinq zones ajoutées à `port/extract/zones_natives.txt` (fichier local
  généré) et `natives.json` des assets régénéré.
* **Nommer.** Variables de la caméra, de la scène et de la phase par `ram.rs` (85 accès) ; constantes d'adresses
  recopiées supprimées avec leurs appels.
* **Contrats des natives restantes.** `fuzz createurs` (`examples/fuzz.rs`, mode hybride, contrôle) compare chaque
  créateur du lot à l'original sur des états tirés : 0 écart après correction de drapeaux que l'ancienne écriture
  oubliait (X du `lsr` du choc, `moveq #0,D0` de la colonne ESHL au pool plein, X de la moitié droite, `moveq` des
  douze blocs de palette, `move.b D1` du décor 11, table des décalages du joueur scripté lue en ROM).
* **Mesure** (`tools/mesure_refacto.py`, fichiers du lot, avant 1f5ae77 / après) : lignes 16 022 / 16 104 ; `regs`
  729 / 425 ; `drap` 40 / 38 ; `adr` 266 / 124 (les sons, `evenements`, `ai`, palettes et vidéo hors lot) ; `pile`
  1 / 2 (`Unwound` explicite du feu d'artifice) ; `brut` 490 / 451 ; `a5` 263 / 174 ; adresses déclarées dans
  plusieurs fichiers touchant le lot 51 / 39 (72 / 47 déclarations du lot) ; lignes dupliquées (`--doublons 6`)
  221 / 223.
* **Restent en `_68k` (et pourquoi).** Les natives appelées par adresse (créateurs de `creators.rs`, `petites.rs`,
  `effects_creators.rs`, `effects_aids.rs` : leur contrat de registres sert aux appelants hors lot), `regs_68k` de
  la boucle des éclats (`effects_f`, D6 de l'allocation relu, glitch d'origine), `terrain::push_68k` (pas de cœur,
  il appelle `video::terrain` par ses registres), `random_68k` des tirages dont D0 est relu (`effects_c`),
  `petites::flash_68k` (drapeaux non prouvés morts), `start_by_character_68k` (`effects_f`, D6 sauvé autour).

## 12. Lot S : services communs (son, palettes, texte, terrain, aides, événements)

Périmètre : `sound.rs`, `palette.rs`, `text.rs`, `video.rs`, `aides.rs`, `evenements.rs`, `terrain.rs`,
`placement.rs`, `ai_aids.rs` (et `ai.rs`, `scroll.rs`, `camera_modes.rs`, laissés par le lot D), plus les
appelants hors des familles des lots A et B. API et mode d'emploi : [guide-refactorisation.md](guide-refactorisation.md),
« Les services communs (lot S) ».

* **La sauvegarde fantôme du son disparaît** (écart assumé, [son-68000.md](son-68000.md)). Trace des lectures sur la
  machine d'origine, 269 scénarios : la copie D0-D7/A0-A4 que chaque wrapper laisse sous le pointeur de sa pile
  privée (FF297E-FF2A19, trois niveaux d'aiguillage au plus) n'est relue que par les restaurations des wrappers et
  par le test de la RAM du menu TEST (0x2d096, qui remet la valeur lue). Elle était déjà dans la zone ignorée par
  `validate` (FF2900-FF2A19) : le blocage signalé par le lot C venait des registres que les sons **lisent** (A0,
  D4, A1), pas de la copie. `validate` n'ignore plus que la sauvegarde (`SAUVEGARDE_FANTOME`) : la fin de la file
  des sons, ses têtes et l'alternance des voix (FF2900-FF2961) sont maintenant comparées.
* **Cœurs.** Son : `jouer(b, son, objet)`, `jouer_global`, `jouer_avec(b, son, Source)` (valeurs `Option` : un son
  qui lit une valeur absente s'arrête), `service(b, Service::…)`, `emettre` ; aiguillages et entrées « selon la
  version » dans le cœur ; la liste `sound::NATIVES` (inutilisée, en double) supprimée. Palettes : luminosité,
  éclairs (`Flash`), couleur d'éclair, palette du personnage. Texte : `print`, `erase`, `print_over` (un seul
  moteur `texte`), `print_tall(_at)`, blocs, `clear_box`, qui rendent la fin du descripteur. Vidéo :
  `terrain(b, x, y) -> Terrain`, `terrain_of`, `terrain_68k`. Aides : `direction*`, `grabbed -> Saisie`, `game_over`
  (sans appel par adresse : `palette::character_palette`, `player::actions::to_stand`). Événements : files de
  points, crédit, pop-ups (`Popup`). Terrain et placement : `terrain::push(b, o)`, `placement::place_on_free_ground(b,
  o)` (plus d'adaptateur `_68k` : registres de 0x24b0 pris dans `Terrain`). IA : `ai_aids::stance`, `in_front`,
  `box_around -> DansBoite` (boîte : `Boite`). `ram` : `JOUEURS_EN_VIE` (0x35, ex-`DIFFICULTE`), `DEFILEMENT_Y_PLAN1`
  (0x4ba), `DSW` (0x7fd0), réexportés par `player::ram`. Les natives restent pour les appels par adresse et
  reproduisent les registres de l'original.
* **Appelants convertis** (hors familles des lots A et B) : ~330 appels de son (script `tools/convertir_sons.py`,
  puis à la main ceux qui lisent D4/A1 ou prennent une adresse calculée : la valeur posée dans D4 devient l'argument),
  ~110 appels de palettes, textes, directions, terrain, événements, fin de partie. Noms faux corrigés : les
  « scripts d'animation de palette » de la phase 0 (`game.rs`, 0x1324) effacent des rectangles de texte
  (`TEXT_BOXES`) ; `DIFFICULTE`.
* **Registres gardés, et pourquoi.** `pshl::stopped_by_terrain` (et, par prudence, `eshl.rs`, `shell.rs`) gardent les
  registres de 0x24a8 (`terrain_68k`) : D3 est relu par un gestionnaire suivant (`boss11_scene42_mort`, image 4447,
  la divergence est apparue en passant au cœur) ; `peff_e.rs` : D2 du terrain relu par le rebond du suivant
  (lot C). `combat.rs` (`jouer_avec` des aiguillages 0xe662a/0xe823a : D4 posé par l'appelant pour la cible, peut-être
  un `ParD4`). Les tours de `peff_m`, `peff_n`, `pshl_d` lisent encore leur cible dans A1 (posé par l'appelant).
* **Glitch mis au jour.** La barre de vie (0x19dce) laisse A2 sur la file des pop-ups, et `player_hits_enemies`
  garde A2 comme boîte d'attaque du joueur pour les ennemis suivants : après un coup qui affiche une barre, les
  ennemis suivants de la liste sont testés contre la file des pop-ups (`combat::show_enemy_hp`, marqué ; vu en
  passant au cœur `evenements::creer_popup` : une dizaine de scénarios divergeaient).
* **Fuzz.** `fuzz son` (wrappers, aiguillages hors sélecteurs aléatoires, services) et `fuzz video` sans écart ;
  `fuzz u1` sans écart sur les natives du lot (un écart antérieur, hors lot : D6 de `popups_effacer_tout`,
  `popups.rs`, 0x0000ffff attendu) ; `fuzz_monde ia_` (nouveau : 0xa470, 0x7f0c, 0x7ef4, 0x7f19c) : mêmes écarts
  avant et après (drapeau X seulement, que ces natives n'ont jamais reproduit).
* **Mesure** (`tools/mesure_refacto.py`, avant 44d7794 / après) : total `adr` 2 896 / 2 347, `regs` 5 596 / 5 365,
  `drap` 536 / 522, lignes 148 020 / 148 078 ; `player` `adr` 263 / 71, `scene` 257 / 173, `modes` 185 / 158,
  `families` (lot C seulement) 2 053 / 1 839 ; modules du lot : `sound` `regs` 54 / 24, `aides` 46 / 28,
  `evenements` 43 / 35 (`drap` 30 / 18), `placement` 16 / 8, `ai_aids` 273 / 249 ; adresses déclarées dans plusieurs
  fichiers 214 / 196 (940 / 872 déclarations). Les `_68k(` passent de 116 à 133 : les adaptateurs du lot portent
  maintenant ce nom (`terrain_68k`, `enfiler_68k`...), alors que les natives les contenaient sans le dire.
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
  événements, `ai_aids::stance/in_front/box_around`, `effects_creators::attack/debris`, `aides::direction*`,
  `aides::grabbed`, `video::terrain_of`) ; restent par adresse dans `enemy_lib` les routines sans cœur (le reste de
  `ai.rs`/`ai_aids.rs` : tirages 0x788c/0x79ce/0x79f6/0xac3a/0xf118, PV, immunités, cibles, saisie 0xfc44/0xfd64/
  0xfd8c/0xfda2, butin, débris 0x1172e/0x11a36, poussée ; secousses de `camera_modes.rs` ; quelques créateurs).
* **Noms faux corrigés** (vérifiés par valeur) : `FACE_TARGET = 0xa470` tirait l'attitude (`lib::draw_stance`),
  `TURN_TO_TARGET = 0x788c` un octet de table (`draw_6b`), `SOUND`/`SHOCK = 0x2824` secouait la caméra
  (`shake_54`), `SHADOW = 0x33fe4` est la collision avec le décor (`collide_terrain`), `boss_common::facing_target`
  testait le sens de la vitesse (`moves_forward`), `target_in_box_far = 0x7f0c` teste une cible en garde
  (`guard_in_front_strict`), `SPEED_Y = +0x16` est l'accélération en X (`Obj::AX`), `ANGLE = +0x2d` l'index de
  la table de vitesses, `DIFFICULTY = A5+0x7f92` le rang dynamique (`var::RANK`).
* **État nommé** : champs génériques par `Obj::X`, `Obj::VX`... et `f::SUB3`, `f::KEY_FRAME` (15 copies des mêmes
  modules de champs supprimées) ; variables de la RAM de travail typées (`enemy_common::var`, `ram::Octet/Mot`).
* **Natives du périmètre en fonctions Rust** : 0x27ee (détachement de l'hôte), 0x81aa4, 0x833b6, 0x87048
  (`boss_common`), 0x81ae0 (riposte du boss 3), réactions communes (`reactions::run_common`, appelées directement
  par `enemy_common::react`, `react_with` pour la routine propre d'un boss) ; leurs natives ne servent plus qu'aux
  appels par adresse restants.
* **Routines d'origine sans native** appelées par les familles, jamais atteintes (le jeu sans 68000 s'y serait
  arrêté) : transcrites dans `enemy_lib` ([code-mort.md](code-mort.md), lot A).
* **J2 : registres résiduels.** Les écritures de registres que plus rien ne relit en Rust (copies des registres de
  l'original, ~800) ont été instrumentées une à une (`registres_morts`, une étiquette par site). La validation
  empoisonnée, puis des bissections automatiques sur les sites (outil de travail : `DDSOM_POISON=lota@fichier:ligne:`),
  ont désigné les écritures relues par la suite : un gestionnaire suivant (A1 de `load_target`, D1-D4 du boss 15,
  D5 de l'archer, D2 du boss 6...) ou un compteur de boucle relu au tour suivant. Elles restent, marquées
  `// relu : …` ; 725 autres sont supprimées. `boss_1` et `boss_9` (lecteurs nombreux) gardent leurs registres.
  Piège : une écriture dans une boucle peut être relue au tour suivant (lecture placée avant dans le texte).
* **Besoins d'API** (signalés) : cœurs pour le reste de `ai_aids.rs` et `ai.rs` (tirages, cibles, PV, immunités,
  saisie, butin, débris), les secousses de `camera_modes.rs`, les créateurs 0xd5f30/0xd6952/0xd5ef8 (SET de la barre
  de vie), `popups::effacer_tout` (0x19eb0), 0x57c66/0x5cdc6 (`creators.rs`, privés) ; `ram::DIFFICULTE` était le
  nombre de joueurs en vie (corrigé au lot S).

Mesure (périmètre du lot, `tools/mesure_refacto.py` restreint aux fichiers du lot ; avant c8107db / après) :

| | lignes | regs | drap | adr | brut | a5 | `_68k(` | doublons (6 lignes) | adresses déclarées aussi ailleurs |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| avant | 56 744 | 1 729 | 111 | 1 423 | 213 | 426 | 7 | 13 464 | 70 (294 décl.) |
| après | 55 330 | 973 | 21 | 161 | 191 | 191 | 12 | 12 579 | 11 (18 décl.) |

Des 161 `adr` restants, ~100 sont les appels de la façade `enemy_lib` (un par routine d'un autre lot) ; les autres :
tables de la ROM (réactions propres à un type, créateurs par table), quelques créateurs appelés une fois, natives
`(c, b)` du périmètre. Les `_68k(` gardés : `random_68k` (D0 relu), `terrain_68k` (registres de 0x24a8 relus),
`shock_68k` (A1, D1, retenue relus), `in_arena_68k` (`bhi` de `reactions.rs`).

## 14. Lot « noyau final » : J3 achevé, l'ordonnanceur en Rust

Périmètre : `dispatch.rs`, `game.rs`, `taches/`, `kernel.rs`, `vblank.rs`, `sans68k.rs`, `demarrage.rs`, `lib.rs`,
`families/mod.rs`, `families/cx.rs`, et les reprises restantes de `modes/` (boucle, démarrage, démonstration),
`families/set.rs` et `effects_g.rs` (cessions des gestionnaires). Détail : [noyau.md](noyau.md), « Ordonnanceur et
traps », « Appels en cours en Rust », « Le jeu sans 68000 ».

* **Plus de cession par la pile 68000.** Les appels en cours de la boucle de partie sont des étapes : fin d'image
  (`partie::Etape::FinImage`, ex-native `0x1c4c4`), fin de la phase 12 (`game::Etape::ApresMonde`, ex-`PHASE_12_RESUME`),
  mise à jour du monde (`game::Etape::Monde { ordre, etape }`, ex-`WORLD_RESUMES`, 46 natives), boucles d'aiguillage
  (`dispatch::Etape::Aiguillage`, ex-`RESUME_*`, 10 natives), retours des phases de la démonstration
  (`demo::Etape::AfterPhaseA/B`, ex-`resume!` `0x1f98c`/`0x20a98`, et `ApresMonde` `0x1fa4a` pour sa phase 12), boucle
  de la machine à modes (`modes::Etape`, ex-natives `0x133fe`, `0x1341a`, `0x13422`), entrée de la tâche de démarrage
  (`boot::Etape::Entree`, ex-native `0x11b16`), pas de jeu (`partie::Etape::Pas`, ex-`resume!` `0x1c4b0`).
* **Céder au milieu d'une boucle Rust** sans abandon par panique : une boucle qui appelle un gestionnaire puis
  continue ouvre un *appel attendu* (`table::appeler_attendu`, `crate::appeler_attendu`) dont le `rts` lui rend la
  main ; si le gestionnaire cède (porte : `fondu_de_sortie::fondu_au_noir` ne diverge plus ; feu d'artifice), la
  cession est notée (`taches::a_cede`, remise à faux à chaque point de premier niveau), les `rts` suivants ne font
  rien, et chaque niveau rend `Flow::Yield` ; la tâche reprendra par les cadres. L'épilogue étranger des bulles
  dépile quatre cadres (`table::depiler_appels`, A1-A4 = leurs adresses d'origine) et revient au cinquième ; observé
  aussi dans la démonstration de l'attract (`modes/attract`, images 16 857 et suivantes), d'où l'étape `ApresMonde`
  de sa phase 12.
* **Disparaissent** : `Unwound` (et le `catch_unwind` de chaque native), `call_at` (`Cx::call_at` ; `Cx::tail` est un
  `call`), `resume!`, `Reprise::Cession`, `table::mettre_sur_la_pile`, `relever`, `etape_depilee`,
  `reprendre_de_la_pile`, `vers_le_68000` et la liste `remis`. `Cadre` reste, comme pile des suites d'une tâche (sa
  hauteur de pile 68000 ne sert plus qu'à distinguer un appel par adresse, qui empile une sentinelle) ; il gagne
  l'attente (`attendu`). `crate::call` n'admet plus de cession ni de retour plus haut : une panique nomme le site
  (aucun cas dans les 272 scénarios).
* **Le noyau ne pose plus de PC.** L'ordonnanceur (`0x928`-`0x984`) et le démarrage (`0x350`-`0x50a`) sont une machine
  à états (`kernel::Point`, rangé dans la table des tâches) que `sans68k::ligne` fait avancer d'un point à la fois ;
  l'interruption de VBLANK est une fonction Rust prise entre deux points (`vblank::interruption`). Une tâche est
  lancée en appelant le pas de son étape ; toutes les reprises et entrées sont des étapes (l'écran mort `0x19068`
  n'en a pas). Le plafond des pas est compté par le jeu (`sans68k::pas_de_jeu`, `Point::PasRetenu`) au lieu du test
  du PC (`cps2::StepCap` reste pour la machine de référence). Natives retirées : les cinq de l'ordonnanceur, les dix
  des handlers de traps et de leur retour (`trap #12` change directement le masque), le VBLANK, les cinq du démarrage.
* **Mode hybride retiré** (D1 jusqu'au bout) : avec des tâches lancées par un ordonnanceur Rust et des appels en cours
  hors de la pile 68000, le code d'origine ne peut plus prendre la suite d'une native. `install` = jeu sans 68000,
  `DDSOM_HYBRIDE=1` arrête les outils avec un message (`refuser_hybride`). La machine de référence (`install_reference`)
  garde son 68000 et n'a pas changé ; `DDSOM_CHECK` et `DDSOM_ADOPT_REGS` (original sur une copie) marchent toujours.
  `examples/cessions` et `reste68k` (68000 restant du mode hybride) n'ont plus d'objet.
* **J1 dans le périmètre** : la mise à jour du monde et la fin de phase appellent directement `player::update::update_all`,
  `player::overlays::update`, `combat::combat_update`, `rank::tick_68k`, `aides::game_over` ; la fin de la phase 12 est
  commune au jeu et à la démonstration (`game::apres_le_monde`). Restent par adresse (corps privés d'autres modules) :
  jauge `0xb3e`, sprites `0x1de96`, caméras `0x33e20`, pop-ups `0x19e20`, script `0xf2e5c`, entrées `0x2aec`, et les
  quatre services du VBLANK (`0x80c`, `0x68c`, `0x143da`, `0x2dcf6`).

**Sortie du `Cpu` (étape 3) : pas atteinte, ce qui bloque.** `Native::run` garde `(c, b)` et les contextes leur `c` :
(1) le **PC** sert encore aux sauts d'une native à une autre au sein d'une tâche (`Sequence::jump`/`jump_to`/`tail`,
~25 sites de `modes/`, `scene/`, `taches/` ; `taches::revenir` vers une `Suite::Adresse`, retour d'un `crate::call` ;
`ai_aids::landing_by_kind`, `jmp (A1)`) et aux appels par adresse (`crate::call`, 929 `adr`, dont ~430 dans
`families`) ; (2) les **registres** : 4 269 accès (`regs`) dont 2 195 dans `families`, 349 `modes`, 282 `scene`, 249
`ai_aids`, 186 `player`, 172 `ai` ; les registres résiduels relus d'une routine à l'autre (bilans des lots C, B, S, A :
boucle de dispatch A0/D7/D0/A1, `reinsert`, `random_68k`, `terrain_68k`, créateurs, boss 1 et 9...) et ceux que le
noyau transmet aux tâches neuves et que les cessions rangent dans le contexte ; (3) la **pile 68000** des appels par
adresse (sentinelle) et de l'interruption (FF0E66 et piles des tâches, ignorées par `validate`). Une structure
`Registres` portée par les contextes n'a de sens qu'une fois (1) fait : elle remplacerait le banc `Cpu` à l'identique.

Mesure (`tools/mesure_refacto.py`, avant b30905d / après) : `pile` total 117 / 81 (`taches` 22 / 14, `kernel` 17 / 3,
`sans68k` 10 / 3, `families` 18 / 6) ; natives déclarées 330 / 317 (sans compter les 46 + 10 natives de reprise
générées) ; `regs` `kernel` 47 / 33, `sans68k` 9 / 3 ; `adr` total 943 / 929 (`game` 15 / 14 : les appels directs
remplacent `call` mais le compte inclut les commentaires d'adresse) ; lignes 146 821 / 146 702. Écritures de `c.pc` :
`kernel` 11 / 0, `sans68k` 2 / 0, `demarrage` 5 / 0, `vblank` 1 / 0, `families/mod.rs` 3 / 2 (repli hors jeu de
`jsr_origine`), `taches` 2 / 2 (`revenir` vers une adresse ; `executer` pose l'adresse d'origine de l'étape :
un pas qui rendrait la main sans céder ni sauter s'arrêterait sur « pas de native »).

## 15. Lot IA : aides d'IA et derniers services

Périmètre : `ai.rs`, `ai_aids.rs`, `camera_modes.rs` (secousses), `scroll.rs`, `popups.rs`, créateurs privés (SET
0xd5f30, 0xd6952, 0xd5ef8 ; 0x57c66, 0x5cdc6), initialisation des pools (0x2062, 0x2134, 0x2150, 0x21bc), appels
des événements (0x19ae6, 0x19b08, 0x19dce) et des palettes (0x1686, 0x3af8), plus leurs appelants dans les familles,
`player/`, `scene/`, `modes/`, `combat.rs`.

* **Cœurs.** Chaque aide d'IA est une fonction Rust sur l'objet (`ai::pick_target(b, o, Rule) -> Option<Obj>`,
  `target_by`, `random_player`, `initial_hp`, `set_immunities -> u16`, `hp_rank`, `decision_a/b -> bool`,
  `pick_byte`, `target_in_front(b, o, Boite) -> DansBoite`, `facing_target`, `target_faces_me`, `back_to_target`,
  `turn_decision -> Turn`, `moving_forward`, `keep_on_screen/keep_in_zone -> bool`, `turn_toward`, `drift_heading` ;
  `ai_aids::` points de marche, prises, tirages par tables, parades (`Parry`), débris, butins, éclair et secousse,
  `landing_routine`). La machine à registres `ai::Mach` (opérations à drapeaux exacts) et les ~60 natives des deux
  modules sont supprimées : plus aucun appel par adresse ne les vise (code-mort.md, lot IA). Autres cœurs :
  `creators::set_14`, `set_15_b`, `in_right_half`, `chance_10_16`, `set_residu::attach_life_bar`,
  `popups::effacer_tout`, `popups::animer`, `effects_creators::impact_spark`, `terrain::push_collides`,
  `camera_modes::shake::of_routine`, `camera_modes::update_camera_1/2` et `scroll::update_cameras` (appelables
  directement, encore sur le processeur : les caméras appellent des natives vidéo par adresse).
* **Appelants.** La façade `enemy_lib` n'appelle plus que des cœurs ; ses doublons Rust du lot A (`outside`,
  `facing_target`...) sont fondus avec eux. Familles (`eshl*`, `items*`, `effects*`, `petites`, `joueur/`), joueurs,
  scènes, modes, combat : secousses par `start_shake`, zone et écran par `ai::keep_*`, animations de l'IA par
  `enemy_lib::start/step`, événements par `evenements::enfiler_*`/`creer_popup`, palettes par `palette::full_brightness`
  et `placement::load_effect_palette`, pools par `Pool::reset`.
* **Preuve (J2).** Les appels par adresse ont d'abord été sondés (`ai::sonde`, diagnostic retiré depuis) : après
  chaque native, les registres qu'elle avait changés et ceux que l'appelant avait posés pour elle étaient
  empoisonnés (`DDSOM_POISON=ia_<adresse>`, un registre à la fois : `ia_<adresse>.d2`), validations complètes puis
  bissections par étiquette et par site. Lecteurs trouvés, écrits en clair (glitches d'origine) : index de réaction
  des boss 7, 8, 19 et 3 (octet haut de D0 laissé par 0x10112, 0xb648, 0xfe5a), `+0x8e` de la harpie (D0 de 0xfe5a),
  D5 de la zone et du décor relu par le squelette (`zone_and_terrain -> bool`), côté de la boîte relu dans D0
  (squelette, golem), D2 de 0xb5e0 relu par le boss 11 (`target_in_box_68k`, seul adaptateur gardé), D2/D3 de
  `drop_rewards` donnant leurs vitesses à l'étincelle de 0x103ba (`flash_and_shake`), A1 des cibles (boss 5, 11,
  harpie : les cœurs rendent la cible), D2 de 0x43ae (boss 10), D0/A1 de la routine d'atterrissage (0x3e480).
* **Fuzz.** `examples/fuzz_ia.rs` compare chaque cœur à la routine d'origine (interpréteur seul) sur des états tirés
  d'une partie réelle : 0 écart sur les 70 entrées (skeleton, kobold, hellhound, boss 11, boss 19). Il a corrigé
  deux routines du lot A (0xffda, 0x10298) et mon propre `in_right_half` (sens de la soustraction).
* **Restent.** `scroll.rs`/`camera_modes.rs` appellent par adresse les natives vidéo (0x26d4, 0x2766, 0xbc6, 0x3525c)
  et 0xa8ae8 ; les adaptateurs `_68k` des autres lots (`random_68k`, `terrain_68k`, `push_68k`, `shock_68k`,
  `in_arena_68k`, `reinsert_68k`) n'ont pas été réexaminés.

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
| Lot C : fondu des portes en sous-programme de tâche, J1 dans le lot, registres morts des gestionnaires sans son | e3a49b7 (wt-decors) | complète, 269/269 sans 68000 (30 min 49 s, 8 tâches) et 269/269 depuis les assets (`DDSOM_ASSETS`) ; `fuzz_porte` (portes 7 et 29), `fuzz boutique`, `fuzz petites` sans écart |
| Lot S : services communs sans registres (sauvegarde fantôme du son, cœurs son/palette/texte/terrain/aides/événements, appelants hors lots A-B) | voir `git log` (wt-services) | complète, 269/269 sans 68000 et 269/269 depuis les assets (`DDSOM_ASSETS`), JOBS=8 ; `fuzz son`, `fuzz video`, `fuzz u1`, `fuzz_monde ia_` |
| Lot B : créateurs en fonctions Rust, appels directs, replis, registres prouvés morts, étape du feu d'artifice | voir `git log` (wt-effets) | complète, 270/270 sans 68000 et 270/270 depuis les assets ; `DDSOM_POISON=effets` et `restes` 270/270 |
| Lot A : ennemis et boss (façade `enemy_lib`, noms, variables typées, natives du lot en Rust, routines sans native, registres résiduels prouvés par poison) | voir `git log` (wt-boss) | complète après fusion de main (lots S, B, 9b27479), 272/272 sans 68000 et 272/272 depuis les assets (`DDSOM_ASSETS`, assets réextraits : `zones_natives.txt` de main), JOBS=8 ; poison par site et bissections (boss/, enemy/, cov) |
| Lot « noyau final » : J3 achevé (appels en cours en étapes, appel attendu, cession notée au lieu de `Unwound`), ordonnanceur, démarrage et VBLANK en machine à états Rust, mode hybride retiré, appels directs de la fin de phase | voir `git log` (wt-noyau) | complète après fusion de main (f8f67af), 272/272 sans 68000 (14 min 55 s, 8 tâches) et 272/272 (10 min 53 s) depuis les assets (`DDSOM_ASSETS`, `data` réextrait par l'`extract` de main : `reste.json`) |
| Lot IA : cœurs des aides d'IA et services restants, natives retirées, glitches de registres en clair, fuzz_ia | voir `git log` (wt-ia) | complète, 272/272 sans 68000 et 272/272 depuis les assets (`noyau_assets`), JOBS=8 ; poison par adresse et par registre (sondes), `fuzz_ia` 0 écart |
