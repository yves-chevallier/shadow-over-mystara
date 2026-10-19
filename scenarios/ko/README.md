# Scénarios en écart, gardés pour le diagnostic

Ces scénarios ne sont pas dans la validation courante : leur écart avec le code d'origine ne vient pas
(ou pas seulement) de la routine qu'ils exercent. `boss13_dragon` est le dernier.

Les scénarios `_mort` (boss 5, 8, 11, 12, 15, 18, 19, 21 : PV du boss ramenés à 1 après 1 500 images) et les grands
`boss12_scene40`, `boss18_scene26` sont passés dans `scenarios/boss/`. Les écarts d'origine (FFD514/FFD574/FFD692 de
`game_phase12`, FF8664 de `combat_update`, vitesse d'effet du boss 8) avaient été corrigés par les campagnes de registres
résiduels. Ceux qui restaient (images 6211, 6652, 7111, 7741, 11040 ; mots FF8556, FF855E, FFCAF6, FF806E, FF2A38/58) ne
viennent d'aucune native : c'est la « tempête de pas » (`docs/couverture.md`), l'avance rapide du jeu d'origine (la tâche
principale enchaîne 100 à 140 pas par image, `examples/passages` avec `DDSOM_SANS=` le montre : 0 puis 113 passages en 0x1c86e).
Elle est déclenchée par un bouton **pressé ou maintenu** à l'entrée de la scène suivante ou pendant la fin de scène ; la
troncature d'un scénario laissait justement des boutons enfoncés. `gen_boss.py` (`tronquer`) coupe donc les entrées
après la mort du boss et relâche les boutons restés enfoncés : les scénarios `_mort` sont VALIDES sur 12 000 images (le
décompte 0xf6658 des boss 5 et 11 : aucune native en cause, seul le nombre de pas de la tempête diffère). `boss18_scene26` reste tronqué (image 10 900, mort du Lich à 10 541) ; `boss12_scene40` ne l'est plus.

```text
boss13_dragon.txt (scènes 7 et 9, boss de type 13 réécrit dans families/boss_dragon.rs)
  validate : VALIDE jusqu'à l'image 9990 ; écart à l'image 9991 (fin de scène : FF8032, FF8050, FF8054 et FF806E,
  la fin survient 23 images plus tôt que dans l'original), aucune désactivation d'un seul groupe de natives ne
  le supprime. Les écarts précédents (images 3100 et 7241 : FF85E8, FF85E6) venaient des registres hérités :
  set_0, build_sprite_list et camera1/2 laissaient un D4 différent, relu par 0xad7b0 (ajoute D4 à la position
  des joueurs quand A5+0x51e vaut 4). DDSOM_CHECK=1 DDSOM_CHECK_ALL=1 : VALIDE sur 12000 images.
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
