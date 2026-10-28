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

## Branches en cours (non fusionnées)

Chacune a été arrêtée proprement. Le dernier commit est poussé, préfixé « EN COURS (non validé) » s'il n'a pas été validé. Sa ligne dans « Avancement » de refactorisation.md dit où elle en est.

| Branche | Worktree | Lot | Suite |
|---|---|---|---|
| `wt-pile` (b9a0702, NON validé) | `.claude/worktrees/pile` | Partie 4 : un retour identifié par une profondeur d'appel et non plus par A7 (fait) ; VBLANK = `Point::Interruption`, plus de PC/SR empilés (fait) ; `Restes` en état nommé (pas commencé) | 1) lire la validation empoisonnée restée en cache (`DDSOM_POISON=reprise,neuve`) ; 2) réduire `Contexte` aux registres relus, nommés ; 3) sonde gratuite hors poison, et poison de FF0166-FF0FFF pour prouver que le jeu ne relit pas la pile (zone déjà ignorée par validate : `ram_du_noyau`) ; 4) `Restes` ; 5) fusionner main, valider |

La partie 2 (`wt-ecrans2`, écrans : `modes/`, `scene/`, crédits, HUD, service) est fusionnée (6163236). Elle a été validée 296/296 dans les deux modes : `adr` 0 ou presque dans ces zones, 14 natives `Memoire`. Il reste les adaptateurs `construire_68k` (D4), `update_cameras_68k` (D4), `recopier_68k` (D0-D4), l'A0 de la sortie de scène, et les natives des phases en `Processeur` (refactorisation.md § 17).
| `wt-locales` (0e2ccf8 étape 1, 1a3aff7 étape 2 NON validée) | `.claude/worktrees/locales` | Partie 5, familles. Étape 1 : `Restes` reçoit D2/D3, et les façades `enemy_lib`/`enemy_common` sont sans processeur. Étape 2 : `reactions.rs` sans registres, deux tables par type en Rust, réaction de la gargouille 0x65236 réécrite (elle manquait) | 1) valider 0e2ccf8 (DDSOM_ASSETS jamais lancé) puis 1a3aff7, dans les deux modes ; 2) `fuzz_couv` C02/C04 contre main ; 3) les fonctions qui lisent un registre de l'appelant, famille par famille (228 candidats ; l'outil `lecteurs.py` était dans le scratchpad, à refaire) ; 4) `registres_generique.py` puis `registres_sans.py` |

Pour reprendre une branche :
1. Fusionner main dans la branche.
2. Relancer la validation complète (elle est reprenable).
3. Fusionner dans main seulement si les deux modes sont à 100 % VALIDE.

## Après ces trois lots

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
