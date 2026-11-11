# Plan de fin : du port fidèle au moteur lisible

État de départ : main 9a69409 (2026-11-02). Le jeu tourne en Rust pur : `ddsom` ne lie ni `m68k` ni `z80`. 308
scénarios sur 308 sont VALID contre l'original, en mode par défaut et avec `DDSOM_ASSETS`. Tout le code est en anglais.

Le moteur reste pourtant organisé comme la ROM :

* `ddsom --assets` reconstruit en mémoire une image de la ROM (`ddsom_assets::rebuild::contents`) : programme,
  graphismes, échantillons, région du Z80 et DSP.
* Le code lit ses données par adresse dans cette image.
* L'état vit dans la RAM émulée 0xFF0000-0xFFFFFF, à des décalages de champs.
* La vidéo passe par les tampons CPS2. Le son passe par la région d'adresses du Z80.

Ce document mesure cet écart, fixe l'architecture d'arrivée et découpe le travail en lots validables. Il remplace la
liste « Suite » de [reprise.md](reprise.md) (point 5).

Objectif de l'utilisateur, rappelé :

* un moteur Rust beau, minimal, lisible et modulaire, sans ROM ;
* des assets lisibles : PNG pour les graphismes et les palettes, WAV pour les échantillons, JSON pour les données et
  la musique séquencée ;
* un comportement **strictement identique**, avec la validation image par image contre l'original gardée comme filet ;
* aucun asset dérivé des ROMs dans git.

## 1. Mesures (main 9a69409)

### Méthode

Les chiffres viennent de scripts Python lancés sur `port/ddsom-game/src` :

* les commentaires `//` sont retirés avant le comptage ;
* le dossier `tools/` est exclu, sauf mention contraire ;
* les comptes sont faits par expressions régulières, donc approchés à quelques pour cent près.

Un autre agent nettoie les lints et le module `tools` en parallèle : les chiffres peuvent bouger un peu. Le lot L0
versionne le script (`tools/measure_end.py`) pour que chaque lot donne ses chiffres avant et après.

### Taille des crates

| Crate | Lignes (src) | Exemples, tests | Rôle aujourd'hui |
|---|---:|---:|---|
| `ddsom-game` | 174 701 | 16 611 (58 exemples) | le jeu (natives Rust), plus `tools/` (7 018 lignes, 70 fichiers, fonctionnalité `tools`) |
| `ddsom-assets` | 2 604 | 1 168 | chargement des assets et **reconstruction de la ROM** (`rebuild.rs`, 599 lignes) |
| `extract` | 3 003 | — | ROMs → assets |
| `ddsom-sound` | 2 194 | 1 305 | pilote son Rust (séquenceur), données lues par accesseurs nommés sur la région du Z80 |
| `m68k` | 2 342 | — | 68000, pour la référence et les outils seulement |
| `cps2` | 1 480 | 186 | machine CPS2 (bus, déchiffrement, EEPROM, liaison son) ; `m68k`/`z80` optionnels |
| `z80` | 1 133 | — | référence et outils seulement |
| `ddsom` | 789 | — | application (minifb, clavier, manette, audio, sauvegarde) |
| `qsound` | 563 | — | DSP QSound haut niveau (d'après MAME) |
| `recomp` | 460 | — | recompilateur, outils seulement |
| `cps2-gfx` | 398 | 60 | rendu CPS-A/CPS-B (tilemaps, sprites, priorités, palettes) |
| `ddsom-rc` | 13 | 178 | code 68000 recompilé (généré à la compilation depuis les ROMs locales) |
| `vrender` | 71 | — | rendu d'instantanés |

### `ddsom-game` par zone

Les zones ci-dessous correspondent aux périmètres des lots de la section 3.

| Zone (périmètre) | Fichiers | Lignes | Lectures ROM par accesseur¹ | Champs à décalage littéral² | A5 + littéral³ | `rd*`/`wr*` bruts | Appels de son par adresse⁴ | Constantes `u32` dans 0x400-0x3fffff⁵ | Plus gros fichier |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Effets (`families/effects*.rs`) | 8 | 13 267 | 292 | 525 | 135 | 322 | 58 | 358 | `effects_c.rs` (5 162) |
| Effets après L5 (`families/effects/`, `effects_creators/`) | 33 | 13 908 | 0 | 0 | 0 | 311 | 0 | 0 | `effects/d.rs` (881) |
| SET (`families/set*.rs`, `scenery.rs`) | 10 | 9 911 | 247 | 1 108 | 182 | 346 | 22 | 184 | `set_b.rs` (3 628) |
| SET après L6 (`families/set*`, `scenery.rs`) | 52 | 11 104 | 0 | 0 | 0 | 287 | 0 | 196 | `set/name_entry/cursor.rs` (623) |
| ESHL, SOBJ, objets, caisses | 10 | 15 808 | 274 | 2 378 | 71 | 51 | 91 | 424 | `eshl_c.rs` (3 300), `eshl_b.rs` (3 054) |
| ESHL, SOBJ, objets, caisses après L7 (`eshl*/`, `sobj/`, `items/`, `boxes/`, `shell/`) | 113 | 16 543 | 0 | 0 | 0 | 39 | 0 | 0 | `boxes/chest.rs` (649) |
| Ennemis (`families/enemy*`, `reactions.rs`) | 94 | 32 564 | 170 | 1 302 | 18 | 25 | 1 | 205 | `reactions.rs` (2 223) |
| Ennemis après L8a (façades `enemy_common/`, `enemy_lib`, `enemy`, `reactions/` : 12 fichiers) | 102 | 32 859 | 135 (façades 0) | 1 132 (façades 0) | 14 (façades 0) | 8 | 0 | 180 (façades 0) | `enemy_riyakaa.rs` (1 051) |
| Ennemis après L8b (`enemy_*/`, `enemy_*.rs`, façades) | 107 | 33 188 | 0 | 0 | 0 | 8 | 0 | 0 | `enemy_beetle.rs` (793) |
| Boss (`families/boss*`) | 83 | 30 835 | 207 | 3 182 | 171 | 146 | 0 | 114 | `boss_11/` (2 419 en 5 fichiers) |
| Boss après L9a (boss 0, 1, 3 à 9 et `boss_common/` : 42 fichiers, à 0 ; reste L9b) | 86 | 30 840 | 107 (L9a 0) | 1 899 (L9a 0) | 107 (L9a 0) | 127 | 0 | 58 (L9a 0) | `boss_5.rs` (775) |
| Boss après L9b (`families/boss*`, toute la zone) | 86 | 30 965 | 0 | 0 | 0 | 9 | 0 | 0 | `boss_5.rs` (776) |
| Joueurs (`player/`, `families/player/`, `combat*`) | 57 | 29 048 | 378 | 2 545 | 67 | 383 | 68 | 494 | `player/hurt.rs` (1 761) |
| Écrans (`modes/`, `hud/`, `service/`, `scene/`, `credits*`, `text`, `popups`) | 41 | 18 134 | 62 | 15 | 137 | 1 637 | 74 | 437 | `scene/exit.rs` (1 535) |
| Communs des familles (`cx.rs`, `mod.rs`, `small.rs`, `creators.rs`) | 4 | 1 413 | 11 | 164 | 20 | 61 | 1 | 7 | `creators.rs` (520) |
| Moteur (`objects/`, `tasks/`, `ai*`, `terrain`, `scroll`, `camera_modes`, `palette`, `video`, `sprites`, `sound*`, `dispatch`, `kernel`...) | 48 | 16 703 | 58 | 30 | 56 | 1 232 | 1 | 175 | `camera_modes.rs` (1 605) |
| **Total (hors `tools/`)** | 355 | ~167 700 | **1 699** | **~11 250** | **~785** | **~4 200** | **317** | **~2 400** | |

Notes du tableau :

1. Appels de `data8/16/32`, `rom8/16/32`, `rom_i16` et `prog8/16/32`. Leurs arguments se répartissent ainsi :
   * 812 pointeurs ou expressions (souvent une adresse ROM lue dans un champ de l'objet) ;
   * 704 constantes plus un index ;
   * 167 littéraux plus un index ;
   * 16 constantes ou littéraux seuls.

   S'y ajoutent les lectures `rd16(b, TABLE + i)` vers la ROM, que l'expression régulière ne distingue pas de la RAM.
   La mesure exacte reste `rom_reads` (fonctionnalité `rom-trace`, [sans-rom.md](sans-rom.md)).
2. Appels `x.u8(0x5a)`, `o.set_u16(b, 0x1c, …)`, etc. dont le décalage est un nombre. Il y a 30 170 accès aux champs en
   tout, dont 18 900 par une constante nommée (`Obj::X`) ou une variable.
3. `A5 + 0x…`, `a5_u8(0x…)`, `ram8(…)`. Les variables nommées de `ram.rs` (`Byte`/`Word`/`Long`/`Table`) sont 61
   déclarations et 702 utilisations `ram::X`.
4. `sound::play(b, 0xede08, o)` et ses variantes `play_global` et `play_with` : le son est désigné par l'adresse de
   son enveloppe 68000 (504 enveloppes dans `sound_table.rs`, générée par `tools/gen_sound_wrappers.py`). Les boss
   jouent leurs sons par un autre chemin, `x.sound(…)`.
5. Adresses de tables ROM et adresses de routines d'origine (entrées des natives, étapes, cibles des tables
   d'aiguillage). Sur ~2 400, `families/` en compte 1 565.

**Données brutes encore lues par adresse.**

* `data/natives.json` : 2 963 zones lues sans table décrite. Par lecteur :
  * `families/cx.rs` : 1 069 (accesseurs génériques, donc tous les gestionnaires) ;
  * `player/mod.rs` : 363 ;
  * `scene/mod.rs` : 290 ;
  * `sound.rs` : 193 ;
  * `boss_11/chain.rs` : 184 ;
  * `ai_aids.rs` : 156 ;
  * `families/player/peff_e.rs` : 154 ;
  * `objects/anim.rs` : 126 ;
  * `families/mod.rs` : 121.
* `data/reste.json` : environ 540 000 octets en 7 400 blocs (espace données) et 528 000 octets en 5 450 blocs (espace
  programme). C'est tout le non-code de la ROM, gardé pour que la couverture soit complète par construction.

**Accès aux zones matérielles par adresse littérale.**

* GFX RAM (0x90xxxx) : 407 littéraux dans 61 fichiers (couches de texte, HUD, écrans).
* Registres CPS-A/CPS-B (0x80xxxx) : 72 littéraux dans 17 fichiers.
* Sortie objets (0x40000x) : 21.
* RAM partagée du son (0x618xxx) : 20.

`Cps2Bus` est nommé dans 1 495 lignes, dont 366 dans `tools/`. Toutes les fonctions du jeu prennent `&mut Cps2Bus`.

**Doublons repérés.**

* `random` : 5 copies (`reactions.rs`, `effects_d.rs`, `eshl_c.rs`, `eshl_d.rs`, `enemy_common.rs`, en plus de
  `rng.rs` et `machine.rs`).
* `rom32` : 4 copies (`pshell.rs`, `sobj.rs`, `enemy_common.rs`, `eshl_b.rs`).
* `set8`/`set16` locaux : 4 copies (`camera_modes.rs`, `scene/transition/mod.rs`, `credits_task.rs`, `service/mod.rs`).
* `alloc` : 4 versions (`objects/pools.rs`, `objects/current.rs`, `enemy_common.rs`, `ai_aids.rs`).
* `camera_modes.rs` réimplémente les accesseurs de `Cx` et de `Machine` pour son propre contexte.

### Dépendances entre modules

On ne compte que les arêtes `crate::x` hors `tools/`. Les 46 modules de tête et leurs 286 arêtes forment **une seule
composante fortement connexe de 37 modules**. Seuls `mem`, `ops`, `diag`, `text`, `rng`, `scenario` et `sound_table`
restent en dehors.

Arêtes « à l'envers » qui ferment les cycles (une couche basse qui appelle une couche haute) :

| Arête | Site | Remède |
|---|---|---|
| `objects → families` | `objects/current.rs:199`, `impl Current for families::cx::Cx` | déplacer l'`impl` dans `families/cx.rs` |
| `video`, `sprites`, `inputs`, `scroll → tasks` | `tasks::table::table(b).leftovers` (`video.rs:82`, `sprites.rs:207`, `inputs.rs:161`, `scroll.rs:711`) | passer `&mut Leftovers` en argument ; déplacer `Leftovers` de `dispatch` vers le module de l'état du monde |
| `video → modes`, `scene`, `tasks` | `video.rs:166-168` (étape d'un écran logée dans `video`) | déplacer cette étape dans `scene/` |
| `machine → scene` | `machine.rs:54`, `impl Machine for scene::Ctx` | déplacer l'`impl` dans `scene/` |
| `ram`, `sound → objects` | `use crate::objects::Obj` | sortir `Obj` et `Link` dans un module de base (`world::obj`) |
| `eeprom ↔ config` | `eeprom.rs:319` → `config::validate` | inverser : `config` appelle `eeprom` |
| `kernel ↔ tasks` | l'ordonnanceur appelle les étapes (`tasks::execute`), et les tâches cèdent (`kernel::wait`) | légitime, mais à loger dans un même module `core` |
| `ai_aids`, `effects_aids → families` | `effects_creators::impact_spark`, `leftovers::{Pure, Regs}` | descendre ces créateurs et traits dans `objects/` ou `world/` |

### Échafaudage hérité de la ROM

| Élément | Où | Taille | Sort prévu |
|---|---|---|---|
| Image ROM reconstruite | `ddsom_assets::rebuild` (599 lignes) ; `cps2::Contents` ; `Cps2Bus::rom` et `rom_dec` | 4 Mo par espace, plus 0x1800000 octets de graphismes, plus 4 Mo d'échantillons | lot L14 : le jeu charge `GameData` ; `rebuild` ne sert plus qu'aux tests sans perte d'`extract` |
| Accès aux données par adresse | `data*`/`rom*`/`prog*`, `rd*` vers la ROM | 1 699 appels, plus les lectures `rd*` | lots L4 à L11 (données typées) |
| `reste.json` et `natives.json` | assets | 2 963 zones, environ 1 Mo de blocs | décrits table par table ; `reste.json` devient un filet de dernier recours (lot L14) |
| RAM émulée sur le bus | `Cps2Bus::ram`, `mem::rd*`/`wr*` avec décodage d'adresse à chaque accès | ~4 200 accès bruts, ~30 000 accès aux champs | vues typées (lots L4 à L11), puis RAM tenue par le jeu (L15) |
| Natives et registre d'adresses | `lib.rs` (`Native`, `lookup`, crible et table de hachage), `dispatch.rs` (10 tables d'aiguillage lues en ROM par `prog32`, puis `lookup`), `kernel.rs:213` (`tasks::entry(entry)`) | 481 natives côté jeu (`G` 434 + `M` 47), 992 côté outils (`P` + `E`) | lot L13 : tableaux de fonctions par classe et `enum` d'entrées de tâche ; le registre par adresse ne sert plus qu'aux outils |
| `ram.rs`, `mem.rs` | variables `Byte`/`Word`/`Long` à adresse ; `A5 = 0xff8000` | 149 et 114 lignes | absorbés par les vues typées (L1 pour le socle) |
| Sons par adresse d'enveloppe | `sound.rs`, `sound_table.rs` | 317 sites, 504 enveloppes | lot L2 (sons nommés) |
| `no68k` | nom du module de boucle d'image | 319 lignes | renommé `frame` (L15) |
| Région du Z80 | `ddsom_sound::Rom` (accesseurs nommés sur une région de 0x50000 octets) ; l'état du pilote garde des adresses Z80 | 299 lignes | lot L3 (tableaux indexés) |
| Programme du DSP | `qsound` lit `qsound_dsp.json` placé aux adresses du DSP | 3 tables | lot L4 (tables nommées) |

### Performance (base de mesure)

Dernière mesure : lot F, `examples/bench` sur bot30k, 12 000 images, pilote son Rust.

* 1 862 images/s avec le rendu, et 11 408 images/s sans.
* Avec le rendu, les couches prennent 35 % du temps (`pixels8`, lecture des tuiles) et les sprites 7 %.
* Sans le rendu, QSound prend environ 45 % (110 ns par échantillon). Le jeu lui-même prend environ 25 µs par image.
* Le bus (`read_word`/`write_word`, décodage d'adresse) et `lookup` pèsent chacun moins de 5 %.
* Après L4 (atlas de tuiles, QSound vectorisé) : environ 15 500 images/s sans rendu et 3 300 avec ; le rendu seul
  prend ~230 µs par image. Détail dans le « Résultat » de L4.

## 2. Architecture d'arrivée

### Principes

1. **Le filet reste.** `validate` compare chaque image à la machine de référence : RAM de travail, GFX RAM, RAM
   d'objets, palette, RAM partagée et écritures du pilote son. On ne renonce à aucune de ces comparaisons. Toute forme
   d'état et de données doit donc pouvoir se **projeter exactement** sur ce que l'original a en mémoire.
2. **Les adresses deviennent des identifiants, pas des emplacements.** L'original range des adresses ROM dans la RAM :
   * le script d'animation suivant (`Obj::ANIM_NEXT`) ;
   * la table de vitesses (`Obj::SPEED_TABLE`) ;
   * les pointeurs de script de scène ;
   * les entrées des tâches.

   Ces valeurs sont comparées par `validate` : elles restent dans l'état, mais comme **clés** (`RomAddr(u32)`) de
   tables typées. Elles ne sont plus lues comme des octets d'une image. Les JSON gardent déjà les adresses en clé
   (`"0x2b45d8"`, [assets.md](assets.md) « Conventions »).
3. **Un seul chemin de code dans les deux modes.** Le mode par défaut (avec ROMs) et le mode assets chargent la même
   structure `GameData` :
   * depuis les ROMs, par `extract` en mémoire ;
   * depuis les fichiers, par `ddsom-data`.

   La validation dans les deux modes compare alors bien les données. Elle ne compare plus deux chemins de lecture
   différents.

### Crates d'arrivée

```
port/
  ddsom/          application (façade macroquad, L21) : fenêtre et filtres (view.rs), clavier/manette (keys.rs,
                  gamepad.rs), sortie audio (audio.rs), sauvegardes et EEPROM (save.rs)
  ddsom-hd/       pack HD (L22) : export, retour aux index, chargement, rendu HD ; outil hd-pack
  ddsom-game/     le moteur du jeu (ci-dessous), sans 68000, sans ROM, sans Cps2Bus
  ddsom-data/     données typées : structures, chargement depuis assets/ (et depuis les ROMs pour extract et les tests)
  ddsom-sound/    pilote son (séquenceur), données en tableaux indexés
  cps2-gfx/       moteur de rendu de sprites et tilemaps au format CPS2, atlas de tuiles décodé
  qsound/         mixeur QSound (échantillons depuis une banque chargée des WAV)
  extract/        ROMs → assets (et ROMs → GameData en mémoire)
  -- référence et outils (jamais liés par ddsom) --
  cps2/  m68k/  z80/  recomp/  ddsom-rc/  vrender/
  ddsom-tools/    (lot L17, facultatif) validate, fuzz_*, couverture : aujourd'hui ddsom_game::tools + exemples
```

`ddsom-assets` se scinde en deux :

* le chargement passe dans `ddsom-data` ;
* `rebuild` (réencodage au format ROM) ne sert plus qu'aux tests sans perte et à `programme_complet`. Il passe dans
  `extract` (`extract/src/rebuild.rs`) ou dans les tests.

### Modules d'arrivée de `ddsom-game`

L'arrivée est un graphe sans cycle, en couches. Chaque couche n'appelle que celles du dessous.

| Couche | Modules (état après L15c, `port/ddsom-game/src/<couche>/`) | Rôle |
|---|---|---|
| `data` (crate `ddsom-data`) | — | tables immuables : sprites, animations, décors, palettes, terrain, textes, tables de jeu, scripts |
| `hw` | `mem`, `video`, `io` | interface avec le matériel modélisé ; aucune logique de jeu |
| `world` | `World`, `ram`, `record`, `objects/` (pools, listes, animation, mouvement, rang, visibilité, fiche `Player`), `tasks/` (table des tâches, traps du noyau, étapes `stages`, API de cession), `data`, `tables`, `screen_tables`, `terrain`, `rng`, `placement`, `events`, `machine`, `ops`, `diag` | état du monde, moteur d'objets, tâches comme données |
| `services` | `palette`, `fading`, `text`, `sprites`, `scroll`, `video`, `popups`, `sound`, `inputs`, `eeprom`, `config` | services de jeu appelés par les acteurs et les écrans |
| `actors` | `families/` (ennemis, boss, effets, SET, ESHL...), `player/`, `ai`, `ai_aids`, `aids`, `effects_aids`, `combat`, `camera_modes`, `dispatch`, `update` (mise à jour du monde) | gestionnaires d'objets, joueurs, caméras |
| `screens` | `modes/`, `scene/`, `hud/`, `service/`, `credits*`, `prompt`, `fixed_screen`, `block`, `obj`, `vars` | écrans et séquences |
| `core` | `kernel` (ordonnanceur), `tasks/` (exécution des étapes, tâches `game` et `freeze`), `vblank`, `startup`, `frame` (ex-`no68k`), `game` (phases 0 et 12), `scenario` | ordonnanceur, boucle d'image |

### État du jeu : la RAM reste l'état, d'abord typée, puis détachée du bus

Faut-il garder l'état dans une RAM émulée tant que la validation compare la RAM ? **Oui.** C'est même la forme
d'arrivée pour l'essentiel de l'état. Trois raisons, mesurées :

* L'original exploite la disposition de la RAM :
  * liens de listes sur 16 bits (`Link`, adresse étendue par `movea.w`) ;
  * objets lus au-delà de leur pool ;
  * champs relus par un autre gestionnaire (les restes) ;
  * pointeurs ROM rangés dans les champs ;
  * tableaux indexés hors bornes (`Table::nth`, « the original sometimes indexes beyond »).

  Un état en structures Rust devrait reproduire chacun de ces cas par une projection à chaque image. Ce serait plus
  long et moins sûr que la RAM elle-même.
* La comparaison de la RAM à chaque image est le meilleur filet : elle voit une divergence des centaines d'images
  avant qu'elle ne se voie à l'écran. Une validation « à l'image » seule (pixels plus son) serait plus faible.
* Le précédent existe et montre le coût d'une projection : la table des tâches est un état Rust
  (`tasks::table::Table`, rangée dans `Cps2Bus::ext`). `validate` la compare champ par champ à FF0064 de la référence
  (`gaps`). Il a fallu une sonde par champ et plusieurs lots.

La stratégie a trois étapes.

1. **Vues typées sur la RAM (lots L1, puis L4 à L11).**
   * Chaque enregistrement devient un type avec des champs nommés, déclarés une fois. Exemples : `Enemy(Obj)`,
     `Player(Obj)`, `Boss11(Obj)`, et les variables de `world::ram`.
   * Une macro `record!` produit les accesseurs, par exemple `hp: u16 @ 0x5a`.
   * Plus de décalage littéral hors des déclarations. Cible : environ 11 250 décalages littéraux et 785 `A5 + littéral`
     ramenés à 0.
2. **RAM tenue par le jeu, plus par le bus (lot L15).**
   * `World { ram: Box<[u16; 0x8000]> }` est lu et écrit sans décodage d'adresse.
   * Le matériel (vidéo, son, entrées, EEPROM) passe par `hw`.
   * `Cps2Bus` ne reste qu'à la machine de référence. `validate` compare `world.ram` comme aujourd'hui `bus.ram`.
3. **État Rust plus projection, au cas par cas (lot L16, facultatif).**
   * À ne faire que pour un bloc que personne ne lit hors de son module : condition vérifiée par une trace de lecture
     par adresse (`watch_writes`, étendu aux lectures).
   * Candidats : la file du son (FF2900...), les états propres à un écran (modes, menu TEST) et les compteurs de pièces.
   * Non candidats : les pools d'objets, les fiches de joueurs (0x200 octets lus partout) et la caméra.

   La projection s'écrit comme `gaps` : comparaison dans `validate`, et écriture dans la RAM seulement si un lecteur
   d'origine en a besoin.

### Données : `ddsom-data`, chargées directement

`GameData` est immuable et chargée une fois. Les fonctions du jeu y accèdent par `data()`, un `static OnceLock`.
C'est un choix assumé : les données sont constantes, et les faire passer en argument dans 30 000 sites n'apporte rien.
La machine de référence garde ses ROMs ; elle n'utilise pas `GameData`.

| Famille | Fichier d'asset (existant) | Type | Consommateurs principaux |
|---|---|---|---|
| Images de sprites, listes de morceaux | `sprites/frames.json` | `Frames` indexé par `RomAddr` | `sprites.rs`, `objects/anim.rs` |
| Scripts d'animation | `sprites/anims.json` | `Anim` (entrées 16/24 octets, sauts) | `objects/anim.rs` |
| Tables d'objets (boîtes, marques au sol, visibilité) | `sprites/tables.json` | `Hitbox`, `Mark`... | `combat/`, `objects/cull.rs`, `ai_aids.rs` |
| Décors scroll 1/2/3 | `levels.json` | `Level` (bandes, blocs) | `scroll.rs`, `camera_modes.rs`, `scene/` |
| Palettes | `palettes.png` (RGBA, une palette par ligne), `scene_palettes.json` | `PaletteLib`, `ScenePalettes` | `palette.rs` |
| Terrain | `terrain.json` | `TerrainMap`, `TerrainKind` | `terrain.rs` |
| Textes, dialogues, bulles | `texts.json`, `data/dialogs.json` | `Text`, `Dialog` | `text.rs`, `hud/`, `families/set_c_window.rs` |
| Tables de jeu | `data/players.json`, `combat.json`, `enemies.json`, `boxes.json`, `scenes.json`, `misc.json`, `spawns.json` | `DataFile` → structures nommées par table | familles, joueurs, scènes |
| Zones brutes | `data/natives.json` → nouvelles tables nommées dans `data/*.json` | une structure par table | lots L4 à L11 |
| Reste | `data/reste.json` | `Residue` : lecture de dernier recours, comptée | L14 |

Formats d'assets visés. On garde ceux d'aujourd'hui, qui sont déjà lisibles :

* Graphismes : PNG indexé, cellules 16x16 (`gfx/cells_NN.png`). Sans changement.
* Palettes : PNG RGBA (`palettes.png`). On le garde : chaque couleur CPS2 (4 bits par composante, plus la
  luminosité) s'y relit sans perte, ce que vérifie `palettes_lossless`.
* Échantillons : WAV PCM 16 bits, avec la boucle et la note dans les métadonnées (`sounds/NNN.wav`). Sans changement.
* Données : JSON. On ne passe pas à RON : serde_json est déjà là, les tests sans perte existent, et un format de plus
  n'apporte rien.
* Musique séquencée : `sound/music.json`, réorganisé en tableaux indexés au lot L3.

Pour chaque table nouvelle, le lot qui la décrit fournit trois choses :

* son extraction (`extract/src/<famille>.rs`) ;
* son type et son chargement (`ddsom-data/src/<famille>.rs`) ;
* son test sans perte (`<famille>_lossless`).

**Lectures hors bornes.** Une table typée a des bornes. L'original, lui, lit parfois au-delà :

* la table des boutiques avec un numéro hors bornes ([sans-rom.md](sans-rom.md)) ;
* les durées du pilote son ;
* les 32 palettes copiées depuis l'adresse 0.

Chacun de ces cas devient un cas nommé dans le type (`Table::get_or_overread`), avec les octets qui suivent extraits.
Un accès hors bornes non prévu passe par `Residue`, qui compte et journalise l'accès (panique avec `DDSOM_STRICT=1`).
Ainsi, un chemin jamais joué ne casse pas le jeu de l'utilisateur, et on le voit.

### Rendu : le modèle CPS2 est gardé comme moteur de rendu

Garder `cps2-gfx` est légitime. C'est un moteur de rendu de sprites et de tilemaps à priorités, de 340 lignes, exact
au pixel près, et `validate` compare justement ses entrées. Ce qui change :

* **L'interface du jeu vers la vidéo** (`hw::video`) devient typée :
  * `Sprites::push(x, y, code, attr)` remplace les écritures dans la RAM d'objets à 0x70xxxx et 0x40000x ;
  * `Tilemap::{scroll1, scroll2, scroll3}.set(col, row, code, attr)` remplace les 407 littéraux 0x90xxxx ;
  * `Registers::{scroll_x, scroll_y, layer_enable, priority}` remplace les 72 littéraux 0x80xxxx ;
  * `Palette::set(page, index, color)`.

  Les tampons sous-jacents gardent le format CPS2, puisque c'est ce que `validate` compare.
* **L'interface du rendu** est `cps2_gfx::render(&DrawList, &TileAtlas)` depuis L20 (le jeu donne `Game::frame()
  -> DrawList`, voir « L20 ») ; avant, `cps2_gfx::render(&VideoState, &TileAtlas) -> Frame`. Le `TileAtlas` est fait des
  cellules 8 bits décodées depuis les PNG. On cesse de réencoder en plans 4 bits (`rebuild::gfx_region`) pour décoder
  aussitôt à chaque pixel (`pixels8`). C'est plus simple, et c'est le gain de rendu attendu (couches : 35 % du temps).

### Audio

* **Pilote** (`ddsom-sound`) : il garde son rôle, en lecture de tableaux indexés (lot L3).
  * Son état ne garde plus d'adresses Z80 : un pointeur de piste devient `(piste, position)`, un banc d'instruments
    devient un index.
  * Les débordements utiles restent écrits comme cas nommés : une durée pointée et en triolet lit les décalages
    d'octave qui suivent.
  * `validate` compare les écritures du pilote, les registres QSound et la RAM partagée. Les adresses d'échantillons
    écrites dans les registres QSound restent donc les adresses d'origine.
* **Banque d'échantillons** : une mémoire d'échantillons construite au chargement depuis les WAV, aux adresses des
  fiches. Ce n'est pas une image ROM : c'est l'espace d'adresses que les registres QSound désignent.
* **Mixeur** (`qsound`) : ses trois tables (pan, pas de l'ADPCM, filtres) deviennent des tableaux nommés chargés
  depuis `qsound_dsp.json`. Il ne lit plus aux adresses du programme du DSP.
* **Sortie** (L20) : le pilote et le mixeur restent dans le cœur ; `Game::sound()` rend les échantillons stéréo
  mixés de l'image (24 kHz), que la façade joue.

### Entrées et plateforme

`ddsom` (`main.rs`, `view.rs`, `keys.rs`, `gamepad.rs`, `audio.rs`, `save.rs`) reste la seule crate de plateforme
(façade macroquad depuis L21). Elle remplit `hw::inputs` et lit `Frame` et les échantillons. Depuis L20, elle ne parle au jeu que par `ddsom_game::Game`
(`step(Inputs)`, `frame() -> DrawList`, `sound()`, `eeprom()`). Son ouverture devient
`Game::new(GameData::load(assets)?, eeprom)` au lieu de `rebuild::contents` suivi de `Machine::ddsom`.

## 3. Lots

### Règles communes

Celles de [agents/brief-commun.md](agents/brief-commun.md) et de [reprise.md](reprise.md), « Méthode qui marche ».

* **Parallélisme.** Au plus 3 agents. Un worktree par lot (`wt-<lot>`). Les périmètres de fichiers sont disjoints. Un
  seul lot à la fois touche les API communes : `families/cx.rs`, `families/mod.rs`, `objects/current.rs`,
  `world::ram`, `ddsom-data/src/lib.rs`.
* **Validation.** `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1 DDSOM_JOBS=8 setsid -w tools/validate_all.sh 30000`, une
  fois en mode par défaut et une fois avec `DDSOM_ASSETS` sur des assets **réextraits par la branche**. Il faut
  308/308 dans chaque mode. Après chaque fusion de deux lots, on relance la validation complète.
* **Code mort.** Il est consigné dans [code-mort.md](code-mort.md) avant d'être retiré.
* **Mesure.** Chaque lot de familles met à jour sa ligne du tableau de la section 1 (avant/après) avec
  `tools/measure_end.py`.
* **Fichiers partagés par tous les lots de données.** Ce sont `extract/src/main.rs` et `ddsom-data/src/lib.rs`. Chaque
  lot y ajoute **une ligne** : enregistrement de son module. Le conflit à la fusion est alors trivial. Les tables vont
  dans `extract/src/<famille>.rs` et `ddsom-data/src/<famille>.rs`, propres au lot.

### Démarche d'un lot de famille

Un lot de famille suit trois étapes, dans cet ordre. Chaque étape fait l'objet d'un commit validé.

* **(a) Découpage.**
  * On ne fait que déplacer : fichiers géants en modules par type ou par thème.
  * Le comportement ne change pas. On vérifie que `examples/names` donne les mêmes natives et adresses avant et après.
  * Une seule validation en mode par défaut suffit.
* **(b) Données typées.**
  * Les appels `data*`/`rom*`/`prog*` et les `rd*` vers la ROM du périmètre sont remplacés par des accès à `data()`.
  * Les zones de `natives.json` dont le lecteur est dans le périmètre deviennent des tables nommées.
  * Fin : zéro accesseur ROM dans le périmètre, et `rom_reads` (`rom-trace`, 308 scénarios) n'attribue plus aucune
    lecture du bus dans 0-0x3fffff aux fichiers du périmètre.
* **(c) Vues typées.**
  * Les décalages littéraux deviennent des champs d'un `record!` par type. Les noms viennent de
    [familles.md](familles.md), [creatures.md](creatures.md), [objets.md](objets.md) ou [joueur.md](joueur.md).
  * Un sens deviné est marqué `(h)` dans le commentaire, jamais par un nom vide de sens comme `field_5a`.
  * Les doublons du périmètre sont factorisés.
  * Fin : zéro décalage littéral et zéro `A5 + littéral` hors des déclarations.

### Ordre et parallélisme

```
Série 0   L0 ─ L1 ─ L2
Vague 1   L3 (son)  ║  L4 (rendu, mixage, perf)  ║  L5 (effets)
Vague 2   L6 (SET)  ║  L7 (ESHL/SOBJ/objets)     ║  L8 (ennemis)
Vague 3   L9 (boss) ║  L10 (joueurs)             ║  L11 (écrans)
Vague 4   L12 (moteur, puis communs des familles)
Série 5   L13 ─ L14 ─ L15 ─ (L16) ─ (L17) ─ L18
Bonus     L19 (saisie du nom), à tout moment, en parallèle de n'importe quelle vague
```

Les lots L3 et L4 ne touchent pas `ddsom-game` : ils peuvent aussi tourner pendant les vagues 2 et 3 si un agent est
libre.

### Série 0 (un agent, API communes)

**L0. Mesure et point de départ** (petit) — **fait** (branche `wt-serie0`)
* Périmètre : `tools/measure_end.py` (nouveau) et `docs/fin.md`.
* Travail :
  * le script reproduit le tableau de la section 1 par périmètre ;
  * il ajoute le graphe des modules et ses composantes fortement connexes ;
  * il prend un `--json` pour comparer deux révisions.
* Lancer `rom_reads` sur les 308 scénarios et ranger le résultat par lecteur : c'est la mesure de référence des
  étapes (b).
* Fin : le script versionné redonne les chiffres de la section 1 à ±2 %.
* Résultat :
  * `tools/measure_end.py` (zones de la section 1, tailles des crates, graphe des modules et composantes fortement
    connexes, `--graph`, `--json`, `--rev REV`, `--compare A B`). Sur 9a69409, il redonne la section 1 à ±2 % pour
    les fichiers, lignes, accesseurs ROM (1 700), constantes d'adresses (2 398), `rd*`/`wr*` (4 203), fichiers de
    plus de 1 500 lignes (14), graphe (286 arêtes, composante de 37). Écarts connus : décalages littéraux 11 742
    (+4 %, tous dans `players` : 2 912 contre 2 545, définition plus large des accesseurs), `A5 + littéral` 854 (la
    somme des lignes du tableau donne 857 : le total « ~785 » était faux), sons par adresse 300 (appels dont
    l'argument est un littéral ou une constante `u32` du fichier ; 316 appels `sound::play*` en tout).
  * Mesure de référence par lecteur : `rom_reads` avec `DDSOM_READERS=1` (premier appelant de chaque mot lu,
    hors accesseurs génériques) sur les 308 scénarios, rangée par `tools/rom_readers.py report` (5b55ead) :
    914 567 mots lus (1,83 Mo). Par zone : moteur 772 327 (`sprites.rs` 394 571, `objects/anim.rs` 156 070,
    `scroll.rs` 102 399, `palette.rs` 68 765, `video.rs` 38 356), SET 59 695, écrans 37 827, joueurs 25 619,
    ESHL/SOBJ/objets 4 928, effets 4 726, boss 4 649, ennemis 4 495, communs des familles 297.
    `tools/rom_readers.py zones <fichier> <sorties>` donne les zones lues par un fichier : c'est la liste de
    départ de chaque étape (b).

**L1. Socle des données typées et des vues typées** (moyen) — **fait** (branche `wt-serie0`, voir « Résultat »)
* Périmètre :
  * `ddsom-data/` (nouvelle crate : loaders repris de `ddsom-assets/src/{lib,sprites,levels,tables,data,texts,terrain,scene_palettes,spawns}.rs`) ;
  * `extract/src/lib.rs` (`extract::to_data(roms) -> GameData` en mémoire) ;
  * `ddsom-game/src/{lib.rs, objects/anim.rs, objects/motion.rs, sprites.rs, ram.rs, mem.rs, objects/mod.rs, families/cx.rs}` ;
  * `ddsom/src/main.rs`.
* Travail :
  * `GameData`, `RomAddr`, `data()` ;
  * chargement dans les deux modes : ROMs → `extract::to_data`, assets → `GameData::load` ;
  * première famille : `sprites/frames.json` et `anims.json`, la plus lue (380 Ko et 113 Ko d'octets lus) ;
  * vitesses de `motion.rs` (0x1c67ce-0x1ca180, aujourd'hui dans `natives.json`) ;
  * macro `record!` et premier enregistrement `Obj` en champs déclarés.
* Fin :
  * `objects/anim.rs`, `objects/motion.rs` et `sprites.rs` sans accesseur ROM ;
  * `rom_reads` à zéro dans les régions des images et des animations ;
  * 308/308 dans les deux modes ;
  * `bench` : même empreinte d'état final.
* Risques :
  * les pointeurs d'animation lus en RAM doivent tomber sur une entrée connue. Les 35 entrées incohérentes et la
    paire chevauchante `0x174c70`/`0x174c78` ([assets.md](assets.md) « Ce qui reste ») passent par `Residue` ;
  * `extract` dépend de `m68k`, pour la découverte du code. Il ne faut pas lier `extract` à `ddsom` : `to_data` reste
    dans une fonctionnalité utilisée par `validate` et le mode avec ROMs. Sinon, le mode ROM passe par une extraction
    dans un dossier temporaire.
* Résultat :
  * crate `ddsom-data` : `GameData { frames, anims, speeds }`, `GameData::load(assets)`, `RomAddr`, `data()` (un
    `OnceLock` ; `install` puis `data()`), `pointers::Pointers` (tableaux d'adresses du code). Les loaders de
    `ddsom-assets` (sprites, levels, tables, data, texts, terrain, scene_palettes, spawns, json) y sont déplacés ;
    `ddsom-assets` les réexporte et garde `music` (lot L3), `rebuild`, `remainder`, les cellules et les palettes.
    **Pour les lots de données suivants** : une ligne par famille dans `GameData` et `GameData::FILES`, le
    chargement dans `ddsom-data/src/<famille>.rs`, l'extraction dans `extract/src/<famille>.rs` appelée par
    `extract::to_data_from`, l'écriture dans `rebuild` et un test `<famille>_lossless`.
  * `extract` est une bibliothèque (plus le programme) : `to_data(roms)`, `to_data_from(&Program)`. Son 68000 (pour
    `reste.json`) est derrière la fonctionnalité `full` du programme : `ddsom` lie `extract` sans `m68k` ni `z80`
    (`cargo tree -p ddsom -e normal`). `ddsom` installe `GameData::load(assets)` ou `extract::to_data(roms)` ;
    les outils (`install_no_68k`, `install_reference`) extraient les données du programme de la machine si aucune
    n'est installée. Test `extract/tests/to_data.rs` : les données extraites des ROMs égalent celles des assets.
  * Lecteurs : `objects/anim.rs` (entrées, sauts, jeux, jeux par personnage), `objects/motion.rs` (tables et
    tableaux de vitesses) et `sprites.rs` (images) lisent `data()`. Ce que les tables ne décrivent pas passe par le
    **résidu** (`ddsom-game/src/data.rs`, `Missing { table, key }.read16/read32/prog32`) : lu sur le bus, compté
    par clé manquante, fatal avec `DDSOM_STRICT=1`, imprimé par `validate` (ligne `residue:`) et résumé par
    `tools/residue.py`.
  * Nouvelles données : `data/speeds.json` (120 tables jointives `0x1c4836-0x1ca2b6`, deux tableaux de 44 et 76
    tables ; 58 zones retirées de `native_zones.txt`), `character_sets` d'`anims.json` (trois tableaux de 12
    jeux), `extra` de `frames.json` (38 images hors table), et les jeux et entrées que seul le code désigne
    (`port/extract/code_sprites.txt`, mesuré par `tools/residue.py --sprites`).
  * `record!` (`ddsom-game/src/record.rs`) : `getter, setter, OFFSET: type @ décalage;` produit la constante de
    décalage (pour le code qui passe encore des décalages) et les accesseurs typés. Les champs communs d'`Obj` sont
    déclarés ainsi (`obj.anim_timer(b)`, `obj.set_frame(b, v)`, `Link` pour `PREV`/`NEXT`) ; `anim.rs` et `motion.rs`
    les emploient. Sans la crate `paste` (pas de dépendance hors ligne), les trois noms sont écrits.
  * Mesures : `anim.rs`, `motion.rs` et `sprites.rs` n'ont plus d'accesseur ROM (`measure_end.py`, zone `engine` :
    1 232 → 1 218 `rd*`/`wr*`, les lectures du résidu étant regroupées dans `data.rs`). Résidu sur les 308
    scénarios (`tools/residue.py`, identique dans les deux modes) : 16 146 lectures pour 19 clés, toutes des
    lectures d'origine hors des tables (glitchs) : 6 pointeurs d'image invalides (`0x32003200`, `0xb200b30`...),
    9 entrées et 3 jeux de script incohérents autour de `0x1af22c-0x1bf902` (leurs images ne sont pas des images),
    et la table de vitesses 0 d'un objet sans table. Avant la mesure du code (`code_sprites.txt`) : 205 clés.
    Aucune image ni entrée connue n'est plus lue par adresse. `bench` bot30k 12 000 images : même empreinte
    `a65ecd8aa6e38d4f` que main, sans les outils, avec ROMs comme avec assets.
  * Validation : 308/308 VALID en mode par défaut et avec `DDSOM_ASSETS` (assets réextraits par la branche),
    `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1` ; tests `ddsom-assets` (dont `speeds_lossless`), `extract`
    (`to_data`) ; `cargo clippy` sans avertissement (avec et sans `tools`).

**L2. Sons nommés** (petit, mécanique, transverse) — **fait** (branche `wt-serie0`, voir « Résultat »)
* Périmètre :
  * `sound.rs` et `sound_table.rs` ;
  * les 317 sites `sound::play*` (39 fichiers) et les appels de son des boss ;
  * `tools/gen_sound_wrappers.py`.
* Pourquoi en série : ce lot touche tous les périmètres. Fait d'un coup, au début, il évite 9 conflits plus tard.
* Travail :
  * chaque enveloppe devient une constante `Sound` (commande, D2, D3, options : la ligne de `sound_table.rs`), nommée
    d'après la commande et [son.md](son.md) quand le sens est connu (`sound::SWORD_HIT`), sinon d'après la commande
    (`sound::CMD_3A`) ;
  * `play(b, Sound, o)` sans adresse ;
  * l'adresse de l'enveloppe reste un champ de la constante, pour les outils et les natives des outils.
* Fin : `grep -E 'sound::play[a-z_]*\([^,]+, *0x'` ne trouve rien ; 308/308 dans les deux modes.
* Résultat :
  * `sound::Sound` (entrée de l'enveloppe et façon de composer la commande : enveloppe, aiguillage, son par version,
    son muet). Les 504 enveloppes sont des constantes nommées d'après leur commande, générées par
    `tools/gen_sound_wrappers.py` (qui redonne `sound_table.rs` à l'identique) : `CMD_3A`, `CMD_1C_1D` (deux voix),
    `CMD_A0_BY_CHARACTER`, `CMD_B9_BY_PLAYER`, doublons `_B`, `_C`... Les 21 autres sont nommées à la main dans
    `sound.rs` (`SCENE_MUSIC`, `SCENE_MUSIC_OR_STOP`, `WEAPON_VOICE`, `LANDING_EFFECT`, `PRAYER`, `STORM`, `MOON`,
    `SILENT`...). [son.md](son.md) ne nomme pas les sons un à un : les noms de sens restent les alias locaux des
    familles (`SFX_LAND`, `DEATH_EFFECT`...), désormais de type `Sound` ; les alias qui n'étaient qu'une adresse
    (`E7376`, `SOUND_0ECF10`...) sont remplacés par la constante.
  * `play(b, Sound, o)`, `play_global`, `play_with`, `Ctx::sound`, `enemy_lib::sound` prennent un `Sound` ; un son
    désigné par une table de la ROM se lit `Sound::at(entrée)` (12 sites). Plus de recherche par adresse pour un son
    nommé ; les natives des outils gardent l'entrée (`Sound::entry`).
  * Mesure (`measure_end.py`, colonne `sound`) : 300 sons par adresse → 0 ; le `grep` de fin ne trouve rien.
  * Validation : avec L1 (mêmes validations).

### Vague 1 (trois agents)

**L3. Son en tableaux indexés** (moyen)
* Périmètre : `ddsom-sound/`, `extract/src/music.rs`, `ddsom-assets/src/music.rs` (ou `ddsom-data/src/music.rs`),
  `qsound/src/lib.rs`, `cps2/src/sound.rs`.
* Travail :
  * `music.json` réorganisé : sons, pistes (octets du séquenceur), instruments (bancs), fiches d'échantillons, tables
    du pilote (durées, octaves, tons, amplitudes...) ;
  * état du pilote en indices au lieu des adresses Z80 ;
  * débordements écrits en clair ;
  * `Driver::ram_image` garde la projection vers la RAM du Z80 pour les outils ;
  * tables du DSP en tableaux nommés.
* Fin :
  * `ddsom_sound::Rom` n'a plus de `region: Vec<u8>` ;
  * `music_driver_identical`, `sound_compare`, `no_rom` et 308/308 dans les deux modes, avec le son comparé ;
  * l'écart connu du Z80 en retard ne doit pas grandir (10 scénarios en ordre, 2 en valeur).
* Risque : un pointeur de piste qui saute d'une piste à l'autre, par une suite d'octets contiguë entre deux
  définitions, ne se modélise pas en `(piste, position)`. Il faut d'abord mesurer avec `sound_reads`, puis garder une
  « piste » couvrant la plage contiguë si le cas existe.
* **Avancement** (branche `wt-l3-son`, détails dans [son.md](son.md) « Tableaux indexés ») :
  * fait : `ddsom_sound::Rom` remplacé par `SoundData` (sons typés, pistes en octets jointifs, bancs, fiches
    d'échantillons, tables), sérialisé tel quel dans `music.json` (fonctionnalité `serde`) ; état du pilote en
    `TrackPos`, numéro de banc et numéro de son ; `ram_image` retraduit en adresses ; débordements en clair (durées,
    `LevelTables`, fiches d'échantillons au-delà de la table que les données désignent : bancs 1 175-255 et
    l'instrument initial 0xff00 d'une piste, rejoué à la fin d'un effet) ; région du Z80 dans `region.rs` (lecture
    et réécriture) ; `qsound::DspTables` ; `sound_reads` retiré ;
  * risque mesuré : un saut relatif peut changer de piste (`SoundData::jump`), aucune lecture ne déborde d'une piste ;
  * validé après fusion de main (39be0e4) : 308/308 VALID en mode par défaut et avec `DDSOM_ASSETS` (assets
    réextraits, `DDSOM_LONG=1`) ; écart du Z80 en retard inchangé ou moindre (ordre global différent dans 5 scénarios,
    valeurs suspendues dans 2 : `covb_long_59`, `long_scene_20`) ; fuzz et tests dans [son.md](son.md) ;
  * reste (L14) : sans ROM, `cps2::Contents` transporte encore la région du Z80 et le programme du DSP, relus en
    tables au démarrage ; `music.rs` est resté dans `ddsom-assets` (à déplacer dans `ddsom-data` avec le lot L1).

**L4. Rendu et mixage depuis les assets, optimisation** (moyen) — **fait** (branche `wt-l4-rendu`, voir « Résultat »)
* Périmètre : `cps2-gfx/`, `qsound/` (boucle des voix, hors tables), `ddsom-assets/src/rebuild.rs` (parties
  `gfx_region` et `qsound_samples`), `cps2/src/lib.rs` (`render`, `Contents`), `ddsom/src/main.rs`,
  `ddsom-game/examples/{bench,render_check}.rs`. Coordination avec L3 sur `qsound/src/lib.rs` : L3 prend les tables,
  L4 la boucle des voix. Les deux lots font des commits séparés sur des fonctions différentes.
* Travail :
  * `TileAtlas` décodé depuis `gfx/cells_NN.png`, sans passer par les plans 4 bits ;
  * banque d'échantillons construite depuis les WAV ;
  * rendu des couches par lignes de tuiles depuis l'atlas ;
  * QSound : boucle des 16 voix.
* Fin :
  * `render_check` donne les mêmes hachages (pixels et audio) sur attract, bot30k, boss10_scene65 et cov_4p_a,
    4 000 images ;
  * `bench` bot30k progresse : au moins 2 500 images/s avec le rendu (base 1 862), au moins 14 000 sans (base 11 408) ;
  * 308/308 dans les deux modes.
* Risque : le rendu n'est pas vérifié par `validate`, qui ne calcule plus l'image. `render_check` et le test
  `mame_snapshots` sont le seul filet : les lancer à chaque étape.
* Résultat :
  * **Atlas de tuiles** (`cps2_gfx::TileAtlas`, `cps2-gfx/src/atlas.rs`) : les cellules 16x16 en un octet par
    pixel (crayon 0-15, 15 transparent), 256 octets par cellule, partagées (`Arc`, cloner `Contents` ne copie plus
    les graphismes). Sans ROM, `ddsom_assets::tile_atlas(gfx)` lit `gfx/cells_NN.png` tels quels (`Cells::into_atlas`,
    aucune conversion) ; avec les ROMs, `cps2_gfx::rom::load_atlas` décode les plans une fois au chargement.
    Correspondance : les 4 octets de plans au décalage matériel `o` deviennent les 8 crayons au décalage `2 * o` de
    l'atlas ; le rendu garde donc l'adressage matériel des tuiles (8x8 : demi-ligne choisie par la colonne).
    `cps2::Contents::graphics`, `Machine::gfx` et `render(&VideoState, &TileAtlas)` prennent l'atlas ;
    `rebuild::gfx_region` ne sert plus qu'au test `rebuilt_graphics`.
  * **Rendu des couches** par lignes de tuiles depuis l'atlas : une ligne de tuile est une tranche de l'atlas
    (`[u8; T]`, retournée si miroir), les tuiles entières sont dessinées en blocs de taille fixe sans branchement
    (vectorisés), les lignes entièrement transparentes sont sautées (la couche 8x8 est surtout vide), plus de
    division par `% ram.len()` ni par `% pal.len()`. Sprites : lignes de cellule découpées une fois à l'écran.
  * **Banque d'échantillons** : `ddsom_assets::samples::bank` (sorti de `rebuild`) place les WAV aux adresses des
    fiches ; `qsound` la lit par un masque (complétée à une puissance de deux, plus de lecture hors bornes).
  * **QSound** : registres des 16 voix en tableaux (`Voices`), échantillons lus pour toutes les voix puis phases
    avancées sur des tableaux entiers (vectorisé) ; gains de pan lus dans `DspTables` à l'écriture du pan
    (`path_gains`, un tableau par chemin) ; mixage des 19 voix et filtres FIR en produits scalaires par
    `pmaddwd` SSE2 (`dot_product`, seul `unsafe` du mixeur, exact modulo 2^32, test `dot_product_wraps_like_the_dsp`) ;
    FIR sur un tampon circulaire déroulé (lu d'un seul tenant depuis `delay_pos`, reconstruit si le nombre de
    coefficients change, invalidé par le chemin général).
  * Outils : `render_check` accepte `DDSOM_ASSETS` et `DDSOM_DUMP_VIDEO=<dir>` (états vidéo d'une image sur
    `DDSOM_DUMP_EVERY`) ; `cps2-gfx/examples/render_bench <roms> <dumps>...` rend ces états seuls et vérifie le
    hachage de chaque image (boucle d'optimisation du rendu en secondes, sans recompiler le jeu).
  * Contrôle : `render_check` identique au code d'avant le lot (39be0e4) sur attract, bot30k, boss10_scene65 et
    cov_4p_a, 4 000 images, pixels et audio, à chaque étape, avec les ROMs et avec `DDSOM_ASSETS` (assets
    réextraits) ; `mame_snapshots` identique à MAME ; tests `ddsom-assets` (dont `rebuilt_graphics`,
    `rebuilt_samples`) et `qsound`.
  * `bench` bot30k, 12 000 images, meilleur de 3 en alternance avec le binaire de 39be0e4 (machine partagée avec
    d'autres agents) : sans rendu 15 505 images/s (base mesurée au même moment 11 542) ; avec rendu 3 268 images/s
    (base 1 623 au même moment, 1 862 au lot F). Même empreinte d'état final `a65ecd8aa6e38d4f`. Rendu seul
    (`render_bench`) : de ~500 à ~230 µs par image.
  * Validation (après fusion de main 498db69, L3 et L5 compris) : 308/308 VALID en mode par défaut et avec
    `DDSOM_ASSETS` (assets réextraits par la branche), `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1` ; `cargo clippy`
    sans avertissement (espace de travail avec les outils, `ddsom-game` et `ddsom` sans).
  * Pour les lots suivants : `cps2::Contents::graphics` est un `TileAtlas` (plus de région planaire dans le jeu) ;
    un outil qui lisait `Machine::gfx` en plans lit `atlas.pens()` au décalage `2 * o` (voir `level_maps`). La
    banque d'échantillons dépend de `Music::sample_count`/`sample_span`/`sample_tails` (L3).

**L5. Effets** (moyen) — **fait** (branche `wt-l5-effets`, voir « Résultat »)
* Périmètre : `families/effects.rs`, `effects_b.rs` à `effects_g.rs`, `effects_creators.rs`, plus
  `extract/src/effects.rs` et `ddsom-data/src/effects.rs`.
* (a) Découpage : `effects_c.rs` (5 162 lignes) en `families/effects/c/` par groupe de types EFFCT (8 à 10 fichiers de
  moins de 800 lignes) ; `effects.rs` (2 424) et `effects_g.rs` (1 273) idem.
* (b) Données : 292 accesseurs ROM et 358 constantes d'adresses ROM. Par exemple, le rideau de la porte du SET E
  (`effects_e.rs`, 0xadad0-0xae118), et les tables de pièces et de couleurs.
* (c) Champs : 525 décalages littéraux et 135 `A5 + littéral`.
* Fin : chiffres (b) et (c) à 0, aucun fichier au-delà de 1 000 lignes, 308/308 dans les deux modes.
* Résultat :
  * (a) Découpage (48fb00b) : `families/effects/` — socle (`mod.rs` : `three_states`, `bad_index`...) et types de
    l'ancien `effects.rs` en `player.rs`, `screen.rs`, `bursts.rs`, `followers.rs` ; `c/` (11 fichiers : `shops`,
    `carriers`, `scenery_fx`, `intro`, `voting`, `attacks`, `mechanisms`, `ending`, `screen`, `paths`, `mod`),
    `e/` (`gates`, `attacks`), `g/` (`thrown`, `enemy`, `rockets`), `b.rs`, `d.rs`, `f.rs` ;
    `effects_creators/` (`mod`, `leftovers`, `scenery`), chemin inchangé pour ses appelants. `examples/names`
    identique ; plus grand fichier 881 lignes (`effects/d.rs`). `measure_end.py` : zone `effects` étendue aux
    dossiers.
  * (b) Données : `data/effects.json` (`ddsom-data/src/effects.rs`, `GameData::effects`), 204 tables nommées
    déclarées une fois par la macro `tables!` (nom, espace données ou programme, taille des éléments, adresse,
    fin éventuelle, sens) ; `extract/src/effects.rs` lit chaque table jusqu'à la table suivante ou au prochain
    code exécuté (`code_68k.txt`, au plus 0x400 octets) sauf fin donnée ; `rebuild::put_effects`, tests
    `effects_lossless` et `to_data` (ROM = assets). Le jeu lit par `families/effects/tables.rs` : `at8/16/32`
    (décalage en octets, comme l'original), `nth16/32`, `held8/16` (donnée désignée par une adresse rangée dans
    l'objet : script des lettres, chemin du messager, trajectoire des canons) ; au-delà des octets extraits, le
    résidu (`Missing::byte_in/word_in/long_in`, ajoutés à `data.rs`). Les tables à fiches de longueur variable
    (décalages puis fiches) sont une seule table, lue relativement à son début. Quelques palettes de la
    bibliothèque copiées par les effets sont des extraits (`interface_palettes`, `purse_portraits`,
    `line_portraits`...) : mêmes octets que `palettes.png`, à remplacer par la bibliothèque typée (L12). Cas
    d'origine gardés : bulle de la balise lue à la variante -1 (`beacon_sheets` commence une fiche avant),
    vitesses X des éclats lues jusque dans la table des Y, fin de la jauge lue à rebours (`purse_gauge_end`).
    Plus de lecture des tables d'aiguillage : `effects::target` ne lit plus la ROM (il rend la case ; `items.rs`
    s'en sert encore pour ses messages d'erreur), `bad_index` pour les cas hors table. Entrées des natives en
    littéraux de `handler!`, jeux d'animation, tables de vitesses et entrées de tâches en `RomAddr` (clés).
    Résidu de la zone sur les 308 scénarios : 0 (les clés restantes sont celles de L1 : images, animations,
    vitesses).
  * (c) Champs : `record!` par type dans `effects/records.rs` (communs : `Effect`, `Carrier` — l'objet porteur
    ou créateur, `PlayerView` — les champs de la fiche de joueur lus par les effets, à fusionner avec la fiche
    de L10 —, `Interface`, `Blinker`, `Coin`) et `effects/c/records.rs` (`Ballot`, `VoteCursor`, `VoteTimer`,
    `Shop`, `Purse`, `Buyer`, `Spell`, `Toss`, `Card`, `Letters`, `Courier`, `Pillar`, `Ferry`...) ; un sens
    deviné est marqué (h). Variables de la RAM de travail dans `effects/vars.rs` (`Byte`/`Word`/`Long` de
    `ram.rs`), sinon `crate::ram` (`SCENE`, `CAMERA_X`, `PLAYERS.nth(k)`...). Les modules de constantes de champs
    déjà nommées (`shop::PRICE`...) restent tels quels.
  * Mesures (`measure_end.py`, zone `effects`) : 8 → 33 fichiers, 13 245 → 13 908 lignes, accesseurs ROM
    292 → 0, décalages littéraux 551 → 0, `A5 + littéral` 136 → 0, constantes d'adresses ROM 331 → 0,
    `rd*`/`wr*` 322 → 311, plus gros fichier 5 158 → 881 lignes.
  * Validation : 308/308 VALID après (a) (mode par défaut) ; après (b) seul, 308/308 (mode par défaut) ; final,
    après fusion de main (0b0486d, L3) : 308/308 VALID en mode par défaut et avec `DDSOM_ASSETS` (assets
    réextraits par la branche), `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1` ; résidu identique dans les deux
    modes à celui de L1 (16 146 lectures, 19 clés, aucune de la zone) ; `DDSOM_STRICT=1` sans arrêt sur les
    scénarios qui lisaient hors des tables avant leur extension ; tests `ddsom-assets` (`effects_lossless`),
    `extract` (`to_data`) ; `cargo clippy` sans avertissement, avec et sans `tools`.

### Vague 2 (trois agents)

**L6. SET** (moyen) — **fait** (branche `wt-l6-set`, voir « Résultat »)
* Périmètre : `families/set.rs`, `set_b.rs`, `set_c*.rs`, `set_residue.rs`, `scenery.rs`.
* (a) `set_b.rs` (3 628 lignes) et `set.rs` (2 500) découpés par type de SET.
* (b) 247 accesseurs ROM. Tables par variante des scènes de SET C (`set_c_scene.rs`, 0xbcfc6-0xc821c), finales
  (0xce51a), positions (0xd4198), table du rideau (0xbcea4).
* (c) 1 108 décalages littéraux, 182 `A5 + littéral`.
* Fin : comme L5.
* Résultat :
  * (a) Découpage : `set.rs` et `set_b.rs` deviennent des racines de modules (`set/`, `set_b/`), un fichier par
    type ; `set_c_*.rs` rangés dans `set_c/` (`scene`, `window`, `window/text`, `finale`, `finale/later` et ses
    trois sous-modules, `finale/status`), sans `#[path]` ; plus gros fichier 623 lignes. `examples/names`
    identique ; 308/308 en mode par défaut (10b7b88). `measure_end.py` compte les sous-dossiers dans la zone set.
  * (b) Données : `GameData.set` (**nouveau `data/set.json`**, `extract/src/set.rs`, test `set_lossless`) et
    `GameData.dialogs` (`data/dialogs.json`, déjà extrait, désormais chargé). Les scripts des SET 0, 4 et 6 quittent
    `rest.json`, les portes et vagues des SET B quittent `scenes.json`. Tables de forme connue (scripts de scène et
    de vagues, rideau, portes...) et tables à étendue automatique (jusqu'au code exécuté ou à la table suivante :
    les lectures hors bornes de l'original restent dedans) ; couleurs lues dans la bibliothèque de palettes en
    tables `palette@<a>`. Le jeu lit par adresse dans ces tables (`families/set/tables.rs` : `byte`/`word`/`long`,
    `pc_*` en espace programme ; une adresse de RAM passe au bus, une adresse hors tables au résidu clé « set »).
    247 accesseurs → 0, et les lectures `rd*` de la ROM du périmètre (scripts de cases et de palettes, définitions
    et textes des fenêtres...) converties. Résidu sur les 308 scénarios : aucune clé « set » (seules restent les 16
    clés d'images et d'animations de L1).
  * (c) Vues : macro `view!` et `record!` dans `set/fields.rs`, `set_b/fields.rs`, `set_c/fields.rs` (`SetObj`
    commun : sous-état, attribut, paramètre, argument, propriétaire, phase de suspension ; `DoorObj` partagé par
    les portes 7, 25, 27, 29 ; `DialogObj`, `LifeBarObj`, `ContinueObj`, vues par variante de SET 15...) ;
    `PlayerFields` pour les champs de joueur que `player::Player` ne nomme pas encore (0x7d, 0x140, 0x15c) ;
    variables A5 dans `set/vars.rs` ; le `Pool` en double de `set_b` remplacé par `pools::Pool`. Décalages
    littéraux 1 141 → 0, `A5 + littéral` 180 → 0 (`measure_end.py`).
  * Validation (après fusion de main 334316e : L4, L5, L7, L8a) : 308/308 VALID en mode par défaut et avec
    `DDSOM_ASSETS` (assets réextraits par la branche), `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1` ; résidu sans
    clé « set » ; tests `ddsom-assets` (dont `set_lossless`), `extract` (`to_data` : set et dialogs) ; `clippy` sans
    avertissement avec et sans `tools`. Une première validation de (c) avait trouvé 9 échecs : deux noms de champ
    identiques dans une même vue (`RefillObj::MARK`, `RankingObj::ORDER`) avaient fait écrire l'ordre d'un effet
    créé au mauvais décalage ; corrigé (`ScreenEffect::ORDER`). Les constantes d'adresses ROM restent (196 dans la
    zone : tables nommées par leur adresse, routines d'origine).
  * Décisions pour les lots suivants : un lot de données peut charger un fichier `data/*.json` déjà extrait dans
    `GameData` (comme `dialogs`) plutôt que de le recopier ; les champs nommés ici que d'autres familles lisent
    (`SetObj::OWNER`/`PARAM`/`ARG` des objets créés par les scripts, `PlayerFields`, `WindowEffect`) sont à
    reprendre dans les API communes au lot L12 (et `PlayerFields` dans `player::Player` au lot L10). Les constantes
    de champ déjà nommées des modules (avant L6) restent des constantes `u32` à côté des vues.

**L7. ESHL, SOBJ, objets, caisses** (moyen à grand) — **fait** (branche `wt-l7-objets`, voir « Résultat »)
* Périmètre : `families/eshl*.rs`, `sobj.rs`, `items.rs`, `items_b.rs`, `boxes.rs`, `shell.rs`.
* (a) `eshl_c.rs` (3 300 lignes), `eshl_b.rs` (3 054), `items.rs` (2 397) et `sobj.rs` (2 269) découpés par type.
* (b) 274 accesseurs ROM, dont le tir ESHL 5 (listes d'images, boîtes, retournements déjà dans `sprites/tables.json`),
  les types d'objets de SET de `sobj.rs` et la table `0xda398` des objets par personnage.
* (c) 2 378 décalages littéraux, dont 599 dans `sobj.rs` pour 6 nommés : c'est le pire ratio du jeu.
* Factoriser : `rom32` de `sobj.rs` et `eshl_b.rs`, `random` de `eshl_c.rs` et `eshl_d.rs`.
* Résultat :
  * (a) Découpage (388dac6) : `eshl.rs`, `eshl_b.rs`, `eshl_c.rs`, `eshl_d.rs`, `sobj.rs`, `boxes.rs` deviennent des
    dossiers, un fichier par type (`eshl_c/shot40.rs`... ; `sobj/scene/` coupé en `mod.rs` et `later.rs`, variantes
    8 à 23) ; `items.rs` est coupé par type d'ITEM (`items/treasure.rs`, `consumable`, `inventory`, `bonus`, `scene`,
    `reserve`, `escort`, `contact`, `equipment`). Chemins des modules inchangés pour leurs appelants, `examples/names`
    identique ; plus gros fichier 652 lignes. `measure_end.py` : zone étendue aux dossiers. 308/308 VALID (mode par
    défaut).
  * (b) Données (94a2113) : `data/objects.json` (`ddsom-data/src/objects.rs`, `GameData::objects`), 172 tables
    nommées par leur lecteur. La macro `tables!` de L5 est généralisée (nom de la structure en tête, `load`,
    `to_map`, `find` dans la macro) et sert aux deux familles ; `extract/src/effects.rs` expose `read_table`
    (même règle de fin : table suivante de la famille, code exécuté, 0x400 octets), `rebuild::put_named`, test
    `objects_lossless`, `to_data` (ROM = assets). `sprites/tables.json` est chargé dans `GameData` (`sprite_tables`,
    `extract::tables::sprite_tables`) : le tir ESHL 5 lit `shot5` et `frame_lists`, les tirs et les objets de contact
    lisent les boîtes de collision des joueurs par `box_byte`. Le jeu lit par `families/shell/tables.rs` (le trait
    `Read` de L5, `held8/16/32` pour une adresse lue dans une table ou rangée dans l'objet). Tables d'aiguillage et
    de routines (`effects::target`, `jump`, effets des consommables, effets spéciaux des caisses, emplacements de
    l'équipement) : plus lues, `bad_index` pour les cas hors table ; `effects::target` et `outside_table` retirés
    ([code-mort.md](code-mort.md), « Lot L7 »). `rom32` de `sobj` et `eshl_b`, `random` de `eshl_c` et `eshl_d`
    supprimés (lectures typées, `x.random()`). Extraits gardés : fiches des caisses, butin et chances de chute
    (aussi dans `data/boxes.json`) et quelques palettes de la bibliothèque (`sobj_palettes`...), à remplacer par
    les données typées de L12. 308/308 VALID (mode par défaut) ; résidu de la zone : deux lectures d'origine hors
    table (débris 45, guide), tables étendues en (c).
  * (c) Champs (529e5df) : `shell/records.rs` — `Base` (champs communs des quatre pools : sous-états, fractions,
    palette, orientation, jeu de boîtes, réglage `+0x2f`, indices de boîtes), `Shot` (ESHL : attaque, propriétaire,
    drapeaux de coup), vues partagées des objets que lisent les tirs et objets (`Host` : ennemi ou boss porteur,
    `Spawner`, `PlayerView` — à fusionner avec la fiche de joueur de L10, comme celle de L5 —, `SetView`,
    `EffectView`) ; une vue par type pour ses champs propres, déclarée par `view!` (structure, `at` et `record!`) en
    fin de fichier (`Shot15`, `Badge`, `Target`, `Element`...) : 56 vues, un sens deviné marqué (h). Les modules de
    constantes de champs déjà nommés (`chest::f`, `items_b::f`...) restent. Les champs de SOBJ qui ne sont pas des
    vitesses (décalages par rapport à la caméra, rectangles, compteurs) ont leurs noms propres. Variables de la RAM
    de travail : `shell/vars.rs` (46 `Byte`/`Word`/`Long`), sinon `crate::ram` ; `shell::camera` choisit la caméra
    du joueur (`A5+0x36`). Jeux d'animation, tables de vitesses et entrées de scripts en `RomAddr` ; entrées des
    `handler!` en littéraux. Les modules des types sont `pub mod` (accesseurs des vues sans avertissement).
  * Mesures (`measure_end.py`, zone `eshl_sobj_items`) : 10 → 113 fichiers, 15 766 → 16 543 lignes, accesseurs ROM
    274 → 0, décalages littéraux 2 426 → 0, `A5 + littéral` 70 → 0, constantes d'adresses ROM 361 → 0,
    `rd*`/`wr*` 51 → 39, plus gros fichier 3 289 → 649 lignes.
  * Validation : 308/308 VALID après (a) et après (b) (mode par défaut) ; final, après fusion de main (71846a8,
    L4 et L8a) : 308/308 VALID en mode par défaut et avec `DDSOM_ASSETS` (assets réextraits par la branche),
    `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1`. Résidu de la zone : une lecture restante (mode 0x1e du guide,
    `scene_end/end_01`), table étendue ensuite (0xdcbdc) et `end_01` revalidé dans les deux modes sans résidu de la
    zone ; les clés restantes sont celles de L1 (images, animations, vitesses). Tests `ddsom-assets`
    (`objects_lossless`), `extract` (`to_data`) ; `examples/names` identique à main ; `cargo clippy` sans
    avertissement, avec et sans `tools`. `rom_reads` (`rom-trace`) n'a pas été relancé : les lectures `rd*`
    restantes de la zone sont toutes en RAM (relevé à la main).

**L8. Ennemis** (grand, coupé en deux lots successifs ou parallèles) — **fait** (L8a : branche `wt-l8a-ennemis`, voir
« Résultat L8a » ; L8b : branche `wt-l8b-ennemis`, voir « Résultat L8b »)
* L8a : `enemy_common.rs`, `enemy_lib.rs`, `enemy.rs` et `reactions.rs` (2 223 lignes, à découper), d'abord seul, car
  ce sont les façades des autres.
* L8b : `enemy_*/` et `enemy_*.rs`, en 94 fichiers, plus réguliers.
* (b) 170 accesseurs ROM : tables d'attitude par niveau (squelette 0x5942a, bête 0x7473e, trog 0x5af66, guerrier
  0x77634...) et cercle de réactions 0x1c94b6. Elles vont dans `data/enemies.json`.
* (c) 1 302 décalages littéraux. Un `record!` par famille d'ennemi, avec les champs de [creatures.md](creatures.md).
* Factoriser : `random` et `rom32` de `enemy_common.rs` sur la version de `world`.
* Résultat L8a :
  * (a) Découpage (3936666) : `reactions.rs` en `families/reactions/` — `mod.rs` (aides, aiguillage des routines,
    natives), `steps.rs` (pas communs : chutes, rebonds, relevé, mort), `throws.rs` (reculs, projections, glissades),
    `hits.rs` (tenu, étincelles, désintégration, étourdi, culbutes, à terre), `grabs.rs` (saisies, tremblement,
    pétrification, brûlure), `spins.rs` (tourbillons) ; plus gros fichier 480 lignes. Commentaires restés en
    français traduits. 308/308 VALID (mode par défaut).
  * (b) Données : les tables vont dans le `data/enemies.json` existant (format `DataFile` de `data.rs`, comme
    `set.json` de L6, plutôt que la macro `tables!` de L5 : le fichier existait, ses tables `anim_sets`,
    `no_grab_down`, `launch_*`, `immunities`, `hp_*`, `approach/*` sont gardées). `extract/src/enemies.rs`
    (`specs`, appelé par `combat::enemies`) déclare : les 47 tables de réactions (`reactions/<famille>`, 43
    routines par code `+0x49`, 42 pour le golem), `hit_sounds`, `death_sounds`, `get_up_routines`,
    `anim_set_routines/<i>` (immédiat du `movea.l` des petites routines de `anim_sets`, types 13, 19, 34, 54),
    `pop_speeds`, `daze_delays`, `quiver_delays`, `seized_offsets`, `seized_damage`, `seized_hit_speeds`,
    `rank_masks`, `chance_rows`/`chance_masks`, `palette_scripts`, et les listes de pièces de `spawn_list`
    (`beetle_nest/…`, `scorpion/…`). `GameData::enemies` (une ligne dans `GameData`, `FILES`), chargé par
    `DataFile::load` ou construit par `extract::data::build` (même fonction que L6, ajout identique) dans
    `to_data_from` ; test `to_data` étendu, `enemies_lossless` existant. Le jeu lit par
    `families/enemy_common/tables.rs` : `table(nom)` ou les accesseurs nommés (`tables::hit_sounds()`...),
    `at8/16/32` à un décalage en octets comme l'original, `nth32` ; `reactions(adresse)` (la table de réactions
    reste désignée par son adresse, clé que la famille passe à `react`/`reaction_routine`) ; `held8/16/32`
    pour une donnée désignée par une adresse lue dans une table (listes de pièces, lignes de masques) ; au-delà
    des octets extraits, le résidu (`Missing`). Le cercle des tourbillons 0x1c94b6 est une table de vitesses de
    `speeds.json` (`tables::circle`). Résidu des façades sur les 308 scénarios : 0.
  * (c) Champs : `families/enemy_common/records.rs`, vue `record!` `Enemy` (champs communs aux ennemis et aux
    chefs : états, positions et vitesses en mots, palette propre `+0x2a/+0x2b`, `+0x2f` groupe, cible `+0x50`,
    gel, étourdissement, PV, `VULNERABLE` `+0x67`, réaction `+0x49`, saisie `+0x84..+0x87`, état à reprendre
    `+0x8a`, compteur `+0x90`, immunités, `ANIM_VARIANT` `+0xbf`, champs du parent d'un ennemi composite
    marqués (h)...). Les vitesses suivent `Obj` (`+0x16` = `AX`, `+0x1c` = `VY`), pas les anciens noms de
    `reactions.rs` (`VY` = 0x16, `VX2` = 0x1c). `enemy_common::f` garde ses noms (`SUB2`, `SUB3`...) comme
    alias des constantes d'`Enemy`. Variables dans `enemy_common/vars.rs` (`var` reste l'alias), plus
    `BOSS_DOWN` (`A5+0x4f`) et `NAGPA_VISIBLE` (0xff2064). Places libres des pools par
    `pools::X.free_count`, joueurs par `ram::PLAYERS`, champs du joueur par `Player::…`.
  * Factorisation : `random` et `rom32` d'`enemy_common` supprimés ; les familles appellent `x.random()` et
    `x.prog32()` (trait `Machine`, réexporté par `enemy_common` pour les modules qui l'importent en bloc) :
    remplacement mécanique dans `enemy_*` et `boss_*`. `spawn_list(x, "beetle_nest", liste)` prend le nom de la
    famille de listes. Le module `enemy_common::rom` (adresses de routines inutilisées, `PAUSE_FLAG`) est retiré
    (`var::PAUSE`).
  * Mesures (`measure_end.py`, 12 fichiers des façades) : accesseurs ROM 35 → 0, décalages littéraux 171 → 0,
    `A5 + littéral` 3 → 0, constantes d'adresses ROM 20 → 0, `rd*`/`wr*` 23 → 6 (écritures de la palette et des
    liens des pièces), plus gros fichier 2 218 → 683 (`enemy_lib.rs`). Zone `enemies` : 94 → 102 fichiers,
    accesseurs ROM 170 → 135, décalages 1 303 → 1 132, `A5` 17 → 14, constantes 204 → 180 (regex de la zone
    étendue au dossier `reactions/`).
  * Validation : 308/308 VALID après (a) (mode par défaut) ; final (1cfb4b7, main c9881d0 inchangé) : 308/308
    VALID en mode par défaut et avec `DDSOM_ASSETS` (assets réextraits par la branche), `DDSOM_SCENES=1
    DDSOM_COV=1 DDSOM_LONG=1` ; résidu identique dans les deux modes à celui de L1 (16 146 lectures, 19 clés,
    aucune des tables des ennemis) ; tests `ddsom-assets` (`enemies_lossless`),
    `extract` (`to_data`), `ddsom-game --lib` ; `cargo clippy` sans avertissement, avec et sans `tools`.
* Pour L8b :
  * une table d'une famille = une ligne `prog`/`data` dans `extract/src/enemies.rs` (`specs`), nommée
    `<famille>/<table>` (les tables d'attitudes par niveau : squelette 0x5942a, bête 0x7473e, trog 0x5af66,
    guerrier 0x77634...) ; la lire par `enemy_common::tables::table("<famille>/<table>").at16(x.b, off)`, ou
    ajouter un accesseur nommé au `named!` de `tables.rs` ; une donnée pointée par une table, par `held8/16/32` ;
  * les constantes `REACT_FIRST`... des familles restent des adresses : ce sont les clés des tables
    `reactions/<famille>` (`react`, `reaction_routine`) ; les familles et chefs qui lisent encore leur table de
    réactions eux-mêmes (`x.prog32(REACTIONS + code)` : `enemy_double`, boss 0, 1, 3, 4, 7, 8, 11, 16, 19)
    peuvent passer par `reaction_routine` (leurs tables sont déjà extraites) ;
  * les champs communs sont ceux d'`Enemy` (`Enemy::VULNERABLE` pour les `VULNERABLE: u32 = 0x67` des
    familles, `STRIKE_KIND`/`STRIKE_POWER`, `GROUP`, `AI_SHOT`...) ; un `record!` par famille pour `+0xa0` et
    au-delà ; `f::SUB2`/`f::SUB3` peuvent devenir `Enemy::STEP`/`Enemy::SUB_STEP` ;
  * les `x.prog32(...)` et `(x.random() as u32)` posés mécaniquement sont à reprendre dans chaque famille.
* Résultat L8b :
  * (a) Découpage (95c723a) : `enemy_riyakaa.rs` (1 051 lignes) en `enemy_riyakaa/` (`mod` : création, arrivées ;
    `decide` : sous-état 2 et aides des décisions ; `attacks` : sous-états 4, 6, 8 ; `hit`) et `enemy_double.rs`
    (818) en `enemy_double/` (`mod` : socle commun, `beasts` : types 22 à 24, `dark_warrior` : type 26).
    `examples/names` identique. 308/308 VALID (mode par défaut).
  * (b) Données : 161 tables des familles ajoutées à `data/enemies.json` par `extract/src/enemies.rs`
    (`families`, une ligne par table, nommée `<famille>/<table>` : `goblin/near_moves`, `trog/blink_delays`,
    `warmachine/wide_jumps`...). Les tables par rang (16 lignes de 32 octets, `RANK / 2`) passent par `rank(...)` ;
    une table dont la fin n'est pas connue du code par `upto(...)` (jusqu'à la table suivante du fichier ou au
    prochain code exécuté, `code_68k.txt`, au plus N octets ; `extract::effects::code_starts` rendu `pub(crate)`) ;
    une table d'adresses de tables par rang par `lists(...)` (la table et les tables qu'elle désigne, lues par
    `held8`). Les tables qui se chevauchent dans la ROM (téléportations : les Y débordent sur les X) sont déclarées
    chacune avec sa longueur lue. Le jeu lit par `enemy_common::tables::table(nom)`, `rank_byte(x, nom)` (octet tiré
    au hasard dans la ligne du rang ; remplace les `row_byte`/`pick_byte` recopiés dans neuf familles),
    `tables::speed16` (tables de vitesses de `speeds.json` désignées par adresse, scarabée), et
    `tables::reactions(clé)` (le double de type 26 lit la table de réactions de son maître). Les tables de sauts
    relatifs de la machine de guerre restent des mots relatifs à leur table (`table_target` ajoute `Table::rom`) ;
    les adresses de code qu'elles désignent sont les clés des branches. Lectures sans effet retirées : la table
    0x71aea de l'ogre (lue puis ignorée, seul le tirage reste) et le saut 0x78fe4 de la machine de guerre (trois
    blocs identiques). Jeux d'animations, clés de réactions et table de vitesses de la dérive en `RomAddr`.
    `pick_wait` de la machine de guerre est `enemy_lib::draw_6e` (même code). Résidu de la zone : 0 (scénarios
    ciblés, puis validation complète).
  * (c) Champs : `Enemy` complété (paramètres d'IA `+0x68..+0x79` nommés d'après `ai`/`ai_aids` :
    `AI_WANDER_ROW/AI_WANDER`, `AI_WAIT_*`, `AI_STANCE*`, `AI_DECISION_A/B`, `AI_BOX_*`, `AI_BYTE*`,
    `AI_POINT_SET` ; `HEADING`, `GROUND_MARK` `+0x3b`, `BOUNDS`/`PRIORITY` `+0x3c/+0x3f`, `BLINK_MASK/COUNT`
    `+0x42/+0x43`, `HIT_STRENGTH` `+0x57`, `LOOT_ROW`/`LOOT` `+0x80/+0x81` (anciens `FLAG_80/81`),
    `WEAK_TO_B/C` `+0x82/+0x83` (armes 0xb et 0xc de `combat`), `AI_ROW_8E`, `HP_TIER(S)`/`HP_RANK`
    `+0xaa..+0xac`, `CHILDREN` `+0xaf` (sur le créateur), `Y_FRACTION`...), vue `Prey` (`HELD` `+0x188` du joueur
    tenu ; les autres champs du joueur par `player::Player`). Un `record!` par famille dans son `mod.rs` (`Goblin`,
    `Kobold`, `Trog`, `Riyakaa`, `DisplacerBeast`, `Double`, `Tentacle`, `Harpy`, `HarpyShot`, `OgreArm`...) pour
    `+0xa0` et au-delà et les champs propres ; les anciens modules `g` sont supprimés (outil
    `records.py` du scratchpad : un champ de `g` dont le décalage est un champ commun devient `Enemy::…`). Variables
    `var::FRAME_COUNTER` (`A5+0x6e`, masqué en place par quelques familles comme l'original), `var::LEVEL`
    (`A5+0x48`). `(x.random() as u32) & m` remplacé par `rand_and(x, m)` ; plus aucun `x.prog32` dans la zone.
  * Mesures (`measure_end.py`, zone `enemies`) : 102 → 107 fichiers, 32 859 → 33 188 lignes, accesseurs ROM
    135 → 0, décalages littéraux 1 132 → 0, `A5 + littéral` 14 → 0, constantes d'adresses ROM 180 → 0,
    `rd*`/`wr*` 8 (façades), plus gros fichier 1 051 → 793 (`enemy_beetle.rs`).
  * Validation : 308/308 VALID après (a) (mode par défaut) ; final (après fusion de main 6038d49, L6 et L7) :
    308/308 VALID en mode par défaut et avec `DDSOM_ASSETS` (assets réextraits par la branche), `DDSOM_SCENES=1
    DDSOM_COV=1 DDSOM_LONG=1` ; résidu 0 sur les scénarios `scenarios/enemy/` (41) et `modes/attract` ; tests
    `extract`, `ddsom-assets`, `ddsom-game --lib` ; `cargo clippy` sans avertissement, avec et sans `tools`.
  * Pour les lots suivants : les boss lisent encore leurs tables de réactions eux-mêmes (`x.prog32(REACTIONS +
    code)`) : `tables::reactions(clé).at32(x.b, code)` ou `reaction_routine` ; `Prey` et `Enemy` sont à fusionner
    avec la fiche de joueur (L10) et la vue `Host` de `shell/records.rs` (L7) ; les champs d'IA d'`Enemy`
    (`AI_*`) sont ceux d'`ai_aids::g`, qui garde ses propres constantes (L12) ; `rank_byte`/`rank_row` servent aux
    boss qui tirent dans une table par rang.

### Vague 3 (trois agents)

**L9. Boss** (grand, coupé : L9a boss 0 à 9, L9b boss 10 à 22, avec `boss_common.rs` dans L9a) — **fait** (L9a :
branche `wt-l9a-boss`, voir « Résultat L9a » ; L9b : branche `wt-l9b-boss`, voir « Résultat L9b »)
* (b) 207 accesseurs ROM : tables de décision des boss 5, 7, 17 et 21, vitesses du boss 15 (0x1c67b8), oscillation du
  dragon (0x8faa2), chaîne du boss 11 (`0x1069f8`..., lignes de 32 octets et trajectoires : la seule donnée de jeu
  relevée mais non extraite, [assets.md](assets.md)).
* (c) 3 182 décalages littéraux, le plus gros stock. Un `record!` par boss.
* Fin : comme L5 ; `fuzz_enemy families` et `fuzz_boss` sans nouvel écart contre main, mêmes graines.
* Résultat L9a (boss 0, 1, 3 à 9, `boss_common` ; boss 2 et 22 sont dans `boss_22/`, le dragon, type 13, dans
  `boss_dragon.rs` : tous trois pour L9b) :
  * (a) Rien à découper : plus gros fichier 767 lignes (`boss_5.rs`, 775 après). `boss_common.rs` devient un dossier
    (`mod.rs` : aides ; `records.rs` ; `vars.rs` ; `warrior.rs`). `examples/names` identique à main.
  * (b) Données : 130 tables dans `data/enemies.json`, déclarées par le nouveau `extract/src/bosses.rs` (appelé par
    `enemies::specs`, une ligne ; `enemies::Bounded` rendu `pub(crate)`), nommées `<boss>/<table>` (`boss/bob_offsets`
    pour `boss_common`). Formes : `rank(...)` (16 lignes de 32 octets par rang), `upto(...)` (fin inconnue du code),
    `pointed(...)` (table d'adresses et tables désignées, lues par `held8/16/32` ; généralise `lists` de L8b à tout
    format), `decision(...)` (tables de choix de 16 octets et table de sauts relatifs dont la longueur est le plus grand
    octet des choix, calculée à l'extraction). Lecture côté jeu par `boss_common` : `decide(x, choix, r, sauts)` (adresse
    du talon d'état choisi, clé du `match`), `jump_target`, `byte`/`word`/`long` (table nommée, décalage en octets),
    `per_rank` (un octet par rang), `enemy_common::rank_byte` (ligne du rang), `tables::reactions` par
    `reaction_routine(x, REACTIONS)` (boss 0, 1, 3, 4, 7, 8), `tables::circle` (tourbillon du boss 5). Les
    aiguillages d'états des boss 7 et 8 (une cinquantaine de tables `jmp (table,PC,Dn)`) deviennent des `match` sur
    l'octet d'état ; les tables d'entrée des décisions (boss 0 et 6) et des formes du corps (boss 3) des tableaux ou
    `match` Rust ([code-mort.md](code-mort.md), « Lot L9a »). Adresses de la ROM restantes (jeux d'animations, clés de
    réactions, routines) en `RomAddr`.
  * (c) Champs : `boss_common::records` : `Boss` (champs partagés : paramètres de l'IA tels que les boss les emploient,
    `WALK_STEPS` `+0x6b`, `IDLE_COUNT`, `WALK_DELAY`, `SHOT`, `DOWN_TIME` `+0x84`, `SHAKE` `+0x90`, crédit d'une mort
    par sort `DEATH_CREDIT`/`DEATH_VALUE` `+0x93/+0x9a`, `HIT_FLAGS` `+0x57`, `X_LOW`/`HEIGHT_LOW`, `BOB_TIMER`
    `+0xa8` ; et le gabarit des boss 0, 1, 4, 6 — et 10, 17, 18, 21 — `ANIM_KIND`/`ANIM_LAST` `+0xa0/+0xa1`,
    `SAVED_STATE`, `TIMER`/`TIMER2`, `PHASE`/`PHASES`, `INVULNERABLE` `+0xad`, `MARK_X` `+0xbc`), `Message` (fin d'un
    message de scène `+0x22`) ; `boss_common::warrior::Warrior` (gabarit des deux guerriers noirs, boss 7 et 8) ; un
    `record!` par boss dans son `mod.rs` (`Boss0`..`Boss9`) pour le reste ; `Salvo` (tir ESHL 0x2a du boss 6). champs du
    joueur tenu par `player::Player` (`SLOW` `+0x66`, `TIMER_6F` `+0x6f`, `HOLDER_ENEMY` `+0x186`, `HELD`), champs du maître des doubles par
    `enemy_double::Double`, rang des pièces par `enemy_dbeast::DisplacerBeast::PART`. Variables :
    `boss_common::vars` (réexporte `enemy_common::vars` ; `STAGE_CLOCK` `A5+0x38`, `PLAYERS_FROZEN` `A5+0x42`,
    `BOSS_FIGHT` `A5+0x50`, `CAMERA_MODE`, `PREV_CAMERA_X/Y`, `FADE_*`, `SCENE_STEP`, `BODY_OWNER` `A5-0x5fb2`,
    `SCENE_ACTOR` `A5-0x55ba`, `DEFEAT_*` `A5-0x557c..`), `var` de `boss_common` désigne ce module ; places libres par
    `pools::X.free_count`.
  * Factorisation : `boss_common::warrior` (choix d'un joueur, balayage, demi-tour, pas de côté, esquive, choix
    d'attaque et suite, aire de jeu par `Kit`) remplace les deux `aids.rs` recopiés des boss 7 et 8 ; le seuil de ralenti
    du boss 8 (0x89788) est `warrior::hp_threshold` (0x87048, même code) ; `record_defeat` (relevé « boss vaincu » du
    décompte) remplace six recopies ; `jump_target`, `ai_box`, `expired` du boss 1 sont ceux de `boss_common`.
  * Mesures (`measure_end.py`, fichiers de L9a : `boss_common/`, boss 0, 1, 3 à 9) : 40 → 42 fichiers, 14 067 →
    13 741 lignes, accesseurs ROM 105 → 0, décalages littéraux 1 322 → 0, `A5 + littéral` 67 → 0, constantes
    d'adresses ROM 61 → 0, `rd*`/`wr*` 22 → 2 (`set_abs8`, gardé pour L9b, et le mot de contrôle vidéo 0x400004 du
    boss 9). Zone `bosses` : 83 → 86 fichiers, accesseurs ROM 207 → 107, décalages 3 191 → 1 899, `A5` 171 → 107,
    constantes 114 → 58, `rd*`/`wr*` 146 → 127, plus gros fichier `boss_5.rs` (775).
  * Validation : voir « Validation L9a » ci-dessous.
* Pour L9b :
  * même recette : tables dans `extract/src/bosses.rs` (ajouter une section `// ---- boss N ----` ; `decision`,
    `rank`, `upto`, `pointed`), lecture par les aides de `boss_common` ; vérifier que chaque nom lu est déclaré et
    lu (`tools/boss_refactor/check_tables.py`) ;
  * champs : outils de `tools/boss_refactor/` (README) : `brec.py` (le `mod g` d'un boss en `record!`, en prenant les
    champs de `Boss`/`Enemy` de même nom et de même décalage ; option gabarit pour les boss 10, 17, 18, 21), puis
    `common_fields.py` avec `BOSS_FIELDS=1` (décalages communs), `romaddr.py` (constantes de la ROM en `RomAddr`),
    `jt.py` (aiguillages `dispatch`/`step` en `match` sur l'octet, d'après `prog_dec.bin`) ; ces outils ne changent
    aucune valeur, mais `common_fields.py` prend `x.set8(off, 0)` pour un décalage (vérifier les `for off in`) ;
  * piège : `boss_common::var` est `boss_common::vars`, qui réexporte `enemy_common::vars` ; une variable de même nom
    déclarée dans `vars.rs` masque celle des ennemis sans erreur (le dragon a cassé sur `FROZEN`, `A5+0x4e`, d'où
    `PLAYERS_FROZEN` pour `A5+0x42`) ;
  * restent pour L9b dans `boss_common` : `set_abs8` et `A5_DEATH` (utilisés par les boss 10 à 22 : passer par
    `record_defeat` ou `var::DEFEAT_*`, puis les retirer), `leader`, `scroll_x` (dragon) ; l'oscillation du dragon
    (0x8faa2) et ses seuils (0x8f97a), la chaîne du boss 11, les tables de décision des boss 17 et 21, les vitesses du
    boss 15 ;
  * les tables de réactions des boss 10, 11, 16, 17, 19, 21, 22 sont déjà déclarées (`reactions/boss_*`) :
    `reaction_routine(x, REACTIONS)`.
* Résultat L9b (boss 10, 11/12, 15/20, 16, 17, 18, 19/14, 21, 2/22 de `boss_22/`, dragon `boss_dragon.rs` ; restes de
  `boss_common`) :
  * (a) Rien à découper : plus gros fichier de la zone 776 lignes (`boss_5.rs`), aucun fichier de L9b au-delà de
    800 lignes.
  * (b) Données : 138 tables de plus dans `data/enemies.json` (268 pour les boss) (sections `// ---- boss N ----` de
    `extract/src/bosses.rs`, mêmes formes que L9a) : décisions des boss 17, 18 et 21 (`decision(...)`, plus les tables
    d'entrée par phase et côté `front_phases`/`far_phases`/`far_levels`, mots relatifs lus par `jump_target`), tables
    par rang, par difficulté et par nombre de joueurs, coins et points de téléportation, lancers ; réactions relatives du
    boss 18 (`boss_18/reactions`, par code / 2) ; **chaîne du boss 11 extraite** (`boss_11/rows` : 22 lignes de 32
    octets par pose ; `boss_11/paths` : 12 × deux adresses ; `boss_11/paths/{0,8,10}` : 32 orientations × 32
    inclinaisons × 4 mots ; `boss_11/rises/{0,8,10}` : 32 mots), lue par `boss_11::chain` (`path16` : résolution de
    l'adresse parmi les six tables, sans balayage) ; chemin d'approche `boss_11/route_0` lu par adresse
    (`tables::held16`). Oscillation du dragon 0x8faa2 (`boss_13/bob_speeds`, fenêtres de quatre mots tous les deux
    mots) et seuils 0x8f97a (`boss_13/stops`). Vitesses par table de `speeds.json` désignées par adresse
    (`tables::speed16` : sinus du boss 15 0x1c67b6, poussée 0x1c6636, tables de vitesse des objets),
    `tables::circle` (arc du boss 11). Réactions par `reaction_routine` (boss 11, 16, 19 ; 10, 17, 21 déjà) ;
    aiguillages relatifs du boss 11 (0x8d5ca : lecture sans effet, D1 n'est pas un reste ; 0x8dbde..., 0x8ed7a,
    0x8efa2) et du boss 19 (0x90ada) en `match` sur l'octet (outil `jt.py`, étendu à `dispatch0`).
  * Lectures au-delà des tables gardées comme l'original (glitches d'origine, tables étendues) : `boss_11/give_up_masks`
    (16 masques, l'index ajoute l'octet fort d'un D2 laissé par les tests de boîte : 0x4008 octets, observé jusqu'à
    0x288), `boss_22/counter_chances` (cinq longs, lus par lignes de 16 octets : code lu à partir de deux joueurs),
    `boss_13/stops` (quatre seuils puis le code, 48 mots).
  * (c) Champs : un `record!` par boss (`Boss10`, `Boss11` et `Ring`, `Boss15`, `Boss16`, `Boss17`, `Boss18`,
    `Boss19`, `Boss21`, `Boss22`, `Dragon` et `Leader`) ; le gabarit `Boss` `+0xa0..+0xbd` pour 10, 17, 18 et 21
    (`Boss::TIMER2` remplace leurs `COUNT`, `Boss::IDLE_COUNT` leurs `APPROACH`) ; `Boss::WEAK_ELEMENT` /
    `RESISTED_ELEMENT` (`+0x9e/+0x9f`, `combat::damage`). Objets créés par les champs des autres lots : `ShotB::HOST`,
    `Shot32`, `Shot33` (tirs ESHL 0x20/0x21), `Base::SETTING`, `Enemy::CREATOR`, `Message::DONE`, `Player::…`
    (`SPELL_FROZEN`, `HELD`, `TIMER_6F`, `SUBACTION`, `SLOW`, `CARRY`, `AURA`, `HOLY`, `SPELL_CANCELLED`). Variables
    (`boss_common::vars`) : `INPUT_MASK` `A5+0x43`, `BLINK_SUSPENDED`, `NO_WALK_IN`, `NO_VICTORY`, `LANDING_CUE`
    `A5+0x500`, `ALT_CAMERA_EDGES` `A5+0xa`, `END_COUNTDOWN` `A5-0x713a`, `SCENE_SEQUENCE` `A5-0x55c8`,
    `RANK_HIGH` (octet fort de `RANK`, lu comme un octet par le boss 11), réexports `DISPLAY_PANELS`,
    `SCROLL_Y_PLANE1` ; aucune ne masque une variable d'`enemy_common::vars` (vérifié nom par nom). Joueurs par
    `ram::PLAYERS`, caméra par `ram::camera`, places libres ESHL par `pools::ESHL.free_count`.
  * `boss_common` : `set_abs8` et `A5_DEATH` retirés (relevé de la défaite par `record_defeat`, ou `var::DEFEAT_*` /
    `DEFEAT_NAME_WORD` quand le premier octet n'est pas 0) ; `leader` et `scroll_x` déplacés dans `boss_dragon.rs`,
    leur seul utilisateur.
  * Outils (`tools/boss_refactor/`) : `self_fields.py` (champs propres du boss et des objets créés, sans prendre
    `x.set8(off, 0)` pour un décalage, fichier `SPEC` des noms par receveur), `a5vars.py` (variables A5, `set_abs8`),
    `jt.py` (aussi `dispatch0`), `check_tables.py` (tables lues par adresse).
  * Mesures (`measure_end.py`, zone `bosses`, depuis L9a) : 86 → 86 fichiers, 30 844 → 30 965 lignes, accesseurs ROM
    107 → 0, décalages littéraux 1 899 → 0, `A5 + littéral` 107 → 0, constantes d'adresses ROM 58 → 0, `rd*`/`wr*`
    127 → 9 (chaîne du boss 11 passée par les champs `Ring` ; restent : colonne ESHL 0x47, cible du boss 11, le mot
    relu à `A0 - 0x7f92` des boss 11 et 19, palette du boss 22, contrôle vidéo du boss 9), plus gros fichier 776.
  * Fuzz (mêmes graines que main) : `fuzz_boss` (bot_char0 4500/60, boss11_scene42 3000/40 graine 7, boss17_scene13
    3000/40 graine 3, `DDSOM_ETATS=1`) VALIDE, sorties identiques à main ; `fuzz_enemy families` (kobold_scene7
    3000/40) : sortie identique à main (mêmes paniques d'états tirés hors contrat).
  * Validation : voir « Validation L9b » ci-dessous.
* Validation L9b (d982c10, main b252be3 fusionné : L10, L11, L19) : 309/309 VALID en mode par défaut et avec
  `DDSOM_ASSETS` (assets réextraits par la branche), `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1` ; scénarios de boss
  ciblés avec résidu 0 ; tests `extract`, `ddsom-assets` ; `cargo clippy` sans avertissement, avec et sans `tools`.
* Pour L12 et la suite : toute la zone `bosses` est à 0 (accesseurs, décalages, `A5`, constantes) ; les tables des
  boss sont lues par nom (`boss_common::{byte, word, long, per_rank, decide, jump_target}`) sauf celles désignées
  par une adresse rangée en ROM (`tables::held16`, `tables::speed16`, `tables::reactions`) ; les décisions
  gardent comme clé l'adresse du talon d'état (`match` sur `decide(...)`), à transformer en index si L13 traite les
  tables d'aiguillage ; `Boss`/`Enemy`/`Player` portent les champs communs, les vues de tirs (`Shot32`, `Shot33`,
  `ShotB`) restent dans `eshl_b`.

**L10. Joueurs** (grand) — **fait** (branche `wt-l10-joueurs`, voir « Résultat »)
* Périmètre : `player/`, `families/player/`, `combat/`, `combat.rs`.
* (a) `player/hurt.rs` (1 761 lignes), `cinematics.rs` (1 733), `moves.rs` (1 376) et `characters/*.rs` (cinq
  fichiers de plus de 1 100 lignes).
* (b) 378 accesseurs ROM, le plus gros stock. Réactions aux coups (0xf1cc4-0x1010b2), combos (0xf193a), trajectoires
  de l'anneau (`peff_ring.rs`), déplacements des coquilles (`pshell.rs`), tables de décalages des états de blessure et
  des cinématiques. `data/players.json` et `combat.json` existent déjà : les brancher.
* (c) 2 545 décalages littéraux, dont 2 350 dans `families/player/`. Le `record!` `Player` (0x200 octets) est commun :
  il est déclaré dans `world` par L1, et L10 en est le seul rédacteur pendant la vague.
* Fin : comme L5, plus `fuzz_player` et `fuzz_player_shots` sans nouvel écart.
* Résultat :
  * (a) Découpage (114716e) : `player/hurt.rs` en `hurt/` (`mod`, `throws` : réactions simples, projections,
    chutes, relevés ; `seizures` : réactions à automate propre, 0x22 à 0x3e), `cinematics.rs` en `cinematics/`
    (`mod`, `endings` : fins de scène ; `entries`, `entries_late` : entrées en scène), `moves.rs` en `moves/`
    (`mod`, `climb`, `poses`, `leaps`), `characters/{magic_user,cleric,elf,common}.rs` en dossiers (`spells`,
    `conjurations`, `turning`, `prayers`, `jumps`, `strikes`). Plus gros fichier de ces découpages 617 lignes ;
    `examples/names` identique ; `player/cov.rs` suit les nouveaux modules (`pub(super) fn` compris). 308/308
    VALID (mode par défaut).
  * (b) Données : **nouveau `data/player_tables.json`** (`ddsom-data/src/player_tables.rs`, `PlayerTables`, macro
    `tables!` ; `extract/src/player_tables.rs` ; `rebuild::put_named` ; test `player_tables_lossless` ; `to_data`
    étendu), 311 tables nommées par leur lecteur, et **`data/players.json` et `data/combat.json` branchés** (chargés
    dans `GameData::players`/`combat`, comme `dialogs` au lot L6). Lecture par `player/tables.rs` : par nom
    (`player_data().hurt_animations`, `players("hp_by_level")`, `combat("shot_guard")`) ou, quand le code passe
    l'adresse d'une table (tables de pas des réactions et des cinématiques, tables par personnage, lignes désignées
    par une autre table), par adresse (`pc8/16/32` en espace programme, `byte/word/long` en espace données) dans
    les tables des joueurs, de `players.json`, `combat.json`, puis les vitesses et les boîtes de collision de
    `sprites/tables.json` ; une adresse de RAM passe au bus. Les mots et longs sont lus à l'adresse arrondie au pair
    comme le bus (`Aligned::word_at`/`long_at`) : sans cela `fuzz_player_shots` trouvait un nouvel écart (PEFF 11,
    index impair tiré par le fuzz). Les tables de pas sont nommées par leur adresse (`hurt_steps_3a1fc`,
    `cinematic_24138`) ; les réactions aux coups 0xf1cc4-0x101166 (`stun_chances`, `reaction_rows_*`,
    `reaction_rows`), les combos (`combo_sequences` 0xf193a, `combo_gates` : immédiats des `move.l` relus comme
    données), les trajectoires de l'anneau (`ring0_*`, `ring1_*`), les décalages des coquilles et effets
    (`*_offsets`), les images des armes tenues (`overlay_frames` 0x1147ce-0x11bffa). `hand_offsets` est lu dans
    `data().effects`. Lectures `rd*` de la ROM de la zone converties (palettes d'aura et de sortie, boîtes,
    boucliers à faibles PV, butin). Glitchs gardés en données : mot du code relu comme minuterie
    (`lightning_code_word`), saut hors table de la cinématique 0x24aba (`cinematic_24ad8` étendue à 0x24aea), lecture
    de `+0x15e` au-delà d'un effet (`FOLLOWING_SPECIAL_KIND`).
  * (c) Champs : **une seule fiche `Player`** (`player/mod.rs`, `record!`, 0x200 octets, ~110 champs nommés
    d'après [joueur.md](joueur.md), (h) pour un sens deviné) ; les vues partielles des autres lots y sont
    fusionnées et retirées : `PlayerView` des effets (L5) et des objets (L7), `PlayerFields` du SET (L6), `Prey`
    des ennemis (L8b, `Player::HELD`) ; leurs usages remplacés mécaniquement (`Player::HP` pour `LIFE`,
    `Player::RANK` pour `PHASE`/`ORDER`, `Player::INVULN` pour `RETURN_TIMER`/`DYING`...). Octets de travail de
    l'action courante (`+0x1e8` et suivants) nommés d'après leur usage le plus courant (`ACTION_COUNT`,
    `SPELL_LEVEL`, `SPELL_BLINKS`...). Tirs et effets des joueurs : vue `Shot` (`families/player/records.rs` :
    sous-états, palette, orientation, réglage, attaque, propriétaire, drapeaux de coup), vue `Victim` (champs des
    ennemis que lisent les sorts : `HIT_BY_KIND`, `DEATH_VALUE`, `BITES`...), champs propres à chaque type en
    constantes nommées dans son module ; champs posés par les créateurs (`player/spawn.rs`, module `arg`) ; champs
    des caisses et des faiblesses dans le module `f` de `combat.rs`. Variables A5 dans `families/player/vars.rs`
    (`crate::ram` sinon), compteurs des pools par `pools::X.free_count`. Jeux d'animation, tables de vitesses et
    routines d'origine en `RomAddr` (clés).
  * Mesures (`measure_end.py`, zone `players`) : 57 → 75 fichiers, 29 018 → 29 981 lignes, accesseurs ROM
    378 → 0, décalages littéraux 2 912 → 0, `A5 + littéral` 67 → 0, constantes d'adresses ROM 388 → 141 (adresses
    de tables passées aux lecteurs par adresse et routines des aiguillages), `rd*`/`wr*` 383 → 349, fichiers de plus
    de 1 500 lignes 2 → 0, plus gros fichier 1 759 → 916 lignes (`player/spawn.rs`).
  * Fuzz (mêmes graines que main, `fuzz_player` fighter, cleric, elf, dwarf, mage, thief, spawn, player_action,
    300 tours ; `fuzz_player_shots` PSHL et PEFF, 200 tours, bot30k) : sorties identiques à main (écarts déjà
    connus de main seulement : PEFF 30, PSHL 18).
  * Validation : 308/308 VALID après (a) (mode par défaut) ; final (840f01c, main 6a340af fusionné, L8b compris) :
    308/308 VALID en mode par défaut et avec `DDSOM_ASSETS` (assets réextraits par la branche),
    `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1` ; résidu sans aucune clé des joueurs dans les deux modes (restent les
    clés d'images de L1) ; tests `ddsom-assets` (dont `player_tables_lossless`), `extract` (`to_data` : tables des
    joueurs, `players.json`, `combat.json`), `ddsom-game --lib` ; `cargo clippy` sans avertissement, avec et sans
    `tools` ; `examples/names` identique après (a). `mame_snapshots` non relancé (rendu non touché).
  * Pour les lots suivants : la fiche `player::Player` est la seule vue des joueurs (`Player::of(obj)` pour un
    objet désigné comme joueur) ; ses champs sont à lire par `p.hp(b)` ou `p.u8(b, Player::X)`. `families/player`
    garde ses constantes de champs par type. Restent dans la zone 141 constantes d'adresses de tables lues par
    adresse (à passer en `&Table` au lot L12 ou plus tard) et les lectures `rd*` de la RAM.

**L11. Écrans** (moyen) — **fait** (branche `wt-l11-ecrans`, voir « Résultat »)
* Périmètre : `modes/`, `hud/`, `service/`, `scene/`, `credits*.rs`, `text.rs`, `popups.rs`.
* (b) 62 accesseurs ROM et 437 constantes d'adresses. Scripts de démonstration (0x20070), écrans fixes, colonnes du
  HUD (`hud/layer.rs`, 0x3f54c2-0x3f6300), chaînes terminées par `@`, tables des scènes, classement (0xfc6d8).
* (c) Ici, peu de champs d'objets (15) mais 1 637 `rd*`/`wr*` bruts : variables de la RAM de travail et GFX RAM. On
  les fait passer par `world::ram` et par l'interface `hw::video` (tilemaps, couche de texte), à créer dans ce lot
  pour la couche 8x8.
* Factoriser : `set8`/`set16` locaux de `scene/transition/mod.rs`, `credits_task.rs` et `service/mod.rs`.
* Résultat :
  * (a) Découpage (f0cd2d3) : `scene/exit.rs`, `scene/entry.rs`, `modes/ending.rs`, `modes/demo.rs`, `hud/slot.rs` et
    `scene/transition/early.rs` deviennent des dossiers (`exit/{mod,sequences,custom}`, `entry/{mod,levels,later}`,
    `ending/{mod,states}`, `demo/{mod,presentation,recorder}`, `slot/{mod,play,choose}`, `early/{mod,timed}`) ;
    `examples/names` identique ; 308/308 VALID (mode par défaut).
  * (b) Données : nouveau `data/screens.json` (`DataFile`, `extract/src/screens.rs` sur le constructeur de tables de
    `set.rs` rendu réutilisable : `Tables::new`, `pointed`, palettes) ; `GameData` charge aussi `data/scenes.json`,
    `data/misc.json` et `texts.json` (`scenes`, `misc`, `texts` ; `Texts::regions`, `From<RomAddr> for u32`). Tables
    nommées : colonnes du HUD (`hud_columns` 0x3f54c2-0x3f6c08), lettres, portraits, palettes des panneaux,
    démonstrations (sons, phases, présentation, textes 0x20070), sélection, fin, classement (`ranking_palette`
    0xfc6d8, `ranking_text`), titre, démarrage, transitions (réponses, textes défilants, primes), scripts
    d'apparition (une table exacte par script, plus la table 0xf2f62), menu de service, octets du test ROM
    (`rom_test@…`) ; texte de l'invite (`prompt_texts`). Le jeu lit tout par adresse : `screens/tables.rs` (`byte`,
    `word`, `long`, `pc_*`, type `Rom` = table ou texte désigné par son adresse d'origine ; une adresse de RAM passe
    par le bus ; hors tables, résidu `Missing { table: "screens" }`). Les chaînes `@` passent par les descripteurs
    de `texts.json` (`text.rs`). Adresses de reprise des `steps!` en littéraux, entrées de tâches en `RomAddr`.
    Tests `screens_lossless`, `to_data` étendu (scenes, misc, screens, texts).
  * (c) Variables et matériel : `hw/` (nouveau, couche matériel) — `hw::video` (`Cell`, `Tilemap` `TEXT`/`SCROLL1`,
    `Vram`, `palette(page, couleur)`, `regs` CPS-A/B et sortie objets) et `hw::io` (ports `IN0`/`IN2`, RAM de la carte
    de développement `DevRam`, `Memory` pour le test RAM du menu). Variables de la RAM de travail typées
    (`ram::Byte`/`Word`/`Long`, communes dans `screens/vars.rs`, propres dans les modules), champs nommés
    (`CoinSlot` en `record!`, `screens/obj.rs`, constantes de fiches, `Obj`/`Player`), blocs d'état par
    `screens/block.rs` (`Block`) qui remplace les `get8`/`set8`/`get16`/`set16` locaux de `scene/transition` et du
    menu de service (ceux de `credits_task.rs` sont supprimés). Outils de conversion mécanique dans le scratchpad
    (variables, champs, adresses ROM), chaque étape revalidée sur les scénarios des écrans (113 scénarios).
  * Mesures (`measure_end.py`, zone `screens` étendue à `screens/`) : 41 → 56 fichiers, 18 085 → 18 531 lignes,
    accesseurs ROM 62 → 0, décalages littéraux 15 → 0, `A5 + littéral` 137 → 0, `rd*`/`wr*` bruts 1 637 → 3 (les
    lectures de RAM de `screens/tables.rs`), constantes d'adresses ROM 400 → 0, plus gros fichier 1 533 → 800
    (`modes/select.rs`). Littéraux de la GFX RAM et des registres hors `hw/` : les seuls restants sont les bornes du
    test RAM du menu de service (`service/memory.rs`) et les constructeurs typés (`Port(0x804001)`, `Memory(…)`).
  * Validation : 308/308 VALID après (a) (mode par défaut) ; chaque étape revalidée sur les 113 scénarios des écrans
    (modes, scènes d'entrée et de sortie, menu de service, crédits, boutiques) ; `render_check` identique au code
    d'avant le lot sur attract, bot30k, boss10_scene65 et cov_4p_a (4 000 images) ; `mame_snapshots` identique.
    Final, après fusion de main 2090293 (L6, L8b, L10, L9a, L19) : 309/309 VALID en mode par défaut et avec
    `DDSOM_ASSETS` (assets réextraits par la branche), `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1` ; résidu identique
    dans les deux modes (16 472 lectures, 20 clés, toutes celles de L1 : images, animations, vitesses), aucune de la
    table `screens`. Tests `ddsom-assets` (`screens_lossless`), `extract` (`to_data`), `ddsom-game --lib` ;
    `cargo clippy` sans avertissement, avec et sans `tools`.
  * Pour les lots suivants : la vidéo s'écrit par `hw::video` (`Cell`, `TEXT`, `palette`, `regs`), les ports par
    `hw::io` ; une donnée désignée par adresse se lit par `screens::tables` (ou la table de sa famille) ; les
    caméras et scrolls partagés sont dans `screens::vars` ; `RomAddr` se convertit en `u32` (`From`). Les
    `rd*`/`wr*` restants de la zone sont les lectures de RAM de `screens/tables.rs` ; le test RAM du menu de
    service passe par `hw::io::Memory`.

### Vague 4

**L12. Moteur et communs des familles** (moyen, seul, car il touche les API communes) — **fait** (branche
`wt-l12-moteur`, voir « Résultat »)
* Périmètre : `objects/`, `ai.rs`, `ai_aids.rs` (1 431 lignes), `aids.rs`, `effects_aids.rs`, `terrain.rs`,
  `scroll.rs`, `camera_modes.rs` (1 605), `palette.rs`, `video.rs`, `sprites.rs`, `placement.rs`, `events.rs`,
  `families/{cx,mod,small,creators}.rs`.
* (b) 58 et 11 accesseurs ROM. Pointeurs d'aides à la visée (`ai_aids.rs`, 0x78ae-0x16c78c), tables de scrolls
  (0x33bdc, 0x321fc), index d'entrées (`anim.rs`).
* (c) 1 232 `rd*`/`wr*` bruts, dont une bonne part de vidéo (sprites, scroll, palette) vers `hw::video`.
* Factoriser :
  * `camera_modes.rs` passe sur `Cx` et `Machine` au lieu de sa copie d'accesseurs ;
  * les 4 `alloc` se réduisent à `objects/pools.rs` plus `Current::alloc` ;
  * `random` se réduit à `rng.rs`.
* Résultat :
  * (a) Découpage (79f257a) : `ai_aids.rs` en `ai_aids/` (`mod` : animations par clé, paliers de PV, atterrissage ;
    `hitbox` : posture, boîte devant un adversaire et restes de registres ; `walk` : points de marche ; `grabs` :
    attaques sur les joueurs, saisies, poussées ; `draws` : tirages par ligne, parades, compte à rebours ; `loot` :
    débris et butin) ; `camera_modes.rs` en `camera_modes/` (`mod` : initialisation et mise à jour ; `follow`,
    `stage`, `modes`, `camera2`). `examples/names` identique ; 309/309 VALID (mode par défaut).
  * (b) Données : **nouveau `data/engine.json`** (`extract/src/engine.rs`, constructeur de tables de `set.rs` :
    tables d'adresses et tables désignées, étendue automatique ; 402 tables : `ai/*`, `aids/directions`,
    `placement/*`, `scroll/*`, `camera/*`, `camera2/*`, `video/*`, `sprites/*`, `small/*`, `creators/*`,
    `rank/presets`, `palette/from_zero`, `ai/hp_thresholds`), test `engine_lossless`, `to_data` étendu. `GameData`
    charge aussi `levels.json`, `terrain.json`, `scene_palettes.json`, `palettes.png` (`ddsom_data::Palettes`, crate
    `png` dans `ddsom-data`), `data/rest.json` et `data/boxes.json` (**23 fichiers**). Lecture par `crate::tables`
    (nouveau) : `Table` (déplacée d'`enemy_common::tables`, qui la réutilise) et `Tables` (par nom, et par adresse dans
    des plages d'octets fusionnées) ; `tables::engine()` réunit `engine.json`, `enemies.json`, `scenes.json`,
    `rest.json` par nom et, par adresse, les cartes des décors, les cartes et types de terrain, les tables des
    palettes de scène, la bibliothèque de palettes, les boîtes de collision et les textes (`texts.json`, dialogues) ;
    `engine_table!("nom")` résout une table une fois par site d'appel. Les aides à la visée (`ai_aids`), les scrolls
    et les chargeurs de grilles (cartes de `levels.json` lues comme l'original les adresse), les caméras (tables par
    scène de `scenes.json`), les palettes, les sprites (boîtes de visibilité, marques au sol, ombres = boîtes de
    collision typées, textes), la visibilité (`cull`), le terrain, le rang, les événements, les créateurs ne lisent
    plus la ROM. Lectures hors des tables de l'original nommées ([code-mort.md](code-mort.md), « Lot L12 ») ; résidu
    0 pour la zone (restent les clés de L1 et `damage_crit_row` de L10).
  * (c) Champs et variables : `Enemy` déplacée dans `objects/enemy.rs` (couche monde) et complétée (paramètres d'IA
    `AI_GOAL_X/Y`, `AI_LAST_POINT`, `AI_BEHAVIOUR`, `AI_OFFSCREEN`, `AI_DRAW_85`, `ONCE_*`, `HIT_BY`, `ANIM_MASTER`) :
    `ai::f`, `ai_aids::f`/`g` supprimés ; `Obj` porte les champs communs (`FLIP`, `TERRAIN`, `OWN_PALETTE`,
    `PALETTE`, `BOX_SET`, `SETTING`, `BODY_BOX`, `PUSH_BOX`, `ATTACK_BOX`, `GROUND_MARK`, `BOUNDS`, `PRIORITY`,
    `ARG`, `OWNER`, `TIED`, `ATTACHED_A/B`) ; `scroll::Camera` (`record!` des deux caméras, champs de `scroll::cam` et
    de `camera_modes::f` réunis) ; vues locales nommées pour les objets créés (`ai_aids::Debris`, `Treasure`,
    `effects_aids::Created`, `Census`, `small::Spawn`). Variables partagées dans `ram.rs` (`SHAKE`, `CAMERA1_X/Y`,
    `CAMERA2_X/Y`, `LEVEL`, `LEVEL_ROW`, `SCENE_LEVEL`, `STAGE_CLOCK`, `CUTSCENE`, `PLAYERS_FROZEN`, `INPUT_MASK`,
    `LEADER`, `PHASE_LONG`, `CABINET_PLAYERS`, `GAME_OVER_DELAY`), réexportées par les `vars` des familles qui les
    déclaraient en double ; `video::VIDEO_CONTROL`, `LAYER_ENABLE`, `OBJECT_BANK`. Vidéo par `hw::video` (`Vram`,
    `Cell`, `palette`, `regs`, nouveaux `ROW_SCROLL_OFFSET` et `OBJECT_BANK`).
  * Factorisation : `camera_modes` sur `Cx<()>` (`Machine`, `Current::alloc`/`move_z`, champs de `Camera`) au lieu
    de sa copie d'accesseurs ; `alloc` d'`ai_aids` remplacé par `Pool::alloc` ; tirages par `rng`. `Cx` passe par
    `Obj` et `ram::{Byte, Word, Long}` ; ses accesseurs de ROM et `Machine::prog8/prog32/data*` sont retirés.
  * Restes des autres lots : `Victim` (joueurs) fusionnée dans `Enemy`, `Leader` (dragon) dans `Player::STEP_X`,
    `Host::LIFE/LIFE_MAX` remplacés par `Enemy::HP/HP_MAX` ; `SetObj::PARAM/ARG/OWNER` et `WindowEffect::WINDOW`
    déclarés sur les champs communs d'`Obj` (`SETTING`, `ARG`, `OWNER`, `TIED`) ; les 16 extraits de la
    bibliothèque de palettes d'`effects.json` et d'`objects.json` sont lus dans `palettes.png`
    (`palette::library_excerpt!`) et retirés de ces fichiers ; les fiches, le butin et l'usure des caisses ne sont
    plus que dans `data/boxes.json` (retirés d'`objects.json`) ; clés du résidu de L1 documentées. Non faits : les
    141 constantes d'adresses de tables de la zone des joueurs et les lectures par adresse `held16`/`speed16`/
    `reactions`/`screens::tables::Rom` (L13/L14 : il faut d'abord savoir quelles adresses restent des clés d'état).
  * Mesures (`measure_end.py`, fichiers du lot : 24 → 35 fichiers, 10 346 → 10 844 lignes) : accesseurs ROM 58 → 0,
    décalages littéraux 194 → 0, `A5 + littéral` 73 → 0, `rd*`/`wr*` 815 → 11 (les accesseurs primitifs d'`Obj` et
    le repli sur le bus des lectures de RAM de `tables`), constantes d'adresses ROM 114 → 10 (des décalages A5 et des
    adresses de RAM que le script compte, plus l'adresse `0xff1190` du tampon des éclairs), plus gros fichier 1 605 →
    829 (`scroll.rs`). Tout le jeu : accesseurs ROM 76 → 13 (hors zone : `config`, `mem`, `data`, `dispatch`,
    `tasks/game`), décalages littéraux 194 → 0, `A5 + littéral` 76 → 3, `rd*`/`wr*` 2 312 → 1 505, constantes
    d'adresses ROM 518 → 414, fichiers de plus de 1 500 lignes 1 → 0, plus gros fichier 916 lignes
    (`player/spawn.rs`).
  * `render_check` identique au code d'avant le lot (d1ea264) sur attract, bot30k, boss10_scene65, cov_4p_a (4 000
    images, avec les ROMs et avec `DDSOM_ASSETS`) ; `mame_snapshots` réussi. `bench` bot30k 12 000 images (meilleur de
    3, en alternance avec d1ea264) : sans rendu 19 567 images/s (base 19 408), avec rendu 3 887 (base 3 857), même
    empreinte `a65ecd8aa6e38d4f`.
  * Validation : 309/309 VALID après (a) (mode par défaut) ; final (2aa6005, code inchangé depuis) : 309/309 VALID en
    mode par défaut et avec `DDSOM_ASSETS` (assets réextraits par la branche), `DDSOM_SCENES=1 DDSOM_COV=1
    DDSOM_LONG=1` ; résidu identique dans les deux modes (clés de L1 et `damage_crit_row` de L10, aucune du moteur) ;
    tests `ddsom-assets` (dont `engine_lossless`), `extract` (`to_data` : engine, levels, terrain, palettes de scène,
    bibliothèque, rest, boxes), `ddsom-data`, `ddsom-game --lib` ; `cargo clippy` sans avertissement, avec et sans
    `tools`.

### Série 5 (un agent, transverse)

**L13. Aiguillage sans adresses** (moyen) — **fait** (branche `wt-l13-aiguillage`, voir « Résultat »)
* Périmètre : `dispatch.rs`, `families/mod.rs` et les macros `handler!`, `lib.rs` (`Native`, `lookup`, `NATIVES`),
  `kernel.rs`, `tasks/mod.rs`, `no68k.rs`.
* Travail :
  * les 10 tables d'aiguillage (`Class::table`, lues en ROM par `prog32`) deviennent des tableaux
    `[Handler; N]` construits à la compilation, indexés par `Obj::KIND`. Les adresses ne restent que comme
    métadonnées, pour les outils ;
  * les entrées de tâches (`tasks::entry(entry)`) deviennent un `enum Task`. Leur adresse, comparée par `gaps` à la
    table FF0064 de la référence, est rendue par `Task::address()` ;
  * `lookup`, le crible et la table de hachage passent dans `tools`. Le jeu n'a plus de registre par adresse.
* Fin :
  * `grep -n 'lookup\|prog32' ddsom-game/src --exclude-dir=tools` est vide ;
  * `cargo build -p ddsom-game --no-default-features` ne compile plus `all_natives` ;
  * `DDSOM_CHECK=1` compare toujours (via `tools`) ;
  * 308/308 dans les deux modes.
* Risque : un type hors table, comme l'original le permet (`(kind << 2) as u16`), devient un index hors bornes. Il
  faut le garder comme cas nommé : `unreachable!` consigné dans [code-mort.md](code-mort.md), ou lecture de la suite
  de la table.
* Résultat :
  * Tables d'aiguillage : `dispatch::Class` porte `handlers: &[Handler]`, dix tableaux `[Handler; N]` construits à la
    compilation (`PSHL_HANDLERS`... `SOBJ_HANDLERS`, 25, 48, 29, 23, 73, 92, 4, 34, 10 et 16 types, relevés dans
    `prog_dec.bin`), indexés par `Obj::KIND`. `Handler::Rust { run, native }` : la fonction et sa native (adresse et
    nom, métadonnées pour `DDSOM_CHECK`, la couverture et le diagnostic d'arrêt), vérifiée à la compilation
    (`h(&native)` refuse un corps autre que `Body::Handler`) ; `Handler::Dead { entry }` pour les quatre entrées sans
    gestionnaire (ENEMY 17, EFFCT 2, 71, 78) ; un type au-delà de la table est un `unreachable!`
    ([code-mort.md](code-mort.md), « Lot L13 » : ce que l'original lirait, souvent la table d'une autre classe).
    `Class::table`/`entry` restent des métadonnées (et l'adresse de reprise de `Step::Dispatch`) ; `Class::index`
    remplace la recherche de la classe à chaque balayage.
  * `handlers!` nomme chaque gestionnaire (`ESHL_00 0x9dbfc "eshl_00" shot0::run` : une constante par entrée, plus
    `NATIVES` pour les outils), comme `handler!` ; les tables désignent ces constantes.
  * Tâches : `tasks::Task` (`Startup`, `Modes`, `Game`, `Service`, `Fading(Fade)`, `Credits`, `Interface`,
    `Portraits`, `Prompt`, `Freeze`, `Scores`) et `tasks::fading::Fade` (les dix fondus, ex-`TASKS`) remplacent les
    adresses d'entrée : `kernel::create_task(b, slot, Task)`, `kernel::restart(b, Task)`, `tasks::restart(x, Task)`,
    `Resume::Entry(Task)`, `Slot::task: Option<Task>` ; `Task::first_step()` remplace `tasks::entry`, `Task::address()`
    sert à `gaps` (FF0064) et au diagnostic. Les constantes `RomAddr` des créateurs de tâches (fondus de
    `scene::fading`, `rom::DIM_TASK`, `fireworks::FADE_*`, `MODE_LOOP`, `BOOT_TASK`...) sont des `Task`. Côté outils,
    `tools::kernel::task_at` retrouve la tâche d'une entrée posée par le code d'origine (A0 de `0x9b6`/`0xae4`) et
    `Resume::Address` (fonctionnalité `tools`) garde une entrée qui n'est pas une tâche du jeu (table reconstruite
    depuis FF0064 par `fuzz_couv`).
  * Registre par adresse : `lookup`, le crible, la table de hachage, `is_native`, `all_natives`, `game_natives` et la
    liste `NATIVES` de `lib.rs` (natives mémoire appelées par le seul code d'origine) passent dans `tools::registry`
    (réexportés par `lib.rs` avec `tools`, pour les exemples) ; `no68k::native` devient `no68k::note` (dernières
    natives pour le diagnostic d'arrêt). `lib.rs` garde les types `Native`, `Body`, `regs` (métadonnées). Les
    réactions communes des ennemis sont reconnues par leur liste d'adresses (`reactions::ROUTINES`), plus par la
    liste des natives. Phase 8 de la partie : l'adresse du gestionnaire (relue par l'entrée de scène, A0) est une
    constante (`PHASE_8_HANDLER`, 0x20c38) ; `mem::prog32` passe dans `tools` (le résidu lit deux mots).
  * Fin : `grep -n 'lookup\|prog32' port/ddsom-game/src --exclude-dir=tools` vide ; `cargo build -p ddsom-game
    --no-default-features` ne compile plus le registre ; `DDSOM_CHECK=1` compare toujours (bot30k, 2 000 images :
    21 444 appels comparés, 0 écart) ; tests `tools::registry` (`unique_inputs`, `dispatch_handlers_are_game_natives`).
  * Boucle d'image (`no68k::line`) : le point et le masque d'interruption sont lus une fois par tour dans la table
    des tâches (trois lectures `dyn Any` auparavant), et les dernières natives (diagnostic d'arrêt) sont un anneau
    fixe (`Cell`) au lieu d'un `VecDeque` sous `RefCell`.
  * Mesures (`measure_end.py`, tout le jeu hors `tools/`) : accesseurs ROM 13 → 9 (restent `config`, `mem`,
    `data`), constantes d'adresses ROM 414 → 410, lignes 172 911 → 173 211 (les dix tableaux) ; `tools/` 6 448 →
    6 606 lignes (`registry.rs`).
  * `bench` bot30k, 12 000 images, meilleur de 3 en alternance avec main d2b0b7c construit dans le même dossier
    cible : sans rendu 19 607 images/s (main 19 464), avec rendu 3 838 (main 3 887), même empreinte
    `a65ecd8aa6e38d4f`. Instructions (`perf stat`, 12 000 images) : sans rendu −1,2 %, avec rendu −0,2 %. L'écart
    avec rendu vient de la disposition du code (rendu inchangé, même nombre d'instructions, cycles +1,3 %) : un
    binaire au code identique mais décalé (fonction ajoutée) varie de ±3 % ; avant l'optimisation de la boucle
    d'image, la branche mesurait 18 800 images/s sans rendu, pour 0,2 % d'instructions de plus que main.
  * Laissé pour L14 et après : les clés d'adresses de routines lues dans les données restent des `u32` comparés
    (`match decide(...)` des boss 0, 1, 4, 6, 17, 18, 19, 21 ; `player::scripted::run_at`, `player::hurt::run_at`,
    les étapes des cinématiques ; `reactions::run_common` ; `sound::OTHERS`) : ce sont des identifiants de routines
    désignés par des tables de la ROM (mots relatifs, adresses), pas des lectures ; les rendre en index demande de
    typer ces tables à l'extraction (L14). De même les lectures par adresse `tables::held16`/`speed16`/`reactions`,
    `screens::tables::Rom` et les 141 constantes d'adresses de tables des joueurs (`player/tables.rs` : `pc*`/`byte`...
    par adresse, une table pouvant être lue au-delà de sa fin), et les natives `Body::Memory` restées dans les modules
    du jeu (listes `NATIVES`, lues par les seuls outils : L17).
  * Validation (code de da35373, inchangé depuis) : 309/309 VALID en mode par défaut et avec `DDSOM_ASSETS`
    (assets réextraits par la branche), `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1` ; `DDSOM_CHECK=1` (bot30k) ; tests
    `ddsom-game --lib` ; `cargo clippy` sans avertissement, avec et sans `tools` (et `ddsom`).

**L14. Le jeu sans image ROM** (moyen) — **fait** (branche `wt-l14-sans-rom`, voir « Résultat »)
* Préalable : L1, L5 à L13. `rom_reads` doit compter zéro lecture du bus dans 0-0x3fffff sur les 308 scénarios.
* Périmètre : `cps2/src/{lib,bus}.rs` (machine du jeu sans `rom`/`rom_dec`, les vecteurs de reset ne sont plus lus),
  `ddsom/src/main.rs`, `ddsom-assets/src/rebuild.rs` (déplacé vers `extract`/tests), `ddsom-game/src/startup.rs`.
* Travail :
  * `ddsom --assets` appelle `GameData::load`, la banque d'échantillons et l'atlas ;
  * plus de `rebuild::contents` dans le jeu ;
  * `reste.json` n'est plus lu que par `Residue`.
* Fin :
  * `grep -rn rebuild ddsom/src ddsom-game/src` est vide ;
  * `Cps2Bus` de la machine du jeu n'a plus de ROM ;
  * `no_rom` (ex-`sans_rom`), et 308/308 dans les deux modes ;
  * `Residue` est consulté 0 fois sur les 308 scénarios et sur `fuzz_couv` (C02, C04, C06, C10).
* Décision de l'utilisateur (2026-11-11) : retirer `reste.json` des assets (`extract` ne l'écrit plus, le jeu ne le
  charge plus) une fois `Residue` à zéro sur les scénarios et sur `fuzz_couv` ; sinon le garder et lister les
  lectures restantes. Toute lecture restante doit d'abord devenir une donnée typée.
* Résultat :
  * machine du jeu sans ROM : `cps2::Machine::game(GameContents, eeprom)` ; `Cps2Bus::rom` est un
    `Option<Arc<ProgramRom>>` (vue données et vue programme déchiffrée), `None` pour le jeu (une lecture de la ROM y
    panique avec un message) ; les vecteurs de reset ne sont plus lus (seul `Machine::ddsom`, la machine de
    référence et des outils, fait `reset`). `cps2::GameContents` porte l'atlas de tuiles, `ddsom_sound::SoundData`,
    `qsound::DspTables` et la mémoire d'échantillons : plus de région du Z80 ni de programme du DSP
    (`Sound::with_driver`, `QSound::with_tables`). `cps2::Contents` reste pour la machine de référence ;
  * chargement : `extract::install_game_assets` (`ddsom --assets`, `validate`/`bench`/`render_check` avec
    `DDSOM_ASSETS`) installe `GameData::load`, le résidu (paresseux) et rend `ddsom_assets::game_contents` (atlas,
    `music.json`, `qsound_dsp.json`, banque d'échantillons) ; `extract::install_game` fait de même depuis les ROMs
    (extraction en mémoire). `ddsom` avec ROMs joue sur la même machine sans ROM ; seul le diagnostic
    `DDSOM_ORIGINE`/`DDSOM_INTERP` (fonctionnalité `original`) garde une machine avec ROMs ;
  * `rebuild` passe dans `extract` (`extract::rebuild`, tests sans perte `tests/rebuild.rs`, `tests/roundtrip.rs`,
    exemple `full_program`) ; `remainder` dans `ddsom-data`. `grep -rn rebuild ddsom/src ddsom-game/src` : vide ;
  * dernières lectures du jeu par adresse dans la ROM devenues tables typées de `data/engine.json` : réglages
    (`config/*` : blocs par variante, pièces, crédits, message d'erreur de l'EEPROM), en-tête du programme
    (`startup/header` : drapeau de débogage 0xc0, région, carte de développement), palette de démarrage, piles des
    tâches, tables des commandes des sons (`sound/*@<adresse>`, liste versionnée `extract/sound_tables.txt`, test
    `tables_listed_for_the_extraction`), table de panoramique, textes des réglages de pièces, tuiles du classement,
    octet du gestionnaire de la pluie d'éclats lu comme donnée ; `mem::prog16`/`prog8` et `Machine::prog16` ne
    servent plus qu'aux outils ;
  * résidu et débordements (docs/sans-rom.md « Le résidu et les débordements de l'original ») : `Missing` lit
    d'abord les 17 débordements nommés de l'original (`data/overreads.json`, liste `extract/overreads.txt`), puis le
    résidu (`ddsom_data::residue`), seul lecteur de `natives.json` et `reste.json` dans le jeu, chargé à la première
    consultation. **`Residue` consulté 0 fois sur les 309 scénarios, dans les deux modes.** `fuzz_couv` (C02, C04,
    C06, C10 : 14 scénarios, 12 000 images, graine 1, 281 337 essais) : **0 depuis les états de base** (appels
    réels), mais 2 536 lectures avec des perturbations « plausibles » et 983 584 en tout, toutes sur des clés que la
    perturbation rend incohérentes (adresses de SET, entrées et sauts d'animation surtout) : aucune table typée ne
    peut les servir. **`reste.json` est donc gardé** (règle de la décision ci-dessus), comme assurance pour les
    chemins jamais joués ;
  * `no_rom` (ex-`sans_rom`) : deux processus (données des ROMs, données des assets) comparés par empreinte à chaque
    image ; `bot_2p`, `boss13_dragon`, `cov_4p_a`, `ending` (12 000), `end_02` (9 000), `cov_service_tests`,
    `attract` (6 000) : identiques, résidu 0 des deux côtés ;
  * `render_check` bot30k 12 000 images : identique à main (`09850058246a1e2c`), aussi avec `DDSOM_ASSETS` ;
  * assets : `extract` écrit `data/overreads.json` ; **réextraire `data`** (un dossier sans ce fichier est refusé ;
    `play.bat` le teste) ;
  * performance : les boîtes de collision ne sont plus lues sur le bus mais dans `sprites/tables.json` ; lues
    octet par octet (`player::tables::byte`, deux recherches par octet), elles coûtaient ~1,5 % sur bot30k. Chaque
    paire de `combat` copie désormais les six octets de ses deux boîtes une fois (`player::tables::box_bytes`,
    `Pair::new`) ;
  * `bench` bot30k, 12 000 images, meilleur de 3 en alternance avec main 538d4ff construit dans le même dossier cible
    (même empreinte `a65ecd8aa6e38d4f`) : sans rendu 20 168 images/s (main 20 110), avec rendu 3 914 (main 3 881).
    Avant la copie des boîtes : 19 711 contre 19 783 (−0,4 % à −1,5 % selon les tirages) ;
  * validation (code de a9b18cb, binaire `validate` c450296a) : **309/309 VALID en mode par défaut et avec
    `DDSOM_ASSETS`** (assets réextraits par la branche), `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1 DDSOM_JOBS=6`,
    `residue: 0` dans les 618 journaux ; tests `ddsom-game --lib` (20) et `extract`/`ddsom-assets`/`ddsom-data`/
    `qsound`/`ddsom-sound` (dont `rebuild` et `roundtrip`, sur les assets réextraits) ; `cargo clippy` sans
    avertissement, avec et sans `tools` (et `ddsom`).
  * piège de construction : un chemin `DDSOM_ROMS` écrit autrement (`port/../rom` au lieu de `rom`) relance la
    génération de `ddsom-rc` ; une fois, sa compilation est restée bloquée plus de deux heures sur une unité
    (`lto cgu.00`) : l'arrêter et relancer l'a ramenée à 6 minutes.

**L15. RAM du jeu hors du bus, modules en couches** (moyen à grand, en deux commits validés) — **L15a fait,
L15b partiel** (branche `wt-l15-world`, voir « Résultat »), **achevé par L15c** (branche `wt-l15c-couches`, plus bas)
* L15a, `World` :
  * la RAM de travail (`[u16; 0x8000]`) et la table des tâches appartiennent au jeu ;
  * `world::ram` y accède sans décodage d'adresse ;
  * `hw` regroupe vidéo, son, entrées et EEPROM ;
  * les fonctions prennent `&mut World` au lieu de `&mut Cps2Bus` (renommage mécanique, 1 129 lignes hors `tools`) ;
  * `validate` lit `world.ram`.
* L15b, couches : supprimer les arêtes à l'envers de la section 1 et déplacer les modules dans
  `core/world/services/actors/screens`. `no68k` devient `frame`.
* Fin :
  * `measure_end.py` ne trouve plus de composante fortement connexe de plus d'un module entre couches (les cycles à
    l'intérieur d'une couche sont tolérés : `kernel ↔ tasks` dans `core`) ;
  * `bench` sans rendu progresse d'au moins 5 % (bus) ;
  * 308/308 dans les deux modes.
* Résultat (branche `wt-l15-world`) :
  * **L15a, `World`** (ac25e07) : `ddsom_game::world::World { ram: Box<[u16; 0x8000]>, tasks: Option<Table>,
    leftovers, hw: Cps2Bus }`. La RAM de travail est lue et écrite sans décodage d'adresse (`World::read16`... : un
    test `a >> 16 == 0xff` puis un index), le reste passe au matériel (`hw`, le `Cps2Bus` dont la RAM reste vide sur
    la machine du jeu). La table des tâches est un champ typé : plus de `Cps2Bus::ext` ni de `dyn Any` dans le jeu.
    `cps2` gagne le trait `Board` (matériel et RAM d'une machine) et `Machine<B: Board = Cps2Bus>` ; la machine du
    jeu est `ddsom_game::GameMachine = Machine<World>` (`install_no_68k(m) -> GameMachine`, la RAM du bus y passe
    sans copie), la référence garde `Cps2Bus`. Toutes les fonctions du jeu prennent `&mut World` (renommage outillé
    sur 148 fichiers, puis correction des champs matériels guidée par les erreurs du compilateur : `b.hw.gfxram`) ;
    `combat::World` devient `combat::Combat`. `mem::Memory` (trait des `rd*`/`wr*`) sert au jeu (`World`) et aux
    outils (`Cps2Bus`). Côté outils, `World::lend` et `World::as_bus` passent la RAM et la table du bus au monde et
    retour sans copie (le 68000 tourne sur un bus) ; `Native::execute` et `call` restent sur le bus,
    `execute_on`/`call_in` sur le monde ; `scenario`, `ddsom` et quelques exemples sont génériques sur `Board`.
    `validate` compare `world.ram`. Validé 309/309 dans les deux modes.
  * **L15b, couches** (d99e04e, 697f69b) : `src/` n'a plus que `lib.rs`, `tools/` et six couches, du bas vers le
    haut : `hw` (`mem`, `video`, `io`), `world` (`World`, `ram`, `record`, `objects/`, `data`, `tables`,
    `screen_tables`, `terrain`, `placement`, `events`, `rng`, `machine`, `ops`, `diag`), `services` (`palette`,
    `text`, `sprites`, `scroll`, `camera_modes`, `video`, `popups`, `sound`, `inputs`, `eeprom`), `actors`
    (`families/`, `player/`, `ai`, `ai_aids`, `aids`, `effects_aids`, `combat`), `screens` (`modes/`, `scene/`,
    `hud/`, `service/`, `credits*`, `block`, `obj`, `vars`), `core` (`kernel`, `tasks/`, `dispatch`, `vblank`,
    `startup`, `frame` (ex-`no68k`), `game`, `config`, `scenario`). Déplacement par `git mv` et réécriture outillée
    des chemins (`crate::x`, `$crate::x`, `ddsom_game::x`, groupes `use crate::{...}`). Arêtes à l'envers retirées :
    `hw` ne dépend plus que de `mem::Memory` (générique, la mise en œuvre pour `World` est dans `world`) ; fiche
    `Player` dans `world::objects::player` ; terrain sous un point dans `world::terrain` ; `RANK` dans
    `world::objects::rank` ; `sbcd` dans `world::ops` ; `impl Current for Cx` dans `families::cx` ; `impl Machine for
    scene::Ctx` dans `scene` ; écran noir en trois images dans `screens::scene::black_screen` ; les registres
    résiduels (`Leftovers`) sont un champ de `World` (comparé par `frame::Snapshot`, gardé dans l'extension du bus
    avec la table pour les outils : `world::Parked`) ; `ai`, `ai_aids`, `aids`, `effects_aids`, `combat` passent dans
    `actors` (ils appellent familles et joueurs) ; `screens::tables` devient `world::screen_tables`.
  * Mesures (`measure_end.py`, qui compte maintenant les modules par couche `couche/module`, les composantes entre
    couches et les arêtes vers une couche plus haute) : lignes du jeu hors `tools/` 173 594 (main 173 324) ; main :
    55 modules de tête, composante de 41 ; branche : 59 modules, composante de 46 **entre couches** (toutes sauf
    `hw`, désormais hors de tout cycle avec `world/ops`, `world/diag`, `core/scenario`), 27 arêtes vers une couche
    plus haute (54 juste après le déplacement).
  * **Critère de fin non atteint** : il reste une composante entre couches. Les 27 arêtes restantes vont presque
    toutes vers la mécanique des tâches de `core` (`tasks`, `kernel`, `dispatch`) : chaque écran et beaucoup
    d'acteurs cèdent la main en rangeant une continuation typée (`tasks::Step`, un `enum` qui nomme les étapes de
    tous les modules) dans la table, que `World` contient. Tant que la table est typée par `Step`, `world → core →
    (screens, actors) → world` est un cycle. Deux voies, à décider : (1) déclarer les étapes (identifiants et
    adresses, données pures) dans `world` et associer chaque étape à sa fonction dans `core` (la plus propre, une
    trentaine d'`enum` à déplacer et l'API de cession — `wait`, `call`, `cede`, `Flow`, `Sequence`, le contexte
    `scene::Ctx` — à descendre dans `world`) ; (2) effacer le type des continuations (pointeurs de fonction ou objets
    `dyn`), contraire au but de L15 (plus de `dyn` dans la table). Restent aussi : `actors/families → screens`
    (Flow, `scene::Ctx`, scripts de scène appelés par les SET), `screens → core/game` (mise à jour du monde
    appelée par les séquences de scène), `screens/services → core/config` (réglages, blocage sur erreur
    d'EEPROM), `services/camera_modes → actors/families` (contexte `Cx`, liste de décor).
  * `bench` bot30k 12 000 images sans rendu, même dossier cible, meilleur de 3 en alternance, même empreinte
    `a65ecd8aa6e38d4f` : **23 959 images/s contre 20 148 pour main 8f550eb (+18,9 %)**, 41,7 µs par image contre
    49,6. Le gain vient de L15a (accès à la RAM sans décodage d'adresse, table des tâches sans `dyn Any`) : la
    branche mesurait déjà 23 877 images/s avant les couches.
  * `render_check` bot30k 12 000 images : identique à main (`09850058246a1e2c`), aussi avec `DDSOM_ASSETS`.
  * Validation (L15a au code de ac25e07, L15b au code de 697f69b) : **309/309 VALID en mode par défaut et avec
    `DDSOM_ASSETS`** (assets réextraits par la branche), `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1`, `residue: 0` dans
    tous les journaux ; tests `ddsom-game --lib` (20) ; `cargo clippy` sans avertissement, avec et sans `tools` (et
    `ddsom`, aussi avec `--features original`).
  * Aucun code mort nouveau.

**L15c. Couches sans arête vers le haut** (branche `wt-l15c-couches`, depuis main 1dc7207) — **fait**
* Décision (option 1 de L15, « Résultat ») : les étapes des tâches et l'API de cession descendent dans `world`, la
  table ne dépend plus des couches hautes ; aucun effacement de type (ni `dyn` ni pointeur de fonction) dans la table.
  Retirer aussi les autres arêtes à l'envers (`actors/families → screens`, `screens → core/game`, `→ core/config`,
  `services/camera_modes → actors/families`).
* Fin : `measure_end.py` sans composante entre couches et 0 arête vers une couche plus haute (ou une courte liste
  justifiée) ; `bench` sans rendu pas plus lent que main ; 309/309 dans les deux modes ; `render_check` identique.
* Résultat (commits 1af4e1e, 55c5733, 22c07f2) :
  * **`world::tasks`** (nouveau) :
    * `table` (ex-`core/tasks/table.rs`, sans `gaps` ni `Next::address`) ;
    * `traps` : les traps du noyau tels que les tâches les appellent (`create_task`, `kill_task`, `wait`,
      `cede_ready`, `finish`, `restart`, `block`, `keep_step`, masque d'interruption) et `Point` ; ils ne font que
      modifier la table ;
    * `mod.rs` : `Flow`, `Ctx` (la structure ; ses accès nommés à la RAM restent dans `screens::scene`), `Wait`,
      `cede`, `wait`, `one_frame`, `wait_then_return`, `restart`, `finish`, `has_ceded`, `Task`, `Step` (l'`enum`
      des étapes de toutes les tâches) et le trait `Sequence` ;
    * `stages` : les 31 `enum` d'étapes (une par module qui cède la main, données pures : variantes, objets,
      compteurs, restes) et les types qu'elles portent (`Fade`, `Brightness`, `Component`, `Target`, `Seq`,
      `GameStart`, `CoinSlot`, `InterludePlayer`).
  * **Le code de chaque étape reste dans son module** : `pub use crate::world::tasks::stages::Title as Step;` puis
    `crate::steps! { impl Step { Entry = 0x... => entry, ... } }`, qui donne `address` et `step` (ou un `impl Step`
    écrit à la main pour les étapes à champs). Les variantes sont donc nommées deux fois (déclaration dans `world`,
    correspondance adresse et fonction dans le module), le compilateur vérifie que la correspondance est exhaustive.
    `core::tasks` associe chaque étape de `Step` à celle de son module (`run_steps!`), donne `Task::first_step`,
    `Task::address`, `Next::address`, et exécute (`execute`, `resume_step`, `return_to_caller`). `core::kernel`
    garde l'ordonnanceur, l'adresse d'un point (`kernel::address`) et `gaps` (comparaison avec la table FF0064).
  * **Une inversion de dépendance, justifiée** : un sous-programme qui revient sans céder exécute aussitôt la
    continuation de son appelant, qui peut être l'étape de n'importe quel module (`Sequence::subroutine`, le retour
    d'un gestionnaire dans la boucle de dispatch, `title::t18_sub`). Ce lancement appartient au cœur : le trait
    `Sequence` (attente, sous-programme, retour empilé ou abandonné, `return_to_caller`) est déclaré dans `world` et
    mis en œuvre par `core::tasks` pour `Ctx`. C'est un appel statique, sans effacement de type ; le remplacer par
    un retour différé (`Flow::Done` propagé jusqu'à `execute`) aurait changé l'ordre des effets dans les appelants
    qui ne sont pas en position terminale (`service::task::turn`, `exit::run_shared`, `exit_fading`).
  * **Déplacements** :
    * `dispatch` et la mise à jour du monde (`update_world`, ordres, joueurs, combat : ex-`core/game.rs`) vont dans
      `actors` (`actors::dispatch`, `actors::update`) ; `core::game` garde les phases 0 et 12 et la fin d'image ;
    * les fondus (`core/tasks/fading.rs`) vont dans `services::fading`, `config` dans `services::config` (ses
      fonctions du menu TEST prennent le monde, plus le contexte d'écran) ;
    * les tâches `prompt`, `fixed_screen` et `service` (`screens::service::task`) vont dans `screens` ;
    * `after_world`, `debug_scene_skip` et `clear_game_variables` (ex-`tasks::game::clear_variables`) vont dans
      `screens::scene` ;
    * `place_players` (0x20de6) va dans `actors::player::init` ; l'état du lecteur de script
      (`ram::SPAWN_SCRIPT_STATE`) est nommé dans `world::ram`, que le décor (SET) remet à zéro sans appeler
      `screens` ;
    * `camera_modes` va dans `actors` avec `update_cameras` et `update_cameras_in_game` (pris à `scroll`) : ses
      scripts créent des objets (contexte `Cx`, listes de décor) ;
    * l'anneau des dernières natives (`note`, diagnostic d'un arrêt) va dans `world::diag`.
  * Mesures (`measure_end.py`) : main 59 modules, composante de 46 entre couches, 27 arêtes vers une couche plus
    haute ; branche 64 modules, **aucune composante entre couches, 0 arête vers une couche plus haute** ; restent
    des cycles à l'intérieur d'une couche (tolérés) : `world` (9 modules : `World` contient la table, dont les étapes
    portent des objets), `actors` (8 : familles, joueurs, aides, dispatch, caméras), `core` (`frame`, `kernel`,
    `tasks`), `screens` (`modes`, `scene`), `services` (`config`, `eeprom`). Lignes du jeu hors
    `tools/` : 174 191 (main 173 594, +597 : les variantes nommées deux fois). Aucun appel de méthode d'une couche
    haute depuis une couche basse (vérifié : `world` n'appelle ni `address` ni `step`, `services` n'appelle plus
    les accès de `scene::Ctx`).
  * `bench` bot30k 12 000 images sans rendu, même dossier cible, meilleur de 3 en alternance, même empreinte
    `a65ecd8aa6e38d4f` : **23 933 images/s contre 23 857 pour main 1dc7207** (+0,3 %, dans le bruit : sur 10 passes
    alternées, médianes 23 730 contre 23 796). Piège rencontré : sans LTO, une fonction non générique n'est pas
    incorporée d'une unité de compilation à l'autre ; après les déplacements, la branche perdait 2 à 4 % jusqu'au
    `#[inline]` des petites fonctions appelées à chaque objet ou à chaque ligne depuis un autre module (`table::call`,
    `call_expected`, `returned`, `ret`, `pop_return`, `has_ceded`, `note_cession`, `forget_cession`, `Point::startup`,
    `between_two_tasks`, `traps::point`, `interrupt_mask`, `set_interrupt_mask`, `diag::note`,
    `tasks::return_to_caller`) ; d'autres `#[inline]` essayés (méthodes de `Table`, `Class::update`) ralentissaient.
  * `render_check` bot30k 12 000 images : identique à main (`09850058246a1e2c`), aussi avec `DDSOM_ASSETS`.
  * Validation au code de 22c07f2 : **309/309 VALID en mode par défaut et avec `DDSOM_ASSETS`** (assets réextraits
    par la branche), `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1`, `residue: 0` dans tous les journaux (aussi 309/309
    dans les deux modes aux étapes 1af4e1e et 55c5733) ; tests `ddsom-game --lib` (20) ; `cargo clippy` sans
    avertissement, avec et sans `tools`, exemples et tests compris (et `ddsom`, aussi avec `--features original`).
  * Aucun code mort nouveau.

**L16. État Rust plus projection** (facultatif, petit par bloc). Voir la section 2. Un bloc par commit, avec la
preuve qu'aucune lecture d'origine ne sort du module (trace des lectures par adresse sur les 308 scénarios).

**L17. Outils hors du jeu** (facultatif, moyen) — **registre fait, crate `ddsom-tools` abandonnée** (branche
`wt-l17-outils`, voir « Résultat »)
* `ddsom_game::tools` (7 018 lignes, 70 fichiers) et les exemples passent dans `ddsom-tools`. Le jeu exporte les
  points d'accroche nécessaires (`check_handler`, `natives` des outils) derrière une fonctionnalité `hooks`.
* Gain : `ddsom-game` n'a plus de `#[cfg(feature = "tools")]`.
* Risque : beaucoup de `pub(crate)` à ouvrir. Si la liste dépasse une centaine d'éléments, on garde le module en
  l'état.
* Résultat (branche `wt-l17-outils`, depuis main 6fd951d) :
  * **Crate `ddsom-tools` : abandonnée selon la règle de risque.** L'essai (branche `wt-l17-crate-essai`, d8b8ef8,
    ne compile pas) a fait la moitié du chemin : `ddsom-game` compilait sans aucun `#[cfg(feature = "tools")]`,
    avec un module `hooks` (fonctionnalité `hooks` : `set_check_handler`, `set_original_code_resumes`,
    `set_call_site`, trois `OnceLock`) et un `Native` réduit aux gestionnaires des tables d'aiguillage ; outils et
    exemples déplacés dans `port/ddsom-tools` (`ddsom_game::tools::` → `ddsom_tools::`, `Native`/`Body`/`regs`
    des outils dans la crate des outils). Mais les natives pour le code d'origine (`tools/<module>.rs`, qui
    reflètent chacune un module du jeu par `use ddsom_game::<module>::*`) lisent **environ 230 éléments
    `pub(crate)` du jeu** (sans compter la trentaine qu'ouvrait l'essai pour les natives du jeu déplacées), plus du double du seuil d'une centaine :
    `families/effects_creators` 48 (les variantes `*_via` qui rendent les registres), `families/eshl_creators` 30,
    `palette` 23, `events` 18, `eeprom` 18, `families/creators` 14, `objects/rank` 13, `families/small` 13,
    `scroll` 8, `sound` 7, `text` 5, `placement` 5, `terrain` 4, `video` 3, `effects_aids` 3, puis 1 ou 2 dans
    `popups`, `player/scripted`, `player/init`, `families/boss_3`, `aids`, `rng`, `objects/{pools,lists,cull,anim}`,
    `modes/demo`, `hud`, `frames`, `set_residue`, `diag`, `credits*`, `tasks` (`forget_cession`, `table_if`).
    Le module `tools` reste donc dans `ddsom-game` derrière la fonctionnalité `tools` (« on garde le module en
    l'état »). Une reprise ne vaudrait que si ces natives à registres disparaissent (elles ne servent qu'au fuzz
    différentiel et à `DDSOM_CHECK` de routines déjà validées).
  * **Registre de natives hors des modules du jeu** (fait, sans rien ouvrir) : les listes `NATIVES`, `natives()`,
    `JOIN`, `SHAKES` des modules du jeu, les 47 natives `Body::Memory` et les 93 natives de gestionnaires que seul le
    code d'origine appelle par adresse (26 actions des personnages, 3 poses rares, 3 secousses, 9 relevés après une
    réaction commune, 52 réactions communes) sont construites dans `tools::registry` (`MEMORY`, `HANDLERS`, macro
    `action!` sur `characters::routine`, `Body::HandlerIn` des outils pour les réactions de
    `reactions::ROUTINES`) ; les gestionnaires des tables d'aiguillage y sont pris dans `dispatch::CLASSES`. Le jeu
    ne garde que ses données : les 341 gestionnaires des dix tables (`handlers!`, adresse et nom pour le
    diagnostic d'un arrêt). Même liste de natives que main (`names` : 1 473 natives, identiques). Retirés : les
    masques `REGS_*` d'`objects` (jamais lus), les enveloppes `init_p1..4`, `clear_scroll1..3`, `send_to_z80`,
    `config::init` (seules les natives les appelaient). `measure_end.py` compte maintenant le registre (colonne
    `natives` : 71 sur main, **0** sur la branche).
  * **`DDSOM_CHECK=1` remarche** : depuis L14 il paniquait aussi sur main (la copie de la machine du jeu, sans ROM,
    exécutait l'original) ; `tools::set_original_rom` donne aux copies la ROM de la machine de référence
    (`validate`). bot30k, 2 000 images : 21 444 appels comparés, 0 écart.
  * `bench` bot30k 12 000 images sans rendu, même dossier cible, 7 passes alternées, même empreinte
    `a65ecd8aa6e38d4f` : médiane 23 485 images/s contre 23 561 pour main (−0,3 %, dans le bruit ; le chemin du jeu
    est inchangé). `render_check` bot30k 12 000 images : identique (`09850058246a1e2c`). `fuzz_couv` (bot30k, C10,
    3 000 images, graine 1) : sortie et carte de couverture identiques à main (résidu : 623 lectures, 0 depuis les
    états de base).
  * Validation au code de 9074ae0 : **309/309 VALID en mode par défaut et avec `DDSOM_ASSETS`** (assets réextraits
    par la branche), `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1`, `residue: 0` dans tous les journaux ; `bench` avec
    rendu : 3 999 images/s contre 3 993 pour main (médianes de 3 passes alternées) ; tests `ddsom-game --lib` (19 : `dispatch_handlers_are_game_natives` est
    devenu vrai par construction) ; `cargo clippy` sans avertissement, avec et sans `tools`, exemples et tests
    compris (et `ddsom`, aussi avec `--features original`) ; `cargo tree -p ddsom -e normal` sans `m68k`, `z80`,
    `ddsom-rc`.
  * Aucun code mort de l'original nouveau.

**L18. Documentation finale** (petit à moyen) — **fait** (branche `wt-l18-docs`, voir « Résultat »)
* `README.md` réécrit pour le joueur :
  * installer Rust ;
  * extraire ses assets (`extract <roms> <assets>`, avec les fichiers ROM attendus) ;
  * jouer (`ddsom --assets`, `play.sh`, `play.bat`) ;
  * touches (`touches.cfg`), manette, sauvegarde ;
  * ce qui n'est jamais versionné.
* La partie « rétro-ingénierie » du README actuel passe dans `docs/`.
* `docs/moteur.md` (nouveau, le guide du moteur) :
  * les couches de la section 2 ;
  * la boucle d'image, les tâches et l'aiguillage des objets ;
  * comment lire et écrire un champ (`record!`) ou une donnée (`data()`) ;
  * comment ajouter une table d'asset ;
  * comment valider : `validate_all.sh`, `DDSOM_ASSETS`, `fuzz_couv`, `DDSOM_CHECK`.
* `docs/README.md` (index) : documents de référence (assets, son, familles, créatures, niveaux, code mort) et
  historique (refactorisation, guides des lots, sans-rom, atlas, couverture). On n'efface rien.
* Fin : un lecteur qui n'a que le dépôt et ses ROMs joue en suivant le README, sans autre document ; les liens de
  `docs/` sont vérifiés par un script (`tools/check_links.py`).
* Résultat (branche `wt-l18-docs`, depuis main 56b84c0, après L20, L21 et L22) — **fait** :
  * **`README.md`** réécrit pour le joueur : ce qu'il faut (ROMs du jeu `ddsom` de MAME et `qsound.zip`, avec la
    liste des fichiers ; Rust ; sous Linux `build-essential pkg-config libudev-dev`, vérifié : `libudev-sys` est le
    seul paquet de construction lié à une bibliothèque du système, X11, OpenGL et PulseAudio sont chargés à
    l'exécution), préparer `rom/`, extraire (`extract rom assets`), jouer (`ddsom --assets assets`, `play.sh`,
    `play.bat`), fichiers écrits dans le dossier courant, touches et `touches.cfg` (noms anglais, anciens noms
    français lus puis réécrits, `DDSOM_TOUCHES=1`), manettes, sauvegarde rapide, EEPROM, échelle, filtres, plein
    écran, pack HD, ce qui n'est jamais versionné. La partie « rétro-ingénierie » passe dans
    [retro-ingenierie.md](retro-ingenierie.md) (matériel, chiffrement, Ghidra, MAME, premiers jalons).
  * **[moteur.md](moteur.md)** : crates, couches, une image du jeu (de `Game::step` à l'aiguillage), tâches et
    étapes (`world::tasks`, `steps!`), objets et `handlers!`, état (`record!`, `world::ram`), données (`data()`,
    `engine_table!`, résidu), ajouter une table d'asset, l'interface `Game`/`DrawList` et les façades, valider
    (`validate`, `validate_all.sh`, `DDSOM_ASSETS`, `render_check`, `bench`, `DDSOM_CHECK`, `fuzz_couv`, `no_rom`,
    clippy, tests, `measure_end.py`, `check_links.py`).
  * **[README.md](README.md)** (index de docs/) : guides, référence (données, jeu partie par partie), historique et
    méthode. Rien n'est effacé ; [reprise.md](reprise.md) donne l'état final et ce qui pourrait suivre.
  * **`tools/check_links.py`** : liens relatifs de `README.md` et `docs/**/*.md` (cibles et ancres, règle de GitHub ;
    liens dans le code ignorés) ; les 297 liens vers les images que les outils génèrent dans `assets/` (non
    versionné : [creatures.md](creatures.md), [niveaux.md](niveaux.md)) sont comptés à part. Résultat : **46 fichiers,
    0 lien cassé** (aucun n'était cassé avant L18 non plus).
  * **Essai d'un clone neuf** (`git clone` de la branche, ROMs remises en `ddsom.zip` et `qsound.zip`, sans
    `DDSOM_ROMS`), README suivi à la lettre : `unzip` dans `rom/` (22 fichiers) ; `extract rom assets` : 12 s avec la
    compilation, 420 fichiers, 37 Mo ; `cargo build -p ddsom` : 30 s ; `ddsom --assets assets` dans la fenêtre WSLg :
    `touches.cfg` créé, son ouvert, touches injectées par XTest (`5`, `1`, `F5`, `F9`, `F6`, `F7`, `Échap`) :
    sauvegarde et reprise à l'image 645, `nvram/ddsom.eeprom` et `sauvegardes/rapide.txt(.eeprom)` écrits ;
    `DDSOM_IDENTITY=900` : 0 pixel différent ; `./play.sh` (avec ROMs) : compile, joue, quitte par `Échap` ; `git
    status` propre ensuite (tout ce qui a été produit est ignoré). Seul écart avec le premier jet du README : la durée
    de l'extraction (une seconde, pas vingt), corrigée.
  * **Vérification finale du code de main** (L21 et L22 ne l'avaient pas faite ; L18 ne change aucun code du jeu) :
    **309/309 VALID en mode par défaut et avec `DDSOM_ASSETS`** (assets réextraits), `DDSOM_SCENES=1 DDSOM_COV=1
    DDSOM_LONG=1 DDSOM_JOBS=6`, `residue: 0` dans les 618 journaux ; `render_check` bot30k 12 000 images
    `09850058246a1e2c` dans les deux modes ; `cargo clippy -D warnings` propre (sept variantes, section 4) ; tests
    des crates du jeu ; `bench`, `measure_end.py` : section 4. Puis « Bilan ».

### Bonus

**L19. Bug de saisie du nom** — **fait** (branche `wt-l19-nom`)
* Petit, à tout moment. Périmètre : `scenarios/`, `hud/` ou `modes/` (le classement), `docs/modes.md`.
* Selon [reprise.md](reprise.md), ce bug existe dans d'autres révisions du jeu, pas dans la nôtre (Euro 960619).
* Travail :
  1. écrire un scénario qui entre un nom au classement en reproduisant la manipulation connue des autres révisions ;
  2. constater qu'il est VALID, donc que le port fait comme l'original ;
  3. documenter le bug, sa cause dans les autres révisions si on la trouve dans `docs/modes.md`, et le scénario.

  Corriger le bug ne se ferait que derrière une option (`DDSOM_*`), hors validation, sur décision de l'utilisateur.
  Le comportement par défaut reste celui de l'original. **Décidé le 2026-11-11 : le bug des anciennes révisions
  n'est pas reproduit, aucune option `DDSOM_*`.**
* Fin : le scénario est dans la validation (309/309), et la section de `docs/modes.md` est écrite.
* Résultat :
  * le bug (fiche KLOV du jeu) : avec un personnage dont le nom par défaut a six lettres, le bouton « nom par
    défaut » puis des lettres en série débordent du tampon du nom sur la fiche du joueur et son inventaire (« environ
    110 M »), le délai repartant à chaque lettre ; corrigé dans une révision ultérieure ;
  * cause (h) : dans notre révision, `0xc9d16` (nom par défaut) pose le verrou de saisie `+0x19` quand le nom a six
    lettres (`0xc9d5c-0xc9d62`) ; sans ce test, la longueur passerait de 6 à 7 et le test « = 6 » de `0xc9c90` ne
    serait plus jamais vrai (écritures jusqu'à `+0x217` du joueur). Invérifiable faute d'autres ROM ;
  * notre révision (Euro 960619) n'a pas le bug : le scénario `scenarios/modes/name_entry_overflow.txt` (voleur,
    MORIAH, 110 appuis sur M, effacement, M, fin) laisse le tampon à six lettres (`MORIAM`), et il est VALIDE ;
  * documenté dans [modes.md](modes.md) § 6 ; aucun code du jeu modifié. Validation complète 309/309 dans les deux
    modes.

### Présentation (décidé avec l'utilisateur le 2026-11-02)

La logique ne migre pas dans un moteur 2D (Godot, Unity, Bevy) : elle perdrait le comportement identique et le filet de
validation. Le moteur 2D n'est qu'une façade interchangeable.

**L20. Cœur de jeu en bibliothèque** — **fait** (branche `wt-l20-coeur`, voir « Résultat »)
* Après L15. Interface unique : `Game::step(inputs)`, puis `Game::frame() -> DrawList` (sprites avec leur numéro
  d'image de `frames.json`, couches de décor, palettes, effets) et `Game::sound() -> SoundCommands`. Aucune fenêtre,
  aucun matériel dans le cœur ; le modèle CPS2 (`cps2-gfx`) devient une façade de rendu parmi d'autres.
* Fin : `ddsom` ne parle au jeu que par cette interface ; validation 308/308 inchangée.
* Résultat (branche `wt-l20-coeur`, depuis main 4aa9a74) :
  * **L'interface** (`ddsom_game::core::interface`, réexportée `ddsom_game::{Game, Inputs}`) :
    ```rust
    impl Game {                                       // le jeu (Game<World>)
        pub fn new(contents: cps2::GameContents, eeprom: Option<&[u8]>) -> Game;
    }
    impl<B: Board> Game<B> {
        pub fn on(machine: cps2::Machine<B>) -> Self;  // machine déjà construite (code d'origine, outils)
        pub fn step(&mut self, inputs: Inputs);        // une image
        pub fn frame(&self) -> DrawList<'_>;           // l'image montrée par le dernier pas
        pub fn sound(&self) -> &[[i16; 2]];            // le son du dernier pas, mixé, à SAMPLE_RATE (24 kHz)
        pub fn eeprom(&self) -> Vec<u8>;               // à rendre à Game::new à la session suivante
        pub fn frame_count(&self) -> u64;
        pub fn machine(&self) -> &Machine<B>;          // outils seulement (empreintes, pokes des scénarios)
        pub fn machine_mut(&mut self) -> &mut Machine<B>;
    }
    pub use cps2::Inputs;                              // ports IN0, IN1, IN2 (actifs à 0, câblage MAME cps2_4p4b)
    pub const SAMPLE_RATE: f64;                        // qsound::SAMPLE_RATE
    pub const STOP: &str;                              // début du message d'un arrêt du jeu (panique)
    pub trait Board: cps2::Board { fn latched_frames(&self) -> &[SpriteFrame]; } // World, Cps2Bus (aucune image)
    ```
    `step` pose les entrées, vide le son de l'image précédente et lance `Machine::run_frame` sans rendu
    (`rendering = false`) ; le dessin est fait par l'appelant à partir de `frame`. L'état du jeu ne change pas : la
    RAM, le matériel et la table des tâches sont ceux que `validate` compare ; l'interface ne fait que les lire.
  * **`DrawList`** (`cps2_gfx::draw`) : une vue sans copie, décodée des registres CPS2, de l'image à dessiner :
    * `palette: Palette` : 0xc00 couleurs au format CPS2 (`color(i) -> u16`, `rgb(u16) -> [u8; 3]`) ; les sprites
      prennent les couleurs 0 à 0x1ff, les couches à partir de 0x200 ; fondus et flashs sont des changements de
      palette (les « effets ») ;
    * `layers: [Layer; 3]` : couches 1 à 3 (tuiles de 8, 16, 32 pixels, 64 x 64 tuiles), `tile(col, row) -> Tile`
      (`code`, `attr` : palette, miroirs), `origin(ligne) -> (x, y)` : coordonnées dans la carte du pixel de gauche
      d'une ligne d'écran (défilement et défilement par ligne de la couche 2 compris), `palette_base` ;
    * `order: [Option<usize>; 3]` : couches dessinées de l'arrière vers l'avant (couche coupée : `None`) ;
    * `sprite_masks: [u32; 8]` : masque de chaque priorité de sprite contre les couches ;
    * `sprites: Sprites` (`len`, `get(i) -> Sprite` : `code`, `palette`, miroirs, `priority`, blocs
      `columns` x `rows`, `cell(col, row) -> (code, x, y)` à l'écran) ;
    * `frames: &[SpriteFrame]` et `shown_frames()` : l'**identité des images** pour un pack HD (L22) : pour chaque
      image de `frames.json` dessinée, la première entrée et le nombre d'entrées de la liste de sprites, le
      **numéro d'image** (index de la table, puis les images hors table), l'ancre (`position(&list)` à l'écran) et le
      miroir ; la palette est celle des entrées (`palette(&list)`). Les tuiles des couches sont identifiées par
      (couche, `code`, palette).
    `DrawList::from_video(&VideoState)` décode un état vidéo (registres CPS-A/CPS-B, GFX RAM, table d'objets
    verrouillée, copie de la palette) ; `with_frames` ajoute les identités. Le moteur de rendu CPS2
    (`cps2_gfx::render(&DrawList, &TileAtlas)`) ne lit plus que la liste : c'est une façade parmi d'autres.
    `DrawList` vit dans `cps2-gfx` parce que c'est la plus basse crate que voient à la fois le jeu (par `cps2`) et
    toute façade (qui a besoin de l'atlas de tuiles, `cps2_gfx::TileAtlas`, pour dessiner les codes de tuiles) ; une
    crate à part n'aurait contenu que ces types.
  * **Image montrée et identités** : le matériel verrouille la table d'objets à la fin de chaque image et l'image
    suivante l'affiche. `cps2::Machine` garde donc la table montrée par la dernière image (`shown_objects`,
    échangée sans copie avec la table verrouillée, `Machine::shown() -> VideoState`) ; `Machine::render` (outils)
    garde son sens. Les identités suivent la même règle : `services::sprites::build` note, dans le banc de la RAM
    d'objets qu'il écrit, chaque image tirée de la table (`World::sprite_frames`, deux `Vec<SpriteFrame>`, hors de
    l'état comparé, jamais lu par le jeu, gardé avec la table des tâches quand les outils prêtent la RAM au bus) ;
    `Game::step` recopie celles du banc verrouillé et les montre à l'image suivante, comme la table d'objets.
    `Frames::find` (ddsom-data) rend le numéro avec l'image (mêmes recherches qu'avant).
  * **Son : échantillons mixés plutôt que `SoundCommands`.** Le pilote (séquenceur) et le mixeur QSound restent
    dans le cœur : leurs écritures (registres QSound, RAM partagée) font partie de l'état que `validate` compare, et
    une façade n'a qu'à jouer du PCM stéréo 16 bits à 24 kHz (ou le rééchantillonner). Des commandes de son
    obligeraient chaque façade à embarquer le pilote, le mixeur et la banque d'échantillons, sans rien gagner.
  * **Entrées** : `Inputs` est l'image des trois ports (comme l'écrit `scenario`) ; `keys.rs` les remplit.
    `Scenario` sépare `apply_inputs(f, &mut Inputs)` et `apply_pokes(f, machine)` (`apply` fait les deux, outils) ;
    `Recorder::record(Inputs)` et `Recorder::resumes(out, image, Inputs)`.
  * **`ddsom`** : `Game::new` (ou `Game::on` pour le code d'origine, `--features original`), puis à chaque image
    `step`, `cps2_gfx::render(&game.frame(), &atlas)`, `sound()` ; `eeprom()` à la sortie. Sauvegarde rapide :
    `save::Session` rejoue les entrées d'un fichier sur un `Game` neuf (les pokes d'un scénario ne sont pas rejoués :
    l'application n'en écrit jamais). `ddsom` ne dépend plus de `ddsom-assets`, `ddsom-data` ni `qsound` ;
    `cargo tree -p ddsom -e normal` : ni `m68k`, ni `z80`, ni `ddsom-rc`. Le binaire démarre et joue (fenêtre
    WSLg, avec ROMs et avec `--assets`, quelques secondes chacun, sans arrêt).
  * Outils : `bench` et `render_check` passent par `Game` (et le rendu CPS2 de `frame`) ; `render_check` vérifie en
    plus les identités : bot30k 12 000 images, **122 051 images montrées, 786 886 sprites, 0 écart** (nombre
    d'entrées et codes des morceaux de `frames.json`). `vrender`, `render_bench`, `creatures`, `level_maps` et le test
    `mame_snapshots` passent par `DrawList::from_video`.
  * `render_check` bot30k 12 000 images : identique à main (`09850058246a1e2c`, fichier par image identique), aussi
    avec `DDSOM_ASSETS`.
  * `bench` bot30k 12 000 images, même dossier cible, 7 passes alternées avec main 4aa9a74, même empreinte
    `a65ecd8aa6e38d4f` : médianes **sans rendu 24 962 images/s contre 23 517 (+6 %)**, **avec rendu 4 049 contre
    4 006 (+1 %)**. Le gain sans rendu vient de `step`, qui vide le son de chaque image (le `bench` de main laissait
    grossir le vecteur d'échantillons) ; le coût des identités (une poussée par image dessinée) ne se voit pas.
  * `DDSOM_CHECK=1` bot30k 2 000 images : 21 444 appels comparés, 0 écart, VALID.
  * `measure_end.py` : 65 modules, aucune composante entre couches, 0 arête vers une couche plus haute.
  * Validation (code de f29f3e0 ; le mode assets avec en plus le renommage `Game::frames` → `frame_count`, jamais
    appelé) : **309/309 VALID en mode par défaut et avec `DDSOM_ASSETS`** (assets réextraits par la branche),
    `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1 DDSOM_JOBS=6`, `residue: 0` dans tous les journaux ; tests
    `ddsom-game --lib` (19), `cps2-gfx` ; `cargo clippy` sans avertissement (`ddsom-game` avec et sans `tools`,
    `ddsom` avec et sans `--features original`, `cps2-gfx`).
  * Aucun code mort nouveau.

**L21. Façade de présentation** — **fait** (branche `wt-l21-facade`, voir « Résultat »)
* **Décidé le 2026-11-11 : façade minimale en macroquad** (pas Bevy).
* Rendu à échelle variable, filtres (CRT…), mixage, manettes, plein écran, build web.
* Fin : le jeu se joue dans la façade, image identique au rendu CPS2 à l'échelle 1.
* Résultat (branche `wt-l21-facade`, depuis main b402539) :
  * **minifb est remplacé** : une seule façade, `ddsom` en macroquad 0.4 (sans la fonctionnalité `audio`). Rien ne
    manquait (fenêtre, plein écran, clavier, texture, shader), garder minifb derrière une option aurait doublé
    `main.rs` et `keys.rs` pour rien. macroquad charge X11/GL dynamiquement : aucune dépendance de compilation sous
    Linux.
  * **Image** (`view.rs`) : `cps2_gfx::render(&game.frame(), &atlas)` (384 x 224, RGB) est chargé dans une texture
    RGBA filtrée au plus proche, dessinée centrée sur fond noir. Échelle (`F6`) : plus grand multiple entier qui tient
    (défaut) ou ajustée à la fenêtre (proportions gardées) ; fenêtre redimensionnable, 2x au départ. Filtres (`F7`,
    aucun par défaut) : lissé (bilinéaire) et CRT (shader GLSL 100 de lignes de balayage : la moitié basse de chaque
    ligne de l'image s'assombrit ; à l'échelle 1 seul le milieu de la ligne est lu, l'image ne change pas). Plein
    écran `F11` (`set_fullscreen`, souris cachée). Pour L22 : `Screen::show` prend une image ; un pack HD remplira
    d'autres textures à partir de `DrawList::shown_frames()`, le reste (placement, filtres) ne change pas.
  * **Identité à l'échelle 1, vérifiée en relisant la fenêtre** : `DDSOM_IDENTITY=N ddsom ...` ouvre une fenêtre de
    384 x 224 (échelle 1, sans filtre), relit après chaque image du jeu le tampon de la fenêtre (`get_screen_data`,
    après le dessin, avant l'échange) et le compare pixel à pixel à la sortie de `cps2_gfx::render`, puis quitte.
    WSLg (OpenGL logiciel llvmpipe) : **1 500 images, 0 pixel différent, avec ROMs et avec `--assets`** (aussi 896 et
    600 images). Contrôle négatif : la même comparaison sans retourner les lignes de la relecture donne 29 484 010
    pixels différents sur 900 images : la vérification n'est pas creuse. Par construction aussi : texture RGBA8 sans
    sRGB, filtre au plus proche, rectangle à coordonnées entières de la taille de la texture, couleur blanche (aucune
    modulation), alpha 255.
  * **Cadence** : la boucle tourne au rythme de l'affichage ; le jeu avance de `FRAME_HZ` (59,637 Hz, la cadence
    CPS2 de l'ancienne application) images par seconde de l'horloge (`Clock` : 0, 1 ou 2 pas par image affichée,
    resynchronisé après une pause, une reprise ou un retard de plus de 200 ms ; avance rapide `Tab` : 8 pas par image
    affichée, sans son). Seule la dernière image du jeu est rendue. Sans synchronisation verticale (WSLg), la boucle
    dort jusqu'à l'image suivante du jeu quand rien n'est dû (sinon ~900 tours/s) : ~2 tours par image du jeu.
  * **Son** (`audio.rs`) : le mixage reste dans le cœur (`Game::sound()`, 24 038 Hz) ; macroquad ne joue que des sons
    entiers (pas de flux PCM), on garde donc les sorties de l'ancienne application, chargées à l'exécution
    (PulseAudio simple sous Linux, waveOut sous Windows : aucune dépendance de compilation ; cpal aurait demandé
    `libasound2-dev` et sa fonctionnalité `pulseaudio` sous WSLg, qui n'a pas d'ALSA). Nouveau : une file entre le jeu
    et le fil de sortie, qui **tire** les échantillons à 48 kHz (blocs de 10 ms, 30 ms tenus par le serveur PulseAudio ;
    3 blocs de 20 ms pour waveOut), **rééchantillonnés** linéairement ; le rapport est corrigé d'au plus 0,5 %
    (inaudible) selon le remplissage de la file, ce qui tient la latence près de 40 ms (+ ~30 ms du périphérique) au
    lieu de dériver entre l'horloge du jeu et celle de la carte son (avant : 24 000 Hz pour 24 038, file qui
    grossissait puis perdait des blocs). La file s'amorce à 40 ms (au départ et après s'être vidée), se coupe à 200 ms
    après un arrêt. Test `latency_stays_near_the_target` (horloges à ±0,2 %). L'ouverture se fait dans le fil de
    sortie : le jeu n'attend plus le serveur son (pendant les essais, le serveur PulseAudio de WSLg a refusé puis
    bloqué les connexions quelques minutes, d'où ce changement ; ouvert normalement avant et après).
  * **Manettes** (`gamepad.rs`) : gilrs 0.11 (evdev sous Linux, Windows.Gaming.Input/XInput, IOKit), deux manettes,
    disposition Xbox (`Pad1A` bas, `Pad1B` droite, `Pad1X` gauche, `Pad1Y` haut, `LB`, `RB`, `Start`, `Back`, croix ou
    stick gauche `Pad1Up`...). Gain : des manettes sous Linux et macOS (avant : XInput sous Windows seulement). Coût :
    `libudev-dev` pour compiler sous Linux (play.sh, README). Pas testé avec une vraie manette (aucune sous WSL).
  * **`touches.cfg` en anglais** : actions `p1.up`, `p1.button1`, `p1.coin`, `p2.start`, `test`, `service`, `pause`,
    `save`, `load`, `fast`, `fullscreen`, `scale`, `filter`, `quit` ; touches aux noms de l'ancienne fenêtre minifb
    (`LeftCtrl`, `Key1`, `NumPad0`... : les fichiers existants restent valables ; les noms macroquad `LeftControl`,
    `Kp0` sont aussi lus) ; boutons `Pad1Up`. Les anciens noms (`j1.haut`, `sauver`, `plein_ecran`, `Pad1Haut`...)
    sont lus, et un fichier qui en contient est réécrit en anglais (les commentaires de l'utilisateur sont perdus).
    Test `former_names_are_read_and_rewritten_in_english`.
  * **Inchangé** : arguments (`[roms]`, `--assets [dir]`), `--features original` (`DDSOM_ORIGINE`, `DDSOM_INTERP`),
    sauvegarde rapide `F5`/`F9` (`save.rs`), `DDSOM_ENREGISTRE`, EEPROM `nvram/ddsom.eeprom` écrite à la sortie
    (`Échap` ou fermeture de la fenêtre : `prevent_quit`), arrêt propre sur une routine manquante. Commandes de
    `play.sh`/`play.bat` inchangées (commentaires mis à jour).
  * **Build web : compile, ne joue pas.** `cargo build --release -p ddsom --target wasm32-unknown-unknown` passe (tout
    le graphe : jeu, données, extract ; clippy sans avertissement ; sans son ni manettes : gilrs et les sorties son
    sont hors wasm). Mais le jeu lit ses données par `std::fs` (13 appels dans `ddsom-data` `json.rs`/`palettes.rs`,
    `ddsom-assets`, `cps2::Contents`, `ddsom-sound` `region.rs`, plus le résidu lu à la demande) : sur le web, il
    faudrait une source de fichiers abstraite dans quatre crates, un préchargement asynchrone (`load_file`) d'environ
    37 Mo d'assets (420 fichiers), une page `index.html` avec le chargeur JS de miniquad, l'EEPROM et les sauvegardes
    hors système de fichiers, et un greffon WebAudio pour le son (macroquad ne diffuse pas de PCM). Ce n'est pas
    « un peu de code » : **non fait, à décider**.
  * Lancements (WSLg) : avec ROMs et avec `--assets` (assets réextraits), quelques dizaines de secondes, sans arrêt,
    sortie son ouverte ; touches injectées par XTest (pièce, Start, `F5`, `F9`, `F6`, `F7`, `F11`, `Échap`) :
    sauvegarde et reprise (image 1477), plein écran 1920 x 1200 ajusté avec le filtre CRT, EEPROM écrite à la sortie.
  * Validation : le jeu (`ddsom-game`, `cps2`, `cps2-gfx`) n'est pas touché. `render_check` bot30k 12 000 images :
    **`09850058246a1e2c`** (identités : 122 051 images, 0 écart), aussi avec `DDSOM_ASSETS` ; `validate` VALID sur
    `modes/name_entry_overflow`, `boss/boss0_poke`, `boss/boss0_poke_mort`, `scene/scene_01`, et avec `DDSOM_ASSETS`
    `name_entry_overflow` et `scene_01` ; pas de validation complète (rien du jeu n'a changé). `cargo tree -p ddsom -e normal` : ni `m68k`, ni `z80`, ni
    `ddsom-rc`, ni `minifb`. `cargo clippy -p ddsom` sans avertissement : `--all-targets`, `--features original`,
    `--target x86_64-pc-windows-gnu`, `--target wasm32-unknown-unknown`. Tests `ddsom` : 3.

**L22. Pack HD (upgrade graphique)** — **fait** (branche `wt-l22-hd`, voir « Résultat » ; guide : [pack-hd.md](pack-hd.md))
* Outil d'export : chaque image de sprite assemblée et colorée (palette principale) en PNG RGBA, chaque niveau en une
  grande image continue.
* Retour aux index : l'image retouchée (upscaler pixel art, Gemini…) est requantifiée sur les 16 index de sa palette
  d'origine → PNG HD indexé ; variantes de palette, flashs et fondus continuent de marcher.
* Chargement d'un dossier `hd/` (`frames/NNNNN.png`, `levels/NN.png`) par la façade, repli sur l'original si absent.
  Jamais versionné (dérivé des graphismes de Capcom).
* Fin : un pack partiel s'affiche, la logique et la validation ne changent pas.
* Résultat (branche `wt-l22-hd`, depuis main 567b93c) :
  * **Nouvelle crate `ddsom-hd`** (`port/ddsom-hd`) : format du pack (`FrameMeta`, `LevelMeta`, PNG indexés),
    retour aux index (`index_frame`, `index_level`), chargement (`load(dir) -> HdPack`), rendu HD
    (`render_hd(&DrawList, &TileAtlas, &HdPack)`), et l'outil `hd-pack` (fonctionnalité `tool`, par défaut) :
    `export`, `upscale [--scale2x]`, `index`, `check`. La façade prend `ddsom-hd` sans `tool` (ni jeu en double, ni
    `extract` en plus de ce qu'elle a déjà). Le rendu HD est dans `ddsom-hd` et non dans `cps2-gfx` pour ne pas
    relancer la compilation de `ddsom-rc` à chaque retouche ; `cps2-gfx` n'a reçu que `render::tile_pens` (rangée d'une
    tuile de couche, extraite de `tile_row`, même calcul).
  * **Export** : les images de sprites vues en jouant des scénarios (`--run <scénario> <images>`, ~1 s pour bot30k
    12 000 images : 604 images), assemblées depuis `frames.json` et l'atlas, non retournées, avec les couleurs **le
    plus souvent** à l'écran (un premier essai par « la plus claire » prenait des flashs blancs) ; `--all` ajoute les
    21 887 en gris. Niveaux : décor complet des couches 2 et 3 de chaque scène depuis `levels.json` (une carte partagée
    écrite une fois : 146 images), palettes des pages 2 et 3 de la scène (`scene_palettes.json` + `palettes.png`).
    Écart au plan : **une image par scène et par couche** (`levels/NN_scroll2.png`, `NN_scroll3.png`) et non
    `levels/NN.png` : les deux couches défilent à des vitesses différentes (parallaxe) et se recouvrent, une image
    fusionnée ne se redécoupe pas.
  * **Identité des images** : une image de sprite est identifiée par son numéro (`SpriteFrame` de L20) ; ses pixels
    valent `16 × emplacement + encre`, un emplacement par palette distincte de ses morceaux, qui prend à l'exécution
    la palette de l'entrée du morceau désigné (palette imposée par l'objet comprise). Une tuile de décor est
    identifiée par son numéro de graphisme (`tile_key` : couche 3 `code & 0x3fff`, couche 1 avec la parité de la
    colonne), stockée non retournée : la carte du niveau ne sert qu'à l'édition, le rendu n'a pas besoin de savoir
    où la caméra est dans le niveau (le registre de défilement ne le dit pas : la carte de 64 × 64 tuiles boucle).
  * **Retour aux index** : couleur la plus proche (distance RVB) parmi les 15 de la palette de la tuile, ou de toutes
    les palettes de l'image de sprite ; alpha < 128 transparent ; échelle entière vérifiée. Les palettes contiennent
    parfois deux fois la même couleur : le premier pack « identité » différait de 4,2 millions de pixels sur 3 000
    images (une autre palette les distingue, ex. l'écran du trésor) ; l'export écrit donc aussi les encres d'origine
    (`.pens.png`), qui départagent les ex aequo.
  * **Rendu HD** : celui de `cps2_gfx::render` pixel à pixel à l'échelle `S` du pack (couches dans l'ordre et marques
    de couche, sprites de la dernière entrée à la première avec `sprite_masks` et la marque 31) ; une image du pack
    est tracée d'un bloc quand la boucle atteint la dernière entrée de l'image, les autres entrées sautées ; ce qui
    n'est pas dans le pack est répété `S × S`. Les morceaux bouclent sur l'espace des sprites de 1024 : une image à
    cheval sur son bord est tracée des deux côtés (la herse de l'écran titre manquait avant). Couleurs par la palette
    de l'image en cours : variantes, flashs, fondus.
  * **Façade** : `ddsom` charge `hd/` (dossier courant) ou `DDSOM_HD=<dossier>` s'il existe, `F8` (`hd` dans
    `touches.cfg`) coupe et remet le pack, même en pause ; `view::Screen::show(image, échelle)` suit la taille de la
    texture, placement et filtres inchangés. `DDSOM_IDENTITY` ignore le pack.
  * **Vérifications** :
    * pack « identité » complet (export bot30k, `upscale 2`, `index` : 604 images, 18 586 tuiles) : `hd-pack check`
      bot30k 12 000 images : 122 051 images de sprites remplacées, **0 pixel différent** de l'original agrandi ×2 (ce
      qui vérifie ancres, miroirs, palettes à l'exécution, priorités et défilement) ;
    * pack partiel ×2 au plus proche (53 images 00000-00099 et `levels/00_scroll2`, 551 tuiles) : 4 197 images
      remplacées, 0 pixel différent ; pack partiel ×4 Scale2x (mêmes images) : guerrier et rochers adoucis, le reste
      (interface, objets, ombres) d'origine ; les tuiles HD prennent la palette du coucher de soleil de l'écran
      « Game Over » (image 10 000), autre palette que celle de l'export ;
    * fenêtre WSLg (`ddsom rom`, reprise `F9` à l'image 4400, plein écran 1920 × 1200, `F8`) : pack ×4 affiché, `F8`
      rend l'image d'origine ; `DDSOM_IDENTITY=1500` sans `hd/` : **0 pixel différent** ;
    * temps du rendu HD : 1,7 ms par image à ×2, 6,7 ms à ×4 ;
    * jeu inchangé : `render_check` bot30k 12 000 images **`09850058246a1e2c`** (identités : 122 051, 0 écart) ;
      `validate` VALID sur `scene/scene_01` et `modes/name_entry_overflow` ; pas de validation complète (seul
      `cps2-gfx` est touché, par une extraction de fonction que `render_check` couvre ; `validate` ne compare pas le
      rendu) ;
    * `cargo clippy` sans avertissement : `cps2-gfx`, `ddsom-hd` (avec et sans `tool`), `ddsom` (`--all-targets`,
      `--target wasm32-unknown-unknown`) ; `--features original` non relancé (code non touché) ; tests `ddsom-hd` (2),
      `cps2-gfx`, `ddsom` ; `cargo tree -p ddsom -e normal` : ni `m68k`, ni `z80`, ni `ddsom-rc`.
  * **Limites** (voir [pack-hd.md](pack-hd.md)) : une tuile retouchée vaut partout (première image de niveau gagnante
    en cas de conflit) ; couche 1 (texte, interface) non exportée ; sprites hors table des images (ombres…) non
    remplacés ; une image HD reste dans le cadre et les 16 couleurs de l'original ; positions par pas de `S` pixels HD ;
    une seule échelle par pack (la plus grande) ; images jamais vues dans les scénarios exportées en gris ; mode
    `--assets` de l'outil non essayé (même code, atlas identique).

## 4. Fin du projet : critères mesurables

Mesures finales (L18, code de main 56b84c0, inchangé par L18) : validation et `render_check` dans les deux modes
(assets réextraits), `tools/measure_end.py`, `bench` (meilleur de 3, machine au repos).

| Critère | Aujourd'hui (fin, L18, main 56b84c0) | Cible | Atteint |
|---|---:|---:|:---:|
| Validation (deux modes, `DDSOM_LONG=1`) | **309/309** dans les deux modes | 308/308 (plus les nouveaux scénarios) | oui |
| Accesseurs ROM `data*`/`rom*`/`prog*` dans le jeu | 0 | 0 | oui |
| Zones brutes `natives.json` lues par le jeu | 0 (seul le résidu, jamais consulté) | 0 (toutes nommées) | oui |
| Lectures de `Residue` (308 scénarios) | 0 (309 scénarios, deux modes : « residue: 0 » dans les 618 journaux) | 0 | oui |
| Décalages de champs littéraux | 0 | 0 hors des `record!` | oui |
| `A5 + littéral` | 3 (2 `engine`, 1 `screens`) | 0 hors de `world::ram` | presque |
| Littéraux GFX RAM et registres CPS dans le jeu | 0 + 24 (motif `0x90xxxx` / `0x80xxxx` hors `hw/` et `tools/`, commentaires retirés : constantes nommées des ports d'E/S, de l'EEPROM et des registres CPS dans `core/startup`, `core/vblank`, `services/eeprom`, `services/inputs`, `screens/service/memory`, `screens/modes/boot`) | 0 hors de `hw::video` | GFX RAM oui ; ports et registres non |
| Sons par adresse | 0 | 0 | oui |
| Registre de natives dans le jeu | 0 (341 gestionnaires des tables d'aiguillage, données du jeu) | 0 (outils seulement) | oui |
| Fichiers de plus de 1 500 lignes (hors `tools/`) | 0 (plus gros : `actors/player/spawn.rs`, 916) | 0 | oui |
| Plus grande composante fortement connexe des modules | 9 (dans `world`), aucune entre couches, 0 arête vers une couche plus haute (65 modules) | 1 module par couche, plus `core` | entre couches oui ; dans les couches non (cycles tolérés) |
| `rebuild` dans le jeu | non | non | oui |
| `bench` bot30k, avec et sans rendu | 4 067 et 25 255 images/s (empreinte `a65ecd8aa6e38d4f`) | au moins 2 500 et 14 000 images/s | oui |
| `ddsom` ne parle au jeu que par `Game` | oui (L20) | oui | oui |
| Image de la façade à l'échelle 1 identique au rendu CPS2 | oui (`DDSOM_IDENTITY=900` : 0 pixel, aussi dans le clone neuf de L18) | oui | oui |
| Pack HD « identité » ×2 identique au rendu CPS2 agrandi | oui (L22, `hd-pack check`, 0 pixel) | oui | oui |
| `render_check` bot30k 12 000 images | `09850058246a1e2c` dans les deux modes (122 051 identités, 0 écart) | inchangé | oui |
| `cargo clippy` | 0 avertissement (`--workspace --all-targets` ; `ddsom-game` avec et sans `tools` ; `ddsom` avec et sans `--features original` ; `ddsom-hd` avec et sans `tool` ; `-D warnings`) | 0 | oui |
| Un joueur suit le README dans un clone neuf | oui (L18 : construction, extraction, jeu, `play.sh`) | oui | oui |
| Liens de la documentation | 0 cassé (`tools/check_links.py`, 46 fichiers) | 0 | oui |

Autres mesures de `measure_end.py` : 600 fichiers, 173 531 lignes dans le jeu hors `tools/` (`ddsom-game` :
180 340 lignes de source dont 6 809 dans `tools/`, 16 916 d'exemples et tests) ; 1 462 `rd*`/`wr*` bruts (accès
nommés à la RAM et au matériel par constantes) ; 402 constantes d'adresses ROM (clés de tables lues par adresse :
SET 194, joueurs 141, moteur 52, écrans 15). Tests : `ddsom-game --lib` 19, `ddsom` 3, `ddsom-hd` 2, `extract`,
`ddsom-data`, `ddsom-sound`, `qsound`, `cps2-gfx` (dont `mame_snapshots`) : tous passent.

## Bilan

**Le but est atteint.** Le jeu se joue en Rust pur, sans 68000 et sans ROM, depuis des assets lisibles (PNG, WAV,
JSON) que l'utilisateur extrait de ses ROMs ; il se comporte strictement comme l'original, vérifié image par image sur
309 scénarios dans les deux modes ; le moteur est en six couches sans arête vers le haut, ses données sont typées et
nommées, et aucun asset dérivé des ROMs n'est versionné. Un nouveau venu joue en suivant le README et trouve le moteur
expliqué dans [moteur.md](moteur.md).

Critères atteints : tous ceux de la section 4, sauf trois, partiellement :

* `A5 + littéral` : 3 restent (contre 785 au départ) ;
* registres CPS et ports d'E/S : 24 littéraux, tous des constantes nommées dans les modules qui pilotent ce matériel
  (démarrage, VBLANK, EEPROM, entrées, test de la RAM), pas dans `hw::video` (les 407 littéraux de la GFX RAM du
  départ sont à 0) ;
* composantes fortement connexes : aucune entre couches, mais des cycles restent à l'intérieur de `world` (9
  modules), `actors`, `core`, `screens`, `services` ; tolérés par le plan (L15).

Restent ouverts, par décision ou faute de matériel :

* **build web** : compile (`wasm32-unknown-unknown`), ne joue pas ; il faudrait une source de fichiers abstraite,
  un préchargement asynchrone des assets, l'EEPROM et les sauvegardes hors système de fichiers et un greffon WebAudio
  (« L21 ») — **reporté, à décider** ;
* **`reste.json` gardé comme assurance** : jamais consulté sur les 309 scénarios, mais lu sur des états perturbés de
  `fuzz_couv` (« L14 ») ;
* **crate `ddsom-tools` abandonnée** (environ 230 éléments `pub(crate)` à ouvrir, « L17 ») : les outils restent dans
  `ddsom-game` derrière la fonctionnalité `tools` ;
* **jamais testés** : une vraie manette (gilrs ; aucune sous WSL) et la sortie son waveOut sous Windows ;
* **L16** (état Rust plus projection) : facultatif, non fait ;
* pack HD : couche 1 et sprites hors table des images non remplaçables ([pack-hd.md](pack-hd.md)).

Suite possible : [reprise.md](reprise.md), « Ce qui pourrait suivre ».
