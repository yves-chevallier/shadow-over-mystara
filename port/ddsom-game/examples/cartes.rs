//! Cartes des niveaux : image complète du décor (scroll 2 et scroll 3) de chaque scène, sans sprites ni
//! interface, reconstruite depuis les cartes de décor de la ROM. Voir `docs/niveaux.md`.
//!
//!   cartes cartes  <roms> <eeprom> <dossier> [scène...]   une image par scène (1 à 82 par défaut)
//!   cartes planche <dossier> <niveaux.md>                 planche d'aperçu `planche.png` + index Markdown
//!   cartes tables  <roms>                                 dimensions des tables de bandes des 115 scènes
//!
//! Méthode. Les caméras du jeu (`scroll.rs`) recopient dans leurs couches des colonnes de cases lues dans une
//! carte de ROM : des *bandes* de 256x256 pixels (indices de blocs 2x2, banques de blocs) que désigne, pour
//! chaque scène, une table de mots (`0x2ff94 + 4 s` pour le scroll 2, cases de 16 pixels ; `0x32d7c + 4 s` pour
//! le scroll 3, cases de 32 pixels). L'outil lit ces mêmes tables (voir `Couche::grille`), recompose toutes les
//! cases de la carte, les dessine avec la palette de la scène relevée dans la machine (jeu réel lancé jusqu'à
//! la phase de jeu de la scène, `FF8034` forcé comme dans `docs/couverture.md`) et superpose les deux couches
//! dans l'ordre que fixe CPS-B. Aucun sprite, aucune interface, pas de scroll 1.

use std::path::Path;

const MAP1: u32 = 0x1f9ba0;
const BANKS1: u32 = 0x320f6;
const TABLES1: u32 = 0x2ff94;
const MAP2: u32 = 0x2459c0;
const BANKS2: u32 = 0x33a9c;
const TABLES2: u32 = 0x32d7c;
/// Base des tuiles de décor dans la région graphique (banque 1 du mapper CPS2).
const TILES: usize = 0x20000 * 64;
const SCENES: std::ops::RangeInclusive<u32> = 1..=82;
/// Largeur de l'écran : pas de la superposition des couches à parallaxe.
const ECRAN: usize = 384;
const FOND: [u8; 3] = [24, 16, 40];

// ---------------------------------------------------------------------------------------------
// Cartes en ROM
// ---------------------------------------------------------------------------------------------

struct Rom<'a> {
    data: &'a [u16],
    prog: &'a [u16],
    gfx: &'a [u8],
}

impl Rom<'_> {
    fn rd16(&self, a: u32) -> u16 {
        self.data.get((a >> 1) as usize).copied().unwrap_or(0)
    }
    fn rd32(&self, a: u32) -> u32 {
        (self.rd16(a) as u32) << 16 | self.rd16(a.wrapping_add(2)) as u32
    }
    /// Lecture en espace programme (tables du code).
    fn prog32(&self, a: u32) -> u32 {
        (self.prog[(a >> 1) as usize] as u32) << 16 | self.prog[(a >> 1) as usize + 1] as u32
    }
}

/// Une couche de décor : géométrie de sa carte en ROM.
#[derive(Clone, Copy)]
struct Couche {
    map: u32,
    banks: u32,
    tables: u32,
    /// Adresse du mot de pas de ligne de la première table de bandes.
    debut: u32,
    /// Taille d'une case en pixels (16 ou 32) et nombre de cases par côté de bande.
    case: usize,
    cases: usize,
    /// Décalage de tuile ajouté au code de la carte, base de couleur du matériel.
    decalage: u16,
    couleur: u16,
    /// Numéro de bande de remplissage (jamais affichée : la caméra ne va pas jusque-là), laissée vide.
    vide: u16,
}

const SCROLL2: Couche = Couche { map: MAP1, banks: BANKS1, tables: TABLES1, debut: 0x318a2, case: 16, cases: 16, decalage: 0x1800, couleur: 0x40, vide: 0xfc };
const SCROLL3: Couche = Couche { map: MAP2, banks: BANKS2, tables: TABLES2, debut: 0x33564, case: 32, cases: 8, decalage: 0x1f00, couleur: 0x60, vide: 0xfd };

impl Couche {
    /// Case (code de tuile, attribut) à l'adresse `at` de la carte virtuelle (`map_cell` de `scroll.rs`).
    fn case_a(&self, rom: &Rom, at: u32) -> (u16, u16) {
        let off = at.wrapping_sub(self.map);
        let c = off >> 2;
        let (bank, index, sub) = if self.case == 16 {
            let bank = rom.rd32(self.banks + (off >> 16 & 0xc));
            let index = rom.rd16(self.map + (c & 0xe) + (c >> 1 & 0x3fff0)) as u32;
            (bank, index, (off as u16 >> 3 & 8) as u32 + (off & 4))
        } else {
            let bank = rom.rd32(self.banks + ((off >> 16 & 0xf) << 2));
            let index = rom.rd16(self.map + (c & 6) + (c >> 1 & !7)) as u32;
            (bank, index, (off as u16 >> 2 & 8) as u32 + (off & 4))
        };
        let v = rom.rd32(bank.wrapping_add(index << 4).wrapping_add(sub));
        ((v >> 16) as u16 + self.decalage, v as u16)
    }

    /// Grille des numéros de bandes de la scène, de haut en bas (vide si les tables sont incohérentes).
    ///
    /// Les tables de toutes les scènes se suivent en ROM : un mot de pas de ligne `W` (celui de la caméra,
    /// `+0x40`), `K` lignes de `W` mots au-dessus de la ligne d'ordonnée 0 (celle que désigne le pointeur de la
    /// scène : les ordonnées croissantes sont aux adresses décroissantes), puis quelques lignes d'ordonnée
    /// négative (la caméra descend un peu sous 0). Le pointeur d'une table fixe donc où commence la suivante :
    /// la lecture part de la première table et cherche, après chaque pointeur, le nombre de lignes négatives
    /// qui amène à un en-tête et à un pointeur suivants cohérents. Dans le scroll 2 le dernier mot de chaque
    /// ligne est une sentinelle (0xfe).
    fn grille(&self, rom: &Rom, scene: u32) -> Vec<Vec<u16>> {
        let mut pointeurs: Vec<u32> = (0..115).map(|s| rom.prog32(self.tables + 4 * s)).collect();
        pointeurs.sort();
        pointeurs.dedup();
        let cible = rom.prog32(self.tables + 4 * scene);
        let valide = |tete: u32, p: u32| {
            let w = rom.rd16(tete) as u32;
            (1..=64).contains(&w) && p >= tete + 2 && (p - tete - 2) % (2 * w) == 0
        };
        let mut tete = self.debut;
        for (i, &p) in pointeurs.iter().enumerate() {
            if !valide(tete, p) {
                return Vec::new();
            }
            let w = rom.rd16(tete) as u32;
            let apres = (0..=8u32).find(|t| pointeurs.get(i + 1).map_or(*t == 0, |&q| valide(p + 2 * w * (1 + t), q)));
            let Some(apres) = apres else { return Vec::new() };
            if p == cible {
                let sentinelle = (self.case == 16) as u32;
                return (0..=(p - tete - 2) / (2 * w) + apres)
                    .map(|i| (0..w - sentinelle).map(|x| rom.rd16(tete + 2 + 2 * w * i + 2 * x)).collect())
                    .collect();
            }
            tete = p + 2 * w * (1 + apres);
        }
        Vec::new()
    }

    /// Image du décor : les bandes de la grille côte à côte, chaque case dessinée avec la palette `pal`.
    fn image(&self, rom: &Rom, pal: &[[u8; 3]], grille: &[Vec<u16>]) -> Image {
        let bande = self.case * self.cases;
        let mut img = Image::new(grille[0].len() * bande, grille.len() * bande);
        let colonne = if self.case == 16 { 0x40 } else { 0x20 };
        for (ky, ligne) in grille.iter().enumerate() {
            for (kx, &strip) in ligne.iter().enumerate() {
                if strip == self.vide {
                    continue;
                }
                let base = self.map + (strip as u32) * (self.cases * self.cases * 4) as u32;
                for cx in 0..self.cases {
                    for cy in 0..self.cases {
                        let (code, attr) = self.case_a(rom, base + cx as u32 * colonne + cy as u32 * 4);
                        self.tuile(rom, pal, &mut img, kx * bande + cx * self.case, ky * bande + cy * self.case, code, attr);
                    }
                }
            }
        }
        img
    }

    /// La scène utilise la table par défaut (celle de la scène 1) : sa couche n'est pas tirée d'une carte.
    fn partagee(&self, rom: &Rom, scene: u32) -> bool {
        rom.prog32(self.tables + 4 * scene) == rom.prog32(self.tables + 4)
    }


    /// Dessine une tuile (décodage de `cps2-gfx` : un plan par octet, miroirs aux bits 5 et 6 de l'attribut).
    fn tuile(&self, rom: &Rom, pal: &[[u8; 3]], img: &mut Image, x0: usize, y0: usize, code: u16, attr: u16) {
        let t = self.case;
        for py in 0..t {
            for px in 0..t {
                let sx = if attr & 0x20 != 0 { t - 1 - px } else { px };
                let sy = if attr & 0x40 != 0 { t - 1 - py } else { py };
                let ligne = if t == 16 {
                    TILES + code as usize * 128 + sy * 8 + (sx / 8) * 4
                } else {
                    TILES + (code as usize & 0x3fff) * 512 + sy * 16 + (sx / 8) * 4
                };
                if ligne + 4 > rom.gfx.len() {
                    continue;
                }
                let bit = 7 - sx % 8;
                let p = (rom.gfx[ligne + 3] >> bit & 1) << 3 | (rom.gfx[ligne + 2] >> bit & 1) << 2 | (rom.gfx[ligne + 1] >> bit & 1) << 1 | (rom.gfx[ligne] >> bit & 1);
                if p != 15 {
                    img.set(x0 + px, y0 + py, pal[((attr & 0x1f) + self.couleur) as usize * 16 + p as usize]);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------------------------

/// Image RGBA (alpha 0 : rien dessiné).
struct Image {
    w: usize,
    h: usize,
    px: Vec<[u8; 4]>,
}

impl Image {
    fn new(w: usize, h: usize) -> Self {
        Image { w, h, px: vec![[0; 4]; w * h] }
    }

    fn set(&mut self, x: usize, y: usize, c: [u8; 3]) {
        self.px[y * self.w + x] = [c[0], c[1], c[2], 255];
    }

    /// Retire les bords entièrement vides.
    fn recadrer(self) -> Image {
        let (mut x0, mut y0, mut x1, mut y1) = (self.w, self.h, 0, 0);
        for y in 0..self.h {
            for x in 0..self.w {
                if self.px[y * self.w + x][3] != 0 {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
                }
            }
        }
        if x1 == 0 {
            return self;
        }
        let mut img = Image::new(x1 - x0, y1 - y0);
        for y in y0..y1 {
            img.px[(y - y0) * img.w..][..img.w].copy_from_slice(&self.px[y * self.w + x0..][..img.w]);
        }
        img
    }

    fn png(&self, path: &Path) {
        let mut buf = Vec::with_capacity(self.w * self.h * 3);
        for p in &self.px {
            buf.extend_from_slice(if p[3] == 0 { &FOND } else { &p[..3] });
        }
        write_png(path, self.w, self.h, &buf);
    }
}


/// Superpose les couches (de l'arrière vers l'avant), chacune avec l'ordonnée `u` de sa ligne du bas (les
/// ordonnées du décor croissent vers le haut ; l'écart entre couches vient des caméras au début de la scène).
/// Une couche plus étroite que le cadre est à parallaxe : l'écran qui voit la colonne `x` du cadre voit
/// ses colonnes à partir de `x * (w - 384) / (cadre - 384)` ; l'image est donc assemblée par tranches d'un
/// écran, chacune telle que le joueur la voit.
fn composer(couches: &[(&Image, i64)]) -> Image {
    let w = couches.iter().map(|c| c.0.w).max().unwrap();
    let bas = couches.iter().map(|c| c.1).min().unwrap();
    let haut = couches.iter().map(|c| c.1 + c.0.h as i64).max().unwrap();
    let h = (haut - bas) as usize;
    let mut img = Image::new(w, h);
    let mut x0 = 0;
    while x0 < w {
        let camera = x0.min(w.saturating_sub(ECRAN));
        for &(c, u) in couches {
            let decalage = if c.w >= w || w <= ECRAN { camera } else { camera * c.w.saturating_sub(ECRAN) / (w - ECRAN) };
            let y0 = (haut - u - c.h as i64) as usize;
            for x in x0..(x0 + ECRAN).min(w) {
                let sx = x - camera + decalage;
                if sx >= c.w {
                    continue;
                }
                for sy in 0..c.h {
                    let p = c.px[sy * c.w + sx];
                    if p[3] != 0 {
                        img.px[(y0 + sy) * w + x] = p;
                    }
                }
            }
        }
        x0 += ECRAN;
    }
    img.recadrer()
}

fn write_png(path: &Path, w: usize, h: usize, rgb: &[u8]) {
    let mut e = png::Encoder::new(std::fs::File::create(path).unwrap(), w as u32, h as u32);
    e.set_color(png::ColorType::Rgb);
    e.set_compression(png::Compression::Fast);
    e.write_header().unwrap().write_image_data(rgb).unwrap();
}

fn read_png(path: &Path) -> (usize, usize, Vec<u8>) {
    let mut r = png::Decoder::new(std::fs::File::open(path).unwrap()).read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).unwrap();
    (info.width as usize, info.height as usize, buf)
}

// ---------------------------------------------------------------------------------------------
// Machine : atteindre une scène
// ---------------------------------------------------------------------------------------------

fn rw(m: &cps2::Machine, a: u32) -> u16 {
    m.bus.ram[((a - 0xff0000) / 2) as usize]
}

fn phase(m: &cps2::Machine) -> u32 {
    (rw(m, 0xff802c) >> 8) as u32
}

fn scene(m: &cps2::Machine) -> u32 {
    (rw(m, 0xff8034) >> 8) as u32
}

fn poke(m: &mut cps2::Machine, a: u32, v: u32, taille: u8) {
    ddsom_game::scenario::poke(m, a, v, taille);
}

/// Lance le jeu : pièce, Start, personnage choisi, `FF8034` forcé pendant la sélection (scène 1).
fn demarrer(roms: &Path, eeprom: &Path) -> cps2::Machine {
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(roms, ee.as_deref()).unwrap();
    ddsom_game::install(&mut m);
    m.set_overclock(16.0);
    m.idle = Some(ddsom_game::idle);
    let entrees = [(922, "Coin 1", true), (928, "Coin 1", false), (1002, "1 Player Start", true), (1008, "1 Player Start", false), (1442, "P1 Button 1", true), (1448, "P1 Button 1", false)];
    for f in 0..2400 {
        for &(i, nom, bas) in &entrees {
            if i == f {
                ddsom_game::scenario::set_input(&mut m, nom, bas);
            }
        }
        if f == 2399 {
            poke(&mut m, 0xff8034, 1, 1);
        }
        m.run_frame();
    }
    m
}

/// Ce que la machine donne d'une scène en jeu : palette et ordre des couches.
struct Releve {
    pal: Vec<[u8; 3]>,
    /// Couches de décor (2, 3) de l'arrière vers l'avant.
    ordre: Vec<u8>,
    /// Écart d'ordonnée entre les caméras 1 (scroll 2) et 2 (scroll 3) au début de la scène.
    ecart_y: i64,
    en_jeu: bool,
}

/// Saute dans la scène `n` (partie en cours, joueur invincible), attend la phase de jeu et relève la palette.
fn entrer(m: &mut cps2::Machine, n: u32) -> Releve {
    for (a, v) in [(0xff8034, n), (0xff802c, 8), (0xff802d, 0), (0xff802e, 0)] {
        poke(m, a, v, 1);
    }
    let (mut stable, mut en_jeu) = (0, false);
    for _ in 0..4000 {
        m.run_frame();
        poke(m, 0xff85de + 0x62, 0x70, 2);
        if phase(m) == 12 && scene(m) == n {
            stable += 1;
            if stable >= 90 {
                en_jeu = true;
                break;
            }
        }
    }
    let b = &m.bus;
    let v = cps2_gfx::VideoState { gfxram: &b.gfxram, cps_a: &b.cps_a, cps_b: &b.cps_b, objram: &m.obj_buffer, output: &m.pri_ctrl_buffer, palette_ram: Some(&b.palette) };
    let commande = b.cps_b[0x26 / 2];
    // couche active : bit de CPS-B (0x04 / 0x08) et bit de commande vidéo CPS-A (4 / 8), comme `cps2-gfx`
    let active = |l: u8| commande & (1 << l) != 0 && b.cps_a[0x22 / 2] & (1 << l) != 0;
    let ordre = (0..4).map(|i| (commande >> (6 + 2 * i) & 3) as u8).filter(|&l| (l == 2 || l == 3) && active(l)).collect();
    // positions cibles des caméras (enregistrements `A5 + 0x4fe` et `A5 + 0x54e`, champ `+0x0c`)
    let (y1, y2) = (rw(m, 0xff84fe + 0xc) as i16 as i64, rw(m, 0xff854e + 0xc) as i16 as i64);
    Releve { pal: cps2_gfx::render::build_palette(&v), ordre, ecart_y: y1 - y2, en_jeu }
}

/// Scène qui suit naturellement : on force la phase 16 (sortie de scène) et on lit la scène suivante.
fn suivante(m: &mut cps2::Machine) -> Option<u32> {
    let n = scene(m);
    for (a, v) in [(0xff802c, 16), (0xff802d, 0), (0xff802e, 0)] {
        poke(m, a, v, 1);
    }
    for _ in 0..3000 {
        m.run_frame();
        poke(m, 0xff85de + 0x62, 0x70, 2);
        if scene(m) != n && phase(m) == 8 {
            return Some(scene(m));
        }
    }
    None
}

fn cartes(roms: &Path, eeprom: &Path, sortie: &Path, scenes: &[u32]) {
    let mut m = demarrer(roms, eeprom);
    let (gfx, data, prog) = (m.gfx.clone(), m.bus.rom.clone(), m.bus.rom_dec.clone());
    let rom = Rom { data: &data, prog: &prog, gfx: &gfx };
    for d in ["couches", "infos"] {
        std::fs::create_dir_all(sortie.join(d)).unwrap();
    }
    let routes = std::env::var("DDSOM_ROUTES").is_ok();
    for &n in scenes {
        let r = entrer(&mut m, n);
        if std::env::var("DDSOM_REF").is_ok() {
            write_png(&sortie.join(format!("ref_{n:02}.png")), 384, 224, &m.render().concat());
        }
        let (g2, g3) = (SCROLL2.grille(&rom, n), SCROLL3.grille(&rom, n));
        if g2.is_empty() || g3.is_empty() {
            eprintln!("scène {n} : tables de bandes illisibles");
            continue;
        }
        let (s2, s3) = (SCROLL2.image(&rom, &r.pal, &g2), SCROLL3.image(&rom, &r.pal, &g3));
        // Les scènes dont la table du scroll 3 est celle par défaut (scène 1) n'ont pas de carte pour cette
        // couche : sa mémoire est un reste d'une scène précédente, masqué par le scroll 2 (vérifié par capture).
        let sans_s3 = n != 1 && SCROLL3.partagee(&rom, n);
        let ordre: Vec<u8> = r.ordre.iter().copied().filter(|&l| l == 2 || !sans_s3).collect();
        if ordre.is_empty() {
            eprintln!("scène {n} : aucune couche de décor active");
            continue;
        }
        let couches: Vec<(&Image, i64)> = ordre.iter().map(|&l| if l == 2 { (&s2, -r.ecart_y) } else { (&s3, 0) }).collect();
        let img = composer(&couches);
        img.png(&sortie.join(format!("{n:02}.png")));
        s2.png(&sortie.join("couches").join(format!("{n:02}_scroll2.png")));
        if !sans_s3 {
            s3.png(&sortie.join("couches").join(format!("{n:02}_scroll3.png")));
        }
        // sans DDSOM_ROUTES, la scène suivante déjà relevée est conservée
        let ancienne = std::fs::read_to_string(sortie.join("infos").join(format!("{n:02}.tsv"))).ok().and_then(|t| t.trim().split('\t').nth(6).and_then(|s| s.parse().ok()));
        let suite = if routes { suivante(&mut m) } else { ancienne };
        let ordre: Vec<String> = ordre.iter().map(|l| l.to_string()).collect();
        let info = format!("{n}\t{}\t{}\t{}x{}\t{}x{}\t{}\t{}\t{}\n", img.w, img.h, s2.w, s2.h, s3.w, s3.h, ordre.join(">"), suite.map_or(String::from("-"), |s| s.to_string()), r.en_jeu as u8);
        std::fs::write(sortie.join("infos").join(format!("{n:02}.tsv")), info).unwrap();
        println!("scène {n} : {}x{} (scroll 2 {}x{}, scroll 3 {}x{}, ordre {}) suite {suite:?}{}", img.w, img.h, s2.w, s2.h, s3.w, s3.h, ordre.join(">"), if r.en_jeu { "" } else { " ATTENTION : phase de jeu non atteinte, palette douteuse" });
    }
}

// ---------------------------------------------------------------------------------------------
// Planche d'aperçu et index
// ---------------------------------------------------------------------------------------------

/// Chiffres 3x5 pour les numéros de la planche.
const CHIFFRES: [[u8; 5]; 10] = [[7, 5, 5, 5, 7], [2, 6, 2, 2, 7], [7, 1, 7, 4, 7], [7, 1, 7, 1, 7], [5, 5, 7, 1, 1], [7, 4, 7, 1, 7], [7, 4, 7, 5, 7], [7, 1, 1, 1, 1], [7, 5, 7, 5, 7], [7, 5, 7, 1, 7]];

fn etiquette(rgb: &mut [u8], largeur: usize, x0: usize, y0: usize, n: u32) {
    let texte = n.to_string();
    let k = 4;
    // fond sombre sous le numéro
    for y in y0 - 2..y0 + 5 * k + 2 {
        for x in x0 - 2..x0 + texte.len() * 4 * k + 2 {
            rgb[(y * largeur + x) * 3..][..3].copy_from_slice(&[0, 0, 0]);
        }
    }
    for (i, c) in texte.bytes().enumerate() {
        for (ry, ligne) in CHIFFRES[(c - b'0') as usize].iter().enumerate() {
            for rx in 0..3 {
                if ligne >> (2 - rx) & 1 != 0 {
                    for (dy, dx) in (0..k * k).map(|v| (v / k, v % k)) {
                        let (x, y) = (x0 + (i * 4 + rx) * k + dx, y0 + ry * k + dy);
                        rgb[(y * largeur + x) * 3..][..3].copy_from_slice(&[255, 255, 255]);
                    }
                }
            }
        }
    }
}

fn planche(dossier: &Path, doc: &Path) {
    const CELLULE: (usize, usize) = (320, 190);
    const COLONNES: usize = 5;
    let vues: Vec<u32> = SCENES.filter(|n| dossier.join(format!("{n:02}.png")).exists()).collect();
    let lignes = vues.len().div_ceil(COLONNES);
    let (lw, lh) = (COLONNES * (CELLULE.0 + 8) + 8, lines_height(lignes, CELLULE.1));
    let mut rgb = vec![0u8; lw * lh * 3];
    for p in rgb.chunks_mut(3) {
        p.copy_from_slice(&[12, 10, 20]);
    }
    for (i, &n) in vues.iter().enumerate() {
        let (w, h, src) = read_png(&dossier.join(format!("{n:02}.png")));
        // réduction par moyenne, à l'échelle qui fait entrer l'image dans la cellule
        let echelle = (w as f64 / CELLULE.0 as f64).max(h as f64 / CELLULE.1 as f64).max(1.0);
        let (tw, th) = (((w as f64 / echelle) as usize).max(1), ((h as f64 / echelle) as usize).max(1));
        let (ox, oy) = (8 + (i % COLONNES) * (CELLULE.0 + 8), 8 + (i / COLONNES) * (CELLULE.1 + 8));
        for ty in 0..th {
            for tx in 0..tw {
                let (x0, x1) = ((tx as f64 * echelle) as usize, (((tx + 1) as f64 * echelle) as usize).clamp(1, w).max((tx as f64 * echelle) as usize + 1).min(w));
                let (y0, y1) = ((ty as f64 * echelle) as usize, (((ty + 1) as f64 * echelle) as usize).max((ty as f64 * echelle) as usize + 1).min(h));
                let mut acc = [0u32; 3];
                for y in y0..y1 {
                    for x in x0..x1 {
                        for c in 0..3 {
                            acc[c] += src[(y * w + x) * 3 + c] as u32;
                        }
                    }
                }
                let nb = ((y1 - y0) * (x1 - x0)).max(1) as u32;
                for c in 0..3 {
                    rgb[((oy + ty) * lw + ox + tx) * 3 + c] = (acc[c] / nb) as u8;
                }
            }
        }
        etiquette(&mut rgb, lw, ox + 4, oy + 4, n);
    }
    write_png(&dossier.join("planche.png"), lw, lh, &rgb);
    index(dossier, doc, &vues);
    println!("planche : {} vignettes, {lw}x{lh}", vues.len());
}

fn lines_height(lignes: usize, cellule: usize) -> usize {
    lignes * (cellule + 8) + 8
}

/// Boss connus (`docs/couverture.md`, `docs/atlas.md`).
fn boss(n: u32) -> &'static str {
    match n {
        24 => "Manticore",
        32 => "Black Dragon",
        40 => "Salamandre (givre)",
        42 => "Salamandre (flamme)",
        54 => "Tel'Arin",
        _ => "",
    }
}

fn index(dossier: &Path, doc: &Path, vues: &[u32]) {
    let mut md = String::new();
    md += "# Niveaux : images complètes des décors\n\n";
    md += "Une image par scène jouable (1 à 68) et par boutique (69 à 82) : tout le décor de la scène (scroll 2 et\n";
    md += "scroll 3 superposés dans l'ordre du matériel), **sans sprites ni interface**. `planche.png` en donne un\n";
    md += "aperçu (vignette et numéro de chaque scène).\n\n";
    md += "Les images sont tirées des ROMs (œuvre protégée) : elles ne sont **jamais versionnées**. Elles sont écrites\n";
    md += "dans `assets/niveaux/` (dossier ignoré par git) ; les liens ci-dessous ne fonctionnent qu'après les avoir\n";
    md += "générées.\n\n";
    md += "## Régénérer\n\n```sh\n";
    md += "# depuis port/ ; les ROMs sont lues dans DDSOM_ROMS (dossier des fichiers dd2.*), l'EEPROM en deuxième argument\n";
    md += "export DDSOM_ROMS=$PWD/../rom\n";
    md += "cargo run --release -p ddsom-game --example cartes -- cartes ../rom ../testdata/eeprom_ddsom.bin ../assets/niveaux\n";
    md += "cargo run --release -p ddsom-game --example cartes -- planche ../assets/niveaux ../docs/niveaux.md\n";
    md += "```\n\n";
    md += "La première commande démarre le jeu (pièce, Start, personnage), saute dans chaque scène et relève la\n";
    md += "palette ; elle prend une dizaine de minutes pour les 82 scènes (numéros de scènes en arguments pour n'en\n";
    md += "refaire que quelques-unes ; `DDSOM_ROUTES=1` relève aussi la scène suivante en forçant la sortie de scène,\n";
    md += "`DDSOM_REF=1` enregistre une capture du jeu `ref_NN.png` pour comparaison). La seconde recrée la planche\n";
    md += "et réécrit ce fichier.\n\n";
    md += "## Méthode\n\n";
    md += "Le décor n'est pas reconstitué par des captures d'écran : il est relu dans la ROM, comme le font les\n";
    md += "caméras du jeu (`port/ddsom-game/src/scroll.rs`, `docs/video.md` §B1).\n\n";
    md += "* Chaque couche est une grille de *bandes* de 256x256 pixels : 16x16 cases de 16 pixels (scroll 2, carte\n";
    md += "  `0x1f9ba0`, banques `0x320f6`) ou 8x8 cases de 32 pixels (scroll 3, carte `0x2459c0`, banques `0x33a9c`).\n";
    md += "* Pour chaque scène, une table du code (`0x2ff94 + 4 s` et `0x32d7c + 4 s`) donne l'adresse d'une table de\n";
    md += "  numéros de bandes : un mot `W` (pas de ligne de la caméra), `K` lignes de `W` mots au-dessus de la ligne\n";
    md += "  d'ordonnée 0 que désigne le pointeur (ordonnées croissantes = adresses décroissantes), puis quelques\n";
    md += "  lignes sous 0. Toutes les tables se suivent en ROM ; le pointeur de chacune fixe le début de la suivante,\n";
    md += "  ce qui donne les dimensions exactes de chaque carte. Dans le scroll 2 le dernier mot de ligne est une\n";
    md += "  sentinelle (`0xfe`).\n";
    md += "* L'outil lit ces tables, compose toutes les cases (même décodage que `cps2-gfx`), les peint avec la palette\n";
    md += "  de la scène (relevée en jeu après l'ouverture) et superpose les deux couches dans l'ordre de CPS-B.\n";
    md += "* Contrôle : les bandes que la mémoire vidéo contient au début de la scène sont identiques à celles de la\n";
    md += "  table, et les images correspondent aux captures du jeu (`DDSOM_REF=1`).\n\n";
    md += "## Limites\n\n";
    md += "* Pas de scroll 1 (cases de 8 pixels : texte, interface, quelques avant-plans), ni de sprites : décors\n";
    md += "  animés, objets et ennemis sont absents. Les dessins que le jeu écrit dynamiquement dans les couches\n";
    md += "  (scènes à script particulier) ne sont pas reconstitués.\n";
    md += "* Palette : celle du début de la scène, après l'ouverture. Les fondus, éclairs et changements de palette\n";
    md += "  en cours de scène ne sont pas rendus.\n";
    md += "* Parallaxe : quand une couche est plus étroite que l'autre (fond lointain), l'image est assemblée par\n";
    md += "  tranches d'un écran (384 pixels), chacune telle que le joueur la voit à cette position, avec le\n";
    md += "  rapport de défilement déduit des largeurs ; des raccords sont visibles entre tranches. Les deux couches\n";
    md += "  sont alignées par le bas ; les couches seules sont dans `couches/NN_scroll2.png` et `NN_scroll3.png`.\n";
    md += "* Une soixantaine de scènes n'ont pas de carte propre pour le scroll 3 (leur table est celle, par défaut, de\n";
    md += "  la scène 1) : cette couche est alors un reste de mémoire masqué par le scroll 2, et elle est omise.\n";
    md += "* Les écarts verticaux entre couches sont ceux des caméras au début de la scène ; si une caméra se déplace\n";
    md += "  autrement pendant la scène (scripts de mode), l'alignement relatif n'est exact qu'au départ.\n";
    md += "* Scènes 38, 65 et 68 : la phase de jeu n'a pas été atteinte par le saut direct (palette relevée au\n";
    md += "  mieux) ; leurs couleurs sont à vérifier.\n";
    md += "* Les bandes de remplissage des tables (`0xfc`, `0xfd`) ne sont pas dessinées (jamais à l'écran).\n";
    md += "* Noms de lieux : non extraits (les cartons de titre sont des graphismes, pas du texte). Les bosses ne sont\n";
    md += "  donnés que lorsque la documentation du dépôt les identifie.\n\n";
    md += "## Index\n\n";
    md += "![planche](../assets/niveaux/planche.png)\n\n";
    md += "| Scène | Image | Taille (px) | Scroll 2 | Scroll 3 | Ordre | Scène suivante | Boss |\n|---|---|---|---|---|---|---|---|\n";
    for &n in vues {
        let info = std::fs::read_to_string(dossier.join("infos").join(format!("{n:02}.tsv"))).unwrap_or_default();
        let c: Vec<&str> = info.trim().split('\t').collect();
        let get = |i: usize| c.get(i).copied().unwrap_or("?");
        let nom = if n >= 69 { format!("{n} (boutique)") } else { n.to_string() };
        md += &format!("| {nom} | [{n:02}.png](../assets/niveaux/{n:02}.png) | {}x{} | {} | {} | {} | {} | {} |\n", get(1), get(2), get(3), get(4), get(5), get(6), boss(n));
    }
    md += "\nOrdre : couches de l'arrière vers l'avant. « Scène suivante » : scène atteinte en forçant la sortie de\n";
    md += "scène (`-` : non relevée ; la sortie dépend parfois du jeu, voir `docs/scenes.md` §5).\n";
    std::fs::write(doc, md).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("cartes") if a.len() >= 5 => {
            let scenes: Vec<u32> = if a.len() > 5 { a[5..].iter().filter_map(|s| s.parse().ok()).collect() } else { SCENES.collect() };
            cartes(Path::new(&a[2]), Path::new(&a[3]), Path::new(&a[4]), &scenes);
        }
        Some("planche") if a.len() >= 4 => planche(Path::new(&a[2]), Path::new(&a[3])),
        Some("tables") if a.len() >= 3 => {
            let m = cps2::Machine::load_ddsom(Path::new(&a[2]), None).unwrap();
            let rom = Rom { data: &m.bus.rom, prog: &m.bus.rom_dec, gfx: &m.gfx };
            for n in 0..115 {
                let dim = |c: &Couche| {
                    let g = c.grille(&rom, n);
                    format!("{}x{} bandes", g.first().map_or(0, |l| l.len()), g.len())
                };
                println!("scène {n} : scroll 2 {}, scroll 3 {}", dim(&SCROLL2), dim(&SCROLL3));
            }
        }
        _ => eprintln!("usage : cartes cartes <roms> <eeprom> <dossier> [scène...] | cartes planche <dossier> <niveaux.md> | cartes tables <roms>"),
    }
}
