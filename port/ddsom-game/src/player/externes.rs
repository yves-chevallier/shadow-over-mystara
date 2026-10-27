//! Routines d'autres sous-systèmes que les joueurs appellent encore par leur adresse d'origine (`Ctx::rom`) et que
//! plusieurs fichiers partagent : une seule déclaration par adresse. Elles deviendront des appels Rust quand leur
//! module aura une API (son : `sound`, palettes : `palette`, effets : `families`) ; les noms marqués (h) sont des
//! hypothèses d'après l'usage.

/// Sons (« wrappers » de `sound`, A0 = le joueur pour le panoramique).
pub mod son {
    /// Élan d'un coup chargé, en deux temps.
    pub const CHARGE_A: u32 = 0xe792e;
    pub const CHARGE_B: u32 = 0xe1fc0;
    /// Coup porté : armes lourdes (combattant, nain ; seconde frappe de l'elfe et du magicien) et légères (clerc,
    /// elfe, magicien) (h).
    pub const STRIKE_HEAVY: u32 = 0xe1de0;
    pub const STRIKE_LIGHT: u32 = 0xe1e80;
    /// Saut d'attaque (départ, fin) (h).
    pub const LEAP: u32 = 0xe1f20;
    pub const LEAP_END: u32 = 0xe7668;
    /// Attaque tournoyante.
    pub const SPIN: u32 = 0xe2060;
    /// Résurrection, victoire, arme maudite qui tombe (h).
    pub const FANFARE: u32 = 0xe24d8;
    /// Voix du saut, son de l'atterrissage, de la glissade (aiguillages par personnage).
    pub const JUMP_VOICE: u32 = 0xe7622;
    pub const LANDING: u32 = 0xe774c;
    pub const SLIDE: u32 = 0xe78e8;
    /// Prière de soin du clerc, séisme de l'elfe (h).
    pub const PRAYER_QUAKE: u32 = 0xe8db4;
}

/// Palettes de l'écran (`palette`).
pub mod palette {
    /// Pleine luminosité (fondu, 0x1686).
    pub const FULL_BRIGHTNESS: u32 = 0x1686;
    /// Éclairs : vague (A, 0x1aaa0), gris (0x1ab56), retour des palettes (0x1abe0).
    pub const FLASH_A: u32 = 0x1aaa0;
    pub const FLASH_GRAY: u32 = 0x1ab56;
    pub const FLASH_RESTORE: u32 = 0x1abe0;
}

/// Effets créés par d'autres sous-systèmes (`families`, `effects_aids`).
pub mod effet {
    /// Effet de choc (poussière, étincelle) : D0 genre, D1 réglages, D2 et D3 vitesses ; A1 et retenue en retour.
    pub const DUST: u32 = 0xb10c8;
    /// Barre de vie d'un monstre frappé (D0 = joueur et type, D1 = PV max, D2 = PV, D3 = classe).
    pub const HEALTH_BAR: u32 = 0x19dce;
    /// Halo d'entrée en scène.
    pub const HALO: u32 = 0xab708;
    /// Étoiles et étincelles d'un coup (D0, D1 : genre).
    pub const STARS: u32 = 0xb378a;
    /// Objet SET de la baguette.
    pub const WAND: u32 = 0xcc352;
    /// Effets joueur des magies de l'elfe et du magicien : étincelles, éclair, séisme.
    pub const SPARKS: u32 = 0x4dc9a;
    pub const BOLT: u32 = 0x4b7a0;
    pub const QUAKE: u32 = 0x4b340;
    pub const QUAKE_B: u32 = 0x4f132;
}
