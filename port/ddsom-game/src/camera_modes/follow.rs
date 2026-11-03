//! Following the players and objects: the windows that keep the targets on screen (0x30fc8...).

use super::*;

pub(super) fn record(k: u32) -> u32 {
    A5 + PLAYER_BASE + k * PLAYER_STRIDE
}

/// Player `k` exists.
pub(super) fn present(x: &mut Cam, k: u32) -> bool {
    rd8(x.b, record(k)) != 0
}

/// Player `k` exists and has hit points.
pub(super) fn alive(x: &mut Cam, k: u32) -> bool {
    present(x, k) && rd16(x.b, record(k) + Player::HP) != 0
}

pub(super) fn any(x: &mut Cam, test: fn(&mut Cam, u32) -> bool) -> bool {
    (0..PLAYER_COUNT).any(|k| test(x, k))
}

/// Player active for horizontal following: in game, not yet exiting (main state < 4).
pub(super) fn playing(x: &mut Cam, k: u32) -> bool {
    present(x, k) && (rd8(x.b, record(k) + Player::MAIN) as i8) < 4
}

/// Player on the ground or acting: main state <= 2.
pub(super) fn grounded(x: &mut Cam, k: u32) -> bool {
    present(x, k) && (rd8(x.b, record(k) + Player::MAIN) as i8) <= 2
}

/// Largest and smallest (unsigned) value of a quantity of the retained players. Player 0,
/// if retained, sets both without comparison; without players, (0, 0xffff).
pub(super) fn extent(x: &mut Cam, value: impl Fn(&mut Cam, u32) -> Option<u16>) -> (u16, u16) {
    let (mut high, mut low) = (0u16, 0xffffu16);
    for k in 0..PLAYER_COUNT {
        if let Some(v) = value(x, k) {
            if k == 0 {
                (high, low) = (v, v);
            } else {
                high = high.max(v);
                low = low.min(v);
            }
        }
    }
    (high, low)
}

/// Following window of a camera: the target is kept between `far` (front side) and `near`/`back`.
#[derive(Clone, Copy)]
pub(super) struct Window {
    /// Distance from the camera to the most advanced player beyond which it advances.
    far: i16,
    /// Margin before following the least advanced player / moving back.
    near: i16,
    back: i16,
    /// Maximum step (6); beyond this limit the step is 6: the "jerky" variant of 0x30f6a
    /// tolerates 16 forwards (and then moves back by at most 16).
    limit: i16,
}

impl Window {
    const fn new(far: i16, near: i16, back: i16) -> Self {
        Window { far, near, back, limit: 6 }
    }
    const fn jerky(self) -> Self {
        Window { limit: 16, ..self }
    }
}

pub(super) const X_PLAYERS: Window = Window::new(0xf0, 0x20, 0x70);
pub(super) const X_PLAYERS_NARROW: Window = Window::new(0xf0, 0x5c, 0x5c);
pub(super) const X_OBJECTS: Window = Window::new(0x160, 0x1c, 0x1c);
pub(super) const X_OBJECT: Window = Window::new(0x80, 0x20, 0x20).jerky();
pub(super) const Y_PLAYERS: Window = Window::new(0x70, 0x30, 0x30);
pub(super) const Y_PLAYERS_LOW: Window = Window::new(0x90, 0x38, 0x38);
pub(super) const Y_OBJECT: Window = Window::new(0x88, 0x78, 0x78);

/// New camera position (`pos`) to keep within the window the targets whose most
/// advanced is `high` and least advanced `low`: advances by at most 6 pixels per frame when the most
/// advanced leaves the window (without losing the least advanced), moves back when the least advanced is too
/// far behind.
pub(super) fn follow(pos: i16, high: u16, low: u16, w: Window) -> i16 {
    let ahead = (high as i16).wrapping_sub(pos).wrapping_sub(w.far);
    let advance = |pos: i16, d: i16| pos.wrapping_add(if d > w.limit { 6 } else { d });
    if ahead > 0 {
        let near = (low as i16).wrapping_sub(pos).wrapping_sub(w.near);
        return if near <= 0 { pos } else { advance(pos, near.min(ahead)) };
    }
    let behind = (low as i16).wrapping_sub(pos).wrapping_sub(w.back);
    if behind >= 0 {
        return pos;
    }
    let pos = pos.wrapping_add(behind.max(-w.limit));
    let ahead = (high as i16).wrapping_sub(pos).wrapping_sub(w.far);
    if ahead > 0 {
        advance(pos, ahead)
    } else {
        pos
    }
}

/// Applies `follow` to the camera's field `field`.
pub(super) fn track(x: &mut Cam, field: u32, (high, low): (u16, u16), w: Window) {
    let pos = follow(x.u16(field) as i16, high, low, w);
    x.set16(field, pos as u16);
}

/// Abscissa of the players retained by `keep`.
pub(super) fn players_x(x: &mut Cam, keep: fn(&mut Cam, u32) -> bool) -> (u16, u16) {
    extent(x, |x, k| keep(x, k).then(|| rd16(x.b, record(k) + Obj::X)))
}

/// Depth of the players retained by `keep`.
pub(super) fn players_depth(x: &mut Cam, keep: fn(&mut Cam, u32) -> bool) -> (u16, u16) {
    extent(x, |x, k| keep(x, k).then(|| rd16(x.b, record(k) + Obj::Y)))
}

/// 0x30fc8: follows the players on the ground horizontally (nothing if there are none).
pub(super) fn follow_x(x: &mut Cam) {
    if any(x, grounded) {
        follow_x_playing(x);
    }
}

/// 0x31018: follows the players in game horizontally.
pub(super) fn follow_x_playing(x: &mut Cam) {
    let e = players_x(x, playing);
    track(x, cam::X, e, X_PLAYERS);
}

/// Negative abscissa x brought back to 0.
pub(super) fn keep_x_positive(x: &mut Cam) {
    if (x.u16(cam::X) as i16) < 0 {
        x.set16(cam::X, 0);
    }
}

/// 0x315be: follows the living players, horizontally (narrow window) then in depth.
pub(super) fn follow_xy(x: &mut Cam) {
    if any(x, alive) {
        let e = players_x(x, playing);
        track(x, cam::X, e, X_PLAYERS_NARROW);
        keep_x_positive(x);
        follow_depth(x);
    }
}

/// 0x31616: follows the depth of the living players.
pub(super) fn follow_depth(x: &mut Cam) {
    let e = players_depth(x, alive);
    track(x, cam::Y, e, Y_PLAYERS);
}

/// 0x3147c: follows the living players, depth with the "low" window.
pub(super) fn follow_snap(x: &mut Cam) {
    if any(x, alive) {
        follow_x_playing(x);
        keep_x_positive(x);
        let e = players_depth(x, alive);
        track(x, cam::Y, e, Y_PLAYERS_LOW);
    }
}

/// 0x31700: follows the middle of the living players, then the depth of all the players present.
pub(super) fn follow_mid(x: &mut Cam) {
    if any(x, alive) {
        let (high, low) = players_x(x, alive);
        let mid = high.wrapping_add(low) >> 1;
        track(x, cam::X, (mid, mid), X_PLAYERS);
        keep_x_positive(x);
        let e = players_depth(x, present);
        track(x, cam::Y, e, Y_PLAYERS);
    }
}

/// Four pointers (words) to the objects followed by the mode 0x0c camera (A5 - 0x555c).
pub(super) const TRACKED_OBJECTS: u32 = 0xff2aa4;

/// 0x31334: follows horizontally the objects designated by the four pointers.
pub(super) fn follow_objects(x: &mut Cam) {
    let pointers: Vec<u16> = (0..4).map(|k| rd16(x.b, TRACKED_OBJECTS + 2 * k)).collect();
    if pointers.iter().fold(0u16, |s, &p| s.wrapping_add(p)) == 0 {
        return;
    }
    let mut high_low = (0u16, 0xffffu16);
    for (k, &p) in pointers.iter().enumerate() {
        if p != 0 {
            let v = rd16(x.b, p as i16 as i32 as u32 + Obj::X);
            high_low = if k == 0 { (v, v) } else { (high_low.0.max(v), high_low.1.min(v)) };
        }
    }
    track(x, cam::X, high_low, X_OBJECTS);
}
