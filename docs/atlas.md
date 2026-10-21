# Atlas du code 68000 de Dungeons & Dragons: Shadow over Mystara (ddsom)

Carte du programme principal (`prog_dec.bin`, 0x000000-0x0FFFFF) pour répartir la réécriture en Rust.
Document produit par analyse en lecture seule ; rien n'a été modifié ailleurs.

Convention : **(h)** = hypothèse non vérifiée ; « rôle inconnu » = aucun indice. Les adresses sont en hexadécimal.
`A5+n` = `0xFF8000 + n` (n signé : `A5-0x7190` = `0xFF0E70`).

## 0. Ce qu'il faut retenir

1. **Le code atteignable fait ~619 Ko** (≈ 12 000 points d'entrée distincts), mais `decomp/index.tsv` (2 559 fonctions, 188 Ko)
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
| 0x30 | `0x150ac` | 0x1c69a (fin de la phase 0, après `A5+0x34 ← 1`), 0x1f8a2 | **interface joueurs : sélection de personnage / panneaux d'état (h)** : efface `A5+0x30e…` (4 × 0x50 octets par joueur), boucle `0x16c86` (clear tilemap 0x912008, `15216`/`17814`/`175a8`) | listing |
| 0xF0 | `0x1835e` | 0x13740 (mode 0/6) | **tableau des meilleurs scores de l'attract** (Rust : `taches/ecran_fixe.rs`) : étape de l'écran, puis dispatch des classes EFFCT et SET + `0x1de96` (sprites) | listing 0x1835e-0x18384 |
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
| 0 | `0x1c522` | 5 (table 0x1c532 ; dernière étape 0x1c63c-0x1c69a) | initialisation de partie : crée 3 objets SET (variantes 0x3e/0x3f/0x40, animations de palette), `35288/3525c` ; la dernière étape met `A5+0x34 ← 1` (premier niveau), efface score/état (`A5+0x37/0x38`), passe en phase 4 (`A5+0x2c += 4`) et crée la tâche 0x150ac |
| 4 | `0x1ca20` | 8 (table 0x1ca2e → 0x1ca3e…0x1d440) | **sélection des personnages** (h fort) : compte à rebours affiché (0x1d828 → tilemap 0x912428), règle anti-doublon (0x1d714), curseur `charsel_cursor` 0x1d584 |
| 8 | `0x20c38` | 2 (0x20c4a, 0x20e54) | transition/entrée de stage (h) ; zone 0x20c00-0x23800 pleine de sous-scènes à fondus |
| 12 | `0x1c6a0` | – | compte à rebours BCD (`A5+0x39/0x3a`, `sbcd`, 0x59 → 0x00) : écran continue/temps limité (h) |
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
| FF2A1A (`A5-0x55e6`) | pointeur de pile privée (descendante, au-dessus du FIFO) où les wrappers son sauvegardent leurs registres | 0xee044, 0xe0be2 |
| FF255C-FF295B (`A5-0x5aa4`) | FIFO commandes son 64 × 16 o ; `FFFFA95C/E` tête/queue | 0xe0836/0xe0848, 0x68c |
| FF8000-FF800F | entrées : courant (0-5), précédent (6-B), fronts (C-F) | `read_inputs` |
| FF8012 / FF8014 | miroirs de `0x804122` / `0x804166` (activation couches CPS-B) | 0xbc6, 0xbfc |
| FF8010, FF8017, FF8018 | bits compteurs de pièces (lockout) | 0xb96, 0xbac |
| FF801A | état du générateur pseudo-aléatoire (graine 0x1c3) | 0x19a80/0x19a88 |
| FF801E-FF8022 | réglages monnayeur (pièces/crédit…) | code crédits |
| FF8025, FF8026 | mode continue ; flip écran | 0x14xxx, 0x902 |
| FF802C-FF802E | phase / étape / sous-étape de partie | §2.4 |
| FF8030, FF8031 | « partie active » ; masque des joueurs présents (bit k) | 0x13c2c, 0x2e58 |
| FF8033-FF8036 | drapeaux de scène ; **FF8034 = n° de scène** ; FF8035 = nombre/mode de joueurs (h) ; FF8036 = caméra 2 | |
| FF803D, FF8042-44, FF804A, FF804F | pause/gel, tests de sauts d'IA, cinématique (`FF8044`), frein de tâches | |
| FF8043 | masque d'entrées appliqué aux joueurs (0x3F en jeu, 0 en cinématique) | `in_mask` |
| FF806C / FF806E | compteur d'images VBLANK / de partie | 0x574, 0x1c45a |
| FF8070 | réglage utilisateur (langue/difficulté, ambigu) | 0x11be4, 0x4002 |
| FF844E-FF848D | 4 × 0x10 : crédits / état « start » par joueur | 0x13c2c |
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
| FFFFD0 | mot DSW/IO lu en VBLANK (bits testés : 0x80 gel, 0x200 ombres, 0x400 jauge de charge, 0x1000 exécution de l'IRQ) | 0x574, 0x1ec4e |

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


## 4. Sous-systèmes

### 4.1 Vue d'ensemble

« Code atteint » = octets d'instructions trouvés par l'analyse ; « Entrées » = points d'entrée distincts (appels + tables) ;
« Fn index » = fonctions de `decomp/index.tsv` situées dans les plages (cette colonne définit l'affectation des 2 559 fonctions, cf. annexe B).

| ID | Sous-système | Plages (hex) | Code atteint Ko | Étendue Ko | Entrées | Fn index | Appelle surtout |
|---|---|---|---|---|---|---|---|
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 0-b40, ee080-ee16e | 2,1 | 3,1 | 28 | 17 | V1, M2, K2, C1 |
| K2 | Auto-test RAM (« RAM ERROR !! ») | 1800-1a32 | 0,2 | 0,6 | 2 | 2 | - |
| K3 | Entrées vers enregistrements joueur | 2aec-2c16 | 0,3 | 0,3 | 3 | 3 | - |
| K4 | Exceptions et écran de dump debug | 28600-29160 | 1,2 | 2,9 | 18 | 16 | K1 |
| V1 | Primitives vidéo (attente, couches, effacements) | b40-d18 | 0,4 | 0,5 | 13 | 13 | K1 |
| V2 | Texte, rectangles et blocs de tuiles | d18-1460, 24a8-2aec | 2,3 | 3,5 | 30 | 33 | P4 |
| V3 | Palettes et fondus | 1460-1800, 1a296-1aa00, 1ccc2-1d100 | 2,4 | 3,9 | 47 | 38 | K1, F5, W0, W1 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 1de7c-1ee1a | 2,4 | 4,0 | 15 | 15 | O1 |
| B1 | Scrolling / caméras par scène | 2ede0-35200 | 5,6 | 25,6 | 37 | 42 | V2, V1, P4 |
| O1 | Animation, tables de vitesse, physique, culling | 1a32-2062, 2c16-2d9e | 1,1 | 2,0 | 27 | 22 | - |
| O2 | Pools d'objets et listes triées | 2062-24a8, 1ee1a-1f200 | 2,0 | 2,1 | 41 | 41 | - |
| O3 | Aides communes aux types (animations par type) | 3bb00-3e400 | 8,6 | 10,5 | 209 | 25 | O1, C0e, U1, S1 |
| U1 | RNG et files d'affichage (pop-ups) | 19a80-1a296 | 1,0 | 2,1 | 18 | 18 | V2 |
| U2 | Petites routines / tables (rôle inconnu) | 1aa00-1c44a | 0,6 | 6,7 | 18 | 17 | - |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 4200-11b16 | 5,4 | 55,6 | 73 | 69 | U1, O1, O2, X2 |
| X1 | Collisions (boîtes, coups, poussée) | 26100-28600 | 6,4 | 9,5 | 39 | 39 | C0e, S1, U1, X2 |
| X2 | Dégâts, terrain, placement | 3600-4200 | 1,6 | 3,1 | 23 | 19 | U1, V2, O2 |
| W0 | Dispatch des 10 classes d'objets | 398e2-39ff0 | 0,4 | 1,8 | 10 | 10 | - |
| W1 | Boucle de partie et phases 0/4/12/20 | 1c44a-1ccc2, 1d100-1d584 | 3,1 | 3,3 | 20 | 3 | W0, K1, V4, O2 |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 21000-23800 | 7,1 | 10,2 | 167 | 1 | S1, W1, V4, K1 |
| W3 | Scripts de scène (phases 16/28) | f2e00-fa300, fb200-100000 | 26,8 | 49,9 | 455 | 82 | K1, V2, V3, V4 |
| P1 | Initialisation des joueurs | 2d9e-3600 | 1,0 | 2,1 | 26 | 23 | X2, U1 |
| P3 | Comportement scénarisé des joueurs par scène | 23800-26100 | 8,7 | 10,5 | 258 | 3 | O1, P4, O2, S1 |
| P4 | Noyau joueur (états, 26 actions génériques) | 35200-398e2, 39ff0-3bb00 | 14,9 | 25,1 | 307 | 59 | O1, O3, S1, C0e |
| P5 | Code par personnage (6 × 2 variantes) | 3e400-49e00 | 44,5 | 47,6 | 783 | 73 | O1, S1, P4, C12 |
| P6 | Réactions aux coups, saisie des coups spéciaux | f1500-f2600 | 2,0 | 4,4 | 9 | 9 | P4, U1, C10 |
| C06 | PSHL : tirs du joueur | 51c64-54900 | 10,2 | 11,4 | 175 | 39 | O1, O2, S1, U2 |
| C12 | PEFF : effets du joueur | 49e00-51c64 | 26,0 | 32,4 | 426 | 107 | O1, O2, S1, U1 |
| C02 | ENEMY : ennemis | 54900-79400 | 97,7 | 150,3 | 2340 | 205 | O1, E1, U1, V2 |
| C04 | BOSS : boss | 79400-9dbfc | 115,6 | 149,5 | 2427 | 353 | E1, O1, S1, U1 |
| C08 | ESHL : tirs/attaques ennemis | 9dbfc-a8000 | 34,2 | 42,0 | 811 | 127 | O1, O2, S1, E1 |
| C0e | EFFCT : effets | a8000-ba310 | 47,2 | 74,5 | 1116 | 170 | O1, O2, S1, U1 |
| C10 | SET : décors/animations de palette | ba310-d99f0 | 34,7 | 128,7 | 566 | 131 | O2, V3, V2, S1 |
| C0a | BOX : caisses/coffres (h) | d99f0-dabca | 3,5 | 4,6 | 60 | 7 | O2, O1, B1, S1 |
| C0c | ITEM : objets ramassables | dabca-dd61e | 8,4 | 10,8 | 135 | 14 | O1, O2, C0e, C10 |
| C14 | SOBJ : objets système/UI | dd61e-e0400 | 7,5 | 11,7 | 208 | 25 | O2, O1, S1, U1 |
| C1 | Crédits, pièces, start/continue | 13c0a-14e50, e0400-e0836 | 4,7 | 5,8 | 22 | 22 | K1, V2, V1, S1 |
| M1 | Menu TEST (service) | 29160-2d300 | 6,1 | 16,8 | 135 | 76 | V1, V2, M2, K1 |
| M2 | EEPROM, configuration, comptabilité | 2d300-2ede0 | 4,7 | 6,9 | 103 | 60 | M1 |
| M3 | « Secret Test » (debug) et Block Test | ee16e-f1500, f2600-f2e00 | 8,0 | 15,2 | 164 | 29 | V1, V3, V2, V4 |
| S1 | Son : wrappers de commandes + FIFO | e0836-ee080 | 42,9 | 55,4 | 460 | 410 | M1 |
| F1 | Tâche de boot, avertissements région | 11b16-13400 | 0,5 | 6,4 | 5 | 5 | U1, K1, V1, M2 |
| F2 | Machine à modes (attract) | 13400-13c0a, 14e50-15000 | 0,8 | 2,5 | 11 | 0 | K1, V2, V3, V1 |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 15000-18300 | 5,8 | 13,1 | 66 | 49 | M2, O2, S1, V1 |
| F4 | Exécuteur d'écrans fixes (logos) | 18300-19a80 | 2,5 | 6,0 | 10 | 5 | V3, V1, O2, V2 |
| F5 | Curseur de sélection de personnage | 1d584-1de7c | 0,7 | 2,3 | 8 | 10 | K3, M2 |
| F6 | Interpréteur de scripts de scène (démos) | 1f200-21000 | 3,2 | 7,7 | 38 | 18 | V1, K1, O2, V4 |
| F7 | Écrans titre / copyright | fa300-fb200 | 2,3 | 3,8 | 26 | 5 | K1, O2, V2, V3 |
| | **Total** | 0-0xFFFFF | **619,3** | 1048,6 | 11988 | 2559 | |

### 4.2 Fiches

**Plateforme / noyau**

| ID | Racines et fonctions clés (adresses) | RAM / matériel | Remarques |
|---|---|---|---|
| K1 | `reset` 0x350, `boot_init` 0x492, `irq2_vblank` 0x574 (+0x65e, 0x68c, 0x722, 0x902), `read_inputs` 0x80c, `task_scheduler` 0x928, `task_create` 0x9b6, traps 0x9c2-0xae6, `0xa40/0xa86/0xb64` (attentes), gel `0xee080` | table tâches FF0064, FF0E66 (pile), `A5+0x4f` | à remplacer par une boucle Rust, pas à traduire ligne à ligne ; le seul contrat est le **comptage d'images** des attentes. **Fait en natif fidèle** : VBLANK 0x574, ordonnanceur 0x928, traps et enveloppes (`kernel.rs`, `vblank.rs`, `docs/noyau.md`) ; démarrage 0x350, 0x492 et test de RAM aussi (`demarrage.rs`, calendrier de la référence à ×16) |
| K2 | `0x182a`, `ram_test_block` 0x1912 (appelé sans pile par `a6`) | RAM de travail | texte « RAM ERROR !! » (0x19e2) ; inutile dans un moteur sans ROM. **Fait** : seuls les effets restent (`demarrage.rs`, `docs/noyau.md`) ; la branche d'erreur n'est jamais prise |
| K3 | `copy_inputs_to_players` 0x2aec, `get_player_input` 0x2bfe, `0x2bdc` | FF8000-F, `A5+0x43` | déjà réécrit (`port/ddsom-game/src/inputs.rs`) |
| K4 | vecteurs d'erreur 0x28612-0x28666 (BUS ERROR, ADDRESS ERROR, ILLEGAL INST… 0x28e95-0x28f68), dump `0x28800-0x28c38`, libellés de pools | – | **hors périmètre** de la réécriture |
| V1 | `0xb40/0xb46` (jauge de charge), `0xb96/0xbac` (lockout pièces), `0xbc6/0xbfc` (couches), `0xc0c/0xc32` (effacement total), `0xc58` (OBJ RAM), `0xca8/0xcc8/0xce8` (scroll1/2/3 + palette) | 0x804166, 0x708000, 0x910000-0x91FFFF | feuilles |
| V2 | écrivains de texte `0xd5e/0xdca` (blocs `[ligne, colonne, attr(mot), octets…, 0x40]`, 0x2F = saut, 0xFF = répétition), `0xf38/0xf4a` (blocs 2×2 : dessin / effacement, `text.rs`), `0x1136/0x1234` (chaînes à `##`), `0x1324` (rectangle d'espaces), `0x12a0/0x12c4`, chargeurs de blocs 8×8 `0x260c/0x26d4/0x2766` (tables 0x1f1b60, 0x1f9ba0, 0x2459c0), `0x24a8` (attribut de tuile de terrain, cartes 0x3f8e9c/0x3f9c9c), `0x2a52` | GFX RAM 0x912000/0x914000/0x917000/0x91b800 | feuilles, cibles faciles à tester pixel à pixel (jalon 1 existant) |
| V3 | lanceurs de fondu `0x144c/0x1460/0x146c`, pas `0x158e/0x15c8/0x1602/0x164e`, `0x1686-0x17ea`, chargeurs de palette `0x1a296`, `0x1a366`, `0x1a3d4`, `0x1a60a/0x1a67e/0x1a8b2` (pages 0x91C400/800/C00), fondus `0x1ccc2-0x1d0ff`, `0x1cfc8` (interpolation par composante 4 bits) | `A5+0x48e…`, 0x91C000-0x91CFFF | profil : `0x1cfc8` |
| V4 | `0x1de7c` (RAZ listes), **`0x1de96`** (sous-programme principal), `0x1df14/0x1df3e` (parcours liste + test d'écran), **`0x1e062`** (compile un objet : cadre `+0x34` → entrées de 8 octets x,y,tuile,attr avec caméra A6), `0x1e0b4` (objets liés `+0x48/4A`), `0x1e2aa` (variante palette), `0x1e3a6` (liste 2 à cadres fixes 0x1e45c), `0x1ea6c/0x1eb90/0x1ec0c/0x1ec2c`, `0x1ec4e/0x1ed22/0x1edd0` (ombres, tables 0x11bffa/0x11c11a) | OBJ RAM 0x708000, `A5+0x5b8/5bc/5be…` | c'est le sous-système le plus **autonome et le plus chaud** du profil ; entrée = listes + caméra, sortie = OBJ RAM |
| B1 | `0x33e20` (appelé par la boucle monde), `0x2ede0` (2 360 o) et `0x32108` (1 616 o) : init+scroll des caméras 1 et 2 par n° de scène (tables 0x2f008, 0x2f0ee, 0x2f332, 0x32650…), `0x309e0` (chargement de blocs par scène via `0x26d4`), `0x30xxx-0x33xxx` | `A5+0x4fe/54e`, 0x400004, scroll `A5+0x506-0x55a` | dépend fortement de `A5+0x34` ; contient aussi des **objets de décor à parallaxe** (`0x33062-0x332c0` : déplacement = fraction du delta de scroll `A5+0x506-0x524`) et deux allocations ENEMY/BOSS (`0x33152`, `0x334fc`) ; ~110 entrées observées à l'exécution non retrouvées statiquement (sauts calculés) |

**Moteur d'objets**

| ID | Racines et fonctions clés | RAM | Remarques |
|---|---|---|---|
| O1 | `caseD_1a32` 0x1a32 (charge une entrée d'animation : `+0x30` durée, `+0x32` drapeaux, `+0x34/38/3C` cadres, `+0x20` suite), `0x1a3c`, `0x1a56` (pas d'animation), `0x1b64/0x1cb4` (table de vitesses par `+0x2D`), `obj_move_xy` 0x1d62, `0x1d78/0x1d90`, `0x1ed2` (test d'écran + rangement dans les 5 files de collision), `0x2c16/0x2c88/0x2d58` (variantes pour 6 personnages/ennemis : table 0x2c58/0x2cc0) | `+0x20…+0x4C` | **bibliothèque la plus appelée du jeu** (O1 ← classes : 1 550 sites depuis ENEMY seul) |
| O2 | init `0x2062` (+ 0x2086…0x21bc), libération `0x21d8-0x2250`, allocateurs `0x225e-0x2408`, RAZ `0x2448-0x248e`, **`caseD_1ee1a`** 0x1ee1a (insertion dans la liste triée par `+0x10`, 587 appelants), `0x1ef6c` et `0x1f0be` (retrait/libération), `0x1f184/0x1f258/0x1f300` | FFFE5E-70, `A5+0x5be…` | cœur du modèle d'objets : à réécrire d'abord (structure `Pool<T>` + liste intrusive) |
| O3 | réactions communes aux coups (`families/reactions.rs`, `docs/familles/enemy.md` §2) : 52 routines `0x3bb9c`..`0x3ddf6` appelées par les tables de réactions par type ; animations `0x3de2e`, sons `0x3e28e/0x3e388`, relevé `0x3e0dc` | `+0x28`, `+0x2`, `+0x49` | partagé ENEMY/BOSS, réécrit |
| U1 | RNG `0x19a80` (graine 0x1C3) / `0x19a88` (537 appelants : `A5+0x1a/0x1b`), files d'événements `0x19aa2/abc/db4`, `0x19ad6-0x19b9c` (anneaux FF0108/FF018A), pop-ups `0x19dce` (anneau FF020C, entrées 8 o), `0x19e20-0x1a20e` | FF801A, FF0108, FF018A, FF020C | la **reproductibilité** exige un RNG bit-exact |
| E1 | `0x424e` (joueur le plus proche), `0x43ae`, `0x44ee`, `0x4544/0x453c` (palette par variante), `0xa4b4/0xa4c2` (décision pseudo-aléatoire par tables de 32 bits, indexée par `FFFF92`), `0x1047a` (choix d'attaque 1/2 selon portées), `0xb5e0` (recouvrement boîte/cible), `0xac3a`, `0xfe5a`, `0x1040c/0x103d6`, `0x1059e` | `+0x70..+0x80` (paramètres d'IA), `FFFF92` | ~55 Ko d'étendue mais surtout des tables |
| X1 | `0x260ee` (racine), `0x26150/0x261f0/0x265c4/0x272c2/0x2736c/0x27792/0x27d74/0x27e0a/0x2806a/0x28116/0x283d4`, détails `0x2627c`, `0x2630e` (application d'un coup : état `+4`, PV, `+0x6A`), `0x27a4c` (poussée/chevauchement) | files `FFFE72-86`, boîtes via `+0x38/3C` (tables 0x1ca538…) | appelé par les étapes de scène (W3) ; dépendances faibles → bon candidat isolé |
| X2 | `0x3668` (placement), `0x388a`, `0x3bbe` (calcul de dégâts : table × aléa), `0x4002` (dégâts selon réglage `A5+0x70`), `0x3f28` | `A5+0x70`, `FFFF92` | |
| W0 | boucles `0x398e2 … 0x39f82` + tables de types (§3.2) | – | trivial en Rust (`for obj in pool { handlers[obj.ty](obj) }`) |
| W1 | `0x1c45a` boucle de partie, `0x1c44a` (RAZ `A5+0x2c..0x6b`), `0x1c86e` mise à jour du monde (5 variantes d'ordre), phases 0/4/12/20 (§2.4), `0x1d088-0x1d440` | `A5+0x2c-0x2e`, `A5+0x30…` | |

**Joueurs**

| ID | Racines | Remarques |
|---|---|---|
| P1 | `0x2d9e` (RAZ des 4 joueurs), `0x2df8` (RAZ des 8 sous-objets), initialisation de slot `0x2e58/2e6e/2e86/2e9e` → `0x2eb2`, sous-init `0x31f0-0x3552`, `0x3308` | construit l'enregistrement 0x200 |
| P4 | `0x35292` (boucle des 4 joueurs), `0x352c4` (aiguillage `+4`), `0x352f4` (état 2 : `0xf157c` réactions, `0x3760e`, compteurs `+0x194/+0x53/+0x7E/+0x71…`, action `+6` par table `0x35586`, `0x35c0`), `0x35428/0x35436` (états 4/6), actions génériques `0x355ba-0x36f48`, `0x39242/0x39260 player_walk_speed`, `0x39ff0/0x3a0a8` (états `+5` 2 et 1), `0x3a280-0x3b820` | 26 actions ; table de vitesses 0x104xxx |
| P5 | code par personnage : tables `0x359cc` / `0x35a0a` (12 entrées) vers **Fighter** 0x3f0ae/0x3f9bc, **Cleric** 0x406d4/0x41f22, **Elf** 0x42b0c/0x444ca, **Dwarf** 0x45148/0x45988, **Magic-User** 0x4619e/0x4879a, **Thief** 0x49498/0x49d46 (noms ROM 0x1ddd4 : fighter, cleric, elf, dwarf, magic user, dorobou) ; ~780 entrées (sous-états par arme/attaque/magie : tables de 3 à 46 entrées, ex. 0x406e8, 0x461b0) | ≈ 7,4 Ko d'index seulement : tout le reste est hors `decomp/` |
| P3 | `0x23812/0x23b84/0x23d46/0x23f40` (sous-états de l'état principal 0, tables indexées par n° de scène `A5+0x34`) vers ~250 routines 0x23900-0x26100 | comportements scénarisés par scène (h) |
| P6 | `0xf157c` (réaction du joueur aux coups : combinaisons d'états `+4..+7`, tables 0xf1a38…), `0xf186e/0xf1884` (reconnaissance de séquences d'entrées pour coups spéciaux : table d'octets, compteur `+1`), `0xf18fa`, `0xf1b22`, `0xf1c1a`, `0xf232e` | profil : `0xf1884` |

**Crédits, écrans, outils**

| ID | Racines | Remarques |
|---|---|---|
| C1 | tâche `0x13c0a` : `0x141e0/0x1422e` (pièces ; lecture 0x804021), `0x143da` (VBLANK : `0x14546/0x145cc` cumul pièces/crédits `A5+0x44f/450/451`, `0x1e/0x1f`), `0x13c2c` (1 160 o : 4 slots, « start/continue/rejoindre », ré-init de la partie), `0x140b4` (démarrage d'un joueur : débite le crédit, `bset A5+0x31`, `0x2e4ec`), `0x14290/0x14300/0x14322/0x1436e/0x143a6` (compteurs de pièces), affichage `0x14652/0x148be/0x14b74/0x14c1e` (textes CREDITS/COIN(S) 0x1afd8-0x1b1c8), `0x14d74` (bascule vers le menu test), tâche `0xe03fe` (INSERT COIN) | profil : 13c2c, 14652, 143da, 14322, 14b74, 140b4 |
| M1 | tâche `0x29162` (menu + 8 écrans : INPUT, OUTPUT, SOUND & VOICE TEST, COLOR TEST, DOT CROSS HATCH, GAME DATA, CONFIGURATION MENU 7-1/7-2, MEMORY TEST), écrans `0x29a7a…0x2d1ac`, `0x2a4f4` | chaînes 0x29f22-0x2d27c ; hors jeu normal |
| M2 | contrôle EEPROM au boot `0x2d30a` (« EEPROM ERROR » 0x2d7fb), lecture/écriture bit à bit `0x2d662-0x2da90` (accès 0x804040), `0x2dc9a/0x2dca6/0x2dcd2/0x2e8e4…` (défauts par région : « D&D2 JPN/USA/HIS/ASI/EUR/BRA » 0x2dba0, signature « CAPCOMBGSDungeons&Dragons 2 » 0x2ebfb), config `A5+0x1e-0x22`, `A5+0x70/0x82` | à garder (sauvegardes, options) |
| M3 | `0xee16e` (2 730 o, saut par `(0x5e,A0)` en 0xee17e), écrans « Secret Test » (Beta beta mode, Char tbl mode, Block test mode, Message test, langues) 0xeee00, éditeur de tables (`body/push/atck/kage`, hit set, adjx/widx… 0xefd40), Block Test `0xf2600-0xf2e00` (Scroll 2/3 Test) | outil de debug développeur ; **non nécessaire** (mais précieux : noms de tables pl/em/bs/ef/pf/ps/es/we/bx/it/st, 0xefe16) |
| S1 | `0xe0836/0xe0848/0xe0882` (enfile D1/D2/D3 : mot de commande, 2 mots de paramètres), ~390 wrappers `sfx_NNN` de 52-140 octets (`0xe0be2…0xede08` ; 144 appellent `0xedee4` (complément de `FFFFA960`) ou `0xedef2` (rts) ; certains calculent un panoramique depuis la X de l'objet via la table 0xee04e), `0xe0a10/0xe0a2e` (écrit 0x618005), `0xedf80` (affichage des verrous 0x61800x pour le test), `0xee044` (init pile privée FF2A1A) ; consommation par `0x68c` (VBLANK) | profil : `0xee044` ; ~1 660 accès à FF2A1A | **candidat idéal** : tout est tabulaire (identifiant → (cmd, p1, p2, pan)) |
| F1/F2/F7/F4 | tâche de boot, écrans d'avertissement (6 régions), écrans titre `0xfa296/0xfaef2/0xfc576`, `0x1371e-0x13b46` | peu de logique, beaucoup de séquencement |
| F3 | tâche `0x150ac` : init `0x1507a`, boucle `0x16c86` (états `A5+0x3c`, `0x344`), `0x15216`, `0x155b2` (texte 2 colonnes depuis tables 0x156ea/0x156ee), `0x17814`, `0x175a8`, `0x1728e` (curseur d'écriture tilemap) | zone sous-estimée (sauts calculés) |
| F5 | `charsel_cursor` 0x1d584 → `0x1d714`, `0x1d686`, `0x1d6d8`… | cohérent avec la phase 4 |
| F6 | interpréteur de scripts : flux d'octets `(A5-0x715e)`, 0 = suite, négatif = fin (0x1fa92), 2 = son (table de 6 wrappers 0x1f3aa → S1), autres → `0x1fbd6` ; états `0x1f32e` (sous-mode 4) et `0x20926` (sous-mode 10) ; texte « LEVEL/AGE/HIT POINTS/STRENGTH/… » 0x20070 | |


## 5. Les familles d'objets (10 classes)

Chaque classe = un pool (§3.2) + une table de **types** (index = octet `+2`) + une boucle de dispatch (W0). Un type = un handler à
automate d'états (`+4`, `+5`, `+6`, `+7` par `jmp/jsr (tbl,PC,Dn)`), terminé par `jmp 0x1ee1a` (retri dans la liste de rendu).
Le gabarit d'un handler est quasi uniforme : `jsr 0x29ea` (rts, point d'extension de pause) → `jsr 0xf1bfc` (décrément des compteurs `+0x53`, `+0x89`/`+0x88`)
→ saut par état → animation `0x1a32/0x1a56` → `0x1ee1a`. Les types d'une même classe partagent beaucoup de code interne (sous-états communs).

| Classe | Code atteint | Types | Créée par (sites d'allocation) | Alloue (sites) | Tables sprites de debug (0xefe16) | Rôle et preuves |
|---|---|---|---|---|---|---|
| **PSHL** (6) | 10,2 Ko | 25 (22 handlers distincts) | C06 lui-même 33, C12 2, P5 1 | PSHL 33, EFFCT 1 | `ps00-ps17` (24) | tirs du joueur (nom ROM) ; le type 0 recopie X/Y/Z du propriétaire (`+0x5C`) puis `jmp 1ee1a` |
| **PEFF** (0x12) | 26,0 Ko | 48 | C12 lui-même 78, P4 1 (0x3714c : bouton 3 = bit 6 de `+0x6E`), C0e 3 | PEFF 78, PSHL 2, SET 1 | `pf00-pf2d` (31) | effets liés au joueur (nom ROM) ; types 0/1 = parent à 6 enfants (variantes 0-5, `+0x50` = joueur, tables joueur `+0x140/+0x148`) : h = anneau de sélection d'objets/magies |
| **ENEMY** (2) | 97,7 Ko | 28 + 1 nul | W2 4, B1 1, C02 8, C04 8, C08/C10/C0e 1 | ESHL 6, EFFCT 5, ITEM 1, ENEMY 8 | `em00-em1c` (21) | ennemis courants ; noms de debug 0x1a084 (annexe A) ; type d'index 17 = doublon « Wyvern » → entrée nulle (pointe 0x39aec) |
| **BOSS** (4) | 115,6 Ko | 23 (20 handlers) | W2 10, B1 1, C04 4, C0e 3, C10 2 | EFFCT 23, ESHL 19, ENEMY 8, SET 6, BOSS 4 | `bs00-bs15` (20) | boss et mini-boss ; noms 0xd628c (21) via table 0xd6232, bannière de nom affichée par un type SET (code à 0xd61c4, écrit en 0x91215c) ; 3 places seulement |
| **ESHL** (8) | 34,2 Ko | 64 | C08 65, C04 19, C0e 7, C02 6, C10 1 | ESHL 65, EFFCT 1, SET 2, ENEMY 1 | `es01-es46` (22) | projectiles / attaques ennemis (nom ROM) ; ex. type 0 : naît à ±64,+56 du propriétaire avec vitesse −1280 |
| **EFFCT** (0xE) | 47,2 Ko | 64 (1 commun 0xcc25a) | C0e 94, C04 23, W2 10, C02 5, W3/F4 3 | EFFCT 94, SET 8, ESHL 7, BOSS 3, PEFF 3, SOBJ 2 | `ef00-ef3a` (17) | effets visuels/étincelles/explosions (nom ROM) ; créée par presque tous |
| **SET** (0x10) | 34,7 Ko | 34 (1 commun) | C10 9, C0e 8, C04 6, W1 4, F7 3, W2/W3 4, … | SET 9, BOSS 2, EFFCT 2, … | `st10` (1) | « SET WORK » : type 0 = **animation de palette** (cycles dans 0x91C000/400/800/C00, variantes 0x3e-0x40 créées en phase 0) ; les 33 autres types : rôle inconnu (écrans de titre, décors animés, h) |
| **ITEM** (0xC) | 8,4 Ko | 10 | E1 4, C0a 4, C0c 3, X2 1 | ITEM 3 | `it00-it07` (8) | objets ramassables ; `0xdd470` = test de ramassage contre les 4 joueurs (boîtes tables 0xdd578/0xdd590) |
| **BOX** (0xA) | 3,5 Ko | 4 | aucun appel direct à 0x2378 trouvé | ITEM 4, SET 1 | `bx00-bx01` | caisses/coffres lâchant des objets (h ; mécanisme de création non trouvé) |
| **SOBJ** (0x14) | 7,5 Ko | 16 (1 commun) | C14 7, C1 5, F7 2, W3 2, C0e 2 | SOBJ 7 | – | objets système/UI : variantes 0xE4-0xE8 créées par `0x13c2c` (invites de crédit/start) ; animations 0x1c03a6 ; table variante → handler 0xdd6a4 |

Autres observations :

* Les **joueurs** ne sont pas dans les pools ; leurs armes (« P1WEAPON F/B » … « P4WEAPON F/B », 0x2902c-0x2908e) sont des sous-objets de 0x60 octets en `+0x800/+0x860`.
* Les tables sprite de debug (`pl00-pl0b`, `em`, `bs`, `ef`, `pf`, `ps`, `es`, `we00-we2f` armes, `bx`, `it`, `st`) donnent le nombre de jeux de cadres distincts par famille, utile pour dimensionner l'extraction des assets.
* Les **cadres de sprite** (`+0x34`) et scripts d'animation sont dans les régions ROM 0x15xxxx-0x1bxxxx (annexe D).
* Le **mécanisme de génération des vagues d'ennemis** n'est pas localisé : seulement 26 sites d'allocation ENEMY et 22 BOSS dans toute la ROM
  (scènes W2 scriptées, objets de décor B1 `0x33152/0x334fc`, auto-génération par les types). Une partie du code B1/F3 est non atteinte
  (sauts calculés) : le spawner principal s'y trouve probablement (à vérifier avec MAME : `mame/xref.sh` sur les octets `+0x28`).

## 6. Dépendances entre sous-systèmes

Appels/sauts statiques sortants (les plus significatifs) :

| Sous-système | Dépend de |
|---|---|
| C02, C04, C06, C08, C0e, C12 | **O1** (anim/vitesses, 400-1 500 sites chacun), **O2** (alloc/tri), **E1** (IA, surtout C02/C04), **U1** (RNG), **S1** (sons), V2 (texte/tuiles), B1 (position caméra) ; C04 → C0e, C10 ; C02 → C08 |
| P5 (personnages) | O1 (805), S1 (292), P4 (205), C12 (177), C06 (69), X2, U2, P6 |
| P4 / P3 | O1, O3, S1, C0e, P1, C12 ; P3 → P4 |
| X1 (collisions) | C0e, S1, U1, X2, E1 (lit les files remplies par `0x1ed2` de O1) |
| W0 / W1 | W0 appelle les types (toutes classes) ; W1 → W0, V4, O2, V3, V2, K1 |
| W2 / W3 (scènes) | K1 (attentes), V1-V4, S1, W1, C10, C14, X1 (260ee), B1 (33e20) |
| V4 (sprites) | O1 (lecture des champs) uniquement ; lit les listes O2 |
| B1 | V2 (blocs de tuiles), V1 ; lit `A5+0x34` |
| S1 | aucun (écrit le FIFO) ; consommé par K1 (0x68c) |
| C1, M1, M2 | K1, V1, V2, V3, S1 ; M1 → M2 |
| F1-F7 | K1, V1-V4, O2, S1, C14, W3 |
| K1 | V1 (0xb40), M2 (EEPROM à l'init), C1 |

Couches (du bas vers le haut) : **0** matériel + RAM (K2, V1) → **1** V2, V3, U1, S1, K3 → **2** O1, O2 → **3** V4, O3, E1, X2, B1 → **4** X1, W0, familles d'objets C* → **5** joueurs P* → **6** W1, W2, W3, F* → **7** K1/boucle principale.
Cycles : C04 ↔ C02/C08/C0e (créent les uns des autres) ; P5 ↔ C12/C06 ; ces cycles imposent de les traiter en bloc par famille (« même
pool » = même module).


## 7. Ordre de réécriture proposé

Hypothèse de calcul : **1 ligne Rust pour 8 à 14 octets de code 68000 atteint** (médiane 11) ; les tables incrustées et les données de sprites
sortent en `assets/*.ron`. Les estimations excluent les tests et l'extraction d'assets.
Le guide `docs/rewrite-guide.md` demande de réécrire « de haut en bas » (remplacer un sous-système entier) ; l'ordre ci-dessous respecte
cette règle au sein de chaque phase mais commence par les sous-systèmes **sans dépendances** pour livrer vite et alimenter l'oracle.

| Phase | Sous-systèmes | Code atteint | Lignes Rust (médiane ; plage) | Indépendance / parallélisme | Risques |
|---|---|---|---|---|---|
| **Étape 1. Feuilles** (parallélisables dès maintenant) | | | | | |
| 1.1 | **S1 son** (+ FIFO `0x68c`, wrappers) | 42,9 Ko | ~1 500 (table de ~390 entrées + 300 de logique) ; 1 000-3 000 | totalement indépendant (écrit le FIFO ; testable par comparaison des octets écrits en 0x618001-19) | panoramique des wrappers à pile privée (FF2A1A) ; ordre d'émission |
| 1.2 | V1 + V2 + V3 (primitives vidéo, texte, fondus) | 5,1 Ko | ~500 | indépendants entre eux, jalon « rendu pixel » existant | écritures GFX RAM dans l'ordre |
| 1.3 | U1 (RNG + files d'affichage) | 1,0 Ko | ~100 | RNG partagé par tous : à faire tôt, bit-exact | – |
| 1.4 | C1 crédits/pièces ; M2 EEPROM/config | 9,4 Ko | ~900 | indépendants | accès EEPROM bit-à-bit |
| 1.5 | (facultatif) M1 menu test, M3 secret test | 14 Ko | ~1 300 | indépendants, faible priorité | – |
| **Étape 2. Moteur d'objets** | | | | | |
| 2.1 | O2 (pools, listes triées) → O1 (anim, vitesses, culling) | 3,1 Ko | ~300 | **bloquant pour tout le reste** ; une seule personne ; définit `struct Obj` + `Pool` + liste intrusive | layout 68k à conserver tant que des handlers 68000 restent (projection RAM↔struct) |
| 2.2 | V4 sprites (`0x1de96`) | 2,4 Ko | ~250 | dépend de 2.1 (lecture des champs) ; testable seul via instantanés | chaud (profil) ; budget `A5+0x5b8` |
| 2.3 | W0 dispatch + W1 boucle de partie (phases 0/12/20) | 3,5 Ko | ~350 | dépend de 2.1 ; garde les handlers en 68000 via `call()` | ordre exact des boucles de dispatch |
| 2.4 | X1 collisions + X2 dégâts + E1 aides d'IA + O3 | 22 Ko | ~2 000 | X1 isolé ; E1/O3 nécessaires aux ennemis | dépend des champs `+0x38/3C`, tables de boîtes |
| 2.5 | B1 (ID) scrolling/caméras par scène | 5,6 Ko (+ non atteint) | 600-3 000 | dépend de V2 ; très lié à `A5+0x34` | zone mal couverte (sauts calculés) |
| **Étape 3. Joueurs** | | | | | |
| 3.1 | P1 init, P4 noyau (états/actions), P6 réactions, P3 scénarios | 27 Ko | ~2 500 | dépend de 2.1-2.4, S1 (1.1) | P4 : 307 points d'entrée, P3 : 258 ; états par `+4/+5/+6` |
| 3.2 | **P5 : 6 personnages** (fighter, cleric, elf, dwarf, magic user, thief ; chacun 2 variantes) | 44,5 Ko | ~4 000 | **1 développeur par personnage** une fois 3.1 prêt ; code propre à chacun | gros ; magie/objets partagent C12/C06 |
| 3.3 | tirs et effets joueur : C06 PSHL, C12 PEFF | 36 Ko | ~3 300 | avec P5 (cycle P5 ↔ C12/C06) | – |
| **Étape 4. Monde** (le gros du volume) | | | | | |
| 4.1 | projectiles et effets : C08 ESHL, C0e EFFCT, C10 SET, C14 SOBJ | 124 Ko | ~11 000 | **parallélisable par type** (≈ 175 types) ; dépend de 2.1-2.2 ; C0e et C08 sont des dépendances des ennemis | taille |
| 4.2 | C0c ITEM, C0a BOX | 12 Ko | ~1 000 | indépendants | création de BOX inconnue |
| 4.3 | C02 ENEMY (28 types) | 97,7 Ko | ~8 900 | **par type** après 4.1 et 2.4 ; ≈ 3,5 Ko/type | tables de décision, variantes par scène ; réécrits : types 7, 16, 20, 22, 23, 24, 26, 28 (`docs/familles/enemy.md`) |
| 4.4 | C04 BOSS (20 handlers) | 115,6 Ko | ~10 500 | par boss, après 4.3 (ils créent des ENEMY) ; ≈ 5,8 Ko/boss | plus complexes (phases) |
| **Étape 5. Scénarios et flux** | | | | | |
| 5.1 | W2 + W3 + F3 + F5 + F6 | 44 Ko | ~4 000 | après l'étape 4 ; scriptés → candidats à un format de données | beaucoup de cas particuliers par scène ; non atteint à compléter |
| 5.2 | F1/F2/F4/F7 (attract, écrans fixes) ; remplacement de K1 (boucle principale, ordonnanceur → coroutines Rust) | 8 Ko | ~700 | en dernier | – |
| – | K2, K4 | 1,4 Ko | 0 | **hors périmètre** (auto-test RAM, écran d'exception) | – |

Totaux indicatifs : **≈ 619 Ko de code → 45 000-70 000 lignes** (médiane ~55 000) ; si les types similaires sont factorisés (automates communs, tables de données extraites en `assets/`), la taille finale pourrait tomber vers
**30 000-40 000 lignes** (hypothèse). Volume d'« unités de travail » : ~307 handlers de types d'objets + 6 programmes de
personnages + ~25 phases/étapes de scène + ~12 tâches. À titre d'ordre de grandeur, 1 à 2 jours-développeur par type d'objet (validation différentielle comprise)
donne 300-600 jours-développeur pour l'étape 4 seule.

Conseils de découpage entre développeurs : (1) un « propriétaire » de l'étape 2.1 (modèle d'objets O1/O2) ; (2) un pour le son (1.1) puis la vidéo (1.2) ; (3) un par personnage (6) après 3.1 ;
(4) un par groupe de ~10 types pour 4.1/4.3/4.4 (le guide impose le contrôle différentiel par routine et la validation de 30 000 images ; les scénarios
`testdata/scenarios/` doivent exercer chaque type : prévoir des scénarios par stage/boss) ; (5) un pour W2/W3 et le scroll B1 (liés à `A5+0x34`).

## 8. Zones obscures / limites

* **Spawner des vagues d'ennemis** : non localisé (voir §5). Probable dans la partie non atteinte de B1 (0x30000-0x33000, 107 entrées apprises à l'exécution non retrouvées) ou F3/W3.
* **F3 (0x15000-0x18300)**, sauts calculés non résolus (380 entrées dynamiques manquantes) : rôle exact de la tâche 0x150ac (panneaux d'état/HUD ou autre) à confirmer.
* **Rôle des 33 types SET autres que le type 0**, de BOX (création), de la tâche 0xf7dfe, de `0x2dcf6`/`0x664000`, de `0xee080` (gel : h), de la tâche `0x1835e` (écrans fixes : h).
* **Sémantique de nombreux octets `A5+0x20-0x8x`** (drapeaux de scène) et des champs d'objets `+0x68-+0x9x` (paramètres d'IA/réaction : tables de 1 à 20 octets par type).
* **P3 (0x23800-0x26100)** : comportements scénarisés des joueurs par scène (h) ; **W2** : sous-scènes de transition (h) ; phases 8, 24, 28 de §2.4.
* **U2 (0x1aa00-0x1c44a)** : 6,7 Ko d'étendue pour 0,6 Ko de code : principalement des tables de cadres ; petites routines `caseD_1aaa0…` (`jsr 0x1ac24`) de rôle inconnu.
* **E1** : 55 Ko d'étendue pour 5 Ko de code atteint : le reste est de la donnée (tables de décision par bits `0xa4f2…`, paramètres) ; le code d'IA proprement dit est dans les handlers.
* Code non désassemblé correctement dans `listing.txt` : ~28 Ko atteints (surtout 0xA0000-0xEFFFF) ; se fier à `prog_dec.bin` (objdump m68k fonctionne dessus : `m68k-linux-gnu-objdump -D -b binary -m m68k:68000 --start-address=… prog_dec.bin`).
* Les attributions de noms de types (ENEMY/BOSS) par index de tables de debug sont une **hypothèse corroborée** (doublon « Wyvern » n°17 = type nul ; ordre des boss
  conforme à la table 0xd6232), pas une preuve pour chaque index.


## Annexe A. Tables de types par classe

Format : `indice:adresse(étendue en octets)` ; l'étendue = distance jusqu'au handler suivant de la même classe (inclut les tables incrustées) ; « = » signale un handler partagé par plusieurs indices. Les indices sont ceux de l'octet `+2` de l'objet.

**ENEMY** (classe 0x2, table 0x39a78, 29 entrées)

| Indice | Handler | Étendue | Nom de debug (h) |
|---|---|---|---|
| 0 | 0x54914 | 13232 | Kobold |
| 1 | 0x57cc4 | 7910 | Skeleton |
| 2 | 0x59baa | 8700 | Troglodyte |
| 3 | 0x5bda6 | 6868 | Bow Gnoll |
| 4 | 0x5d87a | 7960 | Owlbear |
| 5 | 0x5f792 | 7122 | Ghoul |
| 6 | 0x61364 | 10182 | Hellhound |
| 7 | 0x63b2a | 1158 | Wyvern |
| 8 | 0x63fb0 | 6968 | Gargoyle |
| 9 | 0x65ae8 | 3380 | GiantBeetle |
| 10 | 0x6681c | 4152 | GiantScorpion |
| 11 | 0x67854 | 6960 | mud golem |
| 12 | 0x69384 | 6094 | Shadow Elf |
| 13 | 0x6ab52 | 6532 | Axe Gnoll |
| 14 | 0x6c4d6 | 5450 | Troll |
| 15 | 0x6da20 | 6048 | Goblin |
| 16 | 0x6f1c0 | 4636 | Riyakaa |
| 17 | – | – | (Wyvern bis)=nul |
| 18 | 0x703dc | 6760 | Ogre |
| 19 | 0x71e44 | 12668 | DisplacerBeast |
| 20 | 0x74fc0 | 1796 | ManScorpion |
| 21 | 0x756c4 | 2174 | Harpy |
| 22 | 0x75f42 | 1616 | Chimera |
| 23 | 0x76592 | 1386 | Spectre |
| 24 | 0x76afc | 1042 | Manticore |
| 25 | 0x76f0e | 2088 | DarkWarrior 1 |
| 26 | 0x77736 | 1500 | DarkWarrior 2 |
| 27 | 0x77d12 | 5614 | WarMachine |
| 28 | 0x79300 | 256 | Nagpa |

**BOSS** (classe 0x4, table 0x39b12, 23 entrées)

| Indice | Handler | Étendue | Nom de debug (h) |
|---|---|---|---|
| 0 | 0x794ae | 6204 | Ogre |
| 1 | 0x7acea | 6334 | D.Beast |
| 2 | 0x7c5a8 | 11890 | Man Scorpion |
| 3 | 0x7f41a | 10016 | Harpy |
| 4 | 0x81b3a | 6370 | Chimera |
| 5 | 0x8341c | 3826 | Ezerhorden |
| 6 | 0x8430e | 5866 | Manticore |
| 7 | 0x859f8 | 9672 | Dark Warrior1 |
| 8 | 0x87fc0 | 10140 | Dark Warrior2 |
| 9 | 0x8a75c | 8942 | War Machine |
| 10 | 0x8ca4a | 2900 | Nagpa |
| 11 | 0x8d59e | 8190 | Frost Salamander |
| 12 | = indice 11 | – | Flame Salamander |
| 13 | 0x8f59c | 1352 | Black Dragon |
| 14 | 0x8fae4 | 6732 | Green Dragon |
| 15 | 0x91530 | 7470 | Red Dragon |
| 16 | 0x9325e | 8278 | Beholder |
| 17 | 0x952b4 | 9016 | Tel'Arin |
| 18 | 0x975ec | 7822 | Lich |
| 19 | = indice 14 | – | Black Dragon (bis)? |
| 20 | = indice 15 | – | Synn ? |
| 21 | 0x9947a | 6704 | (21 ?) |
| 22 | 0x9aeaa | 11602 | (22 ?) |

**PSHL** (classe 0x6, table 0x39908, 25 entrées)

0:51c64(322) 1:51da6(680) 2:5204e(486) 3:52234(400) 4:523c4(662) 5:5265a(402) 6:527ec(936) 7:52b94(686) 8:52e42(634) 9:=1 10:=2 11:=2 12:530bc(468) 13:53290(386) 14:53412(196) 15:534d6(452) 16:5369a(352) 17:537fa(526) 18:53a08(1164) 19:53e94(1114) 20:542ee(354) 21:54450(272) 22:54560(442) 23:5471a(390) 24:548a0(96)

**ESHL** (classe 0x8, table 0x39b96, 64 entrées)

0:9dbfc(532) 1:9de10(666) 2:9e0aa(268) 3:9e1b6(294) 4:9e2dc(380) 5:9e458(1906) 6:9ebca(246) 7:9ecc0(320) 8:9ee00(326) 9:9ef46(1072) 10:9f376(176) 11:9f426(386) 12:9f5a8(212) 13:9f67c(266) 14:9f786(1668) 15:9fe0a(1494) 16:a03e0(494) 17:a05ce(1080) 18:a0a06(282) 19:a0b20(314) 20:a0c5a(828) 21:a0f96(490) 22:a1180(274) 23:a1292(236) 24:a137e(830) 25:a16bc(544) 26:a18dc(392) 27:a1a64(562) 28:a1c96(218) 29:a1d70(752) 30:a2060(342) 31:a21b6(484) 32:a239a(394) 33:a2524(268) 34:a2630(674) 35:a28d2(250) 36:a29cc(406) 37:a2b62(486) 38:a2d48(624) 39:a2fb8(556) 40:a31e4(392) 41:a336c(508) 42:a3568(506) 43:a3762(168) 44:a380a(894) 45:a3b88(746) 46:a3e72(76) 47:a3ebe(458) 48:a4088(262) 49:a418e(216) 50:a4266(700) 51:a4522(420) 52:a46c6(1724) 53:a4d82(296) 54:a4eaa(1198) 55:a5358(270) 56:a5466(1238) 57:a593c(410) 58:a5ad6(492) 59:a5cc2(666) 60:a5f5c(1148) 61:a63d8(786) 62:a66ea(378) 63:a6864(6044)

**BOX** (classe 0xa, table 0x39e76, 4 entrées)

0:d99fa(2942) 1:da578(564) 2:da7ac(70) 3:da7f2(984)

**ITEM** (classe 0xc, table 0x39f5a, 10 entrées)

0:dabca(604) 1:dae26(668) 2:db0c2(1820) 3:db7de(1560) 4:dbdf6(1420) 5:dc382(674) 6:dc624(2122) 7:dce6e(708) 8:dd132(1136) 9:dd5a2(124)

**EFFCT** (classe 0xe, table 0x39ce0, 64 entrées)

0:a8594(452) 1:a8758(8130) 2:aa71a(462) 3:aa8e8(472) 4:aaac0(3174) 5:ab726(212) 6:ab7fa(596) 7:aba4e(108) 8:ababa(308) 9:abbee(222) 10:abccc(542) 11:abeea(5516) 12:ad476(456) 13:ad63e(552) 14:ad866(3010) 15:ae428(258) 16:ae52a(610) 17:ae78c(1346) 18:commun(cc25a) 19:aecce(126) 20:aed4c(1556) 21:af360(146) 22:af3f2(2606) 23:afe20(4562) 24:b0ff2(296) 25:b111a(218) 26:b11f4(242) 27:b12e6(136) 28:b136e(210) 29:b1440(984) 30:b1818(640) 31:b1a98(4542) 32:b2c56(116) 33:b2cca(288) 34:b2dea(342) 35:b2f40(272) 36:b3050(874) 37:b33ba(154) 38:b3454(284) 39:b3570(572) 40:b37ac(1180) 41:b3c48(1200) 42:b40f8(2804) 43:b4bec(924) 44:b4f88(952) 45:b5340(276) 46:b5454(784) 47:b5764(646) 48:b59ea(980) 49:b5dbe(504) 50:b5fb6(1298) 51:b64c8(280) 52:b65e0(280) 53:b66f8(508) 54:b68f4(384) 55:b6a74(342) 56:b6c6e(218) 57:b6bca(164) 58:b6d48(426) 59:b6ef2(854) 60:b7248(726) 61:b751e(654) 62:b77ac(186) 63:b7866(10922)

**SET** (classe 0x10, table 0x39eac, 34 entrées)

0:ba310(8712) 1:bc518(764) 2:bc814(47934) 3:c8352(4104) 4:c935a(1062) 5:c9780(6252) 6:cafec(688) 7:cb29c(4030) 8:cc25a(110) 9:cc2c8(250) 10:cc3c2(6376) 11:cdcaa(198) 12:cdd70(390) 13:cdef6(144) 14:cdf86(158) 15:ce024(24812) 16:d4110(1370) 17:d466a(742) 18:d4950(82) 19:d49a2(5560) 20:d5f5a(1048) 21:d6372(1616) 22:d69c2(666) 23:d6c5c(1018) 24:d7056(4040) 25:d801e(236) 26:d810a(846) 27:d8458(2188) 28:d8ce4(234) 29:d8dce(1622) 30:d9424(110) 31:d9492(574) 32:d96d0(800) 33:=3

**PEFF** (classe 0x12, table 0x39992, 48 entrées)

0:49e36(1000) 1:4a21e(1634) 2:4a880(382) 3:4a9fe(428) 4:4abaa(874) 5:4af14(674) 6:4b1b6(510) 7:4b3b4(380) 8:4b530(234) 9:4b61a(430) 10:4b7c8(372) 11:4b93c(810) 12:4bc66(1360) 13:4c1b6(254) 14:4c2b4(778) 15:4c5be(906) 16:4c948(182) 17:4c9fe(716) 18:4ccca(322) 19:4ce0c(640) 20:4d08c(626) 21:4d2fe(228) 22:4d3e2(672) 23:4d682(150) 24:4d718(1514) 25:4dd02(822) 26:4e038(734) 27:4e316(484) 28:4e4fa(464) 29:4e6ca(210) 30:4e79c(1254) 31:4ec82(932) 32:4f026(294) 33:4f14c(346) 34:4f2a6(2420) 35:4fc1a(304) 36:4fd4a(238) 37:4fe38(230) 38:4ff1e(178) 39:4ffd0(376) 40:50148(512) 41:50348(2132) 42:50b9c(1242) 43:51076(1894) 44:517dc(216) 45:518b4(546) 46:51ad6(246) 47:51bcc(152)

**SOBJ** (classe 0x14, table 0x39fa8, 16 entrées)

0:dd61e(1546) 1:ddc28(394) 2:dddb2(102) 3:dde18(116) 4:commun(cc25a) 5:dde8c(56) 6:ddec4(5648) 7:df4d4(476) 8:df6b0(196) 9:df774(314) 10:df8ae(442) 11:dfa68(568) 12:dfca0(164) 13:dfd44(248) 14:dfe3c(1052) 15:e0258(424)


## Annexe B. Affectation des 2 559 fonctions de `decomp/index.tsv` aux sous-systèmes

Une fonction appartient au sous-système dont la plage contient son adresse (première plage trouvée). Compte = fonctions de l'index dans la plage.

| Plage | ID | Sous-système | Fn index |
|---|---|---|---|
| 0x000000-0x000b40 | K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 17 |
| 0x000b40-0x000d18 | V1 | Primitives vidéo (attente, couches, effacements) | 13 |
| 0x000d18-0x001460 | V2 | Texte, rectangles et blocs de tuiles | 16 |
| 0x001460-0x001800 | V3 | Palettes et fondus | 13 |
| 0x001800-0x001a32 | K2 | Auto-test RAM (« RAM ERROR !! ») | 2 |
| 0x001a32-0x002062 | O1 | Animation, tables de vitesse, physique, culling | 18 |
| 0x002062-0x0024a8 | O2 | Pools d'objets et listes triées | 35 |
| 0x0024a8-0x002aec | V2 | Texte, rectangles et blocs de tuiles | 17 |
| 0x002aec-0x002c16 | K3 | Entrées vers enregistrements joueur | 3 |
| 0x002c16-0x002d9e | O1 | Animation, tables de vitesse, physique, culling | 4 |
| 0x002d9e-0x003600 | P1 | Initialisation des joueurs | 23 |
| 0x003600-0x004200 | X2 | Dégâts, terrain, placement | 19 |
| 0x004200-0x011b16 | E1 | Aides d'IA (cible, portées, décisions aléatoires) | 69 |
| 0x011b16-0x011cc0 | F1 | Tâche de boot, avertissements région | 1 |
| 0x011cc0-0x013400 | F1 | Tâche de boot, avertissements région | 4 |
| 0x013400-0x013c0a | F2 | Machine à modes (attract) | 0 |
| 0x013c0a-0x014e50 | C1 | Crédits, pièces, start/continue | 19 |
| 0x014e50-0x015000 | F2 | Machine à modes (attract) | 0 |
| 0x015000-0x018300 | F3 | Interface joueurs : sélection / panneaux d'état (h) | 49 |
| 0x018300-0x019a80 | F4 | Exécuteur d'écrans fixes (logos) | 5 |
| 0x019a80-0x01a296 | U1 | RNG et files d'affichage (pop-ups) | 18 |
| 0x01a296-0x01aa00 | V3 | Palettes et fondus | 12 |
| 0x01aa00-0x01c44a | U2 | Petites routines / tables (rôle inconnu) | 17 |
| 0x01c44a-0x01ccc2 | W1 | Boucle de partie et phases 0/4/12/20 | 2 |
| 0x01ccc2-0x01d100 | V3 | Palettes et fondus | 13 |
| 0x01d100-0x01d584 | W1 | Boucle de partie et phases 0/4/12/20 | 1 |
| 0x01d584-0x01de7c | F5 | Curseur de sélection de personnage | 10 |
| 0x01de7c-0x01ee1a | V4 | Construction de la liste de sprites (OBJ RAM) | 15 |
| 0x01ee1a-0x01f200 | O2 | Pools d'objets et listes triées | 6 |
| 0x01f200-0x021000 | F6 | Interpréteur de scripts de scène (démos) | 18 |
| 0x021000-0x023800 | W2 | Sous-scènes de transition à fondus (phases 8/24) | 1 |
| 0x023800-0x026100 | P3 | Comportement scénarisé des joueurs par scène | 3 |
| 0x026100-0x028600 | X1 | Collisions (boîtes, coups, poussée) | 39 |
| 0x028600-0x029160 | K4 | Exceptions et écran de dump debug | 16 |
| 0x029160-0x02d300 | M1 | Menu TEST (service) | 76 |
| 0x02d300-0x02ede0 | M2 | EEPROM, configuration, comptabilité | 60 |
| 0x02ede0-0x035200 | B1 | Scrolling / caméras par scène | 42 |
| 0x035200-0x0398e2 | P4 | Noyau joueur (états, 26 actions génériques) | 46 |
| 0x0398e2-0x039ff0 | W0 | Dispatch des 10 classes d'objets | 10 |
| 0x039ff0-0x03bb00 | P4 | Noyau joueur (états, 26 actions génériques) | 13 |
| 0x03bb00-0x03e400 | O3 | Aides communes aux types (animations par type) | 25 |
| 0x03e400-0x049e00 | P5 | Code par personnage (6 × 2 variantes) | 73 |
| 0x049e00-0x051c64 | C12 | PEFF : effets du joueur | 107 |
| 0x051c64-0x054900 | C06 | PSHL : tirs du joueur | 39 |
| 0x054900-0x079400 | C02 | ENEMY : ennemis | 205 |
| 0x079400-0x09dbfc | C04 | BOSS : boss | 353 |
| 0x09dbfc-0x0a8000 | C08 | ESHL : tirs/attaques ennemis | 127 |
| 0x0a8000-0x0ba310 | C0e | EFFCT : effets | 170 |
| 0x0ba310-0x0d99f0 | C10 | SET : décors/animations de palette | 131 |
| 0x0d99f0-0x0dabca | C0a | BOX : caisses/coffres (h) | 7 |
| 0x0dabca-0x0dd61e | C0c | ITEM : objets ramassables | 14 |
| 0x0dd61e-0x0e0400 | C14 | SOBJ : objets système/UI | 25 |
| 0x0e0400-0x0e0836 | C1 | Crédits, pièces, start/continue | 3 |
| 0x0e0836-0x0ee080 | S1 | Son : wrappers de commandes + FIFO | 410 |
| 0x0ee080-0x0ee16e | K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 0 |
| 0x0ee16e-0x0f1500 | M3 | « Secret Test » (debug) et Block Test | 26 |
| 0x0f1500-0x0f2600 | P6 | Réactions aux coups, saisie des coups spéciaux | 9 |
| 0x0f2600-0x0f2e00 | M3 | « Secret Test » (debug) et Block Test | 3 |
| 0x0f2e00-0x0fa300 | W3 | Scripts de scène (phases 16/28) | 70 |
| 0x0fa300-0x0fb200 | F7 | Écrans titre / copyright | 5 |
| 0x0fb200-0x100000 | W3 | Scripts de scène (phases 16/28) | 12 |

## Annexe C. Fonctions du profil d'exécution → sous-système

| Fonction(s) | Sous-système | Rôle établi |
|---|---|---|
| `task_scheduler` 0x928, `0xa86` | K1 | boucle d'ordonnancement ; `0xa86` = cession de main (trap #4) utilisée par la tâche de gel |
| `0xd5e` | V2 | écrit un bloc de texte `[ligne, colonne, attr(mot), octets…, 0x40]` dans le scroll 2 (0x912000 + ligne·0x80 + colonne·4) |
| `0x1e2aa`, `0x1e062`, `0x1e3a6`, `0x1df3e`, `0x1de96` | V4 | construction de la liste de sprites ; `0x1e062` compile un objet (cadre `+0x34`), `0x1e2aa` sa variante palette/flip, `0x1e3a6` les objets à sprite unique (table 0x1e45c), `0x1df3e` parcourt la liste 1, `0x1de96` orchestre |
| `0x1324` | V2 | remplit un rectangle de tuiles espace (0x20) (36 appelants : HUD, textes) |
| `0xee044` | S1 | initialise le pointeur de pile privée des wrappers son (FF2A1A) |
| `0x1136` | V2 | écrit une chaîne de mots (`##` = nouvelle ligne) dans le scroll 2 |
| `0x1ed2` | O1 | test « dans l'écran » + rangement dans les 5 files de candidats aux collisions (`FFFE72-7A`, `FFFE7C-84`) ; positions comparées à la caméra `A5+0x4fe/0x54e` |
| `0x574` (VBLANK), `0x80c` | K1 | cf. §2.5 |
| `0x39xxx` | W0 / P4 / P1 | `0x398e2-0x39f82` = **boucles de dispatch des 10 classes** ; `0x392e4` = mise à jour des sous-objets joueur (armes F/B) ; `0x39260 player_walk_speed` ; `0x39ff0/0x3a0a8` = états joueur |
| `0x13c2c` | C1 | tâche crédits : démarrage/continue/rejoindre (4 slots) |
| `0x14652`, `0x14b74` | C1 | affichage des crédits (textes « CREDITS: », « COIN(S): ») |
| `0x143da`, `0x14322`, `0x140b4` | C1 | lecture des pièces en VBLANK ; impulsion compteur de pièces ; démarrage de partie d'un joueur |
| `0x1cfc8` | V3 | pas de fondu : approche chaque composante 4 bits d'une couleur (512 couleurs) de la cible |
| `0xf1884` | P6 | reconnaissance d'une séquence d'entrées (table d'octets, masque `+0x6C/6D`, compteur de délai) pour coups spéciaux |
| `0x155b2` | F3 | écrit des colonnes de texte (tuile, attr 0xE) depuis les tables 0x156ea/0x156ee dans le tilemap |
| `0xdd470` | C0c | test de ramassage d'un ITEM par les 4 joueurs (boîtes joueur 0xdd578, objet 0xdd590) |

## Annexe D. Régions de données ROM référencées (constantes immédiates `lea/movea.l #/pea` du code atteint)

| Région | Référencée par | Contenu (preuve) |
|---|---|---|
| 0x100000-0x10FFFF | joueurs, scène | tables de vitesses (symboles `tbl_walk_*` 0x1045de-0x104ffe), tables de physique ; 26 références P* |
| 0x110000-0x11FFFF | SET, PEFF, tâches d'écran | tables d'ombres (0x11bffa, 0x11c11a), listes de blocs |
| 0x120000 | ENEMY, BOSS, W3 | données de scène (h) |
| 0x150000-0x19FFFF | ENEMY (160000: 134 réf.), BOSS (170000: 51), ESHL (160000/170000), EFFCT (190000: 27), PSHL (150000: 23), PEFF (1b0000: 43) | **scripts d'animation et cadres de sprite** par famille (cf. `lea 0x1aef78,A4` dans PEFF type 0) |
| 0x1A0000-0x1AFFFF | PEFF | animations + table de noms d'ennemis (0x1a084) + textes CREDITS (0x1afd8-0x1b1c8) |
| 0x1B0000 | ITEM, BOX, EFFCT, SET, PEFF | animations d'objets divers |
| 0x1C0000 | SOBJ (0x1c03a6), toutes tâches | boîtes de collision (0x1ca538…), animations UI |
| 0x1D0000-0x1EFFFF | SET | données de décor/palette (h) |
| 0x1F0000, 0x240000, 0x250000 | toutes les tâches d'écran | tables de **blocs de tuiles 8×8** (0x1f1b60, 0x1f3b60, 0x1f9ba0, 0x2459c0) et palettes sources (0x25c0c0, 0x266fe0, 0x279460, 0x27cec0) |
| 0x280000-0x2AFFFF | SOBJ, SET, tâches | palettes (0x28ff00, 0x291f00, 0x28c6e0) et pages de couleurs |
| 0x300000-0x38FFFF | menu test | données du test (h) |
| 0x3F0000 | terrain (V2 `0x24a8`), joueurs, tâche 0x150ac | cartes de terrain/collision de scène (0x3f8e9c, 0x3f9c9c) |

## Annexe E. Chaînes et tables de noms utiles dans la ROM (lecture dans `prog_plain.bin`)

| Adresse | Contenu |
|---|---|
| 0x28ff3-0x2911a, 0x290a6-0x29124 | libellés des zones RAM (`GAME WORK`, `PLAYER 1-4`, `P1WEAPON F/B`…, `PSHL/PEFF/ENEMY/BOSS/ESHL/SET/EFFCT/ITEM/BOX/SOBJ WORK`) + adresses des pools |
| 0x1a084 | noms d'ennemis : Kobold, Skeleton, Troglodyte, Bow Gnoll, Owlbear, Ghoul, Hellhound, Wyvern, Gargoyle, GiantBeetle, GiantScorpion, mud golem, Shadow Elf, Axe Gnoll, Troll, Goblin, Riyakaa, Wyvern, Ogre, DisplacerBeast, ManScorpion, Harpy, Chimera, Spectre, Manticore, DarkWarrior 1/2, WarMachine, Nagpa, FrostSalamander, FlameSalamander, BlackDragon, GreenDragon, RedDragon, Beholder, ShadowElf, Katsuwo, Shin, Shin Dragon |
| 0xd6232 / 0xd628c | table de pointeurs (21) et noms de boss : Ogre, D.Beast, Man Scorpion, Harpy, Chimera, Ezerhorden, Manticore, Dark Warrior1/2, War Machine, Nagpa, Frost Salamander, Flame Salamander, Black Dragon, Green Dragon, Red Dragon, Beholder, Tel'Arin, Lich, (Black Dragon), Synn ; « Ogre Master Bros » |
| 0x1ddd4 | personnages : fighter, fighter2, cleric, cleric2, elf, elf2, dwarf, dwarf2, magic user, magic user2, dorobou, dorobou2 ; 0xb20e8 : Fighter, Cleric, Elf, Dwarf, Magic User, Thief |
| 0x2ba0c | GAME DATA (EEPROM compteurs) : FIGHTER1/2, CLERIC1/2, ELF1… ; 0x2c719 `BOSS n DEFEAT` |
| 0x29f22-0x2d27c | écrans TEST/CONFIGURATION/MEMORY TEST ; 0x2c042 modes monnayeur, 0x2c610 langues |
| 0xeee00-0xefe16, 0xf2ab2 | « Secret Test », éditeur de tables, noms de tables sprite `plXX emXX bsXX efXX pfXX psXX esXX weXX bxXX itXX st10`, Block Test |
| 0x128a6-0x13202, 0x13b56, 0x14ea6-0x15026, 0xfacc8-0xfb267 | avertissements région, Parental Advisory, copyrights TSR/CAPCOM par région |
| 0x2dba0-0x2dc90, 0x2ebfb | `D&D2 JPN/USA/HIS/ASI/EUR/BRA`, signature EEPROM `CAPCOMBGSDungeons&Dragons 2` |
| 0xe04e8 | « INSERT 2 COINS / CONTINUE REQUIRES ONLY 1 » |
| 0x13554-0x13b46 | libellés debug de la séquence d'attract (`shuukyaku demo`, `titile demo`, `play demo`, `ranking`, `nazo hint`, `shoukai demo`, `sound demo`, `eco demo`, `capcom demo`) |
| 0xd01e0-0xd0692 | crédits de fin (staff) |
