# Scénarios en écart

Ce dossier garde les scénarios dont l'écart avec le code d'origine n'est pas (encore) expliqué par la routine qu'ils
exercent. **Il est vide** : tous les scénarios ont été ramenés à « VALIDE » et rangés dans leur dossier
(`boss/`, `enemy/`, `scene/`, `scene_end/`, `cov/`). Historique des causes trouvées (méthode : `DDSOM_SANS=préfixe`, puis
`DDSOM_ADOPT_REGS` / `DDSOM_ADOPT_MASK` / `DDSOM_ADOPT_TRACE`, `examples/watch` (RAM et GFX RAM), `examples/pctrace`,
`examples/cyc` (cycles à l'entrée des natives, hybride contre original)) :

| Scénario | Image | Cause | Correction |
|---|---|---|---|
| `scene/scene_67` | 3546 | régression de la fusion : la porte de passage `set_29` (`set_b.rs`, `door_b`) appelait le fondu `0x1460`, qui cède la main (`0xa40`), avec l'adresse de retour sentinelle de `call` ; au réveil, la tâche revenait à `0xfffe` | `Cx::call_at(addr, ret)` : le fondu est appelé avec l'adresse de retour de l'original (`0xd8f06`) |
| `boss/boss13_dragon`, `scene/scene_07`, `enemy/ghoul_scene19` | – | VALIDES sans autre changement (plafond de pas de jeu par image, registres résiduels déjà corrigés) | – |
| `scene/scene_05`, `scene/scene_11` | 4118, 4951 | harpie (`enemy_harpy`, sortie de vol 0x75884) : l'angle de vol `+0x2d` devenu négatif est remis à zéro (`clr.b` en 0x758c0), ce que les autres variantes de vol ne font pas | `bank(x, reload, clamp)` |
| `scene/scene_08` | 3889 | fenêtre de dialogue (`set_24`, `set_b.rs`, `dialog`) : à la fermeture, la définition suivante est rechargée par `0xd7124` ; pour la sorte 13 elle copie la palette par `movem` dans D0-D6/A3, et l'original relit D6 (nombre de lignes à effacer) *écrasé* par la palette | `copy_palette` laisse D0-D6/A3 ; `shrink` relit D6 |
| `scene/scene_110` | 3355, 3365, 3725, 4668 | (1) `build_sprite_list` laissait A4 (pointeur d'écriture) et A6 (caméra) : la cinématique de fin des joueurs saute pour la scène 110 hors de sa table (0x23f4e, scènes 0 à 108) et lit une animation avec A4 laissé par le constructeur de sprites ; (2) image d'objet sans morceau (boucle `dbf` à -1, 65 536 sprites sur plusieurs images) ; (3) l'écran de fin de niveau lit son aiguillage hors de sa table et plante | A4/A6 rendus par `build_sprite_list` ; trois correctifs communs aux deux machines (`docs/boucle.md` §9) |
| `scene_end/end_01` | 3605 | image d'objet sans morceau (voir ci-dessus) : la machine de référence passait deux images dans la boucle de 65 536 sprites | `sprites::EMPTY_FRAME_PATCHES` |
| `enemy/ghoul_scene97` | 5516 | `players_update` ne laissait pas D5 : le reconnaisseur de combos (`0xf186e`/`0xf187c`) y laisse 0 ou 0x20, relu par une routine suivante ; ses autres registres (D0-D3, D7.b) sont aussi relus (position du sang des boss, ci-dessous) | `combos::recognize_*` et `combos::step` rendent D0-D5, D7.b |
| `cov/cov_tour5_c4` | 24147 | mort du Lich (boss 18) : `0x103ba` crée le sang en `0xb141a` avec D2/D3 laissés par le code précédent (D3 par le reconnaisseur de combos du joueur, voir ci-dessus) | `combos::step` rend D3 |

Piste générale : un écart qui disparaît en mode contrôle (l'état de l'original est réadopté après chaque appel) mais
qu'aucune désactivation isolée ne supprime = registres hérités. `DDSOM_ADOPT_REGS=préfixe,...` fait rendre aux natives
nommées les registres de l'original ; `DDSOM_ADOPT_MASK` limite l'adoption à quelques registres (bit i = Di, bit 8+i = Ai) :
chercher par dichotomie le registre, puis la native, puis le lecteur du registre dans l'original (`examples/pctrace` avec
`DDSOM_SANS=,` donne tous les registres instruction par instruction).
