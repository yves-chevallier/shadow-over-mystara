# SET 2 (contrôleur de scène) et SET 15 (écrans de fin de niveau)

Code : `port/ddsom-game/src/families/set_c.rs` (table `handlers!`), `set_c_scene.rs` (type 2),
`set_c_finale.rs`, `set_c_finale_b.rs`, `set_c_finale_c.rs` (type 15). Les tables restent dans la ROM
et sont lues à l'exécution ; les rôles marqués (h) sont des hypothèses.

Ces « 48 Ko » et « 25 Ko » sont en grande partie des **données** : le type 2 n'a que 2,6 Ko de code
(0xbc814-0xbd272), le reste est constitué des scripts d'événements (table de pointeurs 0xbd276, un
script par variante, 0x14 octets par entrée) ; le type 15 a environ 11 Ko de code (0xce024-0xd0b10).
Aucun des deux ne cède la main à l'ordonnanceur de tâches (pas de `trap`) ni n'attend d'événement
externe autrement que par l'état de la RAM (variables de partie, joueurs, objets enfants).

## SET 2 : contrôleur de scène (0xbc814)

Créé par le type 8 (0xcc25a) avec la variante `+0x2f`. Ne travaille que si `+0x40 == 0` et
`A5+0x35` (difficulté) est non nul. Sa variante `+3` choisit : un script d'événements
(`0xbd276 + 4 × variante`, mot de tête = durée de la scène), une action de départ (octet de la table
0xbc880, 12 actions) et une action de fin (octet de la table 0xbcba8, 16 actions). Beaucoup de
variantes (120 au plus) sont de simples marqueurs : intertitre, rideau, fondu, sortie des joueurs.

| `+4` | Rôle |
|---|---|
| 0 | `A5+0x51`++, curseur de script `+0x20`, durée `+0x24`, appel 0xd9778, puis action de départ : demande de transition `A5+0x51e`, `A5+0x58`, libération si pas de partie (`A5+0x66` nul pour les actions 10 et 11), effacement de la tilemap en 11 images (action 4, GFX 0x91203c / 0x912040), etc. |
| 2 | interpréteur du script d'événements (en niveau 4 ou en phase 0xc) |
| 4 | fin de scène : attend que les objets créés aient disparu (`+0x2a` nul), `A5+0x51`--, 0xd981a, puis action de fin |
| 6 | libération (0x2210) |

Entrée de script (0x14 octets, fin = mot 0x8000) : `+0` attente (bit 15 nul : autant d'images + 1 ;
bit 15 levé : attendre que le nombre d'objets vivants créés `+0x2a` retombe à l'octet bas, le bit 7 de cet
octet exigeant en plus un déblocage extérieur de `+0x2e`), `+2` rang maximal (`A5+0x7f92`), `+4/+6/+8`
X, Z, Y (mots), `+0xa` difficulté minimale, `+0xb` classe de l'objet (déplacement dans la table
d'allocateurs 0xbcb4e : ENEMY 0, BOSS 4, SET 8, ESHL 0xc, ITEM 0x10, EFFCT 0x14), `+0xc` type, `+0xd`
variante, `+0xe` `+0x2f`, `+0xf` `+0x2c`, `+0x10` mot `+0x42`, `+0x12` coordonnées absolues si non nul,
sinon relatives à la caméra (`A5+0x4fe` ou `A5+0x54e` si `A5+0x36`). Les entrées trop difficiles ou de
rang trop haut sont sautées. L'objet créé reçoit `+0x46` = le contrôleur et `+0x2a` est incrémenté (sauf
objets ramassables).

Actions de fin (table 0xbcba8) : 0 recopie de caméra, 1 transition de 0x3c, 2 et 3 intertitres (deux
SET, un SOBJ), 4 sortie des joueurs (état 0x80400, phase de partie 0x18), 5, 6 rideau (22 couleurs
copiées d'après la table 0xbcea4 puis tous les objets des huit pools sont poussés vers l'état 6),
7 quatre effets de type 0x21, 8, 9 un SET de type 0x11, 10 et 15 scène figée, 11 fin simple, 12 figée puis
0, 13 et 14 un boss (types 3 et 1) sauf si `A5+0x50` ou `A5+0x42`.

Couverture : 43 083 appels dans les 8 scénarios (VALIDE) ; fuzz `familles` sur 0xbc814 (variantes 0 à
119, états 0/2/4/6, variables de partie et joueurs tirés ; 128 000 appels) : 0 écart. Non vus par le
fuzz : actions de départ 5 (jamais choisie par la table), échecs d'allocation, parcours de pools remplis
d'objets de chaque classe (le balayage du rideau est identique d'un pool à l'autre).

## SET 15 : écrans de fin de niveau (0xce024)

Aiguilleur : la variante `+3` (0 à 24) choisit une machine à états (`+4`, sous-état `+5`) via la table
0xce034. Les entrées d'états 4 et 6 de toutes les variantes sont communes (4 : `+4 = 6`, 6 : libération ;
variantes 0 et 1 : libération dans les deux cas ; 24 : état 4 propre ; 19 et 20 sans table). Un
« chef d'orchestre » (le SET pointé par `+0x4c`, souvent une variante de type 2 ou une autre variante 15)
pilote les enchaînements par son octet `+8`. Les scénarios appellent le type 15 21 fois (VALIDE) ;
le reste des variantes n est validé que par le fuzz.

| Var. | Adresse | Rôle (h) |
|---|---|---|
| 0 | 0xce066 | clignotement du titre : bascule `+0x43` et appelle 0xea22c / 0xea1d0 de temps à autre |
| 1 | 0xce0f0 | secousse de fin de niveau (tables 0xd0b10, 0xd0b20) avec effets (0xb1a7a) liés à `+0x4c` |
| 2 | 0xce294 | fond qui glisse (GFX 0x920000, position `+0x34`) |
| 3 | 0xce324 | caméra vers un but donné par la table 0xce37a (selon `+0x3f` du chef), grilles de blocs 0x26d4 |
| 4 | 0xce47c | scène défilante n° `+0x2f` (table 0xce4ea, blocs 4x4 0x2766) |
| 5 | 0xce64a | chargeur de deux grilles de blocs (0xce686) |
| 6 | 0xce6bc | comme 4, jusqu'à ce que le chef passe à la variante 2 |
| 7 | 0xce714 | fenêtre de dialogue : palier du joueur `+0x34`, message (tables 0xceaa2), bonus de statistiques `+0x151..`, équipement `+0x10d/0x10e/0x140..` et recalcul 0x3552, cadre (0xce7e4), texte composé dans 0xff2072 |
| 8 | 0xcf3a8 | les PV des joueurs remontent (`+0x62` vers `+0x60`) selon le score `A5+0x38` |
| 9 | 0xcf476 | six effets (type 1) par joueur / personnage absent, puis écran de bonus (variante 0x17) |
| 10 | 0xcf5e6 | secousse d'ouverture puis défilement horizontal accéléré |
| 11 | 0xcf6e8 | comme 9 (état 0 partagé), enchaînement commandé par le chef |
| 12 | 0xcf782 | six marqueurs SOBJ + un dernier |
| 13 | 0xcf8a0 | feu d'artifice (SOBJ variante 0x10, positions 0xcf974) |
| 14 | 0xcf994 | éclairs (SOBJ variante 0x11) |
| 15 | 0xcfa4c | secousse légère (table 0xd0b70) |
| 16 | 0xcfa86 | un effet par joueur, dans un ordre aléatoire, à 0xb4 images d'intervalle |
| 17 | 0xcfb76 | fondu de trois lignes de palette |
| 18 | 0xcfc56 | choix des bonus de fin (table de tests 0xcff10 : listes d'objets, place, `A5+0x5d`) |
| 19 | 0xd004a | décompte BCD (`+0x4c`), événement 0x19ae6 par chiffre |
| 20 | 0xd00c2 | texte défilant (ROM 0xd01de) dans la tilemap 0x910000 |
| 21 | 0xd06e6 | scintillement d'une couleur de palette |
| 22 | 0xd077c | affichage du nom de chaque joueur (SOBJ 0x15, sous-routine 0xd0870) |
| 23 | 0xd0932 | comme 8 avec le bonus propre à chaque joueur (`+0x64`) |
| 24 | 0xd0a0c | tableau des résultats, clignotement de palette |

Points de fidélité (appris du fuzz) : les registres laissés par les sous-routines natives ne sont pas
garantis (seuls leurs `outputs` le sont) : le gestionnaire reconstitue lui-même D0 après 0xb230e ;
`D1` doit contenir le décalage de la secousse quand 0xb1a7a crée un effet (variante 1) ; les index
`(tbl,Dn.w)` sont des mots étendus en signe ; la variante 18 itère `btst` sur un masque de 32 bits.

Validation : `fuzz familles` sur 0xce024 (états 0/2/4/6, sous-états valides par variante, variables de
fin de niveau, joueurs, chef d'orchestre tirés) : environ 500 000 appels, 0 écart sauf un cas
connu et hors contrat (variante 18, `choose_reward` : quand plus de 24 candidats sont refusés l'original
saute dans des données, 0xd5ef8, via la table 0xcff10 ; la réécriture refuse alors sans écart de
principe). Non couvert : les chemins où l'allocation d'objet échoue, les parcours très longs des
variantes 18 et 20 (fin de texte 0xfe), les sous-routines 0xceb46 / 0xcf182 (hors du type : appelées
par d'autres objets), et, pour les instructions comptées par `familles_non_vus.py`, les boucles de
l'original plus longues que la limite de 20 000 instructions du traceur (0xca8, 0xcc8) qui cachent des
chemins pourtant exécutés et comparés.
