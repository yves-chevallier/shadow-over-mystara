//! Liste des routines natives : « adresse nom forme », une par ligne (forme du corps : `P` processeur, `E` processeur
//! avec son entrée, `M` mémoire seule, `G` gestionnaire
//! d'objet sans registres).
fn main() {
    for n in ddsom_game::all_natives() {
        let shape = match n.run {
            ddsom_game::Body::Processeur(_) => 'P',
            ddsom_game::Body::CpuIn(_) => 'E',
            ddsom_game::Body::Memory(_) => 'M',
            ddsom_game::Body::Handler(_) => 'G',
        };
        println!("{:06x} {} {shape}", n.entry, n.name);
    }
}
