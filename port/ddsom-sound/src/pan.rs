//! Évolution du panoramique d'une voie (voies de séquenceur et voies ADPCM).
//!
//! Le pan va de 0 (gauche) à 0x20 (droite), en virgule fixe 8.8 pendant son évolution.

use crate::math::clamp_pan;

/// Comment le pan évolue.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum FadeMode {
    /// Aller-retour : à chaque cible atteinte la cible suivante s'ajoute de `delta`.
    PingPong,
    /// Rampe vers la cible, puis pan fixe.
    Ramp,
    /// Rampe vers la cible, puis rampe vers la butée à la deuxième vitesse.
    TwoStage,
    /// Pan fixe.
    #[default]
    Fixed,
}

impl FadeMode {
    /// Valeur du programme d'origine (0xFF = fixe).
    pub fn code(self) -> u8 {
        match self {
            FadeMode::PingPong => 0,
            FadeMode::Ramp => 1,
            FadeMode::TwoStage => 2,
            FadeMode::Fixed => 0xff,
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct PanFade {
    pub mode: FadeMode,
    /// Position et fraction.
    pub pan: u8,
    pub frac: u8,
    pub delta: u8,
    pub target: u8,
    pub speed: u16,
    pub speed2: u16,
}

impl PanFade {
    fn position(&self) -> u16 {
        u16::from(self.pan) << 8 | u16::from(self.frac)
    }

    /// Réglage demandé par une commande du 68000. Octet 6 : type, 7 : pan de départ (0-0x20),
    /// 8 : amplitude, 9-10 : vitesse, 11-12 : seconde vitesse. `None` si le type est inconnu.
    /// Le type 7 est une rampe vers la droite qui demande aussi une copie vers la gauche sur le
    /// canal voisin : voir [`PanFade::mirrored`].
    pub fn from_command(cmd: &[u8; 16]) -> Option<PanFade> {
        let start = clamp_pan(cmd[7]);
        let amount = clamp_pan(cmd[8]);
        let speed = u16::from(cmd[9]) << 8 | u16::from(cmd[10]);
        let speed2 = u16::from(cmd[11]) << 8 | u16::from(cmd[12]);
        let neg = |x: u16| 0u16.wrapping_sub(x);
        // Cible : `start + amount` ou `start - amount`, bornée à 0-0x20.
        let up = start.saturating_add(amount).min(0x20);
        let down = start.saturating_sub(amount);
        let mut f = PanFade { pan: start, ..PanFade::default() };
        match cmd[6] {
            0 => {}
            // Va-et-vient entre le pan de départ et l'extrémité opposée.
            1 => {
                f.mode = FadeMode::PingPong;
                if start < 0x11 {
                    (f.delta, f.target, f.speed) = (0xe0, 0x20, speed);
                } else {
                    (f.delta, f.target, f.speed) = (0x20, 0, neg(speed));
                }
            }
            // Va-et-vient d'amplitude `amount`.
            2 => {
                f.mode = FadeMode::PingPong;
                if start < 0x11 {
                    (f.delta, f.target, f.speed) = (0u8.wrapping_sub(amount), up, speed);
                } else {
                    (f.delta, f.target, f.speed) = (amount, down, neg(speed));
                }
            }
            // Rampe vers la droite (3, 7) ou la gauche (4).
            3 | 7 => (f.mode, f.target, f.speed) = (FadeMode::Ramp, up, speed),
            4 => (f.mode, f.target, f.speed) = (FadeMode::Ramp, down, neg(speed)),
            // Rampe en deux temps vers la droite (5) ou la gauche (6).
            5 => (f.mode, f.target, f.speed, f.speed2) = (FadeMode::TwoStage, up, speed, speed2),
            6 => (f.mode, f.target, f.speed, f.speed2) = (FadeMode::TwoStage, down, neg(speed), neg(speed2)),
            _ => return None,
        }
        Some(f)
    }

    /// Variante du type 7 pour le canal voisin : rampe vers la gauche.
    pub fn mirrored(&self, cmd: &[u8; 16]) -> PanFade {
        let amount = clamp_pan(cmd[8]);
        let speed = u16::from(cmd[9]) << 8 | u16::from(cmd[10]);
        PanFade { target: self.pan.saturating_sub(amount), speed: 0u16.wrapping_sub(speed), ..*self }
    }

    /// Un pas d'évolution ; renvoie la nouvelle position (pan, fraction) et `false` si le pan est
    /// fixe (rien à écrire).
    pub fn step(&mut self) -> bool {
        if self.mode == FadeMode::Fixed {
            return false;
        }
        // Un pas de rampe : nouvelle position, et la cible n'est-elle pas encore atteinte ?
        let (pos, carry) = self.speed.overflowing_add(self.position());
        let below = self.target < (pos >> 8) as u8;
        let moving = 0xff_u8.wrapping_add(carry as u8).wrapping_add(below as u8) != 0;
        let reached = u16::from(self.target) << 8 | (pos & 0xff);
        let pos = match self.mode {
            _ if moving => pos,
            FadeMode::PingPong => {
                // Cible atteinte : la cible repart de l'autre côté, la vitesse s'inverse.
                self.target = self.target.wrapping_add(self.delta);
                self.delta = 0u8.wrapping_sub(self.delta);
                self.speed = 0u16.wrapping_sub(self.speed);
                reached
            }
            FadeMode::TwoStage => {
                // Butée : 0x20 si on monte, 0 si on descend.
                let limit = if self.speed & 0x8000 != 0 { 0 } else { 0x20 };
                if limit == self.target {
                    self.mode = FadeMode::Fixed;
                } else {
                    self.target = limit;
                    self.speed = self.speed2;
                }
                reached
            }
            _ => {
                self.mode = FadeMode::Fixed;
                reached
            }
        };
        self.pan = (pos >> 8) as u8;
        self.frac = pos as u8;
        true
    }
}
