# Code des personnages (P5) : combattant et clerc

Code : `port/ddsom-game/src/player/characters/` (`common.rs`, `items.rs`, `fighter.rs`, `cleric.rs`). Les adresses sont celles du
programme 68000 d'origine. Les noms marqués (h) sont des hypothèses tirées de l'usage. Le noyau des joueurs (état, marche,
boutons) est décrit dans `docs/joueur.md` ; ce document décrit le code propre à chaque personnage.

## 1. Organisation

Chaque personnage a cinq actions (`characters::Slot`) choisies par l'octet `+6` (action) et aiguillées par
`characters::run` d'après `Player::CHARACTER` : saut (0x0a), attaque du bouton 1 (0x0c), attaque spéciale du bouton 4 (0x0e),
action 0x10, action 0x12. Les deux variantes d'un personnage (`CHARACTER` pair et impair) partagent le même code : chaque table
d'origine contient deux fois la même adresse.

| classe | `CHARACTER` | saut | attaque | spécial | action 0x10 | action 0x12 |
|---|---|---|---|---|---|---|
| combattant | 0, 1 | 0x3e578 | 0x3e7e4 | 0x3f0ae | 0x3f9bc | 0x35a78 (commune aux classes 0 à 2) |
| clerc | 2, 3 | 0x3fd08 | 0x3ff2a | 0x406d4 | 0x41f22 | 0x35a78 |

Les actions réécrites sont des `Native` déclarées à l'adresse d'origine (macro `action_native!` de `characters.rs`) : le noyau les
appelle par `Ctx::rom`, et la validation différentielle (`DDSOM_CHECK=1`, `examples/fuzz_player`) peut comparer chacune à l'original.
L'action 0x12 (0x35a78) fait partie du noyau commun (hors périmètre) et reste en ROM.

| module | contenu |
|---|---|
| `common` | outils partagés par les six personnages : champs de l'enregistrement joueur (`field`), routines d'origine appelées (`rom`), saut (`JumpKit`), glissade (`dash`), prolongement d'un coup (`follow_up`), coups communs (`chain_rank`, `combo_follow`, `single_strike`, `back_hop`, `pose`, `charge_slide`, `run_strike_slide`), début et fin de sort, poussières, étincelles, soins |
| `items` | objets lancés et magies partagés par le combattant et le clerc (foudre, mur, magies 13..20, météores) |
| `fighter` | combattant : saut, 15 coups, 17 objets et magies, glissade |
| `cleric` | clerc : saut, 15 coups (dont le renvoi des morts-vivants), objets, dix prières |

## 2. Modèle d'une action

Une action est un **automate** dont le pas est `Player::STEP` (`+7`, toujours pair). Le programme d'origine l'implémente par une
table de décalages indexée par le pas (`move.b (7,A0),D0b ; jsr (table,PC,D0w)`) ; ici c'est un `match` sur le pas. Un pas inconnu ne
fait rien (l'original sauterait dans des données). Les conventions communes :

- `animation::start` (0x2c16) décale le numéro d'animation de 0x40 sans arme en main ; `animation::start_plain` (0x2c88) ne décale pas.
  Les attaques utilisent presque toujours `start_plain`, les sauts et les gestes de déplacement `start`.
- `animation::step` renvoie vrai quand le script d'animation est terminé (retenue de l'original).
- Le script d'animation signale un événement dans l'octet `+0x33` (`field::EVENT`) quand il atteint une image clé : 1 coup
  porté, 2 second geste. L'action le lit et l'efface.
- L'action se termine par `actions::to_stand` (0x35614) : état immobile.
- Les routines d'origine qui créent tirs, effets joueur (pool PEFF), sons et palettes sont appelées avec `common::call` (valeurs de
  registres de données) ou `Ctx::rom` ; leurs adresses sont des constantes nommées dans les modules `rom`.

### 2.1 Octets de l'enregistrement joueur (`common::field`)

| offset | nom | rôle |
|---|---|---|
| +0x33 | `EVENT` | événement posé par le script d'animation |
| +0x46 | `PENDING_EFFECT` | effet joueur à créer à la fin de l'image (variante, 0 : rien) ; posé par la résolution des coups |
| +0x18f / 0x18e | `MOVE_STATE`, `MOVE_KIND` (h) | mouvement porté : la boîte d'attaque reste active ; genre du mouvement (1 charge ou saut, 2 tourbillon) |
| +0x6b | `AIR_ATTACK_DONE` | un coup a déjà été donné pendant ce saut |
| +0x73 | `AIR_STEERED` | le joueur a changé de direction en l'air |
| +0x80 | `TAKEOFF_X` | abscisse au décollage (mot) |
| +0x70 | `HITS` | coups de l'attaque qui ont porté (bit k : coup du rang k), écrit par `combat` |
| +0x72, 0x74, 0x75 | `COMBO_RANK`, `COMBO_WINDOW`, `COMBO_RELEASED` | rang dans l'enchaînement ; fenêtre de 6 images pendant laquelle le bouton 1 doit rester appuyé ; bouton relâché |
| +0x140, 0x148 | `ITEM`, `ITEM_CHARGES` | ligne 2 de la table d'équipement (identifiant de l'objet, 0x56 : aucun) et ses charges |
| +0x15e | `SPECIAL_KIND` | numéro de l'attaque spéciale (animation de lancement tirée de 0x372e4 d'après l'identifiant de la ligne choisie) |
| +0x15f | `ITEM_NEW` | équipement à enfiler (échange) |
| +0x168 | `TIMER` | compteur de l'action (mot) |
| +0x16a | `SPELL_FROZEN` | figé par le sort d'un autre joueur |
| +0x189 | `SPELL_CANCELLED` | le sort est annulé (protection) : seule une étincelle est jouée |
| +0x1ea | `SPELL_TARGET` | adresse de l'objet visé par le sort (mot) |
| +0x1ee | `AREA` | le sort touche un allié (1) ou le lanceur seul (0) |

## 3. Outils communs (`common`)

- **Saut** (`jump`, `JumpKit`) : cinq pas : 0 accroupissement, 2 montée (jusqu'à `VZ` ≤ 0x180), 4 sommet, 6 descente, 8
  récupération. Le type de saut (`+0x67`, posé par `checks::jump_button` : 0 sur place, 1 en avant, 2 après une course, 3 vertical)
  choisit les animations (`by_jump_kind`) et la ligne de la table de vitesses de décollage (quatre mots vx, ax, vz, az ; table du
  personnage : 0x1044fe combattant, 0x10451e clerc ; le long (vx, ax) est nié d'un bloc quand le personnage regarde à gauche).
  `air_steer` retourne le personnage en l'air (une fois) ; `drift` avance en X sauf saut sur place ; le bouton 1 en l'air donne un coup
  propre au personnage (`air_attack`, une fois par saut). Le kit fixe aussi un crochet de fin de récupération et un crochet de fin
  d'image (le combattant y crée l'effet joueur en attente et gère l'arme maudite).
- **Glissade** (`dash`, action 0x10, identique pour le combattant et le clerc) : élan de 11 pixels par image, bond bref (8 images au
  plus tant que le bouton de saut est tenu) puis freinage avec poussière.
- **Prolongement d'un coup** (`follow_up`, 0x3f036) : au sol, la touche 4 avec l'objet 0x2d en ligne 2, ou un retournement avec le
  bouton 1 appuyé, déclenche l'attaque en demi-tour (action 0x16) si l'équipement le permet (`turn_attack`, 0x3f082).
- **Enchaînement** : `chain_rank` (le coup précédent a-t-il porté ? le rang avance, modulo) et `combo_follow` (fenêtre de 6 images).
- **Sorts** : `begin_spell` / `freeze_others` / `dim_screen` / `end_spell`. Un sort met le jeu en pause (`A5+0x3d`), pose un indicateur
  (`A5+0x4a`), fige les autres joueurs (`+0x16a`), assombrit l'écran (tâche d'écran 0x153e, rétablie par 0x1536) et se termine par
  `clear_spell_state` (remet à zéro `A5+0x4a, 0x3d, 0x5b` et les quatre `+0x16a`).
- **Poussières et étincelles** : `dust_stand`, `dust_run`, `dust_jump` (effet 0xb10c8 avec des vitesses lues dans des tables du code
  par classe) ; `item_spark` (étincelle d'objet : ligne de la table 0x3fbb8).
- **Soins** : `find_ally` (premier allié devant le lanceur, portée donnée), `heal_scan`, `heal_flicker`, `apply_heal`.
- **Registres hérités** : voir §7.

## 4. Combattant (`fighter.rs`)

### 4.1 Saut (0x3e578)

`common::jump` avec la table de vitesses 0x1044fe. Coup en l'air (0x3e754) : animation 9 (équipement lourd) ou 0x2a, 0xc / 0x2b après
une course, 0x32 avec l'arme 8 ou 0x18, 0xb en bas ; puis `item_curse` (arme maudite) et décompte de l'aura (`aura_hit_countdown`).
Fin de saut : `curse_check` (0x377a6) et création de l'effet joueur en attente.

### 4.2 Attaque du bouton 1 (0x3e7e4)

`Player::SUBACTION` (`+0x6a`) choisit le coup (valeurs paires, table de 15 entrées à 0x3e806). Posé par `checks::attack_button` (0, 2,
4, 6, 8, 0xe), `checks::special_attack` (0x1c) ou les reconnaisseurs de coups spéciaux (0xa, 0xc, 0x12, 0x16, 0x18...). Les coups
suivis de `follow_up` sont indiqués ; ensuite `flush_pending_effect` crée l'effet joueur de `+0x46`.

| sous-action | nom Rust | pas | contenu | follow_up |
|---|---|---|---|---|
| 0x00 | `combo` | 0, 2, 4 | coup de base ; rang 0..1 (0..3 avec l'équipement lourd) ; animation 0x26 + rang (rang seul si lourd, 0x30 avec l'arme 8/0x18) ; le pas 2 prolonge par l'animation 0x1a | oui |
| 0x02 / 0x04 | `thrust` | 0, 2 | estoc haut (animations 5, 6) / bas (7, 8), rang selon `HITS` bit 1 / 2 | oui |
| 0x06 | `turn_strike` | 0 ou 2, 4 | coup en se retournant (animations 0x10 / 0x1e) ; le pas 2 est le pas 0 des entrées par état | oui |
| 0x08 | `slow_strike` | 0, 2 | coup en marche lente (0x13 lourd, 0x29) | non |
| 0x0a | `after_jump` | 0, 2 | coup de retombée (0xd, 0x31 avec l'arme 8/0x18) | oui |
| 0x0c | `charge` | 0, 2, 4 | élan 0xc00, geste (événement), glissade avec deux effets (`SLASH_EFFECT`) | non |
| 0x0e | `run_strike` | 0, 2, 4 | élan 0x500 puis arrêt (animation 0xf) | non |
| 0x10 | `common::single_strike(0x11)` | 0, 2 | coup bas | oui |
| 0x12 | `common::back_hop` | 0, 2 | saut en arrière (animation 0x1f), sans pas d'animation pendant le vol | non |
| 0x14 | `guard_break` | 0, 2 | animation 0x12 | non |
| 0x16 | `leap` | 0..0x12 | attaque spéciale 0x2d : grand bond tournoyant (équipement lourd, pas 0..0xa) ou bond léger (pas 0xc..0x12) | non |
| 0x18 | `whirl` | 0, 2 | tourbillon : ôte 12 points de vie au premier contact (`+0x58` bit 0), plancher 1 | non |
| 0x1a, 0x1c | `common::pose(0x1a)` | 0, 2 | geste (déclenché par `special_attack` sans objet) | oui |

Avec l'arme 8 ou 0x18 (`holds_special_weapon`), les coups 2, 4, 6, 8 et 0x14 deviennent le coup de base (`fall_back_to_combo`).

### 4.3 Attaque spéciale (0x3f0ae)

`field::SPECIAL_KIND` (0..20) : le combattant n'a que des objets. `A5+0x5b` reçoit le numéro pendant l'action.

| n° | action | détail |
|---|---|---|
| 0, 1 | objet lancé (animation 0x22) | tir créé par 0x51d02 (0) ou 0x51d54 (1) |
| 2, 4, 5 | objet lancé (animation 0x23) | 0x51f6a, 0x51fb6, 0x52002 |
| 6, 7 | objet lancé (0x21) | 0x52204, 0x5308c |
| 8 | objet lancé (0x20) | 0x52394 |
| 3, 11 | rien | |
| 9 | foudre (`items::lightning`) | cible choisie au geste (0x4f004), éclair 0x4cdd0, 192 images |
| 10 | mur (`items::wall`) | sans assombrissement ; cible 0x50324 |
| 12 | échange d'équipement | l'objet ramassé (`+0x15f` − 0x2e) remplace l'arme ou le bouclier ; l'ancien équipement passe dans la ligne 2 ; `init_weapon_stats` |
| 13..18 | magies lancées (`items::cast`) | 0x28/0x3c/0x78/0x28/0x28/0x78 images ; effets 0x4dcda, 0x54522, 0x52b54..., 0x4e2f4 (aura rouge), 0x4b196, 0x4ec66 |
| 19 | potion de soin | `heal_scan` + `heal_flicker` + `apply_heal` (table 0x416cc) |
| 20 | météores (`items::meteor`) | 176 images |

Un objet lancé consomme une charge (`consume_charge`) seulement si la routine de lancement a créé quelque chose (Z levé au retour
sinon). Les magies consomment une charge à la fin du décompte.

## 5. Clerc (`cleric.rs`)

### 5.1 Saut (0x3fd08)

`common::jump` avec la table 0x10451e et un coup en l'air plus simple (animations 9, 0xc après une course, 0xb en bas), sans arme
maudite ni effet en attente.

### 5.2 Attaque du bouton 1 (0x3ff2a)

Même table de sous-actions que le combattant (voir §4.2). Différences : jusqu'à cinq coups enchaînés (animation = rang 0..4, geste de
prolongement 0x17) ; aucune retombée sur le coup de base avec l'arme 8/0x18 ; pas d'effet joueur en attente ni d'`item_curse`.
Le coup de retombée 0x0a a un geste propre pour la masse d'armes (arme 0x1e : animation 0x18). La sous-action 0x16 est un bond
tournoyant (animation 0x15, vitesse 0x80, vz 0x560) ou, avec la masse d'armes, cinq coups sur place. La sous-action 0x18 est le
**renvoi des morts-vivants** :

1. pas 0 : nombre de renvois par niveau lu dans la table 0x4054e (deux octets par niveau : monstres de type 1 et de type 5 ; 0xff
   avec le talisman `+0x165` = 3), animation 0x21 ;
2. pas 2 : à l'événement, `ACTIVE` passe à 2, les entrées sont figées, l'écran clignote (`A5+0x4ba` = 0x100 une fois sur deux, trois
   fois), effets 0x4b518, 0x3b0c, 0x1ac86, 0x51bb6, 0xe27da ;
3. pas 4 : clignotement ; à la fin `scare_undead` marque (`+0x8f`) les 23 premiers monstres : 1 s'il reste un renvoi, sinon 2
   (3 à partir du niveau 10) ; pas 6 : retour à l'état immobile.

### 5.3 Attaque spéciale (0x406d4)

`SPECIAL_KIND` : 6, 7, 8 (objets lancés, animations 0x1e et 0x1d), 9, 10 et 13..20 (communs, `items`, animations 0x1f et 0x20,
étincelle de genre 1), 12 (échange d'arme simplifié), 0x24..0x2d (prières ci-dessous). Les autres numéros ramènent à l'état immobile.

| n° | nom Rust | effet (h) | gestes |
|---|---|---|---|
| 0x24 | `prayer_hold` | aura qui fige des monstres (`hold_foes` : réaction 0x9c, nombre selon le niveau, table 0x4131e) | 0x2a, 0x2b |
| 0x25 | `prayer_storm` | tempête (effet 0x4ec66) | 0x28 |
| 0x26 | `prayer_swarm` | essaim d'effets (0x4f272, nombre selon le niveau, table 0x40ece) | 0x2c, 0x2d |
| 0x27 | `prayer_beam` | rayon visé (0x4b5ac) dont la cible reçoit le drapeau `+0x5e` | 0x30, 0x31 |
| 0x28 | `prayer_banish` | renvoi de monstres (`banish_foes` : 0x4c26c sur chaque monstre non protégé, table 0x41afe) | 0x2e, 0x2f |
| 0x29 | `prayer_holy_light` | lumière sacrée : `holy_damage` blesse tous les monstres au sol (table 0x433c/0x431e par niveau, +25 % avec `+0x166` = 4 ; tue les types 0 et 0x13) | 0x32, 0x33 |
| 0x2a | `prayer_protection` | aura de protection sur tous les joueurs (durée par niveau, table 0x41e4c) | 0x26 |
| 0x2b | `prayer_blessing` | bonus de puissance (`POWERUP_TIMER`, table 0x410a6) pour un allié ou le clerc | 0x22, 0x23 |
| 0x2c, 0x2d | `prayer_heal` | soin d'un allié ou du clerc (2 × niveau) | 0x24, 0x25 |

Chaque prière retire une charge de la réserve (`+0x152..+0x157`), assombrit l'écran et se termine par `end_spell`.

## 6. Particularités de l'original reproduites

- `apply_heal` (0x41656) : pour les sorts 0x2c et 0x2d, les deux tables de soin chargées par `lea` ne sont jamais lues (le branchement saute le
  `move.w`) : le soin vaut 2 × niveau. Quand plusieurs joueurs sont soignés, le registre du montant reçoit la somme obtenue (avant
  plafond) : chaque allié suivant gagne autant de points que le précédent en avait après le soin.
- `banish_foes` (0x41aa4) : le plafond de monstres ne termine que la zone en cours ; le nombre redescend de 0 à 0xff pour la zone des
  ennemis, qui sont donc tous touchés.
- `scare_undead` (0x40648) : un emplacement de type 5 est examiné même s'il n'est pas en jeu.
- `heal_scan` et `bless_ally` s'arrêtent au premier allié ; les drapeaux `+0x78` des joueurs suivants ne sont pas remis à zéro.
- Le long (vx, ax) de `neg.l (0x14,A0)` est nié d'un bloc (`negate_horizontal_speed`).

## 7. Registres hérités (piège pour la réécriture)

Certaines routines d'origine appelées par les actions lisent des registres que l'action n'a pas posés : ils viennent du code
précédent. Les reproduire est nécessaire pour l'état exact :

- `0x4f004` (cible de la foudre) et `0x4dcda` (magie 13) écrivent `D0.b` dans l'effet créé. `D0.b` vaut le résultat du pas d'animation
  d'origine (0x2d58 : drapeaux de l'entrée quittée si l'animation avance, sinon valeur précédente) ; la valeur précédente est le pas
  de l'action pour les automates à table, deux fois `SPECIAL_KIND` pour les magies 13..20. `common::step_keeping_regs` fait ce pas
  d'animation par la routine d'origine (qui émule les registres).
- `0x4cdd0` (éclair de la foudre) crée l'effet à la position de `A2` (la cible) : `items::lightning` pose `A2`.
- `0x4c26c` avec `D0` = 1 range `A2` (le monstre) dans l'effet : `banish_foes` pose `A2`.
- `0x19dce` (barre de vie) lit `D0..D3`.

Les sons et les routines de palette (0x3af8...) ne lisent aucun registre.

## 8. Validation

- `tools/validate_all.sh 30000` : tous les scénarios sont VALIDE (`bot_char0`..`bot_char5`, `bot_2p`, `bot30k`, les scénarios
  `extra` dont `mash_c0`, `mash_c0b`, `mash_c2`, `mash_c2b` ajoutés pour ce travail).
- `examples/fuzz_player <fighter|cleric> <roms> <eeprom> <scénario> <chauffe> <tours>` : après une chauffe sur un scénario, chaque action
  est appelée en mode contrôle (`ddsom_game::set_check`) sur des états aléatoires (pas, sous-action, entrées, événement, armes, position,
  vitesses, réserve, autres joueurs, monstres de types variés) et comparée à l'original sur une copie. Variables : `DDSOM_ENTREE`
  (une seule entrée), `DDSOM_SEED`, `DDSOM_TRACE` (affiche chaque état tiré), `DDSOM_PROBE` (soin d'origine par niveau). Résultat : 6 000
  tours pour chaque classe sans écart. Il a révélé les registres hérités du §7, la quirk du soin et le pas 0xc du bond léger.
- Couverture : `examples/player_coverage` compte les appels de chaque fonction (les nouveaux modules y sont déclarés dans
  `player/cov.rs`).

## 9. Reste en ROM

- Les créateurs de tirs et d'effets joueur (pools PSHL et PEFF) appelés par ces actions, et leurs gestionnaires (C06, C12) : 0x4a99a, 0x4c58e,
  0x4fbde, 0x4a860, 0x4fd2c, 0x4dcda, 0x4ab48, 0x4ab64, 0x4e6a8, 0x4b5ac, 0x4c19a, 0x4c26c, 0x4aeb2, 0x4f272, 0x4f004, 0x50324, 0x4cdd0 ; tirs
  d'objets lancés 0x51d02..0x5308c, 0x54522 ; effets d'écran 0x1ab3c, 0x1ab56, 0x1abe0, 0x1ac86, 0x3b0c, 0x3b14, 0x3af8, 0x3afe,
  0x37882.
- Les sons (voix des armes 0xe7e10, 0xe7622, wrappers 0xe1xxx..0xe9xxx).
- L'action 0x12 (0x35a78), commune aux classes 0 à 2.

## 10. Ce que les autres personnages réutilisent

Les actions de l'elfe (saut 0x42012, attaque 0x4241c, spécial 0x42b0c, action 0x10 0x444ca), du nain (0x445ba, 0x4491a, 0x45148,
0x45988), du magicien (0x45a78, 0x45cfc, 0x4619e, 0x4879a) et du voleur (0x488bc, 0x48d48, 0x49498, 0x49d46) appellent les mêmes
sous-routines que celles qui sont passées dans `common` : prolongement d'un coup (0x3f036), orientation (0x3f8d2), gel des autres
joueurs (0x3f8f0), poussières (0x3fa7c, 0x3fac4, 0x3fb1a), étincelle d'objet (0x3fb8c), fin de sort (0x3f97c). Leur saut a le même
squelette que `common::jump` (il faut une autre `JumpKit`) ; leur action 0x10 est vraisemblablement la glissade `common::dash` pour
l'elfe. Pour brancher une nouvelle classe : écrire un module avec une `JumpKit`, une fonction par action, des `action_native!` aux adresses
d'origine, les ajouter à `player::NATIVES` et à `cov::SOURCES`, puis lancer `fuzz_player` en adaptant `max_step` aux tables de pas de la
classe (l'argument `personnage` filtre les natives par préfixe de nom). Les `field` de `common` sont nommés d'après les deux classes
déjà réécrites ; les noms (h) sont à confirmer.
