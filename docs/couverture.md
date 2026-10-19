# Couverture des scénarios

Objectif : que les scénarios rejoués par `validate` (routines natives contre code d'origine) exécutent
l'ensemble du jeu, pas seulement le début du stage 1. Aucune donnée de ROM n'est versionnée : les
scénarios ne contiennent que des entrées et des écritures RAM (`poke`, voir `docs/rewrite-guide.md`).

## Méthode d'accès aux stages

* `FF8034` = numéro de scène (`A5+0x34`), 1 à 114 (0x72) :
  1-68 les stages (dont les boss : 24 Manticore, 26, 32 Black Dragon, 40/42 salamandres, 44, 46, 54 Tel'Arin,
  58, 64, 66-67…) et leurs scènes intermédiaires (choix de route : 14 et 38 ; scènes à décor propre 43, 47, 4b, 4d, 4f,
  50, 52 atteintes par progression naturelle), 69-82 boutiques, 83-89 écrans à graphismes non initialisés quand on
  y entre directement, 90-114 scènes de démonstration de l'attract (jouées par l'IA du mode démo).
  Un `poke8 ff8034 N` pendant la sélection du personnage (image 2400) lance la partie dans la scène N ;
  vérifié par captures d'écran pour les 114 scènes (aucun plantage, le jeu se déroule normalement).
* Saut en cours de partie : `ff8034 ← N`, `ff802c ← 8`, `ff802d ← 0`, `ff802e ← 0` (entrée de scène du jeu). Passer
  par la phase 0x10 (fin de scène) est plus fidèle mais peut attendre indéfiniment (précondition de scène).
* Boss : `+0x62` (PV) de l'objet BOSS (`FFBA1E + 0xC0 k`) ramené à 1 : le bot l'achève d'une touche, ce qui exécute
  la mort du boss et la séquence de fin de scène. Joueur invincible : `+0x62` du joueur (`FF85DE + 0x200 k`) remis à 0x70.
* Joueurs 3 et 4 : l'EEPROM d'origine déclare 2 monnayeurs (`FF8021 = 1`) ; `poke8 ff8021 3` à l'image 800 donne un
  monnayeur par joueur (Coin 1-4). Le bot lance la partie avec Start 1 (image 1100) puis les autres joueurs rejoignent.
* Boutons : Button 1 attaque, 2 saute, 3 magie / objet, 4 choix de l'objet ; Service Mode = menu TEST.

## Les scénarios (`tools/gen_scenarios.py` → `scenarios/cov/`, bot réactif `examples/autobot.rs`)

Le bot lit la RAM (joueurs, ENEMY, BOSS, ITEM, BOX) pour rejoindre l'ennemi le plus proche, s'aligner, frapper,
sauter, lancer magie et objets, ramasser objets et caisses, puis avance ; l'enregistrement est un scénario d'entrées
pur, reproduit à l'identique par `validate` (le bot tourne comme la machine B : ×16, code d'origine).
Il ne touche à aucune touche pendant l'entrée de scène (phase 8) ni la fin du compte à rebours de sélection.

| Scénario | Contenu |
|---|---|
| `cov_tour1_c0` … `cov_tour8_c1` | parcours des scènes 1-28, 29-56, 57-86, 87-114 (350 images par scène, boss achevés) ; les 6 personnages : fighter (1, 7), cleric (2, 8), elf (3), dwarf (4), magic-user (5), thief (6) |
| `cov_2p_a/b/c` | 2 joueurs (fighter+dwarf, magic-user+cleric…) sur les scènes 29-56, 1-28, 87-114 |
| `cov_3p`, `cov_4p_a/b/c` | 3 et 4 joueurs, sur chacun des tronçons |
| `cov_natural_c0` | stage 1 en jeu naturel (aucun saut), joueur invincible : combats, objets, caisses, fin de scène naturelle |
| `cov_mortal_c1`, `cov_mortal_late`, `cov_mortal_2p` | sans invincibilité : mort, écran continue (Coin + Start), reprise, sélection du personnage après continue |
| `cov_gameover_c4` | mort sans pièce : game over, retour à l'attract (qui enchaîne ensuite les scènes de démonstration 0x5a-0x72) |
| `cov_attract_coin` | attract seul 20 000 images (titre, classement, démos), puis une partie sans bot |
| `cov_service` | menu TEST (Service Mode) parcouru au hasard (seul l'écran d'entrée est atteint : 163 fonctions) |

Fin du jeu : les séquences de fin de stage et de boss sont exécutées (scènes naturelles 0x43-0x52), mais la
séquence finale (générique de fin, saisie du nom) n'a pas été identifiée avec certitude : zone non couverte.

## Couverture chiffrée (instructions 68000 distinctes, interpréteur pur, `examples/cov`)

Mesure : `tools/run_cov.py` puis `tools/cov_report.py` (dénominateur : lignes d'instruction de `listing.txt` sous
0x100000, approximatif car il contient des données mal désassemblées ; 2 559 fonctions de `decomp/index.tsv`,
fonction touchée = au moins une de ses instructions exécutée).

| Jeu de scénarios | Instructions distinctes | Fonctions touchées | % du listing |
|---|---|---|---|
| bot initial (bot30k, bot_2p, bot_char0-5) | 22 754 | 691 / 2 559 | 14,0 % |
| scénarios `cov_*` seuls | 89 659 | 1 771 / 2 559 | 54,2 % |
| union (initial + `cov_*`) | 90 017 | 1 773 / 2 559 | 54,4 % |

Par scénario (instructions) : tours 27 700-35 600, parties à 2-4 joueurs 30 800-38 700 (maximum : `cov_4p_a`), attract
33 000, natural 22 600, mortal 13 900-23 000, service 2 900. Le bot initial plafonne à 15 000-20 500 par scénario.

## Zones non atteintes

* Blocs de 4 Ko sans aucune instruction exécutée : 0x0d000, 0x12000, 0x1b000, 0x2a000 (écran d'exception, tests M1/M3),
  0x68000, 0x6b000-0x6e000, 0x78000-0x80000, 0x8b000, 0x94000, 0x9b000-0x9c000, 0xa9000, 0xbb000-0xc8000 (en grande
  partie des données mal désassemblées), 0xd1000-0xd3000, 0xef000-0xf0000.
* Menu TEST (0x29162) et « Secret Test » (M3, 0xee16e-0xf1500) : la navigation aléatoire de `cov_service` n'a pas dépassé
  l'écran d'entrée ; il faudrait connaître la séquence d'entrées de chaque écran.
* Génération des vagues d'ennemis, boss et objets rares : couverts seulement dans la mesure où le bot les rencontre
  (350 images par scène) ; pas de joueur 3/4 sur les 90-114, pas de séquence finale, pas de saisie du nom au classement.
* 788 fonctions de `index.tsv` jamais touchées (liste produite par `cov_report.py --missed`), dont beaucoup de
  handlers de tables de sauts de types d'objets rares et du code de diagnostic.

## Validation (`DDSOM_COV=1 tools/validate_all.sh`) et écarts relevés

Les scénarios `scenarios/cov/` sont longs (une heure et plus sous charge à 2-4 joueurs) : `validate_all.sh` ne les
lance que si `DDSOM_COV=1` (images : `COV_FRAMES`, 30 000 par défaut). Les 8 scénarios historiques, les `extra/` et
les `boss/` restent VALIDES. Résultat des scénarios de couverture après la campagne « registres hérités » (première
différence de la RAM entre routines natives et code d'origine, image) :

| Scénario | Résultat | Cause identifiée |
|---|---|---|
| `cov_mortal_c1`, `cov_mortal_2p`, `cov_natural_c0`, `cov_service` | VALIDE sur 30 000 images | – |
| `cov_tour1/4/5/8`, `cov_mortal_c1` | VALIDES sur 30 000 images avec `DDSOM_ADOPT_REGS=player_overlays_update,players_update` | `player_overlays_update` : D3 (et les autres registres) relu par le créateur d'effet 0xb141a (écarts aux images 5150, 4898, 4949, 10051, 10418, 3993, 20977 sans l'adoption : mots FFD5xx-FFD9xx des objets ESHL) |
| `cov_2p_c`, `cov_3p`, `cov_attract_coin`, `cov_gameover_c4` (image 29927), `cov_2p_b`, `cov_4p_*` | écarts aux images 4192, 4326, 3362/4014 | `players_update` (DDSOM_SANS le supprime ; l'adoption des registres ne suffit pas : écarts de mémoire) |
| `cov_tour2/3/6/7`, `cov_mortal_late`, `ko/boss13_dragon` (image 9991) | images 20611, 10523, 7201, 10921, 4267 | phase de jeu qui tourne sans attendre l'image (`0xa86`, cède sans attendre) : l'original surcadencé en exécute des dizaines par image (FF2A58/FF2A38, compteur de l'image FF8054) ; le nombre d'itérations dépend de la durée exacte de chaque routine, que les natives n'imitent pas (`build_sprite_list` coûte 30 000 cycles de moins que l'original sur 3 600 appels, etc.) |
| `cov_mortal_c1`, `cov_mortal_2p` | VALIDE sur 30 000 images | – |
| `cov_attract_coin` | image 21302 (3362 et 4014 corrigés) | 21302 : tempête de pas (voir plus bas), un bouton pressé à l'image 21300 |
| `cov_3p`, `cov_4p_a`, `cov_4p_b`, `cov_2p_c` | images 9850, 4739, 4444, 6394 (2992, 3845, 3152, 3011 corrigés) | `combat_update` (supprimé par `DDSOM_SANS=combat`) ; ensuite, `cov_3p` 10652 : tempête de pas ; `cov_4p_a` 7305 : registres résiduels (`build_sprite_list`, `set_0`) ; `cov_4p_b` 12015 : `set_30` ; `cov_2p_c` 8370 : `enemy_16` (Riyakaa, ffad64), puis 15960 |
| `cov_tour4_c3`, `cov_2p_b` | images 3713, 4754 | `combat_update` (supprime l'écart) |
| `cov_natural_c0` | image 18946 (stage 1) ; 6661 avec d'autres entrées | `combat_update` (mots FF1A6A, FF8648 : drapeaux de touche du joueur 1) |
| `cov_tour1/2/3/5/6/7/8`, `cov_4p_c`, `cov_gameover_c4`, `cov_mortal_late` | images 7063, 19595, 9761, 7109, 4898, 10418, 4949, 4834, 3993, 4266 | écarts de plusieurs routines à la fois (aucune désactivation isolée ne suffit) ou à identifier |
| `cov_2p_a` | n'a pas fini en plus d'une heure | – |
| `cov_service` | écart image 1443 | non analysé (menu TEST) |

Registres hérités corrigés par cette campagne (leurs lecteurs étaient des gestionnaires 68000) : `set_0` (D0-D6,
A1-A3, A6 : D4 relu par 0xad7b0, qui additionne D4 à la position des joueurs quand `A5+0x51e` = 4),
`build_sprite_list` (D4), caméras 1 et 2 (D4 = 0x3f/0x40, 0x1f/0xe0), chargeurs de palettes de scène (0x1a296,
0x1a350-0x1a36c, 0x1a3d4, 0x1a608, 0x1a678, 0x1a8ac : D0-D6, A1-A4, A6, D7 ; relus par le menu TEST en 0x2bba0), et
des écarts de mémoire révélés au passage : `set_30` (décompte de 0x80 images), adresse 32 bits des caisses saisies
(`move.l A1,(0x86,A0)` écrit A1 étendu du signe), `eshl_52` (A1 = dernier élément créé, puis A1 = fin de la boucle des
joueurs).

Outils : `DDSOM_ADOPT_REGS=préfixe,...` (registres de l'original), `DDSOM_ADOPT_MASK=hex` (seuls les registres du
masque : bit i = Di, bit 8+i = Ai, bit 16 = drapeaux), `DDSOM_ADOPT_TRACE=1` (valeurs rendues par la native et par
l'original). Dichotomie : adopter tout, puis le masque registre par registre, puis les natives par groupes.
Corrigés dans le noyau des joueurs : (1) les gestionnaires d'origine appelés par table (`actions`, `characters::run`,
`update`, `hurt`, `scripted`) comptent sur D0 = indice d'aiguillage à octet haut nul (ils font `move.b (7,A0),D0b` puis
indexent par le mot) : un D0 quelconque faisait sauter le saut du nain (0x445ba) en 0x4c1eb et écrire dans
l'enregistrement du joueur 2 ; (2) `player_overlays_update` (0x392e4) et la jauge de chargement (0xb46) laissent
maintenant les registres de l'original (D0-D6, A0-A4 / D2.w, A0) : la boucle de partie enchaîne ces routines et les
boucles de dispatch d'objets sans rien réinitialiser, et des gestionnaires lisent ces registres (un effet recopiait D3
ou D2). **Règle** : une native appelée avant une boucle de dispatch doit reproduire les registres que l'original
laisse (`DDSOM_CHECK=1 DDSOM_CHECK_ALL=1`, lignes `REGS`), pas seulement la mémoire. Diagnostic : `DDSOM_CHECK=1
DDSOM_ADOPT_REGS=préfixe,...` (avec `DDSOM_SANS` pour les dispatch et `game_phase12`, que le mode contrôle ne sait pas
rejouer) fait adopter par ces natives les registres de l'original ; si l'écart disparaît, la cause est un registre
résiduel (`cov_4p_a` : `build_sprite_list` et `set_0`).

Deux remarques sur `validate` : (1) la pile des tâches s'étend sous FF0A00 (mots FF09B0-FF09D2 vus en attract) : la zone
ignorée commence à FF09B0 ; FF0058 (dernière tâche lancée par l'ordonnanceur, écrit en 0x96c) est ignoré ; (2) une touche
pressée pendant l'entrée de scène (phase 8) ou à la fin du compte à rebours de sélection (image où la phase 4 passe à 8)
fait diverger les deux machines avec n'importe quelle routine native active (cause : même sensibilité à la durée que
ci-dessus). Le bot évite ces instants ; les scénarios existants n'y touchent pas.
Routines fautives restantes (autres modules, à corriger par leurs auteurs) : `combat_update` d'abord ; puis
`build_sprite_list` (sprites.rs) et `set_0` (registres résiduels, D4 lu par l'effet 0xad63e), `set_30`, `enemy_16`.
Relancer ensuite les scénarios (`DDSOM_SANS=nom` pour isoler, `examples/pctrace`, `examples/watch`).

Remarques sur `validate` : (1) la pile des tâches déborde sous FF0A00 (FF09B0-FF09D2 vus en attract) : la zone
ignorée commence maintenant en FF0900 ; (2) **tempête de pas** (cause élucidée de « une touche pressée pendant l'entrée
de scène fait diverger les machines ») : un bouton pressé en phase 8 (ou à l'attract) fait passer le jeu en
avance rapide deux images plus tard : une tâche auxiliaire (table des tâches, entrée 13) remet aussitôt la tâche
principale prête, qui enchaîne ses pas de jeu sans attendre l'IRQ (`world_update` 100 à 140 fois par image au lieu
d'une, mesurable avec `examples/passages`). Le nombre de pas par image vaut alors (cycles de l'image) / (cycles d'un
pas) : il dépend du coût en cycles des natives, donc diffère forcément du code d'origine (écart de `ff806e`, le
compteur d'images, d'un pas à l'autre). Le code d'origine seul est déterministe. Ces états ne sont comparables que si
la durée des natives est exacte ; le bot évite ces instants et les scénarios de couverture doivent aussi les éviter
(aucun bouton en entrée de scène). Le moteur final devra fixer explicitement le nombre de pas de cette avance rapide.

Scénarios de boss : les écarts des variantes `_mort` (images 6211, 6652, 7111, 7741, 11040) étaient cette même tempête de
pas, déclenchée par un bouton pressé ou **maintenu** (la troncature des entrées laissait des boutons enfoncés) quand le
jeu change de scène après la mort du boss ; les écarts de `game_phase12` (FFD514, FFD574, FFD692), de `combat_update`
(FF8664) et de l'effet du boss 8 avaient disparu avec les campagnes de registres résiduels. `tools/gen_boss.py` tronque
désormais les entrées avant le bouton fautif et relâche les boutons restés enfoncés : `scenarios/boss/*_mort` et
`boss12_scene40` (entier) sont VALIDES sur 12 000 images ; `boss18_scene26` est tronqué à l'image 10 900.
