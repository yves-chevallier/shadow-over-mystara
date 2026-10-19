# Scénarios en écart, gardés pour le diagnostic

Ces scénarios ne sont pas dans la validation courante : leur écart avec le code d'origine ne vient pas
(ou pas seulement) de la routine qu'ils exercent. `mash_c3*` et `boss6_scene24` (écarts de
`combat_update`, corrigés) sont revenus dans `scenarios/extra/` et `scenarios/boss/`.

```text
boss13_dragon.txt (scènes 7 et 9, boss de type 13 réécrit dans families/boss_dragon.rs)
  validate : écart à l'image 3100 (mot FF85E8 = fraction de X du joueur 1), identique avec DDSOM_SANS=boss
  (natif du boss désactivé) ; toutes natives désactivées (DDSOM_SANS=,) : VALIDE. Aucune désactivation d'un seul
  groupe de natives (combat_update, players_*, player_overlays_update, camera1, game, dispatch...) ne supprime
  l'écart : plusieurs routines à la fois, comme les scénarios cov/ (docs/couverture.md).
  DDSOM_CHECK=1 DDSOM_CHECK_ALL=1 : VALIDE sur 12000 images (chaque appel natif du boss comparé à l'original).

boss10_scene65.txt (scène 65, boss de type 10 réécrit dans families/boss_10.rs)
  validate : écart à l'image 3271 (mot FFCB3A) avant même l'apparition du boss (image 3501), identique avec DDSOM_SANS=boss_10 ;
  DDSOM_SANS=set_ (les natives set_0, set_12, set_13, set_14, set_ecrans_ce024) : VALIDE, mais aucune d'elles désactivée seule
  ne supprime l'écart (combinaison de natives, comme boss13_dragon). Contrôle appel par appel : VALIDE sur 12000 images.
  Le scénario boss10_scene66.txt (scenarios/boss/), qui entre directement dans la scène 66, est VALIDE.

ghoul_scene97.txt (scène de démonstration 97, Ghoul de type 5 réécrit dans families/enemy_ghoul/)
  validate : écart à l'image 5516 (181 mots, objets FFBE9E-FFBF56 : effets), identique avec DDSOM_SANS=enemy
  (toutes les natives ENEMY désactivées) ; DDSOM_SANS=players_update : VALIDE. Contrôle appel par appel
  (DDSOM_CHECK=1 DDSOM_CHECK_ALL=1 DDSOM_SANS=game_phase12,world_update,dispatch) : VALIDE sur 12000 images,
  aucun écart de l'ennemi. Cause : players_update (autre module).

ghoul_scene19.txt (scène 19, Ghoul de type 5 réécrit dans families/enemy_ghoul/)
  validate : écart à l'image 5483 (mot FF8664 : champ du joueur 1), identique avec DDSOM_SANS=enemy_5 ;
  DDSOM_SANS=combat : VALIDE (combat_update, voir docs/couverture.md).
```

Piste : écart qui disparaît en mode contrôle (l'état de l'original est réadopté après chaque appel)
mais qu'aucune désactivation isolée ne supprime = registres hérités. `DDSOM_ADOPT_REGS=préfixe,...`
fait rendre aux natives nommées les registres de l'original : chercher par dichotomie celle dont un
registre de sortie est relu.
