# Règles communes (portage Rust de Shadow over Mystara, dépôt /home/ycr/som)

But du projet : jeu jouable en Rust pur, sans 68000 ni ROM. Méthode : la machine de référence exécute le code 68000 d'origine (surcadencé ×16) ; la machine hybride exécute les routines natives Rust ; `validate` compare RAM, sprites et son image par image. Un scénario est bon s'il finit par « VALIDE ».

- Travaille UNIQUEMENT dans ton worktree (donné dans ta tâche). Ne touche ni à /home/ycr/som (main) ni aux autres worktrees de .claude/worktrees/, où d'autres agents travaillent. Ne fusionne pas dans main.
- Lis d'abord docs/rewrite-guide.md, docs/architecture.md et docs/reste68k.md (inventaire du 68000 restant, à jour). Imite le style des modules voisins déjà réécrits : Rust lisible, français dans les commentaires, état et noms explicites, pas d'exactitude au cycle ni de contrats de registres inutiles (mais rends les registres que l'original relit : voir le guide, DDSOM_ADOPT_REGS).
- Désassemblage : /home/ycr/som/listing.txt (incomplet par endroits), decomp/ (Ghidra), port/ddsom-game/examples (pctrace, watch, dis…).
- Construction : dans port/, `rtk proxy cargo build --release -p ddsom-game --examples` (la sortie cargo est filtrée par rtk sinon). Tests : `rtk proxy cargo test --release --lib`.
- Validation ciblée : `port/target/release/examples/validate /home/ycr/som/rom /home/ycr/som/testdata/eeprom_ddsom.bin <scénario.txt> <images>`. Validation complète obligatoire avant ton dernier commit, toujours avec run_in_background (jamais `&`, jamais au premier plan plus de 10 min) :
  `DDSOM_ROMS=/home/ycr/som/rom DDSOM_EEPROM=/home/ycr/som/testdata/eeprom_ddsom.bin DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_JOBS=6 tools/validate_all.sh 30000` (≈1 h 30, 255 scénarios). Tous doivent être VALIDE.
- Mesure du reste dans ton périmètre : `examples/reste68k` (voir tools/reste68k.sh) sur quelques scénarios qui l'exercent.
- Ne tue jamais de processus que tu n'as pas lancés (pas de pkill). Fichiers temporaires : <scratchpad>/<ton préfixe>_*.
- Pièges : `DDSOM_CHECK=1` seul plante sur vblank (DDSOM_CHECK_ONLY) ; une routine d'origine qui cède la main doit être appelée avec call_at et l'adresse de retour d'origine ; registres résiduels relus par le jeu.
- Commits fréquents sur ta branche, messages en français terminés par la ligne `Co-Authored-By: <ton modèle> <noreply@anthropic.com>` (ex. `Claude Opus 5.5` ou `Claude Sonnet 5.5`), puis `git push origin <ta branche>`. Pas de ROM ni de données extraites dans git.
- Si un sous-ensemble résiste, laisse-le en 68000, documente pourquoi et continue le reste.
- Rapport final court en français : routines réécrites, instructions 68000 restantes dans ton périmètre (avant/après), résultat exact de la validation complète, hash du dernier commit poussé, ce qui reste.

## Code jamais atteint par les scénarios
Quand le moteur n'aura plus de 68000, tout gestionnaire atteignable (par une table de types, de variantes, d'états) mais encore en code d'origine fera planter le jeu. Méthode : recenser les entrées des tables d'aiguillage du périmètre (type → gestionnaire, variante, état) et vérifier pour chacune qu'une native Rust la couvre (lookup des natives par adresse ; examples/reste68k ; `jamais atteintes` de docs/reste68k.md). Pour chaque entrée encore en 68000 : la réécrire en Rust, puis la valider par fuzz différentiel (examples/fuzz_familles.rs avec DDSOM_ENTREE=<adresse>, fuzz_boss.rs, fuzz_enemy.rs… : natives contre original sur états aléatoires) et, si possible, par un nouveau scénario (poke de RAM pour créer l'objet, voir docs/couverture.md) rangé dans scenarios/cov/. Code mort (aucune table ni appel ne le référence) : ne pas le réécrire, le lister.

## Fractionner pour survivre à un arrêt (depuis le 2026-10-26)
- tools/validate_all.sh est reprenable (main 2026-10-26) : résultats dans ~/.cache/ddsom/validate/<empreinte du binaire et des DDSOM_*>, un scénario déjà VALIDE n'est pas rejoué. Après un arrêt, relancer la même commande ne refait que ce qui manque. Récupère le script de main si ta branche ne l'a pas (`git checkout main -- tools/validate_all.sh`, puis commit).
- Lance les validations longues détachées de la session : `setsid -w tools/validate_all.sh 30000 > journal 2>&1` (dans un Bash run_in_background) ; elles survivent si la session meurt.
- Commit (et push) après chaque étape validée, même petite ; note l'avancement dans la doc de ton périmètre pour qu'une reprise sache où tu en es.
- Ne relance pas une validation complète après chaque fusion de main : validations ciblées pendant le travail, UNE validation complète à la fin. JOBS=10 suffit (machine 24 cœurs). Si tu tournes depuis plus de 4 h sans commit de fond, arrête-toi et rends un rapport d'étape.
