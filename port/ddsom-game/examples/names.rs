//! Liste des routines natives : « adresse nom », une par ligne.
fn main() {
    for n in ddsom_game::all_natives() {
        println!("{:06x} {}", n.entry, n.name);
    }
}
