# Boucle de partie, dispatch des objets et phases (W0, W1)

Code Rust : `port/ddsom-game/src/dispatch.rs` (W0) et `game.rs` (W1). `A5` = 0xFF8000.
Tout ce qui est ici est vérifié par la validation différentielle (`tools/validate_all.sh`) sauf mention
« non exercé ».

## 1. La tâche de partie (0x1c45a)

Elle remplace la tâche 0 (machine à modes) au début d'une partie (`0x140b4`, `0x13c2c`). Elle ne
rend jamais la main par un `rts` : c'est une boucle infinie qui cède une fois par image.

```
0x1c45a   A5 ← 0xFF8000 ; phase et étape (A5+0x2c..0x2f) ← 0 ; A5+0x30..0x33 ← 0
0x1c468 ┌ compteur d'images de partie (A5+0x6e) += 1
        │ si A5+0x82 : compteur d'images (A5-0x6610) ; tous les 60, appel de 0x2e540 (service)
0x1c48c │ cession : 0xa40 avec D0 = 1 (une image), sauf la première fois après la remise à zéro
        │         de l'octet A5-0x660e (« déjà cédé » ; A5-0x660f compte alors 1 à 4)
0x1c4b4 │ empile 0x1c468 (retour de boucle), puis appelle la phase : table 0x1c502, indexée par A5+0x2c
        │ → la phase s'exécute **d'une traite**, fait un pas, revient en 0x1c4c4
0x1c4c4 │ remise à vide des 5 files de candidats aux collisions (§4), puis `rts` → 0x1c468
        └
```

Les phases ne cèdent donc jamais elles-mêmes : chacune est un pas par image. C'est ce qui permet de
les réécrire en routines natives (une routine native ne peut pas céder). Seule la boucle principale,
qui contient le `jsr 0xa40`, reste en 68000 (on ne peut pas céder au milieu d'une routine native) ; la
réécrire en Rust demandera de la découper aux points de cession ou d'avoir des tâches
coopératives en Rust.

Rust : `game::FRAME_END` (0x1c4c4).

### Avance rapide et plafond de pas par image

Le test de 0x1c48c lit l'octet FF19F2, que le VBLANK efface (0x5ea) et que la boucle repose en 0x1c4b0 : tant
qu'il est posé, chaque pas cède une image (0xa40, D0 = 1 : état 1 = attente, compteur +1 décrémenté par le
VBLANK). Quand un bouton est pressé (à l'attract, à l'entrée d'une scène, en jeu...), une tâche auxiliaire
(entrée 13 de la table ; tâche 14 dans `tasks`, code 0xee080-0xee164) fait elle-même ce que fait le VBLANK pour
la tâche principale : décrémenter son compteur d'attente et la remettre prête, puis cède par 0xa86 (trap #4 :
reste prête) et recommence. L'ordonnanceur (0x928) la relance aussitôt : **un pas de jeu par passage de la tâche
auxiliaire, sans attendre l'image** (« avance rapide » ; hors bouton, la même tâche tourne à vide, une soixantaine
de fois par image à ×1, sans effet). Le nombre de pas par image vaut donc (cycles de l'image) / (cycles d'un
pas) :

| Mesure (`examples/passages`, `DDSOM_STEP_CAP=0`) | ×1 (arcade) | ×16 |
|---|---|---|
| `cov_attract_coin`, images 21302-21305 | 8, 7, 8, 7 | 136, 128, 129, 128 |
| `cov_tour6_c5`, entrée de scène | 9-10 | 54-170 |
| `cov_tour7_c0`, en jeu | 5-7 | 111-118 |

Cet écart ne se mesure pas sur le code seul : il dépend de la durée de chaque routine (les natives n'ont pas
celle de l'original), donc deux machines surcadencées, ou une native et son original, ne s'accordent plus.

**Plafond** (`ddsom_game::MAX_STEPS_PER_FRAME` = 8 pas, `GAME_STEP` = 0x1c4b0, `cps2::Machine::step_cap`) : au plus 8
passages en 0x1c4b0 par image ; le suivant est retenu, le reste de l'image est sauté (comme `idle`) et le pas
s'exécute au début de l'image suivante, après l'IRQ du VBLANK. Huit est la médiane des mesures de l'arcade (5 à 10
selon le coût du pas). Installé par `ddsom_game::install` (jeu et machine hybride de la validation) et par
`ddsom_game::install_reference` (code d'origine de la validation, `determinisme`, `tasks`, `watch`...), donc
identique des deux côtés ; `DDSOM_ORIGINE=1` (code d'origine à la vitesse de l'arcade) n'est pas plafonné.
C'est un écart volontaire avec le code d'origine **surcadencé** (qui ferait 130 pas), pas avec l'arcade. Un pas
retenu ne change pas l'ordre des tâches : la tâche principale occupe le premier emplacement de l'ordonnanceur,
elle reprend avant les tâches réveillées par le VBLANK, comme dans une image normale.

## 2. Phases (A5+0x2c, pas de 4, table 0x1c502)

| A5+0x2c | Handler | Rôle | Rust |
|---|---|---|---|
| 0 | 0x1c522 | initialisation de la partie (étapes `A5+0x2d` = 0, 4, 8, 12, 16) | `game::PHASE_0` |
| 4 | 0x1ca20 | sélection des personnages (h) | non |
| 8 | 0x20c38 | transition/entrée de stage | non |
| **12** | 0x1c6a0 | **le jeu proprement dit** (le compte à rebours BCD est le temps de la partie) | `game::PHASE_12` |
| 16 | 0xf3142 | séquences de fin/transition de stage | non |
| 20 | 0x1c80c | fin de partie | non (voir plus bas) |
| 24, 28 | 0x21c2a, 0xfb292 | scènes spéciales | non |

### Phase 0 (initialisation, un pas par image)

| Étape (A5+0x2d) | Contenu |
|---|---|
| 0 | `A5+0x38` ← 0x40, partie active, `pool_init_set`, trois objets SET (type 0, variantes 0x3e/0x3f/0x40, indice de palette `+0x2f` = 0/1/2), préparation des joueurs (0x35288, 0x3525c), défilement du plan 2 + 0x100 |
| 4 | deux animations de palette (0x1324, scripts 0x1c546 et 0x1c54a), défilement - 0x100, puis tombe dans l'étape 8 |
| 8 | dispatch SOBJ, SET, sprites, décompte de `A5+0x38` ; à zéro, un objet SET de type 0xc, étape 12, décompte 0x20 |
| 12 | idem ; à zéro, fondu de fin (0x1cf3c), troisième animation de palette (0x1c54e), puis tombe dans l'étape 16 |
| 16 | étape ← 0, `0x1de7c`, `pool_init_sobj`, `pool_init_set`, puis sortie |
| sortie (aussi directe en mode « continue », `A5+0x25` ≠ 0) | phase += 4, partie active, rang chargé (0x1f1b8), scène (`A5+0x34`) ← 1, score et temps remis à zéro, tâche d'interface des joueurs (slot 0x30, entrée 0x150ac) |

### Phase 12 (le jeu)

Par image, dans l'ordre :

1. compteur d'images de partie `A5+0x54` (mot, plafonné à 0xffff) ;
2. compte à rebours BCD sur trois octets (`A5+0x38` minutes, `+0x39` secondes, `+0x3a` images 0-59),
   décrémenté tant que le mot `A5+0x38` est non nul (cascade `sbcd` : l'indicateur X sert de retenue
   et vient de l'instruction précédente de l'original, il est donc lu dans `SR`) ;
3. `0x2aec` ; `0x1c86e` (mise à jour du monde, §3) ; jauge 0x540000 + sprites (`0x1de96`) ;
   jauge 0x480002 + collisions (`0x260ee`) ; jauge 0x4f001e + `0x19e20` ; `0xf2e5c` ; rang
   (`rank_tick`, 0x1f27c) ; `0x28d8` ;
4. saut de niveau de débogage, actif seulement si le bit 15 du mot ROM 0x0000c0 est levé :
   boutons 0x90 → scène suivante, 0x60 → scène précédente, retour à la phase 8. Non exercé (le
   mot ROM est positif dans le jeu de production).

### Phase 20 (0x1c80c)

Fin de partie : tue la tâche 0x30, fondu, vide tout, puis `jmp 0xae4` avec A0 = 0x133fe (remplace la
tâche courante par la machine à modes). Ce saut ne revient jamais : le mode contrôle de la validation
(qui attend un `rts`) ne peut pas le vérifier, la phase est laissée en 68000.

## 3. Mise à jour du monde (0x1c86e)

`game::WORLD_UPDATE`. Chaque étape est soit la jauge de charge (`0xb3e`, D1 = marqueur
0x500001 joueurs, 0x450002 ennemis, 0x530003 décors), soit une routine de joueurs, soit le dispatch
d'une classe. Quatre ordres, choisis par `A5+0x44` (cinématique) et le numéro de scène `A5+0x34`
(comparaisons signées sur un octet) :

| Condition (dans l'ordre) | Ordre |
|---|---|
| `A5+0x44` ≠ 0 | cinématique : EFFCT en premier, puis l'ordre normal |
| scène ≥ 0x53 | normal |
| scène ≥ 0x45 | réduit : EFFCT, SET, caméras, PEFF, SOBJ |
| scène = 8 | normal, EFFCT après les caméras |
| sinon | normal |

Ordre normal :

```
jauge 0x500001, joueurs (0x35292), sous-objets des joueurs (0x392e4)
PSHL, PEFF
jauge 0x450002
BOSS, ENEMY, ESHL, EFFCT
jauge 0x530003
SET, BOX, ITEM
caméras (0x33e20)
SOBJ
```

Les ordres sont des tableaux de `Step` dans `game.rs` ; le choix et la durée propre (instructions de
choix et appels, comptées par `Chemin`) viennent du même code.

## 4. Files de candidats aux collisions

Remises à vide à chaque image (0x1c4c4) : pointeur d'écriture ← début du tableau, nombre ← 0, dans
l'ordre PSHL (0x7e7c/0x7e86), ESHL (0x7e7e/0x7eaa), ENEMY et BOSS (0x7e80/0x7ef6), BOX
(0x7e82/0x7f28), ITEM (0x7e84/0x7f52). Elles sont remplies par `obj_on_screen` pendant le dispatch
et lues par `0x260ee`. Détail : `docs/objets.md` §3.

## 5. Les dix boucles de dispatch (W0)

`dispatch.rs`. Chaque boucle (0x398e2 … 0x39f82) est la même : `A0` parcourt les emplacements du pool
(taille 0x50 à 0xC0), `D7` compte (`dbf`) ; si l'octet `+0x00` est non nul, `jsr (table + 4 * KIND)`
(table de longs rangée dans le code après la boucle, type = octet `+0x02`).

| Classe | Pool | Entrée | Table | Emplacements |
|---|---|---|---|---|
| PSHL | 0x10de | 0x398e2 | 0x39908 | 18 × 0x60 |
| PEFF | 0x179e | 0x3996c | 0x39992 | 48 × 0x60 |
| ENEMY | 0x299e | 0x39a52 | 0x39a78 | 22 × 0xC0 |
| BOSS | 0x3a1e | 0x39aec | 0x39b12 | 3 × 0xC0 |
| ESHL | 0x3c5e | 0x39b70 | 0x39b96 | 38 × 0x60 |
| EFFCT | 0x549e | 0x39cba | 0x39ce0 | 38 × 0x60 |
| BOX | 0x6ede | 0x39e50 | 0x39e76 | 21 × 0x80 |
| SET | 0x4a9e | 0x39e86 | 0x39eac | 32 × 0x50 |
| ITEM | 0x62de | 0x39f34 | 0x39f5a | 32 × 0x60 |
| SOBJ | 0x795e | 0x39f82 | 0x39fa8 | 16 × 0x50 |

(Adresses de pool relatives à A5.) Particularités fidèlement reproduites :

- le gestionnaire est appelé avec `A0` = objet, `A1` = adresse du gestionnaire, `D0.w` = type × 4 ;
  la boucle relit `A0` et `D7` au retour (un gestionnaire pourrait les modifier) ;
- l'état de l'emplacement est examiné à l'instant où la boucle y arrive : un gestionnaire qui
  crée un objet dans un emplacement suivant le verra traité dans la même image ;
- durée : parcours + `jsr (A1)` comptés par la boucle (`c.cycles`), gestionnaires par `call`.

**Remplacer un gestionnaire** : écrire `fn(&mut Cpu, &mut Cps2Bus, Obj)` qui reproduit le gestionnaire
(mémoire identique, durée ajoutée à `c.cycles`, `rts` compris dans la durée) et l'inscrire dans
`dispatch::RUST_HANDLERS` avec le code de classe et le type. Les registres ne sont plus à respecter
pour la boucle, mais le gestionnaire doit les laisser dans l'état attendu par ses autres appelants
éventuels (aucun connu : seules ces boucles appellent les tables).

## 6. Découpage et contraintes de réécriture

- Le Rust pilote : `game::update_world` et `Class::update` sont des fonctions Rust ordinaires, sans
  contrat de registres ; seuls les gestionnaires restent du 68000 (`call`, objet en `A0`). Les
  natives (`world_update`, `dispatch_*`, phases) ne servent plus que d'adaptateurs pour les
  appelants 68000 qui subsistent (autres phases, scènes).
- Aucune durée n'est calculée : le 68000 est surcadencé, seul l'état du jeu à chaque image compte.
- Les routines ne cèdent pas pendant leur exécution (aucun `0xa40` dans les gestionnaires).
- Deux détails de registres subsistent : la retenue X de `sbcd` (horloge de la phase 12) vient de
  `SR`, et l'allocation SET renvoie sa réussite dans la retenue.
- **Pistes** : la boucle principale (`0x1c45a`) et la phase 20 peuvent devenir une machine à états
  Rust, la tâche étant « une image = un appel » ; la phase 20 ne revient pas (`jmp 0xae4`, remplace
  la tâche par la machine à modes) et demande donc de réécrire le planificateur ou la machine
  à modes (`0x133fe`) en même temps.

## 7. Couverture par les scénarios

Exercés (30 000 images, 8 scénarios) : les dix boucles de dispatch (dont six seulement par
l'appel direct de `world_update`), `world_update` (ordre normal), phase 0 (étapes 0 à 16 ; le mode « continue » peut ne pas l'être), phase 12
(hors saut de niveau de débogage), `game_frame_end`.

Non exercés : les ordres cinématique, réduit (scènes ≥ 0x45) et scène 8 de `world_update`
(`ORDER_CINEMATIC`, `ORDER_REDUCED`, `ORDER_SCENE_8`, relus dans l'assembleur mais non vérifiés au
cycle près), le saut de niveau de débogage de la phase 12, la phase 20 (laissée en 68000) et la
tâche principale (`0x1c45a-0x1c4b2`, qui cède).

## 8. Pile et registres fidèles

- `Class::update` reproduit les registres de la boucle d'origine (A0, D7.w, D0 = type × 4, A1, drapeaux
  du `lsl`) : les gestionnaires s'en servent d'un objet au suivant.
- Certains gestionnaires quittent en dépilant les adresses de retour de leurs appelants (bulle de joueur
  de personnage 1 : `movem (SP)+,A1-A4 ; rts` revient à `0x1c468`, début de la boucle de partie, en
  abandonnant la fin de l'image). Pour que la pile Rust ressemble à celle de l'original, `update` et la
  phase 12 réservent la place des adresses de retour de la boucle et de `0x1c86e`, et `lib::call` lève
  `Unwound` quand le code 68000 dépile au-delà de son retour (hors exception : le `trap` change de
  pile) ; `lib::exec` abandonne alors les fonctions Rust et reprend au PC atteint.
