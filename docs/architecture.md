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
   de palettes (PNG RGBA), 290 échantillons (WAV avec boucle et note), tous vérifiés sans perte
   (`cargo test -p ddsom-assets`). À venir : définitions des sprites (assemblage des cellules,
   animations), cartes des niveaux, palettes d'interface, données de jeu, musiques.
4. **Son** : séquenceur réécrit à partir du pilote Z80 (fait : crate `ddsom-sound`, voir [son.md](son.md)), mixeur QSound en Rust pur (à faire).
5. **Critère final** : sur des scénarios d'entrées enregistrés, le moteur autonome produit les mêmes
   images (au pixel près) et les mêmes commandes son que l'original émulé.

### Ordre de grandeur

Le programme compte ~2 500 fonctions 68000 (~630 Ko de code) : IA de chaque ennemi et boss, objets,
magies, collisions, scripts de niveaux, menus, 4 joueurs. Beaucoup se ressemblent (machines à états
par objet, tables de données), ce qui rend la version Rust nettement plus compacte, mais c'est un
travail progressif, piloté par les tests de non-régression contre l'original.
