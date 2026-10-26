# Jouer sans ROM

Le jeu sans 68000 (`DDSOM_SANS_68K=1`, [noyau.md](noyau.md) « Le jeu sans 68000 ») n'exécute plus que les routines
Rust : le code 68000 d'origine n'est plus lu. Il lit encore des **données** dans la région programme (tables de
vitesses, de décisions, images de sprites, cartes...), des données du pilote son dans la région du Z80, des tables
dans le programme du DSP QSound, et bien sûr les graphismes et les échantillons. Tout cela peut venir du dossier
produit par `extract` : le jeu démarre et se joue alors **sans aucun fichier ROM**. Le dépôt se clone et se
construit sans rien de protégé (`ddsom-rc`, le code 68000 recompilé, est vide sans ROM : il ne sert qu'au mode avec
68000).

## Extraire, puis jouer

```sh
cd port
cargo run --release -p extract -- <dossier des ROMs> <assets>     # une fois : ddsom.zip et dl-1425.bin décompressés
cargo build --release -p ddsom
DDSOM_SANS_68K=1 target/release/ddsom --assets <assets>
```

`ddsom --assets <assets>` choisit ce mode de lui-même quand il n'y a pas de dossier de ROMs (ni donné, ni `../rom`).
Aucun fichier ROM n'est ouvert : la machine est faite par `ddsom_assets::rebuild::contenus` puis
`cps2::Machine::ddsom` (la machine ordinaire passe par `cps2::Contenus::depuis_roms`). Le son passe toujours par le
pilote Rust (`Sound::use_rust_driver`). Sans `DDSOM_SANS_68K`, avec un dossier de ROMs, `--assets` ne prend que
les graphismes et les échantillons dans les assets (le programme 68000 vient des ROMs, comme avant).

Le dossier d'assets est local (dérivé des ROMs de l'utilisateur), jamais versionné.

## D'où vient chaque puce

| Puce (`cps2::Contenus`) | Depuis les assets | Ce qui manque, et pourquoi c'est sans effet |
|---|---|---|
| Programme 68000, vue données et vue programme déchiffrée (`Program::load`) | toutes les tables décrites (`palettes.png`, `sprites/*.json`, `levels.json`, `terrain.json`, `scene_palettes.json`, `texts.json`, `data/*.json`) ; `data/natives.json` : les zones lues par les natives qu'aucun autre fichier ne décrit (ci-dessous) | le code 68000 (zéro) : sans 68000 il n'est plus exécuté ni lu |
| Région du Z80 (`rebuild::audio_region`) | `sound/music.json` : pointeurs de la ROM fixe, table des sons, définitions, pistes, instruments, fiches d'échantillons, tables du pilote | le programme du Z80 : remplacé par le pilote Rust (`ddsom-sound`), qui ne lit que ces données |
| DSP QSound (`rebuild::qsound_dsp`) | `sound/qsound_dsp.json` : pan (0x110, 0x150 mots), pas de l'ADPCM (0x9dc, 16), coefficients des filtres (0xd53-0xfff, et 0-2 où le second filtre déborde) | le code du DSP : l'émulation haut niveau (`qsound`, d'après MAME) le remplace et ne lit que ces tables |
| Graphismes (`rebuild::gfx_region`) | `gfx/cells_NN.png` | rien |
| Échantillons (`rebuild::qsound_samples`) | `sound/music.json` et `sounds/NNN.wav` (et la suite de chaque échantillon, `sample_tails`) | les octets qu'aucune voix ne lit |

**Le Z80.** Son programme n'est plus nécessaire : le pilote Rust (`DDSOM_SON_RUST=1`, `m.sound.use_rust_driver()`)
le remplace écriture par écriture ([son.md](son.md)) et ne lit dans la région du Z80 que des données, toutes dans
`music.json`. Contrôle : `sans_rom` compare aussi les échantillons audio produits (pilote Rust sur la région des ROMs
contre pilote Rust sur la région reconstruite, DSP complet contre DSP réduit à ses tables) : identiques.

## Ce que le jeu sans 68000 lit dans la région programme

**Mesure.** `lectures_rom` (fonctionnalité `rom-trace`, [assets.md](assets.md) « Lectures de la ROM programme par le
Rust ») a un mode sans 68000 : `DDSOM_SANS_68K=1`. Toute lecture du bus dans 0x000000-0x3fffff y est comptée (il n'y
a plus d'instruction 68000 à exclure), en espace données (`rd8`/`rd16`/`rd32`, `data16`...) et programme
(`prog16`, `rom16` des familles, `Ctx::prog16`...). Lancement sur **tous les scénarios** (269, autant d'images que
leur validation, plafonnées à leur ligne `quit`), en dix morceaux :

```sh
cd port
CARGO_TARGET_DIR=target/trace cargo build --release -p ddsom-game --example lectures_rom --features rom-trace
target/release/examples/couverture_rom <assets> --octets couvert.bin --sans data/natives.json
DDSOM_SANS_68K=1 DDSOM_COUVERT=couvert.bin target/trace/release/examples/lectures_rom <roms> <eeprom> lectures.txt \
    ../scenarios/bot30k.txt:30000 ...
../tools/zones_natives.py couvert.bin lectures*.txt > extract/zones_natives.txt   # puis extract ... data
```

**Le code 68000 n'est plus lu.** La première mesure (16 scénarios) trouvait encore 30 000 mots de code lus en
espace programme : les durées des natives (`Native::cost`, appelé par l'exécuteur sans 68000) et les durées de
chemins que les natives ajoutent aux cycles (`chemin.rs`, `timing.rs`, `objects/cost.rs`, `ai.rs` : ils décodent
le code d'origine avec `m68k::decode`). Sans 68000 les cycles ne servent plus (le temps de l'image est celui de la
boucle du noyau) : ces fonctions rendent 0 sans rien lire quand `sans68k::actif()`, et l'exécuteur ne calcule plus
le coût. Validation sans 68000 inchangée.

**Résultat** (269 scénarios, 3,7 millions d'images, aucun arrêt faute de native) :

| Espace | Octets lus | Déjà décrits par les assets | Non décrits | Zones de `data/natives.json` |
|---|---:|---:|---:|---:|
| Données | 1 650 068 | 1 581 688 (95,9 %) | 68 380 | 1 494 zones, 84 Ko |
| Programme (déchiffré, lectures relatives au PC) | 26 026 | 3 700 (14,2 %) | 22 326 | 1 298 zones, 28 Ko |

Une zone réunit les octets lus non décrits séparés de moins de 16 octets. Par lecteur (octets ; `d` données, `p`
programme) : `families/cx.rs` (lectures des familles d'objets, `Cx::rd*`/`rom16`) 31 742 d + 16 242 p ;
`sprites.rs` 10 228 d (textes en sprites, listes de morceaux) ; `families/boss_11/chain.rs` 8 332 d ;
`player/mod.rs` 6 056 d + 2 566 p ; `ai_aids.rs` 5 342 d + 1 264 p ; `objects/anim.rs` 3 940 d ;
`objects/motion.rs` 3 488 d ; `hud/layer.rs` 2 850 d ; `sound.rs` 636 d + 2 088 p (octets des enveloppes lus dans
le code des appels de son) ; `scene/mod.rs` 1 972 d + 586 p ; `dispatch.rs` 1 392 p (tables d'adresses de
routines des pools d'objets) ; le reste en petites zones (moins de 1,7 Ko par lecteur). Deux cas particuliers :
`palette.rs` lit 0x000-0x400 (la page 3 des palettes des scènes 0x58 et 0x59 a une entrée nulle : le jeu copie
32 palettes depuis l'adresse 0, vecteurs compris, comme l'original), et `vecteurs` (0-7 en espace programme : pile
et PC du reset, lus par `Cpu::reset` à la construction de la machine). Quatre petites zones lues par des accès
génériques (`Vue`, `mem.rs`) à plus de trois niveaux d'appel sont nommées `inconnu`.

## Vérifications

* **`sans_rom`** (exemple de `ddsom-game`) : `sans_rom <roms> <eeprom> <assets> <scénario>:<images> ...` fait tourner
  deux machines sans 68000 avec le pilote son Rust, l'une sur les ROMs, l'autre sur les seuls assets, et compare
  à chaque image l'image affichée, les échantillons audio, la RAM de travail, la GFX RAM, la RAM d'objets, la
  palette et la RAM partagée. Sur `bot_2p`, `boss/boss13_dragon`, `cov/cov_4p_a`, `modes/ending` (12 000 images
  chacun), `scene_end/end_02` (9 000), `cov/cov_service_tests` et `modes/attract` (6 000) : identique.
* **`validate` avec `DDSOM_ASSETS=<assets>`** (et `DDSOM_SANS_68K=1`) : la machine testée est faite des seuls assets
  (pilote son Rust), la référence garde les ROMs, le 68000 surcadencé et le Z80. `tools/validate_all.sh` s'en sert
  tel quel (la variable entre dans l'empreinte des résultats). RÉSULTATS_VALIDATE
* **Sans ROM du tout** : une copie du dépôt (`git archive`) se construit sans dossier `rom/` (`ddsom-rc` vide, avec
  l'avertissement « ROMs introuvables ») et `ddsom --assets <assets>`, lancé d'un dossier sans `../rom`, démarre le
  jeu sans 68000 depuis les assets.

## Ce qui manque

* **Les zones de `data/natives.json` sont brutes** : mots (ou octets) rangés par zone lue, nommés d'après le fichier
  Rust qui les lit, sans format. Chacune est une petite table (vitesses, décisions des chefs, positions par
  variante, chaînes, colonnes du HUD, tables d'aiguillage d'adresses de routines, vecteurs d'interruption) dont le
  sens est dans son lecteur ; les décrire une à une (comme `data/*.json`) reste à faire, et les tables d'adresses
  de routines disparaîtront quand les aiguillages seront des `match` Rust.
* **Couverture par mesure.** Une zone qu'aucun des 269 scénarios ne lit n'est pas extraite : un chemin jamais joué
  pourrait lire un octet absent (il vaudrait zéro). La validation avec `DDSOM_ASSETS` le montrerait sur un nouveau
  scénario ; il suffit alors de relancer la mesure avec ce scénario et `zones_natives.py`. Les zones sont étendues
  aux trous de moins de 16 octets, ce qui couvre déjà les entrées voisines des tables lues.
* **Les trous du mode sans 68000** (natives manquantes : arrêt avec diagnostic) sont les mêmes avec ou sans ROM.
* L'émulation du DSP QSound lit ses tables comme le DSP (adresses du programme d'origine) ; les tables du pilote son
  aussi (adresses linéaires de la région du Z80). Les ranger en tableaux Rust nommés est l'étape suivante, quand ces
  deux crates ne serviront plus à comparer avec l'original.
