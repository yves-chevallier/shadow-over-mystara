# Noyau : VBLANK, ordonnanceur, traps, crédits (K1, C1)

Code : `port/ddsom-game/src/vblank.rs`, `kernel.rs`, `tache_credits.rs`, `ecran_credits.rs`. `A5` = 0xFF8000.
Tout est validé image par image (`tools/validate_all.sh`) ; les scénarios `scenarios/cov/cov_credits_*`
exercent les écrans des crédits (compteurs séparés, partagés, gros chiffres, crédit à deux pièces, jeu gratuit).

## VBLANK (0x574, vecteur 26)

Une fois par image, avant toute tâche. Tous les registres sont sauvés et rendus (`movem` de l'original) ; la
native finit par `RTE`.

| Ordre | Action | Code |
|---|---|---|
| 1 | sortie 0x8040e1 ← `A5+0x5bc`, `A5+0x7fce` ← 0, 0x804170 ← 0x3f, base de palette 0x80410a ← 0x91c0 | `vblank` |
| 2 | mot de réglages FFFFD0 ← 0xffff (les trois drapeaux de mise au point de la ROM, octets 0xc4/0xc5/0xc7, sont nuls) | |
| 3 | compteur d'images `A5+0x6c`, indicateur « nouvelle image » FF0164 ← 0xff, FF19F2 ← 0 | |
| 4 | registres de défilement CPS-A avec une image de retard (0x722) | `registres_affichage` |
| 5 | lecture des entrées (0x80c), contrôle vidéo (0x902) | native `inputs`, `controle_video` |
| 6 | commandes vers le Z80 (0x68c), monnayeurs (0x143da), comptabilité (0x2dcf6) | natives |
| 7 | réveil des tâches : état 1 → compteur +1 décrémenté, à zéro état 4 ; la première examinée (emplacement 15) ne décompte qu'une image sur quatre tant que `A5+0x4f` ≠ 0 | `reveiller_taches` |

## Ordonnanceur et traps (0x928-0xb64, `kernel.rs`)

Voir l'en-tête de `kernel.rs` (table des tâches, états, tas de piles, traps, enveloppes). Fidélité : chaque
enregistrement examiné est une exécution native séparée (la fenêtre d'interruption entre deux enregistrements
subsiste), les piles contiennent les mêmes octets (cadres de trap, mouvements de registres), et la reprise passe
par `RTE` vers le PC sauvegardé de la tâche. Les enveloppes (`jsr 0xa40`...) sont des natives qui sauvent les 15
registres puis prennent le trap ; leurs adresses de reprise (`0xa4c`...) restaurent les registres.

## Tâche des crédits (0x13c0a)

Boucle : départs (0x141e0), état des crédits (0x13c2c), verrous (0x14290), boutons de continue (0x1436e), une
image de cession (0xb64) tant que l'interrupteur TEST n'est pas actionné (IN2+1 bit 1, actif bas), sinon démontage (0x14d8c) :
écran effacé, tâche d'interface créée (0x29162), tâche terminée. L'état des crédits prépare l'écran au premier
crédit (efface l'écran en cédant), affiche textes et chiffres (`ecran_credits.rs`), puis offre le départ à chaque
joueur (0x140b4) : bouton de départ (IN2) et assez de crédits (1 ou 2 selon `monnayeur+9`) ; le premier départ
crée la tâche de partie (0x1c45a) au lieu de l'écran des crédits. Les cessions d'origine coupent le code : chaque
morceau reprend à l'adresse de retour de l'original (macro `resume!`).

## Tâches Rust (`taches/`)

Une tâche Rust est une machine à états dont chaque *pas* tient dans une native posée à l'adresse que l'original
reprend après une cession (adresse empilée par le `jsr 0xa40`/`0xa86`). Le pas rend une `Attente`
(`Images(n)` : trap 3, `Prete` : trap 4), traduite par `taches::ceder`. Le noyau ne change pas : la table
FF0064, la pile de la tâche (15 registres sauvés, adresse de reprise) et les champs PC (`0xa4c`/`0xa8c`) et USP
sont ceux de l'original, donc `validate` les compare sans exception supplémentaire (la pile reste dans la zone
FF0900-FF0FFF ignorée, les octets +0..+3 de chaque enregistrement aussi) et une tâche Rust cohabite avec une tâche 68000 :
l'ordonnanceur ne les distingue pas. Une tâche reste libre de passer par du code 68000 (`call`, `tail`).

| Tâche | Natives | Rôle |
|---|---|---|
| partie (`taches/partie.rs`) | `0x1c45a` entrée, `0x1c468` tour, `0x1c48c` cession, `0x1c4b0` pas de jeu | boucle de jeu, voir `boucle.md` §1 |
| gel (`taches/gel.rs`) | `0xee080` entrée, `0xee0fa` et `0xee16a` après cession | avance rapide : décompte la tâche principale quand un joueur presse un bouton |
| fondus de luminosité (`taches/fondu.rs`) | tâches `0x14ba`, `0x14c8`, `0x14d0`, `0x14de`, `0x1536`, `0x153e` ; reprises `0x152c`, `0x158c` | un pas de ±0x1000 par cession, puis fin de la tâche (`0x9f8`) ; voir `video.md`, « Fondus » |
| fondus par composante (`taches/fondu.rs`) | tâches `0x1ccf8`, `0x1ce64`, `0x1ced4` (et `0x1cf12`) ; reprises `0x1cd1e`, `0x1ce8a`, `0x1cefe` | vers le blanc, une couleur ou une palette par page |
| écran des scores (`taches/ecran_fixe.rs`) | `0x1835e` entrée, `0x18384` après cession | tableau des meilleurs scores de l'attract (F4), voir `modes.md` §4 |
| interface joueurs (`hud/`) | `0x150ac` entrée, `0x15146` attente du début de partie, `0x15178` tour, `0x15212` après cession | emplacement 0x30 : panneaux, bannières, choix du personnage (`hud.md`) |
| invites (`taches/invite.rs`) | `0xe03fe` entrée, `0xe0440`/`0xe0452`, `0xe0478`/`0xe048a` après cession ; `0xe03c6` arrêt, `0xe03e4` création | emplacement 0xd0 : « INSERT COIN(S) » / « CONTINUE » clignote (40 images affiché, 20 effacé), texte selon le réglage des pièces |
| menu TEST (`taches/service.rs`) | `0x29162` entrée, `0x29190` tour, `0x2918a` retour du gestionnaire, `0x291ec` reprise du menu | menu de service et ses écrans, tout en Rust (`service/`, `docs/service.md`) |

Les **sous-programmes** qui cèdent la main sont traités de même : entrée et adresse qui suit chaque cession sont des
natives, et le `rts` final rend la main à l'appelant (68000, ou native de reprise d'un appelant Rust, qui l'a appelé par
`Sequence::tail`). Fondus de luminosité `0x144c`, `0x1460`, `0x146c` (reprise `0x14b4`) et par composante `0x1ccc2`,
`0x1ce30`, `0x1ce9e` (reprises `0x1cce8`, `0x1ce54`, `0x1cec4`).

Le pas de jeu s'arrête *sur* `0x1c4b0` (`GAME_STEP`) au lieu d'enchaîner : c'est là que la machine plafonne les pas par
image.

**Attente active sautée.** La tâche du gel est toujours prête : sans autre travail, l'ordonnanceur la relançait
~1 000 fois par image à ×16 (16 000 instructions 68000 par image, 95 % de tout le 68000 exécuté). `idle` (`lib.rs`)
avance maintenant jusqu'à l'interruption quand `taches::gel::attente_vide` reconnaît que son prochain pas ne ferait
rien (seule tâche prête, cédée par `0xa86` depuis l'une de ses deux cessions, dernière lancée, pas de bouton qui
réveillerait la tâche principale). `idle` testait l'indicateur de nouvelle image à FF8164 au lieu de FF0164 (il valait
presque toujours vrai : l'attente active n'était jamais sautée) ; corrigé. `examples/idle_check.rs` vérifie, sur deux
machines d'origine, que sauter l'attente ne change pas l'état (RAM hors zones ignorées, GFX, sprites, palette) : identique
sur 12 000 images de six scénarios (attract, crédits, entrée de scène, jeu à deux, fin).

Prochaine étape : remplacer l'enregistrement (SR/PC/USP/pile) par une `Option<Box<dyn Tache>>` suppose que les routines
appelées par un pas ne cèdent plus par le 68000 (`call`/`Unwound`). Les fondus, l'écran des scores, les invites et les
scripts de transition de la sortie de scène (`scene/transition/`, `docs/scenes.md` §5) sont faits. Les « listes de
bulles des boss » n'existent pas : les boss et les objets SET qui appellent `0x9b6` (A0 = `0x14de`, `0x14c8`, `0x1ccf8`...)
lancent une tâche de fondu, déjà en Rust (`taches/fondu.rs`). Le menu TEST `0x29162` est fait aussi (`taches/service.rs` : écrans en fonctions Rust, une seule cession au milieu d'un écran,
reprise dans la native `0x291ec` ; seules les suites de la carte de développement, `A5+0x82`, jamais sur une borne, passent
encore par du 68000, `docs/service.md` §5). Tâche encore en 68000 : l'écran `0x19068` (emplacement 0xf0, créé en `0x13aa0` par un sous-mode de l'attract propre à une région, jamais atteint
par les scénarios). Les tâches Rust gardent encore leur état dans la RAM et les registres sauvés par la cession (le sens
d'un fondu de luminosité est dans D1).

Mesure (`examples/reste68k`, 66 scénarios : `scene_end/`, `cov_tour1` à `8`, `cov_service`, `modes/ending`), avant et
après la réécriture des scripts de transition, de l'entrée de la tâche d'interface, de la chute `0x3b960` et du butin
`0xbad6` : 37,61 M → 37,32 M instructions 68000 (scripts 288 979 → 0, interface 6 060 → 0, butin 1 916 → 0, chute
1 383 → 0). Les seules cessions encore faites par du code 68000 dans ces scénarios étaient celles du menu TEST
(`jsr 0xb64` en `0x2918a`, `0x291e6`), réécrit depuis.

## Démarrage (0x350, 0x182a, 0x1912, 0x492 ; `demarrage.rs`)

Interruptions masquées, pile du superviseur en FF0E66 (zone de piles ignorée par `validate`) :

| Étape | Effets utiles |
|---|---|
| `reset` 0x350 | sorties 0x400000-0x40000A, CPS-A (bases 0x9100/0x9140/0x9180, défilements nuls), 0x804040 ← 0x0F00 (Z80 au reset), RAM de travail effacée, `A5+0x70` ← mot 0xC2 du programme, contrôle vidéo, effacement des objets et des trois plans (0xc58, 0xca8, 0xcc8, 0xce8). Si le bit 14 de 0x804030 est nul (jamais : il vaut 0xe021), `A5+0x82` ← 1 et lecture de la configuration (0x2dc9a, puis 0x2e8e4 si la combinaison 0x3304 de IN2 est pressée) |
| 0x182a | 8 longs du programme (0x19f0) en 0x91C400, 0x804170 ← 0xF, base de palette 0x91C0 (la palette est verrouillée à cet instant), cases (0x20, 0) dans 0x910000-0x913FFF |
| `ram_test_block` 0x1912 | GFX RAM 0x910000-0x91BFFF puis RAM de travail : chaque mot reçoit 0x8888, 0x4444, 0x2222, 0x1111 et **retrouve sa valeur** (aucun effet). Appelé sans pile (`lea ret(pc),a6 ; bra`) |
| 0x18cc | 0x804040 ← 0x0F08 (Z80 relâché), `A5+0x18` ← 8, CFFD ← 0, attente de 0x77 en CFFF (le Z80 l'écrit dans la ligne même de son relâchement) |
| 0x1950 | octets impairs de la RAM partagée : 0 puis 0x55 partout, puis 0xFF jusqu'à 0x619FF9 (CFFD-CFFF gardent 0x55) |
| `boot_init` 0x492 | CFFD ← 0x88, CFFE et CFFF ← 0xFF (le Z80 sort de son attente), `A5+0x16..0x18` (image du port 0x804040), DSW, tas des piles des 16 tâches (16 longs de 0x534 empilés sous FF01A6, sommet en FF005A), réglages 0x2d30a, `move #0x2000,SR` (le VBLANK en attente est pris aussitôt), tâche de démarrage 0x11b16 dans l'emplacement 0, ordonnanceur 0x928 |

La branche d'erreur (« RAM ERROR !! » écrit en GFX RAM par 0x19ae, puis boucle infinie en 0x1948 ou 0x19a2) n'est jamais
prise sur la machine émulée : elle n'est pas réécrite. La remise à zéro logicielle 0x28cd0 (`jmp 0x350` après une
exception du processeur, K4) est hors périmètre.

**Durée.** Mesurée par `examples/demarrage` (code d'origine à ×16 ; identique sur tous les scénarios : aucun ne donne
d'entrée avant l'image 600, et l'EEPROM est la même) :

| Jalon | Image (0 = première), ligne |
|---|---|
| reset | 0, 240 (début de l'image) |
| test de la GFX RAM, puis de la RAM de travail | 0, 73 à 5, 108 |
| Z80 relâché (0x18cc), 0x77 reçu | 5, 108 ; 5, 109 |
| `boot_init` (0x492) | 5, 173 |
| premier VBLANK (après les réglages 0x2d30a), ordonnanceur | 5, 222 ; 5, 223 |

La version native fait tous les effets d'un coup au reset, puis **attend la même position du faisceau** (image et
ligne, champ `Cps2Bus::frame` ajouté pour cela) avant de relâcher le Z80 et avant `boot_init` : ce sont les deux
instants que le Z80, qui démarre en parallèle, peut observer (son relâchement, puis 0xFF en CFFF qui le fait entrer
dans sa boucle principale). Entre les deux, la RAM partagée reçoit son état final dès la poignée de main au lieu
de passer par 0 et 0x55 : le Z80, à son initialisation, ne lit que CFFF (en attente de 0xFF) et sa routine
d'interruption ne touche à la boîte aux lettres que si CFFD vaut 0x88. Le calendrier est celui de la référence à ×16,
quelle que soit la fréquence du 68000. Les registres que le test laisse (longs de la palette, motif 0x5555, adresses
de fin de bloc) sont rendus à `boot_init` (`registres_du_test`). Contrôle : `demarrage <roms> <eeprom> 40 compare`
compare les deux machines image par image, son compris (RAM partagée, RAM et registres du Z80, que `validate` ne
regarde pas) : tout est identique dès l'image 3, le son dès la première.

**Validation.** Pendant les images 1 à 5 (`DEMARRAGE` dans `examples/validate.rs`), celles qui finissent pendant le
test de l'original, le mot en cours de test peut porter un motif à la fin de l'image (0x1111 au lieu de 0x0020 en
0x910B34 à l'image 1). `validate` y tolère **un mot** de différence au plus, en RAM de travail ou en GFX RAM ; tout le
reste (autres mots, objets, palette) est comparé dès l'image 1, et tout dès l'image 6. Seules les images 1 et 2 ont
ce mot (test de la GFX RAM) ; aux images 3 à 5, la fin d'image tombe entre deux mots du test de la RAM de travail.

## Bilan de l'inventaire (`tools/reste68k.sh`, `docs/reste68k.md`)

`examples/reste68k.rs` rejoue des scénarios sur la machine hybride (natives Rust, 68000 interprété pour le
reste) et compte chaque instruction 68000 exécutée hors native (code appelé par `call` compris), par adresse,
puis par fonction (`decomp/index.tsv`) et par sous-système (tableau 4.1 de l'atlas). Le temps passé dans
l'ordonnanceur, les traps et la tâche du gel (0xee080) est compté à part : à ×16 le processeur y tourne tout le
temps que le jeu n'utilise pas (400 milliards d'instructions sur 2,3 millions d'images), ce n'est pas du travail.

Mesure sur 40 scénarios (`scenarios/cov/`, `scenarios/modes/`, quatre bots ; 894 000 images), avant et après :

| | Avant (base 5f04d32) | Après |
|---|---|---|
| instructions 68000 (hors attente active) | 654,6 M (603 par image) | 433,8 M (485 par image) |
| K1 noyau, VBLANK, ordonnanceur | 107,0 M | 2,0 M (reste `reset` 0x350 et `boot_init`, réécrits depuis : voir « Démarrage ») |
| C1 crédits | 70,6 M | 0,14 M (relecture des entrées des démonstrations 0xe05ec, réécrite depuis : `modes/demo.rs`) |
| ordonnanceur et traps | 68000 | natifs (`kernel.rs`) |
| tâche du gel 0xee080, boucle de partie 0x1c45a | 68000 | Rust (`taches/`) |

Gain des tâches Rust (8 scénarios de 8 000 images, avant/après) : attente active 1 031,6 M instructions (16 100 par
image) à **0** ; hors attente 658 à 650 par image (la boucle de partie : ~9 par image). Attente comprise : 16 760 à
650 instructions 68000 par image. Sur les 121 scénarios de `reste68k.sh` (2,15 M d'images) : 526 par image, attente
nulle (avant, sur 40 scénarios : 485 hors attente et 14 500 d'attente). Temps de la machine de référence de `idle_check`
divisé par 2 à 3 (12 000 images).

Sur les 159 scénarios de la validation complète avant ce travail : 1,63 G d'instructions (711 par image),
710 fonctions encore exécutées, 609 entièrement natives, 1 240 jamais atteintes (86 Ko).

## Ce qui reste avant « zéro instruction 68000 », par ordre de volume

1. ~~**Test de RAM au démarrage (K2, 18 %)**~~ et 2. ~~**`reset` 0x350 et `boot_init` 0x492 (K1)**~~ : faits, plus
   aucune instruction 68000 au démarrage (`demarrage.rs`, section « Démarrage » ci-dessous).
3. ~~**Pools d'objets (O2, 9 %)**~~ : fait. `0x1ee1a`/`0x1ef6c` (insertion dans les listes triées par profondeur,
   la plus chaude du jeu) sont installées avec les registres exacts de l'original (`objects::lists::Reinsert`,
   `docs/objets.md`) : D0-D2, A1-A4 et drapeaux sont relus par le handler suivant.
4. **Interface joueurs (F3, 9,5 %)** : `0x15216`, `0x156fa`, `0x17814`, `0x175a8`, `0x1728e`, `0x16c1a`
   (panneaux d'état : PV, score, magies) ; tâche `0x150ac` créée en fin de phase 0.
5. **Défilement et caméras (B1, 4,2 %)** : fait pour `0x33fe4`/`0x3403c` (`terrain.rs`, `docs/joueur.md`),
   `0x31616` et `0x33e20` (`scroll.rs`, `docs/video.md`). Reste `0x30308`, `0x309e0`, `0x2ede0`, `0x32108` et les
   scripts de mode des caméras : une centaine d'adresses à 1-3 % chacune du 68000 restant du sous-système.
6. **EFFCT et IA** : faits `0xb9eba`, `0xb9ee6`, `0xba094` (`families/effects.rs`) et `0xfbae`, `0xfbb0`, `0xe76c`,
   `0xf0e8` (`ai.rs`). Reste le texte V2 (`0x28d8`, `0x1386`), la liste de sprites V4 (`0x1de96`), la FIFO son
   S1 (`0xedf80`) et, hors de toute fonction de l'index (`tools/reste68k.sh` les compte à part : la ligne « hors de toute fonction
   de l'index » de `docs/reste68k.md`, plus de la moitié du reste), les tâches d'interface F3 (`0x15620`, `0x150ac`, `0x15f9e`...), la
   cession de la boucle de partie (`0x1c48c`) et le gestionnaire d'effet de barrière (`0xb9e78`).
3. **Pools d'objets (O2, 9 %)** : `0x1ee1a`/`0x1ef6c`, insertion dans les listes triées par profondeur
   (la plus chaude du jeu, appelée par les natives des familles).
4. **Interface joueurs (F3)** : fait, `hud/` (`docs/hud.md`). Avec le texte V2, les pop-ups U1 et l'éclair U2
   (`0x28d8`, `0x1386`, `0x2810`, `0xd32`, `popups.rs`, `palette.rs`) : environ 50 instructions 68000 par image de
   moins (F3 46, V2 4, U1 0,2, U2 0,25 sur l'échantillon de 40 scénarios), soit 10 % du reste ;
   `0x1de96` n'a jamais été du 68000 (l'`rts` en `0x1df12` est partagé avec `0x1ee1a`).
5. **Défilement et caméras (B1, 7,4 %)** : `0x33fe4`, `0x31616`, `0x33e20`, `0x30308`, `0x309e0`, `0x2ede0`.
6. **EFFCT (C0e, 2,7 %)** : `0xb9eba`, `0xb9ee6` (3 scénarios seulement), puis IA E1 (`0xfbb0`, `0xe76c`),
   texte V2 (`0x28d8`, `0x1386`), liste de sprites V4 (`0x1de96`), FIFO son S1 (`0xedf80`).
7. **Les tâches restantes** (la partie, le gel, les fondus, l'écran des scores, les invites et l'interface joueurs sont en
   Rust) : menu TEST `0x29162`, écran `0x19068` (jamais atteint). Le noyau natif reprend encore chaque tâche par son
   enregistrement (PC, pile) ; voir le plan dans `architecture.md`.
8. Les 1 294 fonctions jamais atteintes (88 Ko) : à classer en code mort (menu TEST, écrans de debug, K4 : hors
   périmètre) ou à couvrir par des scénarios (boss et ennemis rares, ESHL, SET) avant de les réécrire.

## Mesure : routines du monde réécrites avec leurs registres

Instructions 68000 exécutées hors natives (hors attente active) sur 10 scénarios (`bot30k`, `bot_2p`, `bot_char0`,
`bot_char3`, `cov_tour1_c0`, `cov_tour6_c5`, `cov_service`, `cov_mortal_2p` à 30 000 images, `modes/ending` et
`modes/gameover` à 12 000 ; mêmes scénarios et mêmes nombres d'images des deux côtés, `tools/reste68k.sh`) :

| | Instructions | Par image |
|---|---:|---:|
| avant (main a33691e) | 97 846 065 | 370,6 |
| après | 80 722 543 | 305,8 (-17,5 %) |

Gain par routine : `0x1ee1a`/`0x1ef6c` (-10 M), `0x33fe4` (-2,7 M), `0xfbae`/`0xfbb0` (-1,9 M), `0x33e20` (-0,7 M),
`0x31616` (-0,6 M), `0xe76c` (-0,4 M).
