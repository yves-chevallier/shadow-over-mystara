# Menu TEST (M1) et réglages (M2)

Code : `port/ddsom-game/src/taches/service.rs` (la tâche), `src/service/` (`mod.rs` : menu et aides communes ;
`essais.rs` : INPUT, OUTPUT, SOUND & VOICE ; `mire.rs` : COLOR, DOT CROSS HATCH, GAME DATA ; `reglages.rs` :
CONFIGURATION ; `memoire.rs` : MEMORY CHECK), `src/config.rs` et `src/eeprom.rs` (M2). `A5` = 0xFF8000. Les
textes et les tables restent dans la ROM et sont lus à l'exécution.

Tout est validé image par image par six scénarios (`scenarios/cov/cov_service*.txt` et `covs_carte_dev.txt`, voir §6).

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
| `0x2e4ec` | `eeprom::COMPTA_PARTIES` (native) | au départ d'un joueur, en jeu gratuit (`A5+0x25`) : compteur des parties gratuites (`0x2db1e`) ; sur la carte de développement, le compteur des parties payantes ou gratuites de sa RAM |
| `0x2dc9a` | `eeprom::COMPTA_INIT` (native) | démarrage : rien, sauf sur la carte de développement |

### Carte de développement (`eeprom/carte_dev.rs`)

Quand `A5+0x82` est non nul (le reset le pose si le bit 14 du port 0x804030 est nul : jamais sur la machine émulée, où
il vaut 0xe021 ; un `poke8 ff8082 1` le simule), le jeu tient une comptabilité plus fine dans une RAM de 16 Ko en 0x660000,
**en trois exemplaires** par zone (0x661xxx, 0x662xxx, 0x663xxx). Tout est en Rust, validé par `fuzz_carte_dev` (voir §6) :

| Original | Rust (`carte_dev`) | Rôle |
|---|---|---|
| `0x2dc9a` → `0x2dca6`, `0x2dcd2` | `demarrer` | au reset : signature de région (`0x2dd14`, 32 mots comparés à `0x2ebfc + 16 × région`), puis vote et réparation de chaque zone (`0x2dd94`... `0x2e204`), puis recopie vers les variables `A5-0x6164`... (`0x2e48a`...`0x2e77c`). Signature fausse : réinitialisation complète (`0x2e91a`) |
| `0x2e8e4` (boutons `0x3304` tenus au reset) | `reinitialiser` (`eeprom::reinitialiser_carte`, appelée par `demarrage.rs`) | remise à zéro de toutes les zones (sauf les cartes distribuées, `0x2ea24` : l'original ne la remet à zéro que sur désaccord) |
| `0x2e456`, `0x2e4a8`, `0x2e4fc`, `0x2e59a`, `0x2e540` | `pieces`, `departs`, `parties_payantes`, `parties_gratuites`, `temps_de_jeu` | +1 sur les trois exemplaires d'un compteur, vote, bornes, variable de travail (`0x2e540` : une fois par seconde de partie, appelé par `taches/partie.rs`) |
| `0x2e638` | `partie_du_personnage` | +1 au compteur (8 octets) du personnage choisi (`0x1200 + 8 × D0`) |
| `0x2e69e` | `duree_de_partie` | durée de la partie en secondes (D0 / 60), plus longue (`0x1260`) et plus courte (`0x1268`) |
| `0x2e41c` | `vote` | octets bas des mots des trois exemplaires, drapeaux 2 (B≠A), 4 (C≠A), 8 (C≠B) cumulés ; arrêt si les trois diffèrent. 0 : bon ; 6 : A fautif (copié de C) ; 10 : B fautif (de A) ; 12 : C fautif (de B) ; autre : zone perdue, remise à zéro |

Les zones, leurs bornes (pièces et départs > 999 999 : 0 ; temps total > 0x127a3980 et parties > 0x4f5880 : plafonnés)
et leurs variables sont dans l'en-tête du module. Particularités de l'original conservées : la réparation ne recopie que
les 8 premiers octets d'une zone (même celles de 48 et 16 octets) ; les compteurs sont des longs « movep » (un octet par
mot, adresses impaires). La zone `0x1240` (un vote complet, `0x2e2b0`) n'a aucun appelant et n'est pas portée (code mort).

## 6. Scénarios

| Scénario | Exerce |
|---|---|
| `cov_service.txt` | CONFIGURATION (variante 8) : tous les réglages, SAVE & EXIT |
| `cov_service_tests.txt` | INPUT (toutes les entrées), OUTPUT, SOUND & VOICE (codes, répétition, requêtes) |
| `cov_service_ecrans.txt` | COLOR, DOT CROSS HATCH, GAME DATA, MEMORY CHECK (×4), EXIT et redémarrage |
| `cov_service_variante.txt` | `poke` de la variante (4, puis 10, puis 8) et de quatre joueurs : INPUT et OUTPUT à quatre joueurs, distributeur et compteur de cartes, CONFIGURATION à 9 lignes, DEFAULT, redémarrage avec une signature d'EEPROM qui ne correspond pas (valeurs par défaut écrites par `0x2d30a`) |
| `cov_service_reglages.txt` | répétition automatique (touches tenues dans le menu et les réglages), départ gratuit (mode 17 par `poke` : ligne CONTINUE sautée, CONTINUE forcé), jeu gratuit (18), variante 0 (7-1 sans langue ni cartes), modes hors de la liste réduite de la variante 4 |
| `covs_carte_dev.txt` | carte de développement (`poke8 ff8082 1`) : GAME DATA avec temps de jeu (`hh:mm:ss`, indicateur nul, valeur négative, plafond à 99 heures) et compteurs lus en RAM supplémentaire |
| `covs_carte_dev_partie.txt` | `cov_mortal_c1` avec la carte de développement dès l'image 790 : pièces, départs, parties payantes, personnage choisi, durée de partie, temps de jeu à la seconde |
| `covs_carte_dev_gratuit.txt` | `cov_credits_gratuit` avec la carte de développement : parties gratuites (`0x2e59a`), pièces, départs |

**Fuzz différentiel** (`examples/fuzz_carte_dev.rs`, `fuzz_carte_dev <roms> <eeprom> [essais [graine]]`). Les scénarios ne
peuvent pas poser la RAM supplémentaire (`poke` n'écrit que 0xff0000-0xffffff) ni déclencher le reset avec la carte : le
fuzz tire des états de la RAM 0x660000 (zones valides, exemplaires corrompus, compteurs près de leurs bornes, RAM vierge ou
aléatoire, signature d'une des régions 0 à 6) et compare, pour `0x2dc9a`, `0x2e8e4`, `0x2e540`, `0x2e448`, `0x2e49a`,
`0x2e4ec`, `0x2e630` (D0 de 0 à 11, 0xffff, parasite dans le mot haut) et `0x2e696` (D0 de 0 à 0xffffffff), le code d'origine
recompilé seul à la version Rust : RAM de travail, RAM supplémentaire, autres mémoires. Trois graines × 10 000 essais × 8
points d'entrée : aucun écart (les registres de sortie ne sont pas comparés, `FUZZ_REGISTRES=1` les affiche : ce sont des
restes de calcul de l'original).

Générateur : les scénarios sont des suites d'appuis (4 images, 40 d'intervalle) ; l'interrupteur TEST (`Service Mode`)
reste actionné de l'image 600 jusqu'au EXIT final.

## 7. Ce qui reste en 68000

Plus rien : M1 et M2 n'exécutent aucune instruction 68000 (mesure `examples/reste68k` sur `cov_service*.txt` et
`covs_carte_dev*.txt`, 12 000 images chacun : 0 ; avant la réécriture de la carte de développement, les deux scénarios
de partie de la carte en exécutaient 33 354, dont 30 212 pour le seul temps de jeu `0x2e540`, une fois par seconde de
jeu). Historique : M1 et M2 étaient passés de 1 582 438 instructions à 0 sur `cov_service.txt` ; le reste de ce scénario
était alors le test de RAM du démarrage (K2), réécrit depuis (`docs/noyau.md`).

Blocages. « NG » du MEMORY CHECK (boucle `0x2d112`) et « EEPROM ERROR » (boucle `0x2d7fa`) : les messages sont affichés en
Rust, puis le PC est posé sur la boucle de l'original, que `service::BLOCAGE_MEMOIRE` et `BLOCAGE_EEPROM` (natives sans
effet, qui laissent passer le temps jusqu'à la fin de l'image) tiennent sans interpréteur. **Aucun scénario ne les
atteint, et aucune EEPROM modifiée non plus** : l'EEPROM émulée (`cps2::eeprom`, 93C46) écrit toujours ce qu'on lui
demande, donc l'écriture des valeurs par défaut réussit quel que soit le fichier. Essai : EEPROM toute à 0, toute à 0xff,
aléatoire, tronquée à 20 octets, vide, signature effacée (octets 0x20-0x2f à 0) ; sur 11 000 images du code d'origine avec
`cov_service_ecrans.txt`, chacune passe par les valeurs par défaut (`0x2d7aa` exécuté) et jamais par `0x2d7fa`. Le NG du
MEMORY CHECK demanderait une RAM ou une ROM défaillante (les motifs relus, la somme du programme) ou un test de l'EEPROM
raté, ce que la machine émulée ne produit pas. Corollaire connu : avec une EEPROM corrompue, la native `config_init` écrit
les valeurs par défaut instantanément alors que l'original y passe quelques images (écriture bit à bit à ×16) ; `validate`
signale alors un écart de temps dès l'image 6 (RAM des réglages), sans portée en jeu.

Restent en code d'origine, jamais exécutés par une borne :

* le « Secret Test » (M3, `0xee16e`) et la version à 13 lignes du menu (`0x295a0-0x29946` : OBJECT, SCROLL 1-3),
  inaccessibles (mot 0xc0 nul) ;
* du code mort (jamais appelé) : `0x2bbfa-0x2bc1c` et ses aides `0x299ee`, `0x29a24`, `0x29a36` ; `0x2d592`
  (comparaison des deux exemplaires) ; les variantes inutilisées de l'affichage des chiffres (`0x29cfe-0x29d2e`,
  `0x29d46`) ; le vote de la zone `0x1240` de la carte de développement (`0x2e2b0-0x2e324`, `0x2eb0a`).

Chemins Rust que le code d'origine n'a jamais parcourus dans les scénarios (non validés) : les blocages ci-dessus, et
dans SOUND & VOICE les volumes autres que le maximum (le port 0x804030 émulé vaut toujours 0xe021 : jauge pleine).
`0x2e4ec` n'est pas appelé par le menu : il est validé par les scénarios de partie (`cov_credits_gratuit` pour le jeu
gratuit, contrôle `DDSOM_CHECK` sans écart).
