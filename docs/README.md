# Documentation

Pour jouer : le [README du dépôt](../README.md). Pour travailler sur le moteur : [moteur.md](moteur.md), puis les
documents de référence de la partie qui vous intéresse. L'historique garde la trace de la méthode et des mesures ;
rien n'y est effacé.

## Guides

| Document | Contenu |
|---|---|
| [moteur.md](moteur.md) | le guide du moteur : crates, couches, boucle d'image, tâches, aiguillage, état, données, façades, validation |
| [pack-hd.md](pack-hd.md) | fabriquer un pack HD (export, retouche, retour aux index, vérification) |
| [sans-rom.md](sans-rom.md) | comment le jeu se passe des ROMs : assets, résidu, débordements de l'original |
| [retro-ingenierie.md](retro-ingenierie.md) | le matériel CPS2, le chiffrement, Ghidra, MAME, les premiers jalons du port |

## Référence

Données et ressources :

| Document | Contenu |
|---|---|
| [assets.md](assets.md) | les ressources extraites : fichiers, formats, tables, vérification sans perte |
| [creatures.md](creatures.md) | une image de chaque créature (générée depuis vos ROMs) |
| [niveaux.md](niveaux.md) | une image de chaque décor de scène (générée depuis vos ROMs) |
| [son.md](son.md) | le pilote son (Z80 + QSound) : format, protocole, pilote Rust |
| [son-68000.md](son-68000.md) | le son côté 68000 : commandes, file, sons nommés |

Le jeu, partie par partie (routines d'origine et leur réécriture) :

| Document | Contenu |
|---|---|
| [noyau.md](noyau.md) | VBLANK, ordonnanceur, traps, tâches Rust |
| [boucle.md](boucle.md) | boucle de partie, phases, aiguillage des objets |
| [objets.md](objets.md) | le modèle d'objets : pools, listes, animation, mouvement |
| [video.md](video.md) | couches, tilemaps, palettes, fondus |
| [scenes.md](scenes.md) | déroulement des scènes : entrée, sortie, boutiques |
| [modes.md](modes.md) | machine à modes, écrans hors jeu, fin de partie, saisie du nom |
| [hud.md](hud.md) | interface des joueurs : panneaux, bannières, choix du personnage |
| [service.md](service.md) | le menu TEST et les réglages |
| [feuilles.md](feuilles.md) | RNG, événements, pièces, EEPROM |
| [joueur.md](joueur.md) | joueurs : fiche, machine à états, noyau commun |
| [personnages.md](personnages.md) | le code des six personnages |
| [combat.md](combat.md) | collisions, coups, dégâts, aides d'IA |
| [familles.md](familles.md) | familles ITEM, BOX et EFFCT |
| [familles-eshl-set-sobj.md](familles-eshl-set-sobj.md) | familles ESHL, SET et SOBJ |
| [familles/](familles/) | une page par famille : [boss](familles/boss.md), [enemy](familles/enemy.md), [eshl_b](familles/eshl_b.md), [eshl_c](familles/eshl_c.md), [joueur](familles/joueur.md), [set](familles/set.md), [set_b](familles/set_b.md), [set_c](familles/set_c.md), [sobj](familles/sobj.md) |
| [code-mort.md](code-mort.md) | le code mort et les replis jamais atteints de l'original, cas par cas |
| [atlas.md](atlas.md) | l'atlas du code 68000 : zones, routines, documents |

## Historique et méthode

| Document | Contenu |
|---|---|
| [reprise.md](reprise.md) | l'état du projet et le point de reprise |
| [fin.md](fin.md) | le plan de fin (lots L0 à L22), les résultats et mesures de chaque lot, le bilan |
| [refactorisation.md](refactorisation.md) | la refactorisation du Rust « en forme de 68000 » au Rust ordinaire (lots 0 à E, sortie du Cpu) |
| [guide-refactorisation.md](guide-refactorisation.md) | le guide pratique des lots de refactorisation : règles, API commune, check-list |
| [rewrite-guide.md](rewrite-guide.md) | le guide de la réécriture des routines en Rust |
| [architecture.md](architecture.md) | l'architecture du port et ses jalons, de la machine de référence au jeu sans 68000 |
| [couverture.md](couverture.md) | la couverture des scénarios |
| [couverture-fonctions.md](couverture-fonctions.md) | les fonctions d'origine jamais exécutées, validation par fuzz ; branches : [couverture-branches.tsv](couverture-branches.tsv) |
| [agents/brief-commun.md](agents/brief-commun.md), [agents/brief-lots.md](agents/brief-lots.md) | les consignes données aux agents des lots |
