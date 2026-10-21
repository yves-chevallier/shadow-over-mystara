# Architecture du port Rust

## Décision : recompilation statique, puis réécriture progressive

Le jeu fait ~600 Ko de code 68000 (~2 400 fonctions). Trois voies :

| Voie | Principe | Verdict |
|---|---|---|
| Réécriture manuelle | Traduire chaque fonction à la main | Des années avant de voir le jeu tourner, aucune vérification possible avant la fin |
| Émulation | Interpréter le 68000 | Ce n'est pas un port, MAME le fait déjà |
| **Recompilation statique** | Un générateur traduit automatiquement le code 68000 en Rust, exécuté sur un runtime CPS2 écrit en Rust | **Retenue** : le jeu tourne tôt, chaque étape se compare à MAME, puis on remplace fonction par fonction le code généré par du Rust idiomatique |

C'est l'approche des projets N64Recomp / XenonRecomp, adaptée au 68000.

## Contraintes propres à ddsom

- **Pile manipulée directement** : l'ordonnanceur change de pile entre tâches (`move.l SP,...`,
  `movea.l ...,SP`), le boot appelle sans pile (`lea ret(pc),a6 ; jmp (a6)`). On ne peut donc pas
  traduire un `jsr` 68000 en appel de fonction Rust.
  → Granularité **bloc de base** : chaque bloc devient une fonction Rust `fn(&mut Cpu, &mut Bus) -> u32`
  qui renvoie le PC suivant ; `jsr`/`rts` empilent/dépilent sur la pile 68000 émulée en RAM.
  Un répartiteur `match pc` enchaîne les blocs. Fidèle par construction à tous les idiomes.
- **Sauts calculés** : déjà résolus par Cps2Setup (tables) et complétés par la couverture ;
  un PC inconnu au runtime est signalé (et peut tomber sur un interpréteur de secours).
- **Opcodes chiffrés** : sans objet ici, le générateur lit l'image déchiffrée ; les données lues
  via le PC viennent de `prog_merged.bin`.

## Découpage en crates (`port/`)

| Crate | Rôle |
|---|---|
| `cps2-gfx` | Décodage des ROMs graphiques, rendu CPS-A/CPS-B (tilemaps, sprites, priorités, palettes) |
| `cps2-hw` | Carte mémoire 68000 : ROM, RAM, E/S, EEPROM, timings vblank/raster |
| `m68k-recomp` | Générateur : `prog_merged.bin` + carte du code → `generated.rs` |
| `ddsom` | Runtime : état CPU (registres, flags), blocs générés, ordonnanceur d'interruptions |
| `audio` | Z80 + QSound (d'abord émulé, c'est un pilote son autonome) |
| `app` | Fenêtre, entrées clavier/manette, audio (winit/pixels ou SDL2) |

## Jalons, chacun vérifié contre MAME

1. **Rendu** : à partir d'un instantané MAME (GFX RAM, Object RAM, registres CPS), le moteur Rust
   produit une image identique au PNG de MAME.
2. **Interpréteur de référence** : un interpréteur 68000 minimal dans le runtime, comparé trace contre
   trace avec MAME. Il sert d'oracle et de filet de sécurité pour le recompilateur.
3. **Recompilation** : le code généré démarre jusqu'à l'écran WARNING, puis le mode démo,
   en comparant la RAM image par image avec l'interpréteur/MAME.
4. **Jeu jouable** avec son.
5. **Réécriture idiomatique** des sous-systèmes (physique, entrées, IA...) en remplaçant les blocs
   générés, chaque remplacement validé par comparaison de RAM.

## État d'avancement

| Jalon | État |
|---|---|
| 1. Rendu | ✅ pixel près sur 11 instantanés + 6 captures de jeu |
| 2. Interpréteur de référence | ✅ 68000 et Z80 exacts au cycle près (SingleStepTests) ; machine identique à MAME sur 4800 images avec entrées |
| 3. Recompilation | ✅ `recomp` + `ddsom-rc` : 53 000 blocs générés à la compilation depuis les ROMs locales ; 98 % des instructions exécutées en Rust généré, résultats identiques à l'interpréteur et à MAME |
| 4. Jeu jouable avec son | ✅ `./jouer.sh` (code recompilé par défaut, `DDSOM_INTERP=1` pour l'interpréteur) |
| 5. Réécriture idiomatique | à faire |

### Fonctionnement du code recompilé

- `recomp::discover` : descente récursive depuis les vecteurs + `ddsom-rc/entries.txt` (adresses
  apprises à l'exécution : `MISSES_OUT=fichier` avec l'exemple `compare` de ddsom-rc).
- Chaque instruction devient `c.op_*(bus, &I)` avec `I` constante : même code que l'interpréteur,
  spécialisé par LLVM. Le bloc rend la main dès que l'échéance de la ligne est atteinte, ce qui
  garde les interruptions au même cycle qu'avec l'interpréteur.
- Toute adresse sans bloc (reprise en milieu de bloc, code non découvert) est exécutée par
  l'interpréteur : le résultat ne dépend jamais de la qualité de la découverte, seulement la vitesse.
- Le code généré dérive des ROMs : il n'est produit que localement (`build.rs`), jamais versionné.

## Destination finale : un moteur Rust autonome

Le but n'est pas d'émuler mais de **réimplémenter** : un moteur Rust lisible et minimaliste, sans ROM,
sans 68000 ni Z80, au comportement strictement identique à l'original.

```
assets/              extraits localement depuis les ROMs de l'utilisateur (jamais versionnés)
  sprites/*.png      planches de sprites et tuiles, avec leurs palettes
  sounds/*.wav       échantillons QSound
  data/*.ron         niveaux, animations, tables (vitesses, IA, objets...)
engine/              le jeu : état en structures Rust, une image = une fonction update()
  world.rs           joueurs, ennemis, objets, collisions
  render.rs          liste de sprites + décors -> image
  audio.rs           mixeur façon QSound + séquenceur de musique
```

### Comment on y va sans perdre l'exactitude

L'émulateur (m68k, z80, cps2, recomp) ne disparaît pas : il devient **l'oracle de test**. Chaque
morceau réécrit est exécuté en parallèle de l'original et comparé.

1. **Routines natives** (en cours) : remplacer des routines 68000 une à une, à l'adresse d'entrée,
   avec contrôle différentiel. Les adaptateurs registres/drapeaux sont un échafaudage temporaire : ils
   disparaissent quand l'appelant est lui-même réécrit.
2. **Remonter la frontière** : réécrire des tâches entières de l'ordonnanceur (joueur, ennemis,
   décor, menus) puis la boucle de jeu. L'état passe de la RAM émulée à des structures Rust ; tant que
   les deux mondes coexistent, une projection RAM <-> structures sert de pont et de point de comparaison.
3. **Extraction des données** : un outil lit les ROMs de l'utilisateur et produit `assets/`. Le
   moteur ne lit plus que ces fichiers. Fait : graphismes (cellules 16x16 en PNG indexé), bibliothèque
   de palettes (PNG RGBA), 290 échantillons (WAV avec boucle et note), définitions des sprites (21 887
   images, 31 610 entrées d'animation, JSON), cartes des décors des scènes (scroll 2 et 3) et images
   du scroll 1, palettes des scènes, terrain (cartes fixes, types), boîtes de collision et de
   visibilité, marques au sol, textes de la couche 8x8 et noms, musiques et effets (sons, pistes,
   instruments, tables du pilote Z80), scripts d'apparition des scènes, tables de jeu (personnages,
   réactions aux coups, ennemis, caisses, scènes, boutiques, caméras, interface), entrées des
   démonstrations, générique, dialogues et bulles des trois langues, tous vérifiés sans perte
   (`cargo test -p ddsom-assets`, voir [assets.md](assets.md)). À venir : quelques tables encore
   rangées avec le code (inventaire dans assets.md) et la sémantique des commandes du moteur de texte.
4. **Son** : séquenceur réécrit à partir du pilote Z80 (fait : crate `ddsom-sound`, voir [son.md](son.md)), mixeur QSound en Rust pur (à faire).
5. **Critère final** : sur des scénarios d'entrées enregistrés, le moteur autonome produit les mêmes
   images (au pixel près) et les mêmes commandes son que l'original émulé.

### Ordre de grandeur

Le programme compte ~2 500 fonctions 68000 (~630 Ko de code) : IA de chaque ennemi et boss, objets,
magies, collisions, scripts de niveaux, menus, 4 joueurs. Beaucoup se ressemblent (machines à états
par objet, tables de données), ce qui rend la version Rust nettement plus compacte, mais c'est un
travail progressif, piloté par les tests de non-régression contre l'original.

## Dernière étape : des tâches Rust, puis plus de 68000

État : l'ordonnanceur (0x928), les traps 0 à 7 et 12, leurs enveloppes (0x9b6, 0xa14, 0xa40, 0xa86,
0xb64...) et le VBLANK (0x574) sont des routines natives (`kernel.rs`, `vblank.rs`, [noyau.md](noyau.md)) ;
la tâche des crédits est du Rust (`tache_credits.rs`, `ecran_credits.rs`). La table des tâches est une structure Rust
(`taches/table.rs`) : plus d'enregistrements FF0064, de tas des piles ni de pile de l'ordonnanceur en RAM, et les
registres d'une tâche qui cède sont rangés dans la table au lieu de sa pile. Une tâche est encore « du code qui reprend
à une adresse avec le contexte du 68000 sauvé » ; c'est l'adresse de reprise qui devient une native quand la tâche est
réécrite (`resume!`). `tools/reste68k.sh` mesure ce qui tourne encore en 68000 ([reste68k.md](reste68k.md)).

Plan pour supprimer la machine 68000 :

1. **Découper chaque tâche aux points de cession** (fait pour la machine à modes, la fin de partie, les crédits,
   la tâche de partie `0x1c45a` et la tâche du gel `0xee080`, `taches/`, voir [noyau.md](noyau.md), les tâches et
   sous-programmes de fondu (`taches/fondu.rs`), l'écran des scores `0x1835e` (`taches/ecran_fixe.rs`) et l'interface
   joueurs `0x150ac` (`hud/`), les invites `0xe03fe` (`taches/invite.rs`) et les scripts de transition de la sortie de
   scène, qui cédaient à presque chaque pas (`scene/transition/`) ; restent, en 68000 : le menu TEST `0x29162` et
   l'écran `0x19068`, jamais atteint par les scénarios. Les « listes de bulles des boss » créées par `0x9b6` sont des
   tâches de fondu, déjà en Rust).
   Chaque tâche devient une machine à états Rust : une `enum` d'étapes et une fonction
   `pas(&mut Monde) -> Attente` qui rend `Images(n)`, `Prête`, `Suspendue` ou `Terminée`.
2. **Remplacer l'enregistrement de tâche** (fait, `taches/table.rs`, [noyau.md](noyau.md) « Table des tâches en
   Rust ») : 16 emplacements (les numéros d'emplacement servent encore : `kill(0x30)`), l'état (libre, attend,
   suspendue, prête, en cours, neuve), le compte d'attente décrémenté par le VBLANK, la pile 68000 donnée à la tâche et
   sa reprise (entrée, ou contexte du 68000 sauvé à la cession). `validate` compare à chaque image la table Rust à la
   table FF0064 de la machine d'origine (états, comptes, piles, entrées, tas). Le tas des piles, la pile de
   l'ordonnanceur et les registres sauvés sur la pile des tâches ne servent plus au noyau. Restent sur la pile 68000 des
   tâches leurs appels en cours (adresses de retour, sous-programmes qui cèdent) : c'est le point 4.
3. **Appels des anciens sous-programmes** : `ddsom_game::call` n'a plus d'appelant quand toutes les
   routines appelées sont natives ; alors plus aucune instruction 68000 ne s'exécute
   (`tools/reste68k.sh` : zéro sur tous les scénarios). Les fonctions « jamais atteintes » du rapport
   sont soit du code mort à supprimer, soit des chemins à couvrir par de nouveaux scénarios avant leur
   réécriture.
4. **Supprimer la machine** : la boucle d'image devient `vblank(); ordonnanceur()` appelés directement
   (plus d'interruption, plus de `STOP`, plus de surcadencement : l'« avance rapide » reste plafonnée à
   `MAX_STEPS_PER_FRAME` pas par image) ; le CPU 68000 et le recompilateur (`ddsom-rc`) quittent le
   chemin du jeu et ne restent que comme oracle de test (validation différentielle, crate `m68k`).
   L'état du jeu passe de la RAM émulée à des structures Rust, projection par projection, avec la même
   comparaison par image.
