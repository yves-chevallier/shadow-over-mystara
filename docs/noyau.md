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
| 7 | réveil des tâches : état « attend » → compte décrémenté, à zéro « prête » ; la première examinée (emplacement 0, en FF0064 ; D0 = 15 dans l'original) ne décompte qu'une image sur quatre tant que `A5+0x4f` ≠ 0 | `reveiller_taches`, `Table::decompter_image` |

## Ordonnanceur et traps (0x928-0xb64, `kernel.rs`)

Voir l'en-tête de `kernel.rs` (parcours, enveloppes, traps). Fidélité : chaque emplacement examiné est une exécution
native séparée (la fenêtre d'interruption entre deux emplacements subsiste), et une tâche reprend avec les registres,
le SR et la pile qu'elle avait en appelant l'enveloppe de cession, à l'adresse qui suit cet appel.

### Table des tâches en Rust (`taches/table.rs`)

L'original range chaque tâche dans un enregistrement de 16 octets en FF0064 (état, compte, SR, PC, USP, pile) et sauve
ses 15 registres sur sa propre pile ; l'ordonnanceur empile son état sur la pile du superviseur (FF0E66, pointeur en
FF0052), note la tâche courante en FF0056 et tire les piles d'un tas (16 longs sous FF01A6, sommet en FF005A). Le jeu
réécrit tient tout cela dans une structure Rust, rangée dans le bus (`Cps2Bus::ext`) :

| `Table` | Original |
|---|---|
| `emplacements[k].etat` : `Libre`, `Attend`, `Suspendue`, `Prete`, `EnCours`, `Neuve` | octet +0 de l'enregistrement FF0064 + 0x10 k (0, 1, 2, 4, 8, 0xc) |
| `emplacements[k].compte` | octet +1 (images avant le réveil ; gardé tel quel dans les autres états) |
| `emplacements[k].pile` | long +0xc : pile 68000 donnée à la création |
| `emplacements[k].reprise` : `Entree(pc)` ou `Cession(Contexte)` | PC (+4) d'une tâche neuve ; sinon SR (+2), USP (+8) et, sur la pile, les 15 registres et l'adresse de retour de l'enveloppe |
| `courante` | FF0056 (dernière tâche lancée) |
| `piles` | tas des piles (FF005A, FF0166-FF01A5) |

Les numéros d'emplacement sont ceux de l'original divisés par 16 (`kill(0x30)` : emplacement 3). Les enveloppes qui ne
cèdent pas (0x9b6 créer, 0xa14 tuer, 0xaac suspendre, 0xac8 réveiller) ne font plus que l'opération sur la table :
l'original sauvait puis rendait tous les registres, donc ni registre ni drapeau ne change. Les cessions (0xa40, 0xb64,
0xa86) dépilent leur adresse de retour et rangent le contexte dans la table ; l'ordonnanceur le remet dans le processeur
au lancement suivant, sans passer par les reprises d'origine `0xa4c`/`0xa8c`. Les registres que l'ordonnanceur et les
traps laissent (A0 = adresse de l'enregistrement d'origine, D0.w = emplacements restants, D1.w = état, A1 = pile de la
tâche après la sauvegarde...) sont reproduits : une tâche neuve démarre avec les restes de la précédente. Les traps ne
sont pris que par leurs enveloppes, natives ; seul `trap #7` est aussi exécuté directement (0x2c3e0, menu de mise au
point), son handler 0xae6 est une native ; ceux des traps 0 à 6 paniquent s'ils sont atteints. Les natives qui
touchent la table (noyau, VBLANK, tâche du gel, démarrage) s'exécutent toujours, même quand elles débordent sur
l'interruption suivante (`tient_la_table`, `lib.rs`) : le code d'origine, qui chercherait la table en RAM, ne les
remplace jamais.

Une tâche reste « du code qui reprend à une adresse » : les tâches réécrites ont des natives à leurs adresses de
reprise (`resume!`), les autres (écran `0x19068`, suites de la carte de développement du menu TEST) reprennent dans le
code d'origine. Leurs appels en cours ne sont plus sur leur pile 68000 : voir ci-dessous.

### Appels en cours en Rust (`table::Cadre`)

Dans l'original, une tâche qui cède au milieu d'un sous-programme garde sur sa pile les adresses de retour de ses
appels en cours (la boucle de partie `0x1c468`, sa fin d'image `0x1c4c4`, la séquence de scène qui a lancé un fondu...)
et ce que ces sous-programmes ont sauvé (les 15 registres d'un fondu par composante, le D0 d'une impulsion de
verrouillage des crédits). Le jeu réécrit les range dans l'emplacement de la tâche (`Emplacement::cadres`, au plus 8) :

| Cadre | Original | Ouvert par | Fermé par |
|---|---|---|---|
| `Appel(suite)` | adresse de retour d'un `jsr`/`bsr`/`pea` | `table::appeler` : `Sequence::tail` vers une native, `push_return`, boucle de la machine à modes (`0x1341a`), pas de jeu (`0x1c468`, `0x1c4c4`), phase 12 et aiguillages des objets (`game.rs`, `dispatch.rs`), sortie de scène (aiguillage, reprises `RESUME_*`) | `mem::rts` d'une native, ou l'enveloppe de cession appelée en dernier (`jmp 0xa40`) : `table::depiler_retour` |
| `Registres` | `movem.l D0-D7/A0-A6,-(SP)` | fondus par composante (`taches/fondu.rs`) | fin du fondu (`table::reprendre`) |
| `Valeur(v)` | `move.l D0,-(SP)` | impulsion de verrouillage `0x143a6` (`tache_credits.rs`) | sa reprise `0x143c8` |

Chaque cadre retient le pointeur de pile 68000 à son ouverture : un appelé revient au cadre quand il rend la main avec
cette pile ; un appel 68000 imbriqué a toujours une pile plus basse (son adresse de retour y est empilée), ce qui
départage les deux sortes d'appels sans autre marque. Une native qui cède ne laisse donc rien sur la pile 68000, et la
reprise ne dépend plus que de la table : adresse de la native de reprise, registres et SR du contexte, cadres.

Le code d'origine qui reste ne connaît que sa pile 68000 : il y revient par `rts`, y relit ce qu'il a sauvé, et certaines
routines dépilent même les retours de leurs appelants pour revenir plus haut (un effet de bulle, `0xaa9fe`, revient
ainsi directement au début du tour de la boucle de partie `0x1c468`, par-dessus l'aiguillage des EFFCT, la mise à jour du
monde et la fin d'image ; une phase de démonstration revient de même en haut de la boucle de démonstration). Les cadres
ouverts à la hauteur de pile actuelle sont donc remis sur la pile 68000, comme l'original les aurait empilés
(`table::mettre_sur_la_pile`), chaque fois que du code d'origine prend la main avec cette pile :

* `call_at`/`call` les y remet le temps de l'appel et les reprend (relus) quand l'appelé revient normalement ; s'il
  revient plus haut ou cède la main (`Unwound`), ils restent consommés ou empilés, comme dans l'original ;
* `Sequence::jump`/`jump_to`/`tail` vers une adresse sans native, et `Ex::delegate` de la sortie de scène, les y laissent ;
* quand la tâche cède ensuite pour reprendre dans une native, ceux qui sont restés au sommet de la pile 68000 (l'appelé
  a cédé, ou est revenu plus haut en dépilant une partie d'entre eux) en sont repris (`table::relever`, dans
  `kernel::ceder`) ; ceux que la pile a dépassés ont été consommés par le code d'origine.

Une native appelée par un cadre (le dernier cadre de la tâche est à la hauteur de pile actuelle) n'est jamais remplacée
par son original quand elle déborderait sur l'interruption suivante (`run`, `table::appelee_par_un_cadre`), comme les
natives du noyau : l'original ne trouverait pas ses retours, et le pas Rust qui l'a appelée attend sa suite dans les
cadres.

Pour que la pile vue par le code d'origine soit exactement celle de l'original, les appels en cours ouverts par du Rust
pendant un pas sont tous des cadres, y compris ceux qui ne traversent jamais une cession : retour de la mise à jour du
monde dans la phase 12 (`game.rs`), retours des boucles d'aiguillage des classes d'objets (`dispatch.rs`). Seul `call_at`
empile encore une vraie adresse de retour 68000, celle de l'appel qu'il fait. `DDSOM_CHECK=1` remet de même les cadres
sur la pile de la copie qui exécute l'original.

`examples/cessions` liste, pour chaque tâche, ses reprises : profondeur de la pile 68000, contenu, et cadres tenus en Rust
(`>suite`, `R` registres, `=v` valeur). Il exécute le 68000 restant pas à pas en vérifiant qu'aucune instruction ne dépile
un cadre tenu en Rust (pile au-dessus du dernier cadre, ou `rts` à sa hauteur : `table::verifier_68000`, qui montre les
dernières instructions et natives exécutées ; `DDSOM_ARRET=pc` l'arrête avec cette trace quand le 68000 atteint une
adresse) ; c'est ainsi qu'ont été trouvés les retours directs décrits plus haut, et les états de sortie de scène
(séquences 25 et 32) qui sautaient au début de l'aiguillage d'origine et le laissaient appeler l'état suivant par son
`jsr (A1)` : ils exécutent maintenant cet état eux-mêmes (`exit.rs`, `redispatch`).

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
reprend après une cession (adresse qui suit le `jsr 0xa40`/`0xa86`). Le pas rend une `Attente` (`Images(n)` : trap 3,
`Prete` : trap 4) que `taches::ceder` remet directement au noyau (`kernel::attendre`, `kernel::ceder_prete`) : la table
range l'état, le compte, l'adresse de reprise et le contexte de la tâche, sans appel de l'enveloppe d'origine ni
adresse empilée. Les attentes des séquences d'écran (`Sequence::wait`) et des fondus, et la fin d'une tâche de fondu
(`kernel::terminer`), passent aussi directement par le noyau ; les autres cessions (`tail(0xb64, ...)`, code
d'origine) passent par les enveloppes natives, qui dépilent leur adresse de retour. Chaque emplacement porte aussi le
nom de sa tâche (`table::Tache`, d'après son entrée : `Partie`, `Gel`, `Credits`...). Une tâche Rust cohabite avec une
tâche 68000 : l'ordonnanceur ne les distingue pas. Une tâche reste libre de passer par du code 68000 (`call`, `tail`).

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
| messages de pièces (`taches/messages.rs`) | `0xe03e4` création, `0xe03fe` entrée, reprises `0xe0440`, `0xe0452` (blocs) et `0xe0478`, `0xe048a` (texte) | le message du prix clignote sur l'écran des crédits (40 images affiché, 20 effacé) |
| écran noir (`video.rs`, sous-programme) | `0xc32` entrée, reprises `0xc42` et `0xc56` | couches éteintes, une image, objets et couches effacés, une image |

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

L'enregistrement SR/PC/USP/pile est remplacé par la table Rust (`taches/table.rs`, voir plus haut), et les appels en
cours des tâches par ses cadres : toutes les reprises mesurées se font avec une pile 68000 vide (voir « Vers une boucle
sans 68000 »). Les fondus, l'écran des scores, les invites et les
scripts de transition de la sortie de scène (`scene/transition/`, `docs/scenes.md` §5) sont faits. Les « listes de
bulles des boss » n'existent pas : les boss et les objets SET qui appellent `0x9b6` (A0 = `0x14de`, `0x14c8`, `0x1ccf8`...)
lancent une tâche de fondu, déjà en Rust (`taches/fondu.rs`). Le menu TEST `0x29162` est fait aussi (`taches/service.rs` : écrans en fonctions Rust, une seule cession au milieu d'un écran,
reprise dans la native `0x291ec` ; seules les suites de la carte de développement, `A5+0x82`, jamais sur une borne, passent
encore par du 68000, `docs/service.md` §5). Tâche encore en 68000 : l'écran `0x19068` (emplacement 0xf0, créé en `0x13aa0` par un sous-mode de l'attract propre à une région, jamais atteint
par les scénarios). Les tâches Rust gardent encore leur état dans la RAM, les registres du contexte (le sens d'un fondu de luminosité est
dans D1) et leurs cadres.

Mesure (`examples/reste68k`, 66 scénarios : `scene_end/`, `cov_tour1` à `8`, `cov_service`, `modes/ending`), avant et
après la réécriture des scripts de transition, de l'entrée de la tâche d'interface, de la chute `0x3b960` et du butin
`0xbad6` : 37,61 M → 37,32 M instructions 68000 (scripts 288 979 → 0, interface 6 060 → 0, butin 1 916 → 0, chute
1 383 → 0). Les seules cessions encore faites par du code 68000 dans ces scénarios étaient celles du menu TEST
(`jsr 0xb64` en `0x2918a`, `0x291e6`), réécrit depuis.

## Vers une boucle sans 68000

La table des tâches est en Rust et, depuis les cadres (« Appels en cours en Rust »), toutes les reprises mesurées se font
avec une pile 68000 vide : une tâche qui a cédé tient entièrement dans son emplacement (adresse de la native de reprise,
registres et SR, cadres). Mesure (`examples/cessions`, reprises distinctes avec une pile 68000 non vide / reprises
distinctes, par tâche ; 8 scénarios : `cov_3p`, `covb_long3p_02`, `modes/gameover`, `ending`, `attract`, `screens`,
`scene_end/end_01`, `end_105`) :

| Tâche | Avant (main a47037e) | Après |
|---|---:|---:|
| Partie | 117 / 123 | 0 / 158 |
| Démarrage (puis machine à modes) | 119 / 144 | 0 / 148 |
| Modes (machine à modes relancée) | 9 / 11 | 0 / 11 |
| Crédits | 28 / 36 | 0 / 40 |
| Fondu | 11 / 22 | 0 / 22 |
| Gel, Interface, Invite, Scores | 0 / 32 | 0 / 32 |
| **total** | **284 / 368** | **0 / 411** |

(il y a plus de reprises distinctes après : une même adresse de reprise est comptée une fois par contenu des cadres)

Sur 100 autres scénarios passés au vérificateur (22 `cov_*` : service, les huit tours, fin de partie, crédits, morts,
trois et quatre joueurs ; les 56 `scene_end/*`, les 21 `boss/*`, `modes/ending_end`), une seule reprise garde une pile
68000 non vide, et c'est du code d'origine en cours : dans `cov_mortal_late`, un gestionnaire SET d'origine (`0xcb9c0`,
appelé par l'aiguillage des décors) lance le fondu `0x1460` et cède au milieu de la mise à jour du monde (30 images,
pile `cb9c0 39ea2 1c8ee 1c702 1c4c4 1c468`). Elle disparaîtra avec la réécriture de ce gestionnaire (familles C10, hors
de ce travail) : sa native devra appeler le fondu par `Sequence::tail` et reprendre dans une native.

L'image tourne pourtant encore sur la machine : interruption de VBLANK, parcours de l'ordonnanceur par natives
successives, reprise d'une tâche en posant son contexte dans le processeur. Pour que la boucle d'image devienne
`vblank(); ordonnanceur();` sans processeur, il manque :

1. **Plus aucune instruction 68000.** Les `call`/`call_at` vers du code d'origine (`docs/reste68k.md`, « Appels Rust qui
   exécutent encore du 68000 » ; surtout C10 SET, C04 boss, C0e EFFCT non exercés, P3, ESHL), le code d'origine où
   reprennent encore des tâches (écran `0x19068`, suites de la carte de développement du menu TEST), et les natives que
   la machine remplace par leur original : celles qui débordent sur l'interruption suivante (`run`, `lib.rs` :
   `c.cycles + cost > irq_horizon`) et celles qui se déclarent hors contrat (`families::COUT_HORS_CONTRAT`, épilogue
   étranger `0xaa9fe` des bulles). Sans processeur, une native s'exécute toujours en entier.
2. **Un exécuteur de natives sans interpréteur.** Un pas finit en posant `c.pc` sur la native suivante (reprise, suite
   d'un cadre, enveloppe de cession) et la machine la cherche (`lookup`) ; sans 68000, une boucle `while let Some(n) =
   lookup(c.pc) { (n.run)(c, b) }` jusqu'au retour à l'ordonnanceur (`0x984`) suffit, le `Cpu` ne servant plus que de banc
   de registres (D0-D7, A0-A7, SR, PC). `call_at` devient l'appel de la native, `Unwound` (abandon des fonctions Rust
   quand le code appelé cède ou revient plus haut) disparaît avec le dernier code d'origine.
3. **La pile 68000 au sein d'un pas.** Elle est vide à chaque cession, mais un pas s'en sert encore entre deux cessions :
   adresses de retour empilées par `call_at`, cadres remis le temps d'un appel de code d'origine, sauvegardes (`movem`) des
   routines natives qui imitent l'original. C'est de la RAM ordinaire (FF01A6-FF0DA5, ignorée par `validate`) : sans
   processeur elle peut rester, et disparaît quand plus aucun appelant n'est du 68000.
4. **Les registres comme interface.** Les natives se passent encore des valeurs par D0-D7/A0-A6, le contexte d'une
   tâche est fait de registres (sens d'un fondu de luminosité dans D1, bloc d'état dans A0, `Contenu::Registres` d'un
   fondu par composante), et une tâche neuve hérite des registres de la précédente (reproduits par le noyau). Ils
   disparaissent quand tous les appelants sont en Rust ; d'ici là le banc de registres du point 2 les porte.
5. **Les suites désignées par des adresses.** Une reprise, une cession et un cadre `Appel` désignent leur suite par
   l'adresse de la native de l'original (`0x1341a`, `0x14b4`...). C'est un identifiant suffisant pour la boucle du point
   2 ; l'étape suivante en fait des pas Rust nommés (`enum` d'étapes par tâche, `fn` de reprise), quand plus aucun code
   d'origine ne revient à ces adresses.
6. **Le temps de l'image.** Aujourd'hui le VBLANK est une interruption prise là où le temps de l'image s'épuise, et le
   reste de l'image est sauté quand l'ordonnanceur tourne à vide (`idle`). Sans processeur : `vblank()` en début d'image,
   puis l'ordonnanceur jusqu'à ce que plus aucune tâche ne soit prête (ou que seule la tâche du gel le soit sans rien
   avoir à faire, `gel::attente_vide`) ; l'avance rapide reste plafonnée à `MAX_STEPS_PER_FRAME` pas par image, le pas
   retenu s'exécutant au début de l'image suivante, juste après le VBLANK (ce que fait `cps2::StepCap` en `0x1c4b0`). Le
   masque d'interruptions (`trap #12`, fenêtre de `0x952`) et le SR des tâches n'ont plus de sens. L'IRQ raster (niveau
   4) n'est jamais armée par le jeu. Les 8 scénarios où la fin d'image de l'original tombe au milieu d'un passage du gel
   (« Validation » plus haut) gardent leur tolérance : l'ordre devient celui, fixe, de la boucle.
7. **Le Z80 et le démarrage, à leur place dans l'image.** Le son tourne ligne par ligne, entrelacé avec le 68000
   (`Machine::run_line`) ; les natives qui touchent la RAM partagée (`touches_sound`) doivent tenir dans leur ligne. La
   boucle sans processeur doit garder ce calendrier : tout le travail du 68000 au début de l'image (comme à ×16, où il
   finit en quelques lignes après le VBLANK), puis le Z80 ligne par ligne ; et au démarrage, les deux instants que le
   Z80 observe (relâchement en image 5 ligne 108, `boot_init` en image 5 ligne 173, `demarrage.rs`).
8. **L'oracle reste.** `validate`, `DDSOM_CHECK` et les outils de diagnostic comparent à la machine d'origine
   (68000 surcadencé) : elle reste, hors du chemin du jeu.

### Le jeu sans 68000 (`sans68k.rs`, `DDSOM_SANS_68K=1`)

Les points 2, 6 et 7 sont faits, en mode à part : `ddsom_game::install_sans_68k` fait tourner le jeu sans interpréteur
ni code recompilé. `DDSOM_SANS_68K=1` le choisit dans `validate` (machine du jeu sans 68000 contre la machine d'origine)
et dans l'application (`ddsom`) ; le mode normal (`install`) ne change pas.

* **Exécuteur.** `sans68k::pas` cherche la native posée au PC (`lookup`) et l'exécute ; la boucle d'image et
  `call`/`call_at` ne font rien d'autre (plus de `c.step`, plus de `ddsom_rc`). Le `Cpu` ne sert plus que de banc de
  registres et de pile. Une native s'exécute toujours en entier : aucune n'est remplacée par son original quand elle
  déborderait sur l'interruption (`run` n'est plus appelé), et `Ctx::jsr_origine` (familles) appelle la native au lieu
  du code d'origine. `Unwound` sert encore : une native qui cède, ou un épilogue qui revient plus haut, abandonne les
  fonctions Rust en cours comme avant.
* **Arrêt net.** Une adresse sans native, ou une native qui se déclare hors contrat (`families::COUT_HORS_CONTRAT` :
  réaction d'ennemi inconnue, décor qui recharge la scène), arrête le jeu (panique `sans 68000 : …`, que `validate`
  rapporte en `ARRÊT …` puis « ÉCHEC : arrêt sans 68000 à l'image n »). Le diagnostic donne l'adresse, l'image et la
  ligne, la tâche courante (nom, état, cadres), le SR et le sommet de la pile 68000 (les adresses de retour du code
  remplacé), les huit dernières natives exécutées et l'appel Rust en cours (`site_d_appel` : fichier et ligne du
  `call`). Les gestionnaires qui quittent par l'épilogue d'une autre routine (`Ctx::epilogue_etranger`, bulles
  `0xaa9fe`) annoncent `COUT_EPILOGUE_ETRANGER` : la machine hybride les confie encore au code d'origine, mais leur native
  reproduit ce chemin et s'exécute sans 68000.
* **Image.** La machine garde son calendrier ligne par ligne (`cps2::Machine::line_work`) ; à chaque ligne,
  `sans68k::ligne` fait le travail du processeur puis le Z80 joue sa ligne. Ce travail n'a pas de durée : l'interruption
  de VBLANK, levée en fin d'image, est prise (en Rust : SR et PC empilés, autovecteur, acquittement) dès que le SR
  l'autorise, puis les natives s'enchaînent jusqu'à ce que l'ordonnanceur tourne à vide (`idle`, seulement quand aucune
  interruption n'attend : si l'examen d'un emplacement l'a masquée, l'ordonnanceur rouvre d'abord sa fenêtre), ou que le
  pas de jeu soit retenu par le plafond de l'image (`StepCap::passage` en `0x1c4b0`, compteur remis à zéro par
  `run_frame` ; le pas retenu reprend juste après le VBLANK suivant). Tout le travail tombe donc au début de la ligne
  240, avant le Z80 : seuls le VBLANK et `0x68c` touchent la RAM partagée, et ce sont les premiers. Au démarrage, une
  native qui attend une position du faisceau (`demarrage.rs`) rend la main sans changer PC ni registres : la ligne
  s'arrête et elle réessaie à la suivante, ce qui garde les deux instants que le Z80 observe. Durées des natives,
  `idle` de la machine, surcadencement et correctifs `patches` (qui visent le code d'origine) ne servent plus.
  Plafond de sûreté : 1 000 000 natives par image (`PLAFOND_NATIVES`, une image en exécute quelques milliers) ;
  au-delà l'image s'arrête comme une image de l'original finissant au milieu d'une attente active, et `validate`
  compte ces images.
* **Diagnostic.** `DDSOM_SANS_TRACE=n` affiche chaque native exécutée pendant l'image n (ligne, PC, SR, pile).

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

## Mesure : lot « miettes » (tout sauf EFFCT et scripts de transition)

Mêmes 200 scénarios et mêmes nombres d'images des deux côtés (`DDSOM_SCENES=1 tools/reste68k.sh 30000`, 2 851 500 images ;
avant : main e29fa6b, rapport conservé dans l'historique de `docs/reste68k.md`, commit 72e3a40). Zones réservées à
d'autres travaux et exclues du lot : EFFCT (0xa8000-0xba000, `families/effects*.rs`), scripts de transition
(0xf8200-0xfa300), listes de bulles des boss (0x9b6), chute 0x3b960, 0xbad6.

| | Avant | Après |
|---|---:|---:|
| instructions 68000 (total) | 92 987 040 (32,6 par image) | 91 232 723 (32,0 par image) |
| hors zones réservées | 1 829 009 | 76 734 (-95,8 %) |
| fonctions encore exécutées / entièrement natives | 316 / 774 | 203 / 842 |
| C0c ITEM | 782 747 | 22 |
| C04 BOSS | 749 598 | 23 692 |
| C0a BOX | 67 603 | 0 |
| F3 interface joueurs | 32 920 | 1 738 |
| C08 ESHL | 26 134 | 5 985 |
| W1 boucle de partie | 23 059 | 1 063 |
| P4 noyau joueur | 22 863 | 9 374 |
| B1 caméras | 19 561 | 0 |
| C10 SET | 15 190 | 1 927 |
| C1 crédits | 13 944 | 0 |
| F1, V1, C02, V2, U1, U2, C06, C14, S1, F6, K3, M2, C12, P1 | 46 519 | 4 654 |

Restent hors zones réservées (76 734 instructions sur 2,85 M d'images) :

* gestionnaires EFFCT au-delà de 0xba000 (0xba00a, 0xba03c, 0xba18a et voisins, environ 22 000) : famille EFFCT, laissée au
  travail sur les effets ;
* boss 3 (Harpie, 0x7f41a, 10 Ko de code, environ 20 000 instructions dans le seul `scene_11`) : à réécrire comme les boss
  4 et 7 quand d'autres scénarios le feront paraître ;
* ESHL 0xa7200, 0xa66c0, 0xa31c2, 0xa036e (environ 5 500, 2 à 9 scénarios) ; P4 0x35c00-0x35d00 et 0x3b900-0x3ba00 hors
  0x3b960 (environ 7 000) ; HUD 0x1507a (1 728), boucle de partie 0x1c44a (1 044), et une traîne de routines à moins de
  1 000 instructions chacune (SET 0xd4000, 0xd90b0 ; PSHL 0x54818, 0x54872 ; SOBJ 0xe03c6 ; ENEMY 0x76ec4, 0x78a00 ; U2 0x1ad00).

Fonctions jamais atteintes par les 200 scénarios (1 514, 104 Ko), par sous-système, et avis : M1/M3/K4 (menu TEST,
Secret Test, écrans d'exception : 121) sont hors du jeu normal, à réécrire seulement si le menu de service est voulu ;
C04 (331) et C02 (196) sont surtout les boss 0 à 3 et des ennemis de scènes que les scénarios ne visitent pas (scènes 69 et
au-delà) : à couvrir par des scénarios (`gen_enemy.py`) avant réécriture ; C08 (98), C10 (105), C12 (70), P5 (72) sont des
variantes d'attaques et de décors des mêmes scènes rares, même remarque ; X1 (39) et X2 (13) sont des variantes de
collision que les natives remplacent déjà par leurs appelants (probablement mortes) ; le reste (moins de 50 par
sous-système) est à trier au cas par cas.

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
