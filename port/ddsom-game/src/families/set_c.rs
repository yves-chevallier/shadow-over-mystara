//! Types SET 2 (contrôleur de scène, 0xbc814) et 15 (0xce024) : voir `docs/familles/set_c.md`.
//! Les tables (scripts d'événements, adresses de jeux d'animation...) restent dans la ROM.

use super::cx::handlers;

#[path = "set_c_scene.rs"]
mod scene;
#[path = "set_c_finale.rs"]
mod finale;

handlers! {
    0xbc814 "set_scene_bc814" 300 scene::run,
    0xce024 "set_ecrans_ce024" 600 finale::run,
}
