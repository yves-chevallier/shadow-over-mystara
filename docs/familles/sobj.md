# Classe SOBJ : objets spéciaux (décors défilants, scènes, interface)

Code : `port/ddsom-game/src/families/sobj.rs`. Pool de 16 objets de 0x50 octets (A5+0x795e), table des
gestionnaires 0x39fa8, boucle de dispatch 0x39f82, allocation 0x2408, libération 0x2250. Contrairement
aux SET, les objets SOBJ sont dans les listes triées : `+0x42` non nul choisit la liste du premier plan
(0x1ef6c), sinon celle des acteurs (0x1ee1a) pour les types qui en font un choix (type 0). Le type 4
(0xcc25a) est le générateur commun de `set.rs`. Les noms de rôle marqués (h) sont des hypothèses.

Conventions : états `+4` (0 init, 2 vie, 4 mort, 6 libération), sous-états `+5`, tables lues dans la ROM
(`rom8/rom16` pour les opérandes `(tbl,PC,Dn)`, `data*` pour les lectures par registre d'adresse).
Variables A5 : caméra principale `+0x506/0x50a` (longs 16.16), précédente `+0x524/0x528`, caméra
secondaire `+0x556/0x55a`, précédente `+0x574`, `A5+0x36` (la caméra de référence est la secondaire).

| Type | Entrée | Rôle | Appels (scénarios) |
|---|---|---|---|
| 0 | 0xdd61e | élément de décor lié aux caméras (parallaxe, nuages, avant-plan) | 330 349 |
| 1 | 0xddc28 | décor animé fixe à trois comportements | 0 |
| 2 | 0xdddb2 | décor clignotant (16 clignotements par fin de script) | 0 |
| 3 | 0xdde18 | étiquette collée à un objet | 0 |
| 5 | 0xdde8c | objet fantôme, se libère aussitôt | 0 |
| 6 | 0xddec4 | éléments de scènes scriptées (24 variantes) | 18 090 |
| 7 | 0xdf4d4 | bulle de conseil (h) | 0 |
| 8 | 0xdf6b0 | repère attaché à un objet | 0 |
| 9 | 0xdf774 | cadre clignotant | 0 |
| 10 | 0xdf8ae | flèche de conseil (h) | 0 |
| 11 | 0xdfa68 | marque d'interface (bulle et doigt) | 13 186 |
| 12 | 0xdfca0 | pastille d'interface collée à la caméra | 0 |
| 13 | 0xdfd44 | panneau défilant qui libère les joueurs en fin d'animation | 0 |
| 14 | 0xdfe3c | titre de niveau avec fondu de palette | 1 908 |
| 15 | 0xe0258 | cible à frapper (h) | 0 |

## Type 0 : décor lié aux caméras

Variante `+3` -> deux tables d'octets de 256 entrées dans la ROM : 0xdd6a4 (routine d'initialisation parmi
13) et 0xdd8ea (routine par image parmi 23). Les routines de suivi déplacent le décor d'une fraction du
défilement de la caméra (moitié, quart, 7/8...) ou le recalent sur la caméra secondaire ; certaines
attendent la fin du script d'animation (`+0x32` = 0xfe), d'autres sortent par le bas (`MOVE_XY` jusqu'à
Y > 0x240). Initialisation : place l'objet (X, Y, Z = -0x100), choisit sa liste (`+0x42`), ou règle la table
de vitesses 0xd. Un objet qui sort de plus de `2 * 0x60 + 0x180` pixels passe à l'état 4.
Jeu d'animations 0x1c03a6, numéro = variante.

## Type 1, 2, 3, 5

* **1** : décor (jeu 0x1c40c6) ; comportements : sortie d'écran, défilement à demi vitesse, suivi de la
  caméra secondaire (tables d'octets 0xddc6e et 0xddd16 par variante).
* **2** : décor dont l'animation se répète 16 fois (`+0x14`), caché (bit 1 de `+1`) à chaque fin de script.
  La marge de sortie d'écran est le registre D2 laissé par l'appelant (dépendance à un registre).
* **3** : étiquette (animation 0x43) à un décalage constant (X + 0x50, Y - 0x58) de l'objet pointé par
  `+0x14` (adresse longue), jusqu'à ce que cet objet passe à l'état 6.
* **5** : prend l'état 2 puis se libère.

## Type 6 : scènes scriptées (24 variantes)

Chaque variante est une petite machine à états (0, 2, 4 -> mort, 6 -> libération). Le « maître »
(`+0x4c`) est un autre objet dont l'octet `+8` sert de boîte aux lettres : 0 rien, 1 top, 2 étape suivante,
négatif fin. Résumé : 0 sprite lié au maître (jeu/anim lus dans 0xddf6c) ; 1 lumière grandissante ; 2 fond
défilant ; 3 lanceur d'objets SET depuis une liste d'octets (0xde1e0) ; 4 montée d'un sprite ; 5 meneur
créant des variantes 0 (décalages 0xde386) ; 6 lettre qui tombe (table 0xde3ee) ; 7 fondu de palette
0x91c3e0 avec script de transition ; 8 barre qui descend ; 9 texte animé ; 10 plaque en cinq temps ;
11 sprite fixe ; 12 sprite qui traverse l'écran ; 13 cadre (scripts directs 0xdea84) ; 14 insigne
(table 0xdef1e) ; 15 fanion coloré (palettes 0x28f960) ; 16 sprite à flip aléatoire ; 17 bulle ; 18 sprite
sur la caméra secondaire ; 19 pastille de palette ; 20/22 débris qui tombent ; 21 variante de 15 ;
23 étincelle collée à la caméra. Certaines variantes appellent des sons (0xea662, 0xea6a2, 0xea2e8,
0xea77e, 0xea87e, 0xea7fe, 0xea7be, 0xea83e) ou la routine 0x168c.

## Types 7 à 15

* **7 bulle de conseil** : liée à un objet (`+0x46`), anime 0x118 à 0x136 (`+0x14`), 0x50 images chacune ;
  palette 0x28ff20 -> 0x91c380. Inhibée si `0xff0ec6` est non nul.
* **8 repère** : suit l'objet `+0x14` avec un décalage de 0x20 (variante 0) ou 0x10 ; libéré à gauche de l'écran.
* **9 cadre clignotant** : 8 clignotements de 20 images ; position par table 0xdf87a (relative à la caméra)
  ou limitée par un joueur (`+0x2f`).
* **10 flèche de conseil** : naît à la fin d'une bulle ; quatre événements 0x19ae6 (codes de la table 0xdfa0e)
  et animation de la table 0xdf9ae selon le conseil.
* **11 marque d'interface** : variante 0 (bulle d'entrée par le bas, crée la variante 1), variante 1 (doigt
  qui suit le compteur `+0x33` du propriétaire).
* **12 pastille** : collée à la structure de caméra (A5+0x4fe/0x54e), animation 0x1ab + variante.
* **13 panneau** : naît à droite de la caméra, suit son défilement, à la fin de l'animation 0x3b met les
  quatre joueurs (mots en 0xff2aa4) à l'état 4 puis se libère.
* **14 titre de niveau** : animation de la table 0xdff0c (niveau `A5+0x34`), palette 0x91c1e0 éclaircie par
  16 pas de 4 images (palettes 0x291f00), durée 0xb4 ; niveaux 7 et 0x2b particuliers.
* **15 cible** : rectangle (`+0x16..+0x1c`, table 0xe02c6), trois coups d'un joueur (`+0x58` positif) la
  détruisent et lèvent un drapeau (0xff2ac0 ou 0xff2ac1).

## Validation

Voir la section « Couverture » en bas : fuzz `familles` (essais sur objets synthétiques), scénarios
(types 0, 6, 11, 14), `tools/validate_all.sh`.

## Couverture

* Scénarios (`tools/validate_all.sh 30000`, 8 scénarios VALIDE) : types 0 (273 311 appels), 6 (23 140), 11 (12 492)
  et 14 (1 917). Tous les autres types sont validés par `fuzz familles` seulement (0 écart sur plusieurs
  milliers d'appels par type, variantes 0 à 24 ou plus).
* Le fuzz crée un objet générique, perturbe les entrées des gestionnaires et, pour SET et SOBJ, les variables
  globales (mode de jeu, nombre de joueurs, caméras, rang, joueurs : `perturber_globales` dans `examples/fuzz.rs`).
* Instructions jamais atteintes (`tools/familles_non_vus.py`, avant l'ajout de `perturber_globales`) : type 6,
  348 instructions sur 941 (sous-états tardifs de plusieurs variantes, échecs d'allocation) ; type 7 : fin de
  l'aide (0xdf5a2-0xdf63c) ; type 13 : libération des quatre joueurs (0xdfdd0-0xdfe3a) ; type 14 : niveaux 7 et
  0x2b (0xdff98, 0xe0078-0xe0120) ; type 15 : chemin des coups (0xe030a-0xe03fc). Une partie des « non vus »
  du type 0 correspond à des tables de la ROM décodées en instructions.
* Dépendance à un registre laissé par le gestionnaire précédent : type 2 (marge D2 lue sans l'avoir posée). Les
  registres de sortie ne sont pas restitués (contrat A0 et D7 seulement).
