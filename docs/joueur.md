# Joueurs : enregistrement, machine à états, noyau commun (P1, P3, P4, P6)

Code : `port/ddsom-game/src/player/`. Les adresses sont celles du programme 68000 d'origine ; `A5` =
0xFF8000. Tout ce qui est décrit ici est vérifié par la validation différentielle
(`tools/validate_all.sh`) sauf mention « non exercé » (voir §9).

## 1. Enregistrement joueur

4 enregistrements de 0x200 octets : P1 = FF85DE, P2 = FF87DE, P3 = FF89DE, P4 = FF8BDE
(`player::Player::nth(k)`). C'est aussi un objet (`objects::Obj`, champs communs de `docs/objets.md`
+0x00..+0x50) ; il est dessiné via la liste triée des acteurs (`lists::ACTORS`). Deux sous-objets de
0x60 octets par joueur (arme et bouclier) vivent à `A5+0xdde + 0x60 × (2k + n)` (`init::overlay`).

| Offset | Nom Rust (`Player::`) | Rôle |
|---|---|---|
| +0x00 | `ACTIVE` | 0 libre, 1 actif, 2 en train de tomber à terre (état 4) |
| +0x01 | `FLAGS` | bit 1 : masqué (clignotement) |
| +0x02 | `INDEX` | numéro du joueur 0..3 |
| +0x03 | `CHARACTER` | personnage 0..11 ; `>> 1` = classe (0 combattant, 1 clerc, 2 elfe, 3 nain, 4 magicien, 5 voleur) |
| +0x04..07 | `MAIN`, `SUB`, `ACTION`, `STEP` | machine à états (§2) |
| +0x08 / 0c / 10 | X, Z, Y | position 16.16 (voir `Obj`) |
| +0x14..1e | VX, AX, VZ, AZ, VY, AY | vitesses et accélérations 8.8 |
| +0x29 | `BLOCKED` | non nul : contact avec un obstacle (posé par la collision avec le décor 0x33fe4) |
| +0x2a / 2b | `PALETTE_GROUP`, `PALETTE` | palette du joueur (2, numéro du joueur) |
| +0x2c | `FACING` | 0 ou 0x20 (le bit 0 des directions demande 0x20) ; jeu de boîtes `BOX_SET` en +0x2e = classe |
| +0x33 | | drapeau d'événement posé par les scripts d'animation (coup porté, geste) |
| +0x40 / 42 / 44 | | cadre d'animation de l'arme tenue, du bouclier bas, du bouclier haut (index des tables de `overlays`) |
| +0x47 | | identifiant du coup en cours (lu par `hit_reactions`) |
| +0x48 / 4a | `OVERLAY_BODY`, `OVERLAY_ATTACK` | mots : adresses des sous-objets arme / bouclier (0 : pas affiché) |
| +0x53 | `HITSTOP` | images de gel de coup (le gestionnaire ne tourne pas) |
| +0x56 / 57 | | orientation de l'agresseur, durée du gel reçu |
| +0x5a | | dernier bord de l'écran touché (1 gauche, 2 droite) |
| +0x60 / 62 / 64 | `HP_MAX`, `HP`, `HP_PREV` | points de vie |
| +0x66 | `SLOW` | marche lente forcée (animation 1, vitesses 1 pixel par image) |
| +0x67 | | type de saut : 3 vertical, 0 sur place, 1 en avant, 2 après une course |
| +0x68 | | non nul : arme en main (sinon les numéros d'animation sont décalés de 0x40) |
| +0x69 | `PARAM` | compteur propre à l'action |
| +0x6a | `SUBACTION` | type de coup de l'attaque (0, 6, 8, 0xa, 0xc, 0xe, 0x16, 0x18, 0x1a, 0x1c, 0x1e) |
| +0x6c / 6d / 6e | `KEYS_HELD`, `KEYS_PREV`, `KEYS_NEW` | entrées (`inputs.rs`) |
| +0x6f | `TIMER_6F` | compteur de réaction (glissement, relevé, clignotement) |
| +0x70 / 71 | `RECOIL`, `RECOIL_TIMER` | recul reçu ; l'octet 70 s'efface quand 71 retombe à 0 |
| +0x79 / 7a | `POWERUP`, `POWERUP_TIMER` | bonus de puissance temporaire |
| +0x7e | `INVULN` | images d'invulnérabilité |
| +0x86 | `LINK` | long : objet tenu / porteur |
| +0x8a | `SLOW_8A` | le joueur tient quelque chose (marche lente de portage) |
| +0x8c | | mot : objet qui saisit le joueur (action 0x22) |
| +0x100..108 | | caractéristiques tirées au hasard à la création (`init`) |
| +0x109 / 10a | | puissance de base de l'arme A / B ; plafond de dégâts en +0x16e |
| +0x10c | `LEVEL` | niveau initial (table par scène) |
| +0x10d / 10e | `WEAPON_A`, `WEAPON_B` | arme tenue et bouclier / seconde arme (numéros) |
| +0x10f | `POWER_10F` | non nul : bloque boutons et bonus (h) |
| +0x110 / 114 / 118 / 11c | `SCORE_BCD`... | score (BCD, 4 octets) et autres sommes de la partie |
| +0x120..14c | `SLOT_TABLE` | douze longs d'équipement ; lignes de 16 octets des sorts/objets spéciaux (`+0x15d` : ligne choisie) |
| +0x150..158 | | réserve de sorts |
| +0x15a / 15c | | effet joueur créé par le bouton 3 (mot adresse) et son drapeau |
| +0x162 | `SPEED_CLASS` | non nul : marche normale 2 pixels par image, sinon rapide 3 |
| +0x180..184 | `AURA_TIMER`, `AURA`, `AURA_B`, `AURA_C` | auras (clignotement, deux changements de palette) |
| +0x194 / 196 | | compteurs qui décroissent (étourdissement ; compte à rebours de mort) |
| +0x1a0..1db | | neuf reconnaisseurs de coups spéciaux de 6 octets (§6) |
| +0x1e0..1e3 | | minuteries des scripts (`scripted`) et parité de la double mise à jour |

## 2. Machine à états (`+4..+7`)

Les quatre octets forment un long (`states::State`) : état principal, sous-état, action, pas. Le code
d'origine écrit souvent le long d'un coup (`move.l #0x02000c00,(4,A0)`).

| `+4` | état | gestionnaire Rust |
|---|---|---|
| 0 | scénarisé : cinématiques, entrée en scène, résurrection ; `+5` choisit le genre, `+6` est le pas | `scripted::update` (0x352da) |
| 2 | jeu normal ; `+5` = 0 agit, 4 touché, 8 paré | `update::update_playing` (0x352f4) |
| 4 | à terre / mort (`+5` doit valoir 0) | `hurt::downed` (0x3b7f4) |
| 6 | retiré de la partie | `update::remove` (0x35436) |

Jeu normal, sous-état 0 : l'octet `+6` (action, toujours pair) indexe la table 0x35586 ; `+7` est le pas
de l'action (0 : premier appel, l'action y prépare son animation).

| action | nom (`states::action`) | routine | Rust |
|---|---|---|---|
| 0x00 | `STAND` immobile | 0x355ba | `actions::stand` |
| 0x02 | `SLOW_SWITCH` entrée / sortie de la marche lente (h) | 0x35652 | `actions::slow_switch` |
| 0x04 | `WALK` marche | 0x3567a | `actions::walk` (magicien : `walk_magic_user`) |
| 0x06 | `RUN` course | 0x3573e | ROM |
| 0x08 | `TURN` demi-tour | 0x358dc | `actions::turn` |
| 0x0a | `JUMP` saut | 0x3590a | `characters::run(Slot::Jump)` |
| 0x0c | `ATTACK` attaque du bouton 1 | 0x35948 | `characters::run(Slot::Attack)` |
| 0x0e | `SPECIAL` attaque spéciale | 0x359a2 | `characters::run(Slot::Special)` |
| 0x10, 0x12 | actions propres aux personnages (h) | 0x359fc, 0x35a3a | `characters::run(Action10/12)` ; 0x12 : bond d'attaque 0x35a78 et petit saut arrière du magicien 0x35b96 (`moves`) |
| 0x06 | course | 0x3573e | `moves::run` (dérapages d'arrêt et de demi-tour) |
| 0x14 | prendre / lâcher l'objet porté (`+0x8b`) | 0x35c26 | `moves::carry` |
| 0x16, 0x18 | garde au bouclier (bouton 4 / bouton 1 maintenu) | 0x35d52, 0x35dca | `moves::guard` |
| 0x22 | saisi | 0x36abe | `actions::grabbed` |
| 0x2a | saut sur place (h) | 0x36c12 | `moves::hop` |
| 0x2e | pousser l'objet lié | 0x36d68 | `moves::push` |
| 0x32 | sort sans charge | 0x36f48 | `moves::spell_fizzle` |
| autres (0x1a..0x30) | portage, lancers, sorts... | table | ROM |

Après l'action : collision avec le décor (0x33fe4, `terrain::SHADOW`) sauf actions 0x20/0x24/0x2a, entrées figées ou
scène 4 pendant la saisie ; puis `walk::clamp_to_screen`.

Sous-état 4 (touché) : `+6` indexe la table de réactions 0x3a0f4 (33 entrées) ; chaque réaction est un
automate dont `+7` indexe sa propre table de pas. États principal 4 : table 0x3b820 (33 entrées),
qui partage la plupart de ses pas avec les réactions. Voir `hurt.rs` (`HANDLERS` : adresse d'origine ->
Rust ; toute adresse absente est exécutée par `Ctx::rom`).

## 3. Une image de joueur (`update::update_playing`, 0x352f4)

1. `combos::detect` (0xf157c) : reconnaissance des coups spéciaux (§6) ; `checks::near_death`
   (0x3760e) : sort ultime du magicien à 3 joueurs.
2. Compteurs : `FREEZE_194` décroît (masque 0x3f) ; si `HITSTOP` > 0 il décroît et seul le contrôle des
   boutons 3 et 4 s'exécute (étape 6) ; `INVULN` décroît en sous-état 0 ; `RECOIL_TIMER`/`RECOIL`.
3. Hors pause : bonus de puissance (`+0x79`) et auras (`effects`) ; clignotement (`blink`), couleurs
   d'aura (`cycle_aura_b/c`), compte à rebours de mort (`dying_countdown`).
4. Gestionnaire du sous-état (`+5`) : `actions::act` / `hurt::hurt` / parade (ROM 0x39ff0).
5. Si l'aura de vitesse (`+0x183`) est active, l'image est jouée deux fois (parité `+0x1e3`), sauf pendant
   l'attaque spéciale ou à la scène 0x43.
6. `checks::throw_button` (bouton 3) puis `special_attack` (bouton 4) ; `lists::ACTORS.reinsert`.

`update::update_all` (0x35292) fait ce travail pour les 4 joueurs actifs ; `overlays::update` (0x392e4)
recale ensuite arme et bouclier (et fait scintiller la palette sous bonus).

## 4. Création (P1, `init.rs`)

- `clear_players` (0x2d9e) et `clear_overlays` (0x2df8) : remise à zéro (appelées au début de partie).
- `init_slot` (0x2eb2, entrées 0x2e58, 0x2e6e, 0x2e86, 0x2e9e) : à partir de `CHARACTER` (posé par
  l'appelant) : niveau (table par scène, 0x31f0), points de vie (0x3238), caractéristiques au hasard
  (0x328e, un tirage), réserve de sorts (0x348e), équipement (0x3308 ; le magicien reçoit un bonus de
  0x14 pour chacun de ses trois sorts 0x52, 0x53, 0x54), drapeaux débloqués (0x3432 : octets à
  `A5-0x5b88`), armes de départ (0x34e4) et leur puissance (0x3552 : `init::init_weapon_stats`).
  Les tables sont dans la ROM (un pointeur par personnage : `prog32` pour les lectures relatives au PC,
  `data32` pour la table d'équipement 0x3402 qui est lue par un registre).
- Retrait d'un joueur (état 6, `update::remove`) : sauvegarde score/personnage/niveau à `A5-0x5b88+0x70+16k`
  (reprise après « continue »), inscription au tableau des scores (0x180c8, ROM), effacement.

## 5. Transitions et boutons (`checks.rs`)

Chaque contrôle renvoie vrai quand l'original laisse Z à 0 (« a agi »). Quelques chemins de
l'original laissent un Z accidentel (dernier octet écrit nul) : reproduits (`throw_button` selon
le type d'effet, `item_curse`). Boutons : 4 attaque, 5 saut, 6 lancer, 7 spécial.

| routine | rôle |
|---|---|
| 0x36f7e `turn_check` | demi-tour (action `TURN`) ou retournement sur place en marche lente globale |
| 0x37018 `attack_button` | coup au sol ; sous-action 2/4 si un ennemi est à portée (`target_in_range`, 0x371a) ; libère un objet tenu |
| 0x370b2 `jump_button` | saut ; direction -> animation 8, 9, 0xa, 0xf, 0x14 et `+0x67` |
| 0x3714c `throw_button` | crée un effet joueur (pool PEFF) lié au joueur |
| 0x371be `special_attack` | attaque spéciale selon la ligne de sort choisie (`+0x15d`) |
| 0x376ee `item_curse` | armes maudites 0xe et 0xf : effets aléatoires |
| 0x3760e / 0x37658 | sort ultime du magicien (somme BCD des scores >= 2 000 000) |

## 6. Coups spéciaux (P6, `combos.rs`)

`detect` (0xf157c) pose d'abord les *portes* (neuf octets à `A5-0x55e0`) selon l'état du joueur : onze
variantes lues dans le code (`gates::*`, 3 longs à +0x16 du début de chaque variante), puis fait avancer
une dizaine de reconnaisseurs. Un reconnaisseur lit une table de la ROM : paires (masque, valeur) suivies
d'un mot, -1 terminant la séquence ; le masque négatif demande « touches inchangées » ; `step`
(0xf1884) gère délai et avancement, `recognize_both` (0xf186e) essaie l'orientation normale puis
miroir, `recognize_mirror` (0xf187c) seulement le miroir. Séquence complète et porte ouverte : l'action
du coup est lancée (`moves::combo`, routines d'origine 0x3733c..0x374aa) ; si elle change l'état, les
suivants ne sont pas examinés. Ces actions mettent presque toutes le joueur en attaque (action 0xc) avec
une sous-action (`+0x6a` : 10, 12, 22, 24, 26, 30) et le sens mémorisé par la séquence (`+0x1b7`,
`+0x1bd`...) ; 0x37386 lance la course (vrai seulement vers la gauche : Z accidentel de l'original),
0x373a8 l'action 0x10, 0x373cc l'action 0x12 (0x1a avec la classe de vitesse 2), 0x374aa la saisie d'un
ennemi de dos par le voleur (ennemi au sol, même sens, assez proche ; table 0x37588 des types insaisissables).

## 7. Réactions aux coups (P4/P6)

- `hurt.rs` : réactions du joueur touché (recul 0 et 0xa, projections 6/0xe/0x1a/0x20, glissement,
  relevé, clignotement d'invulnérabilité), états à terre et mort. `hurt_animation` (0x3bb16) utilise un
  jeu d'animations par personnage distinct de celui des actions.
- `hit_reactions.rs` : `enemy_reacts` (0xf232e, appelée par la résolution des coups) choisit la réaction
  de l'ennemi frappé d'après la classe de l'attaquant, l'identifiant du coup, le type de la victime et
  un compteur d'étourdissement (`+0x88`) ; `maybe_stun` (0xf1c1a) tire au sort le sonnement.
  Particularité reproduite : l'original laisse dans A2 la table de seuils choisie, que la résolution des
  coups relit.
- `scripted.rs` (P3) : résurrection (sous-état 4 : chute, atterrissage, 150 images d'invulnérabilité),
  attente d'ouverture des commandes (sous-état 8), cinématique de la scène 1 (entrée en marchant, pose,
  course finale), marche d'entrée des scènes 2/15/16....
- `cinematics.rs` (P3) : les ~25 autres entrées en scène (table 0x23826 : apparition, attente de l'étape
  `A5+0x2e` du script d'entrée, marche, pose, arme en main, commandes en phase 12 ; chutes du haut de
  l'écran, joueur posé sur l'objet suivi par la caméra `A5+0x46`) et les 16 cinématiques de fin de scène
  (table 0x23f4e : pose de victoire 35/36 et son, marche vers une place par rang `+0x7d`, attente de
  l'étape `A5-0x55c8` du script de fin, sortie à droite, disparition par trois palettes). Chaque routine
  est un automate à table de décalages (`+6`) ; comme l'original, D0 = pas et D1 = décalage à l'entrée du
  pas (0xb378a, appelée par un pas, lit son réglage dans D1). Les cinématiques de fin réinsèrent le joueur
  dans la liste des acteurs sauf à leur dernier pas.
- Réactions (`hurt.rs`) : les réactions à table de pas (26, `STEP_TABLES`) sont aiguillées en Rust, ce
  qui fait passer leurs pas partagés par les versions Rust ; en plus des précédentes : prise (0x3ad96,
  projection puis secousses, `+0x1f0`), étourdissement (0x3ae8a), assis à se débattre (0x3b1e4).

## 8. Interface pour les personnages (P5) et les tirs (C06/C12)

- Le combattant et le clerc sont réécrits (`docs/personnages.md`) : leurs actions sont des `Native` déclarées aux adresses d'origine
  (macro `action_native!`), le reste de ce paragraphe décrit le mécanisme général.
- Le code propre à chaque personnage est atteint par `characters::run(g, p, Slot)` : table d'origine
  `Slot::table()` (12 adresses, indexées par `CHARACTER`), ou une fonction Rust ajoutée à
  `characters::OVERRIDES` (`(personnage, slot, fn(&mut Ctx, Player))`), ou une `Native` déclarée à
  l'adresse d'origine (le noyau appelle les routines d'origine par `Ctx::rom`, donc par l'exécuteur).
- Fonctions communes réutilisables : `checks::{turn_check, attack_button, jump_button, throw_button,
  special_attack, target_in_range}`, `actions::{to_stand, start_stand}`, `walk::{walk_speed, set_speed,
  move_xy, move_x, move_xz, move_xyz, move_z, clamp_to_screen}`, `animation::{start, start_plain, step}`,
  `hurt::hurt_animation`, `effects::*`, `combos::clear_recognizers`.
- Les animations de joueur sont des entrées longues (5 longs) ; `animation::start(n)` décale `n` de
  0x40 si le joueur n'a pas d'arme en main (`+0x68` nul).
- Un tir ou effet joueur (PSHL/PEFF) garde son créateur dans `+0x50` (mot) ; le bouton 3 crée un PEFF
  dont le type (`+2`) dépend du personnage (table 0x371b2).
- `Ctx` (registres + mémoire) est le contexte de tout le module ; `Player(u32)` désigne un joueur par
  son adresse ; `player::cov` et `examples/player_coverage` comptent les appels de chaque fonction.

## 9. Validation et couverture

`tools/validate_all.sh 30000` : les 8 scénarios sont VALIDE (état identique image par image) ; les
tests unitaires passent. Natives du module : `players_update` (0x35292), `player_overlays_update`
(0x392e4), `player_clamp_to_screen` (0x35c0), `player_init_1..4` (0x2e58..), `players_clear`,
`players_clear_overlays`, `enemy_reacts_to_player_hit` (0xf232e). Plus aucune durée exacte : `cost`
est indicatif.

Scénarios supplémentaires `scenarios/extra/mash_*.txt` (générés par `tools/make_bot.py --mash` :
directions et boutons tirés au hasard, ils exercent coups spéciaux, course, bas + saut, bouton 4...) :
`mash_c4`, `mash_c2`, `mash_2p` VALIDE sur 12 000 images. `mash_c5` diverge à l'image 6525 (positions
d'un effet joueur d'une famille d'objets) **même avec toutes les natives du module désactivées** : écart
hors périmètre (natives d'autres sous-systèmes), à examiner par leurs auteurs.

Fonctions réécrites jamais appelées par les 8 scénarios (voir `examples/player_coverage`) :
`curse_fall`, `team_score_reached`, `all_ready`, `clear_recognizers`, `set_stun_time`, `launch_a`,
`sparkle`, `walk::crawl_speed`, `walk::run_speed`, plusieurs pas de `scripted` ; les mash exercent en
plus `slow_switch` et les coups spéciaux. La chute scénarisée 0x3b960 (pas `+7` propres, appelée par la cinématique 0x2447e) est en Rust
(`hurt::fall` : projection sans perte de vie, sommet, retombée, rebond, glissement, puis action suivante).
Restent en ROM : actions 0x1a..0x30 hors 0x22/0x2a/0x2e
(peu exercées), les pas de
réaction rares (`hurt::STEP_TABLES`, ceux absents de `HANDLERS`), 0x37882, effets 0xe2100, 0xdb6c0,
0xe21a0, 0xe7a12, 0xb10c8.

Mesure (`examples/reste68k`, bot30k + cov_tour1_c0 + cov_4p_a, 90 000 images) : P3 470 849 → 0
instructions 68000, P4 47 157 → 4 531, P1 190 → 14, P5 1 722 → 0 (aiguillage 0x3e480), X2 2 322 → 0.

## 10. Méthode

Cartes de l'exécution : `examples/zones` (entrées de blocs du code d'origine par zone d'adresses) pour
savoir quels états et actions un scénario exerce ; `examples/player_coverage` pour les fonctions Rust.
Les tables d'aiguillage du code (décalages relatifs, longs) sont relues à l'exécution : une réaction non
réécrite continue donc à fonctionner telle quelle.

## Placement (X2, `placement.rs`)

`0x3668` place un joueur qui ressuscite (et des décors) : depuis un point de départ par joueur (tables
0x3658/0x3660 relatives à la caméra), cherche un terrain libre (`video::terrain` < 4). Particularité
reproduite : le point n'est décalé qu'une fois (+16 en profondeur, puis +16 en x), les essais suivants
répètent le même point ; à défaut, le centre de l'écran (+192, +80). La hauteur est caméra + 256 -
profondeur (le joueur tombe du haut de l'écran). `0x3af8..0x3b14` copient une des cinq palettes d'effet
(données 0x25a640) dans la palette 0x91c1e0. `0x35dc` (recadrage en x seulement) n'est plus appelé :
l'action 0x2a qui l'utilisait est en Rust (`walk::clamp_x`).

## Collision avec le décor (`terrain.rs`, 0x33fe4 et sa copie 0x3403c)

Appelée à chaque image par les joueurs, les ennemis, les ombres et les tirs (A0 = objet). Si le bit 0x800 du mot
`A5+0x7fd0` est levé, le numéro de terrain sous l'objet (`video::terrain`, 0x24b0 : attribut de la case de la
couche de défilement 2, ou carte fixe en ROM aux scènes 2 et 0x14) est écrit en `+0x29` et indexe une table de
types en ROM (entrées de 16 octets : genre, sous-programme, sens, échelle, bord, arrêt). Genre 0 : libre
(`A5+0x64/0x60/0x62` remis à zéro, D2 = 0) ; genre 1 : obstacle plein, l'objet est repoussé (X ou Y corrigé
selon que le bord est en dessous, au-dessus, à gauche ou à droite) puis **tout recommence** (`beq 0x33fe4`) tant
que le champ « arrêt » est nul ; genre > 1 : pente, un seul passage, profil linéaire dont l'échelle est un
décalage. D5 = 1 dès que l'objet a été déplacé (la copie `0x3403c` le pose avant de tester le bord, et sa
table de types est en `0x34a5c`). Dernière position testée en `A5+0x60` (x), `+0x62` (y), `+0x64` (objet).
Les sous-programmes de l'original sont atteints par des tables de sauts : A1 vaut l'adresse du sous-programme
(`0x340b6...` : obstacles de la copie 1, `0x34120...` : copie 2, `0x341a4...` : pentes), que le Rust reproduit.
