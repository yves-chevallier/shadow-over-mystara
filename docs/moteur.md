# Le moteur : guide

Ce guide explique comment le moteur Rust de ddsom est fait et comment y travailler : où est quoi, comment une image
du jeu se déroule, comment lire et écrire l'état et les données, et comment vérifier qu'un changement garde le
comportement de l'original. Ce n'est pas un inventaire : les détails sont dans les documents de référence
([README.md](README.md)) et dans les commentaires du code. Pour jouer : [le README du dépôt](../README.md).

Deux principes guident tout le reste :

* **Le comportement est celui de l'original**, glitches et bugs compris. Le jeu est rejoué image par image contre le
  code 68000 d'origine (la « machine de référence ») et l'état doit rester identique ; un écart est une erreur, même
  s'il corrige un bug. Les glitches gardés sont marqués `// Original glitch:` dans le code, le code mort de
  l'original est consigné dans [code-mort.md](code-mort.md).
* **L'état reste dans la RAM de travail** (64 Ko, comme l'original), lue et écrite par des accès nommés. C'est ce que
  `validate` compare ; la RAM tient aussi les glitches qui exploitent sa disposition (voir [fin.md](fin.md),
  « État du jeu »).

## 1. Les crates

Toutes dans `port/` (un seul espace de travail cargo, `port/Cargo.toml`).

| Crate | Rôle |
|---|---|
| `ddsom` | l'application : façade macroquad (fenêtre, filtres, clavier, manettes, son, sauvegardes) |
| `ddsom-game` | le moteur du jeu, sans 68000 ni ROM ; outils (`validate`, `fuzz_*`...) derrière la fonctionnalité `tools` |
| `ddsom-data` | les données typées (`GameData`) et leur chargement depuis `assets/` |
| `ddsom-sound` | le pilote son (séquenceur réécrit du programme Z80) |
| `qsound` | le mixeur QSound |
| `cps2-gfx` | la liste d'affichage (`DrawList`) et le rendu CPS2 (sprites, couches, priorités, palettes) |
| `cps2` | la machine : matériel modélisé (vidéo, entrées, EEPROM, son), et la machine de référence avec 68000 et Z80 |
| `extract` | ROMs → assets (`extract`), et ROMs → `GameData` en mémoire (mode avec ROMs) |
| `ddsom-assets` | chargement des graphismes et des échantillons depuis les PNG et WAV |
| `ddsom-hd` | le pack HD et l'outil `hd-pack` ([pack-hd.md](pack-hd.md)) |
| `m68k`, `z80`, `recomp`, `ddsom-rc`, `vrender` | référence et outils seulement : jamais liés par `ddsom` |

`ddsom-rc` est le code 68000 du jeu recompilé en Rust **à la compilation, à partir de vos ROMs** (variable
`DDSOM_ROMS`, sinon `rom/`). Il ne sert qu'à la machine de référence des outils. Sa génération prend de 12 à 40
minutes ; elle est relancée dès que la chaîne `DDSOM_ROMS` change (même un chemin équivalent écrit autrement) :
exportez toujours exactement la même. `cargo tree -p ddsom -e normal` ne doit contenir ni `m68k`, ni `z80`, ni
`ddsom-rc`.

## 2. Les couches de `ddsom-game`

`port/ddsom-game/src/` a six couches ; chacune n'appelle que celles du dessous (`tools/measure_end.py` le vérifie :
« aucune composante entre couches, 0 arête vers une couche plus haute »).

| Couche | Contenu | Rôle |
|---|---|---|
| `hw` | `mem` (accès `rd*`/`wr*`), `video` (cellules, palette, registres CPS), `io` | le matériel modélisé ; aucune logique de jeu |
| `world` | `World`, `ram` (variables nommées), `record`, `objects/` (pools, listes, animation, mouvement, fiches `Obj`, `Enemy`, `Player`), `tasks/` (table, traps, étapes), `data`, `tables`, `terrain`, `rng`, `events`... | l'état du monde et le moteur d'objets |
| `services` | `palette`, `fading`, `text`, `sprites`, `scroll`, `video`, `popups`, `sound`, `inputs`, `eeprom`, `config` | les services appelés par les acteurs et les écrans |
| `actors` | `families/` (ennemis, boss, effets, décors SET, tirs ESHL...), `player/`, `ai`, `combat`, `camera_modes`, `dispatch`, `update` | les gestionnaires d'objets |
| `screens` | `modes/` (attraction, titre, classement), `scene/` (entrée, sortie, boutiques), `hud/`, `service/` (menu TEST), crédits | les écrans et séquences |
| `core` | `kernel` (ordonnanceur), `tasks/`, `vblank`, `startup`, `frame`, `game`, `interface`, `scenario` | la boucle d'image et l'interface `Game` |

Le module `tools/` (fonctionnalité `tools`) est à part : machine de référence, natives à registres pour comparer une
routine à son original, registre des natives par adresse ([fin.md](fin.md) « L17 »).

Les noms des fichiers et des fonctions suivent le découpage du code d'origine : chaque module dit en tête quelles
routines d'origine il réécrit (adresses `0x...`), et [atlas.md](atlas.md) donne la carte inverse (zone du code
68000 → document et module).

## 3. Une image du jeu

L'application appelle `Game::step(inputs)` une fois par image (59,64 Hz) ; tout le reste se passe dans le moteur :

1. **`Game::step`** (`core/interface.rs`) pose les entrées, vide le son de l'image précédente et fait avancer la
   machine d'une image (`cps2::Machine::run_frame`, sans rendu).
2. **La boucle d'image** (`core/frame.rs`, `line`) fait le travail du processeur au début de l'image : l'interruption
   VBLANK (`core/vblank.rs` : copie des objets, palettes, entrées, pièces), puis l'**ordonnanceur**
   (`core/kernel.rs`) qui parcourt les 16 tâches de la table (`world/tasks/table.rs`) et lance l'étape de chaque
   tâche prête (`core/tasks/mod.rs`, `execute`), jusqu'à ce que plus aucune ne soit prête.
3. **La tâche de jeu** (`core/tasks/game.rs`) exécute la phase courante (`core/game.rs`) ; en partie (phase 12),
   **la mise à jour du monde** (`actors/update.rs`, `update_world`) met à jour les joueurs puis les dix classes
   d'objets dans l'ordre de la scène.
4. **L'aiguillage** (`actors/dispatch.rs`) parcourt le pool de chaque classe et appelle, pour chaque objet actif, le
   gestionnaire de son type (§ 5).
5. Les gestionnaires écrivent dans la RAM d'objets et la GFX RAM (par `services::sprites`, `hw::video`) ; le son
   passe par `services::sound` vers le pilote (`ddsom-sound`), puis le mixeur (`qsound`).
6. L'application lit le résultat : `Game::frame()` (la liste d'affichage, § 8) et `Game::sound()`.

Le démarrage (`core/startup.rs`), les écrans hors partie (`screens/modes/`) et le menu TEST (`screens/service/`)
sont d'autres tâches de la même table. Pour la mécanique fine (points, interruption, cessions, plafond d'étapes) :
le commentaire de tête de `core/frame.rs` et [noyau.md](noyau.md) ; pour les phases et l'aiguillage :
[boucle.md](boucle.md) et [objets.md](objets.md).

## 4. Tâches et étapes

L'original est un système coopératif : une tâche s'exécute jusqu'à ce qu'elle **cède la main** (`jsr 0xa40` :
attendre une image, `0xa86` : attendre n images...), puis reprend juste après à une image suivante. En Rust, une
tâche est une machine à états :

* chaque point de reprise est une **étape**, une variante d'un `enum` par module, déclarée dans
  `world/tasks/stages.rs` (données pures : la variante et ce qu'elle doit garder) ; l'`enum` `Step` de
  `world/tasks/mod.rs` les réunit, et la table des tâches range l'étape de chaque tâche ;
* le code de chaque étape reste dans son module, associé par la macro `steps!` à l'adresse de reprise d'origine (un
  nom, gardé pour les outils) :

  ```rust
  pub use crate::world::tasks::stages::Prompt as Step;

  crate::steps! {
      impl Step {
          Entry = 0xe03fe => entry,
          CoinsShown = 0xe0440 => |x| {
              print_shown(x, Print::ClearCoins);
              cede(x, Wait::Images(HIDE_FRAMES), Step::CoinsHidden)
          },
          // ...
      }
  }
  ```

  (`screens/prompt.rs`). Une étape reçoit un `Ctx` (le monde et des accès nommés) et rend un `Flow` : `Done` (elle
  revient à son appelant) ou `Yield` (elle a cédé : `cede`, `wait`, `one_frame`... de `world/tasks/mod.rs`, qui
  rangent dans la table l'attente et l'étape de reprise) ;
* **ajouter une étape** : une variante dans l'`enum` du module (`world/tasks/stages.rs`), et dans `Step` si c'est un
  nouveau module ; son code dans le module (`steps!`) ; `core/tasks/mod.rs` (`run_steps!`) associe chaque `enum` à
  son module. Le compilateur vérifie que chaque variante a son code.

Un sous-programme qui revient sans céder exécute aussitôt la suite de son appelant (`Sequence::subroutine`) : la
seule inversion de dépendance entre couches, déclarée dans `world` et mise en œuvre par `core` ([fin.md](fin.md)
« L15c »).

## 5. Les objets et leur aiguillage

Les objets vivent dans des **pools** de la RAM (`world/objects/pools.rs`) : un emplacement par objet, des champs à
des décalages fixes. Il y a dix classes (tirs et effets des joueurs `PSHL`/`PEFF`, ennemis `ENEMY`, boss `BOSS`,
effets `EFFCT`, objets `ITEM`, caisses `BOX`, décors `SET`, tirs ennemis `ESHL`, objets spéciaux `SOBJ`), décrites
par `dispatch::Class` (`actors/dispatch.rs`) : leur pool et leur tableau de gestionnaires, indexé par le type de
l'objet (`Obj::KIND`).

* Un gestionnaire est une fonction `fn(&mut World, &mut Dispatch)` déclarée par `handlers!` (`actors/families/cx.rs`)
  avec l'adresse et le nom de la routine d'origine :

  ```rust
  handlers! {
      ESHL_00 0x9dbfc "eshl_00" shot0::run,
      ESHL_01 0x9de10 "eshl_01" shot1::run,
  }
  ```

* Les tableaux de `dispatch.rs` désignent ces constantes ; un type sans gestionnaire dans l'original est
  `Handler::Dead`, un type au-delà de la table est un `unreachable!` documenté.
* Une famille se range dans `actors/families/<famille>/` : `mod.rs` (aiguillage par état, `handlers!`), un fichier
  par état ou par variante, `fields.rs` ou `records.rs` (ses champs, § 6), `tables.rs` (ses données, § 7). Les
  documents de [familles/](familles/) et [familles.md](familles.md) décrivent chaque famille.

## 6. Lire et écrire l'état

Le monde (`world::World`, `world/mod.rs`) tient la RAM de travail (`ram: Box<[u16; 0x8000]>`), la table des tâches
et le matériel (`hw`). On n'écrit jamais un décalage littéral hors d'une déclaration :

* **Variables globales** : constantes nommées de `world/ram.rs` (ou `vars.rs` d'un module : `screens/vars.rs`,
  `actors/families/set/vars.rs`...), lues par les accès de `hw::mem` ou les aides de `world::ram`.
* **Champs d'un objet** : la macro `record!` (`world/record.rs`) déclare chaque champ une fois, avec son type et son
  décalage, et produit la constante et les deux accès :

  ```rust
  record! {
      impl Enemy {
          /// Type of the object in its class.
          kind, set_kind, KIND: u8 @ 0x02;
      }
  }
  // e.kind(b), e.set_kind(b, 3), Enemy::KIND
  ```

  Les fiches communes sont `Obj` (`world/objects/mod.rs`), `Enemy` (`world/objects/enemy.rs`), `Player`
  (`world/objects/player.rs`) ; une famille déclare ses champs propres sur sa vue (`Boss11(Obj)`...).
* **Vidéo** : par `hw::video` (`Cell`, palette, registres) et les services (`services::sprites`, `scroll`,
  `palette`), jamais par une adresse 0x90xxxx écrite à la main.
* **Hasard** : `world::rng` (le générateur de l'original : un tirage de plus ou de moins se voit à la validation).

L'ordre des écritures compte : `validate` compare la RAM à chaque image, et un autre gestionnaire peut relire un
champ laissé par le précédent (les « restes », `Leftovers`).

## 7. Lire une donnée

Les données de l'original (sprites, animations, décors, textes, tables de jeu) sont chargées une fois dans
`ddsom_data::GameData` et lues par **`data()`** (`static`, `world/data.rs` la réexporte) :

* tables typées : `data().frames`, `data().anims`, `data().levels`, `data().texts`, `data().speeds`... ;
* tables nommées d'une famille : `data().effects.shop_prices` (déclarées par `tables!`), ou par le `tables.rs` de la
  famille ;
* tables du moteur, par nom : `engine_table!("rank/presets").at8(b, i)` (`world/tables.rs`), et par adresse
  (`engine().byte(...)`) quand l'état désigne une table par son adresse d'origine (les adresses de la ROM rangées
  dans la RAM restent des **clés**, `RomAddr`).

Les deux modes (avec ROMs, ou `--assets`) remplissent la même `GameData` : depuis les ROMs par l'extraction en
mémoire (`extract::install_game`), depuis les fichiers par `extract::install_game_assets`. Une lecture hors de toute
table (index au-delà de la fin, sur un chemin qu'aucun scénario ne joue) passe par le **résidu** (`Missing`,
`world/data.rs`) : comptée, fatale avec `DDSOM_STRICT=1`, jamais consultée sur les 309 scénarios
([sans-rom.md](sans-rom.md) « Le résidu »).

## 8. Ajouter une table d'asset

Une table de l'original que le jeu doit lire devient une donnée nommée, extraite une fois :

1. **Extraction** : une ligne dans le fichier de sa famille de `port/extract/src/` (par exemple
   `t.prog("ai/criteria_strict", 0x426a, "u8", Some(16));` dans `engine.rs`, ou une ligne de `tables!` dans
   `ddsom-data/src/effects.rs`, `objects.rs`, `player_tables.rs`), avec son adresse, son espace (données, ou
   programme pour une table lue relativement au PC), sa taille d'élément et, si le code la borne, son nombre
   d'éléments.
2. **Type et chargement** : dans `port/ddsom-data/src/` (un champ de `GameData` si c'est un nouveau fichier,
   chargé par `GameData::load`).
3. **Test sans perte** : la reconstruction de la région programme depuis les assets doit rester identique à la ROM
   hors code (`port/extract/tests/rebuild.rs`, `roundtrip.rs` ; `cargo test --release -p extract`).
4. **Lecture** dans le jeu, par son nom (§ 7).
5. **Réextraire les assets** (`extract rom assets`) et valider dans les deux modes (§ 10).

Formats et tables existantes : [assets.md](assets.md) ; débordements de l'original : [sans-rom.md](sans-rom.md).

## 9. L'interface du jeu et les façades

Une application ne voit le jeu que par `ddsom_game::Game` (`core/interface.rs`) :

```rust
let mut game = Game::new(contents, eeprom);   // contents : extract::install_game(_assets)
game.step(inputs);                            // une image ; Inputs : les trois ports CPS2 (actifs à 0)
let list = game.frame();                      // DrawList : ce qu'il faut dessiner
let samples = game.sound();                   // &[[i16; 2]] : le son de l'image, 24 038 Hz
let nvram = game.eeprom();                    // à rendre à Game::new la fois suivante
```

`DrawList` (`port/cps2-gfx/src/draw.rs`) décrit l'image sans la dessiner : palette, trois couches de tuiles et leur
défilement, ordre des couches, masques de priorité, sprites, et l'identité de chaque image de sprite (son numéro
dans `frames.json`). Deux rendus la dessinent :

* `cps2_gfx::render(&list, &atlas)` : le rendu CPS2 exact, 384 x 224 ;
* `ddsom_hd::render_hd(&list, &atlas, &pack)` : le même avec les images d'un pack HD ([pack-hd.md](pack-hd.md)).

La façade `ddsom` (`port/ddsom/src/`) : `main.rs` (boucle, cadence à l'horloge, sauvegarde rapide), `view.rs`
(texture, échelle, filtres, plein écran), `keys.rs` (`touches.cfg`), `gamepad.rs` (gilrs), `audio.rs`
(rééchantillonnage à 48 kHz, PulseAudio ou waveOut), `save.rs` (sauvegarde par rejeu des entrées). Une autre façade
(autre moteur 2D, web...) n'a qu'à refaire ces fichiers autour de `Game` ; la logique ne bouge pas.
`DDSOM_IDENTITY=N` vérifie qu'une façade montre l'image du rendu CPS2 au pixel près (relecture de la fenêtre à
l'échelle 1).

## 10. Valider

### Construire

```sh
cd port
export DDSOM_ROMS=$PWD/../rom      # toujours la même chaîne (sinon ddsom-rc se régénère)
cargo build --release -p ddsom-game --examples     # validate, bench, render_check, fuzz_*... (target/release/examples/)
cargo build --release -p ddsom -p extract -p ddsom-hd
```

### Valider un scénario

Un scénario (`scenarios/**/*.txt`) est une suite d'entrées par image (et parfois de pokes de RAM pour entrer
directement dans une scène). `validate` le rejoue sur le jeu et sur la machine de référence et compare, à chaque
image, la RAM de travail, la GFX RAM, la table des sprites, la palette, la table des tâches et les écritures du
pilote son :

```sh
port/target/release/examples/validate rom testdata/eeprom_ddsom.bin scenarios/scene/scene_01.txt 6500
```

La dernière ligne est `VALID` ou `FAILED` avec la première différence. `testdata/eeprom_ddsom.bin` est l'EEPROM
de départ des scénarios (non versionnée, comme les ROMs).

### Validation complète

```sh
DDSOM_ROMS=$PWD/rom DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1 DDSOM_JOBS=6 setsid -w tools/validate_all.sh 30000
```

309 scénarios (bots des six personnages, boss, ennemis, modes, entrées et sorties de scènes, couverture, bots
longs), 15 à 30 minutes ; **tous doivent être `VALID`**. Les résultats sont gardés dans `~/.cache/ddsom/validate/`
sous l'empreinte du binaire : relancer après un arrêt ne rejoue que ce qui manque. Toujours valider aussi **sans
ROM**, sur des assets fraîchement extraits :

```sh
cargo run --release --manifest-path port/Cargo.toml -p extract -- rom /tmp/assets_frais
DDSOM_ASSETS=/tmp/assets_frais DDSOM_ROMS=$PWD/rom DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1 DDSOM_JOBS=6 \
  setsid -w tools/validate_all.sh 30000
```

### Les autres filets

| Outil | Ce qu'il vérifie |
|---|---|
| `render_check rom testdata/eeprom_ddsom.bin scenarios/bot30k.txt 12000` | l'image et le son, image par image, par une empreinte : `09850058246a1e2c` tant que le jeu ne change pas (aussi avec `DDSOM_ASSETS`) |
| `bench rom testdata/eeprom_ddsom.bin scenarios/bot30k.txt 12000` | la vitesse du jeu seul (`DDSOM_RENDU=1` : avec le rendu) ; comparer avec main construit dans le même dossier cible |
| `DDSOM_CHECK=1 validate ...` | chaque gestionnaire d'objet appelé est aussi comparé à son original sur une copie de la machine |
| `fuzz_couv ...` | fuzz différentiel guidé par la couverture : les chemins qu'aucun scénario ne joue, natives contre original sur des états perturbés ([couverture-fonctions.md](couverture-fonctions.md)) |
| `no_rom rom testdata/eeprom_ddsom.bin <assets> <scénario>:<images>` | le jeu chargé des assets joue exactement comme le jeu chargé des ROMs |
| `cov`, `tools/cov_report.py` | la couverture des instructions d'origine par les scénarios ([couverture.md](couverture.md)) |
| `cargo clippy --release -p ddsom-game --all-targets` (avec et sans `--no-default-features`), `-p ddsom` (avec et sans `--features original`), `-p ddsom-hd` | aucun avertissement |
| `cargo test --release -p ddsom-game --lib`, `-p ddsom`, `-p ddsom-hd`, `-p extract`... | les tests unitaires et sans perte |
| `tools/measure_end.py` | les marques « en forme de ROM » qui restent et le graphe des couches ([fin.md](fin.md) § 1) |
| `tools/check_links.py` | les liens de la documentation |

Les outils d'exemple sont dans `port/target/release/examples/` ; leur commentaire de tête (`port/ddsom-game/examples/*.rs`)
donne leurs arguments et variables d'environnement.

### Quand un scénario échoue

`validate` donne la première image et l'adresse qui diffèrent. Pistes : `examples/watch` (qui écrit une adresse),
`pctrace` (routines d'origine exécutées), `dumpram` ; le guide pratique est [guide-refactorisation.md](guide-refactorisation.md)
(check-list, `DDSOM_POISON`, pièges) et [rewrite-guide.md](rewrite-guide.md). Une routine qui manque arrête le jeu
avec un diagnostic (`STOP`) plutôt que de deviner.

## Pour en savoir plus

* [architecture.md](architecture.md) : la machine de référence et l'historique de l'architecture.
* [fin.md](fin.md) : le plan qui a mené au moteur d'aujourd'hui, lot par lot, avec les mesures.
* [README.md](README.md) : tous les documents.
