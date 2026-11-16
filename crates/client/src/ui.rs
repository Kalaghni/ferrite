//! Look of the launcher and lobby: the palette, typeface and key art of
//! the FERRITE UI kit ("The Frontline"). Everything here is built into the
//! executable.

use macroquad::prelude::*;
use std::cell::RefCell;

/// Palette of the UI kit (tokens/ferrite_theme.rs).
#[allow(dead_code)]
pub mod color {
    use macroquad::prelude::Color;
    pub const INK: Color = Color::new(0.047, 0.063, 0.125, 1.0);
    pub const NIGHT: Color = Color::new(0.082, 0.094, 0.165, 1.0);
    pub const PANEL: Color = Color::new(0.106, 0.125, 0.208, 1.0);
    pub const PANEL_RAISED: Color = Color::new(0.145, 0.173, 0.263, 1.0);
    pub const EMBER: Color = Color::new(1.0, 0.459, 0.298, 1.0);
    pub const EMBER_BRIGHT: Color = Color::new(1.0, 0.682, 0.475, 1.0);
    pub const EMBER_DIM: Color = Color::new(0.553, 0.298, 0.302, 1.0);
    pub const COBALT: Color = Color::new(0.412, 0.678, 1.0, 1.0);
    pub const COBALT_BRIGHT: Color = Color::new(0.718, 0.863, 1.0, 1.0);
    pub const COBALT_DIM: Color = Color::new(0.216, 0.357, 0.569, 1.0);
    pub const WHITE: Color = Color::new(0.965, 0.953, 0.98, 1.0);
    pub const MUTED: Color = Color::new(0.71, 0.714, 0.808, 1.0);
    pub const QUIET: Color = Color::new(0.506, 0.537, 0.647, 1.0);
    pub const SUCCESS: Color = Color::new(0.506, 0.878, 0.729, 1.0);
    pub const WARNING: Color = Color::new(1.0, 0.824, 0.541, 1.0);
    pub const DANGER: Color = Color::new(1.0, 0.408, 0.486, 1.0);
    pub const LINE: Color = Color::new(0.471, 0.506, 0.659, 1.0);
    pub const GLASS: Color = Color::new(0.047, 0.063, 0.125, 0.78);
    pub const GLASS_STRONG: Color = Color::new(0.047, 0.063, 0.125, 0.88);
    pub const STROKE: Color = Color::new(0.706, 0.761, 0.882, 0.38);
}

pub fn fade(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, c.a * a)
}

/// Reference size of the key art; layouts anchor to it.
pub const ART_W: f32 = 1672.0;
pub const ART_H: f32 = 941.0;

static LAUNCHER: &[u8] = include_bytes!("../assets/launcher-background.png");
static LOBBY: &[u8] = include_bytes!("../assets/lobby-background.png");
static WORDMARK: &[u8] = include_bytes!("../assets/ferrite-wordmark.png");
static EMBLEMS: [&[u8]; 2] = [include_bytes!("../assets/emblem-ember.png"), include_bytes!("../assets/emblem-cobalt.png")];
static REGULAR: &[u8] = include_bytes!("../assets/Quantico-Regular.ttf");
static BOLD: &[u8] = include_bytes!("../assets/Quantico-Bold.ttf");

thread_local! {
    static FONTS: RefCell<Option<(Font, Font)>> = const { RefCell::new(None) };
}

/// Quantico reads larger than the font the layouts were sized for.
const FONT_K: f32 = 0.8;

#[derive(Clone)]
pub struct Art {
    pub launcher: Texture2D,
    pub lobby: Texture2D,
    /// The FERRITE wordmark (1634 x 316) and the two team emblems.
    pub wordmark: Texture2D,
    pub emblems: [Texture2D; 2],
}

/// Decode the fonts and key art. Call once, after the window is up.
pub fn init() -> Art {
    let font = |b: &[u8]| load_ttf_font_from_bytes(b).ok();
    if let (Some(r), Some(b)) = (font(REGULAR), font(BOLD)) {
        FONTS.with(|f| *f.borrow_mut() = Some((r, b)));
    }
    let tex = |b: &[u8]| {
        let t = Texture2D::from_file_with_format(b, Some(ImageFormat::Png));
        t.set_filter(FilterMode::Linear);
        t
    };
    Art { launcher: tex(LAUNCHER), lobby: tex(LOBBY), wordmark: tex(WORDMARK), emblems: [tex(EMBLEMS[0]), tex(EMBLEMS[1])] }
}

/// Headings are set in the bold cut.
fn bold(size: f32) -> bool {
    size >= 20.0
}

pub fn text(s: &str, x: f32, y: f32, size: f32, c: Color) {
    FONTS.with(|f| match &*f.borrow() {
        Some((r, b)) => {
            let font = if bold(size) { b } else { r };
            draw_text_ex(
                s,
                x,
                y,
                TextParams { font: Some(font), font_size: (size * FONT_K).round() as u16, color: c, ..Default::default() },
            );
        }
        None => {
            draw_text(s, x, y, size, c);
        }
    });
}

pub fn width(s: &str, size: f32) -> f32 {
    FONTS.with(|f| match &*f.borrow() {
        Some((r, b)) => measure_text(s, Some(if bold(size) { b } else { r }), (size * FONT_K).round() as u16, 1.0).width,
        None => measure_text(s, None, size as u16, 1.0).width,
    })
}

pub fn text_c(s: &str, cx: f32, y: f32, size: f32, c: Color) {
    text(s, cx - width(s, size) / 2.0, y, size, c);
}

pub fn text_r(s: &str, rx: f32, y: f32, size: f32, c: Color) {
    text(s, rx - width(s, size), y, size, c);
}

/// Scale at which the key art covers a `sw` x `sh` screen.
pub fn art_scale(sw: f32, sh: f32) -> f32 {
    (sw / ART_W).max(sh / ART_H)
}

/// Fill the screen with a piece of key art. It is pinned to the top left,
/// where the wordmark goes, and cropped on the right and bottom.
pub fn backdrop(t: &Texture2D, sw: f32, sh: f32) {
    let k = art_scale(sw, sh);
    draw_texture_ex(t, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(ART_W * k, ART_H * k)), ..Default::default() });
}

/// The FERRITE wordmark over the key art (the art itself carries none):
/// `x`, `y` and `width` in key-art pixels, scaled with the art.
pub fn logo(art: &Art, sw: f32, sh: f32, x: f32, y: f32, width: f32) {
    let k = art_scale(sw, sh);
    let size = vec2(width, width * 316.0 / 1634.0) * k;
    draw_texture_ex(&art.wordmark, x * k, y * k, WHITE, DrawTextureParams { dest_size: Some(size), ..Default::default() });
}

/// Where the launcher and the lobby put the wordmark (key-art pixels).
pub const LAUNCHER_LOGO: (f32, f32, f32) = (46.0, 64.0, 620.0);
pub const LOBBY_LOGO: (f32, f32, f32) = (30.0, 30.0, 440.0);

/// A glass plate with a hairline border.
pub fn plate(x: f32, y: f32, w: f32, h: f32, fill: Color, stroke: Color) {
    draw_rectangle(x, y, w, h, fill);
    draw_rectangle_lines(x, y, w, h, 2.0, stroke);
}
