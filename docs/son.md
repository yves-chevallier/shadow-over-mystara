# Le pilote son de ddsom (Z80 + QSound)

Rétro-ingénierie du programme du Z80 (`dd2.01`, 0x0000-0x3FFF utilisés, le reste est de la donnée)
et sa réécriture en Rust (crate `port/ddsom-sound`, type `Driver`). Les adresses citées sont celles
de la ROM du Z80 ; rien de ce document n'est tiré de la ROM sauf les adresses et les formats.

## 1. Vue d'ensemble

Le 68000 ne pilote pas la QSound : il dépose des **commandes de 16 octets** dans une RAM partagée
avec le Z80. Le Z80 les range dans une file, démarre des musiques et des effets (pistes lues dans
la ROM), et fait vivre 16 voies de séquenceur + 3 voies ADPCM en écrivant les registres de la QSound.

```
68000 --(RAM partagée C000-CFFF)--> Z80 --(D000-D002)--> QSound (DSP)
         commandes, poignée de main       registres      16 voies PCM + 3 ADPCM + écho
```

Tout est cadencé par une interruption à **250 Hz** (toutes les 32 000 périodes d'horloge à 8 MHz).

## 2. Cartes mémoire

Z80 : 0000-7FFF ROM fixe, 8000-BFFF fenêtre de ROM (banque choisie en D003, valeur | 0x80),
C000-CFFF RAM partagée, D000-D002 écritures QSound (octet haut, octet bas, numéro de registre),
D003 banque, D007 état QSound (bit 7 = prête), F000-FFFF RAM.

Adresse « linéaire » : la ROM est lue avec des pointeurs sur 3 octets. Un pointeur `L < 0x8000`
désigne la ROM fixe ; sinon c'est l'adresse dans la fenêtre 8000-BFFF de la banque
`(L >> 14) - 2` (et, côté fichiers : `L` est le décalage dans `dd2.01` puis `dd2.02`). Le Z80
passe à la banque suivante quand une lecture atteint C000 ; le pilote Rust lit avec l'adresse
linéaire, ce qui revient au même.

### RAM partagée (décalage depuis C000 ; côté 68000 : 0x618000 + 2 x décalage, un octet par mot)

| Décalage | Rôle |
|---|---|
| 000-00F | **boîte aux lettres** : la commande (16 octets). L'octet 00F est le drapeau : le 68000 y laisse une valeur autre que 0xFF avec la commande, le Z80 la copie dans sa file et remet **0xFF** (acquittement). Le 68000 ne poste que s'il voit 0xFF. |
| 005 | rapport du Z80 : dernier pan écrit (0-0x20, 0x10 en mono) ; recouvre l'octet 5 de la commande |
| FFD | 0x88 quand le 68000 a initialisé la boîte aux lettres (sinon la routine d'interruption n'y touche pas) |
| FFE | réglage stéréo recopié par le Z80 (0 = mono : tous les pans sont forcés au centre) |
| FFF | poignée de main : le Z80 écrit **0x77** à la mise en route ; le 68000 finit par y écrire **0xFF** quand il est prêt, et le Z80 attend cet octet avant de traiter quoi que ce soit |

### RAM du Z80 (F000-FFFF) : variables

| Adresse | Nom (dans `Driver`) | Rôle |
|---|---|---|
| F000 | `irq_count` | compteur d'interruptions |
| F001 / F002 / F003 | `pending.commands / ticks / adpcm` | tâches à faire (voir §4) |
| F004 | `is_music` | 0 pendant le traitement d'une voie d'effet, non nul pour une voie de musique |
| F005, F006, F007 | `cur` | voie en cours (F006 = 8 x voie, F007 = 4 x voie ADPCM) |
| F008 | `stereo` | copie de FFE |
| F009 | `sfx_volume` | volume de base des effets |
| F00B | `g.step` | pas de temps de la voie en cours |
| F010, F012-F014 | `song_count`, `song_table` | nombre de sons, adresse de la table |
| F015, F016 | `queue_head/tail` | file de commandes (indices d'octets, 16 entrées de 16 octets en F100) |
| F017-F019 | `song` | adresse (3 octets) de la définition du son en cours de démarrage |
| F01A | | banque courante (non modélisée : le pilote Rust adresse linéairement) |
| F01B | `priority` | priorité de la commande en cours |
| F020, F022 | `music_step`, `tempo_adjust` | pas de temps des voies de musique (instruction « pas »), ajustement de tempo (commande 8) |
| F024, F026 | `music_factor`, `music_volume` | facteur de volume de la musique (en-tête), volume courant |
| F028, F029 | `music_transpose`, `pitch_bend` | transposition globale, variation de hauteur globale |
| F030 | `sfx_override` | une voie d'effet occupe le canal : la voie de musique n'écrit pas dans la QSound |
| F040-F044 | `fade_*` | fondu général (commandes 12, 13) |
| F045-F04B | `ramp_*` | rampe de volume de la musique (commandes 6, 7) |
| F100-F1FF | `queue` | file de commandes |
| F200-F6FF | `sfx[16]` | voies d'effets (16 x 0x50 octets) |
| F700-FBFF | `music[16]` | voies de musique (16 x 0x50 octets) |
| FC00-FC47 | `adpcm[3]` | voies ADPCM (3 x 0x18 octets) |

`Driver::ram_image()` reconstitue cette RAM à partir de l'état du pilote Rust : les outils de
validation la comparent octet par octet à la RAM du Z80 d'origine.

## 3. Démarrage

1. À la mise en route (reset relâché par le 68000), le Z80 écrit 0x77 en CFFF, active les
   interruptions et attend **deux tics** (deux fois quatre interruptions : la routine d'interruption
   incrémente F002 une fois sur quatre).
2. Il initialise la QSound : une table de 12 registres (numéros en 0x00D5, valeurs en 0x00E1 :
   filtres et délais de l'écho, jusqu'au registre 0xE3 inclus), puis coupe les 16 voies et les 3 voies
   ADPCM. Pendant ce temps les interruptions sont **masquées** : avec la remise à zéro de la RAM
   (`LDIR` de 4 Ko, 86 000 cycles) cela fait environ 128 000 cycles, soit 4 périodes de 250 Hz dans
   la machine complète. Trois interruptions sont perdues, une seule est prise au démarrage
   (`INIT_MASKED_PERIODS`).
3. Il lit l'en-tête des sons (adresse en 0x39E1, voir §7), puis réactive les interruptions et boucle
   sur CFFF jusqu'à y lire 0xFF. Ensuite commence la boucle principale.

## 4. Interruption et boucle principale

La routine d'interruption (0x0038) :

- incrémente F000 et, selon ses deux bits bas, un compteur de tâches : `n & 3 == 0` : F002 (tic
  du séquenceur, 62,5 Hz) ; `== 2` : F003 (tic ADPCM, 62,5 Hz) ; impair : F001 (une commande à
  traiter, 125 Hz) ;
- si CFFD vaut 0x88 : si C00F n'est pas 0xFF, copie les 16 octets de la boîte aux lettres dans la
  file (F015 avance de 16) et écrit **0xFF en 00F** ; puis recopie CFFE dans F008.

La boucle principale (0x017D) enchaîne en boucle, dans cet ordre : une commande si F001 > 0 (en
décrémentant), un tic ADPCM si F003 > 0, un tic du séquenceur si F002 > 0.

**Durée d'exécution.** Le travail d'un tic (16 voies, avec des multiplications 16 x 16 bits faites par
décalages, et des écritures QSound qui attendent chacune D007) dépasse souvent 32 000 cycles : il
déborde alors sur l'interruption suivante, et l'ordre relatif d'un acquittement (écrit par la routine
d'interruption à l'instant de l'interruption) et des écritures de la boucle principale dépend de la
durée des instructions du Z80. Le pilote Rust ne modélise pas la durée : une interruption lui fait
faire en une fois tout le travail que le Z80 mène à bien avant l'interruption suivante... ou après.
L'**ordre** des écritures de chaque catégorie est identique ; leur répartition entre deux
interruptions successives peut différer (voir §13).

## 5. Commandes du 68000 (16 octets)

Octets 0-1 : si l'octet 0 vaut **0xFF**, commande système de numéro octet 1 ; sinon numéro de son
(grand-boutiste, pris modulo le nombre de sons). Octets 6-12 : réglage du pan (voir plus bas).

### Commandes système (octet 1 ; paramètres en 2-4)

| N° | Effet |
|---|---|
| 0 | tout couper : fondu général arrêté, voies d'effets et de musique (32 enregistrements), ADPCM |
| 1 | couper tous les effets (la musique du canal reprend) |
| 2 | couper l'effet du canal `octet 4` |
| 3 | couper les 3 voies ADPCM |
| 4 | couper la voie ADPCM `octet 4` |
| 5 | couper la musique (les canaux occupés par un effet continuent) |
| 6 / 7 | rampe du volume de la musique vers le haut / vers le bas : niveau visé = mot de la table des niveaux à l'index `octet 2 & 0xFE`, vitesse = mot `octets 3-4` |
| 8 | ajustement du tempo de la musique : `octet 4 x 0x369 >> 8`, négatif si `octet 3 == 0xFF` |
| 9 | variation de hauteur des voies de musique : `octet 4 << 4`, négative si `octet 3 == 0xFF` |
| 12 / 13 | fondu général de tous les canaux, montant / descendant, vitesse `octets 3-4` |

(Les numéros 10, 11, 14 et 15 sont sans effet ; au-delà de 15 le Z80 saute dans le vide.)

### Démarrage d'un son

Le numéro indexe la table des sons (§7) : entrée de 4 octets `[adresse sur 3 octets, 0]`, nulle = vide.
Le premier octet de la définition donne le type :

| Octet | Type | Contenu |
|---|---|---|
| 0 | **musique** | 16 déplacements (mots grand-boutistes, depuis le début de la définition ; 0 = pas de piste sur ce canal), puis les pistes |
| 1-0x7F | **effet** (c'est la priorité) | idem : 16 déplacements. Une piste ne démarre que si la priorité de la commande est au moins celle de la voie d'effet qu'elle remplace |
| 0x80-0x8F | **note simple** (voie 0-15) | drapeaux/priorité (bit 7 : jouée jusqu'à l'arrêt), durée (mot), temps de tenue (mot), index d'amplitude, instrument (mot), note, accord fin |
| 0x90-0x92 | **ADPCM** (voie 0-2) | drapeaux/priorité, 4 octets ignorés, index d'amplitude, numéro d'échantillon (mot) |

Les 245 entrées vides mises à part, la ROM contient 868 effets, 53 musiques et 2 sons ADPCM ;
aucune note simple (ce chemin n'est donc exercé que par les tests synthétiques).

Démarrer une musique coupe d'abord ce que faisaient les voies de musique, règle le pan des canaux
libres au centre, puis remet à zéro tempo, transposition, variation de hauteur et rampe, et calcule le
volume de la musique : `hi16(hi16(0xFFFE, sfx_volume), music_factor)`.

### Pan (octets 6-12, pour effets, notes simples et ADPCM)

Position de 0 (gauche) à 0x20 (droite), registre QSound = 0x110 + pan. L'octet 6 choisit comment le
pan évolue : 0 fixe (octet 7) ; 1 et 2 va-et-vient ; 3 et 4 rampe vers la droite / gauche (octet 8 =
amplitude, octets 9-10 = vitesse en 8.8) ; 5 et 6 rampe en deux temps (octets 11-12 = deuxième
vitesse) ; 7 rampe vers la droite **et** copie symétrique sur le canal voisin après 4 tics. Détail dans
`port/ddsom-sound/src/pan.rs`.

## 6. Pistes

Une piste est une suite d'octets lue par une voie. Un octet **>= 0x20** est une note : bits 7-5 =
durée (1-7, table 0x1119 : mots qui doublent à chaque cran ; la 2e série est pour les
pointées, la 3e pour les triolets), bits 4-0 = note de 1 à 31 (0 = silence) à laquelle s'ajoute le
décalage de l'octave (table 0x1149, indexée par les 4 bits bas des drapeaux). Un octet **< 0x20**
est une instruction, suivie de ses paramètres (tableau de saut en 0x1179) :

| Op | Rôle | Paramètres |
|---|---|---|
| 0 / 1 / 2 / 3 | bascule triolet / bascule liaison / prochaine note pointée / bascule +2 octaves | |
| 4 | remplace les drapeaux 6, 5 et 3 | 1 |
| 5 | pas de temps (de la voie, ou de toute la musique) | 2 |
| 6 | proportion de la durée où la note est tenue | 1 |
| 7 | amplitude de la voie (table 0x33E1) | 1 |
| 8 | instrument (index dans le banc courant) | 1 |
| 9 | octave 0-7 | 1 |
| 10 / 11 / 12 | transposition de la musique / de la voie / accord fin | 1 |
| 13 | glissando (vitesse x 2) | 1 |
| 14-17 | début de boucle (compteur 0-3) : tours, saut en arrière | 1 + 2 |
| 18-21 | sortie de boucle au dernier tour : drapeaux, saut | 1 + 2 |
| 22 | saut relatif | 2 |
| 23 | fin de piste | |
| 24 | pan (musique seulement) | 1 |
| 25 | envoi vers l'écho (table 0x33E1, x4) | 1 |
| 26 | volume de la musique (table 0x34E1) | 1 |
| 27 / 28 / 29 / 30 | profondeur de vibrato / profondeur de trémolo / vitesse du LFO / mode du LFO | 1 |
| 31 | banc d'instruments (table 0x39E5) | 1 |

Les sauts sont des déplacements signés grand-boutistes, relatifs à l'adresse qui suit le
déplacement. Une piste qui boucle sans jamais rencontrer de note gèle le Z80 d'origine ; le pilote
Rust gèle de même son séquenceur (`hung`) au bout de 100 000 instructions.

Les drapeaux de piste (`flag` dans `voice.rs`) : bits 0-2 octave, bit 3 deux octaves de plus, bit 4
pointée (effacé à chaque note), bit 5 triolet, bit 6 « lier à la suivante », bit 7 « note coulée »
(la note qui suit une note liée change seulement la hauteur : ni attaque ni nouvel échantillon).

## 7. Données

### En-tête des sons

Le mot en 0x39E1 (0x9000) est l'adresse de l'en-tête : `[nombre de sons (mot grand-boutiste), index
du facteur de volume de la musique, index du volume de base des effets]`, puis la table des sons
(entrées de 4 octets) dès l'octet 6. Les deux index pointent la table de niveaux (mots en 0x34E1).

### Tables de la ROM (mots petit-boutistes)

| Adresse | Contenu |
|---|---|
| 0x33E1 | amplitudes de voie (index de 0 à 127) |
| 0x34E1 | niveaux : volumes de base, cibles des rampes, niveau de tenue des enveloppes |
| 0x35E1 | vitesses d'attaque |
| 0x3661 | vitesses de chute (decay, chute lente, relâchement) |
| 0x36E1 / 0x37E1 / 0x38E1 | vitesses du LFO / profondeurs de vibrato / de trémolo |
| 0x39E1 | mot : adresse de l'en-tête des sons |
| 0x39E3 | mot : adresse de la table des échantillons |
| 0x39E5 | table des adresses des bancs d'instruments (mots) |
| 0x1FC6 / 0x2032 | octave de chaque demi-ton (0-107) / début de chaque octave |
| 0x203B | taux de lecture de base de chaque demi-ton (mots) |
| 0x2113 | interpolation fine : 12 x 256 octets |
| 0x1119 / 0x1149 | durées des notes / décalage d'octave |

### Échantillons (table dont l'adresse est le mot en 0x39E3, à 0x49E9)

Fiches de 8 octets : `[banque, début, début de boucle, fin, note de référence]` (mots petit-
boutistes). La note de référence est celle de la hauteur nominale de l'échantillon (voir §9). La
lecture d'une fiche donne la banque, le début (registre +1), la fin (+5) et la longueur de boucle
`fin - début de boucle` (+4). *(Lue depuis 0x49E8, la table paraît faite de fiches
`[note, banque, début, boucle, fin]` : la note de la fiche n est alors l'octet qui précède.)*

### Instruments (bancs de 256 fiches de 8 octets)

- instruction « instrument » des pistes : `[échantillon (mot, bit 15 = suit la variation de
  hauteur globale), accord fin, attaque, decay, niveau de tenue, chute lente, relâchement]` ; les
  cinq derniers octets sont des index dans les tables ci-dessus ;
- note simple : `[échantillon bas, échantillon haut & 0x7F, décalage de hauteur (signé), 5 index
  d'enveloppe]`, le banc étant choisi par l'octet haut de l'instrument (index dans la table 0x39E5).

## 8. Voies de séquenceur (`Voice`, 0x50 octets)

Deux jeux de 16 voies, un par canal QSound : effets (`sfx`) et musique (`music`). Une voie d'effet
active sur un canal masque la voie de musique du même canal (elle continue de se dérouler, sans
écrire dans la QSound) ; quand l'effet finit, la musique reprend (échantillon rechargé, pan
rétabli, note redéclenchée si la voie était coulée).

États : 0 libre ; 0xFF piste séquencée ; 1 note simple.
Champs principaux (décalage dans l'enregistrement) : 00 état, 01-03 adresse de lecture, 04 drapeaux,
05-06 temps avant l'évènement suivant (8.8), 07 proportion de tenue, 08-09 temps avant relâchement,
0A transposition, 0B et 0D accords fins, 0C note de référence, 0E-0F début d'échantillon, 10-13
glissando, 14-15 hauteur courante (octet haut = demi-ton, bas = fraction), 16-17 hauteur visée, 18-19
amplitude, 1A-1B envoi d'écho, 1C-1F compteurs de boucles, 20 drapeaux (bit 0 : hauteur à réécrire ;
bit 7 : suit la variation globale), 25-2D LFO, vibrato et trémolo, 2E-2F instrument, 30-31 banc
d'instruments, 33 phase d'enveloppe, 34-3F enveloppe, 40 priorité, 41-42 pas de temps, 43-44 note
simple, 46-4F pan. Les champs de `Voice` portent ces noms ; `Voice::image()` donne l'enregistrement
d'origine.

### Tic d'une voie

1. **Avancer le temps** (piste) : le temps de tenue diminue du pas de temps (la phase d'enveloppe
   passe à « relâchement » à zéro) ; le temps avant l'évènement aussi ; tant qu'il reste du temps, on
   passe à l'étape 3.
2. **Évènement** : instructions, puis note. Durée = table (mode pointé/triolet) ; hauteur = note +
   octave + transposition + accord fin, bornée à 0x6B demi-tons (en 8.8) ; glissando éventuel depuis la
   hauteur précédente ; temps de tenue = durée x proportion ; si la note n'est pas coulée : LFO remis
   à zéro (mode bit 0), enveloppe en attaque, **déclenchement** (§12). Un silence ne fait rien d'autre
   que de rallonger l'attente.
3. **Pan** (rampe selon le mode), **LFO** (la phase avance du taux ; sa retenue inverse le sens),
   **glissando**, **enveloppe**, puis écriture du volume et de la hauteur.

### Enveloppe

Phases : 0 attaque (le niveau monte de la vitesse d'attaque, saturé à 0xFFFF puis phase 1), 1 chute
(descend de la vitesse de chute jusqu'au niveau de tenue ; niveau de tenue nul : fin), 2 tenue (rien
ne bouge), 3 chute lente jusqu'à zéro, 4 relâchement jusqu'à zéro, 5 fin. Le relâchement est déclenché
par la fin du temps de tenue. En phases 2 et 5 rien n'est écrit.

### Volume et hauteur écrits

Volume (registre `8v + 6`) : `hi16(...)` en chaîne de : volume de base (effets : `sfx_volume` ; musique :
`music_volume`, x niveau de rampe si une rampe est active) x niveau de fondu général (si actif) x niveau
d'enveloppe x (0xFFFF - trémolo si profondeur non nulle) x amplitude de la voie, où `hi16(a, b)` est les
16 bits de poids fort du produit.

Hauteur (registre `8v + 2`) : hauteur 8.8 (+ vibrato = profondeur x LFO, signé selon le mode, + variation
globale si le bit 7 est posé) ; `n = (demi-ton - 1) + 0x3C - note de référence`, borné ; `octave =
table1[n]`, `d = n - table2[octave]` ; taux = `table_taux[n] + (interpolation[d][fraction] << octave)`.
La hauteur n'est réécrite que si elle a changé (bit 0), ou si un LFO ou un glissando la fait bouger.
*(Le bornage de l'original est atypique : au-delà de 0x6B il rend 0x6B si le bit de signe de
`n - 0x6C` est posé, sinon 0 ; le pilote Rust le reproduit.)*

## 9. Registres de la QSound utilisés

| Registre | Rôle |
|---|---|
| `8v + 0` | **banque** de la voie `v + 1` (la puce attribue le registre 0 à la voie suivante) : `0x8000 | banque` |
| `8v + 1` | adresse de début de l'échantillon |
| `8v + 2` | taux de lecture (hauteur) ; 0 puis 0x1000 à l'arrêt |
| `8v + 3` | phase : 0x8000 au déclenchement |
| `8v + 4` | longueur de la boucle |
| `8v + 5` | adresse de fin |
| `8v + 6` | volume |
| `0x80 + v` | pan (0x110 + 0..0x20) |
| `0xBA + v` | envoi vers l'écho |
| `0x93`, `0xD9` | rebouclage et longueur de l'écho (0x27FF et 0x07FF à chaque déclenchement) |
| `0xCA/CB/CC/CD + 4a` | ADPCM `a` : début, fin, banque (`0x8000 | banque`), volume |
| `0xD6 + a` | ADPCM `a` : drapeau (1 = arrêt/départ) |
| `0x90 + a` | pan de l'ADPCM `a` |
| `0xDA-0xE7` | initialisation : filtres, délais, volumes d'écho |

**Arrêt d'un canal** (`silence_voice`) : écho 0, volume 0, taux 0, taux 0x1000. **Déclenchement**
(`key_on`) : taux 0, 0x93, 0xD9, écho, début, phase 0x8000, puis volume et hauteur.

## 10. Voies ADPCM

Trois canaux, démarrés par une commande dont le type de son vaut 0x90-0x92. La fiche de
l'échantillon (voir §7) donne banque, début et fin (les octets de début de boucle sont ignorés).
Chaque tic ADPCM : compte à rebours de départ différé ; au premier tic les registres sont écrits (début,
fin, banque, volume x fondu, drapeau) puis le pan ; ensuite la durée restante diminue de 0x40 par tic
et le pan évolue ; à zéro la voie s'arrête, ou repart si elle est « continue ». Volume = `hi16(amplitude,
sfx_volume) << 2`.

## 11. Priorités

Une voie d'effet porte la priorité du son qui l'a démarrée (0 libre). Un son ne démarre sur une voie
que si sa priorité est au moins égale à celle de la voie ; arrêter une voie la remet à 0. Les musiques
n'ont pas de priorité (elles remplacent toujours la musique en cours).

## 12. Particularités de l'original reproduites

- le registre de banque d'une voie est celui de la voie précédente ;
- le bornage de la hauteur (voir §8) ;
- `f01b` (priorité) garde l'octet brut dans les voies ADPCM et la valeur masquée dans les notes
  simples ; le démarrage d'une musique y laisse l'ancien état de la dernière voie ;
- l'acquittement n'a lieu que si CFFD vaut 0x88, et la copie de CFFE aussi ;
- les commandes de numéro de voie ou de type invalides ne sont pas gardées (le Z80 d'origine
  sauterait dans la ROM n'importe où) : le pilote Rust les ignore ;
- le pilote Rust n'a pas d'équivalent aux écritures du numéro de banque (D003), qui ne sont pas
  observables côté QSound.

## 13. Validation et écarts

Outils (`port/ddsom-sound/examples`, à lancer depuis `port/` avec `DDSOM_ROMS=/home/ycr/som/rom`) :

- `son_compare <roms> <eeprom> <scénario> <images>` : machine complète (68000 recompilé + Z80 + QSound)
  sur un scénario ; pour chaque interruption on relève les écritures du Z80 (QSound, RAM partagée)
  et on les compare à celles du pilote Rust nourri de ce que le Z80 a lu dans la RAM partagée ;
- `son_fuzz <roms> songs|random|synth|short|one ...` : le Z80 d'origine **seul** (sans 68000 ni
  QSound) et le pilote Rust reçoivent les mêmes commandes ; on compare écritures et, quand le Z80 est
  au repos, la RAM F000-FFFF à `Driver::ram_image()` ; la couverture du programme du Z80 est
  mesurable (`FUZZ_COVERAGE`) ;
- `son_machine` : machine complète avec le pilote Rust à la place du Z80 (`Sound::use_rust_driver`),
  comparée image par image à la machine d'origine (RAM du 68000, RAM partagée, écritures QSound) ;
- `son_explore`, `son_timing` : journal d'évènements et couverture du Z80, durée du masquage des
  interruptions au démarrage.

Résultats (ROMs dans `/home/ycr/som/rom`, `cd port && DDSOM_ROMS=/home/ycr/som/rom cargo build --release -p ddsom-sound --examples`) :

| Essai | Résultat |
|---|---|
| `son_compare`, 30 000 images de `bot30k` | 158 504 écritures (158 016 QSound, 488 RAM partagée) du Z80 : toutes retrouvées, dans le même ordre, dans le pilote Rust (158 327 écritures QSound/pan et 177 acquittements comparés) |
| `son_machine`, 30 000 images, pilote Rust à la place du Z80 | 158 016 écritures QSound identiques dans l'ordre ; RAM du 68000 identique à chaque image ; RAM partagée identique sauf l'octet 5 (rapport de pan, en avance ou en retard d'un tic) sur 29 images |
| `son_fuzz songs 0 1167 1500` (tous les sons de la ROM) | 1 168 / 1 168 identiques (773 938 écritures), état interne compris |
| `son_fuzz random` (suites aléatoires de commandes) | 1 700 / 1 700 (plus de 3 millions d'écritures) |
| `son_fuzz synth` / `short` (pistes synthétiques) | tous les essais comparables sont identiques (887 sur 900 et 2 902 sur 3 000 ; les autres sont abandonnés, voir ci-dessous) |
| `cargo test --release -p ddsom-sound` | version courte des essais précédents |

La couverture du programme du Z80 par ces essais est complète, à quelques branches de retenue des
multiplications près (une addition qui déborde, jamais observée) et au passage de banque pendant
la lecture d'un en-tête de définition.

### Écart restant : répartition entre interruptions

Le pilote Rust fait en une fois, à chaque interruption, le travail que le Z80 mène à bien avant la
suivante ; le Z80, lui, déborde souvent sur l'interruption suivante (les écritures QSound attendent
D007, les multiplications se font par décalages). Conséquences mesurées sur `bot30k` :

- les **suites** d'écritures (QSound, rapport de pan, acquittements) sont identiques, mais on ne peut
  pas comparer en bloc, interruption par interruption : l'acquittement de commande (écrit par la routine
  d'interruption) est comparé séparément des écritures de la boucle principale ;
- à la fin d'une interruption, l'état des registres QSound est identique dans 97 % des cas (121 425 sur 125 389) ;
  les autres sont des tics que le Z80 n'a pas fini d'écrire ;
- quand le Z80 est occupé plus d'une période de plus que prévu par le séquenceur (piste synthétique qui
  boucle longtemps sans note), l'ordre entre une commande et le travail en attente dépend de la durée
  d'exécution : les essais de validation n'envoient une commande que quand le Z80 a rattrapé son retard.

Reproduire la répartition exacte exigerait de modéliser la durée de chaque instruction du Z80 et les
attentes sur D007 de la QSound émulée ; ce n'est pas fait.

## 14. Utilisation

```rust
let mut driver = ddsom_sound::Driver::new(ddsom_sound::Rom::load(rom_dir)?);
let mut out = Vec::new();
driver.reset(&mut shared, &mut out);          // écrit 0x77 en CFFF
loop { driver.irq_request(&mut shared, &mut out); /* toutes les 32 000 périodes Z80 */ }
```

`shared` est la RAM partagée (4 096 octets, un octet par mot côté 68000) ; `out` reçoit les
`Write::Qsound { reg, data }` à appliquer à la puce (les écritures dans la RAM partagée sont déjà faites
dans `shared`, et rappelées dans `out` pour les outils). Dans `cps2`, `Sound::use_rust_driver()` (ou `DDSOM_SON_RUST=1` pour `jouer.sh`) fait
tout cela à la place du Z80.
