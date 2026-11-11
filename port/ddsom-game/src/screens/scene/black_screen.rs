//! 0xc32: black screen in three frames (subroutine that cedes, `0xb64`, before and after the clearing): a
//! sub-machine ([`Step`]) that the Rust steps call directly (`Sequence::subroutine`); nothing
//! calls it by address any more.

use crate::screens::modes::Flow;
use crate::screens::scene::Ctx;
use crate::core::tasks::one_frame;

/// Layers off, then one frame.
pub fn start(x: &mut Ctx) -> Flow {
    crate::services::video::switch_layers_off(x.b);
    one_frame(x, Step::Clear)
}

crate::steps! {
    /// Steps of the black screen.
    pub enum Step {
        /// Objects and layers cleared, then one frame.
        Clear = 0xc42 => |x| {
            crate::services::video::clear_screen(x.b);
            one_frame(x, Step::End)
        },
        /// Return to the caller.
        End = 0xc56 => |_| Flow::Done,
    }
}
