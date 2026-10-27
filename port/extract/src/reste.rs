//! Le reste de la région programme : `data/reste.json` (`ddsom_assets::reste`, docs/sans-rom.md « Couverture
//! complète des données »).
//!
//! Tout octet de la région programme que les autres fichiers ne décrivent pas y est rangé, sauf le code 68000 :
//! les octets des instructions que la machine de référence exécute sur tous les scénarios (`code_68k.txt`, mesuré
//! par l'exemple `code_execute` de ddsom-game et réuni par `tools/code_68k.sh`). En cas de doute un octet va du côté
//! des données : le code jamais exécuté, les tables rangées dans le code et les octets qu'un appel saute sont donc
//! dans ce fichier. Les zones lues par les natives dans le code même (`natives.json`, mesurées) restent couvertes.
//! Doit être extrait après tous les autres fichiers (il les lit pour savoir ce qu'ils décrivent).

use std::io;
use std::path::Path;

use ddsom_assets::reste::{plages, Bloc, Reste};
use m68k::decode::{Ea, Fetch, Op};

const CODE: &str = include_str!("../code_68k.txt");

/// Plages d'octets exécutés `(début, fin exclue)`.
pub fn code() -> Vec<(u32, u32)> {
    CODE.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let t: Vec<u32> = l.split_whitespace().map(|v| u32::from_str_radix(v, 16).expect("code_68k.txt")).collect();
            (t[0], t[1])
        })
        .collect()
}

/// Espace programme vu par le décodeur.
struct Vue<'a>(&'a [u16]);

impl Fetch for Vue<'_> {
    fn fetch16(&mut self, a: u32) -> u16 {
        self.0.get((a as usize & 0xff_ffff) >> 1).copied().unwrap_or(0)
    }
}

/// Opérandes mémoire d'une instruction : (lus ou écrits comme données, seulement calculés par `lea`/`pea`).
/// Les sauts et appels ne comptent pas : leur cible est du code.
fn operandes(op: &Op) -> (Vec<Ea>, Vec<Ea>) {
    use Op::*;
    let lus = match *op {
        Add(_, a, b) | Addx(_, a, b) | Sub(_, a, b) | Subx(_, a, b) | Cmp(_, a, b) | And(_, a, b) | Or(_, a, b)
        | Eor(_, a, b) | Abcd(a, b) | Sbcd(a, b) | Move(_, a, b) => vec![a, b],
        Adda(_, a, _) | Suba(_, a, _) | Cmpa(_, a, _) | Neg(_, a) | Negx(_, a) | Not(_, a) | Clr(_, a) | Tst(_, a)
        | Tas(a) | Mulu(a, _) | Muls(a, _) | Divu(a, _) | Divs(a, _) | Nbcd(a) | Chk(a, _) | Movea(_, a, _)
        | MoveFromSr(a) | MoveToSr(a) | MoveToCcr(a) | Bit(_, _, a) | ShiftMem { ea: a, .. } | Scc(_, a)
        | Movem { ea: a, .. } => vec![a],
        _ => vec![],
    };
    let adresses = match *op {
        Lea(a, _) | Pea(a) => vec![a],
        _ => vec![],
    };
    (lus, adresses)
}

/// Adresse ROM fixe d'un opérande (relatif au PC ou absolu).
fn adresse(ea: Ea) -> Option<u32> {
    match ea {
        Ea::PcDisp(a) | Ea::PcIdx(a, _) | Ea::AbsL(a) => Some(a & 0xff_ffff),
        Ea::AbsW(a) => Some(a & 0xff_ffff),
        _ => None,
    }
}

/// Données rangées dans le code exécuté : une instruction exécutée lit (ou calcule par `lea`/`pea` hors d'un début
/// d'instruction) une adresse fixe qui tombe dans le code. Ces octets sont des données (par exemple une table indexée
/// qui commence dans le déplacement d'un `bra`) : la plage de code est rendue aux données de la cible à sa fin.
/// Renvoie les plages rendues.
fn donnees_dans_le_code(prog: &[u16], code: &[(u32, u32)], est_code: &[bool]) -> Vec<(u32, u32)> {
    let mut vue = Vue(prog);
    let mut debuts = vec![false; est_code.len()];
    let mut instructions = Vec::new();
    for &(d, f) in code {
        let mut pc = d;
        while pc < f {
            let ins = m68k::decode(&mut vue, pc);
            debuts[pc as usize] = true;
            instructions.push(ins);
            pc += ins.len;
        }
    }
    let mut rendues = Vec::new();
    for ins in &instructions {
        let (lus, adresses) = operandes(&ins.op);
        let cibles = lus.into_iter().filter_map(adresse).chain(
            adresses.into_iter().filter_map(adresse).filter(|&t| (t as usize) < debuts.len() && !debuts[t as usize]),
        );
        for t in cibles {
            if (t as usize) < est_code.len() && est_code[t as usize] {
                let &(_, fin) = code.iter().find(|&&(d, f)| (d..f).contains(&t)).unwrap();
                rendues.push((t, fin));
            }
        }
    }
    rendues
}

pub fn extract(roms: &Path, assets: &Path) -> io::Result<()> {
    let (data, prog) = cps2::load_program(roms)?;
    let octet = |v: &[u16], a: u32| v[(a / 2) as usize].to_be_bytes()[(a % 2) as usize];
    let taille = 2 * data.len() as u32;
    let key = std::fs::read(roms.join("ddsom.key"))?;
    let (_, debut, fin) = cps2::crypt::parse_key(&key);
    assert_eq!(debut, 0, "zone chiffrée inattendue");
    // le déchiffrement (comme MAME) inclut le mot de la borne haute
    let fin_chiffree = 2 * fin + 2;
    // hors de la zone chiffrée, l'espace programme égale l'espace données (la reconstruction le recopie)
    if let Some(w) = (fin as usize + 1..data.len()).find(|&w| data[w] != prog[w]) {
        panic!("espace programme différent de l'espace données hors de la zone chiffrée, en {:#x}", 2 * w);
    }
    let mut est_code = vec![false; taille as usize];
    for &(a, b) in &code() {
        est_code[a as usize..b as usize].fill(true);
    }
    let rendues = donnees_dans_le_code(&prog, &code(), &est_code);
    for &(a, b) in &rendues {
        est_code[a as usize..b as usize].fill(false);
    }
    let autres = ddsom_assets::rebuild::Program::load_files(assets, &|f| f != "data/reste.json")?;
    let blocs = |vue: &[u16], couvert: &dyn Fn(u32) -> bool, n: u32| -> Vec<Bloc> {
        plages(n, |a| !est_code[a as usize] && !couvert(a))
            .into_iter()
            .map(|(a, b)| Bloc::new(a, &(a..b).map(|x| octet(vue, x)).collect::<Vec<u8>>()))
            .collect()
    };
    let donnees = blocs(&data, &|a| autres.data.is_covered(a), taille);
    let programme = blocs(&prog, &|a| autres.prog.is_covered(a), fin_chiffree);
    let code = plages(taille, |a| est_code[a as usize]);
    let reste = Reste {
        description: "Région programme hors code 68000 et hors des autres fichiers (docs/sans-rom.md)".into(),
        fin_chiffree,
        octets_de_code: code.iter().map(|(a, b)| b - a).sum(),
        code: code.iter().map(|(a, b)| format!("{a:#x}-{b:#x}")).collect(),
        donnees,
        programme,
    };
    let path = assets.join("data/reste.json");
    reste.save(&path)?;
    let n = |v: &[Bloc]| v.iter().map(|b| b.bytes().unwrap().len()).sum::<usize>();
    println!(
        "{} : {} blocs, {} octets (données), {} blocs, {} octets (programme), code exclu {} octets en {} plages \
         ({} plages rendues aux données : cibles d'opérandes dans le code)",
        path.display(),
        reste.donnees.len(),
        n(&reste.donnees),
        reste.programme.len(),
        n(&reste.programme),
        reste.octets_de_code,
        reste.code.len(),
        rendues.len()
    );
    Ok(())
}
