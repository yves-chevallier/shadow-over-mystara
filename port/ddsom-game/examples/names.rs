//! Liste des routines natives : « adresse nom forme », une par ligne (forme du corps : `P` processeur, `E` processeur
//! avec son entrée, `M` mémoire seule, `G` gestionnaire
//! d'objet sans registres).
fn main() {
    for n in ddsom_game::all_natives() {
        let forme = match n.run {
            ddsom_game::Corps::Processeur(_) => 'P',
            ddsom_game::Corps::ProcesseurEn(_) => 'E',
            ddsom_game::Corps::Memoire(_) => 'M',
            ddsom_game::Corps::Gestionnaire(_) => 'G',
        };
        println!("{:06x} {} {forme}", n.entry, n.name);
    }
}
