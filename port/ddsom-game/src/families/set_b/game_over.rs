use super::*;

const WAIT_FRAMES: u16 = 0x80;
/// Per-component fade-to-white task.
const FADE_TO_WHITE_TASK: u32 = 0x1ccf8;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 | 2 => {
            if x.a5_u8(0x42) == 0 {
                return;
            }
            // 0xa8ae8 (D0 = 7): scenery list 7
            crate::families::effects_creators::set_list_via(&mut OnBench(&mut x.c), x.b, 7);
            x.add8(0x4, 2);
            x.set16(0x4c, WAIT_FRAMES);
            crate::families::scenery::save_palette_page_1(x);
            // the original chains onto the countdown (0xd9458) from this very frame: 0x80 becomes 0x7f
            countdown(x);
        }
        4 => countdown(x),
        _ => x.free(&pools::SET),
    }
}

fn countdown<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u16(0x4c).wrapping_sub(1);
    x.set16(0x4c, n);
    if (n as i16) >= 0 {
        return;
    }
    x.set_a5_u16(0x490, 1);
    x.set_a5_u8(0x48e, 0xd);
    x.set_a5_u8(0x48f, 1);
    start_fade_task(x, FADE_TO_WHITE_TASK);
    x.free(&pools::SET);
}
