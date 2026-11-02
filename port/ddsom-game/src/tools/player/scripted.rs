//! Tools: the patch of `crate::player::scripted` for the reference machine.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::player::scripted::*;
use crate::ram;

/// Same fix in the original code (reference machine of the validation): at the entry of the dispatch.
pub fn patch_end_by_scene(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let scene = ram::SCENE.get(b) as u32;
    if scene <= LAST_END_SCENE {
        return false;
    }
    c.d[0] = scene << 2 & 0xffff;
    c.a[1] = NO_END;
    c.pc = NO_END;
    true
}
