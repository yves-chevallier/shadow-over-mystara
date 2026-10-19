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
```

boss5_scene58_mort.txt (scène 58, boss 5 réécrit dans families/boss_5.rs, PV du boss ramenés à 1 après 1 500 images)
  validate : écart à l'image 6211, ~1 000 images après la libération du boss (image 5383 ; boss_trace montre la mort aux images
  5038-5383), mots FF2A58 (minuterie du décompte 0xf6658), FF806E (compteur d'images, A en retard de 27), FF8556... :
  identique avec DDSOM_SANS=boss_5 ; aucune désactivation d'un seul groupe ne le supprime (combinaison de natives, comme boss13) ;
  toutes natives désactivées (DDSOM_SANS=,) : VALIDE. VALIDE sur 6 200 images (mort comprise).

boss21_scene44_mort.txt (scène 44, boss 21 réécrit dans families/boss_21/, PV ramenés à 1 après 1 500 images)
  validate : écart à l'image 5545, mot FF8664 (joueur 1 +0x86) 00ff au lieu de ffff ; identique avec DDSOM_SANS=boss ; disparaît
  avec DDSOM_SANS=combat : DDSOM_CHECK=1 désigne `combat_update` (entrée 0x260ee) : ram[ff8664]=00ff au lieu de ffff. Routine
  fautive : combat.rs. VALIDE sur 5 500 images.

boss19_scene32_mort.txt (scène 32, dragon 19 réécrit dans families/boss_19/, PV du boss ramenés à 1 après 1 500 images)
  validate : écart à l'image 4867 (mot FFD514, 0000 au lieu de fffb), 4 images après le début de la mort du boss (image 4863) ;
  identique avec DDSOM_SANS=boss_19 ; DDSOM_CHECK=1 désigne `game_phase12` (entrée 0x1c6a0, game.rs) : ram[ffd514]=0000 au
  lieu de fffb. VALIDE sur 4 866 images.

boss15_scene46_mort.txt (scène 46, dragon rouge 15 réécrit dans families/boss_15/, PV du boss ramenés à 1 après 1 500 images)
  validate : écart à l'image 4237 (mot FFD574, 0000 au lieu de 002c), 1 image après le début de la mort du boss (image 4236) ;
  identique avec DDSOM_SANS=boss_15 ; DDSOM_CHECK=1 désigne `game_phase12` (entrée 0x1c6a0, game.rs) : ram[ffd574]=0000 au lieu
  de 002c (même routine que boss19_scene32_mort). VALIDE sur 4 236 images.

boss18_scene26_mort.txt (scène 26, Lich 18 réécrit dans families/boss_18/, PV du boss ramenés à 1 après 1 500 images)
  validate : écart à l'image 4939 (mot FFD692, 003c au lieu de 0400), à la mort du boss ; DDSOM_CHECK=1 désigne `game_phase12`
  (entrée 0x1c6a0, game.rs) : ram[ffd692]=003c au lieu de 0400, ram[ffd694]=0000 au lieu de fffb. VALIDE sur 4 938 images.
  Le scénario boss18_scene26 de scenarios/boss/ est tronqué à l'image 10 300 (le bot tue le Lich à l'image 10 541, et le même
  game_phase12 diverge à 10 577) : VALIDE sur 12 000 images, le Lich vivant (28 PV).

boss8_scene64_mort.txt (scène 64, DarkWarrior 2 réécrit dans families/boss_8/, PV du boss ramenés à 1 après 1 500 images)
  validate : écart à l'image 6138 (mots FFD572/FFD574 : vitesse d'un effet EFFCT, 0001/0000 au lieu de 0400/0020), 4 images après
  le début de la mort du boss (image 6134) ; identique avec DDSOM_SANS=boss ; DDSOM_CHECK=1 et DDSOM_CHECK_FRAME=6138 ne
  signalent aucun écart ; aucune désactivation d'un seul préfixe ne le supprime. DDSOM_ADOPT_REGS=a,o,p,v,e,s,g (ou tous les
  préfixes sauf b) le supprime, mais aucun préfixe seul : registres hérités par plusieurs natives d'autres modules, non isolées.
  VALIDE sur 6 137 images.

boss12_scene40_mort.txt (scène 40, serpent 12 réécrit dans families/boss_11/, PV du boss ramenés à 1 après 1 500 images)
  validate : écart à l'image 4463 (mot FFD514, 0000 au lieu de ffff), 4 images après le début de la mort du boss (image 4459) ;
  identique avec DDSOM_SANS=boss_11 ; DDSOM_CHECK=1 désigne `game_phase12` (entrée 0x1c6a0, game.rs) : ram[ffd514]=0000 au lieu
  de ffff (même routine que boss19_scene32_mort et boss15_scene46_mort). VALIDE sur 4 462 images.

boss11_scene42_mort.txt (scène 42, serpent 11 réécrit dans families/boss_11/, PV du boss ramenés à 1 après 1 500 images)
  validate : écart à l'image 6211 (mots FF8556, FF855E, FFCAF6 : 0040 au lieu de 0020), ~1 800 images après la mort du boss
  (image 4443, libéré à l'image 5428) ; identique avec DDSOM_SANS=boss_11 ; aucun écart natif en mode contrôle ; même signature
  que boss5_scene58_mort (décompte de fin de scène 0xf6658, image 6211 aussi). VALIDE sur 6 210 images (mort comprise).

boss12_scene40.txt (scenarios/boss/, tronqué à l'image 10 300 par `gen_boss.py`) : entier, le scénario diverge à l'image 10 315
  (mot FF8664, joueur 1 +0x86 : 00ff au lieu de ffff) ; identique avec DDSOM_SANS=boss_11 et DDSOM_ADOPT_REGS=boss_11 ;
  disparaît avec DDSOM_SANS=combat : c'est `combat_update` (combat.rs), comme boss21_scene44_mort. Tronqué, VALIDE sur 12 000 images.

Piste : écart qui disparaît en mode contrôle (l'état de l'original est réadopté après chaque appel)
mais qu'aucune désactivation isolée ne supprime = registres hérités. `DDSOM_ADOPT_REGS=préfixe,...`
fait rendre aux natives nommées les registres de l'original : chercher par dichotomie celle dont un
registre de sortie est relu.
