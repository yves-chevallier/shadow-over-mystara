# Classes PSHL et PEFF : tirs et effets des joueurs

Code : `port/ddsom-game/src/families/joueur/` : `pshl*.rs` (tirs) et `peff*.rs` (effets), éléments communs dans `pshell.rs`
(champs, libération, création de frères, `Cx::state`, `Cx::rand_and`). Même cadre que les autres familles
(`cx::Cx`, `handlers!`, voir `docs/familles-eshl-set-sobj.md`) : un gestionnaire par type, enregistré dans
`families::natives()` sous le nom `pshl_N` ou `peff_N` (`DDSOM_SANS=pshl,peff` les désactive).

## Modèle

Un objet PSHL (pool de 18 × 0x60, table 0x39908, 25 entrées) ou PEFF (48 × 0x60, table 0x39992, 48 entrées) est
créé par une action du joueur (`player/spawn.rs`, docs/personnages.md §8 quater) ou par un autre effet. Champs
communs : `+2` genre (index du gestionnaire), `+3` variante, `+4/+5/+6/+7` états de l'automate (octets pairs), `+0x2c`
regard (0 ou 0x20 ; copié du joueur), `+0x2e` jeu de boîtes de collision, `+0x45/+0x46` puissance du coup (lue par
`combat/shots.rs`), `+0x50` joueur propriétaire (mot ; sert aussi de minuterie après impact pour quelques tirs),
`+0x52` ou `+0x5c` lien vers un objet frère, `+0x57` drapeaux posés par le moteur de collisions (bit 1 : touché),
`+0x30..` tableau des enfants d'un parent.

* **Parent et enfants** : beaucoup d'effets existent en deux exemplaires ou plus, distingués par la variante `+3`
  (le parent crée ses enfants et les pilote par leurs sous-états `+5`).
* Le gestionnaire est appelé par `dispatch.rs` avec A0 = l'objet, D0 = genre × 4 et A1 = adresse du gestionnaire ;
  `Cx::state(table)` laisse D0 = état et D1.w = mot de la table comme `jmp (table,PC,D0.w)` (restes lus par les
  gestionnaires suivants).
* Les tables lues par `(adresse,PC,Dn)` le sont dans le programme déchiffré (`rom8/rom16/rom32`) ; celles lues par un
  registre d'adresse (`lea table,A4` puis `(0,A4,D0)`) le sont en clair (`data8/data16/data32`) : les deux sont
  différentes dans la ROM, y compris pour une table adressée par `lea (d,PC),An`.
* Un tir ou un effet qui meurt appelle `Cx::free_pshl` (0x21d8) ou `free_peff` (0x21e6) ; la création d'un frère passe par
  les allocations 0x225e (PSHL) et 0x228e (PEFF) (`Cx::alloc`, retenue levée si le pool est plein).
* Particularités de l'original reproduites : plusieurs types lisent au-delà de leur objet (`+0x15e` de A0, au lieu du
  joueur ; PEFF 6, 25) ; le tirage aléatoire se masque tantôt en mot (`andi.w`, `Cx::rand_and`), tantôt en octet
  (`andi.b`, `rand_and_b` : l'octet haut du tirage sert alors d'indice, PEFF 12) ; PEFF 12 variante 0 tire `D2` au
  hasard dans un registre laissé par l'appelant ; PEFF 26 relit `(0x168,A1)` avec A1 = adresse du gestionnaire ;
  PSHL 19 ne décrémente jamais l'octet `+0x42` de son état 4 (il reste sorti des listes, occupant son emplacement) ;
  l'effet PSHL 5 compte ses flammes dans `A5-0x5fbc`, PSHL 14, 15, 16 dans `-0x5fba`, `-0x5fb8`, `-0x5fb6`, PSHL 19 dans
  `-0x5fb4` (adresses absolues dans le code).

## PSHL (tirs)

| Type | Entrée | Fichier | Rôle | Validation |
|---|---|---|---|---|
| 0 | 0x51c64 | `pshl.rs` | marqueur d'un coup collé à son porteur (`+0x5c`) | scénarios |
| 1, 9 | 0x51da6 | `pshl.rs` | objet lancé (jeux 0x15ae8a / 0x15c552), rebond puis chute | scénarios |
| 2, 10, 11 | 0x5204e | `pshl.rs` | objet lancé en ligne droite (jeux 0x15af1c, 0x15c5e4, 0x15c62a) | scénarios |
| 3 | 0x52234 | `pshl.rs` | tir à vitesse longue, retombe et se consume | scénarios |
| 4 | 0x523c4 | `pshl.rs` | tir qui rebondit sur le terrain (hache) | scénarios |
| 5 | 0x5265a | `pshl.rs` | projectile qui éclate en flammes (variante 1) avec secousse d'écran | scénarios |
| 6 | 0x527ec | `pshl_e.rs` | tir du rayon (PEFF 9) qui décrit un cercle autour du porteur | fuzz |
| 7 | 0x52b94 | `pshl_b.rs` | sol gelé (magie 15) : image et zone de dégâts | scénarios |
| 8 | 0x52e42 | `pshl_b.rs` | ombres du coup de pied du voleur (trois sortes `+0x2f`) | scénarios |
| 12 | 0x530bc | `pshl_c.rs` | jet de flammes : projectile puis trois gerbes | fuzz |
| 13 | 0x53290 | `pshl_c.rs` | objet d'inventaire lancé (consomme un exemplaire) | fuzz |
| 14 | 0x53412 | `pshl_c.rs` | flamme du jet (compteur `A5-0x5fba`) | fuzz |
| 15 | 0x534d6 | `pshl_c.rs` | projectile à 4 pixels par image (compteur `A5-0x5fb8`) | fuzz |
| 16 | 0x5369a | `pshl_c.rs` | étoile à couleurs tournantes (compteur `A5-0x5fb6`) | fuzz |
| 17 | 0x537fa | `pshl_c.rs` | pierre en arc, secousse et éclair d'écran à l'impact | fuzz |
| 18 | 0x53a08 | `pshl_e.rs` | gerbe de flammes du clerc (cinq variantes) | fuzz |
| 19 | 0x53e94 | `pshl_d.rs` | foudre en chaîne : maillons liés, recherche de cible (ENEMY puis BOSS) | scénarios |
| 20 | 0x542ee | `pshl_d.rs` | mur (une colonne par exemplaire, rang `+3`) | fuzz |
| 21 | 0x54450 | `pshl_d.rs` | couteau lancé | scénarios |
| 22 | 0x54560 | `pshl_d.rs` | boule de feu et son explosion | fuzz |
| 23 | 0x5471a | `pshl_d.rs` | éclair de l'elfe | scénarios |
| 24 | 0x548a0 | `pshl_d.rs` | étincelle de foudre collée à son créateur | fuzz |

## PEFF (effets)

| Type | Entrée | Fichier | Rôle | Validation |
|---|---|---|---|---|
| 0, 1 | 0x49e36, 0x4a21e | `peff_ring.rs` | anneaux de choix d'objets (6 enfants) et d'équipement | scénarios |
| 2 | 0x4a880 | `peff_c.rs` | halo tant que le joueur garde la pose 0x18 | scénarios |
| 3 | 0x4a9fe | `peff_c.rs` | trace d'un coup | scénarios |
| 4 | 0x4abaa | `peff_c.rs` | soin (étoiles, étincelles, potion) | fuzz |
| 5 | 0x4af14 | `peff_c.rs` | lumière sacrée (secousses d'écran en script) | fuzz |
| 6–10 | 0x4b1b6 … 0x4b7c8 | `peff_d.rs` | faisceau, étincelle de fin, lueur, rayon, impact | scénarios |
| 11 | 0x4b93c | `peff_b.rs` | averse de grêlons | scénarios |
| 12 | 0x4bc66 | `peff_e.rs` | le gel (six variantes) | scénarios |
| 13 | 0x4c1b6 | `peff_e.rs` | bénédiction | scénarios |
| 14 | 0x4c2b4 | `peff_e.rs` | renvoi (rayon et quatre faisceaux, blessures) | fuzz |
| 15, 16, 18, 19 | 0x4c5be … 0x4ce0c | `peff_f.rs` | traînée d'arme, bulle d'objet, pluie de pierres, éclair | 16 : scénarios ; autres : fuzz |
| 17 | 0x4c9fe | `peff_l.rs` | curseur d'objet | fuzz |
| 20 | 0x4d08c | `peff_b.rs` | sorcière (parent + éclats) | fuzz |
| 21–23 | 0x4d2fe … 0x4d682 | `peff_g.rs` | étincelle du coup, prière du clerc, lueur | fuzz |
| 24 | 0x4d718 | `peff_o.rs` | tas d'or : reprend l'équipement contre des points (addition BCD) | fuzz |
| 25–29 | 0x4dd02 … 0x4e6ca | `peff_h.rs` | averse, foudre du magicien, anneau, lune, aura | 25, 27, 29 : scénarios ; autres : fuzz |
| 30–32 | 0x4e79c, 0x4ec82, 0x4f026 | `peff_i.rs` | tempête, tourbillon, cible de la foudre | fuzz |
| 33 | 0x4f14c | `peff_b.rs` | brindille projetée depuis un bord de l'écran | scénarios |
| 34 | 0x4f2a6 | `peff_m.rs` | familier du clerc : sort de terre, cherche, bondit et mord | scénarios (mash_c2b), fuzz |
| 35–39 | 0x4fc1a … 0x4ffd0 | `peff_j.rs` | bond, échange, glissade, éclat du coup, éclats d'un monstre touché | 36 : scénarios ; autres : fuzz |
| 40 | 0x50148 | `peff_k.rs` | météores (palette de l'écran recopiée par `movem`) | fuzz |
| 41 | 0x50348 | `peff_n.rs` | gardien invoqué (palette recopiée par `movem`) | fuzz |
| 44 | 0x517dc | `peff.rs` | pastille posée sur la caméra secondaire | scénarios |
| 45 | 0x518b4 | `peff_k.rs` | voyant de l'écran de choix (effets de palette) | fuzz |
| 46, 47 | 0x51ad6, 0x51bcc | `peff_j.rs` | pluie de pièces et minuterie de l'écran de gain | fuzz |

Restent en code d'origine : PEFF 42 et 43 (boutique, 0x50b9c et 0x51076), gros automates non exercés par les scénarios.

## Validation

* `tools/validate_all.sh 30000`, `DDSOM_COV=1` pour les scénarios `cov/` : voir `docs/couverture.md`.
* `examples/fuzz_joueur.rs` : chaque gestionnaire est appelé en mode contrôle depuis des états fabriqués (joueur
  propriétaire aux champs variés, parent et frères factices, compteurs de scène) pendant `DDSOM_PAS` images ;
  `fuzz_joueur <roms> <eeprom> <scénario> <chauffe> <tours> [PSHL|PEFF]`, `DDSOM_ENTREE=<adresse>` pour un seul
  gestionnaire, `DDSOM_ETATS=1` pour tirer aussi des états quelconques (beaucoup sortent des tables de l'original).
* Les écarts du fuzz sur PEFF 30 (variante 1) viennent de parents fabriqués à pointeur absurde (0x1, 0x3) : bruit du
  fuzz, non retenu.
* `examples/types.rs` compte les appels de chaque type par scénario ; `TYPES_OVERCLOCK=16` reproduit la cadence de
  `validate` (sans cela les parties ralentissent et les scénarios n'ont pas le même déroulement).
