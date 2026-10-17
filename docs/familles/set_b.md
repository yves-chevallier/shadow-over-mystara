# Classe SET : décors et animations, types 16 à 32

Code : `port/ddsom-game/src/families/set_b.rs`. Pool de 32 objets de 0x50 octets (A5+0x4a9e), table des
gestionnaires 0x39eac, boucle de dispatch 0x39e86, allocation 0x2348, libération 0x2210. Les objets SET
ne sont pas dans les listes triées ; beaucoup de types sont des *contrôleurs* de scène (portes de
sortie, générateurs de vagues, séquences de fin) plutôt que des décors. Les noms de rôle marqués (h)
sont des hypothèses. Pour tout type : état `+4` (0 init, 2 vie, 4 ou 6 fin), sous-état `+5`, variante `+3`,
paramètre du créateur `+0x2f`.

Joueurs : quatre enregistrements de 0x200 octets à A5+0x5de (`+0` actif, `+4` état long, `+8/+0xc/+0x10`
X/Z/Y, `+0x62` vie, `+0x6c` touches). Caméra : A5+0x506/0x50a (longs 16.16), secondaire A5+0x556/0x55a.
`A5+0x35` nombre de joueurs en jeu, `A5+0x2c` = 0xc partie en cours, `A5+0x3d` pause, `A5+0x50` figé.

| Type | Entrée | Rôle | Appels en scénarios |
|---|---|---|---|
| 16 | 0xd4110 | tremplin (h) : lance les joueurs qui le touchent avec la bonne touche | 0 |
| 17 | 0xd466a | séquence d'arrivée du boss (h) | 0 |
| 18 | 0xd4950 | descente de la caméra secondaire jusqu'à une cible | 0 |
| 19 | 0xd49a2 | générateur de vagues d'ennemis (h), script de la ROM | 0 |
| 20 | 0xd5f5a | barre de vie d'un joueur | 1 786 |
| 21 | 0xd6372 | compte à rebours de continuation (h) | 70 775 |
| 22 | 0xd69c2 | ajustement du rang en début de zone (h) | 24 |
| 23 | 0xd6c5c | porte de sortie de zone (h) | 0 |
| 24 | 0xd7056 | fenêtre de dialogue (cadre + liste de jetons de texte) | 2 984 |
| 25 | 0xd801e | volée de trois tirs quand la caméra recule (h) | 0 |
| 26 | 0xd810a | chasseur de joueurs immobiles (h) | 0 |
| 27 | 0xd8458 | porte de passage vers une zone cachée (h) | 0 |
| 28 | 0xd8ce4 | fin de zone (événement, invitation, 0xa462c...) (h) | 0 |
| 29 | 0xd8dce | porte de passage, variante qui suspend tous les pools (h) | 0 |
| 30 | 0xd9424 | fin de partie (musique, messages) | 0 |
| 31 | 0xd9492 | messages d'attente dans un niveau long (h) | 0 |
| 32 | 0xd96d0 | sélecteur de test (h) | 0 |

## 16 : tremplin (h)

Boîtes de contact et de présence lues dans la ROM (tables de pointeurs 0x11bffa, 0x11c11a, 0x11c23a, ligne
choisie par `+0x38`, `+0x39`, `+0x3a`, jeu `+0x2e` = 0x42). `A5+0x46` est un verrou (un seul tremplin actif à
la fois) relâché après `+0x14` = 0x20 images. Sorte `+0x42` (table 0xd4198 par variante) : 0 à 7, chacune
une touche (`+0x6c` du joueur) et un lancer (vitesses X/Y, hauteur, amortissement, état 0x2002400 ou
0x2002402) ; la sorte 3 pousse seulement le joueur de 0x28. Tant qu'un joueur vivant n'est dans aucune boîte
de présence, l'objet est inerte.

## 17 : séquence d'arrivée du boss (h)

Suite de 13 étapes (`+5`) : attend que la caméra passe 0x100, 0x180, 0x200, 0x2b0 ou qu'un compte à rebours
(`+0x42`) s'écoule ; lance des sons (0x2824, 0x2860, 0x289c, 0xeda26, 0xeda66), des explosions aléatoires
(tables 0xd4910 et 0xd4930, objets via 0x667f4) et crée le boss (BOSS type 2, 0x23a8). La première étape fige la
partie et affiche la liste de messages 0x14de.

## 18, 19

* **18** : descend la caméra secondaire d'un pixel par image jusqu'à `+0x42`, puis libère l'objet lié (`+0x46`).
* **19** : lit un script de la ROM (pointeur par variante en 0xd4c5e), entrées de 0x14 octets : délai ou attente
  d'un nombre d'objets vivants, rang minimal, position, nombre de joueurs minimal, routine de création (table
  0xd4b92 : ENEMY, BOSS ou SET), type et paramètres. Chaque objet créé garde son générateur dans `+0x46`
  (compteur de vivants `+0x2a`). Au démarrage et à la fin il appelle une routine propre à la variante
  (0xd98b8, 0xd995a) et règle la limite de défilement `A5+0x51e`.

## 20 : barre de vie

Trois colonnes de 44 cases dans la couche de fond 1 (0x91215c, 0x912160, 0x912164, 0x80 octets par ligne).
La barre est composée de cases pleines (0x1318000d) et d'une case partielle (pièces de 8 points : table de mots
0xd61b2) ; la vie affichée (`+0x2a`) rejoint la vie réelle (`+0x32`, somme des `+0x62`) d'un point par image.
Au-delà de 0x160 points, passage à la colonne suivante. L'habillage (cases d'extrémité) vient de listes
d'octets par type de joueur (pointeurs 0xd6232, 0xd628c pour le mode « deux joueurs réunis », variante `+3`).

## 21 : compte à rebours de continuation (h)

Trois chiffres (`+0x14`, `+0x15`, `+0x16` : packé dans `+0x43`) dessinés en 0x912b38, décomptés toutes les 60
images (`+0x17`). Le blocage de l'inscription clignotante « prêt » (rangées constantes en 0x912830) et de la
fin de partie (0x9127b0) est reproduit à partir des tuiles de l'original. À zéro, les joueurs vivants passent à
l'état 0x2040600 (condamnés), vie à 0. Les scripts de son 0xd6922 et 0xd694e (routine 0x1324) annoncent/retirent
l'affichage. Variante 1 : sans compte, seulement l'invitation (SOBJ 11 par 0xdfa4a).

## 22 : ajustement du rang (h)

À son premier passage : d'après le plus faible niveau de vie des joueurs présents (catégories <= 0x10, <= 0x30,
<= 0x50, au-delà) et leur nombre, lit une variation dans une table de la ROM (pointeurs 0xd6aa0) et l'applique
au rang `A5+0x7f92`, borné par `A5+0x7f94` et `A5+0x7f96` et masqué à 5 bits.

## 23, 27, 29 : portes de sortie / de passage (h)

* **23** : les joueurs qui restent devant la porte, touche 0 enfoncée, au sol, plus de 8 images, « entrent »
  (état 0x2002006, effet EFFCT 0x2f créé par le premier) ; quand tous les joueurs vivants sont entrés, la porte passe
  à l'état 4 : niveau suivant `A5+0x34` (+1 ; bonus 0x1c...), fin de partie au dernier niveau.
* **27, 29** : portes qui mènent à une zone cachée et en reviennent : elles sauvegardent la caméra et le
  défilement dans l'objet (`+0x30..+0x4a`), suspendent (compteur `+0x40`) les objets des pools (SET, EFFCT, SOBJ
  pour 27 ; tous les pools pour 29), chargent la nouvelle zone (appels aux routines 0x33b24/0x33aa6, 0xf2e56,
  0x1a360, 0x1a678, 0x1a8ac, 0x1686...) et replacent les joueurs ; au retour (`+0x48` = 1) les objets suspendus
  se réveillent et ceux dont le compteur devient négatif sont supprimés. Paramètres par variante : lignes de 16
  octets (0xd84e8 pour 27, 0xd8e5e pour 29). L'appel récursif à `bsr 0xd8458` (ou 0xd8dce) pour les objets de la
  même famille est reproduit.

## 24 : fenêtre de dialogue

Cadre dessiné dans 0x912000 (tuiles à attribut 0x0f, 0x80 octets par ligne) et liste de jetons mots pour le
moteur de texte (tampon 0xff2072, case `A5+0x5c6+(+0x38)`). La variante `+3` désigne une définition de 18 octets
(table 0x1d78d0 : colonne, ligne, indicateurs, texte, position, sorte `+0x2f`...). La sorte (24 valeurs) règle
l'initialisation (rechargement d'une autre définition selon le nombre de joueurs, palettes, ...) puis la
construction du texte (substitution des jetons 0x7000 à 0x7009 : nombre de joueurs, noms, objets équipés...).
Note de lecture du 68000 : la table de sauts des constructeurs (0xd7464/0xd7466) est décalée d'une entrée :
la sorte `k` saute à la `k`-ième entrée de la table (0xd7466).

## 25, 26 : surveillance des joueurs (h)

* **25** : quand la caméra recule dans la moitié gauche du niveau, lance trois tirs (ESHL type 0x12) à la profondeur
  d'un joueur tiré au hasard.
* **26** : pour chaque joueur vivant dont la position ne change pas pendant `+0x1c` images (octet d'une table de la
  ROM selon le rang), crée des poursuivants (routine 0xa44bc) dans la limite d'une table par nombre de joueurs.

## 28 à 32

* **28** : à X = 0x1c0 bloque le défilement, crée un événement SET 0x3f et attend sa fin, lance l'invitation (SOBJ 11)
  puis les routines 0xa462c, 0xe55a2 et 0xe13b0.
* **30** : si `A5+0x42` (fin de partie) : musique (0xa8ae8), attente de 0x80 images, liste de messages 0x1ccf8.
* **31** : chronomètre de niveau : messages après 0x258, 0x4b0 et 0x708 images de jeu.
* **32** : sélecteur 0 à 0x32 avec les touches du joueur 1, affiché en hexadécimal en 0x912bc0.

## Tables de la ROM (jamais recopiées)

0xd4198, 0xd41a6 (tremplins) ; 0x11bffa, 0x11c11a, 0x11c23a (boîtes) ; 0xd4c5e, 0xd4a0e, 0xd4bdc, 0xd4b92
(vagues) ; 0xd61b2, 0xd6232 (barre de vie) ; 0xd6464 (continuation) ; 0xd6aa0 (rang) ; 0xd84e8, 0xd8e5e,
0xd8824, 0xd896e, 0xd89be, 0xd9052 (portes) ; 0x1d78d0, 0xd7218, 0xd7a48, 0xd7e8a, 0xd7604, 0xd7db6, 0xd7d96,
0xd7da6, 0xd7988 (dialogue) ; 0xd80fe, 0xd8186, 0xd8448 (tirs et poursuivants).

## Couverture

* Scénarios (8 VALIDE sur 30 000 images) : types 21 (60 410 appels), 22 (20) et 24 (2 320). Le type 20 n'est pas
  appelé par ces scénarios (0 appel avec le code d'origine) ; il n'est validé que par le fuzz (6 728 appels,
  0 écart, joueur réel en `+0x20`).
* `fuzz familles` : 0 écart pour 16 à 23, 25 à 32 (variantes jusqu'à 24) et pour le type 24 sur ses 256
  variantes (17 097 appels).
* Peu ou pas atteint (relevé avant `perturber_globales`) : type 16 lancers hors présence de joueurs au sol ;
  type 17 étapes 2 à 24 ; type 19 au-delà du démarrage (création des ennemis, fin) ; types 23, 27, 29 :
  passage des joueurs et changement de zone (états 4 et 0x400 ; appels à 0x33b24, 0xf2e56, 0x1a360...). Ces
  chemins ont été relus sur le listing mais ne sont pas validés.
* Les gestionnaires ne restituent que A0 et D7 ; D0-D2 et A1-A4 ne sont pas reconstitués.
