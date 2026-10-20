//! État partagé `0xf590e` des sorties de scène : le décompte de fin de niveau.
//!
//! Un bandeau (« STAGE CLEAR » et le nom de la scène, dessinés hors champ puis glissés depuis la droite),
//! le temps restant, la prime de temps (comptée de 5:00 jusqu'au temps restant, dix secondes par pas), la prime
//! des secondes, puis par joueur la prime totale, deux primes optionnelles (trésor et meilleur score de coups :
//! octet `+0x18a`, mot `+0x190`), enfin le versement des primes au score des joueurs, chiffre par chiffre.
//! Quatorze sous-états (octet `+3` du bloc de la sortie de scène, pas de 2), un par image ; le premier attend la
//! fin d'un fondu en cédant la main à l'ordonnanceur.

use super::tiles::{self, Number, COLUMN, LAYER};
use super::Ctx;
use crate::mem::{prog16, prog32, prog8, rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::ops::abcd;
use crate::{resume, Native};

/// Variables du décompte (RAM de travail, sous A5).
mod v {
    use crate::mem::A5;
    /// Colonne d'arrivée du bandeau (décroît de 48 à 0) et nombre de colonnes déjà visibles - 1.
    pub const BANNER_AT: u32 = A5 - 0x55aa;
    pub const BANNER_WIDTH: u32 = A5 - 0x55a9;
    /// Minuterie (octet) entre deux sous-états, et compteur de pas de la prime de temps.
    pub const TIMER: u32 = A5 - 0x55a8;
    pub const STEPS: u32 = A5 - 0x55a9;
    /// Temps décompté (mot BCD m:ss).
    pub const TIME: u32 = A5 - 0x55a6;
    /// Prime totale (long BCD) et prime en cours (long BCD ; son octet `+2` : prime des secondes).
    pub const BONUS: u32 = A5 - 0x55a4;
    pub const PRIME: u32 = A5 - 0x55a0;
    /// Indice de la scène dans les tables de noms et de primes, et sa variante de bandeau.
    pub const NAME: u32 = A5 - 0x557c;
    pub const NAME_FIRST: u32 = A5 - 0x557b;
    pub const PRIME_INDEX: u32 = A5 - 0x557a;
    /// Primes restant à verser, un long BCD par joueur.
    pub const PAYOUT: u32 = A5 - 0x5574;
    /// Joueur de la prime optionnelle en cours (mot, adresse RAM).
    pub const WINNER: u32 = A5 - 0x5562;
    /// Compteur d'écrans de fin ouverts (incrémenté au début, décrémenté à la fin).
    pub const SCREENS: u32 = A5 - 0x5b8e;
}

/// Variables de la RAM de travail (décalages depuis A5).
mod a5 {
    /// Temps restant (mot BCD m:ss du long de l'horloge).
    pub const CLOCK: u32 = 0x38;
    /// Bit 0 : disposition de l'écran à deux colonnes (positions des panneaux des joueurs).
    pub const LAYOUT: u32 = 0x20;
    pub const HOLD: u32 = 0x33;
    pub const FADE_PAGES: u32 = 0x48e;
    pub const FADE_STEPS: u32 = 0x48f;
    pub const FADE_DELAY: u32 = 0x490;
    pub const FLAG_5F: u32 = 0x5f;
}

/// Champs d'un joueur lus par le décompte.
mod p {
    pub const CHARACTER: u32 = 2;
    pub const STATE: u32 = 4;
    /// Prime du trésor gagnée (octet) ; meilleur nombre de coups (mot).
    pub const TREASURE: u32 = 0x18a;
    pub const HITS: u32 = 0x190;
}

/// Tables de la ROM.
mod rom {
    /// Bandeau : colonnes vides avant le texte (octets, relatifs au PC), selon la variante.
    pub const LEAD_NAME_FIRST: u32 = 0xf6f48;
    pub const LEAD_CLEAR_FIRST: u32 = 0xf6f6a;
    /// Noms des scènes (pointeurs, relatifs au PC) et texte « STAGE CLEAR » des deux variantes.
    pub const NAMES: u32 = 0xf6ed2;
    pub const CLEAR_AFTER: u32 = 0xf7000;
    pub const CLEAR_BEFORE: u32 = 0xf6fec;
    /// Textes « TIME » et « TIME OVER » (descripteurs de 0x1136).
    pub const TIME_TEXT: u32 = 0xf7336;
    pub const NO_TIME_TEXT: u32 = 0xf734e;
    /// Décrément BCD de la prime de temps : dix secondes, ou cinquante pour passer la minute.
    pub const TEN_SECONDS: u32 = 0xf5a62;
    pub const MINUTE: u32 = 0xf5a64;
    /// Prime d'un pas de la prime de temps (longs BCD, 31 valeurs).
    pub const TIME_BONUS: u32 = 0xf6ba0;
    /// Prime d'une seconde (octet BCD, quartet haut).
    pub const SECOND_BONUS: u32 = 0xf5b06;
    /// Primes du trésor et des coups, selon la scène (longs BCD).
    pub const TREASURE_BONUS: u32 = 0x19c0e;
    pub const HITS_BONUS: u32 = 0xf6c1c;
    /// Positions (mots, relatifs à la couche) par personnage et disposition : prime, étiquette, total, versement.
    pub const TOTAL_AT: u32 = 0xf5c08;
    pub const LABEL_AT: u32 = 0xf5c18;
    pub const PANEL_AT: u32 = 0xf5d82;
    pub const PANEL_VALUE_AT: u32 = 0xf5d92;
    pub const PAYOUT_AT: u32 = 0xf5f0a;
    /// Étiquette d'un joueur (paires de tuiles).
    pub const LABEL: u32 = 0xf7320;
}

/// Sous-routines du jeu.
mod sub {
    pub const MUSIC: u32 = 0xe1150;
    pub const TASK_CREATE: u32 = 0x9b6;
    pub const FADE_TASK: u32 = 0x14de;
    pub const PRINT_TALL: u32 = 0x1136;
    pub const ALLOC_SET: u32 = 0xce00a;
    /// Ajoute D0 (joueur << 8 | montant) au score.
    pub const ADD_SCORE: u32 = 0x19ae6;
}

/// Le bandeau est dessiné hors champ (lignes 8 et 9 de la première page de la couche, 0x910000), puis copié.
const BANNER: u32 = 0x910020;

/// Reprises après une cession (adresses qui suivent les `jsr`/`bsr` d'origine).
const FADE_WAIT: u32 = 0xf6906;
const AFTER_FADE: u32 = 0xf596e;

/// Sous-état courant (octet `+3` du bloc désigné par A0).
fn sub_step(x: &mut Ctx) -> u8 {
    rd8(x.b, x.c.a[0] + 3)
}
fn set_sub_step(x: &mut Ctx, s: u8) {
    wr8(x.b, x.c.a[0] + 3, s)
}
fn next_sub_step(x: &mut Ctx) {
    let s = sub_step(x).wrapping_add(2);
    set_sub_step(x, s);
}

/// Minuterie : retire une image, vrai quand elle arrive à zéro.
fn tick(x: &mut Ctx) -> bool {
    let t = rd8(x.b, v::TIMER).wrapping_sub(1);
    wr8(x.b, v::TIMER, t);
    t == 0
}
fn set_timer(x: &mut Ctx, t: u8) {
    wr8(x.b, v::TIMER, t)
}

/// Joueurs en jeu (présents, état principal < 4) : adresses des enregistrements.
fn players_in_game(x: &mut Ctx) -> impl Iterator<Item = u32> {
    let alive: Vec<u32> = (0..4)
        .map(|k| super::PLAYERS + k * super::PLAYER_STRIDE)
        .filter(|&p| rd8(x.b, p) != 0 && (rd8(x.b, p + p::STATE) as i8) < 4)
        .collect();
    alive.into_iter()
}

/// Position dans la couche lue dans une table (mots signés relatifs au PC) indexée par le personnage du joueur et
/// la disposition de l'écran.
fn panel_at(x: &mut Ctx, table: u32, player: u32, pc_relative: bool) -> u32 {
    let mut i = 2 * rd8(x.b, player + p::CHARACTER) as u32;
    if x.v8(a5::LAYOUT) & 1 != 0 {
        i += 8;
    }
    let off = if pc_relative { prog16(x.b, table + i) } else { rd16(x.b, table + i) };
    LAYER.wrapping_add(off as i16 as i32 as u32)
}

/// Un état de 0xf590e.
pub fn run(x: &mut Ctx) -> Flow {
    match sub_step(x) {
        0 => start(x),
        2 => {
            banner_step(x);
            Flow::Done
        }
        4 => clock(x),
        6 => {
            if tick(x) {
                next_sub_step(x);
                wr16(x.b, v::TIME, 0x0500);
                set_timer(x, 1);
                wr8(x.b, v::STEPS, 0);
            }
            Flow::Done
        }
        8 => time_bonus(x),
        10 => second_bonus(x),
        12 => total(x),
        14 => player_totals(x),
        16 => treasure(x),
        18 => announce(x, 996),
        20 => best_hits(x),
        22 => announce(x, 1000),
        24 => payout(x),
        26 => {
            if tick(x) {
                x.set_v8(a5::HOLD, 0);
                let step = rd8(x.b, A5 - 0x55c8).wrapping_add(4);
                wr8(x.b, A5 - 0x55c8, step);
                wr8(x.b, A5 - 0x55c7, 0);
                let n = rd8(x.b, v::SCREENS).wrapping_sub(1);
                wr8(x.b, v::SCREENS, n);
            }
            Flow::Done
        }
        _ => Flow::Done,
    }
}

/// Sous-état 0 : musique, fondu (tâche 0x14de, un niveau par image, pages 13) dont on attend la fin.
fn start(x: &mut Ctx) -> Flow {
    next_sub_step(x);
    x.set_v8(a5::HOLD, 1);
    x.call(sub::MUSIC);
    x.set_v8(a5::FADE_STEPS, 8);
    // 0xf68c8 : A0 sauvegardé autour de la création de la tâche
    let a0 = x.c.a[0];
    x.c.a[0] = sub::FADE_TASK;
    x.c.d[1] = 0;
    x.set_v16(a5::FADE_DELAY, 1);
    x.set_v8(a5::FADE_PAGES, 13);
    x.c.d[0] = x.c.d[0] & !0xffff | 16;
    x.call(sub::TASK_CREATE);
    x.c.a[0] = a0;
    x.push_return(AFTER_FADE);
    x.wait(1, FADE_WAIT)
}

resume!(TALLY_FADE_WAIT, FADE_WAIT, |x| {
    if x.v8(a5::FADE_PAGES) != 0 {
        return x.wait(1, FADE_WAIT);
    }
    Flow::Done
});

// Fin du sous-état 0 : le bandeau est dessiné hors champ, puis son premier pas.
resume!(TALLY_AFTER_FADE, AFTER_FADE, |x| {
    banner(x);
    wr8(x.b, v::BANNER_AT, 48);
    wr8(x.b, v::BANNER_WIDTH, 0xff);
    x.set_v8(a5::FLAG_5F, 0);
    let n = rd8(x.b, v::SCREENS).wrapping_add(1);
    wr8(x.b, v::SCREENS, n);
    banner_step(x);
    Flow::Done
});

/// Le bandeau, deux rangées à partir de la ligne 8 de la colonne 0 (0x0f6e88) : texte « STAGE CLEAR » et nom
/// de la scène, dans l'ordre de la variante, une colonne vide entre les deux ; un mot 0xffff marque la fin.
fn banner(x: &mut Ctx) {
    let at = BANNER;
    let name = rd8(x.b, v::NAME) as u32;
    let name_text = prog32(x.b, rom::NAMES + 4 * name);
    let at = if rd8(x.b, v::NAME_FIRST) != 0 {
        let lead = prog8(x.b, rom::LEAD_NAME_FIRST + name) as u32;
        let at = tiles::blank_columns(x.b, at, lead + 1);
        let at = tiles::glyphs(x.b, at, name_text);
        let at = tiles::blank_columns(x.b, at, 2);
        tiles::glyphs(x.b, at, rom::CLEAR_AFTER)
    } else {
        let lead = prog8(x.b, rom::LEAD_CLEAR_FIRST + name) as u32;
        let at = tiles::blank_columns(x.b, at, lead + 1);
        let at = tiles::glyphs(x.b, at, rom::CLEAR_BEFORE);
        let at = tiles::blank_columns(x.b, at, 2);
        tiles::glyphs(x.b, at, name_text)
    };
    wr16(x.b, at, 0xffff);
}

/// Sous-état 2 : le bandeau avance d'une colonne vers la gauche (lignes 10 et 11), jusqu'à la colonne 0.
fn banner_step(x: &mut Ctx) {
    let col = rd8(x.b, v::BANNER_AT).wrapping_sub(1);
    wr8(x.b, v::BANNER_AT, col);
    let width = rd8(x.b, v::BANNER_WIDTH).wrapping_add(1);
    wr8(x.b, v::BANNER_WIDTH, width);
    // 0x0f71fa : copie des colonnes visibles, une case vide après la dernière
    let mut dst = LAYER + ((col as u32) << 7) + 0x28;
    let mut src = BANNER;
    for _ in 0..=width as u32 {
        let (top, bottom) = (rd32(x.b, src), rd32(x.b, src + 4));
        wr32(x.b, dst, top);
        wr32(x.b, dst + 4, bottom);
        src += COLUMN;
        dst += COLUMN;
        if rd16(x.b, src) == 0xffff {
            tiles::blank(x.b, dst);
            break;
        }
    }
    if col == 0 {
        next_sub_step(x);
        set_timer(x, 30);
    }
}

/// Sous-état 4 : le temps restant, ou « TIME OVER » et la prime du trésor s'il est écoulé.
fn clock(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    if x.v16(a5::CLOCK) == 0 {
        return time_over(x);
    }
    next_sub_step(x);
    set_timer(x, 30);
    x.c.a[4] = rom::TIME_TEXT;
    x.call(sub::PRINT_TALL);
    let t = x.v16(a5::CLOCK);
    tiles::duration(x.b, 0x912db4, t);
    Flow::Done
}

/// Temps écoulé (0x0f6dca) : la prime du trésor en grands chiffres, puis directement les primes des joueurs.
fn time_over(x: &mut Ctx) -> Flow {
    let src = rom::TREASURE_BONUS + 4 * rd8(x.b, v::PRIME_INDEX) as u32 + 1;
    let mut n = Number::new(0x9128b4, 3);
    for i in 0..2 {
        let byte = rd8(x.b, src + i);
        tiles::big_digit(x.b, &mut n, byte >> 4);
        tiles::big_digit(x.b, &mut n, byte);
    }
    let byte = rd8(x.b, src + 2);
    tiles::big_digit(x.b, &mut n, byte >> 4);
    n.lead = true;
    tiles::big_digit(x.b, &mut n, byte);
    x.c.a[4] = rom::NO_TIME_TEXT;
    x.call(sub::PRINT_TALL);
    set_timer(x, 30);
    set_sub_step(x, 16);
    Flow::Done
}

/// Sous-état 8 : le temps décompté descend de dix secondes par pas jusqu'au temps restant (dizaines) ; la prime
/// du pas (table indexée par le nombre de pas, au plus 30) s'affiche à côté.
fn time_bonus(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    set_timer(x, 10);
    let t = rd16(x.b, v::TIME);
    tiles::duration(x.b, 0x91283c, t);
    // `cmpi.b #30 ; ble` : comparaison signée d'un octet utilisé ensuite non signé
    let steps = rd8(x.b, v::STEPS);
    let steps = if (steps as i8) <= 30 { steps } else { 30 } as u32;
    let bonus = rom::TIME_BONUS + 4 * steps;
    let value = rd32(x.b, bonus);
    wr32(x.b, v::BONUS, value);
    tiles::number6(x.b, 0x912d3c, bonus, 0);
    if t == x.v16(a5::CLOCK) & 0xfff0 {
        set_timer(x, 30);
        next_sub_step(x);
        return Flow::Done;
    }
    // soustraction BCD sur deux octets (sbcd), retenue de départ nulle
    let dec = if t & 0xf0 != 0 { rom::TEN_SECONDS } else { rom::MINUTE };
    let (lo, borrow) = crate::game::sbcd(t as u8, rd8(x.b, dec + 1), false);
    let (hi, _) = crate::game::sbcd((t >> 8) as u8, rd8(x.b, dec), borrow);
    wr16(x.b, v::TIME, (hi as u16) << 8 | lo as u16);
    let n = rd8(x.b, v::STEPS).wrapping_add(1);
    wr8(x.b, v::STEPS, n);
    Flow::Done
}

/// Sous-état 10 : les secondes restantes (unités) font la prime des secondes, ajoutée à la prime totale.
fn second_bonus(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    set_timer(x, 30);
    next_sub_step(x);
    let seconds = (x.v16(a5::CLOCK) & 15) as u8;
    tiles::digit(x.b, 0x912a44, seconds, 0);
    tiles::tall(x.b, 0x912a44 + COLUMN, tiles::SECONDS, 0);
    let rate = rd8(x.b, rom::SECOND_BONUS);
    wr32(x.b, v::PRIME, 0);
    if seconds != 0 {
        let (mut prime, mut carry) = (0u8, false);
        for _ in 0..seconds {
            (prime, carry) = abcd(rate >> 4, prime, false);
        }
        let _ = carry;
        wr8(x.b, v::PRIME + 2, prime);
        add_bcd(x, v::BONUS, v::PRIME);
    }
    // le taux (deux chiffres), et la prime des secondes suivie de « 00 »
    let mut n = Number::new(0x912a44 + 2 * COLUMN, 0);
    n.byte(x.b, rate);
    n.digit(x.b, 0);
    let prime = rd8(x.b, v::PRIME + 2);
    let mut n = Number::new(0x912e44, 0);
    n.byte(x.b, prime);
    n.digit(x.b, 0);
    n.last(x.b, 0);
    Flow::Done
}

/// `dst += src` (longs BCD), quatre `abcd` de l'octet bas vers l'octet haut.
fn add_bcd(x: &mut Ctx, dst: u32, src: u32) {
    let mut carry = false;
    for i in (0..4).rev() {
        let (r, c) = abcd(rd8(x.b, src + i), rd8(x.b, dst + i), carry);
        wr8(x.b, dst + i, r);
        carry = c;
    }
}

/// Sous-état 12 : le cadre du total, le temps restant et la prime totale ; chaque joueur la recevra.
fn total(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    next_sub_step(x);
    set_timer(x, 30);
    let at = 0x9127c4;
    for (k, tile) in [0x0460u32, 0x0462, 0x0464].into_iter().enumerate() {
        wr32(x.b, at + 4 * k as u32, tile << 16);
    }
    for (k, tile) in [0x0461u32, 0x0463, 0x0465].into_iter().enumerate() {
        wr32(x.b, at + COLUMN + 4 * k as u32, tile << 16);
    }
    for k in 0..8 {
        wr32(x.b, at + COLUMN + 8 + (k + 1) * COLUMN, (0x0466 + k) << 16);
    }
    let t = x.v16(a5::CLOCK);
    tiles::duration(x.b, 0x912850, t);
    tiles::number6(x.b, 0x912d50, v::BONUS, 0);
    let bonus = rd32(x.b, v::BONUS);
    for k in 0..4 {
        wr32(x.b, v::PAYOUT + 4 * k, bonus);
    }
    Flow::Done
}

/// Sous-état 14 : la prime totale sous l'étiquette de chaque joueur en jeu.
fn player_totals(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    next_sub_step(x);
    set_timer(x, 30);
    for p in players_in_game(x) {
        let total_at = panel_at(x, rom::TOTAL_AT, p, true);
        let label_at = panel_at(x, rom::LABEL_AT, p, true);
        tiles::label(x.b, label_at, rom::LABEL);
        tiles::number6(x.b, total_at, v::BONUS, 2);
    }
    Flow::Done
}

/// Sous-état 16 : le premier joueur qui a trouvé le trésor reçoit sa prime (affichée 60 images), sinon on passe
/// à la prime des coups.
fn treasure(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    let winner = players_in_game(x).find(|&p| rd8(x.b, p + p::TREASURE) != 0);
    let Some(p) = winner else {
        set_sub_step(x, 20);
        set_timer(x, 1);
        return Flow::Done;
    };
    wr16(x.b, v::WINNER, p as u16);
    set_timer(x, 60);
    next_sub_step(x);
    sign(x, 996);
    let src = rom::TREASURE_BONUS + 4 * rd8(x.b, v::PRIME_INDEX) as u32;
    prime_value(x, src);
    Flow::Done
}

/// Sous-état 20 : le joueur au meilleur nombre de coups reçoit la prime des coups ; les compteurs sont remis à 0.
fn best_hits(x: &mut Ctx) -> Flow {
    if !tick(x) {
        return Flow::Done;
    }
    let (mut best, mut winner) = (0i16, 0u32);
    for p in players_in_game(x) {
        if rd8(x.b, p + p::TREASURE) != 0 {
            continue;
        }
        let hits = rd16(x.b, p + p::HITS) as i16;
        if best < hits {
            best = hits;
            winner = p;
        }
        wr16(x.b, p + p::HITS, 0);
    }
    if best == 0 {
        set_timer(x, 1);
        set_sub_step(x, 24);
        return Flow::Done;
    }
    wr16(x.b, v::WINNER, winner as u16);
    next_sub_step(x);
    set_timer(x, 60);
    let src = rom::HITS_BONUS + 4 * rd8(x.b, v::PRIME_INDEX) as u32;
    prime_value(x, src);
    Flow::Done
}

/// La prime optionnelle `src` (long BCD) mémorisée et affichée dans le panneau du gagnant.
fn prime_value(x: &mut Ctx, src: u32) {
    let w = winner(x);
    let at = panel_at(x, rom::PANEL_VALUE_AT, w, false);
    let value = rd32(x.b, src);
    wr32(x.b, v::PRIME, value);
    tiles::number6(x.b, at, src, 2);
}

fn winner(x: &mut Ctx) -> u32 {
    rd16(x.b, v::WINNER) as i16 as i32 as u32 & 0xff_ffff
}

/// Pancarte de quatre colonnes (tuiles `first`.., attributs 30) dans le panneau du gagnant (0x0f5cee, 0x0f5ce6).
fn sign(x: &mut Ctx, first: u16) {
    let w = winner(x);
    let at = panel_at(x, rom::PANEL_AT, w, false);
    for k in 0..4u16 {
        tiles::tall(x.b, at + k as u32 * COLUMN, first + k, 30);
    }
}

/// Sous-états 18 et 22 : la pancarte clignote 30 images, reste 30 images, puis un objet SET (type 19) emporte
/// la prime vers le gagnant.
fn announce(x: &mut Ctx, first: u16) -> Flow {
    if tick(x) {
        next_sub_step(x);
        set_timer(x, 20);
        sign(x, first);
        x.c.d[0] = 19;
        x.call(sub::ALLOC_SET);
        if x.c.sr & m68k::SR_C != 0 {
            return Flow::Done;
        }
        let o = x.c.a[1];
        let prime = rd32(x.b, v::PRIME);
        wr32(x.b, o + 0x4c, prime);
        let w = winner(x);
        let character = rd8(x.b, w + p::CHARACTER);
        wr8(x.b, o + 0x2f, character);
        return Flow::Done;
    }
    let t = rd8(x.b, v::TIMER);
    if (t as i8) <= 30 {
        return Flow::Done;
    }
    if t & 2 != 0 {
        sign(x, first);
    } else {
        // 0x0f5ccc : quatre colonnes vides
        let w = winner(x);
        let at = panel_at(x, rom::PANEL_AT, w, false);
        tiles::blank_columns(x.b, at, 4);
    }
    Flow::Done
}

/// Sous-état 24 : les primes passent au score des joueurs, un chiffre par pas de deux images (centaines, puis
/// milliers, puis dizaines de milliers) ; le reste s'affiche à chaque pas.
fn payout(x: &mut Ctx) -> Flow {
    if x.v16(a5::CLOCK) == 0 {
        next_sub_step(x);
        set_timer(x, 30);
        return Flow::Done;
    }
    if rd8(x.b, v::TIMER) != 0 && !tick(x) {
        return Flow::Done;
    }
    set_timer(x, 2);
    let mut paying = false;
    for k in 0..4u32 {
        let p = super::PLAYERS + k * super::PLAYER_STRIDE;
        let left = v::PAYOUT + 4 * k;
        if rd8(x.b, p) == 0 || (rd8(x.b, p + p::STATE) as i8) >= 4 || rd32(x.b, left) == 0 {
            continue;
        }
        paying = true;
        let (rest, amount) = pay_digit(rd32(x.b, left));
        wr32(x.b, left, rest);
        let character = rd8(x.b, p + p::CHARACTER);
        x.c.d[0] = (character as u32) << 8 | amount;
        x.call(sub::ADD_SCORE);
        let mut i = 2 * character as u32;
        if x.v8(a5::LAYOUT) & 1 != 0 {
            i += 8;
        }
        let at = LAYER.wrapping_add(prog16(x.b, rom::PAYOUT_AT + i) as i16 as i32 as u32);
        tiles::number6(x.b, at, left, 2);
    }
    if !paying {
        next_sub_step(x);
    }
    Flow::Done
}

/// Retire un du chiffre le plus faible non nul parmi les centaines, milliers et dizaines de milliers (0x0f5f1a) ;
/// rend le reste et le montant versé (code de score : 4, 14 ou 33).
fn pay_digit(v: u32) -> (u32, u32) {
    if v & 0x0f00 != 0 {
        (v - 0x100, 4)
    } else if v & 0xf000 != 0 {
        (v - 0x1000, 14)
    } else {
        (v.wrapping_sub(0x1_0000), 33)
    }
}

/// Entrée d'origine (séquences de sortie restées en 68000).
pub const TALLY: Native = Native {
    entry: 0xf590e,
    name: "scene_tally",
    cost: |_, _| 100,
    run: |c, b| {
        let mut x = Ctx::new(c, b);
        if run(&mut x) == Flow::Done {
            crate::mem::rts(x.c, x.b);
        }
    },
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};

pub const NATIVES: &[Native] = &[TALLY, TALLY_FADE_WAIT, TALLY_AFTER_FADE];
