# Collisions, coups et dégâts (X1, X2) et aides d'IA (E1)

Code : `port/ddsom-game/src/combat.rs` (+ `combat/shots.rs`, `contact.rs`, `damage.rs`) et `ai.rs`.
Les tables de barème restent dans la ROM (lues à l'exécution) ; ce document en décrit seulement le
format et le rôle. Les noms marqués (h) sont des hypothèses tirées de l'usage.

## 1. Déroulement d'une image

`0x260ee` (`combat_update`) est appelée une fois par image par la boucle de partie (`0x1c720`, `0x1fa68`
et les étapes de scène `0xf3xxx`), après `sprites::build_sprite_list` : celle-ci a rempli les listes
d'objets visibles. Rien n'est fait si `A5+0x35` (joueurs en vie) est nul. Ordre :

1. si `A5+0x42` = 0 et non (`A5+0x44` et `A5+0x4e`) : parades (joueur contre ennemis puis contre tirs
   ennemis), coups des joueurs sur ennemis puis sur caisses, tirs des joueurs sur ennemis ; si
   `A5+0x3d` (pause) est nul : ennemis sur joueurs, tirs ennemis sur joueurs, caisses sur ennemis,
   caisses sur joueurs, poussées (ennemis puis caisses) ;
2. dans tous les cas : ramassage des objets par les joueurs (`0x283d4`).

## 2. Listes d'objets visibles

Cinq catégories, remplies par `0x1ed2` selon la classe `+0x28` de l'objet :

| n° | classe | contenu | compteur | tableau de mots |
|---|---|---|---|---|
| 0 | 6 | tirs des joueurs | `A5+0x7e72` | `A5+0x7e86` (18) |
| 1 | 8 | tirs des ennemis | `A5+0x7e74` | `A5+0x7eaa` (38) |
| 2 | 2, 4 | ennemis et boss | `A5+0x7e76` | `A5+0x7ef6` (25) |
| 3 | 10 | caisses | `A5+0x7e78` | `A5+0x7f28` (21) |
| 4 | 12 | objets ramassables | `A5+0x7e7a` | `A5+0x7f52` (32) |

L'original parcourt ces tableaux avec un compteur de boucle gardé en RAM (`FF1A62-FF1A70`) : la
valeur finale est visible et reproduite (`combat::scratch`).

## 3. Boîtes

Chaque objet porte un jeu de boîtes (`+0x2e`) et trois index : corps `+0x38`, poussée `+0x39`,
attaque `+0x3a` (0 = pas de boîte). Les tables de pointeurs (un long par jeu, puis 8 octets par boîte)
sont en ROM : corps `0x11bffa`, poussée `0x11c11a`, attaque `0x11c23a` ; tirs joueur `0x121fc8`, tirs ennemis
`0x1221e2`, ramassage `0x1233c8` (indexées directement par l'index, sans jeu). Boîte (6 octets utiles) :
X (signé), demi-largeur, Z (hauteur), demi-hauteur, Y (profondeur), demi-profondeur.

Test de recouvrement sur un axe (arithmétique 16 bits, comme l'original) :
`delta = (déc_A + pos_A) - (déc_B + pos_B)`, `h = demi_A + demi_B` (somme sur 8 bits) ;
recouvrement si `delta + h <= 2h` en non signé. Sur X, le décalage est inversé si l'objet regarde à
gauche (`+0x2c` non nul). Ordre des tests : Y, X, Z. Variantes : sommes de demi-tailles étendues en
signe (collisions avec les caisses), Z ignoré (ramassage), pénétration `h - |delta|` conservée (poussées).

## 4. Résolution des coups

Un coup d'un joueur sur un ennemi (`0x2630e`) : mémorise l'identifiant d'attaque (`+0x3e`) dans l'octet
`victime+0x58+n° du joueur` (un coup ne touche qu'une fois) ; gèle les deux camps (`+0x53/+0x56/+0x57`) ;
refuse si la victime pare (`0xb048`) ; calcule les dégâts (§5) ; retire les PV (plancher 0) ; note
l'agresseur dans la victime (`+0x48..+0x4b`, `+0x54`) ; met la victime en état `0x04000000`
(réaction `+0x49` = 0x4c, `+0x4a` = orientation) ; enfile l'affichage de la jauge (`0x19dce`), crée
l'étincelle (`0xaa96c`) et joue le son (`0xe662a`, ou `0xe823a` pour les coups spéciaux).

Tirs des joueurs (`0x266fe`) : la table `0x267c4` associe à chaque comportement `+2` du tir (0..24) une
routine qui décide si le tir porte. Immunités : le long `victime+0x94` a un bit par comportement
(bits 23, 25, 28, 29, 27, 2, 18, 3, 28, 24, 26, 13, 10, 1, 9, 0, 4...). Les comportements ajoutent au
tir un bonus de puissance (octets `+0x45/+0x46`) selon l'équipement du lanceur (`+0x10e` = 0x20 : +25 %,
0x21 : +50 %, `+0x166` = 4 : +25 %, `+0x165` : +4/+5 fixe). Après un tir qui porte : dégâts via
`0x422a`, +5 si `lanceur+0x166` = 4, achèvement des petits ennemis (`0x26856`), jauge.

Attaques ennemies : `0x2749e` (coup) et `0x2756c` (tir). Tables `0x1067a4` et `0x106912` : par
identifiant d'attaque, 4 octets (action du joueur au sol, en l'air, puissance, puissance si `A5+0x48` > 5).
Le joueur passe en état `0x02040000`, invulnérable 0x28 images (`+0x7e`). Les tirs sont annulés par
certains équipements (`+0x165`, `+0x166`, `+0x167`, table d'octets `0x27690` par type de tir).

Parades (`0x27d74`, `0x27e0a`) : un joueur qui attaque avec la boîte 4 face à une attaque ennemie
(orientations opposées) passe en état `0x02080000` (tir : selon la table `0x27fd6` et l'arme).
Caisses : lancées, elles blessent joueurs (`0x28246`) et ennemis (`0x28296`) avec le barème `0x28334`
(par type de caisse et compteur FFFF92). Poussées : `0x27b84` (ennemis), `0x27a4c` (caisses, tonneaux).
Ramassage : `0x2853a` (bouton) et `0x285b0` (en marchant, table `0x285e0`).

## 5. Formules de dégâts (X2)

* `roll_variation` (0x3bbe) : `d = base + T[min(base-1,4)*32 + (rng & 31)]`, `T` octets signés (0x3be4).
* Tir joueur (0x422a) : base = puissance A (ennemi ordinaire) ou B ; `d = roll(base)`, au moins 1.
* Coup joueur (0x3b36), par `ATTACK_KIND` : 0 arme (`+0x109`), 1 arme (`+0x10a`), 2 base 1, 3 rien.
  +50 % sous bonus (`+0x79`), `roll`, `+ (+0x100)`, plancher 1. Pour les types 0 et 1 : coup de pas 0xc/0x16
  (+50 %), sinon coup critique (+5, `+0x77`) tiré dans 0x3c84/0x3ca2 (le magicien, classe 4 avec l'arme
  0x30, fait `PVmax >> n` via 0x3e4e/0x3ef8). Puis élément (+4 si faiblesse `+0x9e`, moitié si
  résistance `+0x9f`), bonus de groupe (table `0x3d8e`, par arme et nombre de joueurs, avec plancher).
  Dans `player_blow_on_enemy` : +50 % (arme 0xb, `+0x82`), 0x200 (arme 0xc, `+0x83`), +25 % (`+0x184`,
  ou `+0x166` = 5 en attaque 2), +3 sans `+0x163`, +1 si `+0x166` = 2, plafond octet bas `+0x16e`.
* Attaque ennemie (0x4002) : `d = barème[réglage A5+0x70][FFFF92] + puissance`, `roll`, `- défense (+0x10b)`
  (min 1) ; réduction de `d * (n+1) / 32` selon la protection (`+0x108`, table 0x406d) ou, à 32 PV et
  moins, selon (classe, PV) (tables 0x40aa/0x416a). Puis `0x276e0` retire `d - 2 (si +0x165 = 0) -
  armure (table 0x27732)`. Les nombres sont dessinés par `show_damage` (0x3f28/0x3f32) dans la couche de
  caractères (deux chiffres), sauf bit 0x200 de FFFFD0.
* Butin du voleur (0x388a) : classe 5, coup de type 3 : tirage dans les tables 0x3936/0x3954/0x39d8.

## 6. Aides d'IA (E1)

Contrat exact (registres, drapeaux, durée), écrites en double mode (réel / à blanc) : voir `ai.rs`.

| adresse | rôle |
|---|---|
| 0x424e / 0x43ae | cible aléatoire : proche (10/16), PV bas (4/16), éloignée (2/16) ; A1 = joueur, `+0x50` |
| 0x44ee | joueur ciblable tiré au hasard (3 essais) |
| 0x453c / 0x4544 | PV initiaux (type, difficulté, nombre de joueurs, FFFF92) |
| 0xa4b4 / 0xa4c2 | décision par masque de 32 bits (`+0x72` / `+0x73`, FFFF92, tirage) |
| 0xac3a, 0xfe5a, 0x1059e | octet par table, immunités par type, indice d'étalement |
| 0xb5e0, 0x1050c, 0x10560 | cible dans la boîte devant l'ennemi, cible du côté regardé, sens de déplacement |
| 0x103d6 / 0x1040c | animation (0x1a32 / 0x1a56) puis `+0x44 = +0x3e` |

Joueur ciblable : actif (`+0`), non masqué (bit 1 de `+1`), `+4` < 4, `+0x182` nul (0x43ae : un
`+0x182` non nul dispense du test « masqué »).

Aides réécrites ensuite dans `ai_aids.rs` (simples natives, registres de sortie limités à ce que les
appelants relisent) :

| adresse | rôle |
|---|---|
| 0x43bc | cible selon le critère D0 imposé par l'appelant (0 proche, 2 PV bas, 4 éloignée), règle de 0x43ae |
| 0x788c, 0x79f6, 0x79ce | tirages d'attitude : `+0x6b` (ligne `+0x6a`), `+0x6e` (ligne `+0x6d`, ou 1 hors de l'écran), précédé pour 0x79ce d'un bit tiré dans le masque de la ligne `+0x6c` (indexé par FFFF92) |
| 0xb048 / 0xb068 | parade de la victime A1 (sait parer `+0x67`, au sol, face à l'agresseur ; bit du masque `+0x68` / `+0x69`) |
| 0xf118 | `+0x84/+0x85` : 1 hors de l'écran, sinon octet tiré dans la ligne `+0x8e` (bloc par paire d'images) |
| 0x10112 | `+0x68` = 8, ou selon `A5+0x5a` et le nombre de joueurs |
| 0x1172e, 0x11a36 | jeux de débris (EFFCT 0x32 variante 0xc) ; une ligne de table n'est consommée que si l'effet a pu être créé |
| 0xb648, 0xb780, 0xe2f6 | butin : objet unique (`+0x9d`), butin et trésor (ligne `+0x80`, un ou deux tirages selon le nombre de joueurs) |
| 0xe7fa | point de marche le plus proche autour de la cible, numéro mémorisé en `+0x99` |
| 0x3e480 | aiguillage vers la routine d'atterrissage par type (ennemis 0..31, boss ensuite ; table 0x3e49c), appelé par les réactions communes (O3) |

## 7. Validation

`tools/validate_all.sh 30000` : 8 scénarios VALIDE, 0 écart (59 222 appels de `combat_update`, durée des
natives d'IA exacte au cycle près, `exact_cost`). Les routines internes de `combat` sont comptées par
`examples/combat_coverage` (même protocole que `validate`, plus le détail des appels internes).
Exercées : tous les parcours de listes, coups des joueurs (dégâts, critiques, magicien, élément,
bonus de groupe, jauge), attaques et tirs ennemis, poussées d'ennemis et de caisses, ramassage,
parade contre un ennemi. **Jamais exercées par les scénarios** (non validées) : comportements de
tirs joueur (`shots.rs`, types 0..24), butin du voleur (`steal_loot`), parade d'un tir ennemi, caisses
qui blessent (`box_hits_*`), saisie de caisse / tonneau (`grab_crate`, `shove_*`, `push_barrel`), coup
paré (`0xb048`), et côté IA `0x103d6`, `0x1040c`, `0x1059e`, `0x1050c`, `0x10560`.

Particularité reproduite : dans `player_hits_enemies`, `0x19dce` écrase A2 (la boîte d'attaque du
joueur) et l'original ne la rétablit pas ; les ennemis suivants sont testés contre FF820E
(`World::a2`).
