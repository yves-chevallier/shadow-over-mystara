//! Table des tâches en Rust : remplace les enregistrements FF0064 du noyau d'origine (voir `docs/noyau.md`).
//!
//! 16 emplacements, numérotés par leur décalage dans la table d'origine divisé par 16 (`kill(0x30)` : emplacement 3).
//! L'ordonnanceur les examine de 0 à 15 (dans l'original, D0 décompte alors de 15 à 0). Chaque emplacement porte
//! l'état et le compte d'attente de l'original, la pile 68000 donnée à la tâche et ce qu'il faut pour la reprendre :
//! son entrée tant qu'elle n'a pas tourné, puis le contexte du 68000 sauvé à sa dernière cession (registres,
//! adresse de retour de l'enveloppe de cession, SR et pointeur de pile). Les adresses de reprise sont des natives
//! quand la tâche est réécrite (`resume!`), du code d'origine sinon.
//!
//! La table vit dans le bus (`Cps2Bus::ext`) : la machine du jeu n'écrit plus FF0052-FF01A5 (pile de
//! l'ordonnanceur, tâche courante, tas des piles, enregistrements), et les enveloppes de cession ne sauvent plus les
//! registres sur la pile de la tâche. [`ecarts`] compare la table à celle de la RAM d'une machine d'origine.

use cps2::Cps2Bus;

/// Nombre d'emplacements.
pub const EMPLACEMENTS: usize = 16;
/// Adresse de l'enregistrement d'origine de l'emplacement 0 ; l'emplacement k est à `+ 0x10 k`.
pub const ENREGISTREMENTS: u32 = 0xff_0064;

/// Adresse (fictive dans la machine du jeu) de l'enregistrement de l'emplacement `k`, que l'original laisse dans A0.
pub fn enregistrement(k: usize) -> u32 {
    ENREGISTREMENTS + 0x10 * k as u32
}

/// Emplacement désigné par un décalage dans la table (octet bas de D0 des traps).
pub fn emplacement(decalage: u32) -> usize {
    (decalage as usize & 0xff) / 0x10
}

/// État d'un emplacement (octet +0 de l'enregistrement d'origine).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Etat {
    #[default]
    Libre,
    /// Attend que le VBLANK ait décompté son compte jusqu'à zéro.
    Attend,
    /// Attend qu'une autre tâche la réveille (trap 6).
    Suspendue,
    /// Sera lancée au prochain passage de l'ordonnanceur.
    Prete,
    /// Lancée, n'a pas encore cédé la main.
    EnCours,
    /// Créée (trap 0) ou relancée (trap 7) : démarrera à son entrée.
    Neuve,
}

impl Etat {
    /// Octet d'état de l'original.
    pub fn octet(self) -> u8 {
        match self {
            Etat::Libre => 0,
            Etat::Attend => 1,
            Etat::Suspendue => 2,
            Etat::Prete => 4,
            Etat::EnCours => 8,
            Etat::Neuve => 0xc,
        }
    }

    pub fn depuis_octet(octet: u8) -> Option<Etat> {
        [Etat::Libre, Etat::Attend, Etat::Suspendue, Etat::Prete, Etat::EnCours, Etat::Neuve]
            .into_iter()
            .find(|e| e.octet() == octet)
    }

    /// L'ordonnanceur lance la tâche (état ≥ 4 dans l'original).
    pub fn lancable(self) -> bool {
        matches!(self, Etat::Prete | Etat::EnCours | Etat::Neuve)
    }
}

/// Contexte du 68000 d'une tâche qui a cédé la main.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contexte {
    /// Adresse de reprise : retour de l'enveloppe de cession (`jsr 0xa40`...).
    pub pc: u32,
    pub sr: u16,
    /// Pointeur de pile utilisateur à la reprise (adresse de retour dépilée).
    pub usp: u32,
    pub d: [u32; 8],
    pub a: [u32; 7],
}

/// Où reprendre une tâche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reprise {
    /// Pas encore lancée : son entrée.
    Entree(u32),
    /// A cédé la main.
    Cession(Contexte),
}

#[derive(Clone, Copy, Debug)]
pub struct Emplacement {
    pub etat: Etat,
    /// Images d'attente restantes (gardé tel quel par les états qui ne s'en servent pas, comme l'original).
    pub compte: u8,
    /// Pile 68000 donnée à la tâche à sa création, rendue à sa mort.
    pub pile: u32,
    pub reprise: Reprise,
}

impl Default for Emplacement {
    fn default() -> Self {
        Emplacement { etat: Etat::Libre, compte: 0, pile: 0, reprise: Reprise::Entree(0) }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Table {
    pub emplacements: [Emplacement; EMPLACEMENTS],
    /// Dernière tâche lancée (celle qui cède quand une enveloppe de cession est appelée).
    pub courante: usize,
    /// Piles libres : la dernière est la prochaine donnée (le tas FF005A de l'original, rempli par `boot_init`).
    pub piles: Vec<u32>,
}

impl Table {
    /// Trap 0 : la tâche `k` (re)part de `entree` avec une pile neuve ; celle d'une tâche déjà là est rendue.
    pub fn creer(&mut self, k: usize, entree: u32) {
        if self.emplacements[k].etat != Etat::Libre {
            self.rendre_pile(k);
        }
        let pile = self.piles.pop().expect("plus de pile libre pour une tâche");
        self.emplacements[k] = Emplacement { etat: Etat::Neuve, compte: 0, pile, reprise: Reprise::Entree(entree) };
    }

    /// Trap 1 (la tâche se termine) et trap 2 sur une tâche vivante.
    pub fn liberer(&mut self, k: usize) {
        self.emplacements[k].etat = Etat::Libre;
        self.rendre_pile(k);
    }

    /// Trap 2 : tue la tâche `k`, si elle existe.
    pub fn tuer(&mut self, k: usize) {
        if self.emplacements[k].etat != Etat::Libre {
            self.liberer(k);
        }
    }

    fn rendre_pile(&mut self, k: usize) {
        self.piles.push(self.emplacements[k].pile);
    }

    /// Trap 5 : suspend la tâche `k`.
    pub fn suspendre(&mut self, k: usize) {
        let e = &mut self.emplacements[k];
        e.etat = Etat::Suspendue;
        e.compte = 0;
    }

    /// Trap 6 : la tâche `k` redevient prête (avec ce compte, que rien ne lit).
    pub fn reveiller(&mut self, k: usize, compte: u8) {
        let e = &mut self.emplacements[k];
        e.etat = Etat::Prete;
        e.compte = compte;
    }

    /// Trap 7 : la tâche courante repart de `entree` (même pile).
    pub fn recommencer(&mut self, entree: u32) {
        let e = &mut self.emplacements[self.courante];
        e.etat = Etat::Neuve;
        e.compte = 0;
        e.reprise = Reprise::Entree(entree);
    }

    /// Cession de la tâche courante : nouvel état, et son contexte pour la reprise.
    pub fn ceder(&mut self, etat: Etat, compte: Option<u8>, contexte: Contexte) {
        let e = &mut self.emplacements[self.courante];
        e.etat = etat;
        if let Some(compte) = compte {
            e.compte = compte;
        }
        e.reprise = Reprise::Cession(contexte);
    }

    /// Une image de moins pour la tâche `k` si elle attend ; à zéro, elle est prête.
    pub fn decompter(&mut self, k: usize) {
        let e = &mut self.emplacements[k];
        if e.etat == Etat::Attend {
            e.compte = e.compte.wrapping_sub(1);
            if e.compte == 0 {
                e.etat = Etat::Prete;
            }
        }
    }

    /// Réveils du VBLANK : chaque tâche qui attend perd une image ; l'emplacement 0 (le premier examiné) ne
    /// décompte qu'une image sur quatre tant que le ralenti (`A5+0x4f`, décompté ici) n'est pas épuisé.
    pub fn decompter_image(&mut self, ralenti: &mut u8) {
        for k in 0..EMPLACEMENTS {
            if self.emplacements[k].etat != Etat::Attend {
                continue;
            }
            if *ralenti != 0 && k == 0 {
                *ralenti -= 1;
                if *ralenti & 3 != 0 {
                    continue;
                }
            }
            self.decompter(k);
        }
    }
}

/// La table des tâches de la machine (créée au premier accès).
pub fn table(b: &mut Cps2Bus) -> &mut Table {
    let ext = b.ext.get_or_insert_with(|| Box::new(Table::default()));
    ext.as_mut().as_any_mut().downcast_mut().expect("extension du bus : table des tâches")
}

/// La table des tâches, si la machine en a une (machine du jeu ; la machine d'origine garde la sienne en RAM).
pub fn lire(b: &Cps2Bus) -> Option<&Table> {
    b.ext.as_deref()?.as_any().downcast_ref()
}

/// Octets de la RAM du noyau d'origine (décalage depuis FF0000) que le jeu ne tient plus en RAM : pile de
/// l'ordonnanceur, tâche courante et sommet du tas (FF0052-FF005D), enregistrements (FF0064-FF0163), tas des piles
/// (FF0166-FF01A5), piles des tâches (FF01A6-FF0DA5) et du superviseur (jusqu'à FF0E66). Les outils qui comparent la
/// RAM du jeu à celle de l'original les ignorent (avec le reste de FF0E66-FF0FFF, ignoré de tout temps) et comparent
/// la table avec [`ecarts`].
pub fn ram_du_noyau(decalage: u32) -> bool {
    (0x52..0x5e).contains(&decalage) || (0x64..0x164).contains(&decalage) || (0x166..0x1000).contains(&decalage)
}

// ---------------------------------------------------------------------------------------------
// Test différentiel : table Rust contre la table FF0064 d'une machine d'origine

/// Champs d'un enregistrement de l'original.
mod champ {
    pub const ETAT: u32 = 0;
    pub const COMPTE: u32 = 1;
    pub const SR: u32 = 2;
    pub const PC: u32 = 4;
    pub const USP: u32 = 8;
    pub const PILE: u32 = 0xc;
}

/// Reprises des enveloppes de cession de l'original (`0xa40`/`0xb64`, `0xa86`) : elles restaurent les 15 registres
/// sauvés sur la pile de la tâche puis reviennent (`rts`) à l'adresse de reprise.
const REPRISES_ORIGINE: [u32; 2] = [0xa4c, 0xa8c];
/// Tas des piles de l'original : sommet (FF005A) et fond.
const TAS: u32 = 0xff_005a;
const TAS_FOND: u32 = 0xff_01a6;

/// Emplacement de la tâche principale (partie, machine à modes), que la tâche du gel décompte elle-même.
const PRINCIPALE: usize = 0;

/// Les deux états de la tâche principale ne diffèrent que d'une image décomptée par la tâche du gel (avance rapide).
/// Quand le jeu travaille jusqu'au bout de l'image (avance rapide d'une scène chargée), la fin d'image tombe à un
/// instant qui dépend de la durée des routines, natives ou d'origine : peu avant ou peu après un passage du gel, voire
/// au milieu (compte décrémenté, état pas encore « prête »).
fn un_decompte_d_ecart(rust: (Etat, u8), origine: (u8, u8)) -> bool {
    let attend = Etat::Attend.octet();
    let suivant = |(etat, compte): (u8, u8)| match (etat == attend, compte.wrapping_sub(1)) {
        (true, 0) => Some((Etat::Prete.octet(), 0)),
        (true, reste) => Some((attend, reste)),
        _ => None,
    };
    let rust = (rust.0.octet(), rust.1);
    // « en cours » vaut « prête » ; « attend » à zéro est un décompte du gel interrompu entre le compte et l'état
    let normal = |(etat, compte): (u8, u8)| {
        let prete = etat == Etat::EnCours.octet() || etat == attend && compte == 0;
        (if prete { Etat::Prete.octet() } else { etat }, compte)
    };
    let (rust, origine) = (normal(rust), normal(origine));
    rust == origine || suivant(rust) == Some(origine) || suivant(origine) == Some(rust)
}

/// Différences entre la table Rust et la table FF0064 de la RAM `ram` (FF0000-FFFFFF) d'une machine d'origine, à la
/// même image : état (« prête » et « en cours » confondus : la fin d'image peut tomber pendant qu'une tâche tourne
/// dans l'original ; pour la tâche principale, une image d'écart décomptée par le gel est admise, voir
/// [`un_decompte_d_ecart`]), compte, pile, entrée d'une tâche neuve, et tas des piles. Avec `tout`, le contexte des tâches
/// qui ont cédé aussi (diagnostic) : adresse de reprise, pile, SR et registres. Ils peuvent différer sans effet : une
/// native reprend parfois à une copie identique du code de reprise d'origine (`0xf32cc` pour `0xf5902`), ou laisse
/// un registre que personne ne relit.
pub fn ecarts(t: &Table, ram: &[u16], tout: bool) -> Vec<String> {
    let mot = |a: u32| ram[(a as usize & 0xffff) / 2];
    let octet = |a: u32| (mot(a & !1) >> if a & 1 == 0 { 8 } else { 0 }) as u8;
    let long = |a: u32| (mot(a) as u32) << 16 | mot(a + 2) as u32;
    let mut ecarts = Vec::new();
    for (k, e) in t.emplacements.iter().enumerate() {
        let r = enregistrement(k);
        let etat = octet(r + champ::ETAT);
        let normal = |o: u8| if o == Etat::EnCours.octet() { Etat::Prete.octet() } else { o };
        let mut ecart = |quoi: &str, rust: u32, origine: u32| {
            if rust != origine {
                ecarts.push(format!("tâche {k} : {quoi} {rust:x} au lieu de {origine:x}"));
            }
        };
        let compte = octet(r + champ::COMPTE);
        if !(k == PRINCIPALE && un_decompte_d_ecart((e.etat, e.compte), (etat, compte))) {
            ecart("état", normal(e.etat.octet()) as u32, normal(etat) as u32);
            ecart("compte", e.compte as u32, compte as u32);
        }
        if e.etat == Etat::Libre || etat == Etat::Libre.octet() {
            continue;
        }
        ecart("pile", e.pile, long(r + champ::PILE));
        let en_cours = e.etat == Etat::EnCours || etat == Etat::EnCours.octet();
        match e.reprise {
            Reprise::Entree(entree) => ecart("entrée", entree, long(r + champ::PC)),
            // la pile d'une tâche en cours a changé depuis sa cession
            Reprise::Cession(c) if tout && !en_cours => {
                let usp = long(r + champ::USP);
                if !REPRISES_ORIGINE.contains(&long(r + champ::PC)) {
                    ecart("reprise (PC de l'enregistrement)", c.pc, long(r + champ::PC));
                    continue;
                }
                ecart("reprise", c.pc, long(usp + 60));
                ecart("pile de reprise", c.usp, usp + 64);
                ecart("SR", c.sr as u32, mot(r + champ::SR) as u32);
                for i in 0..8 {
                    ecart(&format!("D{i}"), c.d[i], long(usp + 4 * i as u32));
                }
                for i in 0..7 {
                    ecart(&format!("A{i}"), c.a[i], long(usp + 32 + 4 * i as u32));
                }
            }
            Reprise::Cession(_) => {}
        }
    }
    let sommet = if long(TAS) == 0 { TAS_FOND } else { long(TAS) };
    let tas: Vec<u32> = (sommet..TAS_FOND).step_by(4).rev().map(long).collect();
    if tas != t.piles {
        ecarts.push(format!("tas des piles {:x?} au lieu de {:x?}", t.piles, tas));
    }
    ecarts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_et_mort_rendent_les_piles_dans_l_ordre() {
        let mut t = Table { piles: vec![3, 2, 1], ..Table::default() };
        t.creer(0, 0x100);
        t.creer(3, 0x200);
        assert_eq!((t.emplacements[0].pile, t.emplacements[3].pile), (1, 2));
        t.creer(0, 0x300);
        assert_eq!(t.emplacements[0].pile, 1, "la pile rendue est reprise aussitôt");
        t.tuer(3);
        t.tuer(3);
        assert_eq!(t.piles, vec![3, 2]);
    }

    #[test]
    fn le_ralenti_ne_decompte_l_emplacement_0_qu_une_image_sur_quatre() {
        let mut t = Table::default();
        for k in [0, 1] {
            t.emplacements[k] = Emplacement { etat: Etat::Attend, compte: 2, ..Emplacement::default() };
        }
        let mut ralenti = 8;
        for _ in 0..4 {
            t.decompter_image(&mut ralenti);
        }
        assert_eq!(ralenti, 4);
        assert_eq!((t.emplacements[0].etat, t.emplacements[0].compte), (Etat::Attend, 1));
        assert_eq!(t.emplacements[1].etat, Etat::Prete);
    }
}
