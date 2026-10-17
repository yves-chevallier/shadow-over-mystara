//! Garde des routines natives longues contre l'horizon d'interruption.
//!
//! La machine ne sert une interruption qu'entre deux blocs de code. Une routine native qui appelle
//! d'autres routines (`call`) peut durer des milliers de cycles : si elle franchit la fin d'image
//! (ou la fin de ligne quand les compteurs raster sont armés), l'original aurait été interrompu en
//! son milieu, ce que la native ne peut pas reproduire. Elle ne doit donc s'exécuter que si elle se
//! termine avant l'horizon (`Cps2Bus::irq_horizon`) ; sinon le code d'origine tourne à sa place.
//!
//! Sa durée dépendant des données, on la mesure : à distance de l'horizon, une marge généreuse
//! (ou le double de la plus longue durée observée) suffit ; plus près, la routine est d'abord jouée
//! à blanc sur une copie de l'état.

use std::sync::atomic::{AtomicU64, Ordering};

use cps2::Cps2Bus;
use m68k::Cpu;

/// Marge au-delà de laquelle l'horizon est jugé hors d'atteinte (cycles).
const MARGE: u64 = 60_000;

/// Valeur de `Native::cost` qui interdit la version native (le code d'origine s'exécute).
pub const REFUSEE: u64 = u64::MAX / 4;

/// Plus longue durée observée d'une routine native.
pub struct Garde(AtomicU64);

impl Garde {
    pub const fn new() -> Self {
        Garde(AtomicU64::new(0))
    }

    /// `Native::cost` d'une routine qui compte elle-même ses cycles : 0 si elle se termine avant
    /// l'horizon, `REFUSEE` sinon.
    pub fn cout(&self, c: &Cpu, b: &mut Cps2Bus, run: fn(&mut Cpu, &mut Cps2Bus)) -> u64 {
        let restant = b.irq_horizon.saturating_sub(c.cycles);
        if restant >= MARGE.max(2 * self.0.load(Ordering::Relaxed)) {
            return 0;
        }
        let (mut c2, mut b2) = (c.clone(), b.clone());
        let debut = c2.cycles;
        crate::a_blanc(|| run(&mut c2, &mut b2));
        if c2.cycles - debut <= restant { 0 } else { REFUSEE }
    }

    /// Exécute la routine et retient sa durée.
    pub fn executer(&self, c: &mut Cpu, b: &mut Cps2Bus, run: fn(&mut Cpu, &mut Cps2Bus)) {
        let debut = c.cycles;
        run(c, b);
        self.0.fetch_max(c.cycles - debut, Ordering::Relaxed);
    }
}
