//! 0xc32: black screen in three frames (subroutine that cedes, `0xb64`, before and after the clearing): a
//! sub-machine ([`Step`]) that the Rust steps call directly (`Sequence::subroutine`); nothing
//! calls it by address any more.

use crate::world::tasks::Flow;
use crate::world::tasks::Ctx;
use crate::world::tasks::one_frame;
pub use crate::world::tasks::stages::BlackScreen as Step;

/// Layers off, then one frame.
pub fn start(x: &mut Ctx) -> Flow {
    crate::services::video::switch_layers_off(x.b);
    one_frame(x, Step::Clear)
}

crate::steps! {
    impl Step {
        Clear = 0xc42 => |x| {
            crate::services::video::clear_screen(x.b);
            one_frame(x, Step::End)
        },
        End = 0xc56 => |_| Flow::Done,
    }
}
