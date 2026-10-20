# Les créatures du jeu

Image de chaque créature (types d'ennemis du pool ENEMY, boss du pool BOSS, six personnages jouables) dans sa pose
de base (premier cadre de l'animation de marche ou d'attente), avec sa palette, sur fond transparent, à l'échelle
1:1 (`x2` : agrandie deux fois) avec une bande de cadres d'animation (`anim`). Un même type d'objet a souvent
plusieurs **variantes** (octet `+3`) : autre couleur, autre créature, autre rôle ; chacune a son image.
Les noms des types sont ceux de `docs/familles/enemy.md` et `boss.md` (hypothèses : une variante peut montrer
autre chose que le nom du type).

Les images sont tirées des ROMs : **elles ne sont pas versionnées**. Le dossier `assets/creatures/` est ignoré
par git (`assets/` dans `.gitignore`) ; les liens ci-dessous ne fonctionnent qu'après génération.

## Régénérer les images

```sh
DDSOM_ROMS=rom tools/creatures.sh      # scan des scénarios de couverture puis assemblage -> assets/creatures/
tools/creatures.sh build               # seulement l'assemblage (images, planches, ce fichier)
```

Le script compile `port/ddsom-game/examples/creatures.rs`, lance `creatures scan` sur les scénarios
`scenarios/cov/cov_tour*.txt` et `cov_natural_c0.txt` (4 en parallèle, 30 000 images chacun), puis `creatures build`
qui réécrit les PNG, les planches `planche_*.png` et ce fichier. `CREATURES_EXTRA="nom ..."` ajoute des scénarios
de `scenarios/cov/`.

## Méthode

À chaque image de la partie rejouée (code natif), `scan` lit les objets des pools ENEMY et BOSS et le joueur 1.
Pour un objet dont le cadre de sprite (`+0x34`, voir `docs/objets.md`) change, il reconstruit les entrées d'Object
RAM de cet objet seul, comme `sprites.rs` : morceaux de 16x16 décalés, attributs par morceau, palette imposée
(`+0x2a`/`+0x2b`), objets rattachés (`+0x48`/`+0x4a`) devant ou derrière. Il les rend avec `cps2-gfx` et la
**palette courante de la machine** ; la transparence vient de deux rendus sur deux fonds différents (un pixel est
opaque s'il est identique dans les deux). L'orientation est celle de la ROM (sans miroir) ; les ombres sont omises.
`build` choisit la pose de base (premier échantillon en état actif `+4 = 2`, sous-état de marche `+5 = 0`), et prend
la suite de cadres distincts du même objet pour la bande d'animation.

## Limites

* Seules les créatures rencontrées dans les scénarios sont présentes ; une créature n'apparaît que dans les scènes
  atteintes par ces parcours (colonne « Scènes »).
* Un objet constitué de plusieurs objets (corps et membres séparés, boss à parties) n'est rendu que par son objet
  principal et ses objets rattachés.
* La palette est celle de la scène : une créature peut changer de teinte d'une scène à l'autre ; les images montrent
  la première rencontrée.

## Personnages jouables

![planche des personnages](../assets/creatures/planche_personnages.png)

| Type | Variante | Nom | Famille | Scènes (`FF8034`) | Images | Remarque |
|---|---|---|---|---|---|---|
| 0 | 0 | Fighter | joueur | – | [1:1](../assets/creatures/player_00_00.png) · [x2](../assets/creatures/player_00_00_x2.png) · [anim](../assets/creatures/player_00_00_anim.png) |  |
| 2 | 0 | Cleric | joueur | – | [1:1](../assets/creatures/player_02_00.png) · [x2](../assets/creatures/player_02_00_x2.png) · [anim](../assets/creatures/player_02_00_anim.png) |  |
| 4 | 0 | Elf | joueur | – | [1:1](../assets/creatures/player_04_00.png) · [x2](../assets/creatures/player_04_00_x2.png) · [anim](../assets/creatures/player_04_00_anim.png) |  |
| 6 | 0 | Dwarf | joueur | – | [1:1](../assets/creatures/player_06_00.png) · [x2](../assets/creatures/player_06_00_x2.png) · [anim](../assets/creatures/player_06_00_anim.png) |  |
| 8 | 0 | Magic-User | joueur | – | [1:1](../assets/creatures/player_08_00.png) · [x2](../assets/creatures/player_08_00_x2.png) · [anim](../assets/creatures/player_08_00_anim.png) |  |
| 10 | 0 | Thief | joueur | – | [1:1](../assets/creatures/player_10_00.png) · [x2](../assets/creatures/player_10_00_x2.png) · [anim](../assets/creatures/player_10_00_anim.png) |  |

## Ennemis (pool ENEMY, 29 types)

![planche des ennemis](../assets/creatures/planche_ennemis.png)

| Type | Variante | Nom | Famille | Scènes (`FF8034`) | Images | Remarque |
|---|---|---|---|---|---|---|
| 0 | 0 | Kobold | ENEMY | 99 | [1:1](../assets/creatures/enemy_00_00.png) · [x2](../assets/creatures/enemy_00_00_x2.png) |  |
| 0 | 3 | Kobold | ENEMY | 6, 20 | [1:1](../assets/creatures/enemy_00_03.png) · [x2](../assets/creatures/enemy_00_03_x2.png) · [anim](../assets/creatures/enemy_00_03_anim.png) |  |
| 0 | 6 | Kobold | ENEMY | 17, 20, 29 | [1:1](../assets/creatures/enemy_00_06.png) · [x2](../assets/creatures/enemy_00_06_x2.png) |  |
| 0 | 8 | Kobold | ENEMY | 17, 20, 29 | [1:1](../assets/creatures/enemy_00_08.png) · [x2](../assets/creatures/enemy_00_08_x2.png) |  |
| 1 | 0 | Skeleton | ENEMY | 91 | [1:1](../assets/creatures/enemy_01_00.png) · [x2](../assets/creatures/enemy_01_00_x2.png) · [anim](../assets/creatures/enemy_01_00_anim.png) |  |
| 1 | 1 | Skeleton | ENEMY | 3, 18 | [1:1](../assets/creatures/enemy_01_01.png) · [x2](../assets/creatures/enemy_01_01_x2.png) · [anim](../assets/creatures/enemy_01_01_anim.png) |  |
| 1 | 3 | Skeleton | ENEMY | 58, 64, 91 | [1:1](../assets/creatures/enemy_01_03.png) · [x2](../assets/creatures/enemy_01_03_x2.png) · [anim](../assets/creatures/enemy_01_03_anim.png) |  |
| 1 | 5 | Skeleton | ENEMY | 13 | [1:1](../assets/creatures/enemy_01_05.png) · [x2](../assets/creatures/enemy_01_05_x2.png) |  |
| 2 | 0 | Troglodyte | ENEMY | 101, 104, 105 | [1:1](../assets/creatures/enemy_02_00.png) · [x2](../assets/creatures/enemy_02_00_x2.png) |  |
| 2 | 4 | Troglodyte | ENEMY | 12 | [1:1](../assets/creatures/enemy_02_04.png) · [x2](../assets/creatures/enemy_02_04_x2.png) · [anim](../assets/creatures/enemy_02_04_anim.png) |  |
| 3 | 0 | Bow Gnoll | ENEMY | hors parcours ; essai forcé en scène 1 | [1:1](../assets/creatures/enemy_03_00.png) · [x2](../assets/creatures/enemy_03_00_x2.png) · [anim](../assets/creatures/enemy_03_00_anim.png) |  |
| 4 | 0 | Owlbear | ENEMY | 90 | [1:1](../assets/creatures/enemy_04_00.png) · [x2](../assets/creatures/enemy_04_00_x2.png) · [anim](../assets/creatures/enemy_04_00_anim.png) |  |
| 4 | 2 | Owlbear | ENEMY | 90, 102 | [1:1](../assets/creatures/enemy_04_02.png) · [x2](../assets/creatures/enemy_04_02_x2.png) |  |
| 5 | 0 | Ghoul | ENEMY | 95 | [1:1](../assets/creatures/enemy_05_00.png) · [x2](../assets/creatures/enemy_05_00_x2.png) |  |
| 5 | 1 | Ghoul | ENEMY | 95, 97 | [1:1](../assets/creatures/enemy_05_01.png) · [x2](../assets/creatures/enemy_05_01_x2.png) |  |
| 5 | 3 | Ghoul | ENEMY | 19 | [1:1](../assets/creatures/enemy_05_03.png) · [x2](../assets/creatures/enemy_05_03_x2.png) |  |
| 5 | 5 | Ghoul | ENEMY | 26 | [1:1](../assets/creatures/enemy_05_05.png) · [x2](../assets/creatures/enemy_05_05_x2.png) |  |
| 6 | 1 | Hellhound | ENEMY | 40 | [1:1](../assets/creatures/enemy_06_01.png) · [x2](../assets/creatures/enemy_06_01_x2.png) · [anim](../assets/creatures/enemy_06_01_anim.png) |  |
| 6 | 6 | Hellhound | ENEMY | 21, 62, 107 | [1:1](../assets/creatures/enemy_06_06.png) · [x2](../assets/creatures/enemy_06_06_x2.png) |  |
| 7 | 0 | Wyvern | ENEMY | 32, 66, 103, 104 | [1:1](../assets/creatures/enemy_07_00.png) · [x2](../assets/creatures/enemy_07_00_x2.png) · [anim](../assets/creatures/enemy_07_00_anim.png) | tentacule : partie attachée à un hôte |
| 7 | 1 | Wyvern | ENEMY | 32, 66, 103, 104 | [1:1](../assets/creatures/enemy_07_01.png) · [x2](../assets/creatures/enemy_07_01_x2.png) | tentacule : partie attachée à un hôte |
| 8 | 0 | Gargoyle | ENEMY | hors parcours ; essai forcé en scène 42 | [1:1](../assets/creatures/enemy_08_00.png) · [x2](../assets/creatures/enemy_08_00_x2.png) · [anim](../assets/creatures/enemy_08_00_anim.png) |  |
| 8 | 1 | Gargoyle | ENEMY | 63 | [1:1](../assets/creatures/enemy_08_01.png) · [x2](../assets/creatures/enemy_08_01_x2.png) |  |
| 9 | 0 | Giant Beetle | ENEMY | 14, 16, 18 | [1:1](../assets/creatures/enemy_09_00.png) · [x2](../assets/creatures/enemy_09_00_x2.png) · [anim](../assets/creatures/enemy_09_00_anim.png) |  |
| 9 | 4 | Giant Beetle | ENEMY | 43, 61, 112 | [1:1](../assets/creatures/enemy_09_04.png) · [x2](../assets/creatures/enemy_09_04_x2.png) · [anim](../assets/creatures/enemy_09_04_anim.png) |  |
| 10 | 0 | Giant Scorpion | ENEMY | 36, 37 | [1:1](../assets/creatures/enemy_10_00.png) · [x2](../assets/creatures/enemy_10_00_x2.png) |  |
| 10 | 5 | Giant Scorpion | ENEMY | 36, 37, 39 | [1:1](../assets/creatures/enemy_10_05.png) · [x2](../assets/creatures/enemy_10_05_x2.png) · [anim](../assets/creatures/enemy_10_05_anim.png) |  |
| 11 | 0 | Mud Golem | ENEMY | hors parcours ; essai forcé en scène 1 | [1:1](../assets/creatures/enemy_11_00.png) · [x2](../assets/creatures/enemy_11_00_x2.png) |  |
| 12 | 0 | Shadow Elf | ENEMY | 92, 94, 96 | [1:1](../assets/creatures/enemy_12_00.png) · [x2](../assets/creatures/enemy_12_00_x2.png) |  |
| 12 | 2 | Shadow Elf | ENEMY | 92, 94, 96 | [1:1](../assets/creatures/enemy_12_02.png) · [x2](../assets/creatures/enemy_12_02_x2.png) · [anim](../assets/creatures/enemy_12_02_anim.png) |  |
| 12 | 4 | Shadow Elf | ENEMY | 5, 11, 92, 94, 96 | [1:1](../assets/creatures/enemy_12_04.png) · [x2](../assets/creatures/enemy_12_04_x2.png) · [anim](../assets/creatures/enemy_12_04_anim.png) | faisceau blanc d'apparition, avant la forme visible |
| 12 | 6 | Shadow Elf | ENEMY | 103, 104 | [1:1](../assets/creatures/enemy_12_06.png) · [x2](../assets/creatures/enemy_12_06_x2.png) | faisceau blanc d'apparition, avant la forme visible |
| 13 | 0 | Axe Gnoll | ENEMY | hors parcours ; essai forcé en scène 1 | [1:1](../assets/creatures/enemy_13_00.png) · [x2](../assets/creatures/enemy_13_00_x2.png) · [anim](../assets/creatures/enemy_13_00_anim.png) |  |
| 14 | 0 | Troll | ENEMY | hors parcours ; essai forcé en scène 1 | [1:1](../assets/creatures/enemy_14_00.png) · [x2](../assets/creatures/enemy_14_00_x2.png) · [anim](../assets/creatures/enemy_14_00_anim.png) |  |
| 15 | 0 | Goblin | ENEMY | hors parcours ; essai forcé en scène 1 | [1:1](../assets/creatures/enemy_15_00.png) · [x2](../assets/creatures/enemy_15_00_x2.png) · [anim](../assets/creatures/enemy_15_00_anim.png) |  |
| 16 | 4 | Riyakaa | ENEMY | 99, 102 | [1:1](../assets/creatures/enemy_16_04.png) · [x2](../assets/creatures/enemy_16_04_x2.png) |  |
| 16 | 8 | Riyakaa | ENEMY | 15, 22, 100 | [1:1](../assets/creatures/enemy_16_08.png) · [x2](../assets/creatures/enemy_16_08_x2.png) |  |
| 18 | 255 | Ogre | ENEMY | 66 | [1:1](../assets/creatures/enemy_18_255.png) · [x2](../assets/creatures/enemy_18_255_x2.png) · [anim](../assets/creatures/enemy_18_255_anim.png) | montre un manticore ailé, pas un Ogre |
| 19 | 0 | Displacer Beast | ENEMY | 1 | [1:1](../assets/creatures/enemy_19_00.png) · [x2](../assets/creatures/enemy_19_00_x2.png) | guerrier à bouclier et bombe (gobelin ou gnoll), pas un Displacer Beast : nom du type hypothétique |
| 19 | 4 | Displacer Beast | ENEMY | 35 | [1:1](../assets/creatures/enemy_19_04.png) · [x2](../assets/creatures/enemy_19_04_x2.png) · [anim](../assets/creatures/enemy_19_04_anim.png) | guerrier à bouclier et bombe (gobelin ou gnoll), pas un Displacer Beast : nom du type hypothétique |
| 19 | 5 | Displacer Beast | ENEMY | 4, 27, 91, 98 | [1:1](../assets/creatures/enemy_19_05.png) · [x2](../assets/creatures/enemy_19_05_x2.png) | guerrier à bouclier et bombe (gobelin ou gnoll), pas un Displacer Beast : nom du type hypothétique |
| 19 | 6 | Displacer Beast | ENEMY | 4, 27, 91, 98 | [1:1](../assets/creatures/enemy_19_06.png) · [x2](../assets/creatures/enemy_19_06_x2.png) | guerrier à bouclier et bombe (gobelin ou gnoll), pas un Displacer Beast : nom du type hypothétique |
| 19 | 7 | Displacer Beast | ENEMY | 0, 1 | [1:1](../assets/creatures/enemy_19_07.png) · [x2](../assets/creatures/enemy_19_07_x2.png) · [anim](../assets/creatures/enemy_19_07_anim.png) | guerrier à bouclier et bombe (gobelin ou gnoll), pas un Displacer Beast : nom du type hypothétique |
| 19 | 8 | Displacer Beast | ENEMY | 1 | [1:1](../assets/creatures/enemy_19_08.png) · [x2](../assets/creatures/enemy_19_08_x2.png) · [anim](../assets/creatures/enemy_19_08_anim.png) | guerrier à bouclier et bombe (gobelin ou gnoll), pas un Displacer Beast : nom du type hypothétique |
| 19 | 9 | Displacer Beast | ENEMY | 28 | [1:1](../assets/creatures/enemy_19_09.png) · [x2](../assets/creatures/enemy_19_09_x2.png) | guerrier à bouclier et bombe (gobelin ou gnoll), pas un Displacer Beast : nom du type hypothétique |
| 19 | 10 | Displacer Beast | ENEMY | 28, 29 | [1:1](../assets/creatures/enemy_19_10.png) · [x2](../assets/creatures/enemy_19_10_x2.png) · [anim](../assets/creatures/enemy_19_10_anim.png) | guerrier à bouclier et bombe (gobelin ou gnoll), pas un Displacer Beast : nom du type hypothétique |
| 19 | 11 | Displacer Beast | ENEMY | 4, 16, 27 | [1:1](../assets/creatures/enemy_19_11.png) · [x2](../assets/creatures/enemy_19_11_x2.png) | guerrier à bouclier et bombe (gobelin ou gnoll), pas un Displacer Beast : nom du type hypothétique |
| 20 | 1 | Man Scorpion | ENEMY | 1 | [1:1](../assets/creatures/enemy_20_01.png) · [x2](../assets/creatures/enemy_20_01_x2.png) | épave de la scène 1 (décor), pas un Man Scorpion |
| 21 | 8 | Harpy | ENEMY | 11 | [1:1](../assets/creatures/enemy_21_08.png) · [x2](../assets/creatures/enemy_21_08_x2.png) · [anim](../assets/creatures/enemy_21_08_anim.png) |  |
| 22 | 0 | Chimera | ENEMY | 104, 105 | [1:1](../assets/creatures/enemy_22_00.png) · [x2](../assets/creatures/enemy_22_00_x2.png) · [anim](../assets/creatures/enemy_22_00_anim.png) | morceau recopié sur un maître (tête ou queue d'un boss), pas une créature entière |
| 23 | 0 | Spectre | ENEMY | 104, 105 | [1:1](../assets/creatures/enemy_23_00.png) · [x2](../assets/creatures/enemy_23_00_x2.png) · [anim](../assets/creatures/enemy_23_00_anim.png) | morceau recopié sur un maître (tête ou queue d'un boss), pas une créature entière |
| 24 | 0 | Manticore | ENEMY | 104, 105 | [1:1](../assets/creatures/enemy_24_00.png) · [x2](../assets/creatures/enemy_24_00_x2.png) · [anim](../assets/creatures/enemy_24_00_anim.png) | morceau recopié sur un maître (tête ou queue d'un boss), pas une créature entière |
| 25 | 0 | Dark Warrior 1 | ENEMY | 105, 113 | [1:1](../assets/creatures/enemy_25_00.png) · [x2](../assets/creatures/enemy_25_00_x2.png) · [anim](../assets/creatures/enemy_25_00_anim.png) | patte du dragon rouge (scène 46), pas un Dark Warrior |
| 25 | 1 | Dark Warrior 1 | ENEMY | 105, 113 | [1:1](../assets/creatures/enemy_25_01.png) · [x2](../assets/creatures/enemy_25_01_x2.png) · [anim](../assets/creatures/enemy_25_01_anim.png) | patte du dragon rouge (scène 46), pas un Dark Warrior |
| 27 | 0 | War Machine | ENEMY | hors parcours ; essai forcé en scène 1 | [1:1](../assets/creatures/enemy_27_00.png) · [x2](../assets/creatures/enemy_27_00_x2.png) · [anim](../assets/creatures/enemy_27_00_anim.png) |  |

## Boss (pool BOSS)

![planche des boss](../assets/creatures/planche_boss.png)

| Type | Variante | Nom | Famille | Scènes (`FF8034`) | Images | Remarque |
|---|---|---|---|---|---|---|
| 4 | 0 | Manticore (type 4) | BOSS | 104, 105 | [1:1](../assets/creatures/boss_04_00.png) · [x2](../assets/creatures/boss_04_00_x2.png) · [anim](../assets/creatures/boss_04_00_anim.png) | manticore |
| 5 | 0 | Ezerhorden | BOSS | 58 | [1:1](../assets/creatures/boss_05_00.png) · [x2](../assets/creatures/boss_05_00_x2.png) · [anim](../assets/creatures/boss_05_00_anim.png) | volutes d'apparition (l'image montre peu de choses avant la matérialisation) |
| 6 | 0 | Manticore | BOSS | 24 | [1:1](../assets/creatures/boss_06_00.png) · [x2](../assets/creatures/boss_06_00_x2.png) · [anim](../assets/creatures/boss_06_00_anim.png) |  |
| 7 | 0 | (sans nom) | BOSS | 91 | [1:1](../assets/creatures/boss_07_00.png) · [x2](../assets/creatures/boss_07_00_x2.png) · [anim](../assets/creatures/boss_07_00_anim.png) | silhouette en transparence tramée |
| 8 | 0 | Dark Warrior 2 | BOSS | 64 | [1:1](../assets/creatures/boss_08_00.png) · [x2](../assets/creatures/boss_08_00_x2.png) · [anim](../assets/creatures/boss_08_00_anim.png) |  |
| 10 | 0 | Nagpa | BOSS | 66, 105 | [1:1](../assets/creatures/boss_10_00.png) · [x2](../assets/creatures/boss_10_00_x2.png) · [anim](../assets/creatures/boss_10_00_anim.png) |  |
| 11 | 0 | Salamandre de glace | BOSS | hors parcours ; essai forcé en scène 42 | [1:1](../assets/creatures/boss_11_00.png) · [x2](../assets/creatures/boss_11_00_x2.png) |  |
| 12 | 0 | Salamandre de feu | BOSS | 40 | [1:1](../assets/creatures/boss_12_00.png) · [x2](../assets/creatures/boss_12_00_x2.png) · [anim](../assets/creatures/boss_12_00_anim.png) |  |
| 13 | 0 | Dragon noir en vol | BOSS | 7, 9 | [1:1](../assets/creatures/boss_13_00.png) · [x2](../assets/creatures/boss_13_00_x2.png) · [anim](../assets/creatures/boss_13_00_anim.png) |  |
| 14 | 0 | Dragon vert | BOSS | 103, 104 | [1:1](../assets/creatures/boss_14_00.png) · [x2](../assets/creatures/boss_14_00_x2.png) · [anim](../assets/creatures/boss_14_00_anim.png) |  |
| 15 | 0 | Dragon rouge | BOSS | 113 | [1:1](../assets/creatures/boss_15_00.png) · [x2](../assets/creatures/boss_15_00_x2.png) · [anim](../assets/creatures/boss_15_00_anim.png) | tête du dragon rouge |
| 17 | 0 | Tel'Arin | BOSS | 13 | [1:1](../assets/creatures/boss_17_00.png) · [x2](../assets/creatures/boss_17_00_x2.png) · [anim](../assets/creatures/boss_17_00_anim.png) |  |
| 17 | 1 | Tel'Arin | BOSS | 111 | [1:1](../assets/creatures/boss_17_01.png) · [x2](../assets/creatures/boss_17_01_x2.png) · [anim](../assets/creatures/boss_17_01_anim.png) |  |
| 18 | 0 | Lich | BOSS | 26, 93, 94 | [1:1](../assets/creatures/boss_18_00.png) · [x2](../assets/creatures/boss_18_00_x2.png) · [anim](../assets/creatures/boss_18_00_anim.png) |  |
| 19 | 0 | Dragon noir | BOSS | 32 | [1:1](../assets/creatures/boss_19_00.png) · [x2](../assets/creatures/boss_19_00_x2.png) · [anim](../assets/creatures/boss_19_00_anim.png) |  |
| 19 | 1 | Dragon noir | BOSS | 66 | [1:1](../assets/creatures/boss_19_01.png) · [x2](../assets/creatures/boss_19_01_x2.png) · [anim](../assets/creatures/boss_19_01_anim.png) |  |
| 20 | 0 | Dragon rouge (type 20) | BOSS | 105 | [1:1](../assets/creatures/boss_20_00.png) · [x2](../assets/creatures/boss_20_00_x2.png) · [anim](../assets/creatures/boss_20_00_anim.png) | tête du dragon rouge |
| 21 | 0 | Compagnon de Tel'Arin | BOSS | 111 | [1:1](../assets/creatures/boss_21_00.png) · [x2](../assets/creatures/boss_21_00_x2.png) · [anim](../assets/creatures/boss_21_00_anim.png) |  |

Types d'ennemis sans image : 26 Dark Warrior 2, 28 Nagpa (jamais rencontrés dans les parcours ; l'objet créé à la main par `spawn` ne montre rien).
