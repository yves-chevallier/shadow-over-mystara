# Classe SET : objets de décor et d'animation (types 0, 1, 4 à 7, 9 à 14)

Code : `port/ddsom-game/src/families/set.rs` (un `mod` par type, l'entrée de la table `NATIVES` en bas du
fichier). Pool de 32 objets de 0x50 octets (A5+0x4a9e), table des gestionnaires 0x39eac, boucle de dispatch
0x39e86, allocation 0x2348, libération 0x2210 (`docs/objets.md`). Les objets SET ne sont pas dans les listes
triées : leurs gestionnaires ne finissent pas par `reinsert`. Le type 8 (0xcc25a) était déjà réécrit ; les types
2, 3, 15 et suivants ne sont pas dans ce fichier (le 33 est un alias du 3 ; le 3 n'est pas réécrit, voir §
« Non fait »). Les noms de rôle marqués (h) sont des hypothèses tirées du comportement.

Les gestionnaires sont validés par `fuzz familles` (mode `DDSOM_SCENE`, voir §« Validation ») et par les huit
scénarios (`tools/validate_all.sh 30000`). Dans les tables ci-dessous, « lecture ROM » distingue l'espace
*programme* (`rom16` : adressage `(table,PC,Dn)`) de l'espace de *données* (`data16` : adressage par registre
ou absolu).

## Type 0 (0xba310) : animation de palette par script (h)

Le plus appelé (13 713 appels dans les scénarios). La variante `+3` choisit une entrée de 8 octets de la table
0xba57a (données) : adresse du script (`+0x30`), palettes sources (`+0x34`). `+0x2f` choisit la page de la RAM
palette (0 : 0x91c800, 1 : 0x91cc00, 2 : 0x91c000, autre : 0x91c400). Le script : en-tête (numéro de ligne de
départ, durée), puis blocs durée / nombre de lignes / indices de lignes ; chaque ligne de 32 octets est copiée en
conservant le quartet de luminosité (`& 0xf000f000`) de la destination. Durée 0xffff : relance ; négatif : saut ;
0 : état 4.
États : 0 init + premier bloc, 2 un bloc par échéance de `+0x3c` (pendant une transition de scène, `A5+0x42`,
une table par variante 0xba44c décide : continuer, attendre, finir), 4 fin (table 0xba4ca par variante : libération
ou conversion en variante 0x48, 0x4c, 0x4d), 6 libération. Variante 0x4e : ignore la pause (`A5+0x3d`) et `+0x40`.
Couverture : fuzz (variantes 0 à 0x4f) sans écart ; tout le code réel vu.

## Type 1 (0xbc518) : meneur de vagues d'ennemis (h)

Actif seulement en partie (`A5+0x35`). Attend 180 images (`+0x22` de 0xf0 à 0x3c), verrouille la caméra (0x2860),
puis lance par `spawn_wave` des vagues de trois ennemis ENEMY (un meneur de type 0x14 et deux ailiers de type
0x13, adresses en `+0x2a..+0x2f`) tant que le chef (propriétaire `+0x46`) en réclame ; sous-états `+5` décrits en
tête du `mod wave_leader`. Délai entre vagues tiré dans la table 0xbc63e (programme). Les trois adresses sont
« triées » par l'original au moyen d'une boucle sans effet. Couverture : scénarios (8 462 appels) et fuzz ; non
vus : la branche `+5 = 4` quand le compteur du chef vaut 1 (0xbc65a) et la boucle de tri.

## Type 4 (0xc935a) et type 6 (0xcafec) : animation de carte de tuiles (h)

La variante choisit une entrée de 16 octets (tables 0xc93a2 et 0xcb024, programme) : script, carte de tuiles en
mémoire vidéo, drapeau de boucle. Script : durée, colonnes, lignes, un long par case ; chaque case devient
`(attribut & 0xff80) + long`. Le type 4 s'arrête en pause et quand `+0x40` ; le type 6 laisse intactes les cases
dont le premier mot vaut 0x1800. Couverture : fuzz sans écart ; scénarios : 4 (48 appels). Non vu : le type 6
avec une case 0x1800 (0xcb142).

## Type 5 (0xc9780) : saisie du nom au tableau des scores (h)

Variante 0 : le tableau (un objet) ; variante non nulle : un curseur par joueur. Le tableau garde les adresses des
quatre curseurs, repère les cases où au moins deux curseurs se superposent et les dessine (indicateur tournant
par joueur). Le curseur déplace sa case sur une grille de caractères (11 colonnes × 6 lignes, ou grille large si
`A5+0x70`), écrit les lettres dans le tampon du joueur (`+0x118..+0x11f`, 0xff = vide), gère l'effacement, les
diacritiques (0x9d et 0x9f, tables 0xcabdc et 0xcac64), le nom par défaut (table 0xc9daa), deux familles de codes
secrets (tables 0xcade6 et 0xcad8a, reconnus par un balayage du tampon) et un chiffre de contrôle BCD
(`abcd` du 68000, table 0xcacec). Écrit le résultat dans l'écran (0x912000) et dans le joueur (`+0x160`, `+0x16b`,
tableau d'indicateurs 0xff2478). Le drapeau N laissé par la recherche de codes spéciaux (0xca524) commande, comme
dans l'original, l'exécution de la recherche de codes de remise à zéro (0xca374).
Couverture : fuzz sans écart sur plus de 200 000 appels (tableau avec ses quatre curseurs, curseurs seuls) ; 820
instructions sur 893 vues ; non vus : la bascule de joueur en vedette quand le masque change sans nouvel arrivant
(0xc9956), la commande « valider » depuis la colonne spéciale (0xc9d00), le remplacement par un code spécial
(0xca592), la conversion par 0x9d (0xcaba2). Les scénarios exercent le type 5 (1 454 appels).

## Type 7 (0xcb29c) : porte de sortie de zone (h)

Voir le commentaire de `mod exit_gate`. États : 0 init (table 0xcb344, 16 octets par variante), 2 surveillance
des quatre joueurs (emprise 0x60 × 0x40, 24 images), 4 rechargement de scène, 6 libération. **Partiellement
réécrit** : le changement de zone (quand tous les joueurs vivants sont prêts) et l'état 4 appellent le fondu 0x1460
et le chargeur de scène, qui cèdent la main à l'ordonnanceur (0xa40). La native calcule à blanc (`defers`) si
l'appel emprunterait ce chemin et, le cas échéant, annonce un coût hors contrat : l'exécuteur lance alors le code
d'origine (0xcb844 et suivants, 0xcba48-0xcc18c). Le déclenchement d'un joueur (0xcb8b4), l'initialisation, le
décompte de joueurs prêts et la libération sont natifs. Couverture : scénarios (11 813 appels) ; fuzz : 217 / 684
instructions du code d'origine vues dont le déclenchement n'est pas atteint (compteur de séjour de 24 images
difficile à produire par perturbation aléatoire).

## Type 9 (0xcc2c8), types 11 à 14

* **9** (0xcc2c8) : objet inerte (le test de collision de l'original ne produit rien). Fuzz sans écart.
* **10** (0xcc3c2) : bulle de dialogue temporisée (h) : choisit un texte (tables 0xcc438, programme, puis données),
  crée un cadre SOBJ de type 0xc par 0xdfc78, publie le texte (`A5+0x5ce..0x5d4`), charge la palette 0x91c180,
  expire après 120 images ou quand `A5-0x5fbe` devient non nul. Non exercé par les scénarios.
* **11** (0xcdcaa) : fin de niveau de la partie à bonus (h) : quand `A5+0x556` dépasse 0x320 et que tous les joueurs
  sont à l'état 0x200, les remet à 0x80000, éteint leur arme, les numérote, pose `A5+0x2c` et passe à l'état 4.
  Scénarios (2 841 appels) et fuzz.
* **12** (0xcdd70) : fondu au blanc des palettes (h) : à chaque image les 128 + 3 × 32 couleurs de 0x91c100, 0x91c720,
  0x91c800, 0x91cc00 gagnent un cran par composante et passent à luminosité 0xf. Scénarios (264 appels).
* **13** (0xcdef6) : animation de palette (h) : recopie, toutes les 8 images, de la table de 17 palettes 0x291f00
  (+0x360 si `A5+0x70`) vers 0x91c000 + 32 × variante, de la dernière à la première, puis libération. Scénarios
  (7 280 appels).
* **14** (0xcdf86) : déclencheur de fin de scène (h) : quand la caméra `A5+0x506` atteint 0x1c80 et qu'un joueur
  est vivant, passe les joueurs vivants à l'octet `+0` = 2, lève `A5+0x42` et `A5+0x4a`, pose `A5+0x2c` et passe à
  l'état 4. La variante `+3` n'a pas d'effet. Fuzz seulement.

## Non fait

* **Type 3 (0xc8352, alias 33)** : panneau d'interface des joueurs, 4 Ko de code à 35 sous-variantes (`+0x2f`) et
  tables d'animation de tuiles ; non réécrit faute de temps (ni les scénarios ni le fuzz de ce lot ne l'exercent).
* Type 7 : voir ci-dessus (parties qui cèdent la main).

## Validation

```
export DDSOM_ROMS=/home/ycr/som/rom DDSOM_EEPROM=/home/ycr/som/testdata/eeprom_ddsom.bin
fuzz familles <roms> <eeprom> scenarios/bot30k.txt 3000 <tours>
```

Le fuzz a été enrichi de manière additive (`examples/fuzz.rs`) : `DDSOM_SCENE=1` perturbe les variables de scène (A5),
les entrées des joueurs, les joueurs et des champs libres de l'objet (`DDSOM_CHAMPS=déb-fin[,déb-fin]` en hexadécimal
limite les décalages touchés ; états de l'automate pairs et bornés) ; `DDSOM_PROCHE=1` pose un joueur sur l'objet à
chaque image ; `DDSOM_PROGRES=1` écrit chaque appel sur la sortie d'erreur ; le tableau de saisie des noms (SET 5,
variante 0) reçoit ses quatre curseurs, qui tournent aussi ; les appels dont le coût est annoncé hors contrat
(SET 7 qui cèderait la main) sont sautés. Un fuzz sans `DDSOM_CHAMPS` réaliste (par exemple perturbant `+0x4f`,
numéro de scène, d'une porte) fait diverger le chargeur de scène de l'original : écarts sans signification.
