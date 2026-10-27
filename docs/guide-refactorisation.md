# Guide pratique de la refactorisation

Pour les agents qui refactorisent un lot ([refactorisation.md](refactorisation.md) § 3). Il complète
[rewrite-guide.md](rewrite-guide.md) (conversion du 68000, terminée) : ce qui y est dit des registres et des natives
reste vrai à la frontière, mais le but est maintenant de **faire disparaître la frontière**.

## Règles

1. **Le comportement ne change pas.** Mêmes écritures, au même endroit, dans le même ordre ; mêmes tirages du hasard ;
   mêmes glitches. Ce qui paraît un bug de l'original reste, marqué :
   `// Glitch d'origine : <ce qui se passe, et quand>`. Un écart déjà assumé (plafond de 64 essais d'une allocation
   que l'original répète sans fin...) se marque `// Écart assumé : …` et se cite dans le rapport.
2. **Un pas = une validation.** Petits commits, chacun validé (ciblé pendant le travail, complet en fin de lot). Ne
   jamais mélanger un changement de forme et un changement de logique dans le même pas.
3. **Une fonction Rust appelée par du Rust n'a pas de contrat de registres.** Arguments et résultat en types Rust :
   `Option<Obj>` plutôt que A1 + retenue, `bool` plutôt que Z, `enum` plutôt qu'un octet d'état.
4. **Les registres ne disparaissent qu'avec une preuve.** Tant qu'on ne sait pas qu'aucune suite ne relit un registre que
   l'original laissait, on garde l'adaptateur `_68k` qui l'écrit ; la preuve est `DDSOM_POISON` (plus bas) plus la
   validation complète.
5. **Une seule déclaration par adresse ou variable.** On ne recopie pas `const ALLOC_SET: u32 = 0x2348` dans son module :
   on appelle la fonction (`pools::SET.alloc`), ou on importe la constante de son propriétaire.
6. **Noms.** Champs et variables nommés d'après `docs/*.md` (vue `Obj`, `Player`, `ram::SCENE`) ; un sens incertain est
   marqué (h). Commentaires en français, brefs ; on garde la langue des identifiants du module.
7. **Code mort et replis.** Un bras `_ => x.call(0x…)` ou un `deleguer` jamais atteint (vérifier : `examples/reste68k`,
   compteurs d'appels de `validate`, `DDSOM_SANS_68K=1` qui s'arrête sur une adresse sans native) se remplace par la
   native appelée directement, ou par `unreachable!("…")` documenté si l'aiguillage de la ROM ne peut pas y mener.
   Ne pas inventer de logique pour un chemin jamais exercé : le dire.
8. **Rester dans son lot.** Ne pas modifier `lib.rs`, `families/mod.rs`, `families/cx.rs`, `objects/`, `mem.rs`, `ram.rs`,
   `diag.rs`, `taches/table.rs` hors du lot 0 ; un besoin d'API se code d'abord dans son module et se signale.

## Convention de nommage des couches

| Forme | Exemple | Sens |
|---|---|---|
| cœur | `Pool::alloc(&self, b) -> Option<Obj>` | logique pure sur l'état ; aucun `Cpu` |
| adaptateur | `Pool::alloc_68k(&self, c, b) -> Option<Obj>` | cœur + registres et drapeaux de l'original ; appelle `diag::registres_morts` |
| méthode de contexte | `Cx::alloc(&mut self, pool)` | raccourci pour les gestionnaires ; appelle l'adaptateur tant qu'il existe, le cœur ensuite |
| native | `pools::ALLOC_SET` (`entry: 0x2348`) | appels par adresse restants : adaptateur puis `rts` ; jamais empoisonnée |

`tools/mesure_refacto.py` compte `regs`, `drap`, `adr`... par zone : un lot les fait baisser ; compter aussi
`rtk proxy grep -rn "_68k(" src | wc -l`.

## Registres morts : `DDSOM_POISON`

Un adaptateur appelle, après avoir posé les registres de l'original :

```rust
crate::diag::registres_morts(c, "alloc", regs::a(1) | regs::d(6) | regs::CCR);
```

Avec `DDSOM_POISON=alloc` (plusieurs étiquettes séparées par des virgules, `tout` pour toutes), ces registres sont
**inversés** juste après : toute lecture ultérieure obtient une valeur fausse. Les natives (appels par adresse) n'empoisonnent
jamais. Méthode :

1. convertir les appelants Rust vers l'adaptateur, valider (comportement identique par construction) ;
2. valider avec `DDSOM_POISON=<étiquette>` (complet, ou au moins les scénarios qui exercent la routine, dans les deux
   modes) ;
3. VALIDE partout : remplacer l'adaptateur par le cœur chez ces appelants, supprimer l'appel à `registres_morts` ;
   un écart : il désigne le lecteur (image, adresse). Garder l'adaptateur à cet endroit, avec un commentaire qui nomme
   le lecteur (« A1 relu par le gestionnaire suivant, 0x…  »), et continuer ailleurs.

`DDSOM_POISON` change l'empreinte des résultats de `tools/validate_all.sh` (variables `DDSOM_*`) : pas de mélange avec
les résultats ordinaires.

## Exemple : l'allocation d'objets (pilote)

Avant (une variante parmi une vingtaine, chaque fichier avait sa constante et souvent son `alloc`) :

```rust
const ALLOC_EFFECT: u32 = 0x2318;
…
    x.call(rom::ALLOC_EFFECT);
    if x.c.sr & m68k::SR_C != 0 {
        return;
    }
    let blink = Obj(x.c.a[1]);
```

Après :

```rust
    let Some(blink) = x.alloc(&pools::EFFCT) else { return };
```

L'original qui ne teste pas l'échec garde son glitch, visible :

```rust
    // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
    let Obj(spawned) = x.alloc_unchecked(&pools::BOSS);
```

Une table de la ROM qui désigne le pool par l'adresse de sa routine passe par `pools::pool_of_alloc_routine(adresse)`.
Libération : `x.call(sub::FREE_SET)` devient `x.free(&pools::SET)`.

## Exemple : une tâche qui cède

Avant (`hud/mod.rs`) : la reprise est une native à l'adresse d'origine, la cession un `tail` vers l'enveloppe `0xb64`.

```rust
resume!(AFTER_WAIT, at::AFTER_WAIT, |x| tour(x));
fn tour(x: &mut Ctx) -> Flow { …; x.tail(at::WAIT_ONE_FRAME, at::AFTER_WAIT) }
```

Cible (lot E, après l'API de `taches/table.rs`) :

```rust
enum Etape { Entree, AttentePartie, Tour }
fn pas(etat: &mut Interface, j: &mut Jeu) -> Attente {
    match etat.etape {
        Etape::Tour => { tour(j); Attente::Images(1) }
        …
    }
}
```

L'adresse d'origine reste une méthode (`Etape::adresse`) tant que `validate` compare la table des tâches à FF0064.

## Pièges connus

* **Exécution à blanc** (mode normal, jusqu'au lot 0a) : une logique de `families::Ctx` qui cesse d'appeler une routine par
  son adresse doit la déclarer externe (`Ctx::externe`), sinon le calcul de durée parcourt la routine d'origine et panique
  « branchement sans décision ».
* **Registres hérités** : le gestionnaire suivant de la boucle de dispatch, une tâche neuve, une suite de `resume!`
  héritent des registres. C'est ce que `DDSOM_POISON` attrape ; `DDSOM_ADOPT_REGS`/`DDSOM_CHECK_ALL` aident à trouver
  lequel.
* **`Cx::call` posait A0 = objet et A5** avant chaque appel : un appel converti ne le fait plus. Si une suite relit A0
  après qu'un gestionnaire l'a changé, la validation le montre ; remettre `x.c.a[0] = x.o.0` à cet endroit, commenté.
* **Noms faux hérités** : vérifier une constante par sa valeur, pas par son nom (`ALLOC_BOX = 0x23a8` allouait un BOSS).
* **Remplacements automatiques** : un script qui réécrit des identifiants par leur valeur doit exclure les constantes
  homonymes d'autres types (`dispatch::PSHL` est une `Class`) ; relire le diff.
* **Une validation lancée recompile** : ne pas modifier les sources pendant le `cargo build` de `validate_all.sh`.

## Check-list d'un lot

1. Mesurer avant : `tools/mesure_refacto.py` (zones du lot), `--consts`, `--doublons 6` ; noter dans la doc du lot.
2. Lister les appels par adresse du lot et leurs cibles (`rtk proxy grep -rn "call(" <rép>`), les replis `_ =>`, les
   `deleguer`, les contextes et helpers dupliqués.
3. Pour chaque routine cible appelée par le lot : cœur et adaptateur `_68k` existent-ils (lot 0) ? Sinon, les créer dans
   le module du lot si la routine lui appartient, ou le signaler.
4. Convertir par petits pas : appelants → adaptateur ; constantes et helpers dupliqués supprimés ; champs et variables
   nommés ; glitches marqués. `cargo build` sans nouvel avertissement ; `cargo test --release --lib`.
5. Validation ciblée (scénarios qui exercent le lot : `boss/`, `enemy/`, `cov/`...), mode normal et
   `DDSOM_SANS_68K=1`. Commit + push.
6. `DDSOM_POISON=<étiquettes du lot>` sur les mêmes scénarios, puis passage au cœur là où c'est prouvé. Commit + push.
7. Fin de lot : validation complète, deux modes (`DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_JOBS=10 tools/validate_all.sh 30000`,
   puis avec `DDSOM_SANS_68K=1`), et `DDSOM_ASSETS` sur quelques scénarios ; mesurer après ; rapport : marques avant/après,
   lignes avant/après, résultat exact des validations, glitches et écarts rencontrés, hash du commit poussé.
