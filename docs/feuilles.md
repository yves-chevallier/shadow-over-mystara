# Feuilles réécrites : RNG et événements (U1), pièces (C1), EEPROM (M2)

Routines natives de `port/ddsom-game` correspondant aux sous-systèmes « feuilles » de l'atlas. Le son
(S1) a son propre document : `docs/son-68000.md`. Pour chaque routine : adresse d'entrée, module,
contrat de registres (ce que les appelants lisent) et mode de validation.

## Outils communs

* `chemin.rs` : durée exacte d'un chemin du code d'origine. La routine native enregistre dans un
  `Chemin` les décisions des branchements conditionnels qu'elle prend (dans l'ordre où l'original les
  rencontre), puis `duree` parcourt le code d'origine en additionnant les durées de l'interpréteur
  (sous-routines, boucles `dbf`, tronçons). `duree_hors` laisse à `call` la durée des sous-routines
  non réécrites (leur durée s'ajoute à l'exécution) ; les durées sont mises en cache.
* `ops.rs` : CMP/ADD/ABCD avec les drapeaux exacts du 68000.
* `examples/fuzz.rs <groupe>` : validation différentielle sur états aléatoires (groupes `son`, `u1`,
  `c1`, `m2`). Chaque routine du groupe est appelée en mode contrôle ; l'original s'exécute à côté sur une
  copie ; mémoire, registres du contrat et durée (si elle est déclarée exacte) sont comparés. C'est ce
  qui valide les chemins que les scénarios n'atteignent pas.
* `examples/couverture.rs` : nombre d'appels de chaque routine native (ou, avec `decomp/index.tsv`, de
  chaque fonction d'origine) sur un scénario.
* `mem::diff_bus` ignore, en plus de la pile, la zone morte sous le pointeur de la pile privée des
  wrappers son (copies de registres sans signification).

## U1 : générateur pseudo-aléatoire (`rng.rs`)

État : mot en `A5+0x1a` (octet haut en 0x1a), graine 0x01c3 (`0x19a80`). Tirage (`0x19a88`) :

```
t = ((3 × état) mod 65536) >> 8 ;  octet bas += t ;  octet haut = t ;  résultat = nouvel état
```

Les 537 appelants masquent le résultat (`andi`) ; aucun ne lit les drapeaux, mais X (retenue de
l'addition d'octets) et N/Z sont reproduits. Contrat : D0 (mot bas), CCR. Durée exacte.

## U1 : files d'événements (`evenements.rs`)

Trois anneaux (case vide = 0xFFFF) : points de score (index `A5+0x108/0x109`, cases `A5+0x10a`), second
compteur (`A5+0x18a/0x18b`, `A5+0x18c`), pop-ups (`A5+0x20c/0x20d`, entrées de 8 octets en `A5+0x20e`).
Un événement de score est `(joueur << 8) | type` ; il est enfilé par `0x19ad6` (depuis un objet :
type = octet `+0x7e`, joueur = bits 8-9 du mot `+0x48`), `0x19ae6` / `0x19b08` (mot dans D0).
`0x19b2a` / `0x19b9c` (appelés à chaque image par la tâche d'interface) vident les anneaux : la valeur BCD
du type (table de ROM : `0x19c0e` pour les points, `0x19d52` pour le second compteur) est ajoutée en
décimal codé binaire au compteur du joueur (`+0x110` : 8 chiffres, plafond 99 999 999 ; `+0x114` : 5
chiffres, plafond 99 999). `0x19dce` crée un pop-up. Remises à vide : `0x19aa2`, `0x19abc`, `0x19db4`,
`0x19e10`. Contrat : tous les registres et drapeaux de l'original sont reproduits (centaines
d'appelants). Durées exactes.

`0x19e20` (`popups.rs`) : machine à états des pop-ups (durée, effacement, passage au suivant, durée 0x60 par
défaut) ; le dessin (`0x19ee0`, `0x19f24`, `0x19fd2`, `0x1a20e`) reste en code d'origine, appelé par `call`.

## C1 : pièces et crédits (`credits.rs`)

Port `IN2` (0x804020) : lignes des monnayeurs (bits 12-15 du mot) ; 0x804021 : boutons de service.
Enregistrement de monnayeur (16 octets, `A5+0x44e + 0x10 k`) : `+0` antirebond (plafonné à 50), `+1`
pièces non converties, `+2` crédits (≤ 9), `+3` total de pièces, `+7` « départ pris en compte ». Réglages :
`A5+0x1e` pièces par crédit, `A5+0x1f` crédits par seuil, `A5+0x20` partage des pièces, `A5+0x21`
nombre de monnayeurs moins un, `A5+0x22` compteurs séparés, `A5+0x25` jeu gratuit.

* `0x143da` (VBLANK) : lit les monnayeurs ; une ligne qui remonte après ≥ 2 images basses (sans avoir
  saturé) compte une pièce ; le seuil donne des crédits ; bruit de pièce (`0xe18f8`) et comptabilité
  EEPROM (`0x2e448`, non réécrite) en appel. Contrat : D0, A0 intacts.
* `0x141e0` : boutons de départ des monnayeurs (`0x1422e`), crédit gratuit au départ.
* `0x14290` : verrouillage des compteurs de pièces (`0x14300`, `0x14322`) : appelle `0xb96`/`0xbac`.

Durée : exacte pour la part propre à la routine ; les sous-routines appelées (`call`) ajoutent la leur.

* `0x14e46` : écrit un chiffre de l'affichage des crédits (tuile `D0 & 15`, attribut D1 en (A4)) ; D0, A4 et CCR reproduits.

Non réécrits : `0x13c2c` (démarrage/continue, attentes de tâche), `0x140b4`, `0x1436e`/`0x143a6`
(attentes), `0x14652`/`0x148be` (affichage des crédits).

## M2 : EEPROM et comptabilité (`eeprom.rs`)

EEPROM 93C46 (64 mots de 16 bits) pilotée par le port 0x804040 : bit 12 = DI, bit 13 = CLK, bit 14 = CS ;
DO se lit dans le bit 0 de 0x804020. Les autres bits du port viennent de `A5+0x16` (mot) et `A5+0x18`
(octet). Commande de 10 bits (un 0, le bit de départ, 2 bits d'opération, 6 bits d'adresse) : lecture
`0x180 | adresse`, effacement `0x1c0`, écriture `0x140`, autorisation d'écriture `0x130`. Chaque bit est
posé sur DI puis validé par une impulsion d'horloge (deux écritures du port).

* `0x2d846` lecture de `A5-0x616c + 1` mots depuis l'adresse `décalage / 2` ; `0x2d90e` écriture
  (autorisation, puis pour chaque mot : effacement, attente de fin, écriture, attente) ; `0x2d8aa` écriture de
  la copie `A5-0x6272` + relecture dans `A5-0x61f2` + comparaison, 10 essais ; `0x2d808` lecture de la copie
  avec vérification que les deux exemplaires de la configuration (mots 0-23 et 24-47) concordent.
  Contrat : D0 (0 ou -1) et CCR pour les trois dernières ; les paramètres de transfert sont en RAM
  (`A5-0x6172` tampon, `-0x616e` mots, `-0x616a` décalage...).
* Compteurs de comptabilité `0x2daca` (pièces), `0x2daf4` (départs), `0x2db1e` (continues),
  `0x2db4e` (parties) : le compteur long en RAM est incrémenté ; s'il diffère de son image dans la copie,
  l'image est mise à jour et les deux mots de l'EEPROM réécrits (`0x2d8aa`).
* `0x2e448` / `0x2e49a` : compteur d'EEPROM puis, en mode carte de développement (`A5+0x82`), la suite d'origine par `call`.
* Configuration (`config.rs`) : `0x2d648` (bloc de valeurs par défaut de la variante : D0, A6), `0x2d7aa` (valeurs par défaut en deux exemplaires), `0x2d662` (validation et réparation des réglages, bornes par octet), `0x2d358` (copie en RAM -> variables de monnayage `A5+0x1c..0x2b`, `0x73`, `0x84`, `0x85`, compteurs). L'initialisation `0x2d30a` reste en code d'origine.
* `0x2dcf6` (VBLANK) : lève le bit 0 du mot `0x664000` (RAM supplémentaire) si la comptabilité est active.

L'EEPROM émulée (`cps2/src/eeprom.rs`) termine les écritures instantanément : l'attente de fin
d'écriture dure une impulsion, ce que la durée supposée reflète.
