# Guide de réécriture en Rust

Objectif : remplacer tout le code 68000 de ddsom par du Rust **idiomatique, minimal et lisible**, au
comportement **identique image par image**. Le code vit dans `port/ddsom-game`.

## Principe

Le 68000 de la machine est **surcadencé** (×16, `Machine::set_overclock`) : le jeu finit toujours son
travail avant la fin de l'image, il n'y a plus de ralentissements, et l'état du jeu à chaque image ne
dépend plus de la durée des routines. On l'a vérifié : à ×8 et ×16, la RAM est identique sur 30 000
images (`examples/determinisme.rs`). Conséquences :

- **aucune exactitude au cycle** : une routine Rust n'a pas à reproduire la durée de l'original ;
- la validation compare l'état du jeu **à chaque image** avec le code d'origine surcadencé ;
- la seule différence assumée avec l'arcade : les ralentissements quand l'écran est chargé.

La réécriture se fait **de haut en bas** : le Rust pilote (boucle de partie, phases, mise à jour du
monde, dispatch des objets, handlers...) et appelle le code 68000 restant comme une boîte noire
(`ddsom_game::call`). Une fonction Rust appelée par du Rust n'a aucun contrat de registres : elle
prend et rend des valeurs Rust. Les registres n'existent qu'à la frontière avec le code 68000 restant.

## Ressources

| Quoi | Où |
| --- | --- |
| Pseudo-C de chaque fonction (Ghidra) | `/home/ycr/som/decomp/XXXXXX.c` |
| Index : adresse, nom, taille, appelants, appelés | `/home/ycr/som/decomp/index.tsv` |
| Listing assembleur complet (trous par endroits : désassembler `prog_dec.bin`) | `/home/ycr/som/listing.txt` |
| Programme 68000 (données en clair / opcodes déchiffrés) | `/home/ycr/som/prog_plain.bin`, `prog_dec.bin` |
| Carte du code, ordre de réécriture | `docs/atlas.md` |
| Structures déjà comprises | `docs/*.md`, `symbols.txt`, modules de `ddsom-game` |
| ROMs | `/home/ycr/som/rom` (variable `DDSOM_ROMS` pour `cargo` hors du dépôt principal) |

Le pseudo-C de Ghidra est un point de départ, pas une vérité : les registres implicites
(`unaff_A5`, `in_A0`, `extraout_A0`) et les pointeurs relatifs à A5 (= 0xFF8000) sont à retraduire.
En cas de doute, l'assembleur fait foi. Les lectures relatives au PC lisent le programme déchiffré
(`prog_dec`), les autres lectures les données en clair.

## Routine native

Une routine native remplace une routine 68000 à son adresse d'entrée, partout où elle est appelée :

```rust
pub const NOM: Native = Native {
    entry: 0x1d62,           // adresse d'entrée de la routine d'origine
    name: "obj_move_xy",
    cost: |_, _| 100,        // durée indicative (cycles) ; aucune exactitude requise
    run: obj_move_xy,        // exécute la routine, RTS compris (mem::rts)
    touches_sound: false,    // accède à la RAM partagée Z80 (0x618000-0x619fff) ?
    outputs: regs::d(0),     // registres/drapeaux que les appelants 68000 lisent au retour
    exact_cost: false,       // obsolète : laisser à false
};
```

- **Mémoire** : les écritures doivent laisser le même état que l'original (RAM, GFX RAM, Object RAM,
  registres d'E/S ; même ordre pour les ports d'E/S comme le son).
- **Registres** (`outputs`) : seulement tant que des appelants 68000 subsistent. Vérifier dans les
  appelants (index, listing) ce qui est lu après l'appel ; en cas de doute, l'inclure. Quand tous
  les appelants sont réécrits, la routine devient une simple fonction Rust.
- **Pas de calcul de cycles**, pas d'exécution « à blanc », pas de reproduction des drapeaux qu'aucun
  appelant ne lit. Si du code existant le fait encore, c'est un reste de l'ancienne méthode : on peut
  le simplifier.
- **Sous-routines non réécrites** : `ddsom_game::call(c, b, adresse)` (comme un JSR).
- Une routine qui **cède la main** à l'ordonnanceur (0xa40 attend l'image suivante, 0xa86 cède sans
  attendre) ne peut pas être une native d'un seul tenant : la découper aux points de cession, ou
  réécrire la tâche entière comme une machine à états Rust.
- Ajouter la routine à la liste `NATIVES` de son module, chaînée dans `all_natives()` de `src/lib.rs`
  (le test `entrees_uniques` vérifie qu'aucune adresse n'est remplacée deux fois).

## Style

- Un module par sous-système, commentaires en français, brefs. Le code doit être beau et lisible :
  c'est le produit final, pas un outil.
- Nommer les champs : vues typées (`objects::Obj`, `player::Player`...), constantes de décalage
  regroupées, pas de nombres magiques dispersés. Machines à états en `enum`.
- Le cœur de la logique s'exprime sur les structures du jeu ; l'adaptation aux registres 68000 reste
  à la frontière (`run`) et disparaît quand l'appelant est lui-même réécrit.
- Documenter les structures découvertes dans `docs/` (format, unités, signification des drapeaux).

## Validation (obligatoire avant tout commit)

```sh
export DDSOM_ROMS=/home/ycr/som/rom DDSOM_EEPROM=/home/ycr/som/testdata/eeprom_ddsom.bin
tools/validate_all.sh 30000      # tous les scénarios de scenarios/, en parallèle
cd port && cargo test --release -p ddsom-game --lib
```

Chaque scénario rejoue des entrées sur deux machines surcadencées, jeu réécrit et code d'origine, et
compare à chaque image la RAM de travail (hors piles), la GFX RAM, la table de sprites et la palette.
Résultat attendu : **« VALIDE »** pour chaque scénario. En cas d'écart, `validate` donne l'image et les
adresses ; `DDSOM_CHECK=1 validate ...` compare en plus chaque appel natif à l'original (registres et
mémoire, sur une copie) pour trouver la routine fautive. Outils (`port/ddsom-game/examples`) : `watch`
(qui écrit telle adresse à telle image), `tasks` (table des tâches), `profile`, `fuzz`.

Une routine jamais appelée par les scénarios n'est pas validée : le signaler (la liste des appels
est affichée) et si possible ajouter un scénario qui l'exerce.

## Travailler dans un worktree

Les ROMs et `testdata/` ne sont pas versionnés : depuis un worktree, exporter `DDSOM_ROMS` et
`DDSOM_EEPROM` comme ci-dessus, **dans la même commande que `cargo`** (sinon `ddsom-rc` se recompile,
~8 minutes). Les commandes git passent par `/usr/bin/git -C <worktree>`. La machine est partagée
avec d'autres agents : ne jamais tuer de processus (`pkill`, `kill`) qui ne sont pas les siens.

Commits : un commit par sous-système validé, message en français décrivant ce qui est compris et
réécrit, terminé par la ligne `Co-Authored-By` habituelle.
