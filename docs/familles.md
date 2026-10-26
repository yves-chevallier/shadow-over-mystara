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
- `Ctx::reinsert` passe par la native des listes triées (`objects::lists::REINSERT_ACTORS`, registres exacts) ;
- `Ctx::sans_chemin(cible, corps)` exécute en Rust, sans relevé de chemin, un état ouvert par `saut_table` : sa durée
  est celle de ses sous-routines (les durées exactes sont obsolètes) ;
- `Ctx::jsr_origine` exécute le code d'origine seul : à utiliser quand le gestionnaire lit des registres
  laissés en reste par la sous-routine (le modèle d'objets ne les reproduit pas : voir EFFCT 26) ;
- **états non exercés ou non réécrits** : un état que le gestionnaire ne gère pas est délégué au code
  d'origine (`Ctx::deleguer(adresse)`, exécuté comme un `jmp` de la table, avec D0/D1 comme l'original) ;
  le comportement reste celui de l'original ; ces états sont listés ci-dessous ;
- **retour par-dessus l'appelant** : EFFCT 4 saute, pour le personnage 1, dans l'épilogue d'une autre routine
  (`movem.l (SP)+,{A1-A4} ; rts`, `beq.w 0xaa9fe`) : l'original ne revient donc pas à son appelant mais plus
  haut dans la pile, au début du tour de la boucle de partie `0x1c468` (scénario `bot_char2`, entre autres). La
  routine native fait de même (`Ctx::epilogue_etranger`) : les appels en cours tenus en Rust (aiguillage des
  EFFCT, mise à jour du monde, fin d'image) sont sur la pile 68000, où `call_at` les a remis
  (`docs/noyau.md`, « Appels en cours en Rust ») ; elle en dépile quatre longs, revient au cinquième, et `call_at`
  abandonne les fonctions Rust dépassées (`Unwound`). Il n'y a plus de chemin « hors contrat » où le code
  d'origine s'exécute à la place d'une native (l'ancien coût démesuré `COUT_HORS_CONTRAT` a disparu).

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
| 2 | 0xdb0c2 | pièce d'équipement à emplacement | fiche par variante (table en clair `0xdb522`) : emplacement du joueur (`+0x5b`, huit gestionnaires `0xdb392` : `+0x160`, rien, anneau `+0x162`, arme spéciale `+0x163`, `+0x164`, bouclier `+0x165`, `+0x166`, `+0x167`), identifiant, scintillement, coups, indice de collection, SET 0xa d'annonce ; ramassée, elle remplace la pièce portée, rejetée en arc (ITEM 2 de mode 1) ; déjà portée : événement de score | 3 000 | scénarios (`items_b.rs`, sur `Cx`) |
| 3 | 0xdb7de | équipement | six emplacements du joueur (`+0x140`) ; remplacement d'un équipement du même groupe, emplacement libre, échange, refus (rejet en arc) ; bonus `+0x18b..0x191` pour les variantes 0x24-0x27 | 0 | fuzz |
| 4 | 0xdbdf6 | objet d'inventaire | quantité + `+0x5a` (plafonnée à 9), emplacement vide, échange avec le premier emplacement (l'objet remplacé est recréé), refus selon la classe (rejet en arc) ; mode 1 : objet jeté | 21 775 | scénarios (refus exercé ; échange et quantités : fuzz) |
| 5 | 0xdc382 | bonus animé | mode de création → vitesses initiales ; ramassé : événement `0x19b08` | 50 180 | scénarios |
| 6 | 0xdc624 | objet de scène | 0 et 2 : icône qui suit un joueur et joue un son toutes les 0x3c images (envol et attente aussi) ; 1 : guide qui marche vers le joueur le plus proche et crée des SET ; 3 : voir le code | 10 387 | scénarios (variantes 0, 1, 2) |
| 7 | 0xdce6e | réserve du joueur | compteurs `+0x150 + variante` plafonnés à 9 pour les classes autorisées | 0 | fuzz |
| 8 | 0xdd132 | objet à contact | test de boîtes avec les quatre joueurs (`0xdd470`), crée un SET ; 3 variantes | 0 | fuzz |
| 9 | 0xdd5a2 | accompagnement | étincelle qui suit un objet pendant `+0x14` images | 0 | fuzz |

Plus rien n'est délégué au code d'origine dans les types exercés : type 6 variante 3 (guide de l'attract, palette
puis marche et SET 0x26) et état 4 de l'icône (départ : marche hors de l'écran ou arrêt avec SET) sont écrits sur un
`Ctx` sans relevé de chemin (`Ctx::sans_chemin`, à la place du `deleguer` de l'état). Seuls les indices hors table
restent confiés au code d'origine (sauts dans des données).

## 3. BOX (classe 0xA) : caisses et coffres

Pool de 21 emplacements de 0x80 octets (`0xFFEEDE`), 4 types.

| Type | Entrée | Nom | Comportement | Appels | Validation |
|---|---|---|---|---|---|
| 0 | 0xd99fa | caisse / tonneau / coffre | une fiche de la ROM (`0x1051ae`, par variante 0 à 3) donne la sorte (`+0x60` : < 3 se brise d'un coup, ≥ 3 coffre), le verrou (`+0x61`), les butins (`+0x62`, par classe de joueur `+0x63..`, bonus `+0x69`, aléatoires `+0x6a`) et un effet spécial éventuel (`+0x6b`) ; frappée (`+0x6e` ou `+0x33`), elle joue sa casse, lâche jusqu'à 3 ITEM (centre, droite, gauche) puis clignote et disparaît ; un coffre tremble ; ouvert par un joueur (sous-état 2), il consomme une clé `A5+0x45` s'il est verrouillé (sauf classe 5) ; portée (4), lancée (6 : vol, rebond sur le décor, casse au sol) ou lâchée (8) | 22 216 | scénarios (`boxes::caisse`, sur `Cx` ; tous les sous-états exercés sauf les variantes de lancer de coffre) |
| 1 | 0xda578 | barrière | variantes 0 et 1 ; frappée (`+0x6e`), crée un SET (`0x8b`) ou change l'état de l'objet frappeur | 0 | fuzz |
| 2 | 0xda7ac | décor | s'initialise, se trie chaque image | 0 | fuzz |
| 3 | 0xda7f2 | bloc poussable | poussé par un joueur dans l'action 0x2e (le joueur s'enregistre en `+0x6c`) : variante 0 glisse en X entre `+0x60` et `+0x62` (pas de 2 si le bit 4 de `+0x6e` du joueur, 1 de plus si `+0x33`), accompagnée d'un SET 7 qu'elle prévient en s'arrêtant ; variante 1 glisse en hauteur (directions 2/4) ou en X (6/8) et se recale sur le décor (0x33fe4, 0x35c0) à chaque pas ; le joueur est replacé contre le bloc (décalages par personnage, tables en clair 0xdab52...) | 18 294 | scénarios (`families/boxes.rs`, `pousse`, cadre `Cx`) |

Le type 0 n'a plus d'état délégué (anciennement : sous-états 2, 4, 6 et 8 de l'état actif et tremblement des
coffres). Encore délégués dans le type 1 (jamais exercés) : variantes 2 et 3.

Sous-routines de la famille appelées hors des gestionnaires (natives de `items.rs`) : `0xdabba` efface le tableau des
objets de collection (48 longs en `A5-0x5b88`, appelé par la boucle de partie et les modes) ; `0xdb6c0` fait tomber au
plus un équipement du joueur touché (bouclier `+0x165`, arme spéciale `+0x163`, anneau `+0x162`), chacun avec un compteur
de coups (`+0x16d`, `+0x16c`, `+0x16b`) et une table de chances par coup (bit `rng & 0xf`, tables en clair 0xdb7c4,
0xdb7b8, 0xdb7d0), par `0xcc352` (SET 0xa, native de `creators.rs`) ; le personnage 5 garde son anneau d'origine (1).

## 4. EFFCT (classe 0xE) : effets visuels

Pool de 38 emplacements de 0x60 octets (`0xFFD49E`), 92 types (la table `0x39ce0` va jusqu'à `0x39e50`, début du dispatch BOX). Les gestionnaires n'ont pas de test
`HIDDEN` : l'état choisit directement init / actif / libération.

Les types de la première série sont écrits sur `Ctx` (`effects.rs`, ancienne méthode à durée exacte) ;
les suivants sur `Cx` (`effects_b.rs` : 72 à 79 ; `effects_c.rs` : types de scène et d'ennemis ;
`effects_d.rs` : type 31 ; `effects_e.rs` : types 11, 13, 14 ; `effects_f.rs` : types 0, 35, 53, 54, 55, 68, 77,
89 ; `effects_g.rs` : types 43, 46, 48, 63, 80, 81 ; `effects_creators.rs` : créateurs d'effets). Les ESHL 70 à 72 (`0xa801a`, `0xa81ca`,
`0xa838e`), derniers types de la table ESHL `0x39b96` rangés dans la zone des EFFCT, sont dans `eshl_d.rs`. « Appels » : appels de la routine native sur les
121 scénarios de `scenarios/` (validate_all, 90 scénarios) et `scenarios/cov` (31 scénarios). Les variantes
ou états qu'aucun scénario n'exerce sont réécrits et validés par fuzz (voir « Recensement » ci-dessous),
indiqué dans la colonne « validation ».

Points délicats rencontrés :

- **fonctions sœurs qui tombent l'une dans l'autre** : beaucoup d'états enchaînent l'état suivant dans la
  même image sans `rts` (types 50, 59, 61, 84) ; pour le type 61, l'étape 8 achevée tombe *toujours* dans
  le code de l'étape 12 de la variante 0, quelle que soit la variante ;
- **registres laissés par l'original** : `0xab2b4` (étincelle posée entre deux boîtes, appelée par
  `effects_aids::spark`) laisse D2/D3 que des appelants relisent (impact créé par `0x103ba` → `0xb141a`) ;
  la réécriture les rend comme l'original. Les types 13 (radeau) et 83 (pluie) lisent D4 / A1 laissés à
  l'entrée (A1 = adresse du gestionnaire, posée par la boucle de dispatch) ;
- adresses `lea (d16,PC)` : à prendre dans `examples/dis` (`PcDisp`), pas à recalculer de tête (une erreur
  d'addition sur le type 23 a coûté une itération).

| Type | Entrée | Nom | Comportement | Appels | Validation |
|---|---|---|---|---|---|
| 0 | 0xa8594 | décor d'ouverture | glisse vers sa place, puis met fin aux objets de deux pools (état 6 ; un objet marqué `+0x58` arrête le premier parcours, qui finit sans le second, comme l'original) ; variante 3 : attend, fait avancer la scène (`A5+0x4ff`), suit le défilement de la caméra | — | scénarios (`effects_f.rs`) |
| 1 | 0xa8758 | supprimé | libère l'objet aussitôt | 499 | scénarios |
| 3 | 0xaa8e8 | élément d'écran | fixé à la caméra, hauteur suivant le tremblement `A5+0x4ba` | 13 284 | scénarios |
| 4 | 0xaaac0 | bulle d'action | icône animée liée à un joueur ou à l'objet qu'il manipule, animation selon l'action `+0x3d` du porteur (huit variantes) ; code `0x29` : bulle de variante 3 et étincelle d'impact (`0xb13fa`) ; actions 22 à 32 de la variante 0 (jauge `+0x77`) | 175 021 | scénarios ; action 20 (code de l'action 18) : fuzz ; actions > 32 hors table |
| 5 | 0xab726 | halo | position du joueur + décalage, délai selon `+0x7d`, vit 0x28 images | 6 172 | scénarios |
| 6 | 0xab7fa | gerbe de débris | le premier crée ses frères, vitesses lues dans une fiche | 39 486 | scénarios |
| 7 | 0xaba4e | éclat à animation unique |  | 499 | scénarios |
| 8 | 0xababa | trace | collée à un objet, animation selon son type | 657 | scénarios |
| 9 | 0xabbee | traînée | naît sur une position d'écran, avance en X | 153 984 | scénarios |
| 10 | 0xabccc | dalles | rangée en diagonale de dix dalles (chacune crée la suivante) qui s'enfoncent sous un joueur au sol (décor SET 6) | 14 314 | scénarios |
| 11 | 0xabeea | effets d'attaque | dix variantes : éclat posé au hasard autour de l'attaquant (0), éclat lancé depuis l'attaquant (1), marque qui fait avancer l'attaquant (2), reflet (3), colonne montante (4), traînée liée à une action (5), pièces d'un objet brisé (6), gerbe d'étincelles (7), gerbes d'éclats qui suivent puis retombent (8, 9) | 23 406 | scénarios ; variantes 0, 2, 5, 8, 9 : fuzz et `cov_effct_rares` (0, 8) |
| 12 | 0xad476 | maillon d'une chaîne | rejoue avec retard l'historique des directions du maillon précédent | 14 180 | scénarios ; rupture (`0xad53a`) : fuzz |
| 13 | 0xad63e | radeau | défile et emporte les joueurs, piloté par leurs directions | 26 597 | scénarios |
| 14 | 0xad866 | passage | oscille, attend un joueur vivant, ferme puis ouvre un rideau de tuiles ; entrée dans la sous-scène (`0xada96` : caméras, défilement et musique sauvés dans l'objet, objets des six pools gelés `+0x40`, décor `0xcc200`) puis retour (`0xae11a` : contexte rendu, joueurs replacés devant l'objet suivi) | 40 108 | scénarios |
| 15 | 0xae428 | scintillement | posé sur un ITEM, clignote au passage d'un joueur | 200 044 | scénarios |
| 16 | 0xae52a | double d'un porteur | glisse à sa place puis suit les étapes du porteur, crée un morceau (type 39) | 19 254 | scénarios |
| 17 | 0xae78c | éléments d'interface | 5 variantes posées sur la caméra | 914 180 | scénarios |
| 19 | 0xaecce | balayage d'écran |  | 2 168 | scénarios |
| 20 | 0xaed4c | curseur de don d'or | boutique : déplacé par le joueur, donne 10 ou 100 pièces (BCD) au joueur visé ; dessine la jauge de vie et l'or dans la couche de texte | 55 209 | scénarios |
| 21 | 0xaf360 | bandeau | glisse en Z jusqu'à une cible | 24 768 | scénarios |
| 22 | 0xaf3f2 | comptoir de boutique | zone de vente : annonce l'article au premier acheteur qui entre | 550 750 | scénarios ; achat (`0xafad0` : 78 entrées `bra.w`, sortie, repas, articles avec refus par classe, paiement BCD) : fuzz et `cov_effct_achat` |
| 23 | 0xafe20 | vote du chemin | scrutin (curseur par joueur, compte à rebours, dépouillement au hasard parmi les ex æquo, clignotement du texte gagnant) ; texte en mots (`A5+0x72` = 2) ou en octets (autres langues : 0xff commande de 3 octets, 0xfe de 2, octet ≤ 0x0f caractère de 2 octets) ; minuteur : états 4 (aucune étape n'y mène, relit A6) et 6 (retrait hors écran) | 8 094 | scénarios ; états 4 et 6 du minuteur : fuzz |
| 24 | 0xb0ff2 | accessoire de personnage |  | 6 072 | scénarios |
| 25 | 0xb111a | poussière |  | 753 125 | scénarios |
| 26 | 0xb11f4 | nuage |  | 10 057 | scénarios |
| 27 | 0xb12e6 | effet de ramassage |  | 1 020 | scénarios |
| 28 | 0xb136e | étincelle posée |  | 1 731 | scénarios |
| 29 | 0xb1440 | étincelle d'impact |  | 31 025 | scénarios |
| 30 | 0xb1818 | poissons | banc qui traverse en ondulant ; poisson qui nage, saute quand l'écran tremble | 19 480 | scénarios |
| 31 | 0xb1a98 | écran de décompte | dix variantes : pièce projetée (0), trophée du tableau de fin qui s'anime au signal de son contrôleur puis se décompte (1), récompense (2), rang (3), plaque (4), ligne de résultat dessinée dans le fond 2 (5), compteurs (6), personnage qui marche (7), ligne par joueur (8), enseigne sur la caméra alternative (9) | 200 664 | scénarios ; variante 9 : non exercée |
| 32 | 0xb2c56 | aura |  | 21 890 | scénarios |
| 33 | 0xb2cca | éclat |  | 0 | scénarios du lot précédent (8 scénarios) |
| 35 | 0xb2f40 | silhouette et reflet | d'un porteur (`+0x46`, créés par `0xb2ef6`) : la silhouette s'anime et transmet son image au porteur (`+0xa0`), le reflet la traduit en animation (table `0xb300c`) | — | scénarios (`effects_f.rs`) |
| 34 | 0xb2dea | plateau | avance en emportant les joueurs jusqu'à ce que son décor l'arrête | 2 424 | scénarios |
| 36 | 0xb3050 | objet flottant | scène d'introduction : ondule, est touché (secousse), s'efface ; son reflet | 61 009 | scénarios |
| 37 | 0xb33ba | titre d'épreuve | descend puis attend son créateur | 305 | scénarios |
| 38 | 0xb3454 | message d'épreuve | tombe, clignote, change avec son créateur | 3 274 | scénarios |
| 39 | 0xb3570 | morceau projeté | vole vers un point de l'écran (durée = distance / vitesse), éclate au sol (type 50) | 14 202 | scénarios |
| 40 | 0xb37ac | effets d'un coup | éclat qui suit l'attaquant (0), éclat posé devant lui (1), éclat qui tombe du haut de l'écran (2), débris projeté (4, et 3 tant que l'attaque dure) | 6 688 | scénarios ; variantes 1 à 3 : fuzz et `cov_effct_rares` (1, 2) |
| 41 | 0xb3c48 | objet lancé | quatre sortes tirées au hasard, retombe puis glisse ou rebondit | 579 | scénarios |
| 42 | 0xb40f8 | effets de scène | éclat bref (0), débris lancé (1) ou repoussé avec les joueurs (2), débris, éclats, caméra alternative avec 8 images de retard, étincelles, lueur, bouffées, éclat projeté au hasard (9), poussière | 81 110 | scénarios ; variantes 1, 2, 9 : fuzz et `cov_effct_rares` |
| 43 | 0xb4bec | pluie de débris | en haut de l'écran avec une poussière au départ (réglage 0) ou depuis le créateur (`+0x5c`), vitesses et animation tirées au hasard ; tombe, clignote, disparaît (ou avec le créateur) | — | fuzz et `cov_effct_rares` (`effects_g.rs`) |
| 44 | 0xb4f88 | sorts d'un magicien | six modes liés à `+0x5c` | 3 545 | scénarios |
| 45 | 0xb5340 | déclencheur de proximité |  | 35 669 | scénarios |
| 46 | 0xb5454 | pièces d'armure | suivent le porteur (ENEMY, `+0x46`) et rejouent ses coups reçus, tombent quand sa vie passe sous leur seuil, se brisent en éclats (variante 1) qui rebondissent | — | fuzz et `cov_effct_rares` (`effects_g.rs`) |
| 47 | 0xb5764 | repère | clignote lentement | 1 810 | scénarios |
| 48 | 0xb59ea | machine volante | canon qui tourne autour de son porteur et tire des ESHL 0x2c (1, avec son corps, 0), balayeur au bord de l'écran (2) | — | fuzz et `cov_effct_rares` (`effects_g.rs`) |
| 49 | 0xb5dbe | accessoire porté | tombe quand le porteur passe dans un état donné | 34 198 | scénarios |
| 50 | 0xb5fb6 | effets attachés | seize variantes (reflets, éclats, tremblement) | 211 697 | scénarios |
| 51 | 0xb64c8 | projectile |  | 0 | fuzz (lot précédent) |
| 52 | 0xb65e0 | scintillement de personnage |  | 205 132 | scénarios |
| 53 | 0xb66f8 | traversée | traverse l'écran en cloche, glisse, crée un décor (`0x83`) et attend qu'il ait fini, puis crée le type 55 ; ses étapes s'enchaînent dans la même image | — | scénarios (`effects_f.rs`) |
| 54 | 0xb68f4 | pièces du passage | trois variantes (`0xb68b0`) ; quand l'objet suivi passe à l'état 4, la variante 0 recule la scène et les joueurs de 0x7c8, lance un fondu (tâche `0x1ced4`) puis crée un boss (BOSS 7) | — | scénarios (`effects_f.rs`) |
| 55 | 0xb6a74 | lanceur | délai, puis projette les joueurs, sauve la palette, lance un fondu (`0x1ccf8`) et remplit le pool d'éclats (type 56 : la boucle `dbf D6` partage D6 avec l'allocation, qui le remet à jour) | — | scénarios (`effects_f.rs`) |
| 56 | 0xb6c6e | éclats de débris |  | 0 | fuzz (lot précédent) |
| 57 | 0xb6bca | marque |  | 0 | fuzz (lot précédent) |
| 58 | 0xb6d48 | machine d'introduction | même séquence que le type 36, et son reflet | 53 545 | scénarios |
| 59 | 0xb6ef2 | créature surgissante | s'élève avec la caméra, tire, bondit hors de l'écran | 5 244 | scénarios |
| 60 | 0xb7248 | second double | comme le 16, avec éclats et morceau projeté | 9 234 | scénarios |
| 61 | 0xb751e | créature nageuse | sort de l'eau, ondule, s'en va | 10 909 | scénarios |
| 62 | 0xb77ac | gerbe d'éclats |  | 9 683 | scénarios |
| 63 | 0xb7866 | morceau projeté par un ennemi | (ENEMY 22, 23) part en arc depuis le créateur (`+0x42`), retombe avec des débris, attend, clignote | — | fuzz et `cov_effct_rares` (`effects_g.rs`) |
| 64 | 0xb7a14 | éclat tombant | projeté, retombe sous l'horizon | 3 490 | scénarios |
| 65 | 0xb7ac8 | messager | suit un trajet (directions et durées) puis fait naître un ENEMY décrit par une fiche | 19 | scénarios |
| 66 | 0xb7c40 | gros caractères | quatre blocs animés dans le fond 2 | 1 366 | scénarios |
| 67 | 0xb7e26 | colonne | s'élève sous un objet, relâche les joueurs | 6 080 | scénarios |
| 68 | 0xb7f28 | enseigne de départ | posée sur la caméra, couleurs chargées (`0x28ff20`) ; à la fin de son animation, les joueurs vivants debout repartent (`0x02002c00`) | — | scénarios (`effects_f.rs`) |
| 69 | 0xb802a | silhouette | fait les cent pas en haut de l'écran | 4 562 | scénarios |
| 70 | 0xb80bc | comptoir de scène | zone de vente du type 22 ; vente : vendeur (décor 0x64), enseigne, articles posés (PEFF 0x2a), acheteur devant le vendeur | 4 188 | scénarios ; vente (`0xb81d6`, `0xb83d8`) : fuzz et `cov_effct_vente` |
| 72 | 0xb841c | détecteur de ramassage |  | 365 616 | scénarios |
| 73 | 0xb84c6 | écran de victoire |  | 1 156 | scénarios |
| 74 | 0xb85fa | bulle du créateur |  | 1 382 | scénarios |
| 75 | 0xb871a | bouton de menu |  | 36 572 | scénarios |
| 76 | 0xb88a8 | minuteur de scène | joue un son, lance une transition après un délai | 2 528 | scénarios |
| 77 | 0xb896e | fondu sonore | deux niveaux (commandes 0xff08 / 0xff09) qui montent par paliers de 5 images, puis arrêt (0xff07) | — | scénarios (`effects_f.rs`) |
| 79 | 0xb8df2 | clignotement |  | 46 429 | scénarios |
| 80 | 0xb8ef6 | feu d'artifice | scène d'un personnage (`0x23606`) : le héros tire des fusées pendant un fondu au noir, cède la main quatre images (`0xa40`, reprises natives `0xb91d0` et `0xb8f04`), s'envole et termine la scène ; spectateurs et acolytes (1 à 6) | — | fuzz (reprises comprises) et `cov_effct_rares` (6) (`effects_g.rs`) |
| 81 | 0xb96dc | éclat lancé | depuis la main d'un personnage (`+0x42` geste), glisse en arc, clignote | — | fuzz et `cov_effct_rares` (`effects_g.rs`) |
| 82 | 0xb97d8 | éclat posé | à côté de son créateur, clignote | 0 | fuzz (`fuzz_effct`, 76 instructions atteignables sur 77 vues) |
| 83 | 0xb997c | pluie d'éclats | gouttes en arc depuis le haut de l'écran | 58 498 | scénarios |
| 84 | 0xb9aba | séquence de fin | fondu, caméra, tâches de script (`0x9b6`) | 2 198 | scénarios |
| 85 | 0xb9bbc | pièce de la fin |  | 20 426 | scénarios |
| 86 | 0xb9ca6 | flammèche | s'éloigne de son créateur | 3 982 | scénarios |
| 87 | 0xb9db8 | interrupteur | signale un joueur ou un tonneau (BOX 3 variante 1) au sol dessus | 366 780 | scénarios |
| 88 | 0xb9f36 | barrière | crée ses interrupteurs, s'ouvre quand tous sont appuyés | 127 098 | scénarios |
| 89 | 0xba0b0 | passant | entre par la gauche, attend le signal de la scène (`A5-0x55c8` = 0x14), ressort par la droite | — | scénarios (`effects_f.rs`) |
| 90 | 0xba1aa | poussière haute |  | 1 625 | scénarios |
| 91 | 0xba244 | élément d'écran | comme le type 3 | 3 321 | scénarios |

### Recensement de la table des types (`0x39ce0`, 92 entrées)

Méthode : chaque entrée de la table et chaque table de sauts des gestionnaires (variante, état, sous-état,
action, sorte d'achat), bornée par le premier code qu'elle désigne (au-delà, l'original saute dans des
données : hors contrat) ; `examples/recensement_effct.rs` relève le code d'origine que les natives exécutent
encore sur des trajectoires synthétiques ; les créateurs sont recherchés par les appels de l'allocation
`0x2318`, les scripts d'apparition des scènes (classe EFFCT de `0xf2f38`) et les références aux routines de la
zone depuis le reste du programme.

- **Toutes les entrées atteignables sont natives** : 88 gestionnaires (dont 18 = gestionnaire commun des SET
  `0xcc25a`, `set.rs`), leurs variantes et états, les 12 créateurs de la zone qui restaient en 68000 (`0xb3e4e`,
  `0xb3ec6`, `0xb4bcc`, `0xb4f1c`, `0xb53f4`, `0xb59a6`, `0xb59c8`, `0xb64b0`, `0xb96c0`, `0xb97a2`,
  `0xb97ba`, bloc de tuiles `0xb230e`, `effects_creators.rs`) et les deux points de reprise du type 80.
- **Validation** : « scénarios » dans le tableau ci-dessus ; les chemins qu'aucun scénario n'exerçait sont
  validés par `examples/fuzz_effct.rs` (trajectoires de 60 images depuis la naissance ou un état de la table,
  propriétaire et liens réglés par type, comparaison à l'original sur une copie, couverture des instructions
  atteignables du gestionnaire : toutes vues, aux artefacts de désassemblage près) et, quand c'est faisable, par
  `scenarios/cov/cov_effct_rares.txt` (objets créés par poke pendant la scène 10), `cov_effct_achat.txt` (scène
  69) et `cov_effct_vente.txt` (scène 81). Le fuzz a corrigé deux défauts de natives anciennes jamais exercés :
  état 6 du minuteur du vote (23) ignoré, état des joueurs relancés par l'enseigne de départ (68 : `0x02002c00`).
- **Code mort** (aucune création possible) : types 2 (`0xaa71a` ; son seul créateur `0x17626`, dans l'interface
  joueurs, suit un `bra` et n'est référencé par rien), 71 (`0xb841a`, un `rts`) et 78 (`0xb8a1e`) : aucun appel de
  `0x2318` ni script de scène ne crée ces types. Laissés en 68000, non réécrits.
- **Hors contrat, laissés à l'original** : pool plein dans la naissance du curseur de don d'or (20 : `0xaf2bc`,
  `tst.w (1,A5)`, erreur d'adresse) et dans la créature nageuse (61 : `0xb778c`, saut dans des données) ; texte du
  vote dont le pointeur `+0x30` serait impair en mots (erreur d'adresse dans l'original, seul écart du fuzz sur le
  type 23).
- **Hors périmètre** : le type 44 appelle les créateurs de tirs ESHL `0xa31c2` et `0xa3354` (zone ESHL, encore en
  68000) ; les articles posés par la vente du type 70 sont des PEFF 0x2a (`0x50b9c`, code d'origine).
- Reste mesuré (`examples/reste68k`, scénarios `cov_effct_*`) : aucune instruction 68000 dans la zone EFFCT
  (`0xa8594-0xba310`).

## 5. Résultats de la validation

Lot EFFCT sur `Cx` (types 3 à 91 ci-dessus) :

- `tools/validate_all.sh 30000` (90 scénarios) et les 31 scénarios de `scenarios/cov` : tous `VALIDE` ;
  `cargo test --release -p ddsom-game --lib` vert ;
- `examples/fuzz_familles` (types forcés, 6 scénarios × 8 000 images, 3,3 millions d'essais) : il essaie
  désormais aussi les gestionnaires écrits sur `Cx` (`families::natives()`) et ne compare plus les durées des
  routines sans durée exacte. Il relève de nombreux écarts, mais ceux examinés viennent tous de combinaisons
  (variante, état, sous-état) **hors des tables** de l'original, où le code d'origine saute dans des données :
  le tirage des variantes et sous-états du fuzz (0 à 0x14) ne connaît pas les bornes de chaque type. Le fuzz
  n'est donc pas concluant pour ces types : les chemins que les scénarios n'exercent pas (marqués « non
  exercé » ou délégués ci-dessus, et les branches rares des types exercés) **ne sont pas validés**. Pour le
  rendre utile, il faudrait lui donner, par type, les variantes et états valides.

Recensement des chemins non exercés (`examples/fuzz_effct.rs`, `FUZZ_PROPRE=1`) : variantes et sous-états
bornés par type (les tables de l'original), liens réglés par type (`prepare`), comparaison faite par l'outil
(RAM hors pile et hors table des tâches 68000 que le noyau tient en Rust, GFX RAM, palette, OBJ RAM, A0, D7) ;
balayage des 88 types natifs (variantes 0 à 3) puis campagnes ciblées : aucun écart hors des cas hors contrat
(variante hors table, texte du vote à une adresse impaire). Créateurs : `FUZZ_CREATEURS=...` depuis des registres
tirés au hasard.

Lots précédents :

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

## 6. Recensement des tables d'aiguillage (PSHL, PEFF, ESHL, SET, SOBJ, ITEM, BOX)

Sans 68000, un gestionnaire atteignable par sa table de types mais resté en code d'origine ferait planter le
jeu, même si aucun scénario ne l'exerce. Outil : `examples/recensement` (natives de toutes les entrées des
tables, puis objets synthétiques : un objet neuf de chaque type, appelé jusqu'à 120 images en perturbant ses
entrées, et le code 68000 que la native exécute encore est compté, plage par plage) :

```
recensement <roms> <eeprom> scenarios/bot30k.txt 3000 20 [PSHL,PEFF,ESHL,SET,SOBJ,ITEM,BOX]
  DDSOM_ENTREE=adresse   un seul gestionnaire
  DDSOM_VARIANTES=n      variantes 0..n-1 (0 seule par défaut : au-delà de sa table, un type lit n'importe quoi)
  DDSOM_ETATS=1          la moitié des trajectoires part d'états +4..+7 tirés (sort souvent des tables)
  DDSOM_SAUTER=a,b       gestionnaires non exercés (SET 4 et 6 : une animation de cases synthétique boucle)
```

`examples/reste68k` (inventaire des scénarios) comptait aussi le code d'origine que la machine exécutait à la
place d'une native qui se déclarait hors contrat (l'ancien `COUT_HORS_CONTRAT`, supprimé depuis : bulle EFFCT 4,
pas de réaction non réécrits) : avant, la porte SET 7 de `cov_mortal_late` (96 instructions, et la reprise de la
tâche en 68000 qui s'ensuivait) échappait à la mesure.

### Tables de types

Les tables s'arrêtent à la boucle de dispatch suivante (dans le code) ; PSHL 9 réutilise le gestionnaire du
type 1, PSHL 10 et 11 celui du 2, SET 33 celui du 3, SOBJ 4 et EFFCT 18 celui du SET 8 (0xcc25a).

| Classe | Table | Entrées | Gestionnaires distincts | Natifs | 68000 dans les natives (synthétiques) |
|---|---|---:|---:|---:|---|
| PSHL (6) | 0x39908 | 25 | 22 | 22 | aucun |
| PEFF (0x12) | 0x39992 | 48 | 48 | 48 (PEFF 42 réécrit ici) | aucun (créateurs 0x4aed0 et 0x53746 réécrits ici) |
| ESHL (8) | 0x39b96 | 73 | 73 | 73 | ESHL 37 (0xa2b62) appelle 0xb4bcc, créateur d'EFFCT |
| BOX (0xa) | 0x39e76 | 4 | 4 | 4 | aucun |
| SET (0x10) | 0x39eac | 34 | 33 | 33 (SET 7 complété ici) | aucun |
| ITEM (0xc) | 0x39f5a | 10 | 10 | 10 | aucun |
| SOBJ (0x14) | 0x39fa8 | 16 | 16 (dont 0xcc25a) | 16 | aucun |

Chaque entrée a donc sa native. Mesuré sur `bot30k` (variante 0) et `cov_4p_a` (variantes 0 à 2), 20
trajectoires par type ; `fuzz familles`, `fuzz_joueur`, `fuzz_familles`, `fuzz_set_a`, `fuzz_eshl_c` valident
les natives elles-mêmes (sections précédentes, `docs/familles-eshl-set-sobj.md`, `docs/familles/joueur.md`).

### Sous-tables (états `+4`, `+5`, `+6`/`+7`, variantes `+3`)

Les natives écrites sur `Cx` implémentent tous les états de leurs tables. Celles écrites sur `Ctx` (ITEM, BOX,
une partie d'EFFCT) ont un bras `_ => ctx.deleguer(cible)` : il n'est pris que pour un état absent de la table
de l'original (aucun objet du jeu ne l'atteint ; aucun passage en 68000 sur les trajectoires régulières ni dans
les 256 scénarios). Avec `DDSOM_ETATS=1`, ces délégations s'exécutent, mais sur des adresses impaires ou en
plein milieu de tables (`0x28604`, `0xdfa55`, `0xe0699`...) : c'est l'original qui saute dans des données, pas
un état légitime oublié.

### Réécrit dans ce travail

| Entrée | Rôle | Validation |
|---|---|---|
| SET 7 0xcb29c (états 2 et 4) + reprises 0xcb896, 0xcb9c0 | porte de sortie : changement de zone (prime, joueurs morts cachés), quatre genres de sortie (zone annexe, retour, scène suivante, fin de partie), fondu 0x1460 qui cède (la tâche reprend dans les natives aux adresses de retour), sauvegarde/suspension du monde, retour de zone (état 4) | `cov_mortal_late` (scène suivante), `cov/cov_porte_annexe` (nouveau : entrée puis retour), `fuzz_porte` (nouveau, arrêt à l'entrée du fondu puis reprise, 3000 essais dont 737 avec fondu), `fuzz familles` |
| 0x33b24 | caméras au retour d'une zone annexe (portes 7, 27, 29) | `fuzz petites`, `fuzz_porte`, `cov_porte_annexe` |
| PEFF 42 0x50b9c | article en vitrine de la boutique : zone de vente, message, vente (stock, bourse BCD, inventaire, refus) | `fuzz boutique` |
| 0x514ac / 0x514cc | interprète des dialogues du marchand appelé par le comptoir PEFF 43 (12 commandes) | `fuzz boutique` (scripts tirés) |
| 0x4aed0, 0x53746, 0xa2b4a, 0xa462c | créateurs (PEFF 5, pierres PSHL 0x11, ESHL 0x25, ESHL 0x34 variante 3) | `fuzz petites`, `fuzz_joueur` |
| 0x101da, 0x1ac90, 0xb230e | palette de la scène 0x69, fin d'éclair, deux paires de cases de tuiles | `fuzz petites` |
| reprises de 0x1c86e (51 adresses de retour d'étape) et 0x1c702 | fin de la mise à jour du monde et de la phase 12 quand un gestionnaire a cédé | `scene_67`, `cov_mortal_late`, `cov_porte_annexe` |

### Reste en code d'origine, appelé depuis ces familles

Hors de ce périmètre (créateurs d'objets des familles EFFCT et ENEMY, réécrits par ailleurs) :

| Routine | Appelée par |
|---|---|
| 0xb4bcc (crée un EFFCT) | ESHL 37 (`eshl_b.rs`, `IMPACT_PIECE`), SET 17 introduction de boss (`set_b.rs`, `SMOKE`) |
| 0xb97a2 (crée un EFFCT) | ITEM 3 équipement, variante 0xf (`items.rs`) |
| 0x667f4 (crée un ENEMY) | SET 17 introduction de boss (`set_b.rs`, `BLAST_AT`) |

Et dans EFFCT : `effects.rs` appelle encore la remise en liste 0x1ee1a par `jsr_origine` (code recompilé
exécuté directement, que l'inventaire ne voit pas ; la routine est pourtant native).
