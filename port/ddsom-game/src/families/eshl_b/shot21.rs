use super::*;
use crate::data::RomAddr;
use crate::families::shell::vars;
use crate::families::shell::records::{EffectView, Host, view};
use crate::families::shell::tables::tables;

const ANIMS: RomAddr = RomAddr(0x16e752);
const LOOP: u8 = 0;
const END: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => active(x),
        _ => {
            crate::sound::play(x.b, crate::sound::CMD_37A, x.o);
            x.free_eshl()
        }
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::GAME_PAUSED.get(x.b) != 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    attach(x, &tables().shot21_attach);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, 0xb);
    x.set8(Shot21::TIMER, 0x4f);
    x.set8(Shot21::ENDED, 0);
    x.anim_start(ANIMS.0, 2);
    crate::sound::play(x.b, crate::sound::CMD_379, x.o);
}

fn burn<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Shot21::ENDED, 1);
    kill(x);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::GAME_PAUSED.get(x.b) != 0 {
        return;
    }
    if vars::SCRIPTED.get(x.b) != 0 {
        return burn(x);
    }
    let weapon = x.linked(Shot::OWNER);
    let host = link_of(x, weapon, Shot::OWNER);
    if host.u8(x.b, Host::FROZEN) != 0 || suspended(x) {
        return burn(x);
    }
    match x.u8(Base::SUB_STATE) {
        LOOP => {
            attach(x, &tables().shot21_attach);
            if !x.anim_step() {
                return x.reinsert();
            }
            x.add8(Shot21::TIMER, 0xff);
            if (x.u8(Shot21::TIMER) as i8) < 0 {
                let effect = x.linked(Shot21::EFFECT);
                effect.set_u8(x.b, EffectView::SIGNAL, 1);
                x.add8(Base::SUB_STATE, 2);
                x.anim_start(ANIMS.0, 1);
            } else {
                if x.u8(Shot21::EFFECT) == 0 {
                    if let Some(e) = x.alloc(&pools::EFFCT) {
                        e.set_u8(x.b, Obj::ACTIVE, 1);
                        e.set_u8(x.b, Obj::VARIANT, 0);
                        e.set_u8(x.b, Obj::KIND, 0x18);
                        e.set_u16(x.b, Shot::OWNER, low16(x.o));
                        x.set16(Shot21::EFFECT, low16(e));
                    }
                }
                x.anim_start(ANIMS.0, 0);
            }
        }
        END => {
            attach(x, &tables().shot21_attach);
            if x.anim_step() {
                burn(x);
            }
        }
        _ => {}
    }
    x.reinsert();
}

view! {
    /// Fields of shot 21.
    Shot21 {
        /// Set to 1 at its end (read by shot 20) (h).
        ended, set_ended, ENDED: u8 @ 0x1e;
        /// Countdown (h).
        timer, set_timer, TIMER: u8 @ 0x42;
        /// Effect object (16-bit pointer).
        effect, set_effect, EFFECT: u16 @ 0x46;
    }
}
