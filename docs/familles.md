# Familles d'objets ITEM, BOX et EFFCT

Gestionnaires de types réécrits en Rust dans `port/ddsom-game/src/families/` (`items.rs`, `boxes.rs`,
`effects.rs`, charpente commune dans `mod.rs`). Le modèle d'objets (pools, listes, animation, vitesses) est
décrit dans `docs/objets.md` ; la méthode générale dans `docs/rewrite-guide.md`.

## 1. Mécanisme

La boucle de dispatch d'une classe appelle, pour chaque objet actif, `jsr (table + 4 × type)` avec
`A0` = objet et `D7` = compteur de boucle (`0x39f34` ITEM, `0x39e50` BOX, `0x39cba` EFFCT). Un gestionnaire
rend la main par `rts` en laissant `A0` et `D7` intacts : c'est le seul contrat (`outputs` = A0, D7).
Chaque type est donc une routine native (`families::gestionnaire!`) qui remplace son point d'entrée.

Les gestionnaires sont des automates : octet d'état `+4` (0 initialisation, 2 actif, 4 libération, 6 posé par
le culling), sous-états `+5`, `+6`. Les sauts du 68000 se font par tables de mots relatifs dans le code
(`jmp (table,PC,D1.w)`) ; la réécriture les remplace par un `match` et garde l'adresse de la cible pour le
calcul de durée (`Ctx::saut_table`, qui pose aussi D0 et D1 comme l'original).

### Une seule logique pour le calcul et l'exécution

La durée d'un gestionnaire dépend de ses branchements et de ses sous-routines. La logique est écrite une
seule fois sur un contexte `Ctx` :

- **à blanc** (`families::cout`) : la logique s'exécute sur une copie de la machine ; chaque branchement
  conditionnel du code d'origine est noté par `Ctx::pris(condition)` (`vrai` = branchement pris) dans un
  `Chemin` ; `Chemin::duree_hors` additionne ensuite les durées des instructions du code d'origine le long de
  ce chemin (tronçons ouverts par `Ctx::at` / `saut_table`) ; les sous-routines appelées (`Ctx::jsr`, `natif`,
  `jsr_indirect`) ont été exécutées pour de vrai sur la copie et leurs cycles s'ajoutent ;
- **réel** (`families::executer`) : la même logique s'exécute sur la machine.

La durée est donc exacte (`exact_cost`) et le mode contrôle la vérifie à chaque appel : toute décision
oubliée ou inversée fait échouer le parcours (`Chemin` refuse les décisions en trop ou en moins) ou la
comparaison des cycles. Le coût de la copie de la machine est supporté tant que le moteur tourne avec les
routines 68000 ; il disparaîtra avec elles.

Règles d'écriture (apprises des erreurs trouvées par le contrôle) : une décision par `Bcc` ou `DBcc` de
l'original, dans l'ordre du code, avec la **polarité du branchement** (`beq` pris quand égal, `bne` pris
quand différent) ; un `bsr` local est suivi par le parcours, un `jsr`/`jmp` vers une routine non réécrite
est appelé (`Ctx::jsr`) et ses cycles viennent de l'exécution ; un `jsr (An)` ou un `jmp (An)` dynamique
s'écrit avec `jsr_indirect` (l'instruction est comptée à part) ; l'octet de drapeau de `alloc` est lu par
deux `bcs` successifs quand l'appelant teste la retenue après le `bcs` interne.

### Sous-routines, états délégués

- les routines du modèle d'objets sont appelées directement (`Ctx::natif`, `anim_start`, `anim_step`,
  `reinsert`, `alloc`, `free`) sans passer par l'index des adresses ; les autres par `Ctx::jsr` (sons,
  événements de score, création d'objets non réécrits...) ;
- `Ctx::jsr_origine` exécute le code d'origine seul : à utiliser quand le gestionnaire lit des registres
  laissés en reste par la sous-routine (le modèle d'objets ne les reproduit pas : voir EFFCT 26) ;
- **états non exercés ou non réécrits** : un état que le gestionnaire ne gère pas est délégué au code
  d'origine (`Ctx::deleguer(adresse)`, exécuté comme un `jmp` de la table, avec D0/D1 comme l'original) ;
  le comportement reste celui de l'original ; ces états sont listés ci-dessous ;
- **chemins hors contrat** : EFFCT 4 saute, pour le personnage 1, dans l'épilogue d'une autre routine
  (`movem.l (SP)+,{A1-A4} ; rts`, `beq.w 0xaa9fe`) : l'original ne revient donc pas à son appelant mais plus
  haut dans la pile (c'est ce qui arrive dans le scénario `bot_char2`). La routine native ne sait pas
  reproduire cela dans son contrat ; elle le détecte à blanc (`Ctx::epilogue_etranger`) et annonce un coût
  démesuré (`COUT_HORS_CONTRAT`) : l'exécuteur lance alors le code d'origine à sa place, exactement comme
  quand une routine ne tient pas avant l'échéance de la ligne.

### Outils de validation

- `tools/validate_all.sh` : les huit scénarios, routines en mode contrôle (obligatoire avant commit) ;
- `examples/types.rs` : appels de chaque gestionnaire des trois tables par scénario ; avec `TYPES_ETATS=1`,
  combinaisons (variante, états `+4/+5/+6`) rencontrées ;
- `examples/familles.rs` : contrôle détaillé des gestionnaires (RAM, registres, cycles) avec affichage de
  l'objet fautif et, avec `FAMILLES_TRACE=1`, trace de l'original ; les autres routines natives restent en
  mode contrôle pour suivre exactement la ligne du temps de l'original ; il affiche aussi la **couverture des
  décisions** (`COUV fichier ligne résultat`) : les branchements du Rust jamais pris ou jamais refusés par
  un scénario ne sont pas validés ;
- `examples/fuzz_familles.rs` : mêmes comparaisons sur des états réels **perturbés** (champs de l'objet, de
  son joueur et de la scène) et sur des objets **synthétiques** : le dernier argument
  (`ITEM:3,EFFCT:24...`) force des types, en changeant l'octet de type d'objets réels de la même famille
  (variante, états et pointeurs tirés au hasard). C'est ainsi que sont validés les types que les scénarios
  n'exercent jamais. `FUZZ_TRACE=1` affiche la trace de l'original en cas d'écart.

Validation obtenue : voir le tableau de chaque famille (colonne « validation »). « scénarios » : appelé par
`validate_all` (8 scénarios, 30 000 images, durées exactes comprises) et contrôlé appel par appel par
`familles` ; « fuzz » : seulement sur objets synthétiques ou perturbés (des dizaines de milliers d'essais par
type, écart nul).

## 2. ITEM (classe 0xC) : objets ramassables

Pool de 32 emplacements de 0x60 octets (`0xFFE2DE`), 10 types. Un ITEM naît à la position de son créateur
(ennemi, coffre, décor) ; en l'air (`Z` ≠ 0) il tombe, puis attend au sol un temps limité (0x4b0 à 0x960
images) en clignotant (bit 1 de `FLAGS`) avant de disparaître ; le code du joueur qui le touche pose `+0x58`
(ramassage) et `+0x50` (le joueur), et l'objet produit alors son effet.

Champs communs : `+4` état (0 init, 2 actif, 4 libération, 6 hors écran), `+5` sous-état (0 chute,
2 attente, 4 ramassé, 6 rejet pour les types à inventaire), `+0x2e` jeu d'ombres (0x3c), `+0x2f` mode de
création (1 : objet jeté par un joueur), `+0x42` durée de vie (mot), `+0x50` joueur, `+0x54` variante du
scintillement (EFFCT 15) créé par `0xae3ec`, `+0x58` ramassé (1 : ramassage, 2 : forcé), `+0x59` délai
(objet jeté, rejet), `+0x5d` indice dans le tableau des objets de collection `A5-0x5b88`.

| Type | Entrée | Nom | Comportement | Appels | Validation |
|---|---|---|---|---|---|
| 0 | 0xdabca | trésor | variantes 0 à 5 ; chute puis attente 0x4b0 images ; ramassé : son + événement de score `0x19ae6` | 1 248 | scénarios |
| 1 | 0xdae26 | consommable | 0-1 nourriture (+20/+40 PV, ou score si PV pleins), 2-3 potions (0x2d0 images), 4 sans effet, 5 compteur `A5+0x45` ; durée 0x960 | 16 180 | scénarios |
| 2 | 0xdb0c2 | arme ramassable (h) | cousin du type 3 (cinq gestionnaires de ramassage par emplacement) | 0 | code d'origine |
| 3 | 0xdb7de | équipement | six emplacements du joueur (`+0x140`) ; remplacement d'un équipement du même groupe, emplacement libre, échange, refus (rejet en arc) ; bonus `+0x18b..0x191` pour les variantes 0x24-0x27 | 0 | fuzz |
| 4 | 0xdbdf6 | objet d'inventaire | quantité + `+0x5a` (plafonnée à 9), emplacement vide, échange avec le premier emplacement (l'objet remplacé est recréé), refus selon la classe (rejet en arc) ; mode 1 : objet jeté | 21 775 | scénarios (refus exercé ; échange et quantités : fuzz) |
| 5 | 0xdc382 | bonus animé | mode de création → vitesses initiales ; ramassé : événement `0x19b08` | 50 180 | scénarios |
| 6 | 0xdc624 | objet de scène | 0 et 2 : icône qui suit un joueur et joue un son toutes les 0x3c images (envol et attente aussi) ; 1 : guide qui marche vers le joueur le plus proche et crée des SET ; 3 : voir le code | 10 387 | scénarios (variantes 0, 1, 2) |
| 7 | 0xdce6e | réserve du joueur | compteurs `+0x150 + variante` plafonnés à 9 pour les classes autorisées | 0 | fuzz |
| 8 | 0xdd132 | objet à contact | test de boîtes avec les quatre joueurs (`0xdd470`), crée un SET ; 3 variantes | 0 | fuzz |
| 9 | 0xdd5a2 | accompagnement | étincelle qui suit un objet pendant `+0x14` images | 0 | fuzz |

Délégués au code d'origine : type 2 entier ; type 6 variante 3 et état 4 de l'icône (variantes 0 et 2).

## 3. BOX (classe 0xA) : caisses et coffres

Pool de 21 emplacements de 0x80 octets (`0xFFEEDE`), 4 types.

| Type | Entrée | Nom | Comportement | Appels | Validation |
|---|---|---|---|---|---|
| 0 | 0xd99fa | caisse / tonneau / coffre | une fiche de la ROM (`0x1051ae`, par variante 0 à 3) donne la sorte (`+0x60` : < 3 se brise d'un coup, ≥ 3 coffre), les butins (`+0x62`, par classe de joueur `+0x63..`, aléatoires `+0x6a`) et un effet spécial éventuel (`+0x6b`) ; frappée (`+0x6e` ou `+0x33`), elle joue sa casse, lâche jusqu'à 3 ITEM (centre, droite, gauche) puis clignote et disparaît | 22 216 | scénarios (casse d'une caisse < 3 ; coffres et effets spéciaux : fuzz) |
| 1 | 0xda578 | barrière | variantes 0 et 1 ; frappée (`+0x6e`), crée un SET (`0x8b`) ou change l'état de l'objet frappeur | 0 | fuzz |
| 2 | 0xda7ac | décor | s'initialise, se trie chaque image | 0 | fuzz |
| 3 | 0xda7f2 | plateforme poussable (h) | | 0 | code d'origine |

États délégués dans le type 0 : sous-états 2, 4, 6 et 8 de l'état actif (éclats projetés, tremblement des
coffres), sous-sous-état 4 de l'état « touché ». Dans le type 1 : variantes 2 et 3.

## 4. EFFCT (classe 0xE) : effets visuels

Pool de 38 emplacements de 0x60 octets (`0xFFD49E`), 92 types (la table `0x39ce0` va jusqu'à `0x39e50`, début du dispatch BOX). Les gestionnaires n'ont pas de test
`HIDDEN` : l'état choisit directement init / actif / libération.

| Type | Entrée | Nom | Comportement | Appels | Validation |
|---|---|---|---|---|---|
| 1 | 0xa8758 | supprimé | libère l'objet aussitôt | 9 | scénarios |
| 4 | 0xaaac0 | bulle d'action | icône animée liée à un joueur ou à l'objet qu'il manipule, animation choisie d'après l'action `+0x3d` du porteur (huit variantes) ; code `0x29` : crée une bulle de variante 3 | 4 470 | scénarios (variantes 0, 2, 5 ; autres : fuzz) |
| 5 | 0xab726 | halo | position du joueur + décalage (variante, orientation), délai selon `+0x7d` du joueur, vit 0x28 images | 450 | scénarios |
| 6 | 0xab7fa | gerbe de débris | le premier crée ses frères, chacun lit sa vitesse dans une fiche (variante, mode) | 164 | scénarios |
| 7 | 0xaba4e | éclat à animation unique | | 0 | fuzz |
| 8 | 0xababa | trace | collée à un objet, animation selon son type, disparaît en fin d'animation | 0 | fuzz |
| 9 | 0xabbee | traînée | naît sur une position d'écran de la table, avance en X, attend 0x120 images puis 0x200 | 32 080 | scénarios |
| 15 | 0xae428 | scintillement | posé sur un ITEM, suit l'objet, attend le passage d'un joueur puis clignote 8 fois | 4 944 | scénarios |
| 17 | 0xae78c | éléments d'interface | 5 variantes (curseurs, fenêtres de jauge avec recopie de palettes, indicateurs) posées sur la caméra | 52 540 | scénarios |
| 19 | 0xaecce | balayage d'écran | | 0 | fuzz |
| 21 | 0xaf360 | bandeau | glisse en Z jusqu'à une cible puis décrémente le compteur de son créateur | 1 794 | scénarios |
| 24 | 0xb0ff2 | accessoire de personnage | collé au propriétaire à un décalage selon la variante | 0 | fuzz |
| 25 | 0xb111a | poussière | éclats projetés au hasard puis dérive en clignotant | 19 163 | scénarios |
| 26 | 0xb11f4 | nuage | traverse l'écran de droite à gauche (lit D0 après un tri : `jsr_origine`) | 1 849 | scénarios |
| 27 | 0xb12e6 | effet de ramassage | monte au-dessus du propriétaire 0x1c images | 0 | fuzz |
| 28 | 0xb136e | étincelle posée | collée à un objet, le prévient en fin d'animation | 0 | fuzz |
| 29 | 0xb1440 | étincelle d'impact | collée à une cible (ennemi ou boss), animation tirée au hasard | 572 | scénarios |
| 31 | 0xb1a98 | écran de décompte | variantes 3 (rang), 4 (plaque), 5 état 0 (lignes), 6 (compteurs sur 3 chiffres dessinés dans le fond 2), 8 (ligne par joueur) | 11 418 | scénarios (variantes 3 à 6 et 8 ; variante 5 état 2 : code d'origine) |
| 32 | 0xb2c56 | aura | suit son propriétaire | 4 663 | scénarios |
| 33 | 0xb2cca | éclat | retombe ; variante 0 : crée deux tirs ennemis à l'atterrissage | 257 | scénarios |
| 45 | 0xb5340 | déclencheur de proximité | attend le joueur principal, donne un événement à chaque joueur | 0 | fuzz |
| 51 | 0xb64c8 | projectile | part en arc d'un objet | 0 | fuzz |
| 52 | 0xb65e0 | scintillement de personnage | variante du 15 pour la classe 5 | 0 | fuzz |
| 56 | 0xb6c6e | éclats de débris | | 0 | fuzz |
| 57 | 0xb6bca | marque | collée à un joueur dans un état donné | 0 | fuzz |
| 62 | 0xb77ac | gerbe d'éclats | explose en neuf objets quand l'objet suivi est touché | 0 | fuzz |
| 72 | 0xb841c | détecteur de ramassage | un joueur marche dessus (6 en profondeur, 8 en x) : crée l'éclat ESHL 55 (`0xa532e`) ; variante : crée le clignotement 79 | 6 409 | scénarios (`families/effects_b.rs`) |
| 73 | 0xb84c6 | écran de victoire | sprite qui tombe, clignote, change d'animation selon l'état de son créateur | 180 | scénarios |
| 74 | 0xb85fa | bulle du créateur | posée à côté de lui, monte, clignote, disparaît | 277 | scénarios |
| 75 | 0xb871a | bouton de menu | machine à sept sous-états pilotée par `A5+0x2e` et `0xff2a38` | 3 582 | scénarios |
| 79 | 0xb8df2 | clignotement | clignote tant que son créateur vit ; détecte un joueur de classe 5 voisin | 1 032 | scénarios |

Exercé par les scénarios et laissé en code d'origine (dans un type par ailleurs réécrit) : type 31
variante 5 état 2 (dessin des lignes de score, 3 583 appels). Types jamais exercés et non réécrits : 0, 2,
3, 10 à 14, 16, 18, 20, 22, 23, 30, 34 à 44, 46 à 50, 53 à 55, 58 à 61, 63
(pour 18, la table renvoie à `0xcc25a`, un gestionnaire commun ; 22, 23, 42, 14 et 11 sont de grosses
routines de scène de plusieurs kilo-octets).

## 5. Résultats de la validation

- `tools/validate_all.sh 30000` : les huit scénarios sont `VALIDE`, 0 écart pour toutes les routines de
  famille (durées exactes comprises) : `item_tresor` 1 248 appels, `item_conso` 16 180, `item_inventaire`
  21 775, `item_bonus` 50 180, `item_scene` 10 387, `box_caisse` 22 216, `effct_interface` 52 540,
  `effct_trainee` 32 080, `effct_poussiere` 19 162, `effct_decompte` 11 418, `effct_bulle` 4 453 (17 appels
  du chemin hors contrat passent par le code d'origine), `effct_scintillement` 4 944, `effct_aura` 4 663,
  `effct_nuage` 1 849, `effct_bandeau` 1 794, `effct_impact` 572, `effct_halo` 450, `effct_eclat` 257,
  `effct_debris` 164, `effct_supprime` 9.
- `examples/familles` (contrôle appel par appel, 30 000 images, huit scénarios) : 0 écart.
- `examples/fuzz_familles` (huit scénarios, 8 000 images, 38 types forcés, 750 000 essais sur la dernière
  série, plusieurs centaines de milliers d'autres lors de la mise au point) : 3 écarts, tous sur ITEM 1 avec
  une variante hors table (12) où l'original saute dans des données ; aucune panique.
- **Couverture des décisions** (`COUV`, scénarios + fuzz) : sur environ 350 sites de décision, 108 n'ont vu
  qu'une seule issue. Les principales issues jamais observées : pool plein à la création d'un objet (butins,
  bulle, éclats), nourriture (soins, PV pleins) et potions de ITEM 1 (variantes 2 à 5), échange ou fusion
  d'équipement de ITEM 3 (bonus `+0x18b..`, groupes identiques), recouvrement des boîtes de ITEM 8
  (variantes autres que 0), bulle d'action d'un joueur portant un objet (EFFCT 4 variante 0, actions 16 et 18),
  jauge de l'interface (EFFCT 17 variante 2, sortie de clignotement), variantes 3 et 6 de la scène de ITEM 6 (SET créé
  hors scénario). Ce sont des branches courtes et lues sur le listing, mais **non validées** : un défaut
  éventuel s'y cacherait.
- Limites connues : une variante ou un état hors table fait sauter l'original dans des données ; la
  réécriture délègue au code d'origine avec D0, D1 et A1 posés comme lui, ce qui est exact dans tous les cas
  rencontrés sauf ITEM 1 pour une variante >= 6 (objets qui n'existent pas dans le jeu).
