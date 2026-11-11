use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::view;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16fafa);
const FLY: u8 = 0;
const FADE: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    guarded(x, state)
}

fn state<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 0xc);
            x.set16(Shot35::TIMER, 0x5a);
            let variant = x.u8(Obj::VARIANT) as u32;
            let direction = tables().shot35_directions.at8(x.b, variant);
            x.set8(Obj::SPEED_INDEX, direction);
            speed_table(x, 0xb);
            ai_start(x, ANIMS.0, 0);
        }
        2 => {
            match x.u8(Base::SUB_STATE) {
                FLY => {
                    x.add16(Shot35::TIMER, 0xffff);
                    if (x.u16(Shot35::TIMER) as i16) <= 0 {
                        x.add8(Base::SUB_STATE, 2);
                        ai_start(x, ANIMS.0, 1);
                    } else {
                        ai_step(x);
                        if (x.u16(Shot35::TIMER) as i16) > 0x38 {
                            x.move_by_table_xy();
                        }
                    }
                }
                FADE => {
                    if ai_step(x) {
                        x.add8(Obj::STATE, 2);
                    } else {
                        x.move_by_table_xy();
                    }
                }
                _ => {}
            }
            blink_by_parity(x);
            x.push_terrain();
        }
        _ => x.free_eshl(),
    }
}

view! {
    /// Fields of shot 35.
    Shot35 {
        /// Countdown (h).
        timer, set_timer, TIMER: u16 @ 0x58;
    }
}
