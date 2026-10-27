# Code des personnages (P5) : les six classes

Code : `port/ddsom-game/src/player/characters/` (`common.rs`, `items.rs`, `foes.rs` et un module par classe). Les adresses sont celles du
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
| elfe | 4, 5 | 0x42012 | 0x4241c | 0x42b0c | 0x444ca | 0x35a78 |
| nain | 6, 7 | 0x445ba | 0x4491a | 0x45148 | 0x45988 | 0x35b18 |
| magicien | 8, 9 | 0x45a78 | 0x45cfc | 0x4619e | 0x4879a | 0x35b96 |
| voleur | 10, 11 | 0x488bc | 0x48d48 | 0x49498 | 0x49d46 | 0x35b50 |

Les actions réécrites sont des `Native` déclarées à l'adresse d'origine (macro `action_native!` de `characters.rs`) : le noyau les
appelle par `Ctx::rom`, et la validation différentielle (`DDSOM_CHECK=1`, `examples/fuzz_player`) peut comparer chacune à l'original.
L'action 0x12 (0x35a78, classes 0 à 2, et 0x35b96 du magicien) fait partie du noyau commun (hors périmètre) et reste en ROM ;
celles du nain et du voleur (roulades de dix lignes) sont réécrites (`dwarf::roll`, `thief::roll`).

| module | contenu |
|---|---|
| `common` | outils partagés par les six personnages : champs de l'enregistrement joueur (`field`), routines d'origine appelées (`rom`), saut (`JumpKit`), glissade (`dash`), prolongement d'un coup (`follow_up`), coups communs (`chain_rank`, `combo_follow`, `single_strike`, `back_hop`, `pose`, `charge_slide`, `run_strike_slide`), début et fin de sort, poussières, étincelles, soins |
| `items` | objets lancés et magies partagés par le combattant et le clerc (foudre, mur, magies 13..20, météores) |
| `spawn` (dans `player/`) | créateurs d'effets joueur (pool PEFF) et de tirs joueur (pool PSHL) : voir §8 quater |
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

## 8 bis. Elfe et magicien (`elf.rs`, `magic_user.rs`)

Adresses : elfe (`CHARACTER` 4, 5) saut 0x42012, attaque 0x4241c, spécial 0x42b0c, action 0x10 0x444ca ; magicien (8, 9) saut
0x45a78, attaque 0x45cfc, spécial 0x4619e, action 0x10 0x4879a. Natives `elf_*` et `mage_*`. Scénarios : `bot_char5` (elfe),
`bot_char1` (magicien), `extra/mash_c5*`, `extra/mash_c1*`.

### Compléments au module commun

- `JumpKit` s'enrichit de `steer_push` (poussée en l'air : 0x30, 0x20 pour l'elfe), `takeoff`, `busy` (un geste en l'air suspend
  montée, sommet et descente), `apex_gravity` (le magicien change `AZ` au sommet) et `extra_step` (pas 0xa et 0xc des gestes en l'air :
  coup, tir) ; `JumpKit::PLAIN` donne les valeurs par défaut. `common::land` (atterrissage), `air_steer(push)`.
- `foes.rs` : les boucles « sort qui blesse les monstres » que le programme d'origine recopie huit fois (0x439fc, 0x44188, 0x44384,
  0x47104, 0x47390, 0x474e4, 0x47770, 0x48012, 0x48392) sont une seule fonction `strike_foes` décrite par `Strike` (table de
  dégâts par niveau ou valeur fixe, bit de protection de `+0x94`, réaction `+0x49`, majorations `Boost` dans l'ordre : chef de type
  donné +50 %, arme `WEAPON_B` donnée, accessoire `+0x166` = 4 : +25 %, `+0x165` : +5). Les routines 0x474c4 (dégâts de la foudre,
  `items::lightning_damage`) et 0x4414c (dégâts des météores, `items::meteor_damage`) que le combattant et le clerc appelaient en
  ROM sont donc réécrites (leurs noms `LIGHTNING_END_SOUND`, `METEOR_SOUND` étaient des erreurs).
- `items` : `Style` porte maintenant le champ de la cible de la foudre/mur (`+0x1ea` combattant/clerc, `+0x1ec` elfe, `+0x1e8`
  magicien), l'instant des dégâts de la foudre (0x60, 1 pour l'elfe), la pose finale des magies (elfe 0x25, magicien 0x13 : la
  magie se termine par un pas d'animation au lieu de finir avec le décompte), `d0_is_step` (D0.b hérité par la magie 13 : le pas
  pour l'elfe et le magicien) et le mode de rétablissement de l'écran. `Cast` est public avec un crochet `on_end` (l'aura rouge de
  la magie 16 est posée au geste chez le combattant, à la fin du décompte chez l'elfe — durée 0x708 — et le magicien — 0x8000, décompte
  de 0x1e images).
- `common` : `begin_cast`/`start_cast` (début de sort), `countdown`, `take_event`, `wind_down`, `cancel_cast`, `swap_weapon`
  (échange d'équipement simple), `flash_screen`/`blink_screen`/`intensity_keys` (sorts de gel), `combo_window`, `weapon_voice_b`,
  `end_spell_mode`/`undim_screen_mode`.

### Particularités de l'original reproduites

- Le code du magicien rétablit l'écran avec le mode 1 (0x48772 recopie 0x44474 en oubliant le mode 2 de 0x444a2).
- Les sorts annulés (`SPELL_CANCELLED`) sautent parfois à un pas de sortie : selon les sorts le drapeau `+0x189` est remis à zéro ou
  non à la fin ; chaque site suit l'original (`end_spell` contre `end_spell_keep_cancel`).
- Le magicien n'initialise pas le clignotement `+0x1ed` pour la potion (n° 19) contrairement aux autres.
- Table de vitesses de saut du magicien : 0x10459e (la table 0x10455e du listing est celle d'une routine jamais atteinte).
- Le sort 0x24 du magicien (« sacrifice ») met à un point de vie tous les joueurs en vie, lanceur compris.
- La magie 0x1c de l'elfe a deux variantes selon le regard (pas 0..6 ou 8..0xc) ; celle du magicien aussi (0..6 ou 8..0xe).
- Armes du magicien : baguettes 0x24..0x27 en `WEAPON_B` ; au tir, une baguette épuisée redevient un bâton 0x22 (`rod_shot`).

## 8 ter. Nain et voleur (`dwarf.rs`, `thief.rs`)

Adresses : nain (`CHARACTER` 6, 7) saut 0x445ba, attaque 0x4491a, spécial 0x45148, action 0x10 0x45988, action 0x12 0x35b18 ; voleur
(10, 11) saut 0x488bc, attaque 0x48d48, spécial 0x49498, action 0x10 0x49d46, action 0x12 0x35b50. Natives `dwarf_*` et `thief_*`.
Scénarios : `bot_char4` (nain), `bot_char3` (voleur), `extra/mash_c4*`, `extra/mash_c3c`, `mash_c3d`.

### Nain

- **Action 0x10** : la glissade commune (`common::dash`) ; **spécial** : mêmes numéros que le combattant (`items`, animations 0x1c/0x1d/0x1b/0x1a
  pour les objets lancés, 0x1e foudre et mur, 0x1f magies, étincelle de genre 3), échange d'arme simple (`common::swap_weapon`).
- **Saut** (ne se met pas dans une `JumpKit`) : le profil du saut (`+0x1e8`, ligne de la table de vitesses 0x10455e) vaut le type de saut
  `+0x67` (0 sur place, 2 après une course, 3 vertical) ; un saut en avant prend le profil 4 si gauche ou droite est tenu, 6 sinon. Les profils
  0..3 volent en un seul pas (2) jusqu'à `Z` < 0 ; les profils 4 et 6 passent à la ligne suivante (5, 7) et au pas 4 quand `VZ` devient
  négative. Le coup en l'air (bouton 1) joue les animations 9, 0xc ou 0xb (bas). Avec la touche du bas et le bouton de saut, dans les profils
  0, 3, 6, 7 : **plongée** (pas 8, `+0x6b` = 2, vitesse verticale −0x300, 0x600 en avant si une direction est tenue), rebond à l'impact (pas
  0xa, secousse d'écran `0x2824`), retombée (0xc).
- **Attaque** : mêmes sous-actions que le combattant avec leurs propres gestes. Coup de base à cinq coups (animation = rang) ; les pas 4 et 6 de
  la sous-action 0 sont une fente (vitesse 0x400 au geste porté) partagée avec la sous-action 0x1c. Le **saut périlleux** (0x16) retourne le
  nain en l'air (l'animation y avance de deux pas par image tant qu'elle n'est pas finie), le **tourbillon plongeant** (0x18) coûte 12 points de
  vie au premier contact, la **longue glissade** (0x1a) : vitesse posée par le long 0x400000, déplacement pendant les 20 premières des 50 images, puis pause de 16 images.

### Voleur

- **Action 0x10** : glissade proche de `common::dash` (compteur 8 au lieu de 0xc, pas de poussière en fin de glissade) ; table de vitesses de
  saut 0x1045be.
- **Saut** : le voleur est limité à 3 pixels par image horizontalement (`limit_speed`) ; en l'air il peut **sauter de nouveau** (bouton de saut
  appuyé, une fois par saut, `+0x1ef`) ou **s'agripper au bord de l'écran** (`+0x5a` posé par `walk::clamp_to_screen`, bouton de saut maintenu :
  pas 0xe puis saut en sens inverse). Le bouton 1 en l'air donne un coup (pas 0xa), la touche 4 avec le couteau (objet 0x2c en ligne 2) lance
  le couteau (pas 0xc, `0x54426`). Ces choix sont désactivés dans les 16 pixels qui précèdent le sol quand le voleur descend (`air_options`).
- **Attaque** (16 entrées, 0..0x1e ; ni prolongement de coup ni repli sur le coup de base) : coup de base à trois coups, estocs, charge, coup
  en courant, coup de pied sauté (0x16, qui crée l'ombre : objet 1 de genre 8 lié par `+0x50`), bond arrière avec coup de pied (0x18), **coup dans
  le dos** (0x1a : le voleur prend la place de l'ombre derrière sa cible, décalage lu dans la table 0x4935a selon le genre de l'ombre) et couteau
  lancé (0x1e). Le pas 2 du coup de pied sauté teste « événement absent » (et non présent) : le geste suivant vient donc tout de suite.
- **Spécial** : comme le combattant (animations 0x1a/0x1b/0x19/0x18 pour les objets lancés, 0x1c foudre et mur, 0x1d magies, étincelle de genre 5)
  avec le **couteau lancé** au numéro 11 (animation 0x1e) ; échange d'arme simple.

### Roulades (action 0x12)

`dwarf::roll` (0x35b18) et `thief::roll` (0x35b50) : élan de 3 (nain) ou 6 pixels par image dans le sens du regard, geste 0x1f. Le nain avance à
chaque image ; le voleur seulement quand le script signale un événement (qui n'est pas effacé). Début commun : `common::roll_start`.

### Compléments au module commun

- `by_jump_kind` est public. `roll_start` (roulade).
- `screen_task` (assombrissement et rétablissement de l'écran) met N, Z, V, C à zéro avant `START_TASK` : le `move.w #0x10,D0w` d'origine les
  efface et le `trap` les empile sous la pile ; sans cela la validation différentielle trouvait des écarts sur cette mémoire morte.
- Le voleur n'a aucun prolongement de coup : `follow_up` n'est pas appelé.

### Particularités de l'original reproduites

- Le saut du nain écrit le profil dans `+0x1e8` avant de lire la table ; le long (vx, ax) est nié d'un bloc comme pour les autres classes.
- La fente (pas 4 du coup de base et de la pose 0x1c du nain) ne remet pas l'événement à zéro.
- La table des sous-actions du voleur a seize entrées (0x1e est le couteau, à la place d'une sous-action de prolongement).
- Le saut du voleur ne passe par `curse_check` qu'à la fin du pas 8 (récupération) ; celui du nain à la fin du pas 6, pas à celle de la plongée.
- Validation : 3 000 tours de `fuzz_player` pour le nain et pour le voleur sans écart. Le fuzz met les drapeaux X, N, Z, V, C du processeur à zéro
  avant chaque appel : l'original empile le CCR sous la pile (`trap` du lancement des tâches d'écran) et le X dépend de l'arithmétique précédente
  (écart sans conséquence que l'ancien fuzz de l'elfe et du magicien rencontre aussi : 60 écarts sur 3 000 tours, tous `ram[ff0e56]`).
- Non exercé par les scénarios (seulement par `fuzz_player`) : nain `dive`, `bounce`, `bounce_end`, `guard_break`, `long_dash` ; voleur
  `air_knife`.

## 8 quater. Créateurs d'effets et de tirs (`player/spawn.rs`)

Au geste porté, une action crée un objet lié au joueur (coup d'épée, souffle, ombre, couteau...) : il naît dans le pool PEFF (effets, 48
emplacements) ou PSHL (tirs, 18 emplacements) avec son genre (`Obj::KIND`, qui choisit le **gestionnaire** en ROM chargé de le faire vivre),
quelques paramètres propres au genre et le lien `+0x50` vers le joueur. Les 38 créateurs ci-dessous (un par adresse d'origine) sont réécrits : une fonction Rust par
créateur, appelée par les actions, et une native à l'adresse d'origine (`spawn_*`) pour les appelants restés en ROM (0x35a90, 0x36218,
0x23aaa, 0x24f90, 0x254e6, 0x25786...).

| adresse | fonction | genre | contenu |
|---|---|---|---|
| 0x4a99a | `slash` | 3 | trace d'un coup : `Some(n)` (variante 0, paramètre `n`, rien avec les armes 0x10 à 0x13) ou `None` (variante 1) |
| 0x4a860 | `blood` | 2 | sang du tourbillon du combattant |
| 0x4ab48, 0x4ab64 | `cure`, `potion` | 4 | soin d'un allié (paramètre = prière) et de la potion (variante 2) |
| 0x4aeb2 | `holy_light` | 5 | lumière sacrée |
| 0x4b5ac | `beam` | 9 | rayon : deux effets (paramètres 0 et 1), ou un seul dirigé |
| 0x4c19a | `blessing` | 0xd | bénédiction (variante 0, 1 ou 2) |
| 0x4c26c | `banish` | 0xe | renvoi : un effet, ou quatre (paramètres 3 à 0) qui suivent le monstre visé |
| 0x4c58e | `weapon_trail` | 0xf | traînée de l'arme (armes 0x10 à 0x13) ; l'effet en attente de `flush_pending_effect` |
| 0x4cdd0 | `lightning` | 0x13 | éclair, à la position de la cible, 0x40 au-dessus |
| 0x4dcda | `shower` | 0x19 | averse (magie 13, elfe, magicien) |
| 0x4e6a8 | `aura` | 0x1d | aura de prière |
| 0x4f004 | `lightning_target` | 0x20 | cible de la foudre ; D0 = 1 si créé, 0 sinon (drapeau Z) |
| 0x4f272 | `swarm` | 0x22 | essaim de `count` objets (prière du clerc) tant que la réserve de la scène (`A5 − 0x5fc0`) n'est pas vide |
| 0x4fbde | `leap` | 0x23 | effet du bond (rien avec les armes 0x10 à 0x12) |
| 0x4fd2c | `swap` | 0x24 | échange d'équipement |
| 0x4fe18 | `dash` | 0x25 | longue glissade du nain |
| 0x50324 | `wall_target` | 0x29 | cible du mur ; compte les murs (`A5 − 0x5fbe`) |
| 0x54426 | `knife` | PSHL 0x15 | couteau lancé (puissances 10 et 5) |
| 0x52e20 | `shadows` | PSHL 8 | six ombres (variantes 5 à 0) du coup de pied arrière du voleur |
| (en ligne) | `shadow` | PSHL 8 | l'ombre du coup de pied sauté du voleur (paramètre 2) |
| 0x4e2f4 | `ring` | 0x1b | anneau (magie 16), variante donnée |
| 0x4b196 | `beam_cast` | 6 | rayon (magie 17), variante donnée |
| 0x4ec66 | `storm` | 0x1f | tempête (magie 18 ; prière du clerc) |
| 0x5012e | `meteors` | 0x28 | météores (magie 20) |
| 0x4bb68, 0x4bbbe, 0x4bbf6, 0x4bc42 | `ice_columns`, `ice_mist`, `ice_shards`, `ice_crown` | 0xc | le gel (magie 15 ; sorts de gel de l'elfe et du magicien) : colonnes (4 effets), brume, éclats (6 effets sans lien), dernier effet ; `ice_spell` enchaîne les quatre premiers avec `ice_ground` |
| 0x52b54 | `ice_ground` | PSHL 7 | sol gelé : deux tirs (variantes 0 et 1) |
| 0x54522 | `fireball` | PSHL 0x16 | boule de feu (magie 14) |
| 0x51d02 .. 0x5308c | `thrown(Thrown::A..H)` | PSHL 1, 9, 2, 0xa, 0xb, 3, 0xc, 4 | les huit tirs d'objets lancés ; A et B (vitesse 0x700) choisissent la direction d'animation (`+0x2d` = 2 ou 6), C, D et E (vitesse 0xa00) copient l'orientation, F, G et H n'ont pas de vitesse |

Les natives conservent les registres et drapeaux que lisent les appelants restés en ROM : A1 (l'objet créé) et le résultat de l'allocation
(`0x225e`, `0x228e`, retenue levée si le pool est plein) ; les tirs lancés (`thrown`), `ice_ground`, `lightning_target`, `wall_target` et `knife` rendent aussi D0 et Z comme l'original
(dont l'étrangeté : `wall_target` renvoie « trouvé » même quand le pool est plein, car l'allocation échouée laisse Z clair). Les actions
qui lisent A1 ou D0 après l'appel (`prayer_beam`, `items::lightning`, la magie 13) le font encore : voir §7.

Le fuzz `fuzz_player spawn ...` appelle les natives sur des états de pools variés (vides, pleins, compteurs incohérents), des armes 0x10 à
0x13, une réserve de scène variable : 4 000 tours sans écart (`DDSOM_ENTREE` pour une seule native).

Les lunes de l'elfe (0x4e4de, 0x4e782) et les effets des sorts orage, pluie, mur de feu et vague du magicien (0x4d020, 0x4d660, 0x4d2c0,
0x4dfe2) sont réécrits dans `spawn.rs` (`moon`, `moon_b`, `storm_spell`, `rain_glow`, `wall_spark`, `wave_bolt` ; trous trouvés par les
bots longs, voir `docs/couverture.md`), validés par `fuzz_player spawn` (300 appels chacun, 0 écart). Restent appelés par adresse (avec
native) : étincelles 0x4dc9a, foudre 0x4b7a0, séisme 0x4b340 et 0x4f132 (`player/externes.rs`).
Les **gestionnaires** des tirs (C06) et des effets (C12) sont réécrits : `families/joueur/`, `docs/familles/joueur.md`.

## 9. Reste en ROM

- Les créateurs listés à la fin du §8 quater. Les gestionnaires des effets et des tirs joueur (pools PEFF et PSHL) sont réécrits
  (`families/joueur/`), sauf PEFF 24, 42 et 43 (ramassage avec comptes BCD, boutique) : voir `docs/familles/joueur.md`.
- Les sons (voix des armes 0xe7e10, 0xe7622, wrappers 0xe1xxx..0xe9xxx), les effets d'écran 0x1ab3c, 0x1ab56, 0x1abe0, 0x1ac86, 0x3b0c, 0x3b14,
  0x3af8, 0x3afe, 0x37882.
- L'action 0x12 (0x35a78), commune aux classes 0 à 2, et celle du magicien (0x35b96).

## 10. Pour brancher une nouvelle classe

Écrire un module avec une `JumpKit` (ou un saut propre, comme le nain et le voleur), une fonction par action, des `action_native!` aux
adresses d'origine, les ajouter à `player::NATIVES` et à `cov::SOURCES`, puis lancer `fuzz_player` en adaptant `max_step` aux tables de pas de
la classe (l'argument `personnage` filtre les natives par préfixe de nom ; `DDSOM_WHICH`, `DDSOM_SUB` et `DDSOM_PAS` fixent le numéro
d'attaque spéciale, la sous-action et le pas). Les `field` de `common` sont nommés d'après les classes déjà réécrites ; les noms (h) sont à
confirmer. Les tables de sous-actions de l'original peuvent avoir plus d'entrées que le nombre apparent (le voleur en a 16) : mesurer la
distance entre la table et le premier pas.
