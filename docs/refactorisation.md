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

## Avancement

| Étape | Commit | Validation |
|---|---|---|
| Socle : mesure, bench, rendu facultatif, `ram.rs`, `diag.rs`, adaptateurs `_68k` des pools | 688b54c | ciblée |
| Pilote, appelants par les adaptateurs ; documents | e1f8159 | complète, 269/269 normal et sans 68000 |
| Pilote, poison, glitch de l'orbe, gestionnaires en fonctions pures | voir `git log` | complète, 269/269 normal (15 min 40 s) et sans 68000 (15 min 19 s), en parallèle à 7 tâches chacune |
| Lot 0a : plus de durées du mode hybride, jeu sans 68000 par défaut (`DDSOM_HYBRIDE=1` : diagnostic) | edef15d, documentation | complète, 269/269 sans 68000 (14 min 46 s, 10 tâches) et 269/269 depuis les assets (`DDSOM_ASSETS`) ; hybride : bot30k 5 000 images VALIDE (non exigé) |
