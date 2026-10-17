# Modèle d'objets (O1 et O2)

Ce document décrit les structures découvertes en réécrivant le modèle d'objets en Rust
(`port/ddsom-game/src/objects/`). Les décalages sont ceux de l'enregistrement en RAM ; `A5` = 0xFF8000.
Tout ce qui est ici est vérifié par la validation différentielle (`tools/validate_all.sh`), sauf mention
« non exercé ».

## 1. Vue d'ensemble

Tout ce qui bouge à l'écran (tirs, effets, ennemis, boss, décors animés, objets ramassables, objets
d'interface) est un **objet** : un enregistrement de 0x50 à 0xC0 octets dans l'un des 10 **pools** de la
RAM de travail. Les joueurs ont leur propre enregistrement (0x200 octets, voir `docs/joueur.md`) mais
partagent les champs communs.

Cycle de vie d'un objet :

1. `pool.alloc` réserve un emplacement libre (octet `+0x00` nul) et y écrit la classe (`+0x28`) ;
2. l'appelant remplit l'objet (type `+0x02`, position, animation...) et met `+0x00` à une valeur non nulle ;
3. chaque image, la boucle de dispatch de la classe appelle le gestionnaire du type ; le gabarit d'un
   gestionnaire est : `pause_hook` (0x29ea) → `tick_counters` (0xf1bfc) → automate d'états (`+4..+7`) →
   animation (`anim_step`) → déplacement (`obj_move_*`) → `list_reinsert_actors` (0x1ee1a) ;
4. `pool.release` retire l'objet des listes triées, rend la place et remet l'emplacement à zéro.

## 2. Champs communs d'un objet

| Décalage | Taille | Nom Rust (`Obj::`) | Sens |
|---|---|---|---|
| +0x00 | octet | `ACTIVE` | non nul : emplacement occupé |
| +0x01 | octet | `FLAGS` | bit 0 : visible à cette image (écrit par `obj_on_screen`) ; bit 1 : caché |
| +0x02 | octet | `KIND` | type : index du gestionnaire dans la table de la classe |
| +0x03 | octet | `VARIANT` | variante (pour les objets de personnage : numéro de personnage, indexe les jeux d'animation) |
| +0x04..07 | 4 octets | `STATE` | états de l'automate ; `+4` = 6 : disparition (posé par `obj_cull_offscreen`) |
| +0x08 | long 16.16 | `X` | abscisse en pixels (le mot haut est la partie entière) |
| +0x0c | long 16.16 | `Z` | altitude |
| +0x10 | long 16.16 | `Y` | profondeur sur le sol ; **clé de tri** des listes ; hauteur à l'écran = `Z + Y` |
| +0x14 / 0x16 | mots 8.8 | `VX` / `AX` | vitesse et accélération en X |
| +0x18 / 0x1a | mots 8.8 | `VZ` / `AZ` | idem en Z |
| +0x1c / 0x1e | mots 8.8 | `VY` / `AY` | idem en Y |
| +0x20 | long | `ANIM_NEXT` | adresse de l'entrée suivante du script d'animation |
| +0x24 / 0x26 | mots | `PREV` / `NEXT` | liens de la liste triée (§4) |
| +0x28 | octet | `CLASS` | code de classe (§3) |
| +0x2d | octet | `SPEED_INDEX` | index dans la table de vitesses |
| +0x30 | mot | `ANIM_TIMER` | images restantes de l'entrée d'animation courante |
| +0x32 | mot | `ANIM_FLAGS` | drapeaux de l'entrée (octet haut : 0 suite, 0xFE fin, autre saut) |
| +0x34 | long | `FRAME` | cadre de sprite courant (adresse ROM) |
| +0x38 / 0x3c | longs | `FRAME_DATA_A` / `_B` | données du cadre : ombre (+0x38..3b), boîte de visibilité (+0x3c) et priorité (+0x3f) |
| +0x40 / 0x44 | longs | `FRAME_DATA_C` / `_D` | seulement pour les scripts d'animation longs ; l'octet `+0x40` sert aussi d'indicateur « caché/suspendu » (`HIDDEN`, testé par 0x29ea) |
| +0x4c | long | `SPEED_TABLE` | table de vitesses courante |
| +0x53 | octet | `COUNTER_53` | remis à zéro par `tick_counters` quand il est négatif |
| +0x88 / 0x89 | octets | `COUNTDOWN_FLAG` / `COUNTDOWN` | compte à rebours ; en arrivant à ≤ 0, `COUNTDOWN_FLAG` est effacé |

Les autres champs (`+0x48/4a` objets rattachés, `+0x50` propriétaire, `+0x62` PV...) relèvent des familles
d'objets et des joueurs.

## 3. Pools

Chaque pool : tableau d'emplacements, compteur de places libres (mot), classe.

| Pool | Classe | Début (A5+) | Taille × nombre | Compteur (A5+) | Dans les listes | Allocation | Libération |
|---|---|---|---|---|---|---|---|
| PSHL (tirs joueur) | 6 | 0x10de | 0x60 × 18 | 0x7e5e | oui | 0x225e | 0x21d8 |
| PEFF (effets joueur) | 0x12 | 0x179e | 0x60 × 48 | 0x7e60 | oui | 0x228e | 0x21e6 |
| ENEMY | 2 | 0x299e | 0xC0 × 22 | 0x7e62 | oui | 0x22bc | 0x21f4 |
| BOSS | 4 | 0x3a1e | 0xC0 × 3 | 0x7e6c | oui | 0x23a8 | 0x2234 |
| ESHL (tirs ennemis) | 8 | 0x3c5e | 0x60 × 38 | 0x7e64 | oui | 0x22ea | 0x2202 |
| SET (décors) | 0x10 | 0x4a9e | 0x50 × 32 | 0x7e66 | **non** | 0x2348 | 0x2210 |
| EFFCT | 0xE | 0x549e | 0x60 × 38 | 0x7e68 | oui | 0x2318 | 0x2218 |
| ITEM | 0xC | 0x62de | 0x60 × 32 | 0x7e6e | oui | 0x23d8 | 0x2242 |
| BOX | 0xA | 0x6ede | 0x80 × 21 | 0x7e6a | oui | 0x2378 | 0x2226 |
| SOBJ | 0x14 | 0x795e | 0x50 × 16 | 0x7e70 | oui | 0x2408 | 0x2250 |

**Allocation** (`Pool::alloc`, routines 0x225e-0x2408) : si le compteur est nul, échec sans parcours ;
sinon premier emplacement dont `ACTIVE` est nul ; le compteur baisse, `CLASS` est écrit, l'objet n'est
**pas** encore actif. Contrat de registres : A1 = emplacement (en cas d'échec : début du pool si le
compteur était nul, fin du pool sinon), retenue C = échec (`move #1,CCR`), succès : N/Z d'après D0.b, V = C = 0 ;
D6 sert de compteur de boucle (`moveq n-1` puis `dbf`) : il vaut `n-1-i` au succès à l'indice i, 0xffff
au parcours épuisé, et reste inchangé si le compteur était nul.

**Libération** (`Pool::release`, 0x21d8-0x2250) : retrait des listes triées (`list_unlink`, sauf SET),
compteur + 1, emplacement remis à zéro. L'original laisse D0 = 0, D1.w = 0xffff, A4 = fin de
l'emplacement.

**Initialisation** (0x2062, et séparément SET 0x2134, EFFCT 0x2150, SOBJ 0x21bc) : emplacements remis à
zéro, compteur = nombre d'emplacements, files de collision vidées. L'ordre est PSHL, PEFF, ENEMY, BOSS,
ESHL, SET, EFFCT, BOX, ITEM, SOBJ.

**Files de candidats aux collisions** : cinq catégories, chacune avec un nombre d'entrées, un pointeur
d'écriture (adresse RAM 16 bits) et un tableau de pointeurs d'objets (mots) ; `obj_on_screen` y inscrit
les objets visibles, la phase de collisions (0x260ee) les lit. Remises à vide par l'initialisation des
pools correspondants (et chaque image par la boucle de partie).

| Catégorie (classes) | Nombre (A5+) | Pointeur (A5+) | Tableau (A5+) | Capacité |
|---|---|---|---|---|
| PSHL (6) | 0x7e72 | 0x7e7c | 0x7e86 | 18 |
| ESHL (8) | 0x7e74 | 0x7e7e | 0x7eaa | 38 |
| ENEMY et BOSS (2, 4) | 0x7e76 | 0x7e80 | 0x7ef6 | 25 |
| BOX (0xA) | 0x7e78 | 0x7e82 | 0x7f28 | 21 |
| ITEM (0xC) | 0x7e7a | 0x7e84 | 0x7f52 | 32 |

## 4. Listes triées par profondeur (O2)

Deux listes doublement chaînées intrusives portent les objets à dessiner :

| Liste | Tête (A5+) | Queue (A5+) | Fin de liste |
|---|---|---|---|
| acteurs (personnages, ennemis, décors) | 0x5be | 0x5c0 | 1 |
| premier plan | 0x5c2 | 0x5c4 | 2 |

Liens `+0x24` (précédent, vers la tête) et `+0x26` (suivant, vers la queue) : adresse RAM sur 16 bits
(bit 15 à 1 : 0xFF8000-0xFFFFFF, lue avec extension de signe par `movea.w`), ou une **marque de fin**
positive (1 ou 2) aux deux bouts, ou 0 si l'objet n'est dans aucune liste. Une liste vide a tête et queue à 0.
La liste est triée par `+0x10` (signé 32 bits) **décroissant** de la tête à la queue ; à égalité, l'objet
inséré passe après les autres.

`list_reinsert_actors` (0x1ee1a) / `list_reinsert_foreground` (0x1ef6c), appelés par la fin de presque
tous les gestionnaires (587 sites) :

- objet absent de la liste (`PREV + NEXT == 0`) : recherche depuis la queue de la place d'insertion ;
- objet présent : déplacé seulement si l'ordre est rompu avec un voisin immédiat, en remontant vers la
  tête (`Y` plus grande que celle du précédent) ou en descendant vers la queue (plus petite que celle du
  suivant) jusqu'à la bonne place ; sinon rien.

La durée dépend du nombre de voisins parcourus ; la routine native la reconstitue à partir de la suite
des décisions de branchement de l'original (`objects::cost`).

`list_unlink` (0x1f0be) retire l'objet (cinq cas : seul, premier, dernier, au milieu, absent) et met ses
liens à 0 ; `list_unlink_all` (0x1f184) vide les acteurs puis, **seulement si les acteurs n'étaient pas
vides**, le premier plan.

## 5. Scripts d'animation (O1)

Un script est une suite d'**entrées** en ROM. L'objet recopie l'entrée courante dans ses champs et garde
l'adresse de la suivante (`+0x20`).

| Décalage dans l'entrée | Taille | Contenu | Champ de l'objet |
|---|---|---|---|
| +0x00 | mot | durée en images | `+0x30` |
| +0x02 | mot | drapeaux (l'**octet haut** compte) | `+0x32` |
| +0x04 | long | cadre de sprite | `+0x34` |
| +0x08 | long | données A (ombre) | `+0x38` |
| +0x0c | long | données B (boîte de visibilité, priorité) | `+0x3c` |
| +0x10, +0x14 | longs | seulement dans les entrées **longues** (personnages) | `+0x40`, `+0x44` |

Octet de drapeaux de l'entrée terminée : 0 → l'entrée suivante suit immédiatement ; 0xFE → fin de script
(la durée est remise à 1 et le pas renvoie C = 1 : l'objet reste sur son dernier cadre) ; toute autre
valeur → un long suit l'entrée et donne l'adresse de l'entrée suivante (boucle ou saut).

`anim_step` (0x1a56, variante longue 0x2d58) : `+0x30 -= 1` ; à zéro, charge la suivante selon les
drapeaux. Les appelants testent la retenue (`bcs` = fin de script) ; le pas laisse A4 = adresse après
l'entrée chargée, D0.b = drapeaux.

Démarrage d'un script :

- `anim_start` (0x1a3c) : A4 = adresse de l'entrée ;
- `anim_start_indexed` (0x1a32) : A4 = **jeu d'animations**, D0.w = numéro. Un jeu est un tableau de mots
  (décalages signés relatifs au début du tableau) : le script numéro n commence à `A4 + mot[n]` ;
- jeux propres aux personnages (entrées longues) : 0x2c16 (table de pointeurs en 0x2c58, numéro D6 décalé
  de 0x40 quand l'octet `+0x68` est nul), 0x2c88 (table 0x2cc0), 0x2cf0 (table 0x2d28, non exercé). La
  table de pointeurs est indexée par `VARIANT` × 4 et lue dans l'espace programme (relatif au PC).

## 6. Vitesses (O1)

Unités : vitesse et accélération en 8.8, position en 16.16. Un déplacement = `(vitesse signée) << 8`.

- **Balistique** : `v += a ; pos += v << 8`. Routines `obj_move_*` : 0x1d62 (Y puis X), 0x1d78 (X), 0x1d90
  (Y, X, Z), 0x1da6 (X, Z), 0x1dbc (Z), 0x1dd4 (Y, Z), 0x1e02 (Y). Elles s'enchaînent (le code d'un axe
  tombe dans le suivant) ; D0 = déplacement du dernier axe, drapeaux de la dernière addition.
- **Par table** (`+0x4c` table, `+0x2d` index) : la table contient, par index, deux mots (vx puis vy ou
  vz) ; `v = table + a`. 0x1cb4 : X et Y ; 0x1cee : X et Z. Laissent D0 = index × 4, D1 = déplacement
  du second axe, A4 = table.
- **Choix de la table** : 0x1a94 et 0x1b64 (D0 = numéro de table, tableaux de pointeurs en 0x1ab4 et
  0x1b84, dans le code) : mémorisent la table dans `+0x4c` et chargent `VX`, `VY` depuis l'index courant.

## 7. Présence à l'écran (O1)

Coordonnées écran en mots : x = `X - caméra.x`, hauteur = `Z + Y - caméra.y` (la caméra est celle de
`A5+0x4fe`, ou de `A5+0x54e` si `A5+0x36 != 0`).

- `obj_cull_offscreen` (0x1e1a) : si x + 0x40 ≥ 0x200 ou hauteur + 0x40 > 0x180 et l'objet est actif, `+4` ← 6.
- `obj_in_arena` (0x1e92, marge dans D0.w ; 0x1e90 : marge 0x3e) : C = dedans (zone élargie de la marge).
- `obj_on_screen` (0x1ed2) : visibilité (décors, classe 0 : marge 0x60 ; autres : boîte du tableau
  0x1ca538, 8 octets par index `+0x3c` : dx, largeur, dy, hauteur) ; met à jour le bit 0 de `FLAGS` et
  inscrit l'objet dans la file de candidats de sa classe. Retourne D0 = 0 (hors écran ou caché), 1 (sans
  file), classe × 2. Elle est aussi réécrite à l'intérieur de `sprites.rs` (`check_visible`).

## 8. Divers

- `pause_hook` (0x29ea) : ne fait que `tst.b (0x40,A0)` ; appelé en tête de ~80 gestionnaires, qui
  enchaînent sur `bne` (objet caché ou suspendu).
- `tick_counters` (0xf1bfc) : voir §2 (`+0x53`, `+0x88`, `+0x89`).
- **Rang** (difficulté dynamique, hypothèse ; `objects/rank.rs`) : structure à `A5+0x7f92`. `+0x00` rang
  courant (0..=31), `+0x02` maximum, `+0x04` minimum, `+0x06` rang initial, `+0x0a` images entre deux
  montées, `+0x0c` idem hors partie (0x384), `+0x0e` quatre octets (baisse selon `A5+0x35`), `+0x12`
  compte à rebours (0xFFFF = arrêté). Les réglages `+0x02..0x11` viennent d'une ligne de 16 octets de
  la table 0x1f1d8, choisie par `A5+0x2a`. 0x1f1b8 charge la ligne puis remet à zéro, 0x1f258 remet à zéro
  (rang ← initial), 0x1f27c monte d'un cran toutes les `+0x0a` images (une fois par image, en partie),
  0x1f2c0 monte d'un cran, 0x1f300 baisse (joueur touché). Le rang est lu par de nombreuses routines
  d'IA et de dégâts (0x400c, 0x456c, 0x79de, 0xa480...).

## 9. Routines natives du modèle d'objets

Toutes avec durée exacte (`exact_cost`) sauf `obj_on_screen` (estimée). Contrat : tous les registres et
drapeaux (`regs::ALL`) pour O1 ; pour O2, les seuls registres lus par les appelants (voir
`objects/pools.rs`, `objects/lists.rs`).

| Adresse | Nom | Module |
|---|---|---|
| 0x1a32, 0x1a3c, 0x1a56 | `anim_start_indexed`, `anim_start`, `anim_step` | anim |
| 0x2c16, 0x2c88, 0x2cf0, 0x2d58 | `anim_start_long_*`, `anim_step_long` | anim |
| 0x1a94, 0x1b64 | `obj_select_speed_table_a/b` | motion |
| 0x1cb4, 0x1cee | `obj_move_table_xy/xz` | motion |
| 0x1d62, 0x1d78, 0x1d90, 0x1da6, 0x1dbc, 0x1dd4, 0x1e02 | `obj_move_*` | motion |
| 0x29ea, 0xf1bfc | `obj_pause_hook`, `obj_tick_counters` | motion |
| 0x1e1a, 0x1e90, 0x1e92, 0x1ed2 | `obj_cull_offscreen`, `obj_in_arena*`, `obj_on_screen` | cull |
| 0x2062, 0x2134, 0x2150, 0x21bc | `pools_init_all`, `pool_init_set/effct/sobj` | pools |
| 0x225e-0x2408 | `pool_alloc_*` (10) | pools |
| 0x21d8-0x2250 | `pool_free_*` (10) | pools |
| 0x1ee1a, 0x1ef6c, 0x1f0be, 0x1f184 | `list_reinsert_actors/foreground`, `list_unlink`, `list_unlink_all` | lists |
| 0x1f1b8, 0x1f258, 0x1f27c, 0x1f2c0, 0x1f300 | `rank_load_preset`, `rank_reset`, `rank_tick`, `rank_raise`, `rank_lower` | rank |

Durées : `timing::path_cycles` suffit aux routines sans boucle ; pour celles qui bouclent (allocation,
initialisation, listes) `objects::cost::walk_cycles` parcourt le code d'origine en suivant la suite des
décisions de branchement que la routine native en déduit de la RAM (une `Trace`), ce qui donne la
durée exacte de l'original (vérifiée par la validation à chaque appel).
