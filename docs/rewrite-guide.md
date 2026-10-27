# Guide de réécriture en Rust

> La conversion du 68000 est finie. La phase suivante (rendre le code lisible, sans changer le comportement) suit
> [guide-refactorisation.md](guide-refactorisation.md) et [refactorisation.md](refactorisation.md) ; ce guide reste la
> référence pour la frontière avec le code d'origine (natives, registres, validation). Depuis le lot 0a, une native
> n'annonce plus de durée et le jeu validé est le jeu sans 68000 (le mode hybride, `DDSOM_HYBRIDE=1`, a été retiré au lot « noyau final »).

Objectif : remplacer tout le code 68000 de ddsom par du Rust **idiomatique, minimal et lisible**, au
comportement **identique image par image**. Le code vit dans `port/ddsom-game`.

## Principe

Le 68000 de la machine est **surcadencé** (×16, `Machine::set_overclock`) : le jeu finit toujours son
travail avant la fin de l'image, il n'y a plus de ralentissements, et l'état du jeu à chaque image ne
dépend plus de la durée des routines. On l'a vérifié : à ×8 et ×16, la RAM est identique sur 30 000
images (`examples/determinisme.rs`). Conséquences :

- **aucune exactitude au cycle** : une routine Rust n'a pas à reproduire la durée de l'original ;
- la validation compare l'état du jeu **à chaque image** avec le code d'origine surcadencé ;
- les différences assumées avec l'arcade : les ralentissements quand l'écran est chargé, et le nombre de pas de
  jeu par image dans l'« avance rapide » (bouton pressé à l'attract ou à l'entrée de scène, où le jeu enchaîne
  ses pas sans attendre l'image : 7-8 pas par image sur l'arcade, mais ×16 en ferait une centaine, dépendant de la
  durée de chaque routine). Ce nombre est plafonné à `MAX_STEPS_PER_FRAME` (8, `cps2::Machine::step_cap`),
  **à l'identique** dans `ddsom_game::install` (le jeu) et `ddsom_game::install_reference` (code
  d'origine) : toute machine d'origine créée par un outil doit passer par `install_reference`, jamais par
  `ddsom_rc::install` seul. `DDSOM_STEP_CAP=n` change la valeur, 0 la supprime (mesures à ×1). Voir `docs/boucle.md` §1.
- le test de mémoire du menu TEST (8. MEMORY CHECK) : il dure une vingtaine d'images interruptions masquées et sa durée
  dépend de la fréquence du 68000 ; il est instantané dans les deux machines (correctif `service::memoire::patch_instant_check`
  de la liste `patches` de `lib.rs`, qui exécute le code d'origine sans faire avancer le temps). Voir `docs/service.md` §4.

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
    run: obj_move_xy,        // exécute la routine, RTS compris (mem::rts)
    touches_sound: false,    // accède à la RAM partagée Z80 (0x618000-0x619fff) ?
    outputs: regs::d(0),     // registres/drapeaux que les appelants 68000 lisent au retour
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
mémoire, sur une copie) pour trouver la routine fautive. Les registres que l'original laisse comptent : la boucle de partie
enchaîne les natives sans rien réinitialiser (`DDSOM_CHECK_ALL=1` les liste, `DDSOM_ADOPT_REGS=préfixe,...` fait adopter ceux
de l'original pour isoler une fuite de registre). Outils (`port/ddsom-game/examples`) : `watch`
(qui écrit telle adresse à telle image), `tasks` (table des tâches), `profile`, `fuzz`.

Une routine jamais appelée par les scénarios n'est pas validée : le signaler (la liste des appels
est affichée) et si possible ajouter un scénario qui l'exerce.

## Format des scénarios (`scenarios/*.txt`)

Une action par ligne, `image champ valeur` (voir l'en-tête de `port/ddsom-game/src/scenario.rs`) :

| Ligne | Effet |
|---|---|
| `900 Coin 1 1` / `906 Coin 1 0` | entrée MAME (1 = appuyée, 0 = relâchée) : `P1..P4 Right/Left/Up/Down/Button 1..4`, `Coin 1..4`, `1..4 Players Start`, `Service 1`, `Service Mode` (menu TEST) |
| `2400 poke8 ff8034 24` | **écrit en RAM de travail** (`poke8`, `poke16`, `poke32` ; adresse hexadécimale `ff0000-ffffff`, valeur décimale ou `0x…`, big-endian) juste avant l'image `n + 1`, comme une entrée. Appliquée à l'identique par les deux machines de `validate` |
| `30001 quit` | fin (ignorée par `validate`, utilisée par `mame/scenario.lua`, qui gère aussi `poke`) |

Les scénarios ne contiennent que des entrées et des adresses/valeurs de RAM (aucune donnée de ROM).

### Atteindre un stage ou un boss directement

`FF8034` est le numéro de scène (1-114 : 1-89 jeu, dont 69-82 boutiques ; 83-89 graphismes non initialisés quand on y
entre directement ; 90-114 scènes de démonstration de l'attract). Écrire une scène pendant la sélection du personnage
(`poke8 ff8034 N` vers l'image 2400) lance la partie directement dans cette scène. En cours de partie, un saut propre
s'obtient avec `ff8034 ← N`, `ff802c ← 8`, `ff802d ← 0`, `ff802e ← 0` (phase 8 : entrée de scène ; la phase 0x10 de
fin de scène attend parfois indéfiniment). Les PV d'un joueur sont au mot `+0x62` de son enregistrement
(`FF85DE + 0x200 k`), ceux d'un boss au mot `+0x62` de l'objet BOSS (`FFBA1E + 0xC0 k`) : ramener ces PV à 1 fait mourir
le boss à la prochaine touche (séquences de mort et de fin de scène), les remettre au maximum rend le joueur invincible.
Trois ou quatre joueurs : l'EEPROM d'origine déclare deux monnayeurs ; `poke8 ff8021 3` et `poke8 ff8020 2` (à l'image 800,
après le chargement des réglages) donnent un monnayeur par joueur et la partie à quatre.

### Générer les scénarios de couverture

`tools/gen_scenarios.py` (bot réactif `port/ddsom-game/examples/autobot.rs`, qui joue en lisant la RAM puis écrit un
scénario d'entrées et de `poke`) produit `scenarios/cov_*.txt` ; `tools/run_cov.py` + `tools/cov_report.py` mesurent la
couverture d'instructions 68000 (exemple `cov`). Résultats et zones non atteintes : `docs/couverture.md`.

## Travailler dans un worktree

Les ROMs et `testdata/` ne sont pas versionnés : depuis un worktree, exporter `DDSOM_ROMS` et
`DDSOM_EEPROM` comme ci-dessus, **dans la même commande que `cargo`** (sinon `ddsom-rc` se recompile,
~8 minutes). Les commandes git passent par `/usr/bin/git -C <worktree>`. La machine est partagée
avec d'autres agents : ne jamais tuer de processus (`pkill`, `kill`) qui ne sont pas les siens.

Commits : un commit par sous-système validé, message en français décrivant ce qui est compris et
réécrit, terminé par la ligne `Co-Authored-By` habituelle.
