use super::*;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::actors::families::set_b::fields::GameOverObj;

const WAIT_FRAMES: u16 = 0x80;
/// Per-component fade-to-white task.
const FADE_TO_WHITE_TASK: crate::core::tasks::Task = crate::core::tasks::Task::Fading(crate::core::tasks::fading::Fade::ToWhite);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 | 2 => {
            if x.a5_u8(vars::FROZEN) == 0 {
                return;
            }
            // 0xa8ae8 (D0 = 7): scenery list 7
            crate::actors::families::effects_creators::set_list_via(&mut OnBench(&mut x.c), x.b, 7);
            x.add8(Obj::STATE, 2);
            x.set16(GameOverObj::WAIT, WAIT_FRAMES);
            crate::actors::families::scenery::save_palette_page_1(x);
            // the original chains onto the countdown (0xd9458) from this very frame: 0x80 becomes 0x7f
            countdown(x);
        }
        4 => countdown(x),
        _ => x.free(&pools::SET),
    }
}

fn countdown<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u16(GameOverObj::WAIT).wrapping_sub(1);
    x.set16(GameOverObj::WAIT, n);
    if (n as i16) >= 0 {
        return;
    }
    x.set_a5_u16(vars::FADE_DELAY, 1);
    x.set_a5_u8(vars::FADE_PAGES, 0xd);
    x.set_a5_u8(vars::FADE_STEPS, 1);
    start_fade_task(x, FADE_TO_WHITE_TASK);
    x.free(&pools::SET);
}
