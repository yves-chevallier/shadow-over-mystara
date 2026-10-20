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
| K1 noyau, VBLANK, ordonnanceur | 107,0 M | 2,0 M (reste `reset` 0x350 et `boot_init`) |
| C1 crédits | 70,6 M | 0,14 M (tâche « INSERT COIN » 0xe05ec) |
| ordonnanceur et traps | 68000 | natifs (`kernel.rs`) ; la tâche du gel 0xee080 reste du 68000 |

Sur les 159 scénarios de la validation complète avant ce travail : 1,63 G d'instructions (711 par image),
710 fonctions encore exécutées, 609 entièrement natives, 1 240 jamais atteintes (86 Ko).

## Ce qui reste avant « zéro instruction 68000 », par ordre de volume

1. **Test de RAM au démarrage (K2, 18 %)** : `0x182a`/`0x1912` testent la GFX RAM, la RAM de travail et la RAM
   partagée du Z80 (poignée de main : attente de `0x77` en `0x619fff`). Effets utiles : GFX RAM `0x910000-0x913fff`
   remplie de paires (0x0020, 0), copie de 8 longs en `0x91c400`, octets impairs de `0x618001-0x619ff9` à 0xff.
   À remplacer par ces écritures ; **attention** à la validation différentielle : le test dure quelques images à ×16,
   un démarrage natif plus court décale d'autant tout le jeu ; il faut aligner les deux machines (par exemple en
   comparant à partir de la fin du démarrage de l'original, ou en faisant attendre à la native le même nombre
   d'images).
2. **`reset` 0x350 et `boot_init` 0x492 (K1)** : remises à zéro et registres CPS ; même remarque pour la durée.
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
7. **Les tâches elles-mêmes** : tâche de partie `0x1c45a`, tâche du gel `0xee080`, tâches de fondu
   (`0x1460` et suivantes), tâches d'interface. Le noyau natif les fait encore reprendre en 68000 ; voir le plan dans
   `architecture.md`.
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
