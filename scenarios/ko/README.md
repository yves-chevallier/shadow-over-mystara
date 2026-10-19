# Scénarios en écart, gardés pour le diagnostic

Ces scénarios ne sont pas dans la validation courante : leur écart avec le code d'origine ne vient pas
(ou pas seulement) de la routine qu'ils exercent. `mash_c3*`, `boss6_scene24` (écarts de `combat_update`,
corrigés) et `boss10_scene65` (set_30 : le décompte de 0x80 images commençait une image trop tard) sont
revenus dans `scenarios/extra/` et `scenarios/boss/`.

```text
boss13_dragon.txt (scènes 7 et 9, boss de type 13 réécrit dans families/boss_dragon.rs)
  validate : VALIDE jusqu'à l'image 9990 ; écart à l'image 9991 (fin de scène : FF8032, FF8050, FF8054 et FF806E,
  la fin survient 23 images plus tôt que dans l'original), aucune désactivation d'un seul groupe de natives ne
  le supprime. Les écarts précédents (images 3100 et 7241 : FF85E8, FF85E6) venaient des registres hérités :
  set_0, build_sprite_list et camera1/2 laissaient un D4 différent, relu par 0xad7b0 (ajoute D4 à la position
  des joueurs quand A5+0x51e vaut 4). DDSOM_CHECK=1 DDSOM_CHECK_ALL=1 : VALIDE sur 12000 images.
```

Piste : écart qui disparaît en mode contrôle (l'état de l'original est réadopté après chaque appel)
mais qu'aucune désactivation isolée ne supprime = registres hérités. `DDSOM_ADOPT_REGS=préfixe,...`
fait rendre aux natives nommées les registres de l'original : chercher par dichotomie celle dont un
registre de sortie est relu.
