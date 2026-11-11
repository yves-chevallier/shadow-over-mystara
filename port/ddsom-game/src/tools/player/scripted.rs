//! Tools: the patch of `crate::actors::player::scripted` for the reference machine.

use m68k::Cpu;

use crate::actors::player::scripted::*;
use crate::world::ram;

/// Same fix in the original code (reference machine of the validation): at the entry of the dispatch.
pub(crate) fn patch_end_by_scene(c: &mut Cpu, b: &mut cps2::Cps2Bus) -> bool {
    let scene = ram::SCENE.get(b) as u32;
    if scene <= LAST_END_SCENE {
        return false;
    }
    c.d[0] = scene << 2 & 0xffff;
    c.a[1] = NO_END.0;
    c.pc = NO_END.0;
    true
}
