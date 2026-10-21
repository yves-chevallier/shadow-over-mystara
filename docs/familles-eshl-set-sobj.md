# Familles d'objets ESHL, SET et SOBJ (gestionnaires de types)

Code : `port/ddsom-game/src/families/`. Trois des dix pools d'objets (voir `docs/objets.md`) dont
chaque *type* est une fonction d'état appelée par la boucle de dispatch de la classe :

| Classe | Pool | Table des gestionnaires | Boucle de dispatch | Types |
|---|---|---|---|---|
| ESHL (8) | 38 × 0x60 octets, A5+0x3c5e | 0x39b96 | 0x39b70 | 73 (64 à 72 : `eshl_d.rs`) |
| SET (0x10) | 32 × 0x50 octets, A5+0x4a9e | 0x39eac | 0x39e86 | 34 (le type 33 réutilise le 3) |
| SOBJ (0x14) | 16 × 0x50 octets, A5+0x795e | 0x39fa8 | 0x39f82 | 16 (le type 4 réutilise le 8 de SET) |

## 1. Contrat d'un gestionnaire

La boucle de dispatch fait, pour chaque emplacement actif : `moveq 0,D0 ; move.b (2,A0),D0b ;
lsl.w #2,D0w ; movea.l (table,PC,D0),A1 ; jsr (A1)`, puis passe à l'emplacement suivant avec A0 et
le compteur de boucle D7. Un gestionnaire reçoit donc A0 = l'objet, A5 = 0xFF8000, D7 = compteur de
boucle (il s'en sert pour faire clignoter), D0 = type × 4 (les octets hauts sont nuls, certains
gestionnaires s'y fient), A1 = son propre point d'entrée. Il rend la main par RTS en laissant A0 et D7
intacts : c'est le contrat déclaré (`outputs`) de toutes les natives de ces fichiers.

Chaque gestionnaire native remplace la routine du type à son adresse d'entrée. Les sous-fonctions de
l'original qui ne sont pas des états du type (animation, déplacements, liste triée, terrain, sons) sont
appelées par `Cx::call` : elles sont déjà natives (`objects/`, `video.rs`, `sound.rs`) ou exécutées par
le code d'origine ; leur durée s'ajoute d'elle-même.

## 2. Structure du code

* `cx.rs` : `Cx` (processeur, bus, objet courant `o`), lecture/écriture des champs de l'objet,
  appels `anim_start(jeu, n)`, `anim_step()` (vrai en fin de script), `reinsert()` (liste triée),
  `free_eshl()`, `call(adresse)`, tables de la ROM : `rom16/rom8` (espace *programme*, pour les
  adressages `(table,PC,Dn)`) et `data16/data32/data8` (espace de *données*, pour les adressages
  absolus ou par registre : les deux espaces diffèrent sur CPS2, le programme étant chiffré).
  Macro `handlers!` : table `NATIVES` d'un fichier, `adresse "nom" coût chemin`.
* `shell.rs` : champs et aides propres à ESHL (`f::OWNER`, `place_in_front`, `blink`...).
* `eshl.rs`, `eshl_b.rs`, `eshl_c.rs`, `eshl_d.rs` : types ESHL (`creators.rs` : petites routines de création appelées par le code d'origine) ; `set.rs`, `set_b.rs`, `set_c.rs` : types SET ;
  `sobj.rs` : types SOBJ. Un type = un `mod` avec une fonction `run`, des constantes nommées pour ses
  états et ses tables, et un commentaire décrivant son rôle. Les tables de données restent dans la ROM
  (lues par `rom16`/`data16`), seules leurs adresses sont des constantes.
* `lib.rs::all_natives()` enchaîne `families::natives()`.

La machine à états de l'original est à trois étages : `+4` (état principal), `+5` (sous-état) et
`+6`/`+7` (sous-sous-états), des octets pairs qui indexent des tables de sauts `(table,PC,Dn)`. Les
états 4 et 6 de l'état principal valent presque toujours « mort » : `+4 = 6` demande la libération à la
prochaine image (`free_eshl`, 0x2202) ; la libération directe est aussi courante.

### Durée

Chaque gestionnaire déclare une durée *estimée* (`exact_cost: false`) : somme des instructions d'un
chemin typique du type, sans les sous-routines (qui comptent leur propre durée). La mémoire et les
registres du contrat sont exacts ; en mode contrôle la durée de l'original est reprise à chaque appel,
si bien que la validation globale n'est pas affectée.

## 3. Validation

1. `tools/validate_all.sh 30000` (8 scénarios, deux machines, mode contrôle) et
   `cargo test --release -p ddsom-game --lib` (unicité des entrées).
2. Les scénarios n'exercent qu'une petite part des types (voir §5). Pour les autres,
   `examples/fuzz familles` :

   ```
   export DDSOM_ROMS=... DDSOM_EEPROM=...
   fuzz familles <roms> <eeprom> scenarios/bot30k.txt 3000 300
   ```

   Après 3000 images de chauffe, pour chaque gestionnaire, des trajectoires de 60 appels : un objet
   neuf (type, variante 0..2, orientation, propriétaire tiré parmi les ennemis vivants et les joueurs)
   est créé à l'état 0, puis le gestionnaire est appelé image après image tandis que l'on perturbe
   les entrées que d'autres sous-systèmes modifient (drapeaux de touche `+0x57`, états et drapeaux
   du propriétaire, pause `A5+0x3d`, compteurs de gel). Chaque appel est comparé à l'original.
   Variables : `DDSOM_ENTREE=<adresse>` (un seul type), `DDSOM_VARIANTES=n` (variantes tirées, 3 par
   défaut), `DDSOM_VERBOSE=1`, `DDSOM_TRACE_ORIG=1` (trace de l'original à l'écart) et
   `DDSOM_COUVERTURE=fichier` (écrit les instructions de l'original exécutées).
3. `python3 tools/familles_non_vus.py fichier...` liste, par type, les instructions du listing que le
   fuzz n'a pas atteintes (à interpréter : tables de données mal désassemblées et fonctions de
   création appelées par d'autres objets y figurent aussi).
4. `tools/familles_couverture.sh` compte les appels de chaque gestionnaire dans les scénarios.

Pièges rencontrés : le listing est incomplet par endroits (utiliser `examples/dis`) ; les tables
`(tbl,PC,Dn)` se lisent dans l'espace programme (`prog_dec.bin`), les lectures absolues dans l'espace de
données (`prog_plain.bin`) ; des gestionnaires utilisent des registres laissés par la boucle de
dispatch (D0, A1, D5) ; `movea.w` étend l'adresse en signe ; un `tst.w (0x50,A0) ; beq` peut renvoyer
vers du code qui rappelle le même sous-programme.

## 6. Procédure pour réécrire un lot de types

1. **Repérer le type** : `python3 tools/familles_tables.py` donne l'adresse de chaque type ;
   `python3 tools/dis_plage.py <début> <fin>` affiche le listing de la plage avec les tables de sauts
   `(tbl,PC,Dn)` résolues. Le listing est incomplet par endroits (instructions manquantes, tables de
   données décodées en instructions absurdes comme `ori.b #0x0,D0b`) : pour un doute, `examples/dis
   <roms> <début> <fin>` ou les mots bruts de `prog_dec.bin`. Les fonctions placées entre deux
   gestionnaires qui commencent par `jsr 0x22ea` (allocation) ou ne sont atteintes que par des `jsr`
   d'autres objets sont des *fonctions de création* : elles ne font pas partie du gestionnaire.
2. **Comprendre avant d'écrire** : suivre les trois étages d'états (`+4`, `+5`, `+6`/`+7`), noter
   les champs lus et écrits (nom de champ en constante), les sous-routines appelées et les registres
   qu'elles attendent ou rendent (cherche leur contrat dans `docs/objets.md`, `docs/video.md`,
   `src/ai.rs`...). Les sons (adresses 0xe0836-0xee080) se rappellent par `x.call(adresse)` : les
   noter dans un `mod sfx`.
3. **Écrire** un `mod` par type (fonction `run(x: &mut Cx)`, sous-fonctions nommées par état, constantes
   pour les adresses de tables/jeux d'animation, commentaire de rôle en français) et l'inscrire dans la
   table `handlers!` du fichier avec une durée estimée (600 par défaut ; sera calibrée). Reproduire
   exactement : l'ordre des écritures n'a pas d'importance mais les valeurs finales si ; mots, octets
   et longs distincts (`move.w` n'écrit que la partie entière d'une position 16.16) ; arithmétique
   modulo 2^16 / 2^8 ; extension de signe des adresses de 16 bits (`movea.w`) et des index de mots
   `(tbl,PC,Dn.w)` ; appels doubles de `reinsert` (un `jmp 0x1ee1a` dans un sous-état appelé par `jsr`
   revient à l'appelant qui en refait un) ; drapeaux de retenue/Z après les sous-routines
   (`x.anim_step()` rend la retenue, `x.c.cond(n)` lit les autres conditions : 6 `bne`, 7 `beq`, 14 `bgt`...).
4. **Valider** (dans cet ordre), toujours avec `export DDSOM_ROMS=/home/ycr/som/rom
   DDSOM_EEPROM=/home/ycr/som/testdata/eeprom_ddsom.bin` dans la même commande que `cargo`
   (sinon `ddsom-rc` se recompile, ~10 minutes et beaucoup de mémoire) :
   * `cargo build --release -p ddsom-game --examples` depuis `port/`,
   * `fuzz familles ... scenarios/bot30k.txt 3000 200` (tous tes types) puis, pour chaque type qui
     écarte, `DDSOM_ENTREE=<adresse> DDSOM_TRACE_ORIG=1 DDSOM_VERBOSE=1` ; essayer aussi
     `DDSOM_VARIANTES=8` (les variantes hors table font sortir l'original des rails : ces écarts-là
     ne sont pas des erreurs, vérifier dans la trace que l'original saute en pleine donnée),
   * `DDSOM_COUVERTURE=fichier` + `tools/familles_non_vus.py fichier` : les instructions non vues sont
     du code non validé ; si possible, enrichir le fuzz (il est dans `examples/fuzz.rs`, fonction
     `familles`) pour les atteindre, sinon les signaler,
   * `tools/validate_all.sh 30000` : les 8 scénarios doivent être VALIDE, et `cargo test --release
     -p ddsom-game --lib` doit passer (unicité des entrées natives).
5. **Calibrer la durée** : le fuzz affiche « écart à corriger de la durée estimée » par type ; la
   durée propre vaut l'estimation + cet écart (arrondir). Reporter dans `handlers!`.
6. **Décrire** les types dans `docs/familles/<fichier>.md` (adresse, rôle, états, champs, tables ROM
   utilisées, couverture scénarios/fuzz, instructions non vues).

Un gestionnaire qui cède la main à l'ordonnanceur de tâches (`trap`) ne peut pas être natif tel
quel : ne pas le réécrire et le signaler.

## 4. Types réécrits

Les noms de rôle marqués (h) sont des hypothèses tirées du comportement.

### ESHL (`eshl.rs`)

| Type | Adresse | Rôle |
|---|---|---|
| 0 | 0x9dbfc | projectile horizontal à 5 px/image, 2 variantes ; s'arrête sur contact ou terrain, retombe en clignotant |
| 1 | 0x9de10 | boule lancée en cloche (ou jaillissant selon `+0x2f`), rebondit, attend 80 images, clignote |
| 2 | 0x9e0aa | projectile horizontal sans animation propre, détruit au contact |
| 3 | 0x9e1b6 | projectile en éventail attaché au propriétaire (4 variantes, décalages et vitesses en table) |
| 4 | 0x9e2dc | bombe lancée sans propriétaire, un rebond, explosion en fin d'animation |
| 6 | 0x9ebca | effet de contact collé devant le propriétaire (puissance 6 ou 7) |
| 7 | 0x9ecc0 | étincelle posée sur le propriétaire (décalage en table), jusqu'à la fin de l'animation |
| 8 | 0x9ee00 | coup de pointe horizontal (avance X Y Z jusqu'au drapeau d'animation `+0x33`) |
| 9 | 0x9ef46 | attaque à trois formes selon `+3` : boule qui retombe, souffle collé à un ennemi de type 7, étincelle sur un ennemi |
| 10 | 0x9f376 | projectile de bord, ligne de placement choisie par D5 (particularité de l'original) |
| 11 | 0x9f426 | halo collé à un ennemi, qui peut le mettre dans l'état 4 |
| 12 | 0x9f5a8 | marque posée sur un point d'attache `+0x5a` |
| 13 | 0x9f67c | effet collé au propriétaire dont l'animation suit `+0xa1` du propriétaire |
| 14 | 0x9f786 | partie d'un ennemi composite (8 variantes) pilotée par les drapeaux d'attaque `+0xa8/+0xa9` du propriétaire ; crée des tirs de type 15 |

## 5. Couverture

Appels par type dans les 8 scénarios (30 000 images, code d'origine seul) : ESHL 1 (1372), 2 (186),
11 (9324), 13 (5648), 14 (22592), 23 (3006) ; SET 0, 1, 2, 4, 5, 7, 11, 12, 13, 20, 21, 22, 24 ;
SOBJ 0, 6, 11, 14. Tous les autres types ne sont validés que par le fuzz.

## 7. Registres hérités d'un gestionnaire à l'autre

La boucle de dispatch ne fixe que A0, A5, D7, D0 (type × 4) et A1 (adresse du gestionnaire) ; D1-D6 et A2-A4
sont ceux que le gestionnaire précédent (ou, pour le premier, la boucle précédente) a laissés. Les natives ne
garantissent que A0 et D7.

Outils :

* `DDSOM_CHECK=1 DDSOM_CHECK_ALL=1 validate ...` (et `DDSOM_CHECK_ALL=1 fuzz familles ...`) : une ligne
  `REGS routine appels : d2:n ...` par routine native, avec le nombre d'appels où chaque registre de sortie
  diffère de l'original (tous les gestionnaires natifs diffèrent sur beaucoup de registres : seul compte
  ce qu'un successeur lit) ;
* `DDSOM_RESTES=1 fuzz familles ...` : lecteurs de restes **dynamiques**. Le code d'origine de chaque
  gestionnaire des six tables est exécuté depuis la même mémoire en changeant un seul registre de reste
  (D1-D6, A2-A4, mot haut de D0) ; si la mémoire finale change (piles ignorées), le gestionnaire lit ce
  registre. `DDSOM_RESTES_SITE=1` donne l'instruction qui le lit et le chemin parcouru ;
* `restes_statique <roms>` : analyse statique grossière (registre lu avant écrit, suit branchements et
  tables) ; bruyante (tables de données décodées en code), à n'utiliser que pour orienter ;
* lecture du code des natives : `grep "c\.d\[[1-6]\]\|c\.a\[[2-4]\]"` donne les endroits où le Rust lit lui-même un
  registre hérité.

Résultat (scénarios, 149 gestionnaires des six tables, plusieurs milliers d'essais chacun) : seuls trois
gestionnaires lisent un reste dans un état licite :

| Gestionnaire | Registre | Usage | Prédécesseurs |
|---|---|---|---|
| ESHL 10 (0x9f376) | D5 | ligne de la table 0x9f3c8 (Z, décalage X) à la création ; le seul créateur (0x6ca82, ennemi) fait `moveq 0,D5` avant `jsr 0x9f346`, mais l'objet n'est traité qu'à la boucle ESHL suivante : D5 vaut ce qu'ont laissé les autres gestionnaires (mesuré : 0 dans 35 % des cas, sinon 4, 0xe0, 0x100...) | tout gestionnaire ESHL ou ENEMY précédent |
| SOBJ 2 (0xdddb2) | D2 | marge d'écran passée à 0xddbfc, sans que le gestionnaire la pose | SOBJ précédent ; `cull_beyond` laisse maintenant D0/D1/D2 comme 0xddbfc |
| ITEM 6 variante 1 (0xdcddc) | A2 | joueur « choisi » quand aucun joueur n'est vivant (cas rare, non vu par le fuzz) | tout gestionnaire ITEM précédent |

Aucun des trois n'est appelé par les douze scénarios. Ils ne sont fidèles que si les gestionnaires qui les
précèdent laissent le même registre que l'original, ce que les natives ne font pas en général (SOBJ 0, 6, 14
diffèrent sur D2, 25 % des appels pour SOBJ 0). Pour ESHL 10 la valeur de l'original est de toute façon un reste
quelconque ; un dispatch réécrit en Rust devra soit transmettre ces trois registres d'un objet au suivant, soit
fixer la valeur voulue (D5 = 0 pour ESHL 10). Les autres trouvailles des outils sont hors contrat
(ESHL 61 et 63 dans des sous-états absents de leurs tables, EFFCT 4 quand l'action `+0x3d` de l'hôte dépasse la
table à dix entrées 0xaab5e) ou le résultat d'une sous-routine (D2 après 0xfbae...).

## 8. Cas hors contrat du fuzz

Variante `+3` au-delà de la table de sauts de l'original (l'original saute dans du code ou des données ;
`examples/fuzz*.rs` ne les tire plus, voir `variantes_en_table`) : 0xcc25a (2 variantes ; 71 « écarts » sur 600 appels
quand on tirait la variante 2 : saut au milieu d'un `jsr`), ESHL 5 et 9 (3 variantes), ESHL 15 (2). ESHL 14
(8 variantes) est conforme avec toutes ses variantes ; ESHL 5, 9, 14, 15 et 0xcc25a sont à 0 écart avec les trois
outils de fuzz (`fuzz`, `fuzz_eshl_c`, `fuzz_set_a`, `DDSOM_VARIANTES=8 DDSOM_ETATS=1`).
