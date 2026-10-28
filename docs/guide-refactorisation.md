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
8. **Rester dans son lot.** Ne pas modifier `lib.rs`, `families/mod.rs`, `families/cx.rs`, `objects/`, `machine.rs`, `rng.rs`, `mem.rs`, `ram.rs`,
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

## L'API commune (lot 0b) : ce que les lots A à D utilisent

Le lot 0b a donné au moteur d'objets la forme du pilote `pools` : un cœur pur par routine, un adaptateur `_68k` qui
laisse en plus les registres de l'original, la native pour les appels par adresse restants, et des méthodes de contexte.
**On n'appelle plus une routine du moteur par son adresse** (`x.call(sub::MOVE_XZ)`, `ctx.jsr(rom::ANIM_STEP)`) : les
constantes `MOVE_*`, `ANIM_*`, `SPEED_*`, `REINSERT_*`, `CULL_*`, `RNG`, `RANK_*` ont disparu des familles.

### Contexte commun : le trait `machine::Machine`

Décision : un **trait** plutôt qu'une base `Jeu` embarquée. Tous les contextes tiennent `c` et `b` et y accèdent
séparément (`f(x.c, x.b)`, des milliers de fois) : une structure de base derrière un `Deref` casserait ces emprunts
disjoints. `Machine` (`src/machine.rs`) est implémenté pour `families::cx::Cx`, `families::Ctx`, `player::Ctx`,
`scene::Ctx`, `combat::World` (une ligne par contexte dans la macro `machine!`) et donne une fois pour toutes :

| Méthode | Sens |
|---|---|
| `x.random() -> u16` | le tirage unique du jeu (`rng::random`, 0x19a88) ; les appelants masquent |
| `x.random_68k() -> u16` | idem, et laisse D0.w et les drapeaux (sites dont la suite relit D0) |
| `x.prog8/16/32(a)` | espace programme (tables lues relativement au PC dans l'original) |
| `x.data8/16/32(a)` | espace données (tables atteintes par adresse absolue ou pointeur, RAM) |
| `x.bus()`, `x.machine()` | le bus ; le couple (processeur, bus) pour un adaptateur `_68k` |

`use crate::machine::Machine;` dans le fichier. Les accesseurs propres à chaque contexte (`a5_u8`, `ram8`, `v8`,
`rom16`...) restent pour l'instant (inhérents, prioritaires sur le trait) : chaque lot les remplace dans son périmètre par
les vues typées (`ram::Octet/Mot/Long`, `Obj`) ou par `Machine`, puis les supprime. Attention aux homonymes trompeurs :
`families::Ctx::rom16` lit l'espace **données**, `Cx::rom16` l'espace **programme** ; `Machine` les distingue
(`data16` / `prog16`). Le générateur n'a plus qu'une implémentation (`rng.rs`) ; les copies (`camera_modes`,
`player::Ctx::random`, `ai_aids::random`, `effects_b`) sont supprimées.

### Le moteur d'objets vu d'un gestionnaire : le trait `objects::Courant`

Pour les contextes qui traitent un objet (`Cx` : `x.o` ; `families::Ctx` : A0 au moment de l'appel).
`use crate::objects::Courant;` dans le fichier.

| Méthode | Original | Rend |
|---|---|---|
| `alloc(&pool)`, `alloc_unchecked`, `alloc_or_leftover`, `free(&pool)` | pools (pilote) | `Option<Obj>`... |
| `anim_start(set, n)` | 0x1a32 | adresse après l'entrée (jamais nulle) |
| `anim_start_entry(at)` | 0x1a3c | idem |
| `anim_start_by_character(&anim::SETS_ARMED / SETS_PLAIN / SETS_BY_VARIANT, n)` | 0x2c16 / 0x2c88 / 0x2cf0 | idem |
| `anim_step()`, `anim_step_long()` | 0x1a56, 0x2d58 | `bool` : retenue (fin de script, **ou durée nulle** : glitch, voir `anim::Step::Wrapped`) |
| `move_x/y/z/xy/xz/yz/xyz()` | 0x1d78, 0x1e02, 0x1dbc, 0x1d62, 0x1da6, 0x1dd4, 0x1d90 | `motion::LastAdd` |
| `move_by_table_xy()`, `move_by_table_xz()` | 0x1cb4, 0x1cee | `motion::LastAdd` |
| `select_speed(&motion::SPEEDS_A / SPEEDS_B, n)` | 0x1a94 / 0x1b64 | — |
| `suspended()` | 0x29ea | `bool` (octet `Obj::HIDDEN` non nul) |
| `tick_counters()` | 0xf1bfc | — |
| `reinsert()`, `reinsert_foreground()` | 0x1ee1a, 0x1ef6c | — |
| `unlink()` | 0x1f0be | — |
| `cull_offscreen()` | 0x1e1a | — |
| `in_arena(margin)`, `in_arena_default()` | 0x1e92, 0x1e90 | `bool` (retenue : dedans) |
| `update_visibility()` | 0x1ed2 | `bool` (visible) |

`motion::LastAdd` est la dernière addition `add.l D0,(position)` : `if x.move_xz().gt()` remplace
`x.call(MOVE_XZ); if x.c.cond(14)` (`gt`, `le`, `pl`, `mi`, `zero`, `overflow`, calculés comme le 68000). Un pas
d'animation suivi de `bcs` devient `if x.anim_step()`. Un appelant qui passait l'adresse d'une routine en paramètre
(`leap_climb(x, call::MOVE_XZ)`) prend une fonction : `leap_climb(x, |x| { x.move_xz(); })`.

### Hors gestionnaire : les cœurs et les adaptateurs

Un contexte sans objet courant (joueurs, combat, IA, scènes, natives `(c, b)`) appelle le module :

| Module | Cœur (bus seul) | Adaptateur (étiquette `DDSOM_POISON`) |
|---|---|---|
| `objects::anim` | `start(b, o, set, n)`, `start_entry(b, o, at)`, `start_by_character(b, o, &SETS_*, n)`, `step(b, o, long) -> Step` (`.finished()`) | `start_68k`, `start_entry_68k`, `start_by_character_68k`, `step_68k` (`anim`) |
| `objects::motion` | `ballistic(b, o, motion::XZ) -> LastAdd`, `move_by_table(b, o, AXIS_Y)`, `select_speed(b, o, &SPEEDS_B, n)`, `suspended(b, o)`, `tick_counters(b, o)`, `advance`, `advance_by_table`, `select_speed_table` | `ballistic_68k`, `move_by_table_68k`, `select_speed_68k`, `suspended_68k`, `tick_counters_68k` (`motion`) |
| `objects::lists` | `ACTORS.reinsert(b, o)`, `FOREGROUND.reinsert(b, o)`, `unlink(b, o)`, `unlink_all(b)` | `ACTORS.reinsert_68k(c, b, o)`, `unlink_all_68k` (`lists`) |
| `objects::cull` | `despawn_if_offscreen(b, o)`, `in_arena(b, o, margin)`, `update_visibility(b, o)`, `is_visible`, `camera` | `despawn_if_offscreen_68k`, `in_arena_68k`, `update_visibility_68k` (`cull`) |
| `objects::rank` | `reset(b)`, `load_preset(b, row)`, `tick(b)`, `raise(b)`, `lower(b)` | `reset_68k`, `load_preset_68k`, `tick_68k`, `raise_68k`, `lower_68k` (`rank`) |
| `rng` | `random(b)`, `seed(b)` | `random_68k`, `seed_68k` (`random`) |
| `objects::pools` | (pilote) `POOL.alloc(b)`... | `alloc_68k`, `release_68k` (`alloc`, `free`) |

`combat::Obj` n'existe plus : `combat` utilise `objects::Obj`, qui a gagné `Obj::from_word(w)` (mot étendu en signe),
`o.word()` et `o.long_ref()`.

### Ce qui reste en adaptateur, et pourquoi

Les méthodes de `Courant` (sauf `reinsert` et `reinsert_foreground`, voir plus bas) et `Machine::random` appellent
**le cœur** : la validation complète avec
`DDSOM_POISON=anim,motion,lists,cull,rank,random` a désigné les lecteurs de registres, et chacun a été soit replié dans
une valeur Rust (`if x.anim_step()`, `x.move_xz().gt()`, `let alea = x.random()`), soit laissé sur l'adaptateur,
écrit en clair à l'endroit qui relit le registre :

* `x.random_68k()` suivi d'une lecture de D0 plus loin (`effects_c.rs`, `boss_7/fight.rs`, `boss_8/fight.rs`,
  `boss_11`, `joueur/peff_e.rs`) ; `pshell::rand_and`/`rand_and_b` (contrat : D0 masqué en place) ; `ai::Mach::random`
  et `combat::World::random` (machines à registres de leurs lots).
* Les natives appelées par adresse (`(c, b)`, `scene::Ctx`, `player::Ctx` via `g.rom`, `modes`, `game.rs`) : créateurs
  (`creators.rs`, `eshl_creators.rs`, `effects_creators.rs`, `items.rs` : D0 du tirage relu), `ai_aids.rs` (animation,
  zone de jeu), rang (`scene/entry.rs`, `hud/slot.rs`, `game.rs`, `player/scripted.rs`), graine (`modes`), listes
  (`scene/shop.rs`, `set.rs`, `creators.rs`), animations des joueurs (`player/`, `effects_f.rs`). Leur contrat de
  registres est à prouver par leur lot, comme au pilote.
* `reinsert`, `reinsert_foreground` : la fin ordinaire d'un gestionnaire laisse D0-D2, A1-A4 au gestionnaire
  **suivant** de la boucle de dispatch, et certains les relisent (`joueur/peff_e.rs` tire son rebond dans le D2
  hérité ; `pshl_b.rs` aussi ; `scorpion_scene36`, `mash_c5`). `Courant::reinsert` garde donc l'adaptateur. Le cœur
  `ACTORS.reinsert(b, o)` reste pour les appelants hors boucle de dispatch.
* Lecteurs repérés et documentés sur place : `effects.rs` (D0 haut après la remise en liste : `reinsert_68k`),
  `reactions.rs` (`bhi` après 0x1e92 : `in_arena_68k`).

Pour un lot : `rtk proxy grep -rn "_68k(" <rép>` donne ses adaptateurs restants ; chacun se prouve avec
`DDSOM_POISON='<étiquette>@<fichier>'` (validation des scénarios qui l'exercent), puis passe au cœur.

## Les services communs (lot S) : son, palettes, texte, terrain, aides, événements

Les modules de services ont maintenant un cœur Rust ordinaire ; leur native (adresse d'origine) n'est plus que
l'adaptateur pour les appels par adresse qui restent, et reproduit les registres de l'original. Un gestionnaire
appelle le cœur et n'a plus à garder ses registres pour lui.

| Module | Cœur | Remplace |
|---|---|---|
| `sound` | `jouer(b, son, objet)` ; `jouer_global(b, son)` (son qui ne lit pas d'objet : musiques, interface) ; `jouer_avec(b, son, Source { objet, a1, d4 })` (sept entrées lisent D4 ou A1 : 0xe17a4, 0xe17e2, 0xe6910, 0xe7c20, 0xe7e10, 0xec046, 0xec200) ; `service(b, Service::ArretGeneral / Ff05 / Ff06 { verrou, ligne } / Ff07 {..} / Ff08(p) / Ff09(p))` ; `emettre(b, Commande)` | `x.call(sfx::…)` (A0 = l'objet), `x.call(0xe0882)`, `x.call_d0(0xe0a82, n)`… |
| `palette` | `full_brightness(b)`, `no_brightness(b)`, `or_brightness(b, niveau)`, `set_brightness(b, niveau)` ; `flash(b, Flash::Gray / A…G)`, `save_flash(b)`, `restore_flash(b)`, `set_flash_color(b, FLASH_COLOR_RED / FLASH_COLOR_WHITE_VALUE)` ; `character_palette(b, o)` | 0x1686, 0x168c, 0x168e, 0x173c ; 0x1ab56, 0x1aaa0-0x1ab3c, 0x1ac24, 0x1abe0, 0x1ac68/0x1ac86/0x35288 ; 0x37882 |
| `text` | `print(b, descripteur) -> fin`, `erase`, `print_over`, `print_tall`, `print_tall_at(b, d, col, row)`, `draw_blocks(b, d, objet)`, `clear_blocks`, `clear_box(b, d) -> fin` | `x.c.a[4] = d; x.call(0xd5e)`… (la fin rendue est l'A4 de l'original, pour enchaîner) |
| `video` | `terrain(b, x, y) -> Terrain { numero, x, y, case, index, colonne, .. }`, `terrain_of(b, o)` ; `terrain_68k(c, &t)` laisse D0-D4/A1/Z | 0x24b0, 0x24a8 (`x.call(…)` puis `x.c.d[2]`) |
| `aides` | `direction(b, de, vers) -> u8`, `direction_from(b, o, vers)`, `direction_to(b, o, cible)` ; `grabbed(b, o) -> Saisie` (`.z()` : le drapeau testé) ; `divide(n, d)` ; `game_over(c, b)` | 0x1386, 0x137e, 0x1376 (D1-D4 puis D0) ; 0x2810 ; 0x17ea ; 0x28d8 |
| `evenements` | `enfiler_points(b, ev) -> bool`, `enfiler_compteur`, `evenement_objet(b, o)`, `crediter_points(b)`, `crediter_compteur(b)`, `vider_points/compteur/popups(b)`, `effacer_popup_courant(b)`, `creer_popup(b, Popup { entete, max, valeur, brut })` | 0x19ae6, 0x19b08, 0x19ad6, 0x19b2a, 0x19b9c, 0x19aa2, 0x19abc, 0x19db4, 0x19e10, 0x19dce (barre de vie : `brut` = D3.b == 2) |
| `terrain`, `placement` | `terrain::push(b, o) -> bool` (Z de 0x33fe4) ; `placement::place_on_free_ground(b, o)` | `push_68k`, `place_on_free_ground_68k`, `call(0x3668)` |
| `ram` | `JOUEURS_EN_VIE` (0x35, appelé à tort `DIFFICULTE`), `DEFILEMENT_Y_PLAN1` (0x4ba), `DSW` (0x7fd0) | `a5_u8(0x35)`, `set_a5_u16(0x4ba, …)`… |

**Pour les lots A et B** (familles non converties par le lot S : `boss*`, `enemy*`, `reactions`, `effects*`,
`items*`, `boxes`, `petites`, `creators`) :

* Son : `x.call(SON)` dans un gestionnaire `Cx` devient `crate::sound::jouer(x.b, SON, x.o)` ; un son qui lit D4
  (`ParD4`, `DuD4`) prend `jouer_avec(x.b, SON, Source { objet: Some(x.o), d4: Some(valeur), ..Default::default() })`
  avec la valeur que l'appelant posait dans D4, et la pose de D4 disparaît. Un son appelé avec un objet absent
  ou sans D4 qu'il lit s'arrête (panique avec le message « ce son lit … ») : la validation le montre aussitôt.
  Conversion mécanique : `tools/convertir_sons.py <fichiers>` (constantes résolues ; motifs `x.call`, `g.rom`,
  `call(c, b, …)`, `w.call_ao`), puis relire le diff, retirer les constantes devenues inutiles, valider.
* **La sauvegarde fantôme n'existe plus** ([son-68000.md](son-68000.md)) : un gestionnaire qui joue un son n'a plus
  à garder ses registres « pour le son ». Les registres que le gestionnaire **suivant** relit (fin ordinaire,
  `reinsert`) restent à prouver par `DDSOM_POISON` comme avant.
* Terrain : `let t = crate::video::terrain_of(x.b, x.o); if t.numero …` ; garder `video::terrain_68k(x.c, &t)` tant
  qu'un gestionnaire suivant peut relire D3 (vu : `pshl.rs`, `boss11_scene42_mort` image 4447).
* Direction : `x.c.d[1] = …; x.call(0x137e); let a = x.c.d[0] as u8` devient
  `let a = crate::aides::direction_from(x.b, x.o, (tx, ty))` ; `x.c.a[1] = cible.0; x.call(0x1376)` devient
  `direction_to(x.b, x.o, cible)`.
* Saisie : `x.call(0x2810); if x.c.sr & SR_Z != 0` devient `if crate::aides::grabbed(x.b, x.o).z()` (ou un `match`
  sur `Saisie` : `Gele`, `Libre`, `Par { joueur, normal }`).
* Barre de vie : `x.c.d[0..4] = …; x.call(0x19dce)` devient `crate::evenements::creer_popup(x.b, Popup { … })`.

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

**Par site** (lot 0b). Une entrée `étiquette@motif` n'empoisonne que les appels dont le site (`fichier:ligne:colonne`
du gestionnaire, propagé par `#[track_caller]` à travers `Courant` et les adaptateurs) contient le motif ; `!étiquette@motif`
exclut ces sites : `DDSOM_POISON='motion,!motion@enemy_dbeast/parts.rs'`. Avec `DDSOM_POISON_TRACE=1`, la liste des
sites empoisonnés avant l'image de l'écart se réduit à quelques candidats qu'on essaie un par un (`motion@shell.rs:57`).

`DDSOM_POISON` change l'empreinte des résultats de `tools/validate_all.sh` (variables `DDSOM_*`) : pas de mélange avec
les résultats ordinaires.

### Sondes des appels, des cessions et des retours (sortie du `Cpu`)

Pour un appel par adresse, une étape de tâche ou un rendu, il n'y a pas d'adaptateur où poser `registres_morts` :
des sondes génériques (`diag.rs`) inversent les registres au bon endroit.

| Étiquette | Où | Registres inversés | Site (motif) |
|---|---|---|---|
| `appel_<adresse>` | retour de `crate::call` | ceux que l'appelé a changés | `fichier:ligne:colonne` de l'appel |
| `cession` | `kernel::ceder` (contexte rangé) | D0-D7, A0-A4, A6, drapeaux | nom de l'étape de reprise : `Ending(S04Timer)` |
| `retour` | `taches::executer`, pas qui rend `Flow::Done` | D0-D7, A0-A4, A6, drapeaux | nom de l'étape : `Exit(Both)` |
| `rendu` | `dispatch::Class::update_sans_suite`, mises à jour du monde des écrans | D0-D7, A0-A4, A6, drapeaux | appelant (`#[track_caller]`) |

VALIDE avec `cession@Ending(,retour@Ending(` : aucune étape du générique ne relit, après une cession ou au retour d'un
sous-programme, un registre d'avant ; avec `rendu@modes/` : les gestionnaires d'objets ne relisent rien de l'écran
qui les fait tourner. Les drapeaux sont inversés en deux parts (`nzvc` et `x`) pour savoir lequel est relu.

**Registres épargnés.** Une entrée `étiquette@motif#a1+nzvc` empoisonne le site sauf A1 et N/Z/V/C (une lecture
connue, écrite en clair, comme l'objet rendu par un créateur) ; précédée de `=`, elle épargne ces registres sans
empoisonner le site (à ajouter à une liste large : `tout@modes/,=appel_ddeaa@modes/#a1+nzvc`).

**Bissection.** `tools/bissecte_poison.py <validate> <scénario> <images> <liste>` relance le scénario avec la trace
(`DDSOM_POISON_TRACE`), cherche par dichotomie l'entrée responsable parmi les sites empoisonnés avant l'écart, puis le
ou les registres relus, l'épargne et recommence jusqu'à VALIDE ; il imprime les lecteurs trouvés et la ligne
`GARDES` à ajouter. Méthode du lot « écrans » : une validation complète du code d'origine (avant conversion) avec
`tout@<fichiers du lot>` et les sondes de cession et de retour des tâches du lot, puis une bissection par scénario
qui échoue, jusqu'à ce que tout échec restant soit une lecture connue (épargnée) ; enfin la même validation sur le code
converti avec `rendu` en plus.

## Gestionnaires sans registres (sortie du Cpu, partie 3)

* **Sonde de sortie** : `DDSOM_POISON=sortie` inverse, au retour de chaque gestionnaire de la boucle d'aiguillage, les
  registres qu'il a changés (sauf A0, D7) ; `sortie.d2@4bc66` (un registre, un gestionnaire), `!sortie.d4@ba310`
  (exclusion). La liste des gestionnaires dont la sortie est relue est dans [refactorisation.md](refactorisation.md)
  § 18 ; un scénario nouveau peut en ajouter (bissection : `sortie@<boucle>`, `sortie@<gestionnaire>`, puis par
  registre).
* **Écrire une fonction sans registres** : `fn f<R: Banc>(x: &mut Cx<'_, R>, …)` ; elle sert aux gestionnaires sur le
  processeur (`Cx`) comme aux convertis (`Gx`). `x.reinsert()`, `x.keep_in_zone()`, `x.keep_on_screen(m)`,
  `x.push_terrain()` font ce qu'il faut selon le banc. Un reste relu par un autre gestionnaire passe par
  `dispatch::Restes` (`x.restes().d4` sur `Gx`, `*x.c.d5()` générique).
* **Déclarer un gestionnaire converti** : `0x… "nom" #[sans] chemin` dans `handlers!` (natif
  `Corps::Gestionnaire`). Seulement si sa sortie n'est relue par personne, ou si ses restes relus sont écrits dans
  `Restes`.
* **Créateurs** : écrits sur `effects_creators::restes::Registres` (`r.alloue`, `r.d(n, v)`…) ; les appelants Rust
  passent `&mut Pur` (aucun registre), la native passe le processeur.

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

Depuis le lot « noyau final » (`refactorisation.md` § 14), plus aucune reprise n'est une native : le pas de jeu
(`taches::partie::Etape::Pas`, plafond des pas compté par `sans68k::pas_de_jeu`), les retours des phases de la
démonstration, la boucle de la machine à modes et les appels en cours de la boucle de partie (fin d'image, mise à jour
du monde, boucles d'aiguillage : `game::Etape`, `dispatch::Etape`) sont des étapes. Un gestionnaire d'objet qui fait
céder la main à la tâche (fondu des portes, feu d'artifice) cède, puis revient aussitôt ; le code Rust encore en
cours sous lui le voit (`taches::a_cede`, `taches::table::revenu`) et revient sans rien faire, et la tâche reprendra
par les étapes rangées dans ses appels en cours. Une fonction Rust qui appelle une native puis continue après elle
ouvre un **appel attendu** (`taches::table::appeler_attendu` + `crate::appeler_attendu`) : le `rts` de la native lui
rend la main, et si la native cède, l'appel garde la suite où la tâche reprendra. `crate::call` (appel par adresse)
n'admet plus de cession : il s'arrête sur une panique qui nomme le site.

## Pièges connus

* **Un seul mode.** Le jeu est le jeu sans 68000, partout (`validate`, `validate_all.sh`, `bench`, `ddsom`). Le mode
  hybride (`DDSOM_HYBRIDE=1`) est retiré (lot « noyau final ») : les tâches sont des machines à états lancées par un
  ordonnanceur Rust, et leurs appels en cours ne sont plus sur la pile 68000, le code d'origine ne pourrait plus
  prendre la suite d'une native. Les diagnostics qui exécutent l'original sur une copie (`DDSOM_CHECK`,
  `DDSOM_ADOPT_REGS`) marchent toujours ; `cessions` et `reste68k`, qui mesuraient le 68000 restant du mode hybride,
  n'ont plus d'objet. Il n'y a plus de durée à tenir à jour (lot 0a).
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
* **Durée d'animation nulle** : `anim_step` d'une entrée de durée 0 fait repartir la durée de 0xFFFF et lève la
  retenue (le `bcs` de l'appelant la prend pour une fin de script). `anim::Step::Wrapped`, `Step::finished()` ; le
  premier essai du lot 0b, qui ne rendait vrai qu'en fin de script, faisait diverger `ending` (un son manquant).
* **Drapeaux d'un mouvement** : `bgt`/`ble`/`bpl` après un `move_*` testent la dernière addition ; utiliser la
  `LastAdd` rendue, pas `x.c.cond(...)` (qui ne vaut plus rien depuis que les méthodes appellent le cœur).
* **Lectures de drapeaux indirectes** : des aides (`carry(x)`, `zero(x)`, `ctx.gt()`) lisent les drapeaux de la
  dernière routine ; après une méthode de `Courant`, elles lisent n'importe quoi. Les remplacer par la valeur rendue.
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
