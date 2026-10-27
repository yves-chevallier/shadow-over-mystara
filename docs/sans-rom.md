# Jouer sans ROM

Le jeu sans 68000 (le mode par défaut, [noyau.md](noyau.md) « Le jeu sans 68000 ») n'exécute plus que les routines
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
target/release/ddsom --assets <assets>
```

`ddsom --assets <assets>` sans dossier de ROMs donné joue ainsi sans ROM.
Aucun fichier ROM n'est ouvert : la machine est faite par `ddsom_assets::rebuild::contenus` puis
`cps2::Machine::ddsom` (la machine ordinaire passe par `cps2::Contenus::depuis_roms`). Le son passe toujours par le
pilote Rust (`Sound::use_rust_driver`). Avec un dossier de ROMs donné, `--assets` ne prend que les
graphismes et les échantillons dans les assets (le programme vient des ROMs).

Le dossier d'assets est local (dérivé des ROMs de l'utilisateur), jamais versionné.

**Régénérer ses assets.** Un dossier d'assets produit par une version plus ancienne d'`extract` peut manquer de
données : relancer `extract` sur le même dossier (il réécrit tous les fichiers ; `extract <roms> <assets> data`
suffit quand seules les tables de données ont changé, en quelques secondes). Un dossier sans `data/reste.json`
(antérieur à la couverture complète) est refusé au démarrage (« assets incomplets : data/reste.json »). Sous
Windows, `jouer.bat` réextrait de lui-même un `assets\` sans `data\reste.json` (il lui faut alors `ddsom.zip` et
`qsound.zip` à la racine, ou `rom\` déjà décompressé) ; pour forcer une réextraction après une autre mise à jour
d'`extract`, supprimer `assets\` puis relancer `jouer.bat`.

## D'où vient chaque puce

| Puce (`cps2::Contenus`) | Depuis les assets | Ce qui manque, et pourquoi c'est sans effet |
|---|---|---|
| Programme 68000, vue données et vue programme déchiffrée (`Program::load`) | toutes les tables décrites (`palettes.png`, `sprites/*.json`, `levels.json`, `terrain.json`, `scene_palettes.json`, `texts.json`, `data/*.json`) ; `data/natives.json` : les zones lues par les natives qu'aucun autre fichier ne décrit (ci-dessous) ; `data/reste.json` : tout le reste, sauf le code 68000 (« Couverture complète des données ») | le code 68000 exécuté (486 399 octets, zéro) : sans 68000 il n'est plus exécuté ni lu |
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
Rust ») tourne sans 68000. Toute lecture du bus dans 0x000000-0x3fffff y est comptée (il n'y
a plus d'instruction 68000 à exclure), en espace données (`rd8`/`rd16`/`rd32`, `data16`...) et programme
(`prog16`, `rom16` des familles, `Ctx::prog16`...). Lancement sur **tous les scénarios** (269, autant d'images que
leur validation : la ligne `quit` d'un scénario ne l'arrête pas, le jeu continue en démonstration), en dix morceaux :

```sh
cd port
CARGO_TARGET_DIR=target/trace cargo build --release -p ddsom-game --example lectures_rom --features rom-trace
target/release/examples/couverture_rom <assets> --octets couvert.bin --sans data/natives.json
DDSOM_COUVERT=couvert.bin target/trace/release/examples/lectures_rom <roms> <eeprom> lectures.txt \
    ../scenarios/bot30k.txt:30000 ...
../tools/zones_natives.py couvert.bin lectures*.txt > extract/zones_natives.txt   # puis extract ... data
```

**Le code 68000 n'est plus lu.** La première mesure (16 scénarios) trouvait encore 30 000 mots de code lus en
espace programme : les durées des natives (`Native::cost`, appelé par l'exécuteur sans 68000) et les durées de
chemins que les natives ajoutent aux cycles (`chemin.rs`, `timing.rs`, `objects/cost.rs`, `ai.rs` : ils décodent
le code d'origine avec `m68k::decode`). Sans 68000 les cycles ne servent plus (le temps de l'image est celui de la
boucle du noyau) : ces fonctions rendent 0 sans rien lire quand `sans68k::actif()`, et l'exécuteur ne calcule plus
le coût. Validation sans 68000 inchangée.

**Résultat** (269 scénarios, 4,4 millions d'images, aucun arrêt faute de native) :

| Espace | Octets lus | Déjà décrits par les assets | Non décrits | Zones de `data/natives.json` |
|---|---:|---:|---:|---:|
| Données | 1 652 622 | 1 583 952 (95,8 %) | 68 670 | 1 502 zones, 84 Ko |
| Programme (déchiffré, lectures relatives au PC) | 26 066 | 3 706 (14,2 %) | 22 360 | 1 297 zones, 28 Ko |

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

## Couverture complète des données

La mesure ci-dessus ne voit que les chemins que les scénarios jouent : une partie de l'utilisateur pouvait lire un
octet que nul scénario ne lisait (il valait zéro et le jeu divergeait : objet qui disparaît, écran bloqué avant le
boss, cave de Tritan). `data/reste.json` rend la couverture complète **par construction** : tout octet de la région
programme qui n'est pas du code 68000 est de la donnée, et il y est rangé s'il n'est pas déjà dans un autre fichier.

**Le code exclu est le code exécuté.** `port/extract/code_68k.txt` (versionné : des adresses, pas d'octets de la
ROM) liste les plages d'octets des instructions que la machine de référence (code d'origine recompilé, 68000
surcadencé, comme la machine B de `validate`) exécute sur les 272 scénarios de la validation complète
(`tools/code_68k.sh`, exemple `code_execute` de ddsom-game : têtes des blocs recompilés exécutés, étendues jusqu'à
l'instruction qui termine le bloc de base, plus les instructions interprétées ; environ un quart d'heure avec 12
tâches). En cas de doute un octet va du côté des données : le code jamais exécuté (la découverte statique de
`recomp` trouve 141 504 octets de code de plus, et manque 25 868 octets exécutés : elle n'est pas sûre), les tables
rangées après un `jmp`, les octets qu'un appel saute (données en ligne après un `jsr`) sont dans `reste.json`. Un
octet de code en trop dans un asset local ne gêne pas ; un octet de donnée manquant casse le jeu.

**Données dans le code exécuté.** Deux cas restent couverts à part : les zones mesurées de `natives.json` qui
tombent dans le code (192 zones : la copie de 32 palettes depuis l'adresse 0 qui prend du code, des mots lus juste
avant une table...) restent dans `natives.json` (l'union est gardée) ; et `extract` cherche les opérandes des
instructions exécutées qui lisent une adresse fixe (relative au PC ou absolue ; pour `lea`/`pea`, une adresse qui
n'est pas un début d'instruction) tombant dans le code : la plage est rendue aux données de la cible à sa fin
(9 plages, par exemple une table d'octets indexée qui commence dans le déplacement d'un `bra`, 0x406d).

**Format.** `data/reste.json` : `fin_chiffree` (0x100002 : au-delà, l'espace programme égale l'espace données et la
reconstruction le recopie), `octets_de_code` et `code` (les plages de code, `"0x400-0x456"`), puis `donnees` et
`programme` (zone chiffrée seulement) : des blocs `debut` + lignes de 32 octets en hexadécimal. Environ 540 000
octets en 7 400 blocs (données) et 528 000 en 5 450 blocs (programme) : 3 Mo de JSON. `extract ... data` l'écrit en
dernier (il lit les autres fichiers pour savoir ce qu'ils décrivent).

**Contrôle.** `programme_complet <assets> <roms> [--liste]` (exemple de ddsom-assets) reconstruit la région
programme, vérifie que chaque octet reconstruit égale la ROM dans les deux espaces et que les octets absents sont
tous du code déclaré, et les liste avec leur total ; le test `programme_complet_hors_code` (`tests/rebuild.rs`)
fait la même vérification :

```
espace données : 3708403 octets reconstruits (égaux à la ROM), 485901 absents en 5872 plages, dont 0 hors du code déclaré
espace programme : 3708539 octets reconstruits (égaux à la ROM), 485765 absents en 5753 plages, dont 0 hors du code déclaré
code déclaré (data/reste.json) : 486399 octets en 5761 plages
```

**Mettre à jour.** `tools/code_68k.sh` après l'ajout de scénarios qui exécutent du code nouveau (sans effet sur la
sûreté : du code non mesuré est rangé en données), puis `extract ... data`. Les zones de `zones_natives.txt` ne sont
plus nécessaires hors du code, mais rien n'empêche d'en ajouter (`tools/zones_natives.py`).

## Vérifications

* **`sans_rom`** (exemple de `ddsom-game`) : `sans_rom <roms> <eeprom> <assets> <scénario>:<images> ...` fait tourner
  deux machines sans 68000 avec le pilote son Rust, l'une sur les ROMs, l'autre sur les seuls assets, et compare
  à chaque image l'image affichée, les échantillons audio, la RAM de travail, la GFX RAM, la RAM d'objets, la
  palette et la RAM partagée. Sur `bot_2p`, `boss/boss13_dragon`, `cov/cov_4p_a`, `modes/ending` (12 000 images
  chacun), `scene_end/end_02` (9 000), `cov/cov_service_tests` et `modes/attract` (6 000) : identique.
* **`validate` avec `DDSOM_ASSETS=<assets>`** : la machine testée est faite des seuls assets
  (pilote son Rust), la référence garde les ROMs, le 68000 surcadencé et le Z80. `tools/validate_all.sh` s'en sert
  tel quel (la variable entre dans l'empreinte des résultats) :
  `DDSOM_ASSETS=<assets> DDSOM_SCENES=1 DDSOM_COV=1 tools/validate_all.sh 30000` donne **272 VALIDE
  sur 272** avec `data/reste.json` (2026-10-27, comme le mode par défaut : 272 sur 272). Avant la couverture
  complète, les zones mesurées seules donnaient 269 sur 269, mais chaque partie de l'utilisateur pouvait sortir
  des chemins mesurés (voir ci-dessous).
* **La couverture complète rend les zones ajoutées à la main inutiles** : `cov_cave_tritan` et `cov_objet_tritan`
  (deux parties de l'utilisateur qui divergeaient sans ROM, corrigées sur main en prolongeant 8 zones de
  `zones_natives.txt`) sont VALIDE avec des assets extraits sans ces prolongements ; et encore VALIDE quand
  `natives.json` est réduit aux 192 zones qui touchent le code (tout le reste venant de `reste.json`). Témoin : les
  mêmes assets avec un `reste.json` vidé de ses blocs échouent (images 1 325 et 562).
* **Bots longs** (autobot, hors de la validation complète) : deux joueurs avec magies (90 000 images), un joueur
  qui parcourt 28 scènes en difficulté 7 et en langue 4 (90 000), trois joueurs mortels (60 000). Avec
  `DDSOM_ASSETS`, chacun finit exactement comme en mode par défaut (avec les ROMs) : arrêts sans 68000 faute de
  native en 0x4dfe2 (`magic_user.rs`, image 15 318) et 0x20ae (`scene/shop.rs`, image 86 671), divergence de
  natives à l'image 13 279 (trois joueurs) ; ce sont des trous du mode sans 68000, pas des données. Seul écart
  d'assets trouvé : la lecture hors bornes de la table des boutiques ci-dessous (image 86 640), corrigée. Avec un
  `reste.json` vidé, les trois divergent dès les images 4 370, 4 651 et 5 780.
* **Sans ROM du tout** : une copie du dépôt (`git archive`) se construit sans dossier `rom/` (`ddsom-rc` vide, avec
  l'avertissement « ROMs introuvables ») et `ddsom --assets <assets>`, lancé d'un dossier sans `../rom`, démarre le
  jeu sans 68000 depuis les assets.

## Ce qui manque

* **Les zones de `data/natives.json` sont brutes** : mots (ou octets) rangés par zone lue, nommés d'après le fichier
  Rust qui les lit, sans format. Chacune est une petite table (vitesses, décisions des chefs, positions par
  variante, chaînes, colonnes du HUD, tables d'aiguillage d'adresses de routines, vecteurs d'interruption) dont le
  sens est dans son lecteur ; les décrire une à une (comme `data/*.json`) reste à faire, et les tables d'adresses
  de routines disparaîtront quand les aiguillages seront des `match` Rust.
* **Couverture complète hors code exécuté** (« Couverture complète des données ») : le seul trou possible est une
  lecture, sur un chemin jamais mesuré, d'un octet d'une instruction exécutée (le cas des 192 zones de
  `natives.json` qui touchent le code, ou des opérandes trouvés par `extract`). Exemple trouvé par un bot long :
  la table des boutiques enchaînées (0xf7d72, 20 octets suivis de code) lue avec un numéro de boutique hors
  bornes (234, après un saut de scène du bot) prend un octet du code (0xf7e5c), comme l'original ; zone ajoutée à
  la main à `zones_natives.txt`. La validation avec `DDSOM_ASSETS` montre ces cas ; il suffit alors de relancer
  `lectures_rom` (avec `DDSOM_COUVERT`) sur ce scénario et d'ajouter la zone. Ranger aussi le code dans les assets
  supprimerait ce dernier risque (au prix de 486 Ko de plus, le programme entier).
* **Les trous du mode sans 68000** (natives manquantes : arrêt avec diagnostic) sont les mêmes avec ou sans ROM.
* L'émulation du DSP QSound lit ses tables comme le DSP (adresses du programme d'origine) ; les tables du pilote son
  aussi (adresses linéaires de la région du Z80). Les ranger en tableaux Rust nommés est l'étape suivante, quand ces
  deux crates ne serviront plus à comparer avec l'original.
