//! List of the native routines: "address name shape side", one per line (body shape: `P` processor, `E` processor
//! with its entry, `M` memory only, `G` object handler without registers; side: `game` for the natives of the game,
//! `tools` for those that only the tools have, for the original code: `ddsom_game::tools`).
fn main() {
    let game: std::collections::HashSet<u32> = ddsom_game::game_natives().map(|n| n.entry).collect();
    for n in ddsom_game::all_natives() {
        let shape = match n.run {
            ddsom_game::Body::WithCpu(_) => 'P',
            ddsom_game::Body::CpuIn(_) => 'E',
            ddsom_game::Body::Memory(_) => 'M',
            ddsom_game::Body::Handler(_) | ddsom_game::Body::HandlerIn(_) => 'G',
        };
        let side = if game.contains(&n.entry) { "game" } else { "tools" };
        println!("{:06x} {} {shape} {side}", n.entry, n.name);
    }
}
