//! Collisions et résolution des coups (sous-systèmes X1 et X2 de l'atlas).
//!
//! Chaque image, `combat_update` (0x260ee) confronte les boîtes des objets visibles : les listes par
//! catégorie remplies par `sprites::check_visible` (FFFE72.. et tableaux FFFE86..) sont parcourues
//! par paires (attaquant, victime) ; une paire dont les boîtes se recouvrent déclenche l'application
//! du coup (états, points de vie, étincelles, sons). Voir `docs/combat.md` pour les formats.
//!
//! La logique suit l'original instruction par instruction (mêmes lectures/écritures de la RAM,
//! mêmes arithmétiques 8/16 bits) mais s'exprime sur des vues typées des objets. Les sous-routines
//! hors du périmètre (allocation d'effets, sons, affichage des jauges, RNG...) sont appelées telles
//! quelles par `crate::call`.

use cps2::Cps2Bus;
use m68k::decode::Fetch;
use m68k::{Cpu, SR_Z};

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::{call, Native};

/// Compteurs d'appel des routines réécrites (couverture de la validation).
mod cov {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    static ON: AtomicBool = AtomicBool::new(false);
    static HITS: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());

    pub fn hit(name: &'static str) {
        if ON.load(Ordering::Relaxed) {
            *HITS.lock().unwrap().entry(name).or_default() += 1;
        }
    }
    pub fn enable() {
        ON.store(true, Ordering::Relaxed);
    }
    pub fn report() -> Vec<(&'static str, u64)> {
        HITS.lock().unwrap().iter().map(|(k, v)| (*k, *v)).collect()
    }
}

/// Active le comptage des appels des routines internes (outil `combat_coverage`).
pub use cov::enable as enable_coverage;
/// Nombre d'appels de chaque routine interne depuis l'activation.
pub use cov::report as coverage;

/// Trace de mise au point (variable DDSOM_TRACE).
macro_rules! trace {
    ($($a:tt)*) => { if std::env::var("DDSOM_TRACE").is_ok() { eprintln!($($a)*); } };
}

mod contact;
mod damage;
mod shots;

use contact::*;
use damage::*;
use shots::*;

/// Variables de A5 (RAM FF8000) lues par les collisions.
mod ram {
    /// Octet : nombre de joueurs en vie (0 : plus de collisions).
    pub const PLAYERS_ALIVE: u32 = 0x35;
    /// Octet : pause (les collisions entre joueurs et ennemis sont suspendues).
    pub const PAUSE: u32 = 0x3d;
    /// Octet : jeu gelé (cinématique) ; avec `SCRIPTED_A` et `SCRIPTED_B` : scénario en cours.
    pub const FROZEN: u32 = 0x42;
    pub const SCRIPTED_A: u32 = 0x44;
    pub const SCRIPTED_B: u32 = 0x4e;
    /// Octet : difficulté (> 5 : attaques ennemies renforcées).
    pub const DIFFICULTY: u32 = 0x48;
    /// Octet : valeur copiée depuis l'ennemi tué (`DEATH_VALUE`) quand il le prévoit.
    pub const KILL_FLAG: u32 = 0x4f;
    /// Mot : réglage de dégâts du jeu (0, 2, 4, 6 : table de barème d'attaque ennemie).
    pub const DAMAGE_SETTING: u32 = 0x70;
    /// Compteur 5 bits (FFFF92) qui sert d'index aux tables de hasard.
    pub const TICK: u32 = 0x7f92;
    /// Premier joueur ; les suivants sont espacés de 0x200.
    pub const PLAYERS: u32 = 0x5de;
    /// Nombre d'objets visibles par catégorie (mots) : tirs joueur, tirs ennemis, ennemis/boss,
    /// caisses, objets ramassables.
    pub const COUNTS: u32 = 0x7e72;
    /// Tableaux de pointeurs (mots) correspondants.
    pub const LISTS: [u32; 5] = [0x7e86, 0x7eaa, 0x7ef6, 0x7f28, 0x7f52];
}

/// Compteurs de boucle que l'original tient en RAM (FF1A62..) : leur valeur finale est visible.
mod scratch {
    pub const PLAYER_HITS_ENEMY: u32 = 0xff1a62;
    pub const SHOTS_OUTER: u32 = 0xff1a64;
    pub const SHOTS_INNER: u32 = 0xff1a66;
    pub const ENEMY_ATTACKS: u32 = 0xff1a68;
    pub const PUSH: u32 = 0xff1a6a;
    pub const CLASH: u32 = 0xff1a6c;
    pub const BOX_ATTACKS: u32 = 0xff1a6e;
    pub const ITEMS: u32 = 0xff1a70;
}

/// Champs d'un objet (joueur, ennemi, tir, caisse, objet). Les noms marqués (h) sont des
/// hypothèses tirées de l'usage.
mod f {
    /// Octet : 0 libre ; bit 0 actif ; 2 = en agonie / consommé (ne participe plus).
    pub const ACTIVE: u32 = 0x00;
    pub const FLAGS: u32 = 0x01;
    /// Type d'objet (tir : n° de comportement au contact ; joueur : index du joueur).
    pub const KIND: u32 = 0x02;
    /// Variante ; pour un joueur, `VARIANT >> 1` = classe (0 combattant .. 5 voleur).
    pub const VARIANT: u32 = 0x03;
    /// Long : état principal, sous-état, action et octet 7 (voir `state`).
    pub const STATE: u32 = 0x04;
    pub const SUBSTATE: u32 = 0x05;
    pub const ACTION: u32 = 0x06;
    pub const X: u32 = 0x08;
    /// Hauteur au-dessus du sol.
    pub const Z: u32 = 0x0c;
    /// Profondeur (clé de tri des sprites).
    pub const Y: u32 = 0x10;
    pub const VX: u32 = 0x14;
    /// Classe de pool : 2 ennemi, 4 boss, 6 tir joueur, 8 tir ennemi...
    pub const CLASS: u32 = 0x28;
    /// Non nul : l'objet regarde à gauche (les décalages X des boîtes sont inversés).
    pub const FLIP: u32 = 0x2c;
    pub const FACING_AUX: u32 = 0x2d;
    /// Jeu de boîtes (index dans les tables de pointeurs de boîtes).
    pub const BOX_SET: u32 = 0x2e;
    pub const BODY_BOX: u32 = 0x38;
    pub const PUSH_BOX: u32 = 0x39;
    pub const ATTACK_BOX: u32 = 0x3a;
    pub const ATTACK_KIND: u32 = 0x3c;
    pub const ATTACK_ID: u32 = 0x3e;
    /// Non nul : l'objet est inerte pour les collisions.
    pub const BOXES_OFF: u32 = 0x40;
    pub const ATTACK_ID_ENEMY: u32 = 0x44;
    /// Puissance de l'attaque : A (contre ennemis ordinaires), B (contre boss) (h).
    pub const POWER_A: u32 = 0x45;
    pub const POWER_B: u32 = 0x46;
    pub const HIT_BY_KIND: u32 = 0x48;
    pub const REACTION: u32 = 0x49;
    pub const REACTION_FLIP: u32 = 0x4a;
    pub const HIT_BY_VARIANT: u32 = 0x4b;
    pub const OWNER: u32 = 0x50;
    pub const HITSTOP: u32 = 0x53;
    pub const LINK: u32 = 0x54;
    pub const HITSTOP_VICTIM: u32 = 0x56;
    pub const HIT_FLAGS: u32 = 0x57;
    /// Octets "dernier coup reçu" par joueur (ennemi), bits d'effets (joueur).
    pub const HIT_MARK: u32 = 0x58;
    pub const COMBO_MARK: u32 = 0x5b;
    pub const HP_MAX: u32 = 0x60;
    pub const HP: u32 = 0x62;
    pub const HP_PREV: u32 = 0x64;
    pub const SUBACTION: u32 = 0x6a;
    pub const KEYS_HELD: u32 = 0x6c;
    pub const KEYS_NEW: u32 = 0x6e;
    pub const RECOIL: u32 = 0x70;
    pub const PREV_ACTION: u32 = 0x6f;
    pub const INVULN: u32 = 0x7e;
    pub const FLIES: u32 = 0x86;
    pub const STAGGER_DELAY: u32 = 0x87;
    pub const PREV_STATE: u32 = 0x8a;
    pub const DEATH_CREDIT: u32 = 0x93;
    pub const IMMUNITY: u32 = 0x94;
    pub const DEATH_VALUE: u32 = 0x9a;
    pub const HITS_TAKEN: u32 = 0x9c;
    pub const WEAPON_10D: u32 = 0x10d;
    pub const WEAPON_10E: u32 = 0x10e;
    pub const ARMOR: u32 = 0x160;
    pub const GEAR_163: u32 = 0x163;
    pub const GEAR_165: u32 = 0x165;
    pub const GEAR_166: u32 = 0x166;
    pub const GEAR_167: u32 = 0x167;
    pub const GEAR_184: u32 = 0x184;
    pub const KILLS_FLAG: u32 = 0x18a;
    pub const HITS_LANDED: u32 = 0x190;
    pub const DYING: u32 = 0x196;
    pub const DAMAGE_CAP: u32 = 0x16e;
}

/// Tables de la ROM programme.
mod rom {
    /// Boîtes d'attaque des tirs joueur (entrées de 8 octets).
    pub const SHOT_ATTACK: u32 = 0x121fc8;
    /// Boîtes d'attaque des tirs ennemis.
    pub const ENEMY_SHOT_ATTACK: u32 = 0x1221e2;
    /// Boîtes de ramassage des objets.
    pub const ITEM_PICKUP: u32 = 0x1233c8;
    /// Tables de pointeurs de boîtes par jeu (index `BOX_SET`) : corps, poussée, attaque.
    pub const BODY: u32 = 0x11bffa;
    pub const PUSH: u32 = 0x11c11a;
    pub const ATTACK: u32 = 0x11c23a;
    /// Table des comportements au contact des tirs joueur (adresses de routine).
    pub const SHOT_HANDLERS: u32 = 0x267c4;
    /// Réaction des joueurs aux coups et aux tirs ennemis : par identifiant d'attaque, 4 octets
    /// (action au sol, action en l'air, puissance, puissance en difficulté > 5).
    pub const ENEMY_ATTACK_REACTIONS: u32 = 0x1067a4;
    pub const ENEMY_SHOT_REACTIONS: u32 = 0x106912;
    /// Garde par type de tir ennemi (octets) : 1 parable par certains équipements, 2 magie.
    pub const SHOT_GUARD: u32 = 0x27690;
}

// ---------------------------------------------------------------------------------------------
// Accès à la mémoire

/// Objet du jeu, désigné par son adresse.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Obj(pub u32);

impl Obj {
    /// Objet désigné par un mot de liste (adresse 16 bits étendue en signe : RAM FF8000-FFFFFF).
    fn from_word(w: u16) -> Obj {
        Obj(w as i16 as i32 as u32 & 0xff_ffff)
    }
    /// Le mot bas de l'adresse, tel que les objets se référencent entre eux.
    fn word(self) -> u16 {
        self.0 as u16
    }
}

/// Mémoire et registres, pour les routines qui doivent appeler du code d'origine.
pub struct World<'a> {
    pub c: &'a mut Cpu,
    pub b: &'a mut Cps2Bus,
    /// Valeur de A2 dans l'original : boîte du coup en cours, que certaines sous-routines
    /// (0x19dce, 0x388a) écrasent sans que l'original le rétablisse dans `player_hits_enemies`.
    pub a2: u32,
}

impl World<'_> {
    fn u8(&mut self, o: Obj, off: u32) -> u8 {
        rd8(self.b, o.0 + off)
    }
    fn u16(&mut self, o: Obj, off: u32) -> u16 {
        rd16(self.b, o.0 + off)
    }
    fn u32(&mut self, o: Obj, off: u32) -> u32 {
        rd32(self.b, o.0 + off)
    }
    fn set8(&mut self, o: Obj, off: u32, v: u8) {
        wr8(self.b, o.0 + off, v)
    }
    fn set16(&mut self, o: Obj, off: u32, v: u16) {
        wr16(self.b, o.0 + off, v)
    }
    fn set32(&mut self, o: Obj, off: u32, v: u32) {
        wr32(self.b, o.0 + off, v)
    }
    /// bset sur un octet.
    fn set_bit(&mut self, o: Obj, off: u32, bit: u32) {
        let v = self.u8(o, off) | 1 << bit;
        self.set8(o, off, v)
    }
    fn add8(&mut self, o: Obj, off: u32, d: u8) {
        let v = self.u8(o, off).wrapping_add(d);
        self.set8(o, off, v)
    }
    fn add16(&mut self, o: Obj, off: u32, d: u16) {
        let v = self.u16(o, off).wrapping_add(d);
        self.set16(o, off, v)
    }
    /// Bit 0 de l'octet d'activité : l'objet participe aux collisions.
    fn is_active(&mut self, o: Obj) -> bool {
        self.u8(o, f::ACTIVE) & 1 != 0
    }
    fn a5_8(&mut self, off: u32) -> u8 {
        rd8(self.b, A5 + off)
    }
    fn a5_16(&mut self, off: u32) -> u16 {
        rd16(self.b, A5 + off)
    }
    /// Lecture en espace programme (adressage relatif au PC de l'original : octets déchiffrés).
    fn pc8(&mut self, a: u32) -> u8 {
        let w = self.b.fetch16(a & !1);
        if a & 1 == 0 {
            (w >> 8) as u8
        } else {
            w as u8
        }
    }
    fn pc16(&mut self, a: u32) -> u16 {
        self.b.fetch16(a)
    }
    /// Appelle une routine d'origine avec A0 et A1 (acteur, cible) ; renvoie le drapeau Z.
    fn call_ao(&mut self, addr: u32, a0: Obj, a1: Obj) -> bool {
        self.c.a[0] = a0.0;
        self.c.a[1] = a1.0;
        self.c.a[5] = A5;
        self.c.a[2] = self.a2;
        call(self.c, self.b, addr);
        self.a2 = self.c.a[2];
        self.c.sr & SR_Z != 0
    }
    /// Idem avec un paramètre D1 (type d'étincelle / d'effet).
    fn call_d1(&mut self, addr: u32, a0: Obj, a1: Obj, d1: u32) -> bool {
        self.c.d[1] = d1;
        self.call_ao(addr, a0, a1)
    }
    /// 0x19a88 : générateur pseudo-aléatoire du jeu.
    fn random(&mut self) -> u16 {
        self.c.a[5] = A5;
        call(self.c, self.b, 0x19a88);
        self.c.d[0] as u16
    }
}

fn set_low16(r: &mut u32, v: u16) {
    *r = (*r & !0xffff) | v as u32;
}

fn set_low8(r: &mut u32, v: u8) {
    *r = (*r & !0xff) | v as u32;
}

/// Valeurs de l'état long (`+4` : principal, sous-état, `+6` : action, `+7`).
mod state {
    /// Victime : "touchée", réaction à jouer (`REACTION` donne l'animation).
    pub const HIT: u32 = 0x0400_0000;
    /// Victime qui a paré le coup.
    pub const BLOCKED: u32 = 0x0402_0000;
    /// Joueur : en train d'agir (poussée, ramassage...), action dans l'octet 6.
    pub const ATTACKING: u32 = 0x0200_0c00;
    /// Joueur touché.
    pub const PLAYER_HURT: u32 = 0x0204_0000;
    /// Joueur : parade.
    pub const PARRY: u32 = 0x0208_0000;
    /// Joueur : soulève une caisse.
    pub const GRABBED: u32 = 0x0200_1400;
    pub const GRABBING: u32 = 0x0200_2e00;
    pub const PICKING_UP: u32 = 0x0200_2200;
    pub const CRATE_PUSHED: u32 = 0x0200_2800;
}

// ---------------------------------------------------------------------------------------------
// Boîtes de collision

/// Boîte en ROM : 6 octets utiles (décalage et demi-taille par axe), entrées de 8 octets.
///
/// | octet | contenu |
/// |---|---|
/// | 0 / 1 | décalage X (signé, inversé si l'objet regarde à gauche) / demi-largeur |
/// | 2 / 3 | décalage Z (hauteur) / demi-hauteur |
/// | 4 / 5 | décalage Y (profondeur) / demi-profondeur |
#[derive(Clone, Copy)]
struct HitBox(u32);

impl HitBox {
    const X: u32 = 0;
    const HALF_X: u32 = 1;
    const Z: u32 = 2;
    const HALF_Z: u32 = 3;
    const Y: u32 = 4;
    const HALF_Y: u32 = 5;
}

/// Boîte `index` du jeu de boîtes de l'objet, dans la table de pointeurs `table`.
fn hitbox(w: &mut World, table: u32, o: Obj, index: u8) -> HitBox {
    let set = w.u8(o, f::BOX_SET) as u32;
    let base = rd32(w.b, table + 4 * set);
    HitBox(base + 8 * index as u32)
}

/// Boîte de l'objet pour l'index donné par son champ `field` (corps, poussée ou attaque).
fn object_box(w: &mut World, table: u32, o: Obj, field: u32) -> HitBox {
    let idx = w.u8(o, field);
    hitbox(w, table, o, idx)
}

/// Résultat de la comparaison d'un axe : `delta` = écart entre les centres (mot, en arithmétique
/// 16 bits comme l'original), `half` = somme des demi-tailles.
#[derive(Clone, Copy)]
struct Probe {
    delta: u16,
    half: u16,
}

impl Probe {
    /// Recouvrement : |delta| <= half, calculé comme l'original (`delta + half <= 2 * half`, non signé).
    fn overlaps(self) -> bool {
        self.delta.wrapping_add(self.half) <= self.half.wrapping_add(self.half)
    }
    /// Profondeur de pénétration (`half - |delta|`) et signe de l'écart (vrai : delta < 0).
    fn penetration(self) -> (u16, bool) {
        let neg = (self.delta as i16) < 0;
        let abs = if neg { self.delta.wrapping_neg() } else { self.delta };
        (self.half.wrapping_sub(abs), neg)
    }
}

/// Somme des demi-tailles : octets additionnés sur 8 bits ; `signed` étend ensuite le signe
/// (collisions avec les caisses), sinon l'extension est nulle.
fn half_sum(a: u8, b: u8, signed: bool) -> u16 {
    let s = a.wrapping_add(b);
    if signed {
        s as i8 as i16 as u16
    } else {
        s as u16
    }
}

/// Deux objets et leurs boîtes.
struct Pair {
    a: Obj,
    abox: HitBox,
    b: Obj,
    bbox: HitBox,
    signed: bool,
}

/// Variante de 0x27410 (attaques ennemies) : seule la demi-profondeur de l'attaque compte.
fn overlap3_attack(w: &mut World, victim: Obj, vbox: HitBox, attacker: Obj, abox: HitBox) -> bool {
    cov::hit("overlap3_attack");
    let pair = Pair { a: victim, abox: vbox, b: attacker, bbox: abox, signed: false };
    let (ay, ax, az) = (pair.probe_y(w), pair.probe_x(w), pair.probe_z(w));
    let half = rd8(w.b, abox.0 + HitBox::HALF_Y) as u16;
    let y = Probe { delta: ay.delta, half };
    y.overlaps() && ax.overlaps() && az.overlaps()
}

impl Pair {
    /// Écart des centres sur un axe : (décalage de boîte + position) de `a` moins celui de `b`.
    /// `flip` : les décalages suivent l'orientation de l'objet (axe X).
    fn probe(&self, w: &mut World, off: u32, half: u32, pos: u32, flip: bool) -> Probe {
        let mut ao = rd8(w.b, self.abox.0 + off) as i8 as i16 as u16;
        let mut bo = rd8(w.b, self.bbox.0 + off) as i8 as i16 as u16;
        if flip {
            if w.u8(self.a, f::FLIP) != 0 {
                ao = ao.wrapping_neg();
            }
            if w.u8(self.b, f::FLIP) != 0 {
                bo = bo.wrapping_neg();
            }
        }
        let a = ao.wrapping_add(w.u16(self.a, pos));
        let b = bo.wrapping_add(w.u16(self.b, pos));
        let half = half_sum(rd8(w.b, self.abox.0 + half), rd8(w.b, self.bbox.0 + half), self.signed);
        Probe { delta: a.wrapping_sub(b), half }
    }
    fn probe_y(&self, w: &mut World) -> Probe {
        self.probe(w, HitBox::Y, HitBox::HALF_Y, f::Y, false)
    }
    fn probe_x(&self, w: &mut World) -> Probe {
        self.probe(w, HitBox::X, HitBox::HALF_X, f::X, true)
    }
    fn probe_z(&self, w: &mut World) -> Probe {
        self.probe(w, HitBox::Z, HitBox::HALF_Z, f::Z, false)
    }
    /// Recouvrement sur les trois axes (Y, X puis Z).
    fn overlaps(&self, w: &mut World) -> bool {
        self.probe_y(w).overlaps() && self.probe_x(w).overlaps() && self.probe_z(w).overlaps()
    }
}

fn overlap3(w: &mut World, a: Obj, abox: HitBox, b: Obj, bbox: HitBox) -> bool {
    Pair { a, abox, b, bbox, signed: false }.overlaps(w)
}

// ---------------------------------------------------------------------------------------------
// Listes d'objets visibles

/// Catégories de la liste des objets visibles (voir `sprites::check_visible`).
#[derive(Clone, Copy)]
enum List {
    PlayerShots = 0,
    EnemyShots = 1,
    Enemies = 2,
    Boxes = 3,
    Items = 4,
}

impl List {
    fn len(self, w: &mut World) -> u16 {
        w.a5_16(ram::COUNTS + 2 * self as u32)
    }
}

/// Fin d'un tour de boucle : continuer, ou quitter la boucle (compteur laissé tel quel).
#[derive(PartialEq)]
enum Flow {
    Next,
    Stop,
}

/// Parcourt une liste comme l'original : compteur de boucle tenu dans la RAM (`counter`), pointeur
/// avancé avant le corps.
fn scan(w: &mut World, list: List, counter: u32, mut body: impl FnMut(&mut World, Obj) -> Flow) {
    let n = list.len(w);
    wr16(w.b, counter, n);
    let mut p = A5 + ram::LISTS[list as usize];
    loop {
        let o = Obj::from_word(rd16(w.b, p));
        p += 2;
        if body(w, o) == Flow::Stop {
            return;
        }
        let k = rd16(w.b, counter).wrapping_sub(1);
        wr16(w.b, counter, k);
        if k == 0 {
            return;
        }
    }
}

/// Les 4 enregistrements joueur.
fn players() -> impl Iterator<Item = Obj> {
    (0..4).map(|k| Obj(A5 + ram::PLAYERS + 0x200 * k))
}

// ---------------------------------------------------------------------------------------------
// Racine

/// 0x260ee : toutes les collisions de l'image.
pub fn combat_update(w: &mut World) {
    cov::hit("combat_update");
    if w.a5_8(ram::PLAYERS_ALIVE) == 0 {
        return;
    }
    let scripted = w.a5_8(ram::SCRIPTED_A) != 0 && w.a5_8(ram::SCRIPTED_B) != 0;
    if w.a5_8(ram::FROZEN) == 0 && !scripted {
        player_attacks_clash_enemies(w);
        player_attacks_clash_enemy_shots(w);
        player_hits_enemies(w);
        player_hits_boxes(w);
        player_shots_hit_enemies(w);
        if w.a5_8(ram::PAUSE) != 0 {
            return;
        }
        enemies_hit_players(w);
        enemy_shots_hit_players(w);
        boxes_hit_enemies(w);
        boxes_hit_players(w);
        players_push_others(w);
    }
    players_pick_up_items(w);
}

// ---------------------------------------------------------------------------------------------
// Joueur frappe ennemis / caisses

/// 0x26150 : le coup d'un joueur touche-t-il un ennemi (boîte d'attaque contre boîte de corps) ?
fn player_hits_enemies(w: &mut World) {
    cov::hit("player_hits_enemies");
    if List::Enemies.len(w) == 0 || w.a5_8(ram::PAUSE) != 0 {
        return;
    }
    for p in players() {
        if !w.is_active(p) || w.u8(p, f::ATTACK_BOX) == 0 {
            continue;
        }
        // A2 reste la boîte du joueur d'un ennemi à l'autre, sauf si un coup l'a écrasée (w.a2)
        w.a2 = object_box(w, rom::ATTACK, p, f::ATTACK_BOX).0;
        scan(w, List::Enemies, scratch::PLAYER_HITS_ENEMY, |w, e| {
            if w.is_active(e) && w.u8(e, f::BODY_BOX) != 0 && w.u8(e, f::BOXES_OFF) == 0 && w.u16(e, f::HP) != 0 {
                let body = object_box(w, rom::BODY, e, f::BODY_BOX);
                let atk = HitBox(w.a2);
                if overlap3(w, p, atk, e, body) {
                    player_blow_on_enemy(w, p, e);
                }
            }
            Flow::Next
        });
    }
}

/// 0x261f0 : le coup d'un joueur touche-t-il une caisse ?
fn player_hits_boxes(w: &mut World) {
    cov::hit("player_hits_boxes");
    if List::Boxes.len(w) == 0 {
        return;
    }
    for p in players() {
        if w.u8(p, f::ACTIVE) == 0 || w.u8(p, f::ATTACK_BOX) == 0 {
            continue;
        }
        let atk = object_box(w, rom::ATTACK, p, f::ATTACK_BOX);
        scan(w, List::Boxes, scratch::PLAYER_HITS_ENEMY, |w, bx| {
            if w.is_active(bx) && w.u8(bx, f::BODY_BOX) != 0 && w.u8(bx, f::BOXES_OFF) == 0 {
                let body = object_box(w, rom::BODY, bx, f::BODY_BOX);
                if overlap3(w, p, atk, bx, body) {
                    player_blow_on_box(w, p, bx);
                }
            }
            Flow::Next
        });
    }
}

/// 0x2659e : un joueur frappe une caisse (la caisse retient le coup et réagit).
fn player_blow_on_box(w: &mut World, p: Obj, bx: Obj) {
    cov::hit("player_blow_on_box");
    if w.u8(p, f::ACTION) == 6 || w.u8(p, f::ATTACK_BOX) == 4 {
        return;
    }
    w.set16(bx, f::KEYS_HELD, p.word());
    w.set8(bx, f::KEYS_NEW, 1);
    w.call_d1(0xaa96c, p, bx, 0);
}

/// 0x2630e : un joueur `p` frappe l'ennemi `e` : mémorise le coup, calcule les dégâts, lance la
/// réaction de la victime et l'étincelle.
fn player_blow_on_enemy(w: &mut World, p: Obj, e: Obj) {
    cov::hit("player_blow_on_enemy");
    trace!("coup {:06x} sur {:06x}", p.0, e.0);
    if w.u8(e, f::HITSTOP) != 0 || w.u8(p, f::ATTACK_BOX) == 4 {
        return;
    }
    // le coup de type 3 est sans effet sur le boss de type 0x12 (h)
    if w.u8(p, f::ATTACK_KIND) == 3 && w.u8(e, f::CLASS) == 4 && w.u8(e, f::KIND) == 0x12 {
        return;
    }
    w.set_bit(p, f::HIT_MARK, 0);
    // un même coup (ATTACK_ID) ne touche qu'une fois chaque victime : la victime retient le
    // dernier coup reçu de chaque joueur (octet `HIT_MARK + n° du joueur`)
    let id = w.u8(p, f::ATTACK_ID);
    let slot = e.0 + f::HIT_MARK + w.u8(p, f::KIND) as u32;
    if rd8(w.b, slot) == id {
        return;
    }
    wr8(w.b, slot, id);
    w.set8(p, f::RECOIL, 0);
    if w.u8(p, f::ACTION) != 0xa {
        w.set8(p, f::RECOIL + 1, 0x18);
        w.set8(p, f::RECOIL, 1);
        let m = w.u8(p, f::SUBACTION);
        if m != 0 {
            w.set8(p, f::RECOIL, 2);
            if m != 2 {
                w.set8(p, f::RECOIL, 4);
                if m != 4 {
                    w.set8(p, f::RECOIL, 0);
                }
            }
        }
    }
    let hp = w.u16(e, f::HP);
    w.set16(e, f::HP_PREV, hp);
    let flip = w.u8(p, f::FLIP);
    w.set8(e, f::REACTION_FLIP, flip);
    let var = w.u8(p, f::VARIANT);
    w.set8(e, f::HIT_BY_VARIANT, var);
    let kind = w.u8(p, f::KIND);
    w.set8(e, f::HIT_BY_KIND, kind);
    w.set16(e, f::LINK, p.word());
    w.set_bit(e, f::HIT_FLAGS, 0);
    let st = w.u32(e, f::STATE);
    w.set32(e, f::PREV_STATE, st);
    w.set8(e, f::HITSTOP_VICTIM, 0xc);
    w.set8(p, f::HITSTOP, 0x14);
    w.set8(p, f::HIT_FLAGS, 0x14);
    let weapon = w.u8(p, f::WEAPON_10D);
    if weapon != 0x1d && weapon != 0x1e {
        w.set8(e, f::HITSTOP_VICTIM, 8);
        w.set8(p, f::HITSTOP, 8);
        w.set8(p, f::HIT_FLAGS, 8);
    }
    if w.u8(p, f::ATTACK_KIND) == 3 {
        w.set32(p, f::STATE, state::ATTACKING);
        w.set8(p, f::SUBACTION, 0x12);
        steal_loot(w, p, e);
        enemy_reacts_to_hit(w, p, e);
        return;
    }
    // coup paré / absorbé par la victime (0xb048 : Z clair)
    if !w.call_ao(0xb048, p, e) {
        w.set32(e, f::STATE, state::BLOCKED);
        w.set8(p, f::HITSTOP, 8);
        w.set_bit(p, f::HIT_MARK, 6);
        if w.u8(p, 0x47) == 0x60 {
            w.set32(p, f::STATE, state::ATTACKING);
            w.set8(p, f::SUBACTION, 0x12);
        }
        w.call_d1(0xaaa04, p, e, 4);
        return;
    }
    count_combo(w, p, e);
    let mut dmg = player_damage(w, p, e);
    if w.u8(p, f::WEAPON_10D) == 0xb && w.u8(e, 0x82) != 0 {
        dmg = dmg.wrapping_add(dmg >> 1);
    }
    if w.u8(p, f::WEAPON_10D) == 0xc && w.u8(e, 0x83) != 0 {
        dmg = 0x200;
    }
    if w.u8(p, f::GEAR_184) != 0 {
        dmg = dmg.wrapping_add(dmg >> 2);
    }
    if w.u8(p, f::GEAR_166) == 5 && w.u8(p, f::ATTACK_KIND) == 2 {
        dmg = dmg.wrapping_add(dmg >> 2);
    }
    if w.u8(p, f::GEAR_163) == 0 {
        dmg = dmg.wrapping_add(3);
    }
    if w.u8(p, f::GEAR_166) == 2 {
        dmg = dmg.wrapping_add(1);
    }
    // plafond (octet bas seulement) fixé par le joueur
    let cap = w.u8(p, f::DAMAGE_CAP);
    if (dmg as u8 as i8) > cap as i8 {
        dmg = (dmg & 0xff00) | cap as u16;
    }
    enemy_loses_hp(w, e, dmg);
    if w.u16(e, f::HP) == 0 && w.u8(e, f::DEATH_CREDIT) != 0 {
        let v = w.u8(e, f::DEATH_VALUE);
        wr8(w.b, A5 + ram::KILL_FLAG, v);
        w.set8(p, f::KILLS_FLAG, 1);
    }
    if w.u8(p, 0x18f) != 0 {
        let add = w.u8(p, 0x18e) as u16;
        w.add16(p, f::HITS_LANDED, add);
    }
    show_enemy_hp(w, p, e);
    enemy_reacts_to_hit(w, p, e);
}

/// HP -= dégâts, plancher à 0 (soustraction sur 16 bits, résultat négatif ramené à 0).
fn enemy_loses_hp(w: &mut World, e: Obj, dmg: u16) {
    let hp = w.u16(e, f::HP).wrapping_sub(dmg);
    w.set16(e, f::HP, if (hp as i16) < 0 { 0 } else { hp });
}

/// 0x19dce : enfile l'affichage de la jauge de vie de l'ennemi touché (clé : joueur, type d'ennemi).
fn show_enemy_hp(w: &mut World, attacker: Obj, e: Obj) {
    cov::hit("show_enemy_hp");
    let key = ((w.u8(attacker, f::KIND) as u32) << 8) + w.u8(e, f::KIND) as u32;
    w.c.d[0] = key & 0xffff;
    let (hp_max, hp, class) = (w.u16(e, f::HP_MAX), w.u16(e, f::HP), w.u8(e, f::CLASS));
    set_low16(&mut w.c.d[1], hp_max);
    set_low16(&mut w.c.d[2], hp);
    set_low8(&mut w.c.d[3], class);
    w.call_ao(0x19dce, attacker, e);
}

/// Fin de 0x2630e (0x264e4) : la victime passe en réaction "touchée", l'étincelle est créée et le
/// son d'impact joué (0xe662a pour les coups simples, 0xe823a pour les spéciaux ; queue de saut).
fn enemy_reacts_to_hit(w: &mut World, p: Obj, e: Obj) {
    cov::hit("enemy_reacts_to_hit");
    w.set32(e, f::STATE, state::HIT);
    w.set8(e, f::FLAGS, 1);
    w.set8(e, f::REACTION, 0x4c);
    if w.u8(e, f::FLIES) == 0 {
        w.call_ao(0xf232e, p, e);
    }
    w.call_d1(0xaa96c, p, e, 0);
    let mut d4 = w.u8(p, f::WEAPON_10D);
    if w.u8(p, f::ATTACK_KIND) != 0 {
        d4 = w.u8(p, f::WEAPON_10E);
    }
    set_low8(&mut w.c.d[4], d4);
    let target = if w.u8(p, f::ATTACK_KIND) <= 1 { 0xe662a } else { 0xe823a };
    w.call_ao(target, p, e);
}

/// 0x26562 : compte les coups spéciaux enchaînés (octet `+0x9c` de la victime) hors classe 4.
fn count_combo(w: &mut World, p: Obj, e: Obj) {
    cov::hit("count_combo");
    let class = w.u8(p, f::VARIANT) >> 1;
    if class == 4 {
        return;
    }
    let m = w.u8(p, f::SUBACTION);
    if (m == 0xc || m == 0x16 || m == 0x18) && m != w.u8(p, f::COMBO_MARK) {
        w.add8(e, f::HITS_TAKEN, 1);
        w.set8(p, f::COMBO_MARK, m);
    }
}

// ---------------------------------------------------------------------------------------------
// Routines natives

fn combat_update_native(c: &mut Cpu, b: &mut Cps2Bus) {
    combat_update(&mut World { c, b, a2: 0 });
    crate::mem::rts(c, b);
}

/// Durée estimée : environ 110 cycles par paire (objet, objet) confrontée, plus un socle.
fn combat_update_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let _ = c;
    let mut w_ = [0u64; 5];
    for (i, n) in w_.iter_mut().enumerate() {
        *n = rd16(b, A5 + ram::COUNTS + 2 * i as u32) as u64;
    }
    let [shots, eshots, enemies, boxes, items] = w_;
    let alive = (0..4).filter(|k| rd8(b, A5 + ram::PLAYERS + 0x200 * k) != 0).count() as u64;
    let pairs = shots * enemies + boxes * enemies + alive * (4 * enemies + 2 * eshots + 3 * boxes + items);
    CYCLES_BASE + CYCLES_PER_PAIR * pairs
}

const CYCLES_BASE: u64 = 1_200;
const CYCLES_PER_PAIR: u64 = 110;

pub const COMBAT_UPDATE: Native = Native {
    entry: 0x260ee,
    name: "combat_update",
    cost: combat_update_cost,
    run: combat_update_native,
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};
