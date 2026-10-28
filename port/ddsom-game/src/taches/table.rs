//! Table des tâches en Rust : remplace les enregistrements FF0064 du noyau d'origine (voir `docs/noyau.md`).
//!
//! 16 emplacements, numérotés par leur décalage dans la table d'origine divisé par 16 (`kill(0x30)` : emplacement 3).
//! L'ordonnanceur les examine de 0 à 15 (dans l'original, D0 décompte alors de 15 à 0). Chaque emplacement porte
//! l'état et le compte d'attente de l'original, la pile 68000 donnée à la tâche et ce qu'il faut pour la reprendre :
//! son entrée tant qu'elle n'a pas tourné (son étape d'entrée : `taches::entree`), puis l'étape Rust où elle reprendra
//! et le contexte du 68000 sauvé à sa dernière cession (registres, SR et pointeur de pile ; l'adresse d'origine de
//! l'étape), et ses appels en cours ([`Cadre`]). La table garde aussi où en est le processeur (`kernel::Point`).
//!
//! La table vit dans le bus (`Cps2Bus::ext`) : la machine du jeu n'écrit plus FF0052-FF01A5 (pile de
//! l'ordonnanceur, tâche courante, tas des piles, enregistrements), et les enveloppes de cession ne sauvent plus les
//! registres sur la pile de la tâche. [`ecarts`] compare la table à celle de la RAM d'une machine d'origine.
//!
//! Une tâche reprend toujours à une étape ([`Etape`], `Reprise::Etape`) et ses appels en cours continuent à des étapes
//! ([`Suite::Etape`]) : l'ordonnanceur appelle son pas sans poser de PC. L'adresse d'origine de l'étape
//! (`Etape::adresse`) ne sert plus qu'au diagnostic, et aux registres que l'original y laisse (épilogue étranger des
//! bulles). Une suite désignée par une adresse ([`Suite::Adresse`]) n'est plus qu'un retour de la pile 68000 : celui
//! d'un appel par adresse (`crate::call`, sentinelle) ou d'un outil de diagnostic qui appelle une native.

use cps2::Cps2Bus;
use m68k::Cpu;

use super::Etape;
use crate::mem::rd32;

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

/// Les tâches du jeu, reconnues à leur entrée. Toutes sont en Rust (natives à leur entrée et à leurs adresses de
/// reprise) sauf l'écran `0x19068`, jamais atteint par les scénarios.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tache {
    /// `0x11b16` (emplacement 0) : initialisation, avertissements, création des crédits et du gel (`modes/boot.rs`),
    /// puis la machine à modes (`modes/`), dans laquelle elle continue sans changer d'entrée.
    Demarrage,
    /// `0x133fe` (emplacement 0) : la machine à modes relancée après une partie (`modes/gameover.rs`).
    Modes,
    /// `0x1c45a` (emplacement 0) : la partie (`taches/partie.rs`).
    Partie,
    /// `0x29162` (emplacement 0) : menu TEST (`taches/service.rs`).
    Service,
    /// `0x14ba`... `0x1ced4` (emplacement 1) : fondus de palette (`taches/fondu.rs`).
    Fondu,
    /// `0x13c0a` (emplacement 2) : crédits et départs (`tache_credits.rs`).
    Credits,
    /// `0x150ac` (emplacement 3) : interface des joueurs (`hud/`).
    Interface,
    /// `0xf7dfe` (emplacement 12) : portraits de la boutique (`scene/shop.rs`).
    Portraits,
    /// `0xe03fe` (emplacement 13) : « INSERT COIN » clignotant (`taches/invite.rs`).
    Invite,
    /// `0xee080` (emplacement 14) : attente active et avance rapide (`taches/gel.rs`).
    Gel,
    /// `0x1835e` (emplacement 15) : tableau des scores de l'attract (`taches/ecran_fixe.rs`).
    Scores,
    /// Autre entrée : code d'origine (l'écran `0x19068`, emplacement 15).
    #[default]
    Origine,
}

impl Tache {
    pub fn depuis_entree(entree: u32) -> Tache {
        match entree {
            0x11b16 => Tache::Demarrage,
            0x133fe => Tache::Modes,
            0x1c45a => Tache::Partie,
            0x29162 => Tache::Service,
            0x14ba | 0x14c8 | 0x14d0 | 0x14de | 0x1536 | 0x153e | 0x1ccf8 | 0x1ce64 | 0x1ced4 | 0x1cf12 => Tache::Fondu,
            0x13c0a => Tache::Credits,
            0x150ac => Tache::Interface,
            0xf7dfe => Tache::Portraits,
            0xe03fe => Tache::Invite,
            0xee080 => Tache::Gel,
            0x1835e => Tache::Scores,
            _ => Tache::Origine,
        }
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
    /// Pas encore lancée : son entrée (son étape Rust : `taches::entree`).
    Entree(u32),
    /// A cédé la main ; reprendra à cette étape Rust (le `pc` du contexte n'est que son adresse d'origine).
    Etape(Etape, Contexte),
}

impl Reprise {
    /// Contexte d'une tâche qui a cédé.
    pub fn contexte(self) -> Option<Contexte> {
        match self {
            Reprise::Entree(_) => None,
            Reprise::Etape(_, c) => Some(c),
        }
    }
}

/// Ce qui suit un appel en cours, ou une cession : une native à son adresse, ou une étape Rust.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suite {
    Adresse(u32),
    Etape(Etape),
}

impl Suite {
    /// Adresse d'origine (celle que l'original empile).
    pub fn adresse(self) -> u32 {
        match self {
            Suite::Adresse(a) => a,
            Suite::Etape(e) => e.adresse(),
        }
    }
}


impl From<Etape> for Suite {
    fn from(e: Etape) -> Self {
        Suite::Etape(e)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emplacement {
    /// La tâche qui occupe l'emplacement (d'après sa dernière entrée).
    pub tache: Tache,
    pub etat: Etat,
    /// Images d'attente restantes (gardé tel quel par les états qui ne s'en servent pas, comme l'original).
    pub compte: u8,
    /// Pile 68000 donnée à la tâche à sa création, rendue à sa mort.
    pub pile: u32,
    pub reprise: Reprise,
    /// Appels en cours de la tâche, tenus en Rust au lieu de sa pile 68000 (voir [`Cadre`]).
    pub cadres: Cadres,
}

impl Default for Emplacement {
    fn default() -> Self {
        Emplacement {
            tache: Tache::Origine,
            etat: Etat::Libre,
            compte: 0,
            pile: 0,
            reprise: Reprise::Entree(0),
            cadres: Cadres::default(),
        }
    }
}

/// Un appel en cours d'une tâche, à la place de sa pile 68000. Quand un pas Rust appelle un sous-programme qui peut
/// céder la main (fondu, écran noir, gestionnaire d'objet, mise à jour du monde...), la suite de l'appelant est rangée
/// ici au lieu d'une adresse de retour empilée (ce que le sous-programme garde pendant qu'il cède est dans son étape) :
/// à chaque cession, la pile 68000 de la tâche reste ainsi vide, et sa reprise ne dépend plus que de la table (étape,
/// registres, cadres).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cadre {
    /// Pointeur de pile 68000 quand le cadre a été ouvert : il revient à l'appelé qui rend la main avec cette pile (un
    /// appel par adresse imbriqué, `crate::call`, a toujours une pile plus basse, son adresse de retour y étant
    /// empilée).
    pub pile: u32,
    /// Quand l'appelé revient (`rts` d'une native, fin d'une étape, ou cession terminale `jmp 0xa40`), la tâche
    /// continue à cette suite.
    pub suite: Suite,
    /// Appel attendu par une fonction Rust en cours (voir [`appeler_attendu`]) : son retour rend la main à cette
    /// fonction au lieu d'exécuter la suite. La suite ne sert que si la tâche cède avant.
    pub attendu: Option<u32>,
}

/// Profondeur maximale des appels en cours d'une tâche (6 dans le jeu : tour, fin d'image, phase, monde, aiguillage,
/// sous-programme d'un gestionnaire).
pub const PROFONDEUR: usize = 8;

/// Pile des appels en cours d'une tâche (le dernier est le plus récent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cadres {
    liste: [Cadre; PROFONDEUR],
    n: usize,
}

impl Default for Cadres {
    fn default() -> Self {
        Cadres { liste: [Cadre { pile: 0, suite: Suite::Adresse(0), attendu: None }; PROFONDEUR], n: 0 }
    }
}

impl Cadres {
    pub fn empiler(&mut self, cadre: Cadre) {
        assert!(self.n < PROFONDEUR, "trop d'appels en cours pour une tâche : {:x?}", self.iter().collect::<Vec<_>>());
        self.liste[self.n] = cadre;
        self.n += 1;
    }

    pub fn depiler(&mut self) -> Option<Cadre> {
        self.n = self.n.checked_sub(1)?;
        Some(self.liste[self.n])
    }

    pub fn sommet(&self) -> Option<Cadre> {
        self.n.checked_sub(1).map(|i| self.liste[i])
    }

    pub fn est_vide(&self) -> bool {
        self.n == 0
    }

    /// Du plus ancien au plus récent.
    pub fn iter(&self) -> impl Iterator<Item = Cadre> + '_ {
        self.liste[..self.n].iter().copied()
    }

    /// Retour d'un appelé dont la pile 68000 est `pile` : l'appel en cours, s'il lui correspond.
    fn retour(&mut self, pile: u32) -> Option<Cadre> {
        match self.sommet()? {
            cadre if cadre.pile == pile => {
                self.n -= 1;
                Some(cadre)
            }
            _ => None,
        }
    }

    /// L'appel attendu `id` n'est plus attendu (la fonction Rust qui l'attendait l'abandonne) : s'il est encore là, son
    /// retour exécutera sa suite.
    fn delaisser(&mut self, id: u32) {
        for cadre in &mut self.liste[..self.n] {
            if cadre.attendu == Some(id) {
                cadre.attendu = None;
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub emplacements: [Emplacement; EMPLACEMENTS],
    /// Dernière tâche lancée (celle qui cède quand une enveloppe de cession est appelée).
    pub courante: usize,
    /// Piles libres : la dernière est la prochaine donnée (le tas FF005A de l'original, rempli par `boot_init`).
    pub piles: Vec<u32>,
    /// Où en est le processeur (point du démarrage ou de l'ordonnanceur, ou tâche en cours).
    pub point: crate::kernel::Point,
    /// Numéro du prochain appel attendu, et dernier appel attendu revenu (voir [`appeler_attendu`]).
    prochain_appel: u32,
    revenu: Option<u32>,
}

impl Table {
    /// Trap 0 : la tâche `k` (re)part de `entree` avec une pile neuve ; celle d'une tâche déjà là est rendue.
    pub fn creer(&mut self, k: usize, entree: u32) {
        if self.emplacements[k].etat != Etat::Libre {
            self.rendre_pile(k);
        }
        let pile = self.piles.pop().expect("plus de pile libre pour une tâche");
        let tache = Tache::depuis_entree(entree);
        self.emplacements[k] = Emplacement { tache, etat: Etat::Neuve, pile, reprise: Reprise::Entree(entree), ..Emplacement::default() };
    }

    /// Trap 1 (la tâche se termine) et trap 2 sur une tâche vivante.
    pub fn liberer(&mut self, k: usize) {
        self.emplacements[k].etat = Etat::Libre;
        self.emplacements[k].cadres = Cadres::default();
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
        e.tache = Tache::depuis_entree(entree);
        e.etat = Etat::Neuve;
        e.compte = 0;
        e.reprise = Reprise::Entree(entree);
        e.cadres = Cadres::default();
    }

    /// Cession de la tâche courante : nouvel état, et où la reprendre.
    pub fn ceder(&mut self, etat: Etat, compte: Option<u8>, reprise: Reprise) {
        let e = &mut self.emplacements[self.courante];
        e.etat = etat;
        if let Some(compte) = compte {
            e.compte = compte;
        }
        e.reprise = reprise;
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

    /// Appels en cours de la tâche courante.
    pub fn cadres(&mut self) -> &mut Cadres {
        &mut self.emplacements[self.courante].cadres
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

/// La table des tâches, si la machine en a une, sans la créer.
fn table_si(b: &mut Cps2Bus) -> Option<&mut Table> {
    b.ext.as_deref_mut()?.as_any_mut().downcast_mut()
}

// ---------------------------------------------------------------------------------------------
// Appels en cours (voir `Cadre`)

/// Un pas Rust de la tâche courante appelle un sous-programme qui peut céder la main : quand celui-ci reviendra, la
/// tâche continuera à `suite`. Rien n'est empilé sur la pile 68000.
pub fn appeler(c: &Cpu, b: &mut Cps2Bus, suite: impl Into<Suite>) {
    table(b).cadres().empiler(Cadre { pile: c.a[7], suite: suite.into(), attendu: None });
}

/// Appel attendu (voir [`appeler_attendu`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct Appel(u32);

/// Une fonction Rust appelle une native qui revient par `rts` (le gestionnaire d'un objet) et continue après elle : le
/// retour de la native lui rend la main ([`revenu`]). Si la native cède la main avant, la tâche reprendra plus tard à
/// `suite`, la fonction Rust ayant abandonné.
pub fn appeler_attendu(c: &Cpu, b: &mut Cps2Bus, suite: impl Into<Suite>) -> Appel {
    let t = table(b);
    t.prochain_appel = t.prochain_appel.wrapping_add(1);
    let id = t.prochain_appel;
    t.cadres().empiler(Cadre { pile: c.a[7], suite: suite.into(), attendu: Some(id) });
    Appel(id)
}

/// L'appel attendu est encore en cours (son cadre est dans la pile des appels de la tâche courante).
pub fn en_attente(b: &Cps2Bus, appel: Appel) -> bool {
    lire(b).is_some_and(|t| t.emplacements[t.courante].cadres.iter().any(|k| k.attendu == Some(appel.0)))
}

/// L'appel attendu est-il revenu ? Sinon (la tâche a cédé, ou un appelé a dépilé l'appel pour revenir plus haut), la
/// fonction Rust qui l'attendait abandonne : son cadre, s'il est encore là, exécutera sa suite quand l'appelé reviendra.
pub fn revenu(b: &mut Cps2Bus, appel: Appel) -> bool {
    let t = table(b);
    if t.revenu == Some(appel.0) {
        t.revenu = None;
        return true;
    }
    t.cadres().delaisser(appel.0);
    false
}

/// Retour d'un sous-programme : la suite de l'appel en cours de la tâche courante s'il a été fait par un pas Rust avec
/// cette pile, sinon l'adresse de retour dépilée de la pile 68000. `None` : l'appel était attendu par une fonction Rust
/// ([`appeler_attendu`]), qui reprend la main.
pub fn retour(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Suite> {
    let pile = c.a[7];
    if let Some(t) = table_si(b) {
        if let Some(cadre) = t.cadres().retour(pile) {
            if let Some(id) = cadre.attendu {
                t.revenu = Some(id);
                return None;
            }
            return Some(cadre.suite);
        }
    }
    let adresse = rd32(b, pile);
    c.a[7] = pile.wrapping_add(4);
    Some(Suite::Adresse(adresse))
}

/// Comme [`retour`], pour une cession terminale (`jmp 0xa40`) ou un appel abandonné : la suite, qu'elle soit attendue ou
/// non (la fonction Rust qui l'attendait voit qu'elle n'est pas revenue).
pub fn depiler_retour(c: &mut Cpu, b: &mut Cps2Bus) -> Suite {
    let pile = c.a[7];
    if let Some(cadre) = table_si(b).and_then(|t| t.cadres().retour(pile)) {
        return cadre.suite;
    }
    let retour = rd32(b, pile);
    c.a[7] = pile.wrapping_add(4);
    Suite::Adresse(retour)
}

/// Les `n` appels en cours les plus récents de la tâche courante sont dépilés sans revenir (une routine qui dépile les
/// retours de ses appelants pour revenir plus haut : épilogue étranger des bulles) ; rend leurs suites, de la plus
/// récente à la plus ancienne.
pub fn depiler_appels(b: &mut Cps2Bus, n: usize) -> Vec<Suite> {
    let cadres = table(b).cadres();
    (0..n)
        .map(|_| cadres.depiler().unwrap_or_else(|| panic!("{n} appels en cours à dépiler : il n'y en a pas assez")).suite)
        .collect()
}

/// La native qui va s'exécuter a été appelée par un pas Rust (le dernier cadre de la tâche courante est à la hauteur
/// de pile actuelle) : en mode hybride, elle n'est jamais remplacée par son original, même quand elle déborderait sur
/// l'interruption suivante (l'original ne trouverait pas son retour sur la pile 68000).
pub fn appelee_par_un_cadre(c: &Cpu, b: &Cps2Bus) -> bool {
    c.sr & 0x2000 == 0
        && lire(b).is_some_and(|t| t.emplacements[t.courante].cadres.sommet().is_some_and(|k| k.pile == c.a[7]))
}

thread_local! {
    /// Diagnostic : dernières instructions 68000 et natives exécutées (PC, pile), montrées quand la vérification échoue.
    static TRACE: std::cell::RefCell<std::collections::VecDeque<(u32, u32)>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
}

/// Diagnostic : note une instruction ou une native exécutée (voir [`verifier_68000`]).
pub fn noter(pc: u32, pile: u32) {
    TRACE.with(|t| {
        let mut t = t.borrow_mut();
        if t.len() == 600 {
            t.pop_front();
        }
        t.push_back((pc, pile));
    });
}

/// Diagnostic (`examples/cessions`) : avant chaque instruction 68000, vérifie qu'elle ne dépile pas un cadre de la tâche
/// courante tenu en Rust (pile au-dessus de celle du dernier cadre, ou `rts`/`rtr` à sa hauteur).
pub fn verifier_68000(c: &Cpu, b: &mut Cps2Bus) {
    if c.sr & 0x2000 != 0 {
        return;
    }
    noter(c.pc, c.a[7]);
    // DDSOM_ARRET=pc (hexadécimal) : arrêt, avec la trace, quand le 68000 atteint cette adresse
    static ARRET: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    if *ARRET.get_or_init(|| std::env::var("DDSOM_ARRET").ok().and_then(|v| u32::from_str_radix(&v, 16).ok())) == Some(c.pc) {
        let trace = TRACE.with(|t| t.borrow().iter().map(|(pc, sp)| format!("{pc:x}/{sp:x}")).collect::<Vec<_>>().join(" "));
        panic!("arrêt en {:x} (pile {:x}) ; avant : {trace}", c.pc, c.a[7]);
    }
    let Some(t) = lire(b) else { return };
    let k = t.courante;
    let Some(cadre) = t.emplacements[k].cadres.sommet() else { return };
    let op = m68k::decode::Fetch::fetch16(b, c.pc);
    if c.a[7] > cadre.pile || (c.a[7] == cadre.pile && matches!(op, 0x4e75 | 0x4e77)) {
        let trace = TRACE.with(|t| t.borrow().iter().map(|(pc, sp)| format!("{pc:x}/{sp:x}")).collect::<Vec<_>>().join(" "));
        panic!("code 68000 en {:x} (pile {:x}) : dépile un cadre de la tâche {k} tenu en Rust ({cadre:x?}) ; avant : {trace}", c.pc, c.a[7]);
    }
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
            Reprise::Etape(_, c) if tout && !en_cours => {
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
            Reprise::Etape(..) => {}
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
    fn un_appel_en_cours_ne_revient_qu_a_sa_pile() {
        let mut k = Cadres::default();
        k.empiler(Cadre { pile: 0x1000, suite: Suite::Adresse(0x1341a), attendu: None });
        k.empiler(Cadre { pile: 0x1000, suite: Suite::Adresse(0x1376e), attendu: Some(1) });
        // un appel par adresse imbriqué revient avec une pile plus basse (son adresse de retour y est empilée)
        assert_eq!(k.retour(0xffc), None);
        assert_eq!(k.retour(0x1000).map(|k| k.suite), Some(Suite::Adresse(0x1376e)));
        assert_eq!(k.retour(0x1000).map(|k| k.suite), Some(Suite::Adresse(0x1341a)));
        assert_eq!(k.retour(0x1000), None);
    }

    #[test]
    fn une_image_decomptee_par_le_gel_est_admise() {
        assert!(un_decompte_d_ecart((Etat::Attend, 1), (4, 0)));
        assert!(!un_decompte_d_ecart((Etat::Attend, 2), (8, 0)));
        assert!(un_decompte_d_ecart((Etat::Attend, 2), (1, 3)));
        assert!(un_decompte_d_ecart((Etat::Prete, 0), (1, 1)));
        assert!(!un_decompte_d_ecart((Etat::Attend, 3), (1, 1)));
        assert!(un_decompte_d_ecart((Etat::Attend, 1), (1, 0)));
        assert!(un_decompte_d_ecart((Etat::Prete, 0), (1, 0)));
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
