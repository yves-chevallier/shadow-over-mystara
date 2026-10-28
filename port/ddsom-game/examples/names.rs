//! Liste des routines natives : « adresse nom forme », une par ligne (forme du corps : `P` processeur, `E` processeur
//! avec son entrée, `M` mémoire seule).
fn main() {
    for n in ddsom_game::all_natives() {
        let forme = match n.run {
            ddsom_game::Corps::Processeur(_) => 'P',
            ddsom_game::Corps::ProcesseurEn(_) => 'E',
            ddsom_game::Corps::Memoire(_) => 'M',
        };
        println!("{:06x} {} {forme}", n.entry, n.name);
    }
}
