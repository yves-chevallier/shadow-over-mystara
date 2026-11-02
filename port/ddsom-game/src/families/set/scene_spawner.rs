use super::*;

/// Table (program space) of release routines, indexed by `(class - 0xe) * 2` bytes.
const FREE_BY_CLASS: u32 = 0xcc2b8;
const WAIT: u8 = 0;
const SPAWN: u8 = 2;
const SCENE_CONTROLLER: u8 = 2;
/// Creator's parameter: variant of the controller to create.
const VARIANT_PARAM: u32 = 0x2f;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        WAIT => x.add8(0x4, 2),
        SPAWN => {
            let Some(child) = x.alloc(&pools::SET) else { return };
            child.set_u8(x.b, 0, 1);
            child.set_u8(x.b, 2, SCENE_CONTROLLER);
            let variant = x.u8(VARIANT_PARAM);
            child.set_u8(x.b, 3, variant);
            x.set8(0x4, 6);
        }
        _ => {
            let class = x.u8(0x28) as u32;
            let offset = (class.wrapping_sub(0xe) as u16).wrapping_mul(2) as i16;
            let at = FREE_BY_CLASS.wrapping_add(offset as i32 as u32);
            let free = (x.rom16(at) as u32) << 16 | x.rom16(at + 2) as u32;
            let pool = pools::POOLS
                .into_iter()
                .find(|p| p.free_routine == free)
                .unwrap_or_else(|| unreachable!("{free:#x}: not a pool release routine (class {class:#x})"));
            x.free(pool);
        }
    }
}
