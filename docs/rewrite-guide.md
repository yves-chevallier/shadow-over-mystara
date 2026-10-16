# Guide de réécriture en Rust

Objectif : remplacer, sous-système par sous-système, le code 68000 de ddsom par du Rust **idiomatique,
minimal et lisible**, au comportement **strictement identique**. Le code vit dans `port/ddsom-game`.

## Ressources

| Quoi | Où |
|---|---|
| Pseudo-C de chaque fonction (Ghidra) | `/home/ycr/som/decomp/XXXXXX.c` |
| Index : adresse, nom, taille, appelants, appelés | `/home/ycr/som/decomp/index.tsv` |
| Listing assembleur complet | `/home/ycr/som/listing.txt` |
| Programme 68000 (données en clair / opcodes déchiffrés) | `/home/ycr/som/prog_plain.bin`, `prog_dec.bin` |
| Carte mémoire, structures déjà comprises | `docs/joueur.md`, `symbols.txt`, modules de `ddsom-game` |
| ROMs | `/home/ycr/som/rom` (variable `DDSOM_ROMS` pour `cargo` hors du dépôt principal) |

Le pseudo-C de Ghidra est un point de départ, pas une vérité : les registres implicites
(`unaff_A5`, `in_A0`, `extraout_A0`) et les pointeurs relatifs à A5 (= 0xFF8000) sont à retraduire.
En cas de doute, l'assembleur du listing fait foi.

## Routine native

Une routine native remplace une routine 68000 à son adresse d'entrée (voir `src/inputs.rs`,
`src/physics.rs`) :

```rust
pub const NOM: Native = Native {
    entry: 0x1d62,           // adresse d'entrée de la routine d'origine
    name: "obj_move_xy",
    cost: |c, b| ...,        // durée en cycles 68000
    run: obj_move_xy,        // exécute la routine, RTS compris (mem::rts)
    touches_sound: false,    // accède à la RAM partagée Z80 (0x618000-0x619fff) ?
    outputs: regs::d(0),     // registres/drapeaux que les appelants lisent au retour
    exact_cost: false,       // true seulement si la durée est celle de l'original au cycle près
};
```

Règles :
- **Mémoire** : les écritures (RAM, GFX RAM, Object RAM, registres d'E/S) doivent être identiques à
  l'original, dans le même ordre pour les ports d'E/S.
- **Registres** : seuls ceux de `outputs` doivent correspondre. Vérifier dans les appelants (index,
  listing) quels registres sont lus après l'appel. Une routine appelée depuis une tâche dont
  l'appelant enchaîne sur d'autres appels a souvent un contrat vide (`outputs: 0`) : la validation
  globale ignore les zones où le jeu ne fait que *sauvegarder* des registres (piles des tâches,
  pile logicielle du VBLANK, SR sauvegardé des tâches) et détecte tout registre réellement utilisé.
- **Durée** : `timing::path_cycles` donne la durée exacte d'un chemin sans boucle ; sinon une estimation
  raisonnable (somme des instructions du chemin le plus courant) avec `exact_cost: false`.
- **Sous-routines non réécrites** : `ddsom_game::call(c, b, adresse)` les exécute comme un JSR.
  Réécrire de **haut en bas** : remplacer un sous-système entier, pas une fonction feuille isolée.
- Ajouter la routine à la liste `NATIVES` de `src/lib.rs`.

## Style

- Un module par sous-système (`sprites.rs`, `player.rs`...), commentaires en français, brefs.
- Nommer les champs : constantes de décalage regroupées (`impl Obj { const X: u32 = 0x08; ... }`),
  vues typées sur la RAM (`mem::rd16`, `mem::wr16`...), pas de nombres magiques dispersés.
- Le cœur de la logique s'exprime sur les structures du jeu ; l'adaptation aux registres 68000 reste
  à la frontière (`run`), et disparaîtra quand l'appelant sera lui-même réécrit.
- Documenter les structures découvertes dans `docs/` (format, unités, signification des drapeaux).

## Validation (obligatoire avant tout commit)

```
cd port && cargo build --release -p ddsom-game --examples
cd .. && ./port/target/release/examples/validate rom testdata/eeprom_ddsom.bin testdata/scenarios/bot30k.txt 30000
```

Deux machines tournent en parallèle : l'une avec les routines natives en mode contrôle (chaque appel
est aussi exécuté par l'original sur une copie et comparé), l'autre avec le code d'origine seul. Le
résultat doit afficher **« VALIDE »** (0 écart pour chaque routine, RAM identique).
Outils de diagnostic (`port/ddsom-game/examples`) : `watch` (qui écrit telle adresse à telle image),
`tasks` (table des tâches), `profile` (cycles par fonction).

Une routine non appelée par le scénario n'est pas validée : le signaler (et si possible ajouter un
scénario qui l'exerce dans `testdata/scenarios/`).

Commits : un commit par sous-système validé, message en français décrivant ce qui est compris et
réécrit, terminé par la ligne `Co-Authored-By` habituelle.
