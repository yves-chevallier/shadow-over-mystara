# Code mort et chemins jamais atteints

Règle (demande de l'utilisateur) : avant de supprimer du code mort ou un repli jamais atteint, on consigne ici ce
qu'il faisait, pourquoi il n'est jamais atteint et ce qu'on en a décidé. Un comportement de jeu réel mais
inatteignable (fonction cachée, mode debug, variante abandonnée) se garde en code Rust compilé, marqué
`// Code mort d'origine : …` avec renvoi ici. Une entrée par cas, une section par lot.

## Lot A : ennemis et boss (`families/boss_*`, `enemy*`, `reactions.rs`)

Rien n'a été supprimé : les constantes retirées étaient des adresses recopiées devenues inutiles (les appels
passent par des fonctions), et les aides retirées des doublons renommés (`boss_common::play` → `lib::start`...).

### Routines d'origine sans native, appelées par les familles

Ces routines étaient appelées par adresse depuis des gestionnaires réécrits mais n'avaient pas de native : le jeu
sans 68000 se serait arrêté en les atteignant. Aucun des 269 scénarios ne les atteint (la validation complète,
qui s'arrête sur une adresse sans native, passe). Elles sont maintenant transcrites en Rust dans
`families/enemy_lib.rs` (section « routines d'origine sans native ») d'après `listing.txt`, registres de sortie
compris, et appelées directement. **Non validées par un scénario** : à couvrir (poke, fuzz) quand le chemin sera
atteignable.

| Adresse | Appelant | Ce qu'elle fait | Pourquoi jamais atteinte (h) |
|---|---|---|---|
| 0x10298 | boss 16 (`moves.rs`, réveil) | script de palette : 8 longs de la table 0x102ba copiés en 0x91c380, luminosité maximale | séquence de réveil du boss 16 non jouée par les scénarios (`boss16_poke` démarre après) |
| 0xa2d22 | boss 3 (`attacks.rs`) | six tirs ESHL 0x26 (`+0x2f` = 5..0) liés par `+0x46` | attaque du boss 3 rarement tirée ; pas dans les scénarios |
| 0xffda | boss 3 (`fight.rs`, riposte 0x80a5c) | tirage d'un bit dans un masque (table 0xffd8 par compteur d'images / 8) : attaque 16 ou 20 | riposte non déclenchée dans les scénarios |
| 0xa09ca | Gnoll (`arrival.rs`, lâche sa hache) ; Harpie (`enemy_harpy.rs`) | tir ESHL 0x12 à 0x30 pixels de haut et 0x60 devant ; glitch : la retenue testée par la harpie est celle de l'addition en X | aucune scène de Gnoll armé ni de ce geste de Harpie dans les scénarios |
| 0xa114a | Gnoll (`arrival.rs`, `hit.rs`) | tir ESHL 0x16 de variante D0 (arme reprise / perdue) | idem |
| 0xa0ae0 | Harpie | tir ESHL 0x13 visant la cible (A2), lancé par l'objet | geste de Harpie non joué |
| 0x9f346 | Troll (`attacks.rs`, lancer) | tir ESHL 0xa orienté à l'opposé, lié par `+0x50` | lancer du Troll non joué |
| 0xad7c | réactions (`reactions.rs`, reprise d'état sauvé 0x205) | tirage d'un bit dans la ligne `+0x78` (table 0xadb0) ; faux si `+0x78` < 0 | état sauvé 0x205 jamais rencontré |

### Reste en code d'origine

* 0x818e6, 0x819e2 (formes du corps d'un boss, table 0x81ad4, appelées par `boss_common::push_out_of_boss`) : sans
  native. `+0xa4` (forme) n'est non nul pour aucun boss des scénarios (h). À transcrire avec le boss qui les pose.
