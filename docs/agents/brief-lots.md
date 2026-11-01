# Lots de refactorisation A-D (après le socle 0a/0b et le lot E, main c8107db)

Lis d'abord brief_commun.md (même dossier), puis docs/refactorisation.md (§ 2 architecture cible, § 3 lots, § 5-7 pilote, 0a, E, 0b) et docs/guide-refactorisation.md (règles, API commune du lot 0b : traits `machine::Machine` et `objects::Courant`, cœurs et adaptateurs `_68k`, `DDSOM_POISON` par site, check-list). C'est ta référence : imite le pilote `pools` et le lot 0b.

But : code lisible et de qualité, fonctions réutilisables, À COMPORTEMENT IDENTIQUE (effets de bord, ordre des écritures en RAM, glitches, bugs et RNG d'origine conservés ; marque-les `// Glitch d'origine : …`). La validation contre l'original est le filet ; l'état reste dans la RAM émulée.

Dans ton périmètre, dans cet ordre de priorité :
1. J1 : plus d'appel par adresse (`call`/`call_at`/`x.call(0x…)`) entre natives Rust — appels de fonctions Rust ordinaires ; utiliser l'API commune (alloc, anim, motion, lists, cull, rank, random) partout.
2. J2 : plus de registre ni de drapeau hors frontière : arguments et retours Rust (`Option<Obj>`, `bool`, structs) ; prouver avec `DDSOM_POISON` qu'un registre est mort avant de retirer son écriture ; garder un adaptateur `_68k` documenté là où un lecteur hors périmètre relit encore un registre (ex. boucle de dispatch, `reinsert`).
3. Plus de cession par `call_at`/`Unwound`/cadres là où ton périmètre en a (voir § 7 lot E : appelants restants), en suivant le modèle `Etape` du lot E.
4. Nommer l'état : champs d'objet (vues typées `Obj`, constantes de champ nommées), variables RAM (`ram.rs`), constantes de ROM nommées ; supprimer les adresses déclarées en double dans ton périmètre.
5. Factoriser : les familles/sous-modules qui se dupliquent (mesure : tools/mesure_refacto.py) partagent des fonctions communes dans ton périmètre.
6. Supprimer le code mort et les replis vers l'original jamais atteints (g.rom, x.call de repli, `_ =>` vers l'original) : un cas impossible devient un `unreachable!`/panic avec message expliquant pourquoi.

Règles de périmètre : ne modifie que tes fichiers. API communes (objects/, families/cx.rs, families/mod.rs, rng.rs, ram.rs, machine.rs, taches/, mem.rs, lib.rs hors tes lignes NATIVES) : ne pas les changer ; un besoin nouveau va d'abord en privé dans ton module et se signale dans le rapport. Listes NATIVES de lib.rs : ne retire que tes lignes. Deux autres agents travaillent en parallèle sur d'autres lots.

Validation : ciblée pendant le travail (scénarios de ton périmètre ; `examples/fuzz_*` natives contre original si utile), puis complète avant chaque push d'étape importante et avant le dernier commit : `DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_JOBS=8 setsid -w tools/validate_all.sh 30000` (~15 min, mode par défaut sans 68000) ET la même avec `DDSOM_ASSETS=<scratchpad>/sansrom_assets`. 269/269 VALIDE exigé. JOBS=8 (trois agents se partagent 24 cœurs).

Fractionne : commit + push par sous-ensemble validé (une famille, un sous-module), avancement noté dans docs/refactorisation.md (ta ligne). Rapport d'étape si plus de 4 h. Avant ton dernier commit, fusionne main (d'autres lots auront fusionné).

Rapport final court : ce qui est converti, mesures tools/mesure_refacto.py de ton périmètre avant/après (lignes, regs, drap, adr, pile, brut, doublons), adaptateurs `_68k` restants et pourquoi, besoins d'API commune, glitches documentés, validation exacte, hash poussé.

## Code mort : le consigner avant de le retirer (demande de l'utilisateur)
Avant de supprimer du code mort ou un repli jamais atteint, établis ce qu'il faisait et consigne-le dans docs/code-mort.md (une entrée par cas, section de ton lot) : adresse(s) d'origine, ce que fait le code, pourquoi il n'est jamais atteint (aucune référence, condition jamais vraie, valeur de table jamais produite, réglage DIP/EEPROM, version/région), et l'hypothèse sur sa nature (fonction cachée, mode debug, comportement abandonné ou corrigé, reste d'une autre version, simple repli défensif). Cite tes preuves (recherche de références, fuzz, scénario).
Décision par cas :
- repli défensif ou doublon sans intérêt : supprimer, l'entrée du doc suffit ;
- comportement de jeu réel mais inatteignable (fonction cachée, mode debug, attaque/objet/variante non utilisé) : GARDER le code Rust, compilé et lisible (pas en commentaire), regroupé et marqué `// Code mort d'origine : …` avec renvoi vers docs/code-mort.md, et si possible un moyen de l'atteindre (poke/scénario) pour le valider contre l'original.
