# Menu TEST (M1) et réglages (M2)

Code : `port/ddsom-game/src/taches/service.rs` (la tâche), `src/service/` (`mod.rs` : menu et aides communes ;
`essais.rs` : INPUT, OUTPUT, SOUND & VOICE ; `mire.rs` : COLOR, DOT CROSS HATCH, GAME DATA ; `reglages.rs` :
CONFIGURATION ; `memoire.rs` : MEMORY CHECK), `src/config.rs` et `src/eeprom.rs` (M2). `A5` = 0xFF8000. Les
textes et les tables restent dans la ROM et sont lus à l'exécution.

Tout est validé image par image par quatre scénarios (`scenarios/cov/cov_service*.txt`, voir §6).

## 1. Entrée et tâche (`0x29162`)

La tâche des crédits (`tache_credits.rs`) se démonte quand l'interrupteur TEST est actionné (bit 1 de `IN2+1`, actif
bas) : écran remis à zéro, tâche du menu `0x29162` créée à l'emplacement 0, tâche des crédits terminée.

La tâche du menu est une tâche Rust (`docs/noyau.md`, « Tâches Rust ») :

| Adresse | Étape |
|---|---|
| `0x29162` | entrée : enregistrement du menu (FF1CF8, 0x60 octets) remis à zéro, tâche de l'emplacement 15 tuée |
| `0x29190` | un tour : gestionnaire de l'état du menu (appelé comme par `jsr`, retour `0x2918a` empilé), puis `jsr 0xb64` |
| `0x2918a` | retour du gestionnaire quand il a cédé la main en route : l'image d'attente |
| `0x291ec` | reprise du dessin du menu, après l'image d'attente qui suit l'effacement de l'écran |

Le gestionnaire ne cède la main qu'à un endroit : le dessin du menu (`0x291a0`, à l'entrée et à chaque retour au menu)
efface l'écran, attend une image (`jsr 0xb64`, pile : `0x2918a`, `0x291ec`), puis affiche le menu. Tout le reste tient
dans une image. La sortie (9. EXIT) remplace la tâche par la tâche de démarrage (`0xae4`, A0 = `0x11b16`) : le jeu
redémarre (sans le test de RAM du démarrage).

### Enregistrement du menu (FF1CF8)

| Octet | Rôle |
|---|---|
| 0 | état : 0 préparation, 2 menu, 4 écran de test |
| 1 | ligne du menu (0, 2... 16) |
| 2 | étape de l'écran (0 : préparation) |
| 3, 4, 5 | CONFIGURATION : ligne du menu, étape et ligne du sous-menu |
| 0x10, 0x11, 0x12 | répétition automatique : haut/bas, gauche/droite, tirs (30 images, puis toutes les 8) |
| 0x14 | MEMORY CHECK : décompte avant le retour au menu |
| 0x30, 0x32, 0x33 | SOUND & VOICE : code (mot), volume, code à réafficher |
| 0x5e | « Secret Test » demandé |

Autres variables : `A5+0x7fcc` (menu actif, lu par le son), A5 - 0x62a8 (tampon de la conversion décimale), A5 - 0x6298 /
A5 - 0x6294 (OUTPUT : compteur électrique), A5 - 0x6292... (CONFIGURATION : valeurs d'entrée des réglages).

## 2. Le menu (`0x29254`)

Haut/bas (P1) choisit une ligne, tir 1 entre dans l'écran ; dans chaque écran (sauf CONFIGURATION et MEMORY CHECK),
départ 1P tenu + tir 1 revient au menu (`0x29a48` ; le son est coupé). La ligne « 7. CONFIGURATION » est en couleur
quand les réglages diffèrent des réglages d'usine de la variante (`config::is_factory`, `0x2d5c6`).

« Secret Test » (M3, `0xee16e`) : seulement si le mot 0xc0 du programme n'est pas nul (il l'est) et que les tirs 2 de
P1 et P2 sont pressés ensemble. Jamais atteint ; le menu à 13 lignes (OBJECT, SCROLL 1-3) et ses écrans
(`0x295a0-0x29946`, `0x2af50...`) appartiennent à cette version de mise au point et ne sont pas réécrits.

| Ligne | Original | Rust | Écran |
|---|---|---|---|
| 1. INPUT | `0x2a4f4` | `essais::input` | état de chaque entrée (« 0 »/« 1 ») : service, test, pièces, départs, leviers, boutons ; 2 à 4 joueurs selon le type de monnayeurs (`A5+0x1d`) |
| 2. OUTPUT | `0x2aadc` | `essais::output` | tir 1 de chaque joueur : verrou de son monnayeur (`0xb96`/`0xbac`) ; tir 2 de P1 : compteur électrique ; tir 2 de P2 : distributeur de cartes (variantes 4 et 10) |
| 3. SOUND & VOICE | `0x2b450` | `essais::sound` | code (0-0x4a0) : haut/bas ±1, droite/gauche ±0x10, boutons 3/4 ±0x100 ; tir 1 le joue (`0xe0836`), tir 2 coupe (`0xe0882`) ; volume lu sur le port 0x804030 |
| 4. COLOR | `0x29366` | `mire::color` | quatre dégradés de 15 couleurs (palettes 0x1c-0x1f) |
| 5. DOT CROSS HATCH | `0x2949c` | `mire::cross_hatch` | grille sur le plan 2 |
| 6. GAME DATA | `0x2b808` | `mire::game_data` | compteurs de l'EEPROM : pièces, services, parties gratuites ou cartes |
| 7. CONFIGURATION | `0x2bb0a` | `reglages::configuration` | §3 |
| 8. MEMORY CHECK | `0x2ceb4` | `memoire::memory` | §4 |
| 9. EXIT | `0x29314` | `exit` | redémarrage |

## 3. 7. CONFIGURATION (`0x2bb0a`)

Menu : 1. SYSTEM, 2. GAME (tir 1 : sous-menu), 3. DEFAULT (départ 1P + tir 1 : réglages d'usine, `0x2d546`),
4. SAVE & EXIT (tir 1 : « NOW SAVING », enregistrement `0x2d47c`, retour au menu). Dans un sous-menu, haut/bas
choisit la ligne, gauche/droite ou tir 1/tir 2 change le réglage, tir 1 sur EXIT revient au menu de configuration.
Les réglages changent directement les variables du jeu ; un réglage différent de sa valeur à l'entrée est en
couleur.

7-1 SYSTEM (lignes selon la variante `A5+0x70` : 8 ajoute la langue, 4 et 10 le distributeur et les points de carte) :

| Réglage | Variable | Valeurs | Texte (ROM) |
|---|---|---|---|
| COIN MODE | `A5+0x1c` | 0-18 (variante 4 : 0, 1, 9 ; 10 : 0, 1, 2, 9) | `0x2c042`, 32 car. |
| CHUTE TYPE | `A5+0x1d` | 0-11 (cartes : 0-2) | `0x2c314`, 26 car. |
| CONTINUE | `A5+0x23` | 0/1 (forcé à 1 en mode 17, « départ gratuit » : la ligne est alors sautée) | ON/OFF |
| MONITOR | `A5+0x26` | 0/1 | `0x2c4e8`, 8 car. |
| DEMO SOUND | `A5+0x27` | 0/1 | ON/OFF |
| SOUND MODE | `A5+0x28` | 0/1 (mono, QSound) | `0x2c590`, 16 car. |
| LANGUAGE | mot `A5+0x72` | 2, 4, 6 (0, le japonais, est sauté) | `0x2c610`, 16 car. |
| CARD DISPENSER | `A5+0x84` | 0/1 | ON/OFF |
| CARD POINT | `A5+0x85` | 2-14 | `0x2c71a`, 16 car. |

7-2 GAME : DIFFICULTY (`A5+0x2a`, 0-7, `0x2c862`, 20 car.).

Le menu de configuration recopie le D0 laissé par le chargement des palettes (`0x1a296`) dans les compteurs de
répétition `+0x10`/`+0x11`, comme l'original.

## 4. 8. MEMORY CHECK (`0x2ceb4`)

Étape 0 : textes. Étape 2 : le test (`0x2cf26`), interruptions masquées (trap 12) : quatre motifs (0, 0x5555,
0xaaaa, 0xffff) écrits et relus dans chaque mot de la RAM de travail, de la GFX RAM, des deux bancs de l'Object RAM
(sortie 0x8040e1, basculée par `bchg` : relue à 0, elle vaut 1 après le test) et de la RAM du Z80 (octet de poids
faible ; 0x619ffb vaut 0 pendant le test, 0x88 après) ; test de l'EEPROM (`eeprom::tester`, `0x2da4a` : quatre motifs
dans les 64 mots, puis la copie en RAM validée et réécrite) ; somme d'un octet sur 0xfff de chacun des 8 blocs de
512 Ko du programme, comparée à l'en-tête (0xca). « OK » par ligne ; une erreur affiche « NG » et bloque la tâche.
Étape 4 : retour au menu après 60 images.

**Écart assumé.** Le test dure une vingtaine d'images à ×16 (plusieurs secondes sur la borne), interruptions
masquées ; sa durée dépend de la fréquence du 68000, comme les ralentissements. Il est instantané dans les deux
machines de la validation : en Rust, et dans le code d'origine par un correctif (`memoire::patch_instant_check`,
liste `patches` de `lib.rs`) qui exécute `0x2cf26` jusqu'à son retour sans faire avancer le temps de la machine.
La validation compare donc le test Rust à l'exécution du code d'origine.

## 5. Réglages et EEPROM (M2, `config.rs`, `eeprom.rs`)

| Routine | Rust | Rôle |
|---|---|---|
| `0x2d30a` | `config::INIT` (native : appelée par `0x492` et la tâche de démarrage) | lit l'EEPROM (`0x2d808`) ; signature de la variante (octets 0x20-0x2f) correcte : réglages validés (`0x2d662`) ; sinon (ou lecture impossible) valeurs par défaut écrites (`0x2d7aa`, `0x2d8aa` ; échec : « EEPROM ERROR » et blocage) ; puis variables du jeu (`0x2d358`) |
| `0x2d47c` | `config::save` | variables → deux exemplaires de la copie + signature, écrits (0x30 mots), relus dans les variables |
| `0x2d546` | `config::reset_factory` | réglages d'usine de la variante dans les variables |
| `0x2d5c6` | `config::is_factory` | les variables sont-elles les réglages d'usine ? |
| `0x2da4a` | `eeprom::tester` | test de l'EEPROM du MEMORY CHECK |
| `0x2e4ec` | `eeprom::COMPTA_PARTIES` (native) | au départ d'un joueur, en jeu gratuit (`A5+0x25`) : compteur des parties gratuites (`0x2db1e`) |
| `0x2dc9a` | `eeprom::COMPTA_INIT` (native) | démarrage : rien, sauf sur la carte de développement |

Les variantes de la carte de développement (`A5+0x82` non nul : RAM supplémentaire 0x660000, temps de jeu) restent
en code d'origine : la fin de GAME DATA (`0x2b842`), les suites de `0x2e4ec` (`0x2e4fc`, `0x2e59a`) et `0x2dca6`/`0x2dcd2`.

## 6. Scénarios

| Scénario | Exerce |
|---|---|
| `cov_service.txt` | CONFIGURATION (variante 8) : tous les réglages, SAVE & EXIT |
| `cov_service_tests.txt` | INPUT (toutes les entrées), OUTPUT, SOUND & VOICE (codes, répétition, requêtes) |
| `cov_service_ecrans.txt` | COLOR, DOT CROSS HATCH, GAME DATA, MEMORY CHECK (×4), EXIT et redémarrage |
| `cov_service_variante.txt` | `poke` de la variante (4, puis 10, puis 8) et de quatre joueurs : INPUT et OUTPUT à quatre joueurs, distributeur et compteur de cartes, CONFIGURATION à 9 lignes, DEFAULT, départ gratuit (ligne CONTINUE sautée), redémarrage avec une signature d'EEPROM qui ne correspond pas (valeurs par défaut écrites par `0x2d30a`) |

Générateur : les scénarios sont des suites d'appuis (4 images, 40 d'intervalle) ; l'interrupteur TEST (`Service Mode`)
reste actionné de l'image 600 jusqu'au EXIT final.

## 7. Ce qui reste en 68000

Rien dans les quatre scénarios hors des routines de la carte de développement (§5). Code jamais atteint (non validé) :
voir le rapport du travail (liste des chemins Rust non exercés ci-dessous).
