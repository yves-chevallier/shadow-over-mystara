# Atlas du code 68000 de Dungeons & Dragons: Shadow over Mystara (ddsom)

Carte du programme principal (`prog_dec.bin`, 0x000000-0x0FFFFF) pour répartir la réécriture en Rust.
Document produit par analyse en lecture seule ; rien n'a été modifié ailleurs.

Convention : **(h)** = hypothèse non vérifiée ; « rôle inconnu » = aucun indice. Les adresses sont en hexadécimal.
`A5+n` = `0xFF8000 + n` (n signé : `A5-0x7190` = `0xFF0E70`).

## 0. Ce qu'il faut retenir

1. **Le code atteignable fait ~619 Ko** (≈ 11 990 points d'entrée distincts), mais `decomp/index.tsv` (2 559 fonctions, 188 Ko)
   n'en couvre que **28 %** (173 Ko). Les **72 % restants (447 Ko)** sont des *handlers d'états d'objets* atteints uniquement par
   des tables de sauts (`jmp (tbl,PC,Dn)`, `movea.l (tbl,PC,Dn),A1 ; jsr (A1)`) : ils sont dans `listing.txt` (parfois désalignés,
   ~28 Ko) mais n'ont ni fonction Ghidra ni `.c`. **L'unité de travail réaliste n'est pas la fonction mais le « type d'objet »**
   (≈ 310 entrées de tables, voir §5 et annexe A).
2. **Architecture** : une tâche principale (slot 0) qui alterne boucle de *modes* (attract/titre/écrans) et boucle de *partie* ;
   ~8 tâches annexes ; un moteur d'**objets en 10 pools de classes** mis à jour par 10 boucles de dispatch (`0x398e2-0x39f82`),
   plus 4 enregistrements joueur de 0x200 octets. Le jeu lui-même nomme les pools (chaînes de l'écran d'exception, 0x28ff3-0x2911a) :
   `PSHL, PEFF, ENEMY, BOSS, ESHL, SET, EFFCT, ITEM, BOX, SOBJ`.
3. **Plus gros blocs** : boss 116 Ko, ennemis 98 Ko, effets 47 Ko, joueurs (6 personnages) 71 Ko, son 43 Ko (≈ 390 wrappers
   quasi identiques → réécriture pilotée par table), projectiles ennemis 34 Ko, objets « SET » 35 Ko, tirs/effets joueur 36 Ko.
4. **Ordre conseillé** : feuilles indépendantes (son, texte/palettes, crédits, EEPROM, tests) → modèle d'objets (pools, listes,
   animation, sprites, dispatch, collisions) → joueurs + tirs joueur → familles ennemies (parallélisables par type) → scripts de
   stage/écrans → boucle principale. Détail et estimations au §7.

## 1. Méthode, sources, limites

* **Décodeur 68000 maison** sur `prog_dec.bin` (opcodes déchiffrés partout) : longueurs d'instruction validées à 99,2 % contre
  `listing.txt` sur le code exécuté (`mame/coverage.bin`). Exploration récursive depuis : vecteurs, points d'entrée de tâches,
  tables de sauts relatives au PC (mot : `jmp/jsr (X,PC,Dn)` ; long : `movea.l (X,PC,Dn),A1`) et 10 tables de dispatch d'objets.
  Les tables lues par PC-relatif sont lues dans `prog_dec.bin` ; les données lues par registre d'adresse (chaînes, pointeurs de
  noms) dans `prog_plain.bin`.
* **Validation** : 89 % des mots exécutés pendant les sessions de couverture MAME (111,8 Ko sur 125,5 Ko) sont dans le code atteint ;
  89 % des 4 833 entrées apprises à l'exécution (`port/ddsom-rc/entries.txt`) aussi. Le reste est surtout des tables lues par PC
  (comptées « exécutées » par le tap) et des sauts calculés non résolus dans 0x15700-0x17B00 (sélection/HUD) et 0x30000-0x33000
  (scrolling par stage). Les tailles ci-dessous sont donc des **minorants** pour ces deux zones.
* **Tailles** : « code atteint » = octets d'instructions atteints ; « étendue » = taille de la plage d'adresses (inclut tables
  de données incrustées dans le code, nombreuses dans les handlers).
* **Noms** : issus des chaînes ROM du jeu (écran d'exception, test, tables de debug) quand ils existent, sinon descriptifs.
* Les numéros « sous-système » (K1, V2, C02…) sont définis par plages d'adresses (annexe B) : toute fonction de l'index dont
  l'adresse tombe dans une plage appartient à ce sous-système.

## 2. Tâches et flot de contrôle

### 2.1 API de l'ordonnanceur (0x928, 16 tâches de 16 octets à 0xFF0064)

| Entrée | Rôle (vérifié dans le listing) |
|---|---|
| `0x9b6 task_create` / `trap #0` (0x9c2) | D0 = n° de slot × 0x10, A0 = entrée ; état ← 0x0C |
| `0x9f8` / `trap #1` | termine la tâche courante |
| `0xa14` / `trap #2` (0xa20) | tue le slot D0 |
| `0xa40` / `trap #3` (0xa52) | attend D0 images (D0 += `~DSW & 7`) ; appelle d'abord `0xb40` |
| `0xa86` / `trap #4` (0xa92) | cède la main (état 4 = prêt), utilisé par la tâche de gel `0xee080` |
| `0xaac`/`0xac8`/`0xae4` (traps 5-7, 0xab8/0xad4/0xae6) | modifient l'état d'un autre slot (suspendre / réveiller avec paramètre) |
| `0xb64` | `0xa40` avec D0 = 1 (une image) |

L'IRQ VBLANK (`0x574`) décrémente les compteurs d'attente (état 1 → 4 quand 0), avec ralentissement par `A5+0x4f`.
Dans la réécriture, chaque tâche devient une machine d'états / coroutine ; seul le comptage d'images compte (pas la pile).

### 2.2 Points d'entrée de tâches (recherche `lea/movea.l #X,A0 ; trap #0 / jsr 0x9b6`)

| Slot | Entrée | Créée par | Rôle | Preuve |
|---|---|---|---|---|
| 0x00 | `0x11b16 task0_main` | `boot_init` 0x492 (0x52c) | init (palettes, RAM, sons), écrans d'avertissement région (« WARNING…Japan/USA/Mexico/… », 0x128a6-0x13202, « Parental Advisory »), crée 0x13c0a et 0xee080, puis boucle de modes `0x133fe` | listing 0x11b16-0x11cb4 |
| 0x20 | `0x13c0a` | 0x11c9e | **crédits / start / continue** : boucle `141e0` (pièces), `13c2c` (démarrage/rejoindre/continue pour 4 slots de 0x10 octets en `A5+0x44e`), `14290`, `1436e`, `14d74` (sortie vers le menu test quand l'interrupteur service 0x804021 bit1 change) | listing + `14e3a` |
| 0xE0 | `0xee080` | 0x11cae | **gel/pause** : tourne tant que `FFFFD0 & 0x80` (mot DSW) ; fait avancer les compteurs d'attente des autres tâches et rend la main via `0xa86` (h : « freeze » de l'écran) | listing |
| 0x00 (remplace) | `0x133fe` (boucle, pas une tâche séparée) | `0x11cb4` (jmp) | **machine à modes** : `FF0E70` (mode 0/2) → `FF0E71` (sous-mode, pas de 2) → `FF0E72` ; appelle le handler puis `0xa40` | §2.3 |
| 0x00 (remplace) | `0x1c45a` | 0x1400a / 0x140ae (début de partie) | **boucle de partie** : par image, saute à la *phase* `(A5+0x2c)` (table 0x1c502), réinitialise les 5 compteurs de listes de collision (`FFFE72-7A`) et leurs tableaux (`FFFE7C-84`) | §2.4 |
| 0x00 (remplace) | `0x29162` | 0x14e3a | **menu TEST** (« T E S T M E N U ») : cf. M1 | chaînes 0x29f22+ |
| 0x30 | `0x150ac` | 0x1c69a, 0x1f8a2 | **panneaux d'état joueurs / HUD (h)** : efface `A5+0x30e…` (4 × 0x50 octets par joueur), boucle `0x16c86` (clear tilemap 0x912008, `15216`/`17814`/`175a8`) | listing |
| 0xF0 | `0x1835e` | 0x13740 (mode 0/6) | **exécuteur d'objets pour écrans fixes** : appelle seulement les dispatch des classes EFFCT et SET + `0x1de96` (sprites) – logos/titre (h) | listing 0x1835e-0x18384 |
| 0xD0 | `0xe03fe` | 0xe03e4 | messages « INSERT n COINS / CONTINUE REQUIRES ONLY 1 » (0xe04e8) | chaîne + code |
| 0xC0 | `0xf7dfe` | 0xf798a | écrit 4 emplacements par joueur dans une table (A6) – rôle inconnu (h : curseurs/portraits) | listing |
| 0x10 | `0x14ba`, `0x14c8`, `0x14d0`, `0x14de`, `0x1536`, `0x153e` | ~80 sites (fondus) | **fondu de palette** (paramètres `A5+0x48e` masque de pages, `+0x48f` nombre, `+0x490` délai, `+0x492…` pointeurs cibles) | listing 0x14ba-0x1560 |
| 0x10 | `0x1ced4`, `0x1ccf8` | 0x1cc42, 0xb6a00, 0xd9480… | fondu vers cible par interpolation 1 niveau/étape (`0x1cfc8`) | listing |

### 2.3 Machine à modes (`FF0E70-72`, table 0x13424 / 0x1345c)

`FF0E70` ∈ {0 : init (0x13428), 2 : exécution (0x13450)}. En exécution, `FF0E71` (pas de 2) indexe 11 sous-modes (table 0x1345c) :

| FF0E71 | Handler | Contenu |
|---|---|---|
| 0, 6 | `0x1371e` | crée la tâche 0x1835e (écran fixe), attend l'indicateur `FF0F53`, fondu puis passe au suivant (h : avertissement/logo) |
| 2 | `0x13566` → `0xfa296` | écran titre/copyright (textes « 1996 TSR… CAPCOM », 0xfa8a2) |
| 4, 14 | `0x13472` → `0x1f32e` | interpréteur de scripts de scène (démo, §F6) |
| 8 | `0x1377e` | écran avec 2 blocs tuiles (index 0x1c0/0x1c1) + fondus |
| 10 | `0x13478` → `0x20926` | variante de l'interpréteur de scènes |
| 12 | `0x1356c` → `0xfaef2` | écran titre 2 |
| 16 | `0x1384c` | idem 0x1377e (blocs 0x1c2/0x1c3), conditionné par `A5+0x70 == 2` |
| 18 | `0x13922` | « Parental Advisory / Animated violence » (0x13b52) |
| 20 | `0x13ade` → `0xfc576` | écran 4 (palette 0x53, tuiles 0x91c3e0…) |

Les libellés de debug de la séquence d'attract sont dans le code (0x13554-0x13b46 : `shuukyaku demo 1`, `titile demo 1`, `play demo 1`,
`ranking 1`, `nazo hint`, …, `capcom demo`) mais sont des blocs de texte (format `d5e`), pas une table de contrôle.

### 2.4 Phases de partie (`A5+0x2c`, table 0x1c502 de 8 entrées, appelées par 0x1c45a)

| A5+0x2c | Handler | Sous-états (`A5+0x2d`) | Rôle |
|---|---|---|---|
| 0 | `0x1c522` | 5 (table 0x1c532) | initialisation de partie : crée 3 objets SET (variantes 0x3e/0x3f/0x40, animations de palette), `35288/3525c` |
| 4 | `0x1ca20` | 8 (table 0x1ca2e → 0x1ca3e…0x1d440) | **sélection des personnages** (h fort) : compte à rebours affiché (0x1d828 → tilemap 0x912428), règle anti-doublon (0x1d714), curseur `charsel_cursor` 0x1d584 |
| 8 | `0x20c38` | 2 (0x20c4a, 0x20e54) | transition/entrée de stage (h) ; zone 0x20c00-0x23800 pleine de sous-scènes à fondus |
| 12 | `0x1c6a0` | – | compte à rebours BCD (`A5+0x39/0x3a`, `sbcd`) puis crée la tâche 0x150ac : écran continue (h) |
| 16 | `0xf3142` | 36 (table 0xf31ea → 0xf3232…0xf5782) | séquences de fin/transition de stage : fondu, `A5+0x34++`, repasse en phase 8 ; point de départ par stage via table 0xf3170 |
| 20 | `0x1c80c` | – | fin de partie : fondu, vide tout (`c32`, `1c44a`), retour au mode attract (`133dc/133f6`) |
| 24 | `0x21c2a` | 2 (0x21c4e, 0x21da4) | scène spéciale avec `2aec` (entrées) (h) |
| 28 | `0xfb292` | 26 (table 0xfb2aa → 0xfb6a6…0xfc3f0) | scène à étapes (h) |

`A5+0x34` est l'**identifiant de scène/stage** (comparé à 2, 4, 6, 8, 0xc, 0x14, 0x29, 0x43, 0x45, 0x53, 0x5b, 0x6f…).
Les étapes de phase 16 appellent `1c86e` (mise à jour du monde), `260ee` (collisions) et `1de96` (sprites) : ce sont des
séquences de jeu à scénario, pas des écrans figés.

### 2.5 Ordre de travail d'une image

1. **VBLANK `0x574`** : `FFFFD0` ← DSW ; `0x722` (écriture des registres de scroll CPS-A depuis `A5+0x4b6/4ba/506/50a/556/55a` avec décalage d'une image,
   `0x804108/10c/110/114`, flip ligne…) ; `0x80c` lecture des entrées ; `0x902` (`0x804122` ← `A5+0x12`, `0x400002`) ; `0x68c` envoi **d'une** commande
   son au Z80 (FIFO de 64 × 16 octets en `A5-0x5aa4`, têtes `FFA95C/E`, poignée de main `0x61801f`, octets 0x618001…0x618019) ;
   `0x143da` pièces ; `0x2dcf6` (met à 1 le bit 0 de `0x664000` si `A5+0x82`, rôle inconnu) ; `0x65e` ; décrément des attentes de tâches ; `0xb46` (jauge de charge : écrit D1 dans la 1re ligne du scroll 2 `0x912000`, inhibée par DSW bit 0x400).
2. **Tâche courante** (boucle de partie) → phase → `0x1c86e` : `0xb3e` (jauge de charge, D1 = marqueur 0x500001/0x450002/0x530003 entre les groupes), `0x35292` (4 joueurs : état +4/+5/+6),
   `0x392e4` (sous-objets joueur), boucles de dispatch `0x398e2` (PSHL), `0x3996c` (PEFF), `0x39aec`/`0x39a52` (BOSS/ENEMY), `0x39b70` (ESHL),
   `0x39cba` (EFFCT), `0x39e86` (SET), `0x39e50` (BOX), `0x39f34` (ITEM), `0x33e20` (caméras/scroll), `0x39f82` (SOBJ).
   Variantes de l'ordre pour `A5+0x44`≠0 (cinématique) et `A5+0x34` ≥ 0x45/0x53.
3. Les handlers appellent `0x1ee1a` (réinsertion triée par profondeur) ; puis `0x260ee` (collisions, appelé par les étapes de scène) ;
   puis **`0x1de96` construit la liste de sprites** (OBJ RAM 0x708000, double tampon `A5+0x5bc`→`0x8040e1`) ; `0xa40` rend la main.

## 3. Modèle de données

### 3.1 Carte de la RAM (A5 = 0xFF8000)

| Adresse | Contenu | Source |
|---|---|---|
| FF0064-FF0163 | table des 16 tâches (état, compteur, SP, USP…) | scheduler |
| FF0E70-72, FF0E6E | machine à modes ; compteur d'attente de démarrage | 0x133fe, 0x11b16 |
| FF1A1A… (`A5-0x55e6`) | pointeur de pile privée des wrappers son (sauvegarde de registres) | 0xee044, 0xe0be2 |
| FF255C-FF29xx (`A5-0x5aa4`) | FIFO commandes son 64 × 16 o ; `FFFFA95C/E` tête/queue | 0xe0836/0xe0848, 0x68c |
| FF8000-FF800F | entrées : courant (0-5), précédent (6-B), fronts (C-F) | `read_inputs` |
| FF8012 / FF8014 | miroirs de `0x804122` / `0x804166` (activation couches CPS-B) | 0xbc6, 0xbfc |
| FF8010, FF8017, FF8018 | bits compteurs de pièces (lockout) | 0xb96, 0xbac |
| FF801A | état du générateur pseudo-aléatoire (graine 0x1c3) | 0x19a80/0x19a88 |
| FF801E-FF8022 | réglages monnayeur (pièces/crédit…) | code crédits |
| FF8025, FF8026 | mode continue ; flip écran | 0x14xxx, 0x902 |
| FF802C-FF802E | phase / étape / sous-étape de partie | §2.4 |
| FF8030, FF8031 | « partie active » ; masque des joueurs présents (bit k) | 0x13c2c, 0x2e58 |
| FF8033-FF8036 | drapeaux de scène ; **FF8034 = n° de scène** ; FF8035 = nb de joueurs-1 (h) ; FF8036 = caméra 2 | |
| FF803D, FF8042-44, FF804A, FF804F | pause/gel, tests de sauts d'IA, cinématique (`FF8044`), frein de tâches | |
| FF8043 | masque d'entrées appliqué aux joueurs (0x3F en jeu, 0 en cinématique) | `in_mask` |
| FF806C / FF806E | compteur d'images VBLANK / de partie | 0x574, 0x1c45a |
| FF8070 | réglage utilisateur (langue/difficulté, ambigu) | 0x11be4, 0x4002 |
| FF844E-FF48D | 4 × 0x10 : crédits / état « start » par joueur | 0x13c2c |
| FF848E-FF849E | commande de fondu (masque, nombre, délai, pointeurs source) | §2.2 |
| FF84B6/BA, FF8506/0A, FF8556/5A | scroll plan 1, 2, 3 (X, Y) ; **caméras** `FF84FE` et `FF854E` (X en +8, Y en +0xC) | 0x722 |
| FF859E-FF85B2 | valeurs de scroll de l'image précédente | 0x722 |
| FF85B8 | budget de sprites restants (0x36A à l'init) ; FF85BC bascule OBJ RAM | 0x1de96 |
| FF85BE/C0, C2/C4, C6, CE, D6 | têtes/queues des listes d'objets triées | 0x1ee1a, 0x1de7c |
| **FF85DE + 0x200·k** | **joueurs P1-P4** (voir docs/joueur.md) ; sous-objets « P*WEAPON F/B » en `+0x800`/`+0x860` (0x60 chacun) | 0x392e4 |
| FF90DE… FFF95E | 10 pools d'objets (§3.2) | 0x225e-0x2408 |
| FFFE5E-FFFE70 | compteurs de places libres des pools (un mot par classe) | 0x2200-0x2250 |
| FFFE72-FFFE7A, FFFE7C-84, FFFE86… | files de candidats aux collisions (5 catégories) et leurs tableaux de pointeurs | 0x1ed2, 0x26150 |
| FFFF92 | compteur 5 bits servant d'index aux tables de décision d'IA | 0xa4b4, 0x1f300 |
| FFFFCE | « liste de sprites déjà construite cette image » | 0x1de96 |
| FFFFD0 | mot DSW/IO lu en VBLANK (bit 0x80 gel, 0x400 etc.) | 0x574 |

### 3.2 Les 10 pools d'objets (alloc 0x225e-0x2408, init 0x2062-0x21bc, dispatch 0x398e2-0x39f82)

Chaque objet : `+0x00` actif, `+0x01` drapeaux (bit0 visible/à l'écran, bit1 caché), `+0x02` **type** (index de handler), `+0x03` variante,
`+0x04/05/06/07` états, `+0x28` code de classe (2, 4, … 0x14). Le dispatch fait `jsr (table + type·4)`.

| Code (+0x28) | Pool (nom du jeu) | Base | Places × taille | Compteur libre | Alloc / libération | Dispatch | Types (table) |
|---|---|---|---|---|---|---|---|
| 6 | **PSHL** (tirs joueur) | FF90DE | 18 × 0x60 | FFFE5E | 0x225e / 0x21d8 | 0x398e2 | 25 (0x39908) |
| 0x12 | **PEFF** (effets joueur) | FF979E | 48 × 0x60 | FFFE60 | 0x228e / 0x21e6 | 0x3996c | 48 (0x39992) |
| 2 | **ENEMY** | FFA99E | 22 × 0xC0 | FFFE62 | 0x22bc / 0x21f4 | 0x39a52 | 29 dont 1 nul (0x39a78) |
| 4 | **BOSS** | FFBA1E | 3 × 0xC0 | FFFE6C | 0x23a8 / 0x2234 | 0x39aec | 23 (0x39b12) |
| 8 | **ESHL** (tirs/attaques ennemis) | FFBC5E | 38 × 0x60 | FFFE64 | 0x22ea / 0x2202 | 0x39b70 | 64 (0x39b96) |
| 0x10 | **SET** (décor, mises en place) | FFCA9E | 32 × 0x50 | FFFE66 | 0x2348 / 0x2210 | 0x39e86 | 34 (0x39eac) |
| 0xE | **EFFCT** (effets) | FFD49E | 38 × 0x60 | FFFE68 | 0x2318 / 0x2218 | 0x39cba | 64 (0x39ce0) |
| 0xC | **ITEM** | FFE2DE | 32 × 0x60 | FFFE6E | 0x23d8 / 0x2242 | 0x39f34 | 10 (0x39f5a) |
| 0xA | **BOX** (caisses/coffres, h) | FFEEDE | 21 × 0x80 | FFFE6A | 0x2378 / 0x2226 | 0x39e50 | 4 (0x39e76) |
| 0x14 | **SOBJ** (objets système/UI) | FFF95E | 16 × 0x50 | FFFE70 | 0x2408 / 0x2250 | 0x39f82 | 16 (0x39fa8) |

Libération : `0x1f0be` (retrait des listes triées) puis incrément du compteur (0x21d8-0x2250) et remise à zéro (`0x2448/2454/245c/246c/2474`).
Chaînes ROM correspondantes : « PSHL WORK » = FF90DE, « PEFF WORK » = FF979E, « ENEMY WORK » = FFA99E, « BOSS WORK » = FFBA1E, « ESHL WORK » = FFBC5E,
« SET WORK » = FFCA9E, « EFFCT WORK » = FFD49E, « ITEM WORK » = FFE2DE, « BOX WORK » = FFEEDE, « SOBJ WORK » = FFF95E (table 0x290a6-0x29124).

Champs communs observés (pool objets) : `+0x08` X (mot, long 16.16 pour joueurs), `+0x0C` hauteur Z, `+0x10` profondeur Y (clé de tri),
`+0x14/16/18/1C/1E` vitesses/accélérations (8.8), `+0x20` pointeur de script d'animation, `+0x24/+0x26` précédent/suivant dans la liste triée,
`+0x2A/2B` drapeaux de palette, `+0x2C` orientation (flip), `+0x2D` index de table de vitesses, `+0x2E` groupe de priorité sprite,
`+0x30` compteur d'animation, `+0x32` drapeaux de script, `+0x34` pointeur de cadre de sprite, `+0x38/3C/40/44` données d'attaque/boîtes,
`+0x48/4A` objets liés, `+0x4C` table de vitesses, `+0x50` propriétaire/cible (pointeur RAM 16 bits), `+0x62` PV (joueur/ennemi).

### 3.3 Joueurs

4 × 0x200 en FF85DE (docs/joueur.md) : `+0x04` état principal (0 scénarisé (h), 2 jeu normal, 4, 6 mort/suite (h)), `+0x05` sous-état, `+0x06` action (26 actions, table 0x35586),
`+0x03` personnage 0-11 (paires de variantes : fighter, cleric, elf, dwarf, magic user, thief ; noms ROM 0x1ddd4), `+0x62` PV, `+0x6C/6D/6E` entrées.
Tables par personnage (12 entrées, indexées par `+3`) : 0x359cc (3f0ae, 406d4, 42b0c, 45148, 4619e, 49498) et 0x35a0a (3f9bc, 41f22, 444ca, 45988, 4879a, 49d46).

### 3.4 Matériel

CPS-A 0x804100-0x804116 (scroll), 0x804122/0x804166 (couches), OBJ RAM 0x700000/0x708000 (double tampon via 0x8040E1),
GFX RAM 0x910000 (scroll1), 0x912000 (scroll2, texte), 0x914000, 0x918000, palettes 0x91C000-0x91CFFF (4 pages de 0x400), 0x400000-0x40000B,
RAM Z80/QSound 0x618000-0x619FFF (commandes 0x618001…0x618019, poignée de main 0x61801F, état 0x619FFB/FD/FF), EEPROM/entrées 0x804000-0x80404x.

