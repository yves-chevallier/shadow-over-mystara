//! Types SET 2 (contrôleur de scène, 0xbc814), 3 (fenêtre de texte, 0xc8352) et 15 (0xce024) : voir
//! `docs/families/set_c.md`.
//! Les tables (scripts d'événements, adresses de jeux d'animation...) restent dans la ROM.

use super::cx::handlers;

#[path = "set_c_scene.rs"]
mod scene;
#[path = "set_c_finale.rs"]
mod finale;
#[path = "set_c_fenetre.rs"]
mod window;

handlers! {
    0xbc814 "set_scene_bc814" #[without] scene::run,
    0xc8352 "set_fenetre_c8352" #[without] window::run,
    0xce024 "set_ecrans_ce024" #[without] finale::run,
}
