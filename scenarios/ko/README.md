# Scénarios qui divergent à cause d'une autre routine

Ces scénarios (entrées aléatoires `tools/make_bot.py --mash --char 3`, le voleur) ne sont pas dans la
validation courante : ils échouent à cause de la routine native `combat_update` (`src/combat.rs`), pas à
cause du code des personnages. Ils sont gardés pour corriger cette routine.

| scénario | première différence | routine fautive | diagnostic |
|---|---|---|---|
| `mash_c3.txt` | image 6326 | `combat_update` | `DDSOM_SANS=thief` échoue pareil ; `DDSOM_SANS=combat_update` : VALIDE sur 12 000 images. Mode contrôle : `ram[ff1a6a]=0002 au lieu de 0000` (compteur de balayage `scratch::PUSH` de `combat/contact.rs`, `players_push_enemies` ou `players_push_boxes`), puis la sous-action du joueur 0 (`+0x6a`) diffère (0x10 d'un côté, 0x04 de l'autre). |
| `mash_c3b.txt` | image 11138 | `combat_update` | `DDSOM_SANS=thief` échoue pareil ; `DDSOM_SANS=combat_update` : VALIDE sur 11 200 images. Mode contrôle (`DDSOM_CHECK=1 DDSOM_VERBOSE=1`) : à l'image 11138 la native fait toucher le joueur 0 (action 0x22, un objet est créé à ff:e3fe, lié au joueur par `+0x8c` et `+0x50`) alors que l'original le laisse dans son attaque (action 0x0c, sous-action 0x10). |

Reproduire :

```sh
export DDSOM_ROMS=/home/ycr/som/rom DDSOM_EEPROM=/home/ycr/som/testdata/eeprom_ddsom.bin
port/target/release/examples/validate $DDSOM_ROMS $DDSOM_EEPROM scenarios/ko/mash_c3.txt 12000
DDSOM_SANS=combat_update port/target/release/examples/validate $DDSOM_ROMS $DDSOM_EEPROM scenarios/ko/mash_c3.txt 12000
```
