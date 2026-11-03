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
  ddsom/          application : fenêtre, clavier/manette (keys.rs, gamepad.rs), sortie audio, sauvegarde EEPROM
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

| Couche | Modules (d'après les fichiers d'aujourd'hui) | Rôle |
|---|---|---|
| `data` (crate `ddsom-data`) | — | tables immuables : sprites, animations, décors, palettes, terrain, textes, tables de jeu, scripts |
| `hw` | `video` (tilemaps, sprites, registres CPS, palette), `sound` (file de commandes vers le pilote), `inputs`, `eeprom` | interface avec le matériel modélisé ; aucune logique de jeu |
| `world` | `world::ram` (RAM de travail), `obj` (`Obj`, `Link`, enregistrements typés), `objects/` (pools, listes, animation, mouvement, rang, visibilité), `terrain`, `rng`, `placement`, `events` | état du monde et moteur d'objets |
| `services` | `palette`, `text`, `sprites`, `scroll`, `camera_modes`, `ai`, `ai_aids`, `aids`, `effects_aids`, `combat` | services de jeu appelés par les acteurs |
| `actors` | `families/` (ennemis, boss, effets, SET, ESHL...), `player/` | gestionnaires d'objets et joueurs |
| `screens` | `modes/`, `scene/`, `hud/`, `service/`, `credits*`, `popups` | écrans et séquences |
| `core` | `kernel`, `tasks/`, `dispatch`, `vblank`, `startup`, `frame` (ex-`no68k`), `game`, `config` | ordonnanceur, tâches, boucle d'image |

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
* **L'interface du rendu** est `cps2_gfx::render(&VideoState, &TileAtlas) -> Frame`. Le `TileAtlas` est fait des
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

### Entrées et plateforme

`ddsom` (`main.rs`, `keys.rs`, `gamepad.rs`, `audio.rs`, `save.rs`) reste la seule crate de plateforme. Elle remplit
`hw::inputs` et lit `Frame` et les échantillons. Son ouverture devient
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
  * Validation : VALIDATION_PLACEHOLDER

**L14. Le jeu sans image ROM** (moyen)
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
* Décision laissée à l'utilisateur : retirer `reste.json` des assets une fois `Residue` à zéro, ou le garder comme
  assurance contre les chemins jamais joués.

**L15. RAM du jeu hors du bus, modules en couches** (moyen à grand, en deux commits validés)
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

**L16. État Rust plus projection** (facultatif, petit par bloc). Voir la section 2. Un bloc par commit, avec la
preuve qu'aucune lecture d'origine ne sort du module (trace des lectures par adresse sur les 308 scénarios).

**L17. Outils hors du jeu** (facultatif, moyen)
* `ddsom_game::tools` (7 018 lignes, 70 fichiers) et les exemples passent dans `ddsom-tools`. Le jeu exporte les
  points d'accroche nécessaires (`check_handler`, `natives` des outils) derrière une fonctionnalité `hooks`.
* Gain : `ddsom-game` n'a plus de `#[cfg(feature = "tools")]`.
* Risque : beaucoup de `pub(crate)` à ouvrir. Si la liste dépasse une centaine d'éléments, on garde le module en
  l'état.

**L18. Documentation finale** (petit à moyen)
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

### Bonus

**L19. Bug de saisie du nom** — **fait** (branche `wt-l19-nom`)
* Petit, à tout moment. Périmètre : `scenarios/`, `hud/` ou `modes/` (le classement), `docs/modes.md`.
* Selon [reprise.md](reprise.md), ce bug existe dans d'autres révisions du jeu, pas dans la nôtre (Euro 960619).
* Travail :
  1. écrire un scénario qui entre un nom au classement en reproduisant la manipulation connue des autres révisions ;
  2. constater qu'il est VALID, donc que le port fait comme l'original ;
  3. documenter le bug, sa cause dans les autres révisions si on la trouve dans `docs/modes.md`, et le scénario.

  Corriger le bug ne se ferait que derrière une option (`DDSOM_*`), hors validation, sur décision de l'utilisateur.
  Le comportement par défaut reste celui de l'original.
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

**L20. Cœur de jeu en bibliothèque**
* Après L15. Interface unique : `Game::step(inputs)`, puis `Game::frame() -> DrawList` (sprites avec leur numéro
  d'image de `frames.json`, couches de décor, palettes, effets) et `Game::sound() -> SoundCommands`. Aucune fenêtre,
  aucun matériel dans le cœur ; le modèle CPS2 (`cps2-gfx`) devient une façade de rendu parmi d'autres.
* Fin : `ddsom` ne parle au jeu que par cette interface ; validation 308/308 inchangée.

**L21. Façade de présentation**
* Choix à confirmer avec l'utilisateur : Bevy (Rust natif, sprites, shaders, audio, wasm) ou wgpu/macroquad (minimal).
* Rendu à échelle variable, filtres (CRT…), mixage, manettes, plein écran, build web.
* Fin : le jeu se joue dans la façade, image identique au rendu CPS2 à l'échelle 1.

**L22. Pack HD (upgrade graphique)**
* Outil d'export : chaque image de sprite assemblée et colorée (palette principale) en PNG RGBA, chaque niveau en une
  grande image continue.
* Retour aux index : l'image retouchée (upscaler pixel art, Gemini…) est requantifiée sur les 16 index de sa palette
  d'origine → PNG HD indexé ; variantes de palette, flashs et fondus continuent de marcher.
* Chargement d'un dossier `hd/` (`frames/NNNNN.png`, `levels/NN.png`) par la façade, repli sur l'original si absent.
  Jamais versionné (dérivé des graphismes de Capcom).
* Fin : un pack partiel s'affiche, la logique et la validation ne changent pas.

## 4. Fin du projet : critères mesurables

| Critère | Aujourd'hui | Cible |
|---|---:|---:|
| Validation (deux modes, `DDSOM_LONG=1`) | 308/308 | 308/308 (plus les nouveaux scénarios) |
| Accesseurs ROM `data*`/`rom*`/`prog*` dans le jeu | 1 699 | 0 |
| Zones brutes `natives.json` lues par le jeu | 2 963 | 0 (toutes nommées) |
| Lectures de `Residue` (308 scénarios) | — | 0 |
| Décalages de champs littéraux | ~11 250 | 0 hors des `record!` |
| `A5 + littéral` | ~785 | 0 hors de `world::ram` |
| Littéraux GFX RAM et registres CPS dans le jeu | 407 + 72 | 0 hors de `hw::video` |
| Sons par adresse | 317 | 0 |
| Registre de natives dans le jeu | 481 | 0 (outils seulement) |
| Fichiers de plus de 1 500 lignes (hors `tools/`) | 14 | 0 |
| Plus grande composante fortement connexe des modules | 37 | 1 module par couche, plus `core` |
| `rebuild` dans le jeu | oui | non |
| `bench` bot30k, avec et sans rendu | 1 862 et 11 408 images/s | au moins 2 500 et 14 000 images/s |
