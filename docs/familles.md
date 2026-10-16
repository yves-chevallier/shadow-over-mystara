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
calcul de durée (`Ctx::saut_table`).

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

### Sous-routines, états délégués

- les routines du modèle d'objets sont appelées directement (`Ctx::natif`, `anim_start`, `anim_step`,
  `reinsert`, `alloc`, `free`) sans passer par l'index des adresses ; les autres par `Ctx::jsr` (sons,
  événements de score, création d'objets non réécrits...) ;
- `Ctx::jsr_origine` exécute le code d'origine seul : à utiliser quand le gestionnaire lit des registres
  laissés en reste par la sous-routine (le modèle d'objets ne les reproduit pas) ;
- **états non exercés** : un état dont aucun scénario ne passe par le code est délégué au code d'origine
  (`Ctx::deleguer(adresse)`, exécuté comme un `jmp` de la table) plutôt que réécrit sans validation. Le
  comportement reste celui de l'original ; ces états sont listés ci-dessous.

### Outils de validation

- `tools/validate_all.sh` : les huit scénarios, routines en mode contrôle (obligatoire avant commit) ;
- `examples/types.rs` : appels de chaque gestionnaire des trois tables par scénario ; avec `TYPES_ETATS=1`,
  combinaisons (variante, états `+4/+5/+6`) rencontrées ;
- `examples/familles.rs` : contrôle détaillé des gestionnaires (RAM, registres, cycles) avec affichage de
  l'objet fautif et, avec `FAMILLES_TRACE=1`, trace de l'original ; il affiche aussi la **couverture des
  décisions** (`COUV fichier ligne résultat`) : les branchements du Rust jamais pris ou jamais refusés par
  un scénario ne sont pas validés ;
- `examples/fuzz_familles.rs` : mêmes comparaisons sur des états réels perturbés (champs de l'objet et du
  joueur), pour exercer les branches que les scénarios n'atteignent pas.

## 2. ITEM (classe 0xC) : objets ramassables

Pool de 32 emplacements de 0x60 octets (`0xFFE2DE`), 10 types. Un ITEM naît à la position de son créateur
(ennemi, coffre, décor) ; en l'air (`Z` ≠ 0) il tombe, puis attend au sol un temps limité (0x4b0 à 0x960
images) en clignotant (bit 1 de `FLAGS`) avant de disparaître ; le code du joueur qui le touche pose `+0x58`
(ramassage) et `+0x50` (le joueur), et l'objet produit alors son effet.

Champs communs : `+4` état (0 init, 2 actif, 4 libération, 6 hors écran), `+5` sous-état (0 chute,
2 attente, 4 ramassé), `+0x2e` jeu d'ombres (0x3c), `+0x2f` mode de création, `+0x42` durée de vie (mot),
`+0x50` joueur, `+0x54` variante du scintillement (EFFCT 15) créé par `0xae3ec`, `+0x58` ramassé
(1 : ramassage, 2 : forcé), `+0x5d` indice dans le tableau des objets de collection `A5-0x5b88`.

| Type | Entrée | Nom | Comportement | État |
|---|---|---|---|---|
| 0 | 0xdabca | trésor | variantes 0 à 5 ; chute puis attente 0x4b0 images ; ramassé : son + événement de score `0x19ae6` | réécrit |
| 1 | 0xdae26 | consommable | 0-1 nourriture (+20/+40 PV, ou score si PV pleins), 2-3 potions (0x2d0 images), 4 sans effet, 5 compteur `A5+0x45` ; durée 0x960 | réécrit |
| 2 | 0xdb0c2 | arme ramassable (h) | cousin du type 4 (table de paramètres par variante) | non exercé, code d'origine |
| 3 | 0xdb7de | (h) | | non exercé, code d'origine |
| 4 | 0xdbdf6 | objet d'inventaire | 6 emplacements par joueur (`+0x140` identifiants, `+0x148` quantités) ; rangement, échange ou refus (rejet en arc) ; mode 1 : objet jeté, apparaît après `+0x59` images | réécrit |
| 5 | 0xdc382 | bonus animé | mode de création (`+0x2f`) → vitesses initiales ; ramassé : événement `0x19b08` | réécrit |
| 6 | 0xdc624 | objet de scène | variantes 0 et 2 : icône qui suit un joueur et joue un son toutes les 0x3c images ; variante 1 : guide qui marche vers le joueur le plus proche et crée des SET ; variante 3 : voir le code | réécrit (variantes 0, 1, 2) |
| 7 | 0xdce6e | (h) | | non exercé, code d'origine |
| 8 | 0xdd132 | (h) mouvements latéraux, test de ramassage `0xdd470` | | non exercé, code d'origine |
| 9 | 0xdd5a2 | (h) icône attachée à un objet | | non exercé, code d'origine |

États délégués au code d'origine dans les types réécrits : type 6, icône (variantes 0 et 2) sous-états 2
(envol) et 4 (ramassé) et état 4 ; type 6 variante 3 entière.

## 3. BOX (classe 0xA) : caisses et coffres

Pool de 21 emplacements de 0x80 octets (`0xFFEEDE`), 4 types ; seul le type 0 est exercé.

| Type | Entrée | Nom | Comportement | État |
|---|---|---|---|---|
| 0 | 0xd99fa | caisse / tonneau / coffre | une fiche de la ROM (`0x1051ae`, par variante 0 à 3) donne la sorte (`+0x60` : < 3 se brise d'un coup, ≥ 3 coffre), les butins (`+0x62`, par classe de joueur `+0x63..`, aléatoires `+0x6a`) et un effet spécial éventuel (`+0x6b`) ; frappée (`+0x6e` ou `+0x33`), elle joue sa casse, lâche jusqu'à 3 ITEM (centre, droite, gauche) puis clignote et disparaît | réécrit |
| 1 à 3 | 0xda578, 0xda7ac, 0xda7f2 | (h) | | non exercés, code d'origine |

États délégués dans le type 0 : sous-états 2, 4, 6 et 8 de l'état actif (éclats projetés, tremblement des
coffres), sous-sous-état 4 de l'état « touché ».

## 4. EFFCT (classe 0xE) : effets visuels

Pool de 38 emplacements de 0x60 octets (`0xFFD49E`), 64 types, dont 14 exercés par les scénarios. Les
gestionnaires n'ont pas de test `HIDDEN` : l'état choisit directement init / actif / libération.

| Type | Entrée | Nom | Comportement | État |
|---|---|---|---|---|
| 5 | 0xab726 | halo de joueur | position du joueur + décalage (variante, orientation), délai selon `+0x7d` du joueur, vit 0x28 images | réécrit |
| 9 | 0xabbee | traînée | naît sur une position d'écran de la table, avance en X, attend 0x120 images puis 0x200 | réécrit |
| 15 | 0xae428 | scintillement | posé sur un ITEM, suit l'objet, attend le passage d'un joueur puis clignote 8 fois | réécrit |
| 17 | 0xae78c | éléments d'interface | 5 variantes (curseurs, fenêtres de jauge, palettes) posées sur la caméra | réécrit (variantes 0 à 4) |
| 21 | 0xaf360 | bandeau | glisse en Z jusqu'à une cible puis décrémente le compteur de son créateur | réécrit |
| 25 | 0xb111a | poussière | éclats projetés au hasard puis dérive en clignotant | réécrit |
| 26 | 0xb11f4 | nuage | traverse l'écran de droite à gauche | réécrit |
| 32 | 0xb2c56 | aura | suit son propriétaire | réécrit |
| 33 | 0xb2cca | éclat | retombe ; variante 0 : crée deux tirs ennemis à l'atterrissage | réécrit |

Types exercés pas encore réécrits : 1, 4, 6, 29, 31 ; types non exercés : tous les autres.
