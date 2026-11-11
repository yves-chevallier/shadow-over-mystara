# Reprise du travail (état au 2026-11-03)

Point de départ pour la prochaine session. L'historique détaillé des lots est dans [refactorisation.md](refactorisation.md), § 5 à 18 et « Avancement ».

## Point d'arrêt du 2026-11-03 (reprise la semaine suivante)

**main = e489feb** : lots L0 à L13 et L19 fusionnés, **309/309 VALID dans les deux modes** (ROMs et `DDSOM_ASSETS`),
aucun 68000 ni Z80 dans le jeu, code en anglais, 0 avertissement. Bench bot30k : ~19 600 im/s sans rendu, ~3 900 avec.
Toutes les familles sont à 0 accesseur ROM et 0 décalage littéral ; détails lot par lot dans [fin.md](fin.md).

**Interrompu : L14 (le jeu sans image ROM)**, branche `wt-l14-sans-rom` (commit 4eebd47, poussé, **non validé, peut ne
pas compiler**). Contenu : `rebuild` retiré de `ddsom-assets`, `remainder` déplacé dans `ddsom-data`, résidu et
débordements de l'original nommés (`ddsom-data/src/{residue,overreads}.rs`, `extract/src/overreads.rs`,
`extract/overreads.txt`), tables du son listées (`extract/sound_tables.txt`), `cps2` bus/lib/sound en chantier.

**Pour reprendre :**
1. Recréer le worktree : `git worktree add .claude/worktrees/l14-sans-rom wt-l14-sans-rom` (copier `rom/` et
   `testdata/`, ignorés par git, sinon `mame_snapshots` se saute). Toujours exporter `DDSOM_ROMS`.
2. Faire compiler, puis finir L14 selon [fin.md](fin.md) « L14 » : machine du jeu sans ROM, `ddsom --assets` via
   `GameData::load` + banque d'échantillons + atlas, `cps2::Contents` sans région Z80 ni programme DSP, `Residue`
   consulté 0 fois sur les 309 scénarios et `fuzz_couv`. Validation complète dans les deux modes, `render_check`,
   bench pas plus lent.
3. Puis L15 (World + couches ; `tasks::table` passe par `dyn Any`, coût à supprimer), L16/L17 facultatifs, L18
   (documentation finale), puis L20 (cœur en bibliothèque), L21 (façade), L22 (pack HD).

**Décisions en attente de l'utilisateur :**
* façade de présentation (L21) : Bevy ou minimal (wgpu / macroquad) ;
* `reste.json` : le retirer une fois `Residue` à zéro, ou le garder comme assurance (L14 donnera le compte) ;
* bug de saisie du nom : absent de notre révision (L19) ; option `DDSOM_*` pour reproduire celui des anciennes
  révisions, ou rien ;
* `touches.cfg` en anglais avec compatibilité ; écart connu du Z80 en retard (2 scénarios, comparaison du son
  suspendue) à accepter.

## Où en est le projet

* **Le jeu tourne en Rust pur, sans 68000 et sans ROM.** C'est le mode par défaut de `ddsom`, de `validate` et de `tools/validate_all.sh`. Avec `ddsom --assets <dir>`, toutes les données viennent des assets produits par `extract`. Voir [sans-rom.md](sans-rom.md).
* **Validation de main.** 296 scénarios VALIDE sur 296, en mode par défaut et avec `DDSOM_ASSETS`. Ce total comprend les 272 scénarios habituels et les 24 bots longs de `scenarios/long/` (option `DDSOM_LONG=1`).
* **Couverture des données.** Sans ROM, elle est complète par construction : `data/reste.json` contient tout ce qui n'est pas du code exécuté (voir [sans-rom.md](sans-rom.md)).
* **Couverture de la logique.**
  * Fonctions et branches jouées : [couverture-fonctions.md](couverture-fonctions.md) et `coverage-branches.tsv`.
  * Chemins non joués : fuzz différentiel `examples/fuzz_couv.rs` (section « Validation par fuzz »).
* **Refactorisation.** Lots 0a, 0b, A à E, S, F, IA, noyau final, puis sortie du Cpu parties 1 et 3, tous fusionnés.
  * Sortie du Cpu achevée (parties 1 à 7, étape finale § 20, `wt-sans-cpu`) : le jeu n'a plus de `m68k::Cpu` ; le
    68000 ne sert plus qu'à la machine de référence et aux outils (`ddsom_game::tools`).

## Lots en cours (mise à jour du 2026-11-01)

* **L3, son en tableaux indexés** (branche `wt-l3-son`, 2026-11-02) : `ddsom_sound::SoundData` remplace la région
  d'octets du pilote, `music.json` change de format (**réextraire `music`**), `qsound::DspTables` ; validé 308/308
  dans les deux modes après fusion de main 39be0e4, non fusionné. Voir [fin.md](fin.md) (L3) et [son.md](son.md)
  « Tableaux indexés ».
* **L4, rendu et mixage depuis les assets** (branche `wt-l4-rendu`, 2026-11-02) : atlas de tuiles
  `cps2_gfx::TileAtlas` (cellules 8 bits lues telles quelles dans `gfx/cells_NN.png`), rendu des couches par lignes
  de tuiles, banque d'échantillons `ddsom_assets::samples::bank`, boucle des voix et filtres QSound vectorisés ;
  `bench` bot30k ~15 500 images/s sans rendu, ~3 300 avec ; `render_check` identique. Voir [fin.md](fin.md) (L4).

* **L6, SET** (branche `wt-l6-set`, 2026-11-02) : SET découpés par type (`families/set/`, `set_b/`, `set_c/`),
  tables dans le nouveau `data/set.json` et `data/dialogs.json` chargés dans `GameData` (**réextraire `data`**),
  vues `record!` (`*/fields.rs`), variables A5 dans `set/vars.rs` ; accesseurs ROM, décalages littéraux et
  `A5 + littéral` à 0 dans la zone set. Validé 308/308 dans les deux modes après fusion de main 334316e ; non
  fusionné. Voir [fin.md](fin.md) (L6, « Résultat »).

Aucune branche ouverte : `wt-pile` (partie 4) et `wt-locales` (partie 5, étapes 1-2 et 11 familles d'ennemis) sont fusionnés dans main (edd4e00), 296/296 VALIDE dans les deux modes.

Suite de la sortie du Cpu (détails dans les lignes « partie 4 » et « partie 5 » de refactorisation.md) :
* **Partie 4, point 5** (branche `wt-restes`, validée 296/296 dans les deux modes et empoisonnée `resume,new,stack` sans exemption, non fusionnée) : `kernel_ram` resserré aux piles ; `dispatch::Leftovers` dans `Table::leftovers` ; `Contexte` supprimé (A0, D4, D7 en champs des étapes, SR utilisateur sans masque).
* **Partie 5, suite** (branche `wt-familles2`) : toutes les familles d'ennemis et les boss 0, 1, 2/22, 4, 6, 7, 10, 13, 16, 17, 18, 21 sont en gestionnaires `#[without]` (voir la ligne « partie 5 »). Restent les boss 3, 5, 8, 9, 11, 15, 19, puis `player/`, `set_b`, `effects_c`, enfin `regs_generic.py` et `regs_without.py`.
* Outils de la partie 5 : `tools/regs/` (`readers.py` liste les lecteurs de registres d'entrée, `generic.py`/`clean.py`/`family.sh` rendent une famille générique, `fuzz.sh`/`compare_fuzz.py` comparent fuzz_couv branche/main ; variables `SCRATCH`, `W` et `P` (préfixe des fichiers temporaires)). Exporter `DDSOM_ROMS` pour tout `cargo` (sinon `ddsom_rc` se recompile, ~12 min).

## Après ces lots

0. **Traduire tout le code en anglais** (règle de l'utilisateur, 2026-11-02 : jamais de français dans le code). Identifiants (renommage outillé, module par module) et commentaires ; comportement inchangé, validation complète comme filet. À lancer quand aucune autre branche n'est ouverte (conflits partout sinon).
   **Fait** (branche `wt-anglais1`, après fusion de main 53f80d6) : tout `port/` (families compris) et `tools/` sont en anglais :
   * identifiants renommés par un outil par jeton (code, commentaires entre accents graves, captures de format ; contrôle des
     collisions par fonction) : ~2 300 identifiants, modules et fichiers (`taches/` → `tasks/`, `evenements` → `events`,
     `demarrage` → `startup`, `families/joueur/` → `families/player/`, `Restes` → `Leftovers`, `Etape` → `Step`,
     `Aiguillage` → `Dispatch`, `Banc` → `Bench`, `*_par` → `*_via`...), exemples (`couverture` → `coverage`,
     `fuzz_joueur` → `fuzz_player_shots`...), scripts de `tools/` (`registres/` → `regs/`...), `jouer.sh` → `play.sh` ;
   * commentaires, messages et noms des natives traduits ; étiquettes `DDSOM_POISON` en anglais (`exit`, `resume`, `new`,
     `stack`, `handback`, `call_<adresse>`, `effects`, `leftovers`, `all`) ; verdicts `VALID`/`FAILED` (les outils
     acceptent aussi `VALIDE`) ;
   * gardés en français par compatibilité : noms des variables d'environnement (`DDSOM_*`, `FUZZ_*`), clés JSON de
     `data/reste.json` (`serde(rename)`), noms de fichiers d'assets et de sauvegarde, format de `touches.cfg` (actions
     `j1.haut`..., touches `Pad1Haut`...), étiquettes de lecteur de `native_zones.txt` (noms d'assets), textes des docs
     générés par `examples/creatures.rs` et `examples/level_maps.rs` (docs/creatures.md, docs/niveaux.md restent en
     français), renvois aux fichiers de docs/. Validé 308/308 dans les deux modes (8812560, DDSOM_LONG=1) ; non fusionné.

1. ~~**Retirer `m68k::Cpu` du jeu.**~~ Fait (branche `wt-sans-cpu`, refactorisation.md § 20) : le jeu n'a plus de
   processeur. Les natives à arguments en registres, la machine de référence, l'exécution des natives par le 68000,
   `DDSOM_CHECK` et les sondes de registres sont dans `ddsom_game::tools` (fonctionnalité cargo `tools`, active pour
   les exemples) ; `ddsom-game` sans `tools` et le binaire `ddsom` ne dépendent plus de `m68k` ni de `ddsom-rc`
   (`cargo tree -p ddsom`). `DDSOM_CHECK=1` compare chaque gestionnaire d'objet à son original (et échoue s'il ne
   compare rien). Non fusionné.
2. ~~**Le reste de la couverture.**~~ Fait par le lot « couverture » (`wt-couv2`, [couverture.md](couverture.md),
   « Votes, boutiques, interface et démarrage ») :
   * votes de chemin (sorties des scènes 2, 13, 33 et 38, en décimal : les « routes 0x14/0x38 » étaient les scènes 20
     et 56, sans vote) et boutiques joués par des bots (`long_votes_*`, `long_boutiques_*`, `autobot --wander`) ;
   * F3 hors curseur et noms : tout joué (`long_credits_*`, `long_boss_4p_frail`, bornes à trois joueurs), sauf du code
     mort (0x15234) ;
   * corps des tâches K1, F1, F4, F7, O1, K2 : les chemins atteignables sont joués par des scénarios
     (`cov_demarrage_boutons`, `long_noms_embleme`), le reste est inatteignable (drapeaux de la ROM, carte, erreurs de
     RAM, données) : [code-mort.md](code-mort.md), « Lot couverture » ;
   * boss 15 et 19, sous-états 8 et 0xa : inatteignables, gardés en Rust comme l'original et validés par `fuzz_couv` ;
   * trou trouvé et corrigé : la résurrection d'un joueur qui rejoint pendant que les entrées sont figées (0x23c7a).
   * validé 308/308 dans les deux modes (04d7b47) ; non fusionné dans main.
3. ~~**Petites dettes.**~~ Faites (`wt-couv2`) :
   * noms et commentaires des bots `long_*` d'après les personnages relevés en RAM (`long_voleur_mortel`,
     `long_elfe_objets`, `long_nain_57_86`, `long_scene_20`, `long_scene_56`...) ;
   * `native_zones.txt` gardé : `extract` l'inclut (`natives.rs`) pour les zones lues dans le code ; ses zones hors du
     code font double emploi avec `reste.json` aujourd'hui, mais protègent si `code_68k.txt` grandit (docs/sans-rom.md) ;
   * `examples/reste68k`, `tools/reste68k.sh` et `docs/reste68k.md` retirés (`examples/cessions` l'était déjà).
4. **Son : le pilote Rust seul pilote du jeu** (branche `wt-son`, [son.md](son.md) § 13) :
   * `ddsom` (avec ou sans ROM) n'a plus de Z80 : `cps2` a une fonctionnalité `z80` (machine de référence et outils,
     activée par `ddsom-game/tools`) ; `cargo tree -p ddsom -e normal` ne contient plus `z80`. `DDSOM_SON_RUST` retiré.
   * `validate` compare désormais le son : chaque registre QSound et octet de RAM partagée doit recevoir les mêmes
     valeurs du pilote Rust (machine du jeu) et du Z80 (référence), à 4 images près ; la comparaison est suspendue
     quand le Z80 en retard de plusieurs périodes réordonne une commande et un tic (écart connu : ordre différent
     dans 10 scénarios sur 308, valeurs différentes dans 2), jusqu'à ce que les deux pilotes aient le même état.
   * Validé 308/308 dans les deux modes (`DDSOM_LONG=1`, assets réextraits), non fusionné.
   * Écart corrigé : la définition des deux sons ADPCM perdait un octet dans `music.json` (son faux sans ROM) :
     **réextraire ses assets** (`extract <roms> <assets> music`).
   * Le pilote lit ses données par accesseurs nommés (`ddsom_sound::Rom`) ; elles restent rangées à leurs adresses
     d'origine (voir [sans-rom.md](sans-rom.md) « Ce qui manque »).
5. ~~**Nettoyage.**~~ Fait (branche `wt-nettoyage`, refactorisation.md § 21) : plus aucun `allow` de code mort, les
   restes de registres lus par les seuls outils sont reconstruits dans `tools`, `cargo build` et `cargo clippy` sans
   avertissement (avec et sans `tools`), `fuzz_enemy families` sans écart. Reste à factoriser les exemples de fuzz
   recopiés (`fuzz.rs`, `fuzz_enemy.rs`, `fuzz_eshl_c.rs`, `fuzz_set_a.rs` partagent l'essentiel de leur code).
6. **Suite : la fin** — plan détaillé et lots ordonnés dans [fin.md](fin.md). **Série 0 faite** (branche
   `wt-serie0`, non fusionnée) : L0 (`tools/measure_end.py`, mesure par lecteur `tools/rom_readers.py`), L1 (crate
   `ddsom-data`, `GameData`/`data()`, `extract` en bibliothèque, images, animations et vitesses typées, résidu
   compté `tools/residue.py`, macro `record!`), L2 (sons nommés `sound::Sound`). Les vagues suivantes partent de là :
   voir les paragraphes « Résultat » de L0-L2 dans fin.md. Réextraire les assets (`speeds.json`, `extra` de
   `frames.json`, `character_sets` d'`anims.json`). Suite (données typées, jeu sans image ROM, découpage, optimisation, cœur de jeu en bibliothèque + façade de présentation, pack HD, documentation finale, saisie du nom).

7. **Lot L5 (effets)** fait (branche `wt-l5-effets`, [fin.md](fin.md) « L5 », « Résultat ») : `families/effects/`
   découpé (plus gros fichier 881 lignes), tables typées `data/effects.json` (204 tables, `tables!` de
   `ddsom-data/src/effects.rs`), champs en `record!` et variables nommées : zone `effects` à 0 accesseur ROM,
   0 décalage littéral, 0 `A5 + littéral`, 0 constante d'adresse ROM. **Réextraire les assets**
   (`data/effects.json`). Pour les lots suivants : `effects::target` ne lit plus la ROM (rend la case de la
   table, `items.rs` l'appelle encore) ; `Missing::byte_in/word_in/long_in` dans `data.rs` ; `PlayerView`
   (`effects/records.rs`) est à fusionner avec la fiche de joueur de L10 ; les extraits de palettes de
   `effects.json` sont à remplacer par la bibliothèque typée (L12).
8. **Lot L8a (façades des ennemis)** fait (branche `wt-l8a-ennemis`, [fin.md](fin.md) « L8 », « Résultat L8a ») :
   `reactions.rs` découpé en `families/reactions/` (plus gros fichier des façades 683 lignes), `enemy_common/`
   (`records.rs` : vue `Enemy`, `tables.rs`, `vars.rs`), tables ajoutées à `data/enemies.json`
   (`extract/src/enemies.rs`, `GameData::enemies`) : façades à 0 accesseur ROM, 0 décalage littéral,
   0 `A5 + littéral`, 0 constante d'adresse ROM. **Réextraire les assets** (`data/enemies.json`). `random` et
   `rom32` d'`enemy_common` remplacés par `x.random()` et `x.prog32()` chez les appelants (`enemy_*`, `boss_*`).
   Pour L8b : voir « Pour L8b » dans fin.md (une ligne par table dans `extract/src/enemies.rs`, lecture par
   `enemy_common::tables`, champs communs d'`Enemy`).
   **Lot L8b (familles d'ennemis)** fait (branche `wt-l8b-ennemis`, [fin.md](fin.md) « Résultat L8b ») :
   `enemy_riyakaa/` et `enemy_double/` découpés, 161 tables des familles dans `data/enemies.json`
   (`extract/src/enemies.rs`, `families`), un `record!` par famille, `Enemy` complété (paramètres d'IA) : zone
   `enemies` à 0 accesseur ROM, 0 décalage littéral, 0 `A5 + littéral`, 0 constante d'adresse ROM, plus gros
   fichier 793 lignes. **Réextraire les assets** (`data/enemies.json`).

   **Lot L9a (boss 0 à 9, `boss_common`)** fait (branche `wt-l9a-boss`, [fin.md](fin.md) « Résultat L9a ») : 130
   tables des boss dans `data/enemies.json` (nouveau `extract/src/bosses.rs`), `boss_common/` en dossier (`records` :
   `Boss`, `Message` ; `vars` ; `warrior` : aides communes des boss 7 et 8), un `record!` par boss, aiguillages des
   boss 7 et 8 en `match` : fichiers de L9a à 0 accesseur ROM, 0 décalage littéral, 0 `A5 + littéral`, 0 constante
   d'adresse ROM. **Réextraire les assets** (`data/enemies.json`). Pour L9b : « Pour L9b » dans fin.md, outils
   `tools/boss_refactor/`.
   **Lot L9b (boss 10 à 22, dragon)** fait (branche `wt-l9b-boss`, [fin.md](fin.md) « Résultat L9b ») : 138 tables
   de plus dans `data/enemies.json` (décisions des boss 17, 18, 21, chaîne du boss 11 extraite, oscillation et seuils
   du dragon...), un `record!` par boss, variables A5 nommées, `set_abs8`/`A5_DEATH` retirés : **toute la zone
   `bosses`** à 0 accesseur ROM, 0 décalage littéral, 0 `A5 + littéral`, 0 constante d'adresse ROM. **Réextraire les
   assets** (`data/enemies.json`).

8. **Lot L7 (ESHL, SOBJ, objets, caisses)** fait (branche `wt-l7-objets`, [fin.md](fin.md) « L7 », « Résultat ») :
   `families/eshl*/`, `sobj/`, `items/`, `boxes/` découpés par type (plus gros fichier 649 lignes), tables typées
   `data/objects.json` (172 tables, `ddsom-data/src/objects.rs`), champs en vues `view!`/`record!` et variables
   nommées (`families/shell/{tables,records,vars}.rs`) : zone à 0 accesseur ROM, 0 décalage littéral,
   0 `A5 + littéral`, 0 constante d'adresse ROM. **Réextraire les assets** (`data/objects.json`). Pour les lots
   suivants : la macro `tables!` de `ddsom-data/src/effects.rs` sert à toute famille de tables nommées
   (`tables! { struct Nom; ... }`, `extract::effects::read_table`, `rebuild::put_named`) ; `GameData::sprite_tables`
   (`sprites/tables.json`) est chargé (boîtes de collision, tir ESHL 5) ; `effects::target` et `outside_table`
   n'existent plus (`effects::bad_index`) ; `PlayerView` de `shell/records.rs` (comme celle de L5) est à fusionner
   avec la fiche de joueur de L10, `Host` avec les fiches d'ennemis et de boss (L8, L9).

9. **Lot L10 (joueurs)** fait (branche `wt-l10-joueurs`, [fin.md](fin.md) « L10 », « Résultat ») : `player/hurt`,
   `cinematics`, `moves` et quatre personnages découpés (plus gros fichier de la zone 916 lignes), nouveau
   `data/player_tables.json` (311 tables, `ddsom-data/src/player_tables.rs`) et `data/players.json`,
   `data/combat.json` chargés dans `GameData` (**réextraire les assets**), lecture par `player/tables.rs` (par nom ou
   par adresse, mots alignés comme le bus) ; une seule fiche `record!` `Player` (vues `PlayerView` de L5 et L7,
   `PlayerFields` de L6 et `Prey` de L8b fusionnées), vues `Shot` et `Victim` des tirs et effets : zone `players` à
   0 accesseur ROM, 0 décalage littéral, 0 `A5 + littéral` ; restent 141 constantes d'adresses de tables lues par
   adresse.
9. **Lot L11 (écrans)** fait (branche `wt-l11-ecrans`, [fin.md](fin.md) « L11 », « Résultat ») : `modes/`, `hud/`,
   `service/`, `scene/`, crédits, textes et bulles sans accesseur ROM, sans constante d'adresse ROM, sans décalage
   littéral ni `A5 + littéral`, 3 `rd*`/`wr*` bruts (contre 1 637) ; nouvelle couche `hw/` (`hw::video`, `hw::io`),
   `screens/` (`tables`, `vars`, `obj`, `block`), `data/screens.json`, et `GameData` charge `scenes.json`,
   `misc.json`, `texts.json`. **Réextraire les assets** (`data/screens.json`). Pour les lots suivants : écrire la
   vidéo par `hw::video` (`Cell`, `palette`, `regs`), lire une donnée désignée par adresse par `screens::tables` (ou
   la table de sa famille), `screens::vars` pour les caméras et scrolls partagés.

10. **Lot L19 (saisie du nom)** fait (branche `wt-l19-nom`, [fin.md](fin.md) « L19 », [modes.md](modes.md) § 6) : le
   débordement du nom des anciennes révisions (nom par défaut de six lettres puis lettres en série) n'existe pas dans
   la nôtre (verrou posé par `0xc9d16` en `0xc9d5c`) ; scénario `scenarios/modes/name_entry_overflow.txt` VALIDE,
   validation à 309 scénarios. Aucune option `DDSOM_*` ajoutée (décision de l'utilisateur si l'on veut reproduire le
   bug).

11. **Lot L12 (moteur et communs des familles)** fait (branche `wt-l12-moteur`, [fin.md](fin.md) « L12 »,
   « Résultat ») : `ai_aids/` et `camera_modes/` découpés ; nouveau `data/engine.json` et `GameData` charge aussi
   `levels.json`, `terrain.json`, `scene_palettes.json`, `palettes.png`, `rest.json`, `boxes.json` (**réextraire les
   assets**) ; `crate::tables` (`Table`, `Tables`, `engine()`, `engine_table!`, `boxes()`) ; `Enemy` dans
   `objects/enemy.rs`, champs communs dans `Obj`, `scroll::Camera`, variables partagées dans `ram.rs`. Zone du lot
   sans accesseur ROM, sans décalage littéral ni `A5 + littéral`, 11 `rd*`/`wr*`. Pour la série 5 : L13 (`dispatch`,
   `tasks/game` lisent encore la ROM : `Machine::prog16`), L14 (le jeu sans image ROM : seul le résidu lit encore le
   bus), L15 (`World`, couches : `tables` et `objects` sont de la couche monde, `ai_aids`/`effects_aids` dépendent
   encore de `families`).

12. **Lot L13 (aiguillage sans adresses)** fait (branche `wt-l13-aiguillage`, [fin.md](fin.md) « L13 »,
   « Résultat ») : les dix tables d'aiguillage sont des tableaux Rust `[Handler; N]` (`dispatch.rs`, indexés par
   `Obj::KIND`, entrées mortes `Handler::Dead`, types hors table `unreachable!`), `handlers!` nomme chaque
   gestionnaire ; les tâches sont un `enum tasks::Task` (fondus : `tasks::fading::Fade`), `Task::address()` pour
   `gaps` ; `lookup`, le crible, `all_natives`/`game_natives`/`is_native` sont dans `tools::registry` : le jeu n'a
   plus de registre par adresse. Pas de réextraction des assets. Pour L14 : les clés d'adresses de routines lues
   dans les données (`decide` des boss, `run_at` des joueurs, réactions communes, `sound::OTHERS`), les lectures par
   adresse (`held16`, `speed16`, `reactions`, `screens::tables::Rom`, tables des joueurs) et les accesseurs ROM de
   `config`, `mem`, `data` (résidu).

## Méthode qui marche

* **Consignes des agents.** Elles sont dans [agents/brief-commun.md](agents/brief-commun.md) (règles) et [agents/brief-lots.md](agents/brief-lots.md) (mission type d'un lot de refactorisation et règle du code mort). Remplacer `<scratchpad>` par le dossier temporaire de la session.
* **Au plus 3 agents.** Un worktree par agent, avec `git worktree add -b wt-<lot> .claude/worktrees/<lot> main`. Des périmètres de fichiers disjoints. Un seul agent touche les API communes à la fois.
* **Valider avec les bots longs.** Toujours `DDSOM_LONG=1`. Des registres « prouvés morts » sur les 272 scénarios se sont révélés relus par les bots longs, trois fois.
* **Valider sans ROM.** Toujours aussi avec `DDSOM_ASSETS`, sur des assets frais : `cargo run --release -p extract -- rom <dir>`. Les dossiers d'assets du scratchpad ne survivent pas à la session.
* **Commande.** `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1 DDSOM_JOBS=8 setsid -w tools/validate_all.sh 30000`. Elle prend environ 15 à 30 min et se reprend après un arrêt.
* **Après chaque fusion de deux lots validés séparément**, relancer la validation complète : leurs interactions cassent parfois ce que chacun validait seul.
* **Code mort.** Le consigner dans [code-mort.md](code-mort.md) avant de le retirer. Un vrai comportement de jeu inatteignable se garde en Rust compilé.
