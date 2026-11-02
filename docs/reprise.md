# Reprise du travail (état au 2026-10-28)

Point de départ pour la prochaine session. L'historique détaillé des lots est dans [refactorisation.md](refactorisation.md), § 5 à 18 et « Avancement ».

## Où en est le projet

* **Le jeu tourne en Rust pur, sans 68000 et sans ROM.** C'est le mode par défaut de `ddsom`, de `validate` et de `tools/validate_all.sh`. Avec `ddsom --assets <dossier>`, toutes les données viennent des assets produits par `extract`. Voir [sans-rom.md](sans-rom.md).
* **Validation de main.** 296 scénarios VALIDE sur 296, en mode par défaut et avec `DDSOM_ASSETS`. Ce total comprend les 272 scénarios habituels et les 24 bots longs de `scenarios/long/` (option `DDSOM_LONG=1`).
* **Couverture des données.** Sans ROM, elle est complète par construction : `data/reste.json` contient tout ce qui n'est pas du code exécuté (voir [sans-rom.md](sans-rom.md)).
* **Couverture de la logique.**
  * Fonctions et branches jouées : [couverture-fonctions.md](couverture-fonctions.md) et `couverture-branches.tsv`.
  * Chemins non joués : fuzz différentiel `examples/fuzz_couv.rs` (section « Validation par fuzz »).
* **Refactorisation.** Lots 0a, 0b, A à E, S, F, IA, noyau final, puis sortie du Cpu parties 1 et 3, tous fusionnés.
  * Le `Cpu` sert encore de banc de registres à la plupart des natives.
  * Ce qui bloque sa suppression : [refactorisation.md](refactorisation.md) § 18, « Ce qu'il faut encore pour supprimer `m68k::Cpu` ».

## Lots en cours (mise à jour du 2026-11-01)

Aucune branche ouverte : `wt-pile` (partie 4) et `wt-locales` (partie 5, étapes 1-2 et 11 familles d'ennemis) sont fusionnés dans main (edd4e00), 296/296 VALIDE dans les deux modes.

Suite de la sortie du Cpu (détails dans les lignes « partie 4 » et « partie 5 » de refactorisation.md) :
* **Partie 4, point 5** (branche `wt-restes`, validée 296/296 dans les deux modes et empoisonnée `reprise,neuve,pile` sans exemption, non fusionnée) : `ram_du_noyau` resserré aux piles ; `dispatch::Restes` dans `Table::restes` ; `Contexte` supprimé (A0, D4, D7 en champs des étapes, SR utilisateur sans masque).
* **Partie 5, suite** : familles d'ennemis à vraie lecture de registre (squelette D1, trog A1, gargouille D0, golem D0, bête de l'ombre D0), familles pas encore tentées (scarabée, scorpion, harpie, Riyakaa, guerrier, double, part), puis boss, `joueur/`, `set_b`, `effects_c`, enfin `registres_generique.py` et `registres_sans.py`.
* Outils de la partie 5 : `tools/registres/` (`lecteurs.py` liste les lecteurs de registres d'entrée, `generique.py`/`nettoie.py`/`famille.sh` rendent une famille générique, `fuzz.sh`/`compare_fuzz.py` comparent fuzz_couv branche/main ; variables `SCRATCH` et `W`).

## Après ces lots

1. **Retirer `m68k::Cpu` du jeu.** Plus de `Corps::Processeur` : la machine de référence garde son 68000, le jeu n'en a plus.
2. **Le reste de la couverture.**
   * Boutiques et routes 0x14/0x38 par des bots.
   * Chemins de F3 (interface joueurs) hors curseur et noms.
   * Corps des tâches K1, F1, F4, F7, O1 et K2 dans `fuzz_couv`.
   * Sous-états 8 et 0xa des réactions des boss 15 et 19 (non tranché, voir code-mort.md).
3. **Petites dettes.**
   * Les noms et commentaires des bots `long_*` se trompent de personnage. L'ordre réel est 0 combattant, 1 magicien, 2 clerc, 3 voleur, 4 nain, 5 elfe.
   * `zones_natives.txt` hors du code ne sert plus.
   * `examples/cessions` et `reste68k` n'ont plus d'objet depuis le retrait du mode hybride.
4. **Puis :** optimisation, documentation finale, et le bug de saisie du nom. Ce bug existe dans d'autres révisions du jeu, pas dans la nôtre (Euro 960619).

## Méthode qui marche

* **Consignes des agents.** Elles sont dans [agents/brief-commun.md](agents/brief-commun.md) (règles) et [agents/brief-lots.md](agents/brief-lots.md) (mission type d'un lot de refactorisation et règle du code mort). Remplacer `<scratchpad>` par le dossier temporaire de la session.
* **Au plus 3 agents.** Un worktree par agent, avec `git worktree add -b wt-<lot> .claude/worktrees/<lot> main`. Des périmètres de fichiers disjoints. Un seul agent touche les API communes à la fois.
* **Valider avec les bots longs.** Toujours `DDSOM_LONG=1`. Des registres « prouvés morts » sur les 272 scénarios se sont révélés relus par les bots longs, trois fois.
* **Valider sans ROM.** Toujours aussi avec `DDSOM_ASSETS`, sur des assets frais : `cargo run --release -p extract -- rom <dossier>`. Les dossiers d'assets du scratchpad ne survivent pas à la session.
* **Commande.** `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1 DDSOM_JOBS=8 setsid -w tools/validate_all.sh 30000`. Elle prend environ 15 à 30 min et se reprend après un arrêt.
* **Après chaque fusion de deux lots validés séparément**, relancer la validation complète : leurs interactions cassent parfois ce que chacun validait seul.
* **Code mort.** Le consigner dans [code-mort.md](code-mort.md) avant de le retirer. Un vrai comportement de jeu inatteignable se garde en Rust compilé.
