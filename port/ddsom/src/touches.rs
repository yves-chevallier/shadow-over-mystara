//! Touches configurables : `touches.cfg` (créé avec les touches par défaut s'il manque) associe à chaque
//! action une liste de noms de touches minifb (`Left`, `LeftCtrl`, `A`, `Key1`, `NumPad0`...).
//! Les lettres et les chiffres suivent la disposition du clavier (en AZERTY, `Key1` demande Maj) ;
//! `DDSOM_TOUCHES=1` affiche le nom des touches pressées pour remplir le fichier.

use std::path::Path;

use minifb::{Key, Window};

/// Port d'entrée CPS2 touché par une action (actif à 0).
#[derive(Clone, Copy)]
pub enum Port {
    In0(u16),
    In2(u16),
    Pause,
    Rapide,
    Quitter,
}

/// Actions : nom dans le fichier, effet, touches par défaut (comme MAME).
const ACTIONS: &[(&str, Port, &str)] = &[
    ("j1.droite", Port::In0(0x01), "Right"),
    ("j1.gauche", Port::In0(0x02), "Left"),
    ("j1.bas", Port::In0(0x04), "Down"),
    ("j1.haut", Port::In0(0x08), "Up"),
    ("j1.bouton1", Port::In0(0x10), "LeftCtrl W"),
    ("j1.bouton2", Port::In0(0x20), "LeftAlt X"),
    ("j1.bouton3", Port::In0(0x40), "Space C"),
    ("j1.bouton4", Port::In0(0x80), "LeftShift V"),
    ("j2.droite", Port::In0(0x0100), "G"),
    ("j2.gauche", Port::In0(0x0200), "D"),
    ("j2.bas", Port::In0(0x0400), "F"),
    ("j2.haut", Port::In0(0x0800), "R"),
    ("j2.bouton1", Port::In0(0x1000), "A"),
    ("j2.bouton2", Port::In0(0x2000), "S"),
    ("j2.bouton3", Port::In0(0x4000), "Q"),
    ("j2.bouton4", Port::In0(0x8000), ""),
    ("test", Port::In2(0x0002), "F2"),
    ("service", Port::In2(0x0004), "Key9"),
    ("j1.start", Port::In2(0x0100), "Key1"),
    ("j2.start", Port::In2(0x0200), "Key2"),
    ("j1.piece", Port::In2(0x1000), "Key5"),
    ("j2.piece", Port::In2(0x2000), "Key6"),
    ("pause", Port::Pause, "P"),
    ("rapide", Port::Rapide, "Tab"),
    ("quitter", Port::Quitter, "Escape"),
];

/// Touches de chaque action, par nom minifb.
pub struct Touches {
    actions: Vec<(Port, Vec<String>)>,
    trace: bool,
    avant: Vec<Key>,
    pause_avant: bool,
}

/// Lecture du clavier pour une image.
pub struct Etat {
    /// Ports d'entrée CPS2 (actifs à 0).
    pub in0: u16,
    pub in2: u16,
    /// Pause vient d'être pressée.
    pub pause: bool,
    pub rapide: bool,
    pub quitter: bool,
}

impl Touches {
    /// Lit `fichier` (lignes `action = touches...`, `#` commentaire) ; l'écrit avec les défauts s'il manque.
    pub fn charge(fichier: &Path) -> Self {
        let mut actions: Vec<(Port, Vec<String>)> =
            ACTIONS.iter().map(|&(_, port, keys)| (port, noms(keys))).collect();
        match std::fs::read_to_string(fichier) {
            Ok(texte) => {
                for (n, ligne) in texte.lines().enumerate() {
                    let ligne = ligne.split('#').next().unwrap_or("").trim();
                    if ligne.is_empty() {
                        continue;
                    }
                    let Some((nom, keys)) = ligne.split_once('=') else {
                        eprintln!("{}:{} : « action = touches » attendu", fichier.display(), n + 1);
                        continue;
                    };
                    match ACTIONS.iter().position(|a| a.0 == nom.trim()) {
                        Some(i) => actions[i].1 = noms(keys),
                        None => eprintln!("{}:{} : action inconnue « {} »", fichier.display(), n + 1, nom.trim()),
                    }
                }
            }
            Err(_) => {
                if let Err(e) = std::fs::write(fichier, defauts()) {
                    eprintln!("Écriture de {} impossible : {e}", fichier.display());
                }
            }
        }
        let trace = std::env::var("DDSOM_TOUCHES").is_ok_and(|v| v == "1");
        Touches { actions, trace, avant: Vec::new(), pause_avant: false }
    }

    /// État des actions d'après les touches tenues.
    pub fn lit(&mut self, win: &Window) -> Etat {
        let bas = win.get_keys();
        if self.trace && bas != self.avant {
            let nouvelles: Vec<_> = bas.iter().filter(|k| !self.avant.contains(k)).collect();
            if !nouvelles.is_empty() {
                eprintln!("touches : {nouvelles:?}");
            }
            self.avant = bas.clone();
        }
        let bas: Vec<String> = bas.iter().map(|k| format!("{k:?}")).collect();
        let mut e = Etat { in0: 0xffff, in2: 0xffff, pause: false, rapide: false, quitter: false };
        let mut pause = false;
        for (port, keys) in &self.actions {
            if keys.iter().any(|k| bas.contains(k)) {
                match *port {
                    Port::In0(bit) => e.in0 &= !bit,
                    Port::In2(bit) => e.in2 &= !bit,
                    Port::Pause => pause = true,
                    Port::Rapide => e.rapide = true,
                    Port::Quitter => e.quitter = true,
                }
            }
        }
        e.pause = pause && !self.pause_avant;
        self.pause_avant = pause;
        e
    }
}

fn noms(keys: &str) -> Vec<String> {
    keys.split_whitespace().map(String::from).collect()
}

/// Contenu du fichier créé quand il manque.
fn defauts() -> String {
    let mut s = String::from(
        "# Touches du port ddsom : action = noms de touches minifb séparés par des espaces (vide = aucune).\n\
         # Noms : Left Right Up Down, LeftCtrl RightCtrl LeftAlt LeftShift Space Enter Tab Escape,\n\
         # A..Z, Key0..Key9, NumPad0..NumPad9, F1..F12... Lettres et chiffres suivent la disposition du clavier ;\n\
         # DDSOM_TOUCHES=1 ./jouer.sh affiche le nom des touches pressées.\n\n",
    );
    for (nom, _, keys) in ACTIONS {
        s += &format!("{nom} = {keys}\n");
    }
    s
}
