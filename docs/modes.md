# Machine à modes, écrans hors jeu et fin de partie

Code : `port/ddsom-game/src/modes/` (`mod.rs` : boucle et outils de reprise ; `boot.rs`, `attract.rs`, `title.rs`,
`demo.rs`, `ranking.rs`, `select.rs`, `gameover.rs`, `ending.rs`). `A5` = 0xFF8000. Les tables (textes, scripts,
palettes) restent dans la ROM et sont lues à l'exécution. Les rôles marqués (h) sont des hypothèses.

## 1. Vue d'ensemble

Hors partie, la tâche 0 enchaîne des *écrans*. Couches :

| Couche | Code d'origine | Rust |
|---|---|---|
| démarrage : initialisations, avertissements de région, création des tâches | `0x11b16` | `boot.rs` |
| machine à modes : boucle et aiguillage sur les sous-modes | `0x133fe` | `mod.rs` |
| écrans (un sous-mode chacun) : titres, avertissements, démonstrations, classement | `0x1371e`... `0xfc576` | `attract.rs`, `title.rs`, `demo.rs`, `ranking.rs` |
| partie : sélection des personnages (phase 4), fin de partie (phase 20), générique (phase 28) | `0x1ca20`, `0x1c80c`, `0xfb292` | `select.rs`, `gameover.rs`, `ending.rs` |

La partie elle-même (phases 0, 8, 12, 16, 24) est décrite dans `boucle.md` et `scenes.md` ; la tâche des crédits
(`0x13c0a` : pièces, départ, continue, affichage de CREDIT) tourne en parallèle dans sa propre tâche : seule la
lecture des monnayeurs est réécrite (`credits.rs`) ; sa boucle et son affichage ne le sont pas.

## 2. La boucle (`0x133fe`)

Bloc d'état `FF0E70` (`mode::BASE`) : `+0` mode (0 initialisation, 2 exécution), `+1` sous-mode (pas de 2, l'écran en
cours), `+2` étape de l'écran, `+3` sous-étape, `+8` minuterie (mot), `+0x32` drapeau de démarrage (octet haut du
pointeur de script des démonstrations, voir §4).

```
0x133fe  FF0EAF ← 0xff, FF0EAE ← 0
0x1340a  tour : appelle le gestionnaire du mode (table 0x13424) ; il revient en 0x1341a
0x1341a  D0 ← 1, cession d'une image (0xa40), reprise en 0x13422
0x13422  retour au tour
```

* mode 0 (`0x13428`) : passe au mode 2 ; sous-mode de départ 2 (le titre), ou 20 (l'écran de fin de partie) si le
  drapeau de démarrage est levé ; minuterie 300.
* mode 2 (`0x13450`) : aiguille sur le sous-mode (table `0x1345c`).

Le drapeau de démarrage est levé par la phase 20 (`0x133f6`) et par la fin du générique, baissé en sortant de l'écran
de fin de partie (`0x133f0`).

| Sous-mode | Écran | Rust |
|---|---|---|
| 0, 6 | tableau des meilleurs scores (tâche `0x1835e`, objets EFFCT et SET), puis fondu | `attract::fixed_screen`, `taches/ecran_fixe.rs` |
| 2, 12 | titres (Capcom, « Dungeons & Dragons », droits) | `title.rs` |
| 4, 14 | démonstrations scénarisées (interpréteur `0x1f32e`) | `demo.rs` |
| 8, 16 | image de deux blocs de tuiles, 300 images (16 : seulement si `A5+0x70` = 2) | `attract::legal_screen`, `second_legal_screen` |
| 10 | démonstration de jeu (interpréteur `0x20926`) | `demo.rs` |
| 18 | mention « violence animée », 120 images | `attract::notice_screen` |
| 20 | fin de partie : classement qui défile, scores | `ranking.rs` |

Le cycle de l'attract dure 12 200 images : titre (sous-mode 2, 2 400 images), démonstrations (4), présentation de
personnages (6), droits (8), démonstration de jeu (10), second titre (12), démonstrations (14), puis 16 qui se
désactive et reboucle.

## 3. Cessions : le découpage des écrans

Un écran cède la main à l'ordonnanceur en plein milieu (attente d'une image, d'un fondu). Une native ne pouvant
céder, chaque écran est découpé aux points de cession : la native saute dans l'ordonnanceur en donnant l'adresse que
l'original reprend (celle qui suit le `jsr 0xa40`, `Sequence::wait`), et une autre native, posée à cette adresse (macro
`resume!`), continue. Quand l'original appelle une routine qui peut céder (`0x1460` et `0x144c` : fondus avec
attente ; `0xc32` : effacement vidéo), la native fait de même avec `Sequence::tail(adresse, reprise)`. Un appel `bsr`
qui peut céder est un vrai appel (`push_return`) et le retour a lui aussi sa native (`title::R_T18_AFTER`). Ces appels
ne sont plus empilés sur la pile 68000 : leur suite est rangée dans les *appels en cours* de la tâche
(`taches::table::Cadre`, voir `noyau.md`, « Appels en cours ») et le `rts` d'une native (`mem::rts`) y revient ; la
boucle de la machine à modes y range de même le retour de son gestionnaire (`0x1341a`). Les états dont l'original
quitte par un saut dans une autre routine (`jmp`, `bra` vers la fin d'un autre état) utilisent `Sequence::jump_to`.

Quelques pièges rencontrés : les tables lues relativement au PC par l'original (`lea (...,PC)` suivi d'un `move` du
registre d'adresse est une lecture de **données**, en clair ; `move.w (table,PC,D0w)` est une lecture du programme
**déchiffré**) ; les registres `D0` qui valent le compteur d'une boucle `dbf` quand l'allocateur d'objet le lit ;
`A0` laissé sur l'entrée d'une tâche après `jsr 0x9b6` ; les adresses relatives à A5 (`A5-0x54f0` = `FF2B10`, et non
`FFAB10`).

## 4. Les écrans

**Titre** (`title.rs`, sous-mode 2) : 16 étapes (`FF0E72`, pas de 2) — palette et décor (0), apparition des objets du
logo à intervalles lus dans une table (2), fondus (4, 12, 14), éclairs blancs (8, 18), décor du bas (18), glissement de la
caméra (22), palette du logo (24), texte de droits selon la région `A5+0x70` (28) et sortie (30). Sous-mode 12 : second
titre, 5 étapes, qui réutilise le texte de droits et la sortie. Les objets lisent et écrivent le bloc d'état par le
pointeur `+0x4c` de leur enregistrement.

**Tableau des scores** (`taches/ecran_fixe.rs`, sous-modes 0 et 6) : la machine à modes crée la tâche `0x1835e`
(emplacement 0xf0) et attend son indicateur `FF0F53`. La tâche (bloc d'état `FF0F50`) pose le décor et les rangs 5 à 10,
attend 0x40 images, dessine les rangs 1 à 4 et le titre, fait monter la couche 2 pendant 0xe0 images, attend 0x90 images,
lève l'indicateur et se termine. Lignes du tableau : 14 octets (niveau, personnage, nom de six lettres, score BCD,
emblème), `FF0FD0` (premier), `FF0FDE` (2 à 4), `FF104E` vers le bas (5 à 10).

**Démonstrations** (`demo.rs`) : un *script* de la ROM (pointeur long à `FF0EA2`, deux scripts alternés pour le sous-mode
4/14, un seul pour le 10) est une suite d'entrées de 32 octets : opération (`0` scène, `2` commande son puis scène,
`0x80..` fin, autre : présentation d'un personnage), numéro de scène, durée, quatre joueurs (personnage, variante, trois
paramètres facultatifs), un long. Une scène de démonstration fait tourner le jeu : à chaque image l'interpréteur appelle
la phase courante (8 entrée de scène, 12 le jeu mené par l'IA, `0x1fa38` : `demo_phase_12`) jusqu'à la fin de la durée.
**Deux pas de jeu par image** : un pas sans céder quand l'image vient de commencer (octet `A5-0x65d8`), un pas après la
cession. La présentation d'un personnage charge deux blocs de tuiles, crée les objets du cadre et du portrait et écrit
le texte (cadre de 18 × 17 tuiles et texte vertical, tuile par tuile).

**Sélection des personnages** (`select.rs`, phase 4) : étapes (`A5+0x2d`) 0 écran et objets curseurs, 2 curseurs et
décompte (30 s), 4 attente de 20 images, 6 joueurs figés (routines d'initialisation par personnage, table `0x1d0f6`)
et fondu, 8 second écran, 10/12 annonce clignotante, 14 fondu final et phase 8. Variables : `FF99FE` décompte (octet haut :
secondes ; `FF99FF` : images), `FF9A00` masque des cases prises, `FF9A02` douze octets : l'occupant (numéro de joueur ou
0xFF) de chaque case — deux joueurs au plus par case —, `FF9A0E` cases validées, `FF9A12` nombre de joueurs présents,
`FF9A13` « un joueur vient de valider ». Chaque joueur a un octet `+3` : sa case (bit 7 : validée).

**Fin de partie** (`gameover.rs`, `ranking.rs`) : la phase 20 tue la tâche de l'interface (`0x30`), fondu, vide les pools,
lève le drapeau de démarrage, puis *remplace* la tâche par la machine à modes (`jmp 0xae4`, A0 = `0x133fe`), qui garde
la pile de la tâche. L'écran de classement (sous-mode 20, bloc d'état `FF2B10`) fait défiler le texte du classement
(pointeur `+6`, une ligne tous les 40 pixels, `0x517ba`), affiche les scores des joueurs (objets `0xb1a7a`), puis se
ferme après 300 images ou un bouton (`0x133f0` baisse le drapeau de démarrage).

**Générique** (`ending.rs`, phase 28, bloc d'état `FF2B00`) : 26 états. Le boss est vaincu, des dialogues (messages 89
à 91) et des écrans fixes se succèdent (états 0 à 28), le générique défile (28 à 40), les récompenses de chaque personnage
sont distribuées (46, 48) et la phase 20 reprend (50). Le mot `+0` du bloc est une minuterie, `+2` le lien vers l'objet de
dialogue en cours, l'octet `+8` un signal que les objets du générique modifient. L'état 0 tombe dans l'état 2.

## 5. Validation et couverture

Scénarios (`scenarios/modes/`, ≤ 12 000 images, branchés dans `tools/validate_all.sh`) :

| Scénario | Contenu |
|---|---|
| `attract.txt` | aucune entrée : démarrage, titres, démonstrations, présentations, droits, second titre (12 000 images) |
| `screens.txt` | force les sous-modes 16 et 18 (écrans de droits rares) par `poke` de `FF0E71` |
| `gameover.txt` | début de `bot_char4` : sélection, partie, mort, écran de classement, retour à l'attract |
| `ending.txt` | saut dans la scène 105 (`tools/gen_modes.py`), boss final affaibli : générique jusqu'à l'état 36 |
| `ending_end.txt` | comme `ending.txt`, puis saut à l'état 38 : fin du générique, classement |

Les scénarios par défaut (`bot*`, `mash*`, `boss/`) traversent le démarrage, le titre et la sélection ; `cov_attract_coin`,
`cov_service`, `cov_gameover_c4` et `cov_mortal_*` le reste. Non réécrits : la tâche des crédits (`0x13c0a`), le menu
TEST (M1, `0x29162`) et le « Secret Test » (M3), la boucle des phases (`0x1c45a`), et la fin de la phase 28
au-delà de ce qu'exercent les deux scénarios ci-dessus.
