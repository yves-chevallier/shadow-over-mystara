//! Accès mémoire « réel ou simulé » pour les routines natives dont la durée dépend des données.
//!
//! La même fonction écrit la logique d'une routine une seule fois : exécutée sur une `Vue` réelle elle
//! modifie la machine ; exécutée sur une `Vue` simulée (écritures dans une table locale, appels de
//! sous-routines ignorés) elle ne fait que parcourir les décisions de l'original, pour calculer sa
//! durée exacte avant d'exécuter quoi que ce soit.

use std::collections::HashMap;

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::chemin::Chemin;
use crate::mem;

pub struct Vue<'a> {
    pub b: &'a mut Cps2Bus,
    /// Écritures simulées (octet par octet) ; `None` : écritures réelles.
    simule: Option<HashMap<u32, u8>>,
    /// Décisions de branchement rencontrées, dans l'ordre.
    pub chemin: Chemin,
}

impl<'a> Vue<'a> {
    pub fn reelle(b: &'a mut Cps2Bus) -> Self {
        Vue { b, simule: None, chemin: Chemin::default() }
    }

    pub fn simulee(b: &'a mut Cps2Bus) -> Self {
        Vue { b, simule: Some(HashMap::new()), chemin: Chemin::default() }
    }

    pub fn reel(&self) -> bool {
        self.simule.is_none()
    }

    pub fn rd8(&mut self, a: u32) -> u8 {
        match self.simule.as_ref().and_then(|m| m.get(&(a & 0xff_ffff))) {
            Some(&v) => v,
            None => mem::rd8(self.b, a),
        }
    }

    pub fn rd16(&mut self, a: u32) -> u16 {
        (self.rd8(a) as u16) << 8 | self.rd8(a.wrapping_add(1)) as u16
    }

    pub fn rd32(&mut self, a: u32) -> u32 {
        (self.rd16(a) as u32) << 16 | self.rd16(a.wrapping_add(2)) as u32
    }

    pub fn wr8(&mut self, a: u32, v: u8) {
        match &mut self.simule {
            Some(m) => {
                m.insert(a & 0xff_ffff, v);
            }
            None => mem::wr8(self.b, a, v),
        }
    }

    pub fn wr16(&mut self, a: u32, v: u16) {
        self.wr8(a, (v >> 8) as u8);
        self.wr8(a.wrapping_add(1), v as u8);
    }

    pub fn wr32(&mut self, a: u32, v: u32) {
        self.wr16(a, (v >> 16) as u16);
        self.wr16(a.wrapping_add(2), v as u16);
    }

    /// Lecture en espace programme (tables rangées dans le code, lues par des accès relatifs au PC).
    pub fn prog8(&mut self, a: u32) -> u8 {
        let mot = m68k::Bus::read_prog16(self.b, a & 0xff_fffe & 0xff_ffff);
        if a & 1 == 0 { (mot >> 8) as u8 } else { mot as u8 }
    }

    pub fn prog16(&mut self, a: u32) -> u16 {
        (self.prog8(a) as u16) << 8 | self.prog8(a.wrapping_add(1)) as u16
    }

    /// Enregistre la décision d'un branchement conditionnel de l'original et la rend.
    pub fn pris(&mut self, pris: bool) -> bool {
        self.chemin.pris(pris);
        pris
    }

    /// Appelle une sous-routine non réécrite (sans effet en mode simulé).
    pub fn appeler(&mut self, c: &mut Cpu, adresse: u32) {
        if self.simule.is_none() {
            crate::call(c, self.b, adresse);
        }
    }
}
