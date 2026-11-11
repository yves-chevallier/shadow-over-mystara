//! The displayed frame: the image of the CPS2 renderer (`cps2_gfx::render`, 384 x 224) uploaded as a texture,
//! drawn centred at a variable scale, through an optional filter. Without filter (the default), the texture is
//! sampled at the nearest texel: at a whole scale `s`, each pixel of the frame is an `s` x `s` square of its exact
//! colour, so at scale 1 the window shows the CPS2 image itself (`DDSOM_IDENTITY` checks it by reading the window
//! back, `main.rs`). A future source of frames (HD pack, L22) only has to fill another texture.

use cps2_gfx::{SCREEN_H, SCREEN_W};
use macroquad::prelude::*;

/// How the frame fills the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scale {
    /// The largest whole multiple that fits (pixels stay square and equal).
    Whole,
    /// The largest size that fits, keeping the proportions.
    Fit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    None,
    /// Bilinear interpolation of the texels.
    Smooth,
    /// Scanlines (`CRT_FRAGMENT`).
    Crt,
}

pub struct Screen {
    texture: Texture2D,
    rgba: Vec<u8>,
    pub scale: Scale,
    pub filter: Filter,
    crt: Option<Material>,
}

impl Screen {
    pub fn new() -> Self {
        let texture = Texture2D::from_rgba8(SCREEN_W as u16, SCREEN_H as u16, &vec![0; SCREEN_W * SCREEN_H * 4]);
        texture.set_filter(FilterMode::Nearest);
        let crt = load_material(
            ShaderSource::Glsl { vertex: VERTEX, fragment: CRT_FRAGMENT },
            MaterialParams::default(),
        )
        .map_err(|e| eprintln!("CRT filter unavailable: {e}"))
        .ok();
        Screen { texture, rgba: vec![0; SCREEN_W * SCREEN_H * 4], scale: Scale::Whole, filter: Filter::None, crt }
    }

    /// Shows the frame `image` (rows of RGB pixels, as `cps2_gfx::render` gives them) from now on.
    pub fn show(&mut self, image: &[[u8; 3]]) {
        for (d, p) in self.rgba.as_chunks_mut::<4>().0.iter_mut().zip(image) {
            *d = [p[0], p[1], p[2], 255];
        }
        self.texture.update_from_bytes(SCREEN_W as u32, SCREEN_H as u32, &self.rgba);
    }

    pub fn next_scale(&mut self) {
        self.scale = match self.scale {
            Scale::Whole => Scale::Fit,
            Scale::Fit => Scale::Whole,
        };
    }

    pub fn next_filter(&mut self) {
        self.filter = match self.filter {
            Filter::None => Filter::Smooth,
            Filter::Smooth if self.crt.is_some() => Filter::Crt,
            _ => Filter::None,
        };
        self.texture.set_filter(if self.filter == Filter::Smooth { FilterMode::Linear } else { FilterMode::Nearest });
    }

    /// Where the frame goes in the window: top left corner and scale (whole pixels for the corner).
    pub fn placement(&self) -> (f32, f32, f32) {
        let (w, h) = (screen_width(), screen_height());
        let fit = (w / SCREEN_W as f32).min(h / SCREEN_H as f32);
        let s = match self.scale {
            Scale::Whole => fit.floor().max(1.0),
            Scale::Fit => fit,
        };
        (((w - SCREEN_W as f32 * s) / 2.0).floor(), ((h - SCREEN_H as f32 * s) / 2.0).floor(), s)
    }

    /// Draws the frame on a black window.
    pub fn draw(&self) {
        clear_background(BLACK);
        let (x, y, s) = self.placement();
        if let (Filter::Crt, Some(m)) = (self.filter, &self.crt) {
            gl_use_material(m);
        }
        let size = vec2(SCREEN_W as f32 * s, SCREEN_H as f32 * s);
        draw_texture_ex(&self.texture, x, y, WHITE, DrawTextureParams { dest_size: Some(size), ..Default::default() });
        gl_use_default_material();
    }
}

const VERTEX: &str = "#version 100
attribute vec3 position;
attribute vec2 texcoord;
uniform mat4 Model;
uniform mat4 Projection;
varying highp vec2 uv;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    uv = texcoord;
}
";

/// Scanlines: the lower half of each line of the frame darkens towards its bottom edge, like the gap between the
/// lines of a CRT (from scale 2: one window row in two). At scale 1 only the middle of each line is sampled: the image
/// is unchanged.
const CRT_FRAGMENT: &str = "#version 100
precision highp float;
varying vec2 uv;
uniform sampler2D Texture;
void main() {
    vec3 colour = texture2D(Texture, uv).rgb;
    float line = fract(uv.y * 224.0);
    float beam = 1.0 - 0.5 * smoothstep(0.5, 1.0, line);
    gl_FragColor = vec4(colour * beam, 1.0);
}
";
