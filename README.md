# Rétro-ingénierie de Dungeons & Dragons: Shadow over Mystara (ddsom, CPS2)

Jeu : `ddsom`, Europe 960619 (driver MAME `src/mame/capcom/cps2.cpp`).

## Matériel

| Élément | Détail |
|---|---|
| CPU principal | 68000 @ 16 MHz, opcodes chiffrés (clé `ddsom.key`) |
| CPU son | Z80 + QSound (`dd2.01`, `dd2.02`, samples `dd2.11m`/`dd2.12m`) |
| Graphismes | `dd2.13m`..`dd2.20m` (ROM_LOAD64_WORD) |
| Programme | `dd2e.03e`..`dd2e.10`, ROM_LOAD16_WORD_SWAP, 0x000000-0x3FFFFF |

Carte mémoire 68000 (cps2_map) :

| Adresse | Rôle |
|---|---|
| 000000-3FFFFF | ROM programme |
| 400000-40000B | sortie objets CPS2 |
| 618000-619FFF | RAM partagée QSound (Z80) |
| 660000-664001 | RAM additionnelle |
| 700000 / 708000 | Object RAM (sprites), banque via 8040E0 |
| 804000/10/20 | entrées IN0/IN1/IN2 (+ EEPROM) |
| 804040 | EEPROM sortie |
| 804100-80417F | registres CPS-A / CPS-B |
| 900000-92FFFF | GFX RAM (tilemaps, palettes) |
| FF0000-FFFFFF | RAM de travail (A5 = FF8000 en permanence) |

## Chiffrement CPS2 : ce qu'il faut savoir

- Seuls les accès en **espace programme** sont déchiffrés : les fetchs d'instructions **et les
  lectures de données en adressage relatif au PC** (`move.w tbl(pc,d0.w),d1`, `movea.l tbl(pc,d1.w),a1`).
  Les autres lectures de données sont en clair.
- Pour ddsom, la plage chiffrée est 0x000000-0x0FFFFF. Le reste de la ROM est en clair.
- Instruction watchdog : `cmpi.l #$19660419,d1`, parsemée partout dans le code.
- Le vecteur de reset est lu en espace programme (SP=00FF0E66, PC=000350) ; les autres vecteurs en clair.

## Fichiers produits

| Fichier | Contenu |
|---|---|
| `prog_plain.bin` | image programme brute (données correctes, opcodes chiffrés) |
| `prog_dec.bin` | tout déchiffré (opcodes corrects, données fausses dans le 1er Mo) |
| `prog_merged.bin` | **image de travail** : déchiffré là où c'est du code/lu via PC, clair ailleurs |
| `listing.txt` | listing texte exporté de Ghidra |
| `ghidra_proj/` | projet Ghidra (`ghidraRun`, puis ouvrir `ghidra_proj/ddsom.gpr`) |

Tout se régénère avec `./build.sh`.

## Outils

- `cps2dec/` : déchiffreur CPS2 en Rust (port de `cps2crypt.cpp` de MAME, BSD-3).
- `tools/build_prog.py` : assemble les ROMs programme. `tools/gen_sboxes.py` : génère les s-boxes Rust.
- `ghidra_scripts/Cps2Setup.java` : carte mémoire, labels d'E/S, vecteurs, et fusion itérative
  code déchiffré / données en clair. Résout les idiomes Capcom que Ghidra ne suit pas seul :
  tables d'offsets et de `bra.w` relatives au PC, tables de pointeurs `movea.l (pc,dn)`,
  appels sans pile (`lea ret(pc),a6 ; bra f` / `jmp (a6)`), créations de tâches (`lea f,a0 ; trap #0`).
- `mame/coverage.lua` : couverture réelle (tap sur l'espace opcodes), cumulative entre sessions.
  `build.sh` l'utilise automatiquement si `mame/coverage.bin` existe. `tools/cov_check.py` compare
  couverture et image fusionnée (doit afficher 0 mot manqué).

## Lancer le jeu avec couverture

    mame ddsom -rompath /home/ycr/som -autoboot_script /home/ycr/som/mame/coverage.lua

Jouer (Tab = menu MAME), quitter avec Échap, puis `./build.sh` pour réintégrer la couverture.
Démo seule, sans fenêtre : ajouter `-video none -sound none -nothrottle -seconds_to_run 180`.

## Premières observations

- `reset` (0x350) : init registres CPS, test RAM sans pile (appels via a6), attente du Z80
  (0x619FFF == 0x77), puis 0x492 : init RAM de travail et création de la tâche 0 (0x11B16).
- Ordonnanceur coopératif (0x928) : 16 tâches de 16 octets à FF0064 (A5-0x7F9C), état >= 4 = active.
  `trap #0` crée une tâche (d0 = slot, a0 = entrée), `trap #1` termine la tâche courante.
- `irq2_vblank` (0x574) : handler VBLANK. IRQ4 (raster) pointe sur un handler vide (0x166).

## Port Rust (`port/`)

Architecture et jalons : [docs/architecture.md](docs/architecture.md).
Images des créatures (ennemis, boss, personnages) régénérables depuis les ROMs : [docs/creatures.md](docs/creatures.md).

- `cps2-gfx` : chargement des ROMs graphiques et rendu vidéo CPS2 (tilemaps, sprites, priorités,
  palettes). **Jalon 1 atteint** : identique au pixel près à MAME sur les 11 instantanés de
  `testdata/vsnap` (`cd port && cargo test --release`).
- `vrender` : rend un instantané et le compare à la capture MAME.
- `ddsom-sound` : pilote son (séquenceur, voies, enveloppes) réécrit à partir du programme du Z80, identique
  écriture par écriture ; format et protocole dans [docs/son.md](docs/son.md). Option `Sound::use_rust_driver`
  de `cps2` pour jouer sans Z80.

Capturer de nouveaux instantanés : `VSNAP="f1 f2 ..." VSNAP_DIR=... SCENARIO=...` avec
`mame/session.lua` (voir `mame/vsnap.lua`). Points de fidélité appris en route :
- la table d'objets affichée est celle verrouillée au vblank précédent ;
- la palette est copiée au moment de l'écriture du registre CPS-A 0x0a, pas au rendu ;
- entre sprites, c'est l'entrée d'indice le plus élevé qui reste visible (bit 31 du masque de priorité).

## Jouer au port Rust

    ./jouer.sh

Touches (comme MAME) : `5` pièce, `1` Start, flèches, `Ctrl`/`Alt`/`Espace`/`Maj` (ou `W` `X` `C` `V`)
= boutons 1 à 4 ; joueur 2 : `R F D G`, `A S Q`, `6` pièce, `2` Start ; `F2` test, `P` pause,
`Tab` avance rapide, `Échap` quitter (sauvegarde l'EEPROM dans `nvram/`).

Les touches se changent dans `touches.cfg`, créé au premier lancement (`action = touches`, noms
minifb : `Left`, `LeftCtrl`, `A`, `Key1`, `NumPad0`…). Lettres et chiffres suivent la disposition
du clavier (en AZERTY, `Key1` demande `Maj`) : `DDSOM_TOUCHES=1 ./jouer.sh` affiche le nom des
touches pressées.

Sous Windows, `jouer.bat` (lançable depuis `\\wsl.localhost\...`) compile et lance `ddsom.exe` avec le
Rust Windows (MSVC) : son par waveOut et manettes Xbox par XInput (`Pad1A`, `Pad1Haut`… dans
`touches.cfg`, A B X Y = boutons 1 à 4, Start = Start, Back = pièce). `rom\` doit déjà être préparé
(par exemple par un premier `./jouer.sh`).

Son : `DDSOM_SON_RUST=1 ./jouer.sh` remplace le programme du Z80 par le pilote Rust (`ddsom-sound`).

Sans ROM : une fois les ressources extraites (ci-dessous), `port/target/release/ddsom --assets
assets` joue sans aucun fichier ROM ni 68000 (routines Rust seules, pilote son Rust) ; voir
[docs/sans-rom.md](docs/sans-rom.md).

État actuel : la machine (68000 interprété, Z80, QSound HLE, vidéo CPS2) reproduit MAME à l'identique
sur la RAM de jeu et l'image (4800 images testées avec entrées). Le code 68000 du jeu est recompilé en
Rust à la compilation (crate `ddsom-rc`, à partir des ROMs locales) ; voir docs/architecture.md.

## Extraire les ressources

    cargo run --release --manifest-path port/Cargo.toml -p extract -- rom assets

Produit `assets/` (graphismes PNG, palettes, sons WAV ; formats décrits dans `assets/README.md`),
sans perte : `cargo test --release --manifest-path port/Cargo.toml -p ddsom-assets` le vérifie.
