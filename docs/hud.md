# Interface des joueurs : panneaux d'état, bannières, choix du personnage (F3)

Code : `port/ddsom-game/src/hud/` (`mod.rs` la tâche et les structures, `slot.rs` la machine à états d'un
emplacement, `draw.rs` et `choice.rs` le dessin, `select.rs` le choix du personnage, `status.rs` la ligne d'état
et la fin de partie, `layer.rs` la couche de caractères). `A5` = 0xFF8000. Toutes les routines de `0x15000` à
`0x18300`, initialisations de la tâche comprises, sont réécrites ; validé par la comparaison image par image (`tools/validate_all.sh`).

## La tâche (`0x150ac`)

Créée en fin de phase 0 de la partie (slot 0x30 de l'ordonnanceur). Son entrée (`0x150ac`, native `ENTRY`) remet
les panneaux à zéro (numéro de joueur en `+0`) et les variables de la tâche, puis cède une image à la fois (reprise
`0x15146`, native `STARTING`) : la disposition `A5+0x344` vient du type de borne (`A5+0x20`, table `0x15164`) ; hors
partie, mode 1 et disposition 0 ; en partie, elle attend la phase 4. Elle boucle ensuite sur `0x15178` et cède une
image à chaque tour (`jsr 0xb64`, reprise en `0x15212`). Toutes ces adresses sont des natives de reprise (`resume!`,
voir `tache_credits.rs`) : un tour entier est un appel Rust, et la tâche n'exécute plus de 68000.

Un tour (`hud::tour`) :

1. `A5+0x3c` est le **mode d'affichage** (octet) : négatif, la zone des panneaux (deux bandes de la couche de
   caractères, `draw::clear_panels`) est effacée une fois, quand le mode change ; 0, rien ; 1 à 4, les
   panneaux sont mis à jour. Les modes 1 (jeu), 2 (classement : objets), 3 (choix verrouillé) et 4 (bannière
   seule) sont posés par les scènes (`0x1cca6`, `0xf6282`, `0xf79e8`...).
2. un changement de mode oublie ce qui est affiché (`0x1507a` : drapeaux `+0x1b` et `+0x31` de chaque panneau
   à 0xff) ; chaque emplacement redessine alors ses zones (`slot::mode_change`, ex-`0x178ee`).
3. pour chaque emplacement (4 si `A5+0x344` = 0, 3 si 1, 2 sinon) : `slot::step` (ex-`0x15216`).
4. la ligne d'état (`status::status_line`, ex-`0x17814`), puis le compte à rebours de fin de partie
   (`status::countdown`, ex-`0x175a8`) .
5. services de l'interface (`0x19b2a`, `0x19b9c`) et cession.

## Enregistrements

* **panneau** (`A5+0x30e + 0x50 k`, `hud::panel`) : état (`+4`, pas de 2), valeurs affichées (PV, score, argent)
  et drapeaux « à redessiner » (`+0x09`, `+0x0a`, `+0x0b`, `+0x1b`), personnage choisi (`+0x26`, bit 0 :
  variante), compte à rebours BCD (`+0x28`) et son sous-compteur d'images (`+0x29`), etc.
* **joueur** (`A5+0x5de + 0x200 k`, `hud::player`) : actif, numéro, personnage (`+3`, bit 7 : choix verrouillé),
  état (`+4`), PV (`+0x62`) et PV max (`+0x60`), classe (`+0x10c`), score (`+0x110`, BCD), argent (`+0x114`,
  BCD), objets (`+0x118`, 8 octets), nom (`+0x160`, 8 lettres).
* **compteur de pièces** (`A5+0x44e + 0x10 k`, `+1` pièces partielles, `+2` crédits) : le mode des monnayeurs
  (`A5+0x1d`) choisit le compteur propre, partagé (`0x44e`) ou un pour deux joueurs.

## États d'un emplacement

| État | Fonction | Rôle |
|---|---|---|
| 0 | `enter` | les valeurs affichées prennent celles du joueur, tout sera redessiné |
| 2 | `idle` | « INSERT COIN » / « PUSH START » ; sur le bouton de départ avec un crédit : compteur débité, choix du personnage (état 6), ou entrée directe (`join_now`) en début de partie |
| 4 | `play` | selon le mode d'affichage : panneau (`update_panel`, les éléments qui ont changé), classement (`rank_mode`), choix verrouillé (`lock_mode`) ; joueur tombé (état joueur `0x402`) : début de la reprise (`join_continue`) |
| 6 | `choose` | choix du personnage à la croix (`select`), compte à rebours de 19 secondes ; le choix fait appelle l'initialisation du joueur (`0x2f46`...) |
| 8 | `resume` | reprise (« continue ») : compte à rebours 9 à 0, repart à chaque pièce, accéléré par un bouton dans les 16 dernières secondes ; départ sur la touche |
| 0xa | `resume_end` | message de fin affiché 120 images, puis le panneau est libéré |
| 0xc | `resume_choose` | choix du personnage de la reprise (même personnage : l'équipement est gardé) |

Le bouton de pause (`A5+0x4a`, un bit par joueur) remplace la bannière par celle de pause (`notice_banner`).

## Couche de caractères

Une case = 4 octets (tuile, attributs), colonnes de 0x80 octets (`layer.rs`). Chaque zone d'un panneau est un
rectangle dont l'origine vient d'une table du code (`0x172b4`, relative au PC donc **déchiffrée** : 8 mots par zone,
quatre positions pour les dispositions larges puis quatre pour les colonnes étroites). Les tuiles viennent de
tables de la ROM (`0x3f5xxx`-`0x3f6xxx`, lues comme des données), les palettes de `0x25c4c0`-`0x25c940` :
une palette de 16 couleurs est recopiée dans la page en gardant les quartets de luminosité (`blend_palette`).

Règle de lecture : une table indexée par adresse (`lea`/`movea.l`) est lue **en clair** ; une table lue par une
instruction `(table,PC,Dn)` est lue dans le programme **déchiffré** (`mem::prog16`).

| Zone | Contenu | Fonction |
|---|---|---|
| 1 | nom de sélection, 4x2 cases | `choice::selection_name` |
| 2 | jauge de PV (un segment de 8 PV par case, restes, vide) | `draw::hp_bar` |
| 3 | nom du personnage, 4x2 | `draw::character_name` |
| 4 | bannière à effacer | `draw::clear_banner` |
| 5 | compte à rebours du choix | `draw::countdown_digits` |
| 6 | chiffres de la reprise | `draw::continue_digits` |
| 7 | portrait (3 cases de haut) | `draw::portrait`, `choice::show_character` |
| 8 | visage de la classe | `draw::face` |
| 9 | six objets | `draw::items` |
| 0xa, 0xb | argent, score (BCD, zéros non significatifs vides) | `draw::money`, `draw::score` |
| 0xc | grand cadre (effacement), nom du joueur | `draw::clear_frame`, `choice::names` |
| 0xd | lettres de sélection | `choice::selection_letters` |
| 0xe | nom (joueurs 3 et 4 effacent leurs lignes) | `choice::names_alt` |
| 0xf | bannière « PUSH START » | `draw::start_banner`, `draw::pointer_banner` |
| 0x11 | objets du classement (2x2 cases) | `choice::rank_items` |
| 0x12 | icône de classe | `draw::class_icon` |

## Fin de partie sans joueur (`status::countdown`)

Quand plus aucun panneau n'est actif (somme des `+0x2c` nulle), un enregistrement (`0xff0ec6`) mène un compte à
rebours : autorisée par `A5+0x23`, la reprise affiche le cadre « CONTINUE » (`0x91253c`) et le plus long compte à
rebours des panneaux en grands chiffres ; à zéro, tous les panneaux passent en fin de reprise, le cadre « GAME
OVER » s'affiche 60 images, puis la phase passe à 0x14 et le drapeau de fin de partie est levé (`0x133f6`, un
simple `rts` : la tâche continue).

## Ligne d'état (`status::status_line`)

Deux colonnes de deux cases en `0x912014` : une icône et la valeur `A5+0x45` en BCD, qui clignote 96 images à chaque
changement (drapeau `A5+0x5f`).

## Quirks conservés

* le choix du personnage tient une table de bits des personnages pris ; la variante voisine est essayée avant de
  passer au personnage suivant ; `resume_choice` (0x154e8) ne restitue pas la variante du joueur en avant, mais
  le fait en arrière (comme l'original) ;
* l'état de reprise lit le « dernier chiffre » du score (`+0x113`) et l'incrémente jusqu'à 9 à chaque choix ;
* `status::status_line` dessine le drapeau `A5+0x5f` à la place de la valeur le tour où le drapeau change.
