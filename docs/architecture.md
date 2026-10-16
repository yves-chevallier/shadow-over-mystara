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
