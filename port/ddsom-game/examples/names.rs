//! List of the native routines: "address name shape", one per line (body shape: `P` processor, `E` processor
//! with its entry, `M` memory only, `G` object handler
//! without registers).
fn main() {
    for n in ddsom_game::all_natives() {
        let shape = match n.run {
            ddsom_game::Body::WithCpu(_) => 'P',
            ddsom_game::Body::CpuIn(_) => 'E',
            ddsom_game::Body::Memory(_) => 'M',
            ddsom_game::Body::Handler(_) => 'G',
        };
        println!("{:06x} {} {shape}", n.entry, n.name);
    }
}
