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
   compteurs d'appels de `validate`, le jeu sans 68000 qui s'arrête sur une adresse sans native) se remplace par la
   native appelée directement, ou par `unreachable!("…")` documenté si l'aiguillage de la ROM ne peut pas y mener.
   Ne pas inventer de logique pour un chemin jamais exercé : le dire.
8. **Rester dans son lot.** Ne pas modifier `lib.rs`, `families/mod.rs`, `families/cx.rs`, `objects/`, `mem.rs`, `ram.rs`,
   `diag.rs`, `taches/table.rs` hors du lot 0 ; un besoin d'API se code d'abord dans son module et se signale.

## Convention de nommage des couches

| Forme | Exemple | Sens |
|---|---|---|
| cœur | `Pool::alloc(&self, b) -> Option<Obj>` | logique pure sur l'état ; aucun `Cpu` |
| adaptateur | `Pool::alloc_68k(&self, c, b) -> Option<Obj>` | cœur + registres et drapeaux de l'original ; appelle `diag::registres_morts` |
| méthode de contexte | `Cx::alloc(&mut self, pool)` | raccourci pour les gestionnaires ; appelle le cœur une fois les registres prouvés morts |
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

**Exemple réel (pilote).** `DDSOM_POISON=alloc,free` sur toute la validation : tous les scénarios échouent à l'image 389
(`ram fff9aa : 0000 au lieu de 00ff`). `free` seul passe ; `alloc` non. Le champ `+0x4c` de l'emplacement 0 du pool SOBJ
n'est pas écrit : la machine à modes (`modes/title.rs`, étape `0xfa472`) appelle par adresse le créateur
`SOBJ_VARIANT`, une native en `(c, b)` qui alloue, puis **relit A1** — l'objet créé — que la native laisse parce que
l'allocation l'y a mis. A1 fait partie du contrat de cette native. D'où la règle : une native appelée par adresse (contexte
`(c, b)`, `scene::Ctx`, `player::Ctx`) garde l'adaptateur `_68k` tant que ses appelants relisent ses registres ; les
gestionnaires de la boucle de dispatch (`Cx`, `families::Ctx`, contrat A0/D7) ont leur propre étiquette
(`gestionnaires`) pour être prouvés à part.

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

## Exemple : une tâche qui cède (lot E, machines à états)

Avant (`hud/mod.rs`) : la reprise est une native posée à l'adresse d'origine (`resume!`), la cession un `tail` vers
l'enveloppe `0xb64` (la suite est un cadre, que l'enveloppe dépile pour en faire la reprise).

```rust
resume!(AFTER_WAIT, at::AFTER_WAIT, |x| tour(x));
fn tour(x: &mut Ctx) -> Flow { …; x.tail(at::WAIT_ONE_FRAME, at::AFTER_WAIT) }
```

Après : chaque point de reprise est une variante d'une `enum Etape` du module ; l'adresse d'origine n'est plus que
`Etape::adresse()` (diagnostic, comparaison de la table avec FF0064), et la cession passe directement par le noyau.

```rust
crate::etapes! {
    /// Étapes de la tâche (le haut de la boucle, `0x15178`, n'est jamais une reprise).
    pub enum Etape {
        Entree = 0x150ac => entry,
        Starting = 0x15146 => starting,
        AfterWait = 0x15212 => tour,
    }
}
fn tour(x: &mut Ctx) -> Flow { …; crate::taches::une_image(x, Etape::AfterWait) }
```

Le modèle (`taches/mod.rs`, `taches/table.rs`, `kernel.rs`) :

* **`etapes!`** déclare l'enum (variantes sans champ), `adresse()` et `pas(x) -> Flow`. Une étape qui porte un état
  (le sens d'un fondu, les registres sauvés par un `movem`, le D0 gardé par un `move.l D0,-(SP)`) s'écrit à la main :
  `fondu::Etape::Luminosite(Luminosite)`, `Composante { fondu, registres }`, `tache_credits::Etape::PulseAfterWait
  { d0 }`. Ce que l'original garde sur la pile pendant qu'il cède devient ainsi un champ nommé de l'étape.
* **`taches::Etape`** réunit les étapes de tous les modules (`etapes_des_taches!`, une ligne par module) ; `From`
  permet de passer `Etape::X` du module partout où une suite est attendue (`impl Into<Suite>`).
* **`table::Suite`** : `Adresse(u32)` (une native, pour ce qui reste appelé ou repris par adresse) ou
  `Etape(Etape)`. Une cession range `Reprise::Etape(etape, contexte)` ; un appel en cours (`Cadre`) n'est plus
  qu'une suite et sa hauteur de pile.
* **Le noyau appelle le pas** : `kernel::lancer` remet registres, SR et pile du contexte puis
  `taches::executer(etape)` au lieu de poser `c.pc` ; une tâche neuve dont l'entrée est une étape
  (`taches::entree`) démarre de même. `executer` exécute le pas et, s'il rend `Flow::Done` (retour de
  sous-programme), `taches::revenir` : la suite de l'appel en cours, étape exécutée aussitôt ou adresse posée au PC.
  `mem::rts` passe par `revenir`.
* **Cessions** : `taches::ceder(x, Attente, suite)`, `une_image` (`jsr 0xb64`, D0.w = 1), `attendre` (`jsr 0xa40`,
  D0 tel quel), `attendre_puis_revenir` (`jmp 0xa40`), `terminer` (`0x9f8`), `recommencer` (`0xae4`),
  `creer_puis_revenir` (`jmp 0x9b6`) ; création et mort d'une autre tâche : `kernel::creer_tache`, `tuer_tache`.
* **Sous-programmes qui cèdent** (fondus, écran noir) : des fonctions Rust (`taches::fondu::descente`, `montee`,
  `descente_d0`, `vers_le_blanc`..., `video::ecran_noir::debut`) appelées par `x.sous_programme(f, suite)` (suite
  rangée en cadre, exécutée aussitôt si `f` revient sans céder), ou en appel terminal (`return fondu::montee(x)`).
  Leurs entrées restent des natives tant que des gestionnaires d'objets les appellent par adresse.

Convertir un module :

1. `resume!(NOM, ADRESSE, CORPS)` → variante `Nom = ADRESSE => CORPS` d'un bloc `etapes!` (mécanique : le lot E
   l'a fait par script pour 330 reprises) ; retirer la native de `NATIVES` ; ajouter la ligne du module à
   `etapes_des_taches!` (et à `taches::entree` si c'est une entrée de tâche).
2. Remplacer chaque adresse de reprise passée à `wait`, `tail`, `push_return`, `ceder`... par la variante ; les aides
   qui prenaient une adresse prennent l'`Etape` du module ou `impl Into<Suite>`. Une adresse oubliée compile encore
   (`u32` est une `Suite`) : chercher les adresses converties restées en clair, sinon la reprise s'arrête sur « pas
   de native ».
3. Valider (scénarios qui exercent les écrans : `modes/`, `scene_end/`, `cov_*`).

Ce qui doit rester une native, et pourquoi :

* le **pas de jeu `0x1c4b0`** : la boucle d'image teste le plafond de pas par image (`cps2::StepCap`) sur le PC,
  avant chaque native ;
* l'**adresse de retour d'un `call_at`** vers du code qui peut céder (retours des phases de la démonstration
  `0x1f98c`/`0x20a98`, reprises de la mise à jour du monde et des boucles d'aiguillage, `game.rs`, `dispatch.rs`) :
  la tâche y revient par un `rts` de la pile 68000 ;
* les **entrées appelées par adresse** (sous-programmes de fondu et d'écran noir, que des gestionnaires d'objets
  appellent par `call_at`).

Une étape remise sur la pile 68000 par `call_at` (`mettre_sur_la_pile` écrit son adresse) et dépilée par une
native qui revient plus haut (épilogue étranger des bulles, qui revient au début du tour de la boucle de partie) est
retrouvée par `table::etape_depilee` : la boucle sans 68000 l'exécute au lieu de s'arrêter sur une adresse sans
native.

## Pièges connus

* **Deux modes.** Le jeu est le mode sans 68000, par défaut partout (`validate`, `validate_all.sh`, `bench`,
  `ddsom`). `DDSOM_HYBRIDE=1` donne le mode hybride (natives et 68000 de repli, durée constante par native) pour les
  diagnostics qui exécutent l'original (`DDSOM_CHECK`, `DDSOM_ADOPT_REGS`, `cessions`, `reste68k`) ; il n'est pas tenu
  de finir VALIDE. Il n'y a plus de durée à tenir à jour (lot 0a).
* **Registres hérités** : le gestionnaire suivant de la boucle de dispatch, une tâche neuve, une étape qui reprend
  héritent des registres (le contexte d'une cession les garde, que la reprise soit une étape ou une native). C'est ce que `DDSOM_POISON` attrape ; `DDSOM_ADOPT_REGS`/`DDSOM_CHECK_ALL` aident à trouver
  lequel.
* **`Cx::call` posait A0 = objet et A5** avant chaque appel : un appel converti ne le fait plus. Si une suite relit A0
  après qu'un gestionnaire l'a changé, la validation le montre ; remettre `x.c.a[0] = x.o.0` à cet endroit, commenté.
* **Noms faux hérités** : vérifier une constante par sa valeur, pas par son nom (`ALLOC_BOX = 0x23a8` allouait un BOSS).
* **Remplacements automatiques** : un script qui réécrit des identifiants par leur valeur doit exclure les constantes
  homonymes d'autres types (`dispatch::PSHL` est une `Class`) ; relire le diff.
* **`rts` pour jeter un appel en cours** : `mem::rts` exécute aussitôt une suite qui est une étape ; pour abandonner
  un cadre sans le suivre, `Sequence::drop_return` ou `table::depiler_retour`.
* **Une validation lancée recompile** : ne pas modifier les sources pendant le `cargo build` de `validate_all.sh`.

## Check-list d'un lot

1. Mesurer avant : `tools/mesure_refacto.py` (zones du lot), `--consts`, `--doublons 6` ; noter dans la doc du lot.
2. Lister les appels par adresse du lot et leurs cibles (`rtk proxy grep -rn "call(" <rép>`), les replis `_ =>`, les
   `deleguer`, les contextes et helpers dupliqués.
3. Pour chaque routine cible appelée par le lot : cœur et adaptateur `_68k` existent-ils (lot 0) ? Sinon, les créer dans
   le module du lot si la routine lui appartient, ou le signaler.
4. Convertir par petits pas : appelants → adaptateur ; constantes et helpers dupliqués supprimés ; champs et variables
   nommés ; glitches marqués. `cargo build` sans nouvel avertissement ; `cargo test --release --lib`.
5. Validation ciblée (scénarios qui exercent le lot : `boss/`, `enemy/`, `cov/`...), mode par défaut (sans 68000).
   Commit + push.
6. `DDSOM_POISON=<étiquettes du lot>` sur les mêmes scénarios, puis passage au cœur là où c'est prouvé. Commit + push.
7. Fin de lot : validation complète (`DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_JOBS=10 tools/validate_all.sh 30000`, sans
   68000), et `DDSOM_ASSETS` sur quelques scénarios ; mesurer après ; rapport : marques avant/après,
   lignes avant/après, résultat exact des validations, glitches et écarts rencontrés, hash du commit poussé.
