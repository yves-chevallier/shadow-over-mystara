# Reprise du travail (état au 2026-10-28)

Point de départ pour la prochaine session. L'historique détaillé des lots est dans [refactorisation.md](refactorisation.md), § 5 à 18 et « Avancement ».

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
5. **Suite :** optimisation (base : `examples/bench`, refactorisation.md « Performance »), documentation finale, et le bug de saisie du nom. Ce bug existe dans d'autres révisions du jeu, pas dans la nôtre (Euro 960619). Plan détaillé : [fin.md](fin.md).

## Méthode qui marche

* **Consignes des agents.** Elles sont dans [agents/brief-commun.md](agents/brief-commun.md) (règles) et [agents/brief-lots.md](agents/brief-lots.md) (mission type d'un lot de refactorisation et règle du code mort). Remplacer `<scratchpad>` par le dossier temporaire de la session.
* **Au plus 3 agents.** Un worktree par agent, avec `git worktree add -b wt-<lot> .claude/worktrees/<lot> main`. Des périmètres de fichiers disjoints. Un seul agent touche les API communes à la fois.
* **Valider avec les bots longs.** Toujours `DDSOM_LONG=1`. Des registres « prouvés morts » sur les 272 scénarios se sont révélés relus par les bots longs, trois fois.
* **Valider sans ROM.** Toujours aussi avec `DDSOM_ASSETS`, sur des assets frais : `cargo run --release -p extract -- rom <dir>`. Les dossiers d'assets du scratchpad ne survivent pas à la session.
* **Commande.** `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1 DDSOM_JOBS=8 setsid -w tools/validate_all.sh 30000`. Elle prend environ 15 à 30 min et se reprend après un arrêt.
* **Après chaque fusion de deux lots validés séparément**, relancer la validation complète : leurs interactions cassent parfois ce que chacun validait seul.
* **Code mort.** Le consigner dans [code-mort.md](code-mort.md) avant de le retirer. Un vrai comportement de jeu inatteignable se garde en Rust compilé.
