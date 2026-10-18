# ESHL 15 à 39 (`families/eshl_b.rs`)

Tirs et attaques des ennemis de la classe ESHL (pool de 38 objets de 0x60 octets, table des gestionnaires
0x39b96). Cadre commun, contrat des gestionnaires et conventions : `docs/familles-eshl-set-sobj.md`. Les noms
de rôle marqués (h) sont des hypothèses tirées du comportement. Machines à états : `+4` état principal
(0 création, 2 actif, 4 fin, 6 libération), `+5` et `+6` sous-états ; les valeurs hors des tables de sauts
de l'original n'ont pas de sens (l'original sort alors du code) et ne sont pas reproduites.

Conventions de lecture des tables : `(table,PC,Dn)` lit l'espace programme (`rom8/rom16`), les accès par
registre d'adresse (`lea` puis `(An)+`) lisent l'espace de données (`data8/data16/data32`) ; le dernier cas
concerne des îlots de données en clair au milieu du code chiffré (types 19, 24, 38).

Champs communs utilisés (en plus de ceux de `shell.rs`) : `+0x42` compteur ou hôte, `+0x46` hôte ou compteur,
`+0x50` propriétaire (mot, sauf types 17 et 19 : long), `+0x56` parent (long, type 17) ou compteur,
`+0x58` délai (mot) ou compteur, `+0x5a` ombre ou drapeau, `+0x5c` créateur (types 24 et suivants).
Variables globales lues : `A5+0x3d` (pause), `A5+0x4e` (gel), `A5+0x6e/0x6f` (compteur d'images),
`A5+0x506/0x50a/0x556/0x55a` (caméras), `A5+0x7f92` (variable de niveau), `A5+0x35`.

Pour les types 20 et suivants qui utilisent l'animation de l'IA, `0x103d6` = `anim_start_indexed` puis
`+0x44 = +0x3e`, `0x1040c` = pas d'animation (retenue = fin) puis `+0x44 = +0x3e` (voir `docs/combat.md`).

## Armes et projectiles visés

| Type | Adresse | Rôle |
|---|---|---|
| 15 | 0x9fe0a | pièce lancée par une partie d'ennemi composite (h) : projectile en cloche (variante 0, 6 phases de rebonds) ou arme tenue qui suit le propriétaire (variante 1) |
| 16 | 0xa03e0 | objet porté puis lancé (h) : suit le propriétaire (`+0x5a` nul), puis lancé en cloche avec la table de vitesses 0x18, passe à l'état 4 en retombant ; liste du premier plan |
| 17 | 0xa05ce | projectile à trajectoire visée, trois modes selon `+0x43` (h) : charge 60 images, attend 20 images, part vers `+0x50` ; éclaire la palette 0x91c300 |
| 18 | 0xa0a06 | projectile oblique à table de vitesses choisie par `A5+0x7f92` (h) |
| 19 | 0xa0b20 | projectile lancé vers sa cible, huit sprites selon l'angle (h) |
| 20 | 0xa0c5a | coup d'une partie d'ennemi collé au propriétaire (h), deux variantes ; crée un tir de type 21 |
| 21 | 0xa0f96 | onde attachée à une arme (h) ; crée un effet EFFCT 0x18 |
| 22 | 0xa1180 | éclat lancé par un ennemi (h) : vitesses par variante, retombe et clignote 30 images |
| 23 | 0xa1292 | objet de décor qui glisse le long de la caméra (h) |

### Type 15 (0x9fe0a)
Hiérarchie : le propriétaire (`+0x50`) est un tir de type 14 dont le propriétaire est l'ennemi, dont
le propriétaire (`+0x50`) est sa cible. Création (état 0) : placé devant le propriétaire (variante 0 : X 0x67,
Z 0x4a, Y 0x10 ; variante 1 : 0x42, 0x2e, 0). Variante 0 : état 2 sous-état 0, six phases (`+6`)
de vol avec rebond (vitesses -0x180/0x3c0, -0x100/0x300, -0xc0/0x280, -0x80/0x200, attente de 50 images
(`+0x5c`), clignotement) ; sens inversé toutes les 5 images (`+0x43`, xor 0x20 de `+0x2c`) ; vitesses
initiales de la table 0xa034e (8 octets par ligne, ligne 8, 0x10 ou 0x18 selon la portée entre l'ennemi et sa
cible). Variante 1 : `+5` = 2 (tenue) ou 4 (coup en trois animations 0x11, 0x12) tant que l'ennemi n'est pas
dans l'état 0x402 ni n'a le bit 0 de `+0xa8`. Animations : jeu 0x17a276.

### Type 16 (0xa03e0)
États : 0 (animation 0 ou 1 selon `+0x43`), 2 (`+5` = 0 porté, ≠ 0 lancé), 4 (PV du propriétaire `+0x62`
épuisés : lancé ; sinon `+0x62` de l'objet décide du retour à l'état 2), 6 libération. Animations 0x16e146.

### Type 17 (0xa05ce)
États : 0 palette (8 longs copiés de 0x25a660 vers 0x91c300 puis OU avec les bits de teinte de 0x91c000 et
0xf000f000), décalage d'apparition par `+0x42` (table 0xa0670), animation 0 ; 2 (`+5` : 0 charge 60 images,
2 visée, 4 chute), 4 animation de fin. Le parent (`+0x56`, long) à l'état >= 4 tue le tir. Trois modes (`+0x43` :
0, négatif, positif) ; mode positif : pente = z / (distance / 8) et angle d'après `1376` (tables 0xa07ac) ;
mode 0 : angle d'après `1386` (table 0xa0892, table de vitesses 0x44) ; mode négatif : chute devant la cible
(table 0xa0962). Animations 0x16e1f4, sons 0xed686.

### Type 18 (0xa0a06)
Création : table de vitesses choisie par un octet de 0xa0a60 indexé par `A5+0x7f92` ; état 2 : détruit
sur contact (`+0x57` bit 3 avec son 0xe7376, bit 5), sur terrain 5..8 sans bit 6, sinon déplacement par table.
Animations 0x16e534.

### Type 19 (0xa0b20)
Direction (`+0x2d`) et animation d'après l'angle vers la cible, en 8 pas : tables 0xa0bea (regarde à gauche, base 0x10)
et 0xa0bda (base 9) de paires (`+0x2d`, animation), décalages (X, Z) en 0xa0bfa ; table de vitesses 0x40.
État 2 : vol jusqu'à Z < 0 puis 48 images d'impact (animation 0x10). Animations 0x16e5fa.

### Types 20 et 21 (0xa0c5a, 0xa0f96)
Ils se collent au propriétaire avec un décalage tiré d'une table de 8 octets par variante (X, Y entier, Y
fractionnaire signé, Z : tables 0xa0f86 et 0xa113a). Type 20 : variante 0, attente d'animation aléatoire
(table 0xa0d60) ; quand le propriétaire arme son attaque (`+0xa5` bit 0) ou après un délai
(table 0xa0e2c, variante 1) : coup (sous-état 2, `+6` 0 à 6, crée un type 21 dans `+0x42`).
Type 21 : 0x4f répétitions de l'animation 0, crée un EFFCT (type 0x18) mémorisé en `+0x46`, puis animation de
sortie ; disparaît si `A5+0x4e` est non nul ou si `+0xb2` de l'ennemi hôte l'est (`+0x1e` = 1).
Animations 0x17d7d2 / 0x17d7e0 (20) et 0x16e752 (21).

### Types 22 et 23 (0xa1180, 0xa1292)
22 : vitesses 32 bits par variante (table 0xa123a, 8 octets), décalages par `+0x2f` (0xa1194, 4 octets) ;
vole jusqu'à Z <= 0 puis clignote 30 images (`+0x54`). 23 : placé sur le bord de la caméra, glisse
48 images selon la table de vitesses 0xd, attend `A5+0x42`, repart avec la table 2 pendant 48 images.

## Créateurs, rideaux et débris

| Type | Adresse | Rôle |
|---|---|---|
| 24 | 0xa137e | générateur de vague (h) : variante 0 se pose sur le créateur (`+0x5c`), crée des tirs de variante 1 (positions 0xa1438) et un groupe d'ennemis ENEMY type 1 à `+0x58 == 0x20` |
| 25 | 0xa16bc | projectile qui tombe du ciel sur le créateur (h), ombre par 0x54818, impact (sons 0xab7d8 / 0x2860 / 0xeafdc) |
| 26 | 0xa18dc | projectile lancé en cloche, ombre par 0x54842, 128 images au sol puis 96 en clignotant (h) |
| 27 | 0xa1a64 | épicentre (variante 0) qui crée une rangée de tirs 0x1b (variante 1) (h) |
| 28 | 0xa1c96 | fragment lancé par le créateur, table de vitesses 0x3f, test de sortie d'écran une image sur 4 (h) |
| 29 | 0xa1d70 | flamme (variante 0 centre, 1 rangée, 2 retardée) : rangée de tirs 0x1d (h) |
| 30 | 0xa2060 | coup collé au propriétaire, armé par `+0xaf` (h) |
| 31 | 0xa21b6 | boule à tête chercheuse d'un créateur (h) |
| 32 | 0xa239a | projectile visé sur l'hôte (`+0x42`), animations IA (h) |
| 33 | 0xa2524 | tir en trois temps lié à son hôte (h) |
| 34 | 0xa2630 | mise en scène d'une attaque de boss : voile plein écran, décor SET 0x47, palettes (h) |
| 35 | 0xa28d2 | éclat tombant à 16 variantes (h) |
| 36 | 0xa29cc | débris lancé en l'air (h) |
| 37 | 0xa2b62 | météorite (h), effet d'impact 0xb10c8 / 0xb4bcc |
| 38 | 0xa2d48 | projectile orienté, 16 sprites selon l'angle borné (h) |
| 39 | 0xa2fb8 | boule de feu lancée en l'air (h) |

Aides communes (en tête de la deuxième moitié du fichier) : `spawn_row` (rangée de tirs de part et d'autre,
types 27 et 29), `blink_by_parity`, `guarded` (suspension, état, réinsertion), `ai_start` / `ai_step`.

## Couverture et validation

* Scénarios (`tools/validate_all.sh 30000`, 8 scénarios) : VALIDE, mais **aucun des types 15 à 39 n'y est appelé**
  (seuls ESHL 1, 2 et 11 le sont dans la liste des appels) : leur validation repose sur le fuzz.
* `fuzz familles` (`DDSOM_ETATS=1 DDSOM_PAS=150 DDSOM_VARIANTES=2`, 400 tours après 3000 images de chauffe,
  environ un million d'appels, de 1 100 à 60 000 par type) : 0 écart. Sans `DDSOM_VARIANTES=2`, le type 15
  écarte pour la variante 2 (hors table : l'original saute dans le code du type 29) ; ce n'est pas une erreur.
  Enrichissements additifs de `examples/fuzz.rs` : champs `+0x5c`, `+0x42`, `+0x58`, pointeurs longs des types
  17 et 19, variables `A5+0x7f92/0x35/0x36/0x6e/0x6f/0x42/0x4e`, propriétaire ramené à l'écran, durée des
  trajectoires `DDSOM_PAS`, états de départ valides par type `DDSOM_ETATS`.
* Instructions de l'original jamais exécutées par le fuzz (code non validé), hors fonctions de création
  placées entre les types : 15 : phases 1 à 3 du vol et fin du clignotement (9ffdc-a0148...), propriétaire absent ;
  16 : la branche « `+0x5a` non nul dans l'état 4 » (a0526-a054c) ; 17 : modes 0 et négatif au-delà de l'attente
  (a070a-a0780, a07d0..., a0846..., a0916...) ; 18 : sortie par terrain 5..8 sans mur ; 20 : propriétaire absent ;
  21 : boucle principale `+5 = 0` (a103e-a10c8) et fin ; 22, 23, 24, 25, 26, 27, 28 : seulement des
  cas d'échec d'allocation ; 29 : variante 2 et sorties de créateur ; 30, 31, 32, 33, 34 : cas où `+0x5c` / hôte
  absent ou allocation SET échouée ; 37 : propriétaire absent ; 38 : l'angle visé quand le propriétaire a une cible
  (a2d8c-a2de8) ; 39 : cas où `+0x5c` est absent.
* Registres : seuls A0 et D7 sont garantis, comme dans `eshl.rs` ; les registres D0-D2 / A1-A4 laissés par
  l'original en sortie ne sont pas reconstitués (aucun écart visible aux scénarios, car ces types n'y sont pas appelés).
