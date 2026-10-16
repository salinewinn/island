//! Key bindings, settings and the immediate-mode UI toolkit.

use super::*;
use std::path::PathBuf;
use crate::people::Look;
// ---------------------------------------------------------------------------
// Key bindings
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq)]
pub enum Bind {
    Key(KeyCode),
    Mouse(MouseButton),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Sprint,
    SwimDown,
    Use,
    Interact,
    Drink,
    Drop,
    Slot1,
    Slot2,
    Slot3,
    Slot4,
    Slot5,
}

pub const ACTS: [Act; 16] = [
    Act::Forward,
    Act::Back,
    Act::Left,
    Act::Right,
    Act::Jump,
    Act::Sprint,
    Act::SwimDown,
    Act::Use,
    Act::Interact,
    Act::Drink,
    Act::Drop,
    Act::Slot1,
    Act::Slot2,
    Act::Slot3,
    Act::Slot4,
    Act::Slot5,
];

pub fn act_label(a: Act) -> &'static str {
    match a {
        Act::Forward => "Move forward",
        Act::Back => "Move back",
        Act::Left => "Move left",
        Act::Right => "Move right",
        Act::Jump => "Jump / swim up",
        Act::Sprint => "Sprint",
        Act::SwimDown => "Swim down",
        Act::Use => "Use item (cast / throw)",
        Act::Interact => "Interact / pick up",
        Act::Drink => "Drink coconut",
        Act::Drop => "Drop coconut",
        Act::Slot1 => "Hotbar 1 - Fishing rod",
        Act::Slot2 => "Hotbar 2 - Coconut",
        Act::Slot3 => "Hotbar 3 - Lantern",
        Act::Slot4 => "Hotbar 4 - Bait",
        Act::Slot5 => "Hotbar 5 - Fish bucket",
    }
}

pub fn act_id(a: Act) -> &'static str {
    match a {
        Act::Forward => "Forward",
        Act::Back => "Back",
        Act::Left => "Left",
        Act::Right => "Right",
        Act::Jump => "Jump",
        Act::Sprint => "Sprint",
        Act::SwimDown => "SwimDown",
        Act::Use => "Use",
        Act::Interact => "Interact",
        Act::Drink => "Drink",
        Act::Drop => "Drop",
        Act::Slot1 => "Slot1",
        Act::Slot2 => "Slot2",
        Act::Slot3 => "Slot3",
        Act::Slot4 => "Slot4",
        Act::Slot5 => "Slot5",
    }
}

pub fn default_bind(a: Act) -> Bind {
    match a {
        Act::Forward => Bind::Key(KeyCode::W),
        Act::Back => Bind::Key(KeyCode::S),
        Act::Left => Bind::Key(KeyCode::A),
        Act::Right => Bind::Key(KeyCode::D),
        Act::Jump => Bind::Key(KeyCode::Space),
        Act::Sprint => Bind::Key(KeyCode::LeftShift),
        Act::SwimDown => Bind::Key(KeyCode::C),
        Act::Use => Bind::Mouse(MouseButton::Left),
        Act::Interact => Bind::Key(KeyCode::E),
        Act::Drink => Bind::Key(KeyCode::F),
        Act::Drop => Bind::Key(KeyCode::G),
        Act::Slot1 => Bind::Key(KeyCode::Key1),
        Act::Slot2 => Bind::Key(KeyCode::Key2),
        Act::Slot3 => Bind::Key(KeyCode::Key3),
        Act::Slot4 => Bind::Key(KeyCode::Key4),
        Act::Slot5 => Bind::Key(KeyCode::Key5),
    }
}

macro_rules! keys {
    ($($n:ident),* $(,)?) => { &[$((stringify!($n), KeyCode::$n)),*] };
}

pub const KEYS: &[(&str, KeyCode)] = keys!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7,
    Key8, Key9, Space, LeftShift, RightShift, LeftControl, RightControl, LeftAlt, RightAlt, Tab, Enter, Up, Down, Left, Right, F1, F2,
    F3, F4, F5, F6, F7, F8, F9, F10, Minus, Equal, Comma, Period, Slash, Semicolon, Apostrophe, LeftBracket, RightBracket,
    Backslash, GraveAccent, CapsLock, Insert, Delete, Home, End, PageUp, PageDown
);

pub fn key_label(name: &str) -> String {
    match name {
        "LeftShift" => "L-Shift".into(),
        "RightShift" => "R-Shift".into(),
        "LeftControl" => "L-Ctrl".into(),
        "RightControl" => "R-Ctrl".into(),
        "LeftAlt" => "L-Alt".into(),
        "RightAlt" => "R-Alt".into(),
        "GraveAccent" => "`".into(),
        "LeftBracket" => "[".into(),
        "RightBracket" => "]".into(),
        "Semicolon" => ";".into(),
        "Apostrophe" => "'".into(),
        "Backslash" => "\\".into(),
        "Minus" => "-".into(),
        "Equal" => "=".into(),
        "Comma" => ",".into(),
        "Period" => ".".into(),
        "Slash" => "/".into(),
        n if n.starts_with("Key") && n.len() == 4 => n[3..].to_string(),
        n => n.to_string(),
    }
}

pub fn bind_label(b: Bind) -> String {
    match b {
        Bind::Key(k) => KEYS.iter().find(|(_, kk)| *kk == k).map(|(n, _)| key_label(n)).unwrap_or_else(|| "?".into()),
        Bind::Mouse(MouseButton::Left) => "Left click".into(),
        Bind::Mouse(MouseButton::Right) => "Right click".into(),
        Bind::Mouse(MouseButton::Middle) => "Middle click".into(),
        Bind::Mouse(_) => "Mouse".into(),
    }
}

pub fn bind_to_str(b: Bind) -> String {
    match b {
        Bind::Key(k) => format!("K:{}", KEYS.iter().find(|(_, kk)| *kk == k).map(|(n, _)| *n).unwrap_or("Space")),
        Bind::Mouse(MouseButton::Right) => "M:Right".into(),
        Bind::Mouse(MouseButton::Middle) => "M:Middle".into(),
        Bind::Mouse(_) => "M:Left".into(),
    }
}

pub fn bind_from_str(s: &str) -> Option<Bind> {
    if let Some(n) = s.strip_prefix("K:") {
        return KEYS.iter().find(|(name, _)| *name == n).map(|(_, k)| Bind::Key(*k));
    }
    match s {
        "M:Left" => Some(Bind::Mouse(MouseButton::Left)),
        "M:Right" => Some(Bind::Mouse(MouseButton::Right)),
        "M:Middle" => Some(Bind::Mouse(MouseButton::Middle)),
        _ => None,
    }
}

pub fn b_down(b: Bind) -> bool {
    match b {
        Bind::Key(k) => is_key_down(k),
        Bind::Mouse(m) => is_mouse_button_down(m),
    }
}
pub fn b_pressed(b: Bind) -> bool {
    match b {
        Bind::Key(k) => is_key_pressed(k),
        Bind::Mouse(m) => is_mouse_button_pressed(m),
    }
}
pub fn b_released(b: Bind) -> bool {
    match b {
        Bind::Key(k) => is_key_released(k),
        Bind::Mouse(m) => is_mouse_button_released(m),
    }
}

// ---------------------------------------------------------------------------
// Settings (saved next to the exe)
// ---------------------------------------------------------------------------
pub const RESOLUTIONS: [(u32, u32); 6] = [(1280, 720), (1366, 768), (1600, 900), (1920, 1080), (2560, 1440), (3840, 2160)];
pub const FPS_OPTS: [u32; 6] = [30, 60, 120, 144, 240, 0];
pub const PRESETS: [&str; 4] = ["Low", "Medium", "High", "Ultra"];

/// Multiplier applied to model detail (counts / segments) for a detail level 0..3.
pub fn detail_mult(level: usize) -> f32 {
    [0.7, 1.0, 1.4, 1.9][level.min(3)]
}

/// True when `s` decodes to a Look whose every field is inside its option range.
fn look_valid(s: &str) -> bool {
    match Look::decode(s) {
        Some(l) => (0..crate::people::field_count()).all(|i| crate::people::field_get(&l, i) < crate::people::field_options(i, &l).len().max(1)),
        None => false,
    }
}

#[derive(Clone)]
pub struct Settings {
    pub res: usize,
    pub fullscreen: bool,
    pub fps: usize,
    pub preset: usize,
    pub draw_dist: f32,
    pub water_q: usize,
    pub bump: bool,
    pub show_fps: bool,
    pub show_clock: bool,
    pub show_hints: bool,
    pub sens: f32,
    pub invert_y: bool,
    pub fov: f32,
    /// master sound volume 0..1
    pub volume: f32,
    pub binds: Vec<Bind>,
    /// model detail level 0..3 (Low, Medium, High, Ultra); applies when the world is built
    pub detail: usize,
    /// encoded character look (see people::Look::encode)
    pub look: String,
    /// check GitHub for a newer release when the game starts
    pub auto_update: bool,
    /// sync to the monitor's refresh (smooth, and much better for screen sharing); needs a restart
    pub vsync: bool,
    /// "owner/name" of the GitHub repository that publishes releases
    pub update_repo: String,
    /// the player's chosen name (empty = "Player N")
    pub name: String,
    /// which save slot (0..4) was used last
    pub slot: usize,
    /// found-footage look: film grain, scan lines and a rolling tracking line
    pub camcorder: bool,
}

impl Settings {
    pub fn defaults() -> Self {
        let mut s = Settings {
            res: 0,
            fullscreen: false,
            fps: 1,
            preset: 2,
            draw_dist: 380.0,
            water_q: 2,
            bump: true,
            show_fps: false,
            show_clock: true,
            show_hints: false,
            sens: 1.0,
            invert_y: false,
            fov: 70.0,
            volume: 0.7,
            binds: ACTS.iter().map(|a| default_bind(*a)).collect(),
            detail: 2,
            look: Look::default_for(0).encode(),
            auto_update: true,
            vsync: true,
            update_repo: crate::update::DEFAULT_REPO.to_string(),
            name: String::new(),
            slot: 0,
            camcorder: false,
        };
        s.apply_preset(2);
        s
    }

    pub fn apply_preset(&mut self, p: usize) {
        self.preset = p;
        self.detail = p.min(3);
        match p {
            0 => {
                self.draw_dist = 120.0;
                self.water_q = 0;
                self.bump = false;
            }
            1 => {
                self.draw_dist = 220.0;
                self.water_q = 1;
                self.bump = false;
            }
            2 => {
                self.draw_dist = 380.0;
                self.water_q = 2;
                self.bump = true;
            }
            _ => {
                self.draw_dist = 650.0;
                self.water_q = 2;
                self.bump = true;
            }
        }
    }

    pub fn path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("island_settings.txt")))
            .unwrap_or_else(|| PathBuf::from("island_settings.txt"))
    }

    pub fn load() -> Self {
        let mut s = Self::defaults();
        if let Ok(text) = std::fs::read_to_string(Self::path()) {
            for line in text.lines() {
                let Some((k, v)) = line.split_once('=') else { continue };
                let (k, v) = (k.trim(), v.trim());
                let fl = || v.parse::<f32>().ok();
                let bo = || v == "1" || v == "true";
                match k {
                    "res" => s.res = v.parse::<usize>().unwrap_or(0).min(RESOLUTIONS.len() - 1),
                    "fullscreen" => s.fullscreen = bo(),
                    "fps" => s.fps = v.parse::<usize>().unwrap_or(1).min(FPS_OPTS.len() - 1),
                    "preset" => s.preset = v.parse::<usize>().unwrap_or(2).min(4),
                    "draw_dist" => s.draw_dist = fl().unwrap_or(380.0).clamp(60.0, 900.0),
                    "water_q" => s.water_q = v.parse::<usize>().unwrap_or(2).min(2),
                    "bump" => s.bump = bo(),
                    "show_fps" => s.show_fps = bo(),
                    "show_clock" => s.show_clock = bo(),
                    "show_hints" => s.show_hints = bo(),
                    "sens" => s.sens = fl().unwrap_or(1.0).clamp(0.2, 3.0),
                    "invert_y" => s.invert_y = bo(),
                    "fov" => s.fov = fl().unwrap_or(70.0).clamp(55.0, 110.0),
                    "volume" => s.volume = fl().unwrap_or(0.7).clamp(0.0, 1.0),
                    "detail" => s.detail = v.parse::<usize>().unwrap_or(2).min(3),
                    "auto_update" => s.auto_update = bo(),
                    "vsync" => s.vsync = bo(),
                    "camcorder" => s.camcorder = bo(),
                    "name" => s.name = crate::net::clean_name(v),
                    "slot" => s.slot = v.parse::<usize>().unwrap_or(0).min(crate::game::SLOTS - 1),
                    "update_repo" => {
                        if v.contains('/') && v.len() < 100 && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.') {
                            s.update_repo = v.to_string();
                        }
                    }
                    "look" => {
                        if look_valid(v) {
                            s.look = v.to_string();
                        }
                    }
                    _ => {
                        if let Some(id) = k.strip_prefix("bind.") {
                            if let (Some(i), Some(b)) = (ACTS.iter().position(|a| act_id(*a) == id), bind_from_str(v)) {
                                s.binds[i] = b;
                            }
                        }
                    }
                }
            }
        }
        s
    }

    pub fn save(&self) {
        let mut t = String::new();
        let b = |x: bool| if x { "1" } else { "0" };
        t += &format!("res={}\nfullscreen={}\nfps={}\npreset={}\ndraw_dist={}\nwater_q={}\nbump={}\n", self.res, b(self.fullscreen), self.fps, self.preset, self.draw_dist, self.water_q, b(self.bump));
        t += &format!("show_fps={}\nshow_clock={}\nshow_hints={}\nsens={}\ninvert_y={}\nfov={}\n", b(self.show_fps), b(self.show_clock), b(self.show_hints), self.sens, b(self.invert_y), self.fov);
        t += &format!("detail={}\nlook={}\nvolume={}\n", self.detail.min(3), self.look, self.volume);
        t += &format!("auto_update={}\nupdate_repo={}\nvsync={}\nname={}\nslot={}\ncamcorder={}\n", b(self.auto_update), self.update_repo, b(self.vsync), self.name, self.slot, b(self.camcorder));
        for (i, a) in ACTS.iter().enumerate() {
            t += &format!("bind.{}={}\n", act_id(*a), bind_to_str(self.binds[i]));
        }
        let _ = std::fs::write(Self::path(), t);
    }

    /// The repository to check for updates.
    pub fn repo(&self) -> String {
        if self.update_repo.trim().is_empty() { crate::update::DEFAULT_REPO.to_string() } else { self.update_repo.clone() }
    }

    pub fn bind(&self, a: Act) -> Bind {
        self.binds[a as usize]
    }
    pub fn down(&self, a: Act) -> bool {
        b_down(self.bind(a))
    }
    pub fn pressed(&self, a: Act) -> bool {
        b_pressed(self.bind(a))
    }
    pub fn released(&self, a: Act) -> bool {
        b_released(self.bind(a))
    }
}

// ---------------------------------------------------------------------------
// Tiny immediate-mode UI (designed for 1280x720, scaled to fit)
// ---------------------------------------------------------------------------
pub struct Ui {
    pub scale: f32,
    pub ox: f32,
    pub oy: f32,
    pub mx: f32,
    pub my: f32,
    pub clicked: bool,
    pub down: bool,
    pub drag: Option<usize>,
}

pub const COL_PANEL: Color = Color::new(0.07, 0.09, 0.12, 0.94);
pub const COL_BTN: Color = Color::new(0.16, 0.20, 0.27, 1.0);
pub const COL_BTN_HOT: Color = Color::new(0.24, 0.32, 0.43, 1.0);
pub const COL_ACCENT: Color = Color::new(0.25, 0.65, 0.55, 1.0);

impl Ui {
    pub fn new() -> Self {
        Ui { scale: 1.0, ox: 0.0, oy: 0.0, mx: 0.0, my: 0.0, clicked: false, down: false, drag: None }
    }
    pub fn begin(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        self.scale = (w / 1280.0).min(h / 720.0);
        self.ox = (w - 1280.0 * self.scale) * 0.5;
        self.oy = (h - 720.0 * self.scale) * 0.5;
        let (mx, my) = mouse_position();
        self.mx = (mx - self.ox) / self.scale;
        self.my = (my - self.oy) / self.scale;
        self.clicked = is_mouse_button_pressed(MouseButton::Left);
        self.down = is_mouse_button_down(MouseButton::Left);
    }
    pub fn hover(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mx >= x && self.mx <= x + w && self.my >= y && self.my <= y + h
    }
    /// corner radius (screen pixels) for a box of this size: soft, never more than a pill
    fn radius(&self, w: f32, h: f32) -> f32 {
        (w.min(h) * 0.22).min(16.0) * self.scale
    }
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        let (sx, sy, sw, sh) = (self.ox + x * self.scale, self.oy + y * self.scale, w * self.scale, h * self.scale);
        rr_fill(sx, sy, sw, sh, self.radius(w, h), c);
    }
    pub fn outline(&self, x: f32, y: f32, w: f32, h: f32, t: f32, c: Color) {
        let (sx, sy, sw, sh) = (self.ox + x * self.scale, self.oy + y * self.scale, w * self.scale, h * self.scale);
        rr_line(sx, sy, sw, sh, self.radius(w, h), t * self.scale, c);
    }
    /// A rich window: layered gradient, woven texture, grain, a slow drifting light sweep, a glowing
    /// two-tone rim and corner studs. `accent` tints the rim, the sweep and the studs.
    pub fn panel(&self, x: f32, y: f32, w: f32, h: f32, accent: Color) {
        let sc = self.scale;
        let (sx, sy, sw, sh) = (self.ox + x * sc, self.oy + y * sc, w * sc, h * sc);
        let r = 16.0 * sc;
        let t = get_time() as f32;
        // shadow and body
        rr_fill(sx + 4.0 * sc, sy + 8.0 * sc, sw, sh, r, rgba(0.0, 0.0, 0.0, 0.38));
        rr_grad(sx, sy, sw, sh, r, rgba(0.11, 0.16, 0.22, 0.96), rgba(0.04, 0.05, 0.09, 0.97));
        // warm glow from the accent colour in the lower corner
        let (ix, iy, iw, ih) = (sx + r * 0.6, sy + r * 0.6, sw - r * 1.2, sh - r * 1.2);
        grad_v(ix, iy + ih * 0.55, iw, ih * 0.45, rgba(accent.r, accent.g, accent.b, 0.0), rgba(accent.r, accent.g, accent.b, 0.10));
        // woven texture: fine diagonal hatching, clipped to the inner box
        let step = 7.0 * sc;
        let mut k = -ih;
        while k < iw {
            let (x0, y0) = (ix + k.max(0.0), iy + (-k).max(0.0));
            let len = (iw - k.max(0.0)).min(ih - (-k).max(0.0));
            if len > 0.0 {
                draw_line(x0, y0, x0 + len, y0 + len, 1.0, rgba(1.0, 1.0, 1.0, 0.028));
            }
            k += step;
        }
        // grain
        let mut hsh = 0x9E37_79B9u32;
        for _ in 0..420 {
            hsh = hsh.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let px = ix + (hsh >> 8) as f32 / 16_777_216.0 * iw;
            hsh = hsh.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let py = iy + (hsh >> 8) as f32 / 16_777_216.0 * ih;
            draw_rectangle(px, py, 1.6 * sc, 1.6 * sc, rgba(1.0, 1.0, 1.0, 0.05 + 0.05 * ((hsh & 7) as f32 / 7.0)));
        }
        // top sheen
        grad_v(ix, iy, iw, ih * 0.32, rgba(1.0, 1.0, 1.0, 0.07), rgba(1.0, 1.0, 1.0, 0.0));
        // a soft band of light drifting across
        let bw = iw * 0.22;
        let bx = ix - bw + (iw + bw) * ((t * 0.07).fract());
        let (l, rr_) = (bx.max(ix), (bx + bw).min(ix + iw));
        if rr_ > l {
            let mid = bx + bw * 0.5;
            let a = rgba(accent.r, accent.g, accent.b, 0.0);
            let b = rgba(accent.r, accent.g, accent.b, 0.07);
            if mid > l && mid < rr_ {
                grad_h(l, iy, mid - l, ih, a, b);
                grad_h(mid, iy, rr_ - mid, ih, b, a);
            }
        }
        // rim: outer glow, bright accent edge, inner hairline
        rr_line(sx - 2.0 * sc, sy - 2.0 * sc, sw + 4.0 * sc, sh + 4.0 * sc, r + 2.0 * sc, 4.0 * sc, rgba(accent.r, accent.g, accent.b, 0.13));
        rr_line(sx, sy, sw, sh, r, 2.0 * sc, rgba(accent.r, accent.g, accent.b, 0.85));
        rr_line(sx + 5.0 * sc, sy + 5.0 * sc, sw - 10.0 * sc, sh - 10.0 * sc, r - 4.0 * sc, 1.0, rgba(1.0, 1.0, 1.0, 0.09));
        // a bright line along the top edge that fades out to the sides
        grad_h(sx + r, sy + 1.0, sw * 0.5 - r, 2.0 * sc, rgba(accent.r, accent.g, accent.b, 0.0), rgba(1.0, 1.0, 1.0, 0.7));
        grad_h(sx + sw * 0.5, sy + 1.0, sw * 0.5 - r, 2.0 * sc, rgba(1.0, 1.0, 1.0, 0.7), rgba(accent.r, accent.g, accent.b, 0.0));
        // studs in the corners
        for (cx, cy) in [(sx + 14.0 * sc, sy + 14.0 * sc), (sx + sw - 14.0 * sc, sy + 14.0 * sc), (sx + 14.0 * sc, sy + sh - 14.0 * sc), (sx + sw - 14.0 * sc, sy + sh - 14.0 * sc)] {
            draw_circle(cx, cy, 3.6 * sc, rgba(0.0, 0.0, 0.0, 0.5));
            draw_circle(cx, cy - 0.5 * sc, 2.6 * sc, rgba(accent.r, accent.g, accent.b, 0.9));
            draw_circle(cx - 0.8 * sc, cy - 1.2 * sc, 0.9 * sc, rgba(1.0, 1.0, 1.0, 0.8));
        }
    }
    /// a vertical gradient box with the same soft corners
    pub fn rect_grad(&self, x: f32, y: f32, w: f32, h: f32, top: Color, bot: Color) {
        let (sx, sy, sw, sh) = (self.ox + x * self.scale, self.oy + y * self.scale, w * self.scale, h * self.scale);
        rr_grad(sx, sy, sw, sh, self.radius(w, h), top, bot);
    }
    pub fn text(&self, s: &str, x: f32, y: f32, size: f32, c: Color) {
        draw_text(s, self.ox + x * self.scale, self.oy + (y + size * 0.78) * self.scale, size * self.scale, c);
    }
    pub fn text_w(&self, s: &str, size: f32) -> f32 {
        measure_text(s, None, (size * self.scale) as u16, 1.0).width / self.scale
    }
    pub fn text_c(&self, s: &str, cx: f32, cy: f32, size: f32, c: Color) {
        let w = self.text_w(s, size);
        self.text(s, cx - w * 0.5, cy - size * 0.5, size, c);
    }
    pub fn button(&mut self, x: f32, y: f32, w: f32, h: f32, label: &str, on: bool, accent: bool) -> bool {
        let hot = on && self.hover(x, y, w, h);
        let bg = if !on {
            Color::new(0.12, 0.14, 0.17, 1.0)
        } else if accent {
            if hot { Color::new(0.32, 0.78, 0.66, 1.0) } else { COL_ACCENT }
        } else if hot {
            COL_BTN_HOT
        } else {
            COL_BTN
        };
        // soft drop shadow, a gradient face and a faint rim
        self.rect(x, y + 2.5, w, h, Color::new(0.0, 0.0, 0.0, if on { 0.28 } else { 0.12 }));
        self.rect_grad(x, y, w, h, brighten(bg, if on { 0.07 } else { 0.0 }), Color::new(bg.r * 0.82, bg.g * 0.82, bg.b * 0.82, bg.a));
        self.outline(x, y, w, h, 1.2, Color::new(1.0, 1.0, 1.0, if hot { 0.55 } else { 0.14 }));
        let tc = if on { WHITE } else { Color::new(0.5, 0.5, 0.55, 1.0) };
        self.text_c(label, x + w * 0.5, y + h * 0.5, 22.0, tc);
        let hit = on && hot && self.clicked;
        if hit {
            crate::audio::ui_click();
        }
        hit
    }
    pub fn toggle(&mut self, x: f32, y: f32, w: f32, label: &str, v: &mut bool) -> bool {
        self.text(label, x, y + 8.0, 22.0, WHITE);
        let bx = x + w - 90.0;
        let txt = if *v { "ON" } else { "OFF" };
        if self.button(bx, y, 90.0, 38.0, txt, true, *v) {
            *v = !*v;
            return true;
        }
        false
    }
    pub fn cycle(&mut self, x: f32, y: f32, w: f32, label: &str, idx: &mut usize, names: &[String]) -> bool {
        self.text(label, x, y + 8.0, 22.0, WHITE);
        let bw = 200.0;
        let bx = x + w - bw;
        let mut changed = false;
        if self.button(bx, y, 38.0, 38.0, "<", true, false) {
            *idx = (*idx + names.len() - 1) % names.len();
            changed = true;
        }
        self.rect(bx + 42.0, y, bw - 84.0, 38.0, Color::new(0.1, 0.12, 0.16, 1.0));
        self.text_c(&names[(*idx).min(names.len() - 1)], bx + bw * 0.5, y + 19.0, 20.0, WHITE);
        if self.button(bx + bw - 38.0, y, 38.0, 38.0, ">", true, false) {
            *idx = (*idx + 1) % names.len();
            changed = true;
        }
        changed
    }
    #[allow(clippy::too_many_arguments)]
    pub fn slider(&mut self, id: usize, x: f32, y: f32, w: f32, label: &str, v: &mut f32, lo: f32, hi: f32, show: &str) -> bool {
        self.text(label, x, y + 8.0, 22.0, WHITE);
        let bw = 200.0;
        let bx = x + w - bw;
        self.rect(bx, y + 15.0, bw, 8.0, Color::new(0.1, 0.12, 0.16, 1.0));
        let t = ((*v - lo) / (hi - lo)).clamp(0.0, 1.0);
        self.rect(bx, y + 15.0, (bw * t).max(8.0), 8.0, COL_ACCENT);
        // a round knob
        let (kx, ky) = (self.ox + (bx + bw * t) * self.scale, self.oy + (y + 19.0) * self.scale);
        draw_circle(kx, ky + 1.5 * self.scale, 11.0 * self.scale, Color::new(0.0, 0.0, 0.0, 0.3));
        draw_circle(kx, ky, 11.0 * self.scale, WHITE);
        draw_circle(kx, ky, 5.0 * self.scale, COL_ACCENT);
        self.text(show, bx - 70.0, y + 8.0, 20.0, Color::new(0.8, 0.85, 0.9, 1.0));
        let mut changed = false;
        if self.clicked && self.hover(bx - 8.0, y, bw + 16.0, 38.0) {
            self.drag = Some(id);
        }
        if self.drag == Some(id) {
            if self.down {
                let nv = lo + ((self.mx - bx) / bw).clamp(0.0, 1.0) * (hi - lo);
                if (nv - *v).abs() > 1e-4 {
                    *v = nv;
                    changed = true;
                }
            } else {
                self.drag = None;
            }
        }
        changed
    }
}


// ===========================================================================
// Drawing helpers (screen-pixel space)
// ===========================================================================
use std::cell::Cell;

fn lerp_c(a: Color, b: Color, t: f32) -> Color {
    Color::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t)
}
fn with_a(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}
fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
}
fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::new(r, g, b, a)
}
fn brighten(c: Color, k: f32) -> Color {
    Color::new((c.r + k).min(1.0), (c.g + k).min(1.0), (c.b + k).min(1.0), c.a)
}

fn quad_grad(x: f32, y: f32, w: f32, h: f32, tl: Color, tr: Color, bl: Color, br: Color) {
    let v = vec![
        Vertex::new(x, y, 0.0, 0.0, 0.0, tl),
        Vertex::new(x + w, y, 0.0, 1.0, 0.0, tr),
        Vertex::new(x + w, y + h, 0.0, 1.0, 1.0, br),
        Vertex::new(x, y + h, 0.0, 0.0, 1.0, bl),
    ];
    draw_mesh(&Mesh { vertices: v, indices: vec![0, 1, 2, 0, 2, 3], texture: None });
}
fn grad_v(x: f32, y: f32, w: f32, h: f32, top: Color, bot: Color) {
    quad_grad(x, y, w, h, top, top, bot, bot);
}
fn grad_h(x: f32, y: f32, w: f32, h: f32, left: Color, right: Color) {
    quad_grad(x, y, w, h, left, right, left, right);
}

fn rr_pts(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<Vec2> {
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
    if r < 0.5 {
        return vec![vec2(x, y), vec2(x + w, y), vec2(x + w, y + h), vec2(x, y + h)];
    }
    let seg = 9;
    let mut v = Vec::with_capacity(4 * (seg + 1));
    let corners = [(x + w - r, y + r, -PI * 0.5), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, PI * 0.5), (x + r, y + r, PI)];
    for (cx, cy, a0) in corners {
        for i in 0..=seg {
            let a = a0 + PI * 0.5 * i as f32 / seg as f32;
            v.push(vec2(cx + a.cos() * r, cy + a.sin() * r));
        }
    }
    v
}
pub(crate) fn rr_grad(x: f32, y: f32, w: f32, h: f32, r: f32, top: Color, bot: Color) {
    let pts = rr_pts(x, y, w, h, r);
    let n = pts.len();
    let mut verts = Vec::with_capacity(n + 1);
    verts.push(Vertex::new(x + w * 0.5, y + h * 0.5, 0.0, 0.5, 0.5, lerp_c(top, bot, 0.5)));
    for p in &pts {
        verts.push(Vertex::new(p.x, p.y, 0.0, 0.5, 0.5, lerp_c(top, bot, ((p.y - y) / h.max(1.0)).clamp(0.0, 1.0))));
    }
    let mut idx = Vec::with_capacity(n * 3);
    for i in 0..n {
        idx.push(0u16);
        idx.push(1 + i as u16);
        idx.push(1 + ((i + 1) % n) as u16);
    }
    draw_mesh(&Mesh { vertices: verts, indices: idx, texture: None });
}
pub(crate) fn rr_fill(x: f32, y: f32, w: f32, h: f32, r: f32, c: Color) {
    rr_grad(x, y, w, h, r, c, c);
}
pub(crate) fn rr_line(x: f32, y: f32, w: f32, h: f32, r: f32, t: f32, c: Color) {
    let pts = rr_pts(x, y, w, h, r);
    let n = pts.len();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        draw_line(a.x, a.y, b.x, b.y, t, c);
    }
}

/// A frosted pill behind HUD readouts (screen pixels).
pub fn hud_chip(x: f32, y: f32, w: f32, h: f32) {
    rr_fill(x, y + 2.0, w, h, h * 0.5, Color::new(0.0, 0.0, 0.0, 0.22));
    rr_grad(x, y, w, h, h * 0.5, Color::new(0.12, 0.16, 0.22, 0.80), Color::new(0.04, 0.06, 0.09, 0.80));
    rr_line(x, y, w, h, h * 0.5, 1.2, Color::new(1.0, 1.0, 1.0, 0.16));
}

/// A little clock face showing `hour` (0..24) and `minute`.
pub fn icon_clock(cx: f32, cy: f32, r: f32, hour: f32, minute: f32) {
    draw_circle(cx, cy, r, Color::new(0.92, 0.94, 0.98, 1.0));
    draw_circle(cx, cy, r * 0.86, Color::new(0.14, 0.18, 0.26, 1.0));
    let ha = (hour % 12.0 + minute / 60.0) / 12.0 * TAU - PI * 0.5;
    let ma = minute / 60.0 * TAU - PI * 0.5;
    draw_line(cx, cy, cx + ha.cos() * r * 0.5, cy + ha.sin() * r * 0.5, r * 0.16, WHITE);
    draw_line(cx, cy, cx + ma.cos() * r * 0.74, cy + ma.sin() * r * 0.74, r * 0.11, Color::new(0.75, 0.88, 1.0, 1.0));
}

/// A gold coin.
pub fn icon_coin(cx: f32, cy: f32, r: f32) {
    draw_circle(cx, cy, r, Color::new(0.72, 0.5, 0.08, 1.0));
    draw_circle(cx, cy - r * 0.04, r * 0.88, Color::new(1.0, 0.82, 0.25, 1.0));
    draw_circle_lines(cx, cy, r * 0.58, r * 0.12, Color::new(0.78, 0.55, 0.1, 1.0));
    draw_circle(cx - r * 0.28, cy - r * 0.3, r * 0.14, Color::new(1.0, 0.96, 0.7, 0.9));
}

fn bold_text(s: &str, x: f32, y: f32, size: f32, c: Color) {
    draw_text(s, x + 1.2, y + 1.4, size, Color::new(0.0, 0.0, 0.0, 0.55 * c.a));
    draw_text(s, x, y, size, c);
    draw_text(s, x + 0.8, y, size, c);
}
fn text_w_px(s: &str, size: f32) -> f32 {
    measure_text(s, None, size.max(1.0) as u16, 1.0).width
}
/// bold text centred on (cx, cy)
fn bold_text_c(s: &str, cx: f32, cy: f32, size: f32, c: Color) {
    let d = measure_text(s, None, size.max(1.0) as u16, 1.0);
    bold_text(s, cx - d.width * 0.5, cy + d.height * 0.5, size, c);
}

fn spaced_width(s: &str, size: f32, sp: f32) -> f32 {
    let mut w = 0.0;
    for ch in s.chars() {
        w += text_w_px(&ch.to_string(), size) + sp;
    }
    w - sp
}
/// letter-spaced text with a shadow; `bob` makes the letters gently float
fn draw_spaced(s: &str, x: f32, baseline: f32, size: f32, sp: f32, c: Color, shadow: f32, bob: Option<f32>) {
    let mut cx = x;
    for (i, ch) in s.chars().enumerate() {
        let t = ch.to_string();
        let dy = bob.map_or(0.0, |tm| (tm * 1.4 + i as f32 * 0.55).sin() * size * 0.012);
        if shadow > 0.0 {
            draw_text(&t, cx + shadow, baseline + dy + shadow * 1.2, size, Color::new(0.05, 0.08, 0.15, 0.55 * c.a));
        }
        draw_text(&t, cx, baseline + dy, size, c);
        cx += text_w_px(&t, size) + sp;
    }
}

fn arc_line(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, th: f32, c: Color) {
    let n = (((a1 - a0).abs() * r) / 6.0).ceil().max(2.0) as usize;
    let mut prev = vec2(cx + a0.cos() * r, cy + a0.sin() * r);
    for i in 1..=n {
        let a = a0 + (a1 - a0) * i as f32 / n as f32;
        let q = vec2(cx + a.cos() * r, cy + a.sin() * r);
        draw_line(prev.x, prev.y, q.x, q.y, th, c);
        prev = q;
    }
}

// ---------------------------------------------------------------------------
// Hotbar
// ---------------------------------------------------------------------------

/// One filled hotbar slot.
#[derive(Clone, Copy)]
pub struct HotbarEntry {
    pub kind: crate::items::ItemKind,
    /// stack count to show in the corner (coconuts, bait, fish), if any
    pub count: Option<u32>,
    /// rod tier 0..2 (only meaningful for the rod)
    pub tier: usize,
    /// lantern flame on (only meaningful for the lantern)
    pub lit: bool,
    /// which species (only meaningful for a fish)
    pub fish: usize,
}

fn item_cols(k: crate::items::ItemKind) -> (Color, Color, &'static str) {
    use crate::items::ItemKind as K;
    match k {
        K::Fish => (rgb(0.08, 0.17, 0.26), rgb(0.20, 0.45, 0.62), "FISH"),
        K::FlareGun => (rgb(0.26, 0.09, 0.05), rgb(0.88, 0.38, 0.10), "FLARES"),
        K::Rod => (rgb(0.08, 0.20, 0.19), rgb(0.17, 0.52, 0.42), "ROD"),
        K::Coconut => (rgb(0.20, 0.15, 0.07), rgb(0.50, 0.42, 0.17), "COCONUT"),
        K::Lantern => (rgb(0.30, 0.15, 0.05), rgb(0.86, 0.48, 0.10), "LANTERN"),
        K::Bait => (rgb(0.10, 0.17, 0.28), rgb(0.29, 0.48, 0.70), "BAIT"),
        K::Bucket => (rgb(0.17, 0.21, 0.27), rgb(0.74, 0.34, 0.20), "BUCKET"),
    }
}

fn icon_rod(ix: f32, iy: f32, z: f32, tier: usize) {
    let p = |a: f32, b: f32| vec2(ix + a * z, iy + b * z);
    let (wood, metal) = match tier {
        0 => (rgb(0.68, 0.47, 0.25), rgb(0.74, 0.76, 0.80)),
        1 => (rgb(0.80, 0.85, 0.90), rgb(0.96, 0.82, 0.32)),
        _ => (rgb(0.97, 0.81, 0.32), rgb(0.95, 0.96, 1.0)),
    };
    let a = p(0.14, 0.90);
    let b = p(0.86, 0.12);
    draw_line(a.x + 1.5, a.y + 2.0, b.x + 1.5, b.y + 2.0, (0.07 * z).max(2.0), rgba(0.0, 0.0, 0.0, 0.3));
    draw_line(a.x, a.y, b.x, b.y, (0.06 * z).max(2.0), wood);
    let h = a.lerp(b, 0.22);
    draw_line(a.x, a.y, h.x, h.y, 0.12 * z, rgb(0.28, 0.17, 0.10));
    for f in [0.42, 0.62, 0.82] {
        let q = a.lerp(b, f);
        draw_circle(q.x, q.y + 0.03 * z, 0.022 * z, metal);
    }
    let rc = a.lerp(b, 0.36) + vec2(0.09 * z, 0.09 * z);
    draw_circle(rc.x, rc.y, 0.115 * z, rgb(0.10, 0.11, 0.14));
    draw_circle(rc.x, rc.y, 0.092 * z, metal);
    draw_circle(rc.x, rc.y, 0.05 * z, rgb(0.14, 0.15, 0.18));
    draw_line(rc.x, rc.y, rc.x + 0.12 * z, rc.y - 0.07 * z, (0.025 * z).max(1.5), metal);
    draw_circle(rc.x + 0.12 * z, rc.y - 0.07 * z, 0.03 * z, rgb(0.9, 0.3, 0.2));
    let low = vec2(b.x, iy + 0.62 * z);
    draw_line(b.x, b.y, low.x, low.y, 1.2, rgba(1.0, 1.0, 1.0, 0.85));
    draw_circle_lines(low.x - 0.03 * z, low.y + 0.035 * z, 0.035 * z, 1.5, rgba(1.0, 1.0, 1.0, 0.9));
}

fn icon_coconut(ix: f32, iy: f32, z: f32) {
    let p = |a: f32, b: f32| vec2(ix + a * z, iy + b * z);
    let c = p(0.5, 0.55);
    draw_ellipse(c.x + 0.02 * z, c.y + 0.31 * z, 0.26 * z, 0.05 * z, 0.0, rgba(0.0, 0.0, 0.0, 0.3));
    draw_circle(c.x, c.y, 0.31 * z, rgb(0.27, 0.16, 0.08));
    draw_circle(c.x - 0.03 * z, c.y - 0.04 * z, 0.26 * z, rgb(0.42, 0.27, 0.14));
    draw_circle(c.x - 0.07 * z, c.y - 0.09 * z, 0.16 * z, rgb(0.50, 0.34, 0.18));
    for (a, b) in [(0.0, 0.0), (0.7, 0.9), (1.9, 0.8), (3.1, 0.85), (4.2, 0.9), (5.3, 0.8)] {
        let q = c + vec2((a as f32).cos(), (a as f32).sin()) * (0.30 * z);
        let _ = b;
        draw_line(q.x, q.y, q.x + (a as f32).cos() * 0.05 * z, q.y + (a as f32).sin() * 0.05 * z, 1.5, rgb(0.55, 0.40, 0.22));
    }
    for (a, b) in [(0.40, 0.46), (0.58, 0.46), (0.49, 0.60)] {
        draw_circle(ix + a * z, iy + b * z, 0.05 * z, rgb(0.10, 0.06, 0.03));
        draw_circle(ix + a * z - 0.012 * z, iy + b * z - 0.014 * z, 0.014 * z, rgba(1.0, 1.0, 1.0, 0.35));
    }
}

fn icon_lantern(ix: f32, iy: f32, z: f32, lit: bool, tm: f32) {
    let blk = rgb(0.05, 0.05, 0.07);
    let p = |a: f32, b: f32| vec2(ix + a * z, iy + b * z);
    if lit {
        let g = p(0.5, 0.52);
        draw_circle(g.x, g.y, 0.42 * z, rgba(1.0, 0.75, 0.25, 0.10));
        draw_circle(g.x, g.y, 0.32 * z, rgba(1.0, 0.75, 0.25, 0.14));
    }
    draw_circle_lines(ix + 0.5 * z, iy + 0.16 * z, 0.11 * z, (0.04 * z).max(1.5), blk);
    let r = |x0: f32, y0: f32, x1: f32, y1: f32, c: Color| draw_rectangle(ix + x0 * z, iy + y0 * z, (x1 - x0) * z, (y1 - y0) * z, c);
    r(0.43, 0.20, 0.57, 0.26, blk);
    r(0.30, 0.25, 0.70, 0.33, blk);
    // glass
    let glass = if lit { rgba(1.0, 0.72, 0.28, 0.60) } else { rgba(0.65, 0.72, 0.78, 0.28) };
    r(0.33, 0.33, 0.67, 0.77, glass);
    r(0.31, 0.33, 0.35, 0.78, blk);
    r(0.65, 0.33, 0.69, 0.78, blk);
    r(0.49, 0.33, 0.51, 0.78, rgba(0.05, 0.05, 0.07, 0.55));
    r(0.27, 0.77, 0.73, 0.86, blk);
    // black candle
    r(0.435, 0.56, 0.565, 0.77, rgb(0.13, 0.13, 0.16));
    r(0.435, 0.56, 0.455, 0.77, rgb(0.28, 0.28, 0.33));
    r(0.495, 0.52, 0.505, 0.56, rgb(0.03, 0.03, 0.04));
    if lit {
        let fl = 1.0 + (tm * 13.0).sin() * 0.07 + (tm * 7.3).sin() * 0.05;
        let fc = p(0.5, 0.43);
        draw_ellipse(fc.x, fc.y, 0.085 * z, 0.14 * z * fl, 0.0, rgb(1.0, 0.52, 0.08));
        draw_ellipse(fc.x, fc.y + 0.02 * z, 0.055 * z, 0.10 * z * fl, 0.0, rgb(1.0, 0.82, 0.25));
        draw_ellipse(fc.x, fc.y + 0.04 * z, 0.026 * z, 0.06 * z * fl, 0.0, rgb(1.0, 0.98, 0.8));
    }
}

fn icon_bait(ix: f32, iy: f32, z: f32) {
    let p = |a: f32, b: f32| vec2(ix + a * z, iy + b * z);
    // worms
    for (x0, x1, ph) in [(0.43, 0.36, 0.0f32), (0.58, 0.66, 1.7)] {
        let mut prev = p(x0, 0.45);
        for i in 1..=8 {
            let u = i as f32 / 8.0;
            let q = p(x0 + (x1 - x0) * u + (u * 7.0 + ph).sin() * 0.03, 0.45 - 0.26 * u);
            draw_line(prev.x, prev.y, q.x, q.y, 0.05 * z, rgb(0.86, 0.50, 0.48));
            prev = q;
        }
        draw_circle(prev.x, prev.y, 0.026 * z, rgb(0.92, 0.58, 0.55));
    }
    let tin = rgb(0.73, 0.77, 0.81);
    draw_ellipse(ix + 0.5 * z, iy + 0.80 * z, 0.31 * z, 0.075 * z, 0.0, rgb(0.55, 0.59, 0.64));
    draw_rectangle(ix + 0.19 * z, iy + 0.47 * z, 0.62 * z, 0.33 * z, tin);
    draw_rectangle(ix + 0.62 * z, iy + 0.47 * z, 0.19 * z, 0.33 * z, rgba(0.2, 0.25, 0.32, 0.30));
    draw_rectangle(ix + 0.19 * z, iy + 0.47 * z, 0.05 * z, 0.33 * z, rgba(1.0, 1.0, 1.0, 0.25));
    draw_rectangle(ix + 0.19 * z, iy + 0.56 * z, 0.62 * z, 0.14 * z, rgb(0.78, 0.26, 0.20));
    // tiny worm on the label
    let mut prev = p(0.34, 0.63);
    for i in 1..=7 {
        let u = i as f32 / 7.0;
        let q = p(0.34 + 0.32 * u, 0.63 + (u * 9.0).sin() * 0.025);
        draw_line(prev.x, prev.y, q.x, q.y, (0.025 * z).max(1.0), rgba(1.0, 0.95, 0.9, 0.9));
        prev = q;
    }
    draw_ellipse(ix + 0.5 * z, iy + 0.47 * z, 0.31 * z, 0.08 * z, 0.0, rgb(0.88, 0.91, 0.94));
    draw_ellipse_lines(ix + 0.5 * z, iy + 0.47 * z, 0.31 * z, 0.08 * z, 0.0, 1.2, rgb(0.45, 0.50, 0.56));
    draw_ellipse(ix + 0.5 * z, iy + 0.47 * z, 0.25 * z, 0.05 * z, 0.0, rgb(0.20, 0.13, 0.09));
}

fn icon_bucket(ix: f32, iy: f32, z: f32) {
    let p = |a: f32, b: f32| vec2(ix + a * z, iy + b * z);
    // handle
    let hc = p(0.5, 0.40);
    arc_line(hc.x, hc.y, 0.32 * z, PI, TAU, (0.04 * z).max(1.5), rgb(0.30, 0.33, 0.38));
    // body
    let (tl, tr, bl, br) = (p(0.18, 0.40), p(0.82, 0.40), p(0.27, 0.86), p(0.73, 0.86));
    let body = rgb(0.52, 0.58, 0.66);
    draw_triangle(tl, tr, bl, body);
    draw_triangle(tr, br, bl, body);
    draw_triangle(p(0.52, 0.40), tr, p(0.52, 0.86), rgba(0.0, 0.0, 0.0, 0.0));
    draw_triangle(p(0.58, 0.40), tr, br, rgba(0.1, 0.12, 0.18, 0.28));
    draw_triangle(p(0.58, 0.40), br, p(0.58, 0.86), rgba(0.1, 0.12, 0.18, 0.28));
    draw_triangle(p(0.20, 0.42), p(0.28, 0.42), p(0.31, 0.84), rgba(1.0, 1.0, 1.0, 0.18));
    for f in [0.35, 0.72] {
        let y = 0.40 + 0.46 * f;
        let hw = 0.32 - 0.05 * f;
        draw_line(ix + (0.5 - hw) * z, iy + y * z, ix + (0.5 + hw) * z, iy + y * z, (0.03 * z).max(1.2), rgb(0.30, 0.33, 0.40));
    }
    // rim + water
    draw_ellipse(ix + 0.5 * z, iy + 0.40 * z, 0.33 * z, 0.065 * z, 0.0, rgb(0.82, 0.86, 0.92));
    draw_ellipse(ix + 0.5 * z, iy + 0.41 * z, 0.29 * z, 0.045 * z, 0.0, rgb(0.14, 0.38, 0.52));
    // fish
    let fc = p(0.50, 0.30);
    let rot = -0.38f32;
    let (c, s) = (rot.cos(), rot.sin());
    let tail = [vec2(0.17, 0.0), vec2(0.30, -0.10), vec2(0.30, 0.10)];
    let tp: Vec<Vec2> = tail.iter().map(|q| fc + vec2(q.x * c - q.y * s, q.x * s + q.y * c) * z).collect();
    draw_triangle(tp[0], tp[1], tp[2], rgb(0.38, 0.68, 0.78));
    draw_ellipse(fc.x, fc.y, 0.20 * z, 0.092 * z, rot, rgb(0.60, 0.86, 0.92));
    draw_ellipse(fc.x + 0.01 * z, fc.y + 0.03 * z, 0.16 * z, 0.045 * z, rot, rgb(0.90, 0.97, 0.98));
    let eye = fc + vec2(-0.12 * c + 0.0 * s, -0.12 * s) * z;
    draw_circle(eye.x, eye.y - 0.01 * z, 0.026 * z, WHITE);
    draw_circle(eye.x, eye.y - 0.01 * z, 0.014 * z, BLACK);
}

/// A flare pistol with a little burst of sparks.
fn icon_flare(ix: f32, iy: f32, z: f32, tm: f32) {
    let p = |a: f32, b: f32| vec2(ix + a * z, iy + b * z);
    let orange = rgb(0.95, 0.45, 0.10);
    let black = rgb(0.10, 0.10, 0.12);
    let (a, b) = (p(0.20, 0.56), p(0.78, 0.40));
    draw_line(a.x, a.y, b.x, b.y, 0.15 * z, orange);
    draw_line(b.x - 0.02 * z, b.y, b.x + 0.04 * z, b.y - 0.01 * z, 0.17 * z, black);
    let g0 = p(0.30, 0.60);
    let g1 = p(0.24, 0.84);
    draw_line(g0.x, g0.y, g1.x, g1.y, 0.12 * z, black);
    // sparks
    let k = 0.6 + 0.4 * (tm * 9.0).sin();
    for i in 0..7 {
        let ang = i as f32 * 0.9 + tm * 2.0;
        let c = p(0.86, 0.26) + vec2(ang.cos(), ang.sin()) * (0.10 * z * k);
        draw_circle(c.x, c.y, 0.02 * z, rgb(1.0, 0.8 - 0.05 * i as f32, 0.25));
    }
    let c = p(0.86, 0.26);
    draw_circle(c.x, c.y, 0.045 * z, rgb(1.0, 0.95, 0.7));
}

/// A little fish picture in the species' own colours.
fn icon_fish(ix: f32, iy: f32, z: f32, sp: usize, tm: f32) {
    let s = &crate::fish::SPECIES[sp.min(crate::fish::N_SPECIES - 1)];
    let c = |a: [f32; 3], k: f32| rgb((a[0] * k).min(1.0), (a[1] * k).min(1.0), (a[2] * k).min(1.0));
    let p = |a: f32, b: f32| vec2(ix + a * z, iy + b * z);
    if sp == crate::fish::BOOT {
        let leather = rgb(0.30, 0.21, 0.13);
        draw_rectangle(ix + 0.30 * z, iy + 0.18 * z, 0.22 * z, 0.45 * z, leather);
        draw_ellipse(ix + 0.52 * z, iy + 0.66 * z, 0.30 * z, 0.14 * z, 0.0, leather);
        draw_rectangle(ix + 0.26 * z, iy + 0.76 * z, 0.58 * z, 0.06 * z, rgb(0.14, 0.10, 0.07));
        draw_rectangle(ix + 0.28 * z, iy + 0.16 * z, 0.26 * z, 0.05 * z, rgb(0.20, 0.17, 0.11));
        return;
    }
    let wag = (tm * 3.0).sin() * 0.04;
    let tail = [p(0.20, 0.5 + wag), p(0.04, 0.30 + wag), p(0.04, 0.70 + wag)];
    draw_triangle(tail[0], tail[1], tail[2], c(s.fin, 1.0));
    // body: belly colour below, back colour above
    let h = (s.hb * 2.4).clamp(0.12, 0.34);
    draw_ellipse(ix + 0.52 * z, iy + 0.5 * z, 0.34 * z, h * z, 0.0, c(s.belly, 1.0));
    draw_ellipse(ix + 0.52 * z, iy + (0.5 - h * 0.28) * z, 0.34 * z, h * 0.72 * z, 0.0, c(s.back, 1.0));
    draw_triangle(p(0.40, 0.5 - h * 0.9), p(0.62, 0.5 - h * 0.9), p(0.48, 0.5 - h * 1.5), c(s.fin, 1.0));
    draw_circle(ix + 0.74 * z, iy + 0.45 * z, 0.032 * z, if s.glow_eye { c(s.eye, 1.0) } else { WHITE });
    draw_circle(ix + 0.75 * z, iy + 0.45 * z, 0.016 * z, BLACK);
    if s.teeth > 0 {
        for k in 0..3 {
            let x = 0.80 + 0.04 * k as f32;
            draw_triangle(p(x, 0.52), p(x + 0.03, 0.52), p(x + 0.015, 0.58), WHITE);
        }
    }
}

fn draw_tile(x: f32, y: f32, ss: f32, hh: f32, hs: f32, e: &HotbarEntry, sel: bool, tm: f32) {
    let (top, bot, name) = item_cols(e.kind);
    let name = if e.kind == crate::items::ItemKind::Fish { crate::fish::SPECIES[e.fish.min(crate::fish::N_SPECIES - 1)].name } else { name };
    let k = if sel { 0.10 } else { 0.0 };
    // soft shadow
    draw_rectangle(x + 1.5 * hs, y + 3.0 * hs, ss, ss, rgba(0.0, 0.0, 0.0, 0.35));
    grad_v(x, y, ss, ss, brighten(top, k), brighten(bot, k));
    // header strip
    draw_rectangle(x, y, ss, hh, rgba(0.02, 0.03, 0.05, 0.62));
    let label = if matches!(e.kind, crate::items::ItemKind::Rod) {
        format!("{} {}", name, ["I", "II", "III", "IV", "V"][e.tier.min(4)])
    } else {
        name.to_string()
    };
    let fs = 10.5 * hs;
    let d = measure_text(&label, None, fs as u16, 1.0);
    let fs = if d.width > ss - 6.0 * hs { fs * (ss - 6.0 * hs) / d.width } else { fs };
    let d = measure_text(&label, None, fs as u16, 1.0);
    draw_text(&label, x + (ss - d.width) * 0.5, y + hh * 0.5 + d.height * 0.5, fs, rgba(0.88, 0.93, 0.98, 0.85));
    // border
    let bc = brighten(bot, 0.30);
    draw_rectangle_lines(x, y, ss, ss, 2.0 * hs, with_a(bc, if sel { 0.55 } else { 0.85 }));
    // icon
    let isz = ss - hh - 6.0 * hs;
    let ix = x + (ss - isz) * 0.5;
    let iy = y + hh + 3.0 * hs;
    use crate::items::ItemKind as K;
    match e.kind {
        K::Rod => icon_rod(ix, iy, isz, e.tier),
        K::Coconut => icon_coconut(ix, iy, isz),
        K::Lantern => icon_lantern(ix, iy, isz, e.lit, tm),
        K::Bait => icon_bait(ix, iy, isz),
        K::Bucket => icon_bucket(ix, iy, isz),
        K::Fish => icon_fish(ix, iy, isz, e.fish, tm),
        K::FlareGun => icon_flare(ix, iy, isz, tm),
    }
    // count badge
    if let Some(n) = e.count {
        let txt = format!("{}", n);
        let fs = 15.0 * hs;
        let d = measure_text(&txt, None, fs as u16, 1.0);
        let bw = (d.width + 9.0 * hs).max(16.0 * hs);
        let bh = 16.0 * hs;
        let bx = x + ss - bw - 2.5 * hs;
        let by = y + ss - bh - 2.5 * hs;
        rr_fill(bx, by, bw, bh, 4.0 * hs, rgba(0.02, 0.03, 0.05, 0.82));
        rr_line(bx, by, bw, bh, 4.0 * hs, 1.0, rgba(1.0, 1.0, 1.0, 0.45));
        bold_text(&txt, bx + (bw - d.width) * 0.5, by + bh * 0.5 + d.height * 0.5, fs, WHITE);
    }
}

/// Draws the 5-slot hotbar flush with the bottom edge of the screen (coloured slot tiles,
/// number plates under each slot, bright outline on the selected slot).
/// Empty slots are drawn as dim empty tiles. `keys` are the key labels for slots 1..5.
pub fn draw_hotbar(slots: &[Option<HotbarEntry>; 5], selected: usize, keys: &[String; 5], hs: f32) {
    let sw = screen_width();
    let sh = screen_height();
    let ss = 64.0 * hs;
    let gap = 9.0 * hs;
    let plate_h = 20.0 * hs;
    let hh = 13.0 * hs;
    let total = 5.0 * ss + 4.0 * gap;
    let x0 = ((sw - total) * 0.5).round();
    let plate_y = sh - 3.0 * hs - plate_h;
    let ty = plate_y - 4.0 * hs - ss;
    let tm = get_time() as f32;
    for i in 0..5 {
        let x = (x0 + i as f32 * (ss + gap)).round();
        let sel = i == selected;
        let y = if sel { ty - 6.0 * hs } else { ty };
        match &slots[i] {
            Some(e) => {
                if sel {
                    draw_rectangle(x - 5.0 * hs, y - 5.0 * hs, ss + 10.0 * hs, ss + 10.0 * hs, rgba(0.6, 0.85, 1.0, 0.16));
                }
                draw_tile(x, y, ss, hh, hs, e, sel, tm);
            }
            None => {
                draw_rectangle(x, y, ss, ss, rgba(0.04, 0.06, 0.09, if sel { 0.42 } else { 0.28 }));
                draw_rectangle(x, y, ss, hh, rgba(0.0, 0.0, 0.0, 0.18));
                draw_rectangle_lines(x, y, ss, ss, 1.5 * hs, rgba(1.0, 1.0, 1.0, if sel { 0.35 } else { 0.12 }));
            }
        }
        if sel {
            draw_rectangle_lines(x - 1.5 * hs, y - 1.5 * hs, ss + 3.0 * hs, ss + 3.0 * hs, 2.6 * hs, rgb(0.82, 0.93, 1.0));
        }
        // number plate
        let label = keys[i].as_str();
        let fs = 15.0 * hs;
        let d = measure_text(label, None, fs as u16, 1.0);
        let pw = (d.width + 14.0 * hs).max(ss * 0.46).min(ss);
        let px = x + (ss - pw) * 0.5;
        rr_fill(px, plate_y, pw, plate_h, 5.0 * hs, rgba(0.03, 0.04, 0.07, 0.88));
        rr_line(px, plate_y, pw, plate_h, 5.0 * hs, (1.6 * hs).max(1.0), if sel { rgb(0.82, 0.93, 1.0) } else { rgba(1.0, 1.0, 1.0, if slots[i].is_some() { 0.9 } else { 0.5 }) });
        let fs = if d.width > pw - 8.0 * hs { fs * (pw - 8.0 * hs) / d.width } else { fs };
        bold_text_c(label, px + pw * 0.5, plate_y + plate_h * 0.5, fs, if slots[i].is_some() || sel { WHITE } else { rgba(1.0, 1.0, 1.0, 0.65) });
    }
}

// ---------------------------------------------------------------------------
// Shared scenery (title screen + loading screen)
// ---------------------------------------------------------------------------
fn draw_waves(w: f32, h: f32, horizon: f32, t: f32, s: f32, layers: usize, c_far: Color, c_near: Color, amp: f32, foam: f32) {
    let step = (5.0 * s).max(3.0);
    for k in 0..layers {
        let f = k as f32 / (layers - 1).max(1) as f32;
        let base = horizon + (h - horizon) * (f.powf(1.7) * 0.9);
        let col = lerp_c(c_far, c_near, f);
        let a = amp * (0.35 + f * 1.5) * s;
        let wl = 70.0 * (0.5 + f * 1.1) * s;
        let ph = t * (0.35 + f * 0.45) + k as f32 * 1.7;
        let mut x = 0.0;
        while x < w {
            let y = base + (x / wl + ph).sin() * a + (x / (wl * 0.43) - ph * 1.3).sin() * a * 0.4;
            draw_rectangle(x, y, step + 1.0, h - y + 1.0, col);
            if foam > 0.0 {
                draw_rectangle(x, y, step + 1.0, (1.5 + 2.0 * f) * s, rgba(1.0, 1.0, 1.0, foam * (0.3 + f * 0.7)));
            }
            x += step;
        }
    }
}

fn draw_palm(bx: f32, by: f32, ht: f32, lean: f32, t: f32, ph: f32, s: f32, col: Color) {
    let p0 = vec2(bx, by);
    let p1 = vec2(bx + lean * 0.12, by - ht * 0.55);
    let p2 = vec2(bx + lean, by - ht);
    let n = 18;
    let mut prev = p0;
    for i in 1..=n {
        let u = i as f32 / n as f32;
        let q = p0 * ((1.0 - u) * (1.0 - u)) + p1 * (2.0 * u * (1.0 - u)) + p2 * (u * u);
        let th = (20.0 - 11.0 * u) * s;
        draw_line(prev.x, prev.y, q.x, q.y, th, col);
        draw_circle(q.x, q.y, th * 0.5, col);
        prev = q;
    }
    let crown = p2;
    let nf = 10;
    for k in 0..nf {
        let a = -PI * 0.97 + PI * 0.94 * k as f32 / (nf - 1) as f32;
        let dir = vec2(a.cos(), a.sin());
        let len = ht * (0.36 + 0.07 * (k as f32 * 2.3).sin());
        let droop = len * (0.30 + 0.75 * a.cos().abs());
        let sway = (t * 1.3 + ph + k as f32 * 0.7).sin() * len * 0.07;
        let segs = 12;
        let pt = |u: f32| crown + dir * (len * u) + vec2(sway * u * u, droop * u * u);
        let mut prev_p = pt(0.0);
        let mut prev_l = prev_p;
        let mut prev_r = prev_p;
        for i in 1..=segs {
            let u = i as f32 / segs as f32;
            let q = pt(u);
            let tang = (q - prev_p).normalize_or_zero();
            let nrm = vec2(-tang.y, tang.x);
            let hw = len * 0.085 * (PI * u.powf(0.65)).sin().max(0.0) + 1.0 * s;
            let (l, r) = (q + nrm * hw, q - nrm * hw);
            draw_triangle(prev_l, prev_r, l, col);
            draw_triangle(prev_r, r, l, col);
            prev_p = q;
            prev_l = l;
            prev_r = r;
        }
    }
    for (dx, dy) in [(-7.0, 6.0), (6.0, 8.0), (0.0, 12.0)] {
        draw_circle(crown.x + dx * s, crown.y + dy * s, 7.0 * s, col);
    }
}

fn draw_cloud(cx: f32, cy: f32, sz: f32, lit: Color, shade: Color) {
    draw_ellipse(cx, cy + sz * 0.10, sz * 1.25, sz * 0.26, 0.0, shade);
    for (dx, dy, r) in [(-0.7, 0.0, 0.42), (-0.2, -0.22, 0.58), (0.35, -0.08, 0.5), (0.8, 0.04, 0.34)] {
        draw_circle(cx + dx * sz, cy + dy * sz, r * sz, lit);
    }
    draw_ellipse(cx, cy + sz * 0.2, sz * 1.2, sz * 0.2, 0.0, with_a(shade, 0.7));
}

fn draw_bird(x: f32, y: f32, sz: f32, flap: f32, c: Color) {
    let f = flap * sz * 0.5;
    draw_line(x - sz, y - f, x, y, (sz * 0.18).max(1.2), c);
    draw_line(x, y, x + sz, y - f, (sz * 0.18).max(1.2), c);
}

/// Warm late-afternoon title scene: sky, sun, clouds, sea, a lone island with a tiny campfire,
/// palms in the foreground and a few birds. Pure screen-pixel drawing.
fn draw_title_scene(t: f32) {
    let (w, h) = (screen_width(), screen_height());
    let s = (h / 720.0).max(0.3);
    // deep water: dark teal at the top fading to black
    grad_v(0.0, 0.0, w, h * 0.55, rgb(0.02, 0.10, 0.15), rgb(0.01, 0.05, 0.09));
    grad_v(0.0, h * 0.55 - 1.0, w, h * 0.45 + 1.0, rgb(0.01, 0.05, 0.09), rgb(0.0, 0.01, 0.025));
    // faint slanting light shafts from the surface
    for i in 0..5 {
        let x0 = w * (0.1 + 0.2 * i as f32) + (t * 0.25 + i as f32 * 1.7).sin() * 40.0 * s;
        let a = 0.035 + 0.02 * (t * 0.4 + i as f32 * 2.3).sin().abs();
        let v = [vec2(x0, 0.0), vec2(x0 + 70.0 * s, 0.0), vec2(x0 + 260.0 * s, h * 0.7), vec2(x0 + 120.0 * s, h * 0.7)];
        draw_triangle(v[0], v[1], v[2], rgba(0.45, 0.8, 0.85, a));
        draw_triangle(v[0], v[2], v[3], rgba(0.45, 0.8, 0.85, a * 0.6));
    }
    // drifting marine snow
    for i in 0..90 {
        let f = i as f32;
        let x = ((f * 97.13).sin() * 0.5 + 0.5) * w + (t * 0.3 + f).sin() * 12.0 * s;
        let y = (((f * 53.7).cos() * 0.5 + 0.5) * h + t * (6.0 + (i % 7) as f32 * 2.0) * s).rem_euclid(h);
        draw_circle(x, y, (0.8 + (i % 3) as f32 * 0.5) * s, rgba(0.7, 0.9, 0.9, 0.10 + 0.08 * (i % 4) as f32 / 3.0));
    }
    // vignette
    grad_v(0.0, 0.0, w, h * 0.12, rgba(0.0, 0.0, 0.0, 0.5), rgba(0.0, 0.0, 0.0, 0.0));
    grad_v(0.0, h * 0.8, w, h * 0.2, rgba(0.0, 0.0, 0.0, 0.0), rgba(0.0, 0.0, 0.0, 0.55));
}

/// A deep, sonar-like "bloop" as a 16-bit mono WAV: a sine that sinks in pitch, with a soft attack,
/// a long decay and two quiet echoes.
fn bloop_wav() -> Vec<u8> {
    let rate = 22050u32;
    let secs = 3.2f32;
    let n = (rate as f32 * secs) as usize;
    let mut pcm = vec![0f32; n];
    let tone = |t: f32| -> f32 {
        if t < 0.0 {
            return 0.0;
        }
        // pitch falls from ~150 Hz towards ~45 Hz
        let f = 45.0 + 105.0 * (-t * 3.2).exp();
        let phase = TAU * (45.0 * t + 105.0 * (1.0 - (-t * 3.2).exp()) / 3.2);
        let env = (t / 0.04).min(1.0) * (-t * 2.4).exp();
        let _ = f;
        (phase.sin() + 0.25 * (phase * 2.0).sin()) * env
    };
    for (i, p) in pcm.iter_mut().enumerate() {
        let t = i as f32 / rate as f32;
        *p = tone(t) * 0.8 + tone(t - 0.55) * 0.3 + tone(t - 1.1) * 0.12;
    }
    let mut out = Vec::with_capacity(44 + n * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + n * 2) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&((n * 2) as u32).to_le_bytes());
    for p in pcm {
        out.extend_from_slice(&((p.clamp(-1.0, 1.0) * 26000.0) as i16).to_le_bytes());
    }
    out
}

#[allow(dead_code)]
/// A huge, barely visible silhouette gliding past in the dark.
fn draw_deep_shadow(x: f32, y: f32, len: f32, dir: f32, a: f32, t: f32) {
    let c = rgba(0.0, 0.02, 0.03, a);
    draw_ellipse(x, y, len * 0.5, len * 0.12, 0.0, c);
    let tx = x - dir * len * 0.5;
    let wag = (t * 1.3).sin() * len * 0.05;
    draw_triangle(vec2(tx + dir * len * 0.08, y), vec2(tx - dir * len * 0.2, y - len * 0.16 + wag), vec2(tx - dir * len * 0.2, y + len * 0.16 + wag), c);
    draw_triangle(vec2(x - dir * len * 0.05, y - len * 0.1), vec2(x + dir * len * 0.1, y - len * 0.1), vec2(x - dir * len * 0.1, y - len * 0.26), c);
    draw_circle(x + dir * len * 0.34, y - len * 0.02, len * 0.012, rgba(0.8, 0.9, 0.7, a * 0.5));
}

#[allow(dead_code)]
/// A deep-sea angler: black body, rows of needle teeth, a glowing lure and a cold eye.
fn draw_angler(x: f32, y: f32, sz: f32, dir: f32, a: f32, t: f32) {
    let body = rgba(0.015, 0.02, 0.03, a);
    let bob = (t * 1.1).sin() * sz * 0.04;
    let y = y + bob;
    // tail and fins
    let tx = x - dir * sz * 0.95;
    let wag = (t * 3.0).sin() * sz * 0.10;
    draw_triangle(vec2(tx + dir * sz * 0.2, y), vec2(tx - dir * sz * 0.35, y - sz * 0.38 + wag), vec2(tx - dir * sz * 0.35, y + sz * 0.38 + wag), body);
    draw_triangle(vec2(x - dir * sz * 0.2, y - sz * 0.35), vec2(x - dir * sz * 0.7, y - sz * 0.35), vec2(x - dir * sz * 0.6, y - sz * 0.78), body);
    // body and jaw
    draw_ellipse(x, y, sz, sz * 0.62, 0.0, body);
    let mx = x + dir * sz * 0.72;
    // open mouth: dark cavity with teeth top and bottom
    draw_triangle(vec2(mx, y - sz * 0.02), vec2(mx - dir * sz * 0.55, y - sz * 0.42), vec2(mx - dir * sz * 0.55, y + sz * 0.38), rgba(0.0, 0.0, 0.0, a));
    let teeth = rgba(0.82, 0.88, 0.8, a * 0.9);
    for i in 0..7 {
        let f = i as f32 / 6.0;
        let px = mx - dir * sz * (0.08 + 0.45 * f);
        let top = y - sz * (0.04 + 0.34 * f);
        let bot = y + sz * (0.02 + 0.30 * f);
        let tl = sz * (0.22 - 0.05 * f);
        draw_triangle(vec2(px, top), vec2(px - dir * sz * 0.06, top), vec2(px - dir * sz * 0.03, top + tl), teeth);
        draw_triangle(vec2(px, bot), vec2(px - dir * sz * 0.06, bot), vec2(px - dir * sz * 0.03, bot - tl), teeth);
    }
    // eye
    let (ex, ey) = (x + dir * sz * 0.38, y - sz * 0.3);
    draw_circle(ex, ey, sz * 0.10, rgba(0.85, 0.95, 0.85, a * 0.9));
    draw_circle(ex, ey, sz * 0.045, rgba(0.0, 0.0, 0.0, a));
    // glowing lure on a curved stalk
    let lx = x + dir * sz * 1.05;
    let ly = y - sz * 0.95 + (t * 1.7).sin() * sz * 0.05;
    draw_line(x + dir * sz * 0.25, y - sz * 0.55, x + dir * sz * 0.7, y - sz * 0.95, (sz * 0.03).max(1.5), body);
    draw_line(x + dir * sz * 0.7, y - sz * 0.95, lx, ly, (sz * 0.02).max(1.0), body);
    let pulse = 0.75 + 0.25 * (t * 4.0).sin();
    for k in (1..7).rev() {
        draw_circle(lx, ly, sz * 0.05 * k as f32 * 1.6, rgba(0.35, 0.95, 0.75, 0.035 * a * pulse));
    }
    draw_circle(lx, ly, sz * 0.055, rgba(0.8, 1.0, 0.9, a * pulse));
}

#[allow(dead_code)]
fn draw_title_scene_sunset(t: f32) {
    let (w, h) = (screen_width(), screen_height());
    let s = (h / 720.0).max(0.3);
    let hz = h * 0.60;
    // sky
    grad_v(0.0, 0.0, w, hz * 0.55, rgb(0.11, 0.22, 0.46), rgb(0.33, 0.45, 0.68));
    grad_v(0.0, hz * 0.55 - 1.0, w, hz * 0.30 + 1.0, rgb(0.33, 0.45, 0.68), rgb(0.86, 0.60, 0.55));
    grad_v(0.0, hz * 0.85 - 1.0, w, hz * 0.15 + 2.0, rgb(0.86, 0.60, 0.55), rgb(1.0, 0.80, 0.52));
    // sun + glow
    let (sx, sy, sr) = (w * 0.74, hz - 46.0 * s, 36.0 * s);
    for k in (1..10).rev() {
        draw_circle(sx, sy, sr * (1.0 + k as f32 * 0.6), rgba(1.0, 0.78, 0.45, 0.045));
    }
    draw_circle(sx, sy, sr * 1.18, rgba(1.0, 0.9, 0.65, 0.35));
    draw_circle(sx, sy, sr, rgb(1.0, 0.96, 0.78));
    // clouds
    let span = w + 500.0 * s;
    for i in 0..7 {
        let sp = (5.0 + i as f32 * 2.2) * s;
        let cx = (i as f32 * 331.0 * s + t * sp).rem_euclid(span) - 250.0 * s;
        let cy = hz * (0.12 + 0.13 * ((i * 5) % 4) as f32 / 3.0 + 0.02 * i as f32 / 6.0);
        let sz = (46.0 + (i % 3) as f32 * 16.0) * s;
        draw_cloud(cx, cy, sz, rgba(1.0, 0.86, 0.76, 0.78), rgba(0.80, 0.50, 0.55, 0.45));
    }
    // sea base
    grad_v(0.0, hz, w, h - hz, rgb(0.93, 0.62, 0.45), rgb(0.03, 0.17, 0.30));
    // distant island with a tiny campfire
    let (ix0, iw, ih) = (w * 0.42, w * 0.24, 46.0 * s);
    let col_i = rgb(0.20, 0.22, 0.34);
    let mut x = 0.0;
    while x < iw {
        let u = x / iw;
        let hh = ih * (PI * u).sin().powf(0.85) * (0.78 + 0.22 * (u * 9.0 + 1.0).sin() + 0.10 * (u * 23.0).sin());
        draw_rectangle(ix0 + x, hz - hh, 3.0 * s + 1.0, hh + 1.0, col_i);
        x += 3.0 * s;
    }
    for u in [0.30f32, 0.46, 0.58, 0.72] {
        let hh = ih * (PI * u).sin().powf(0.85);
        let (px, py) = (ix0 + iw * u, hz - hh);
        draw_circle(px, py - 5.0 * s, 6.5 * s, col_i);
        draw_line(px - 9.0 * s, py - 2.0 * s, px, py - 6.0 * s, 2.2 * s, col_i);
        draw_line(px + 9.0 * s, py - 2.0 * s, px, py - 6.0 * s, 2.2 * s, col_i);
    }
    // campfire glow on the shore
    let (fx, fy) = (ix0 + iw * 0.38, hz - 3.5 * s);
    let fl = 0.75 + (t * 9.0).sin() * 0.12 + (t * 5.3 + 1.0).sin() * 0.1;
    draw_circle(fx, fy, 15.0 * s * fl, rgba(1.0, 0.55, 0.15, 0.10));
    draw_circle(fx, fy, 8.0 * s * fl, rgba(1.0, 0.65, 0.20, 0.22));
    draw_circle(fx, fy, 2.6 * s, rgb(1.0, 0.82, 0.40));
    // sea layers
    draw_waves(w, h, hz, t, s, 9, rgb(0.78, 0.55, 0.50), rgb(0.02, 0.16, 0.28), 2.6, 0.22);
    // sun glitter
    for j in 0..26 {
        let f = j as f32 / 26.0;
        let gy = hz + (h - hz) * f.powf(1.6) * 0.62 + 3.0 * s;
        let gw = (10.0 + j as f32 * 7.0) * s * (0.55 + 0.45 * (t * 2.1 + j as f32 * 1.7).sin().abs());
        let gx = sx + (t * 1.1 + j as f32 * 2.1).sin() * (6.0 + j as f32 * 2.8) * s;
        draw_rectangle(gx - gw * 0.5, gy, gw, (1.6 + f * 2.5) * s, rgba(1.0, 0.9, 0.62, 0.55 * (1.0 - f * 0.7)));
    }
    // campfire reflection
    for j in 0..6 {
        let gy = hz + (5.0 + j as f32 * 7.0) * s;
        let gw = (6.0 - j as f32 * 0.7).max(2.0) * s * (0.6 + 0.4 * (t * 3.0 + j as f32).sin().abs());
        draw_rectangle(fx - gw * 0.5 + (t * 2.0 + j as f32).sin() * 1.5 * s, gy, gw, 1.6 * s, rgba(1.0, 0.6, 0.2, 0.4 - j as f32 * 0.05));
    }
    // birds
    for i in 0..5 {
        let sp = (22.0 + i as f32 * 6.0) * s;
        let bx = (i as f32 * 0.21 * w + t * sp).rem_euclid(w + 120.0 * s) - 60.0 * s;
        let by = hz * (0.30 + 0.07 * i as f32) + (t * 0.8 + i as f32).sin() * 8.0 * s;
        draw_bird(bx, by, (7.0 + (i % 2) as f32 * 3.0) * s, (t * 6.0 + i as f32 * 1.3).sin(), rgba(0.10, 0.10, 0.18, 0.8));
    }
    // foreground: dune + palms
    let dark = rgb(0.03, 0.07, 0.10);
    draw_ellipse(w * 0.97, h + 40.0 * s, w * 0.20, 120.0 * s, 0.0, dark);
    draw_ellipse(w * 0.02, h + 50.0 * s, w * 0.14, 110.0 * s, 0.0, dark);
    draw_palm(w * 0.92, h + 10.0 * s, h * 0.80, -w * 0.10, t, 0.0, s, dark);
    draw_palm(w * 0.86, h + 10.0 * s, h * 0.52, w * 0.025, t, 2.0, s, dark);
    draw_palm(w * 0.02, h + 10.0 * s, h * 0.62, w * 0.07, t, 4.0, s, dark);
    // readability gradient behind the menu column
    grad_h(0.0, 0.0, w * 0.45, h, rgba(0.0, 0.02, 0.06, 0.55), rgba(0.0, 0.02, 0.06, 0.0));
    grad_v(0.0, h * 0.8, w, h * 0.2, rgba(0.0, 0.0, 0.0, 0.0), rgba(0.0, 0.0, 0.0, 0.30));
}

// ---------------------------------------------------------------------------
// Loading screen
// ---------------------------------------------------------------------------
thread_local! {
    static LOAD_DISP: Cell<f32> = Cell::new(0.0);
    static PREVIEW: Cell<(f32, bool, f32)> = Cell::new((0.6, false, 0.0));
}

const TIPS: [&str; 7] = [
    "Hold the mouse to charge your cast",
    "Pull the mouse back to reel in a fish",
    "Trade fish with the hermit for better gear",
    "Rest at the tent to save and skip to day or night",
    "Press E to interact",
    "A lit lantern keeps the dark at bay",
    "Coconuts quench your thirst - drink them with F",
];

/// One frame of the loading screen: progress 0..1 and a short status line.
pub fn draw_loading(progress: f32, text: &str, time: f32) {
    set_default_camera();
    let (w, h) = (screen_width(), screen_height());
    let s = (h / 720.0).min(w / 1000.0).max(0.35);
    let dt = get_frame_time().min(0.05);
    let progress = progress.clamp(0.0, 1.0);
    let disp = LOAD_DISP.with(|d| {
        let mut v = d.get();
        if progress < v - 0.3 {
            v = progress;
        }
        v += (progress - v) * (1.0 - (-7.0 * dt).exp());
        d.set(v);
        v
    });

    grad_v(0.0, 0.0, w, h, rgb(0.02, 0.05, 0.13), rgb(0.05, 0.17, 0.27));
    // drifting plankton lights
    for i in 0..46 {
        let f = i as f32;
        let x = ((f * 0.618_034 + 0.13).fract() * w + (time * 0.4 + f).sin() * 14.0 * s).rem_euclid(w);
        let y = ((f * 0.754_877 + 0.31).fract() * h - time * (6.0 + (i % 5) as f32 * 3.0) * s).rem_euclid(h);
        let a = 0.08 + 0.17 * (0.5 + 0.5 * (time * 1.3 + f * 2.1).sin());
        draw_circle(x, y, (1.2 + (i % 3) as f32 * 0.8) * s, rgba(0.55, 0.9, 0.95, a));
    }
    // centre glow
    for k in 1..8 {
        draw_circle(w * 0.5, h * 0.44, (60.0 + k as f32 * 38.0) * s, rgba(0.2, 0.6, 0.65, 0.025));
    }
    // title
    let tsz = 108.0 * s;
    let sp = 14.0 * s;
    let tw = spaced_width("ISLAND", tsz, sp);
    draw_spaced("ISLAND", (w - tw) * 0.5, h * 0.25, tsz, sp, rgb(0.93, 0.96, 0.98), 4.0 * s, Some(time));
    let sub = "A castaway fishing story";
    let ssz = 22.0 * s;
    let subw = spaced_width(sub, ssz, 3.0 * s);
    draw_spaced(sub, (w - subw) * 0.5, h * 0.25 + 38.0 * s, ssz, 3.0 * s, rgba(0.70, 0.85, 0.90, 0.85), 0.0, None);
    // compass icon
    let bob = (time * 1.6).sin() * 4.0 * s;
    let (cx, cy, r) = (w * 0.5, h * 0.46 + bob, 40.0 * s);
    draw_circle(cx, cy, r + 4.0 * s, rgba(0.03, 0.08, 0.14, 0.7));
    draw_circle_lines(cx, cy, r, 2.5 * s, rgba(0.65, 0.9, 0.9, 0.85));
    draw_circle_lines(cx, cy, r - 6.0 * s, 1.0 * s, rgba(0.65, 0.9, 0.9, 0.35));
    let rot = time * 0.7;
    for i in 0..16 {
        let a = rot + i as f32 * TAU / 16.0;
        let len = if i % 4 == 0 { 8.0 } else { 4.0 } * s;
        draw_line(cx + a.cos() * (r - 1.0 * s), cy + a.sin() * (r - 1.0 * s), cx + a.cos() * (r - 1.0 * s - len), cy + a.sin() * (r - 1.0 * s - len), 1.6 * s, rgba(0.8, 0.95, 0.95, 0.8));
    }
    let na = time * 2.1 + (time * 3.0).sin() * 0.6 - PI * 0.5;
    let nd = vec2(na.cos(), na.sin());
    let nn = vec2(-nd.y, nd.x);
    draw_triangle(vec2(cx, cy) + nd * (r * 0.62), vec2(cx, cy) + nn * (4.5 * s), vec2(cx, cy) - nn * (4.5 * s), rgb(0.95, 0.45, 0.35));
    draw_triangle(vec2(cx, cy) - nd * (r * 0.62), vec2(cx, cy) + nn * (4.5 * s), vec2(cx, cy) - nn * (4.5 * s), rgb(0.85, 0.92, 0.95));
    draw_circle(cx, cy, 3.0 * s, rgb(0.1, 0.15, 0.2));
    // progress ring around the compass
    arc_line(cx, cy, r + 12.0 * s, -PI * 0.5, -PI * 0.5 + TAU * disp.max(0.003), 3.0 * s, rgb(0.30, 0.78, 0.66));
    // bar
    let (bw, bh) = (520.0 * s, 14.0 * s);
    let (bx, by) = ((w - bw) * 0.5, h * 0.62);
    rr_fill(bx - 3.0 * s, by - 3.0 * s, bw + 6.0 * s, bh + 6.0 * s, (bh + 6.0 * s) * 0.5, rgba(0.0, 0.0, 0.0, 0.35));
    rr_fill(bx, by, bw, bh, bh * 0.5, rgba(0.10, 0.17, 0.24, 1.0));
    let fw = (bw * disp).max(bh);
    rr_grad(bx, by, fw, bh, bh * 0.5, rgb(0.38, 0.88, 0.76), rgb(0.16, 0.58, 0.52));
    let shine = ((time * 0.7) % 1.6 - 0.3) * fw;
    let (sx0, sx1) = ((bx + shine - 30.0 * s).max(bx + bh * 0.5), (bx + shine + 30.0 * s).min(bx + fw - bh * 0.5));
    if sx1 > sx0 {
        let mid = (sx0 + sx1) * 0.5;
        grad_h(sx0, by + 2.0 * s, mid - sx0, bh * 0.4, rgba(1.0, 1.0, 1.0, 0.0), rgba(1.0, 1.0, 1.0, 0.35));
        grad_h(mid, by + 2.0 * s, sx1 - mid, bh * 0.4, rgba(1.0, 1.0, 1.0, 0.35), rgba(1.0, 1.0, 1.0, 0.0));
    }
    rr_line(bx, by, bw, bh, bh * 0.5, 1.2, rgba(1.0, 1.0, 1.0, 0.25));
    let pct = format!("{}%", (disp * 100.0).round() as i32);
    let psz = 22.0 * s;
    draw_text(&pct, bx + bw - text_w_px(&pct, psz), by - 8.0 * s, psz, rgb(0.85, 0.95, 0.95));
    // status
    let tsz2 = 24.0 * s;
    draw_text(text, (w - text_w_px(text, tsz2)) * 0.5, by + bh + 34.0 * s, tsz2, rgba(0.82, 0.90, 0.94, 0.95));
    // waves along the bottom
    draw_waves(w, h, h * 0.86, time, s, 5, rgba(0.05, 0.20, 0.30, 0.0), rgb(0.02, 0.07, 0.13), 2.0, 0.07);
    // tips
    let period = 4.5;
    let tt = time / period;
    let idx = (tt.floor() as usize) % TIPS.len();
    let ph = tt.fract();
    let a = (ph * 6.0).min((1.0 - ph) * 6.0).clamp(0.0, 1.0);
    let tip = format!("Tip: {}", TIPS[idx]);
    let fsz = 20.0 * s;
    let tw2 = text_w_px(&tip, fsz);
    rr_fill((w - tw2) * 0.5 - 18.0 * s, h * 0.915 - 20.0 * s, tw2 + 36.0 * s, 32.0 * s, 16.0 * s, rgba(0.0, 0.03, 0.06, 0.55 * a));
    draw_text(&tip, (w - tw2) * 0.5, h * 0.915, fsz, rgba(0.88, 0.95, 0.97, a));
}

// ---------------------------------------------------------------------------
// Tab bodies (shared by the main menu and the in-game pause menu)
// ---------------------------------------------------------------------------
const NOTE: Color = Color::new(0.65, 0.70, 0.75, 1.0);

pub fn tab_graphics(ui: &mut Ui, st: &mut Settings, x: f32, w: f32) {
    ui.text("Graphics", x, 80.0, 30.0, WHITE);
    let mut p = st.preset.min(3);
    let names: Vec<String> = PRESETS.iter().map(|s| s.to_string()).collect();
    if ui.cycle(x, 140.0, w, "Quality preset", &mut p, &names) {
        st.apply_preset(p);
    }
    let dd_txt = format!("{:.0} m", st.draw_dist);
    if ui.slider(1, x, 200.0, w, "Draw distance", &mut st.draw_dist, 80.0, 800.0, &dd_txt) {
        st.preset = 4;
    }
    let mut wq = st.water_q;
    let wn: Vec<String> = ["Low", "Medium", "High"].iter().map(|s| s.to_string()).collect();
    if ui.cycle(x, 260.0, w, "Water detail", &mut wq, &wn) {
        st.water_q = wq;
        st.preset = 4;
    }
    if ui.toggle(x, 320.0, w, "Surface bump detail", &mut st.bump) {
        st.preset = 4;
    }
    let mut dl = st.detail.min(3);
    if ui.cycle(x, 380.0, w, "Model detail", &mut dl, &names) {
        st.detail = dl;
    }
    ui.text("Applies the next time the world loads", x, 420.0, 16.0, NOTE);
    let fov_txt = format!("{:.0}", st.fov);
    ui.slider(2, x, 444.0, w, "Field of view", &mut st.fov, 55.0, 110.0, &fov_txt);
    ui.toggle(x, 496.0, w, "Show FPS counter", &mut st.show_fps);
    ui.toggle(x, 536.0, w, "VSync (smoother, restart to apply)", &mut st.vsync);
    ui.toggle(x, 576.0, w, "Camcorder look (grain, scan lines, tracking)", &mut st.camcorder);
    let vol_txt = format!("{:.0}%", st.volume * 100.0);
    ui.slider(4, x, 616.0, w, "Master volume", &mut st.volume, 0.0, 1.0, &vol_txt);
}

pub fn tab_display(ui: &mut Ui, st: &mut Settings, x: f32, w: f32) {
    ui.text("Display", x, 80.0, 30.0, WHITE);
    let rn: Vec<String> = RESOLUTIONS.iter().map(|(a, b)| format!("{}x{}", a, b)).collect();
    let mut r = st.res.min(RESOLUTIONS.len() - 1);
    if ui.cycle(x, 140.0, w, "Window resolution", &mut r, &rn) {
        st.res = r;
        if !st.fullscreen {
            request_new_screen_size(RESOLUTIONS[r].0 as f32, RESOLUTIONS[r].1 as f32);
        }
    }
    let mut fsc = st.fullscreen;
    if ui.toggle(x, 200.0, w, "Fullscreen (F11)", &mut fsc) {
        st.fullscreen = fsc;
        set_fullscreen(fsc);
        if !fsc {
            request_new_screen_size(RESOLUTIONS[st.res].0 as f32, RESOLUTIONS[st.res].1 as f32);
        }
    }
    let fn_: Vec<String> = FPS_OPTS.iter().map(|f| if *f == 0 { "Unlimited".to_string() } else { format!("{} FPS", f) }).collect();
    let mut fp = st.fps.min(FPS_OPTS.len() - 1);
    if ui.cycle(x, 260.0, w, "Frame rate limit", &mut fp, &fn_) {
        st.fps = fp;
    }
    ui.toggle(x, 320.0, w, "Show clock", &mut st.show_clock);
    ui.toggle(x, 380.0, w, "Show help hints", &mut st.show_hints);
    let sens_txt = format!("{:.2}", st.sens);
    ui.slider(3, x, 440.0, w, "Mouse sensitivity", &mut st.sens, 0.2, 3.0, &sens_txt);
    ui.toggle(x, 500.0, w, "Invert mouse Y", &mut st.invert_y);
    ui.toggle(x, 548.0, w, "Check for updates on start", &mut st.auto_update);
    update_row(ui, st, x, 596.0, w);
}

fn rebind_guard(ui: &Ui, i: usize, x: f32) -> bool {
    let col = i / 8;
    let row = i % 8;
    ui.hover(x + col as f32 * 330.0, 130.0 + row as f32 * 56.0 + 24.0, 300.0, 28.0)
}

pub fn tab_controls(ui: &mut Ui, st: &mut Settings, rebinding: &mut Option<usize>, x: f32) {
    ui.text("Controls  (click a button, then press a key or mouse button)", x, 82.0, 24.0, WHITE);
    if let Some(i) = *rebinding {
        let mut got: Option<Bind> = None;
        for (_, k) in KEYS {
            if is_key_pressed(*k) {
                got = Some(Bind::Key(*k));
            }
        }
        for m in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            if is_mouse_button_pressed(m) {
                got = Some(Bind::Mouse(m));
            }
        }
        if is_key_pressed(KeyCode::Escape) {
            *rebinding = None;
        } else if let Some(b) = got {
            // a click on the button itself must not be captured as the new binding
            if !(matches!(b, Bind::Mouse(MouseButton::Left)) && ui.clicked && rebind_guard(ui, i, x)) {
                st.binds[i] = b;
                *rebinding = None;
                st.save();
            }
        }
    }
    for (i, a) in ACTS.iter().enumerate() {
        let col = i / 8;
        let row = i % 8;
        let bx = x + col as f32 * 330.0;
        let by = 130.0 + row as f32 * 56.0;
        ui.text(act_label(*a), bx, by + 4.0, 17.0, Color::new(0.85, 0.9, 0.95, 1.0));
        let lab = if *rebinding == Some(i) { "press a key...".to_string() } else { bind_label(st.binds[i]) };
        if ui.button(bx, by + 24.0, 300.0, 28.0, &lab, *rebinding != Some(i), *rebinding == Some(i)) {
            *rebinding = Some(i);
            ui.clicked = false;
        }
    }
    if ui.button(x, 590.0, 220.0, 34.0, "Reset to defaults", true, false) {
        st.binds = ACTS.iter().map(|a| default_bind(*a)).collect();
        st.save();
    }
}

/// A one-line text box for the player's name. Returns true when the text changed.
#[allow(clippy::too_many_arguments)]
pub fn name_field(ui: &mut Ui, name: &mut String, focus: &mut bool, x: f32, y: f32, w: f32, h: f32, t: f32) -> bool {
    ui.rect(x, y, w, h, Color::new(0.1, 0.12, 0.16, 1.0));
    ui.outline(x, y, w, h, 2.0, if *focus { COL_ACCENT } else { Color::new(1.0, 1.0, 1.0, 0.25) });
    if ui.clicked {
        *focus = ui.hover(x, y, w, h);
    }
    let empty = name.is_empty() && !*focus;
    let shown = if empty { "Type your name".to_string() } else { format!("{}{}", name, if *focus && (t * 2.0) as i32 % 2 == 0 { "|" } else { "" }) };
    ui.text(&shown, x + 12.0, y + (h - 22.0) * 0.5 - 1.0, 22.0, if empty { Color::new(1.0, 1.0, 1.0, 0.3) } else { WHITE });
    let mut changed = false;
    if *focus {
        while let Some(ch) = get_char_pressed() {
            if (ch.is_alphanumeric() || matches!(ch, ' ' | '_' | '-' | '.')) && name.chars().count() < 16 {
                name.push(ch);
                changed = true;
            }
        }
        if is_key_pressed(KeyCode::Backspace) && name.pop().is_some() {
            changed = true;
        }
        if is_key_pressed(KeyCode::Enter) {
            *focus = false;
        }
    }
    changed
}

/// Character customization controls plus the live 3D preview. `mat` is the world material.
/// Draws a heading at y=66, 13 option rows from y=124 (pitch 36) on the left and a preview
/// (276x480 design px, y=112) with Randomize / Reset below it at the right edge of `x..x+w`.
pub fn tab_character(ui: &mut Ui, look: &mut Look, name: &mut String, name_focus: &mut bool, x: f32, w: f32, time: f32, mat: &Material) -> bool {
    use crate::people::{draw_preview, field_count, field_get, field_label, field_options, field_set};
    ui.text("Character", x, 66.0, 30.0, WHITE);
    ui.text("Name", x + 180.0, 72.0, 20.0, Color::new(0.88, 0.92, 0.96, 1.0));
    let name_changed = name_field(ui, name, name_focus, x + 240.0, 62.0, 250.0, 40.0, time);
    let pw = 276.0f32.min(w * 0.45);
    let ph = pw * 480.0 / 276.0;
    let rows_w = (w - pw - 24.0).min(480.0);
    let (prx, pry) = (x + w - pw, 112.0);
    let dt = get_frame_time().min(0.05);

    // option rows
    let bw = 176.0;
    for i in 0..field_count() {
        let y = 124.0 + i as f32 * 36.0;
        if i % 2 == 0 {
            ui.rect(x - 6.0, y - 2.0, rows_w + 12.0, 34.0, Color::new(1.0, 1.0, 1.0, 0.035));
        }
        ui.text(field_label(i), x, y + 5.0, 20.0, Color::new(0.88, 0.92, 0.96, 1.0));
        let opts = field_options(i, look);
        let n = opts.len().max(1);
        let mut cur = field_get(look, i).min(n - 1);
        let bx = x + rows_w - bw;
        let mut changed = false;
        if ui.button(bx, y, 32.0, 30.0, "<", true, false) {
            cur = (cur + n - 1) % n;
            changed = true;
        }
        ui.rect(bx + 35.0, y, bw - 70.0, 30.0, Color::new(0.1, 0.12, 0.16, 1.0));
        if let Some(name) = opts.get(cur) {
            let mut sz = 18.0;
            while sz > 11.0 && ui.text_w(name, sz) > bw - 80.0 {
                sz -= 1.0;
            }
            ui.text_c(name, bx + bw * 0.5, y + 15.0, sz, WHITE);
        }
        if ui.button(bx + bw - 32.0, y, 32.0, 30.0, ">", true, false) {
            cur = (cur + 1) % n;
            changed = true;
        }
        if changed {
            field_set(look, i, cur);
        }
    }

    // preview area
    ui.rect(prx, pry, pw, ph, Color::new(0.04, 0.06, 0.09, 0.92));
    ui.outline(prx, pry, pw, ph, 2.0, Color::new(1.0, 1.0, 1.0, 0.2));
    let (mut yaw, mut dragging, mut last_x) = PREVIEW.with(|c| c.get());
    let (mxp, _) = mouse_position();
    if ui.clicked && ui.hover(prx, pry, pw, ph) {
        dragging = true;
        last_x = mxp;
    }
    if dragging {
        if ui.down {
            yaw += (mxp - last_x) * 0.012;
            last_x = mxp;
        } else {
            dragging = false;
        }
    } else {
        yaw += dt * 0.5;
    }
    PREVIEW.with(|c| c.set((yaw, dragging, last_x)));
    let rect = (ui.ox + prx * ui.scale, ui.oy + pry * ui.scale, pw * ui.scale, ph * ui.scale);
    draw_preview(look, yaw, time, mat, rect);
    ui.text_c("drag to rotate", prx + pw * 0.5, pry + ph - 14.0, 15.0, Color::new(1.0, 1.0, 1.0, 0.45));
    let bw2 = pw * 0.5 - 4.0;
    if ui.button(prx, pry + ph + 12.0, bw2, 34.0, "Randomize", true, false) {
        look.randomize();
    }
    if ui.button(prx + pw - bw2, pry + ph + 12.0, bw2, 34.0, "Reset", true, false) {
        *look = Look::default_for(0);
    }
    name_changed
}

// ---------------------------------------------------------------------------
// Main menu
// ---------------------------------------------------------------------------
/// What the player picked in the main menu.
pub enum Launch {
    Solo,
    Host,
    Join(String),
}

pub struct MenuChoice {
    /// true = start a fresh game in slot (ignore any save there)
    pub new_game: bool,
    pub launch: Launch,
    /// which of the five save slots this game uses
    pub slot: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Main,
    Multi,
    Character,
    Settings,
    Slots,
}

const MB_X: f32 = 88.0;
const MB_W: f32 = 310.0;
const MB_H: f32 = 50.0;

/// kind: 0 normal, 1 selected/accent, 2 warning
fn menu_button(ui: &Ui, anim: &mut f32, x: f32, y: f32, w: f32, h: f32, label: &str, kind: u8, dt: f32) -> bool {
    let hot = ui.hover(x, y, w, h);
    *anim += ((if hot { 1.0 } else { 0.0 }) - *anim) * (1.0 - (-14.0 * dt).exp());
    let a = *anim;
    let (top, bot) = match kind {
        1 => (rgba(0.16, 0.44, 0.40, 0.92), rgba(0.10, 0.32, 0.30, 0.94)),
        2 => (rgba(0.46, 0.20, 0.14, 0.94), rgba(0.34, 0.13, 0.10, 0.96)),
        _ => (rgba(0.10, 0.15, 0.22, 0.78), rgba(0.06, 0.10, 0.15, 0.84)),
    };
    let hot_c = rgba(0.16, 0.40, 0.42, 0.95);
    let (top, bot) = (lerp_c(top, hot_c, a * 0.7), lerp_c(bot, hot_c, a * 0.5));
    let (sx, sy, sw, sh) = (ui.ox + x * ui.scale, ui.oy + y * ui.scale, w * ui.scale, h * ui.scale);
    let sc = ui.scale;
    rr_fill(sx + 2.0 * sc, sy + 3.0 * sc, sw, sh, 8.0 * sc, rgba(0.0, 0.0, 0.0, 0.25));
    rr_grad(sx, sy, sw, sh, 8.0 * sc, top, bot);
    rr_line(sx, sy, sw, sh, 8.0 * sc, 1.5 * sc, rgba(1.0, 1.0, 1.0, 0.14 + 0.30 * a));
    let acc = if kind == 2 { rgb(0.95, 0.5, 0.35) } else { COL_ACCENT };
    let bwid = (4.0 + 5.0 * a) * sc;
    draw_rectangle(sx + 7.0 * sc, sy + 9.0 * sc, bwid, sh - 18.0 * sc, with_a(acc, 0.55 + 0.45 * a));
    ui.text_c(label, x + w * 0.5 + 6.0 * a + 6.0, y + h * 0.5, 24.0, lerp_c(rgb(0.86, 0.90, 0.94), WHITE, a));
    hot && ui.clicked
}

fn wrap_lines(ui: &Ui, s: &str, size: f32, maxw: f32) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let trial = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
        if ui.text_w(&trial, size) > maxw && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = trial;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn panel(ui: &Ui, x: f32, y: f32, w: f32, h: f32) {
    ui.panel(x, y, w, h, COL_ACCENT);
}

fn sanitize_code(s: &str) -> String {
    s.trim().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '.' || *c == ':').take(26).collect::<String>().to_ascii_uppercase()
}

/// The title screen. Runs its own frame loop (calling `next_frame().await`) until the player picks
/// Play / Host / Join. It owns Settings editing (Graphics, Display, Controls) and the character
/// customization screen. `Quit` saves the settings and exits the process.
pub async fn main_menu(ui: &mut Ui, st: &mut Settings, look: &mut Look, mat: &Material) -> MenuChoice {
    show_mouse(true);
    set_cursor_grab(false);
    let t0 = get_time();
    let mut page = Page::Main;
    let mut tab = 0usize;
    let mut rebinding: Option<usize> = None;
    let mut join_text = String::new();
    let mut join_focus = false;
    let mut name_focus = false;
    let mut status = String::new();
    let mut hov = [0.0f32; 16];
    let mut confirm_del: Option<(usize, f32)> = None;
    // the occasional deep bloop from the dark water
    let bloop = macroquad::audio::load_sound_from_bytes(&bloop_wav()).await.ok();
    let mut next_bloop = 4.0f32;
    let mut bloop_n = 0u32;
    loop {
        set_default_camera();
        if let Some(snd) = &bloop {
            if (get_time() - t0) as f32 > next_bloop {
                macroquad::audio::play_sound(snd, macroquad::audio::PlaySoundParams { looped: false, volume: 0.7 });
                bloop_n += 1;
                let r = ((bloop_n as f32 * 12.9898).sin() * 43758.5453).fract().abs();
                next_bloop = (get_time() - t0) as f32 + 9.0 + 11.0 * r;
            }
        }
        let dt = get_frame_time().min(0.05);
        let t = (get_time() - t0) as f32;
        ui.begin();
        if is_key_pressed(KeyCode::F11) {
            st.fullscreen = !st.fullscreen;
            set_fullscreen(st.fullscreen);
            if !st.fullscreen {
                request_new_screen_size(RESOLUTIONS[st.res].0 as f32, RESOLUTIONS[st.res].1 as f32);
            }
        }
        let esc = is_key_pressed(KeyCode::Escape) && rebinding.is_none();
        draw_title_scene(t);
        if page != Page::Character {
            update_banner(ui);
        }

        // ---- logo ----
        if page != Page::Character {
            let (lsz, lsp) = (116.0 * ui.scale, 9.0 * ui.scale);
            let lx = ui.ox + MB_X * ui.scale - 4.0 * ui.scale;
            let ly = ui.oy + 150.0 * ui.scale;
            draw_spaced("ISLAND", lx, ly, lsz, lsp, rgb(0.97, 0.94, 0.86), 4.0 * ui.scale, Some(t));
            let lw = spaced_width("ISLAND", lsz, lsp);
            grad_h(lx, ly + 14.0 * ui.scale, lw, 3.0 * ui.scale, rgba(0.3, 0.8, 0.7, 0.9), rgba(0.3, 0.8, 0.7, 0.0));
            ui.text("A castaway fishing story", MB_X, 178.0, 24.0, rgb(0.98, 0.84, 0.66));
        }

        let mut hi = 0usize;
        let mut by = 232.0f32;
        let pitch = 58.0f32;
        let mut go: Option<MenuChoice> = None;
        match page {
            Page::Main => {
                macro_rules! mb {
                    ($label:expr, $kind:expr) => {{
                        let r = menu_button(ui, &mut hov[hi], MB_X, by, MB_W, MB_H, $label, $kind, dt);
                        hi += 1;
                        by += pitch;
                        r
                    }};
                }
                if mb!("Play", 1) {
                    page = Page::Slots;
                    confirm_del = None;
                }
                if mb!("Multiplayer", 0) {
                    page = Page::Multi;
                    status.clear();
                }
                if mb!("Character", 0) {
                    page = Page::Character;
                }
                if mb!("Settings", 0) {
                    page = Page::Settings;
                    tab = 0;
                    rebinding = None;
                }
                if mb!("Quit", 0) {
                    st.save();
                    std::process::exit(0);
                }
                // the player's name (shown to friends in multiplayer)
                ui.text("Your name", MB_X, 596.0, 18.0, rgb(0.86, 0.90, 0.94));
                let was_focus = name_focus;
                name_field(ui, &mut st.name, &mut name_focus, MB_X, 620.0, MB_W, 42.0, t);
                if was_focus && !name_focus {
                    st.name = crate::net::clean_name(&st.name);
                    st.save();
                }
                ui.text(&format!("v{}  -  Esc: back   F11: fullscreen", crate::update::CURRENT), MB_X, 690.0, 15.0, rgba(1.0, 1.0, 1.0, 0.4));
            }
            Page::Slots => {
                if menu_button(ui, &mut hov[0], MB_X, 600.0, MB_W, MB_H, "Back", 0, dt) || esc {
                    page = Page::Main;
                }
                panel(ui, 420.0, 46.0, 720.0, 630.0);
                let x = 450.0;
                ui.text("Choose a save slot", x, 70.0, 30.0, WHITE);
                ui.text("Five separate islands. In multiplayer the host's slot is the world; yours keeps your own gear and progress.", x, 106.0, 16.0, NOTE);
                for i in 0..crate::game::SLOTS {
                    let y = 138.0 + i as f32 * 100.0;
                    ui.rect(x, y, 660.0, 90.0, Color::new(0.10, 0.13, 0.18, 1.0));
                    ui.outline(x, y, 660.0, 90.0, 1.5, if st.slot == i { COL_ACCENT } else { Color::new(1.0, 1.0, 1.0, 0.14) });
                    ui.text(&format!("Slot {}", i + 1), x + 16.0, y + 10.0, 26.0, WHITE);
                    match crate::game::slot_info(i) {
                        Some(info) => {
                            ui.text(&format!("Day {}   -   Level {}   -   {} coins", info.day, info.level, info.coins), x + 16.0, y + 44.0, 20.0, Color::new(0.8, 0.9, 1.0, 1.0));
                            ui.text(&format!("{}   -   saved {}", info.rod, info.saved), x + 16.0, y + 66.0, 16.0, NOTE);
                            if ui.button(x + 430.0, y + 22.0, 120.0, 46.0, "Continue", true, true) {
                                st.slot = i;
                                go = Some(MenuChoice { new_game: false, launch: Launch::Solo, slot: i });
                            }
                            let armed = matches!(confirm_del, Some((s, tm)) if s == i && t - tm < 4.0);
                            if ui.button(x + 560.0, y + 22.0, 90.0, 46.0, if armed { "Sure?" } else { "Delete" }, true, false) {
                                if armed {
                                    crate::game::delete_slot(i);
                                    confirm_del = None;
                                } else {
                                    confirm_del = Some((i, t));
                                }
                            }
                        }
                        None => {
                            ui.text("Empty", x + 16.0, y + 48.0, 20.0, Color::new(1.0, 1.0, 1.0, 0.4));
                            if ui.button(x + 430.0, y + 22.0, 220.0, 46.0, "New game", true, true) {
                                st.slot = i;
                                go = Some(MenuChoice { new_game: true, launch: Launch::Solo, slot: i });
                            }
                        }
                    }
                }
            }
            Page::Settings => {
                let names = ["Graphics", "Display", "Controls"];
                for (i, n) in names.iter().enumerate() {
                    if menu_button(ui, &mut hov[hi], MB_X, by, MB_W, MB_H, n, if tab == i { 1 } else { 0 }, dt) {
                        tab = i;
                        rebinding = None;
                    }
                    hi += 1;
                    by += pitch;
                }
                if menu_button(ui, &mut hov[hi], MB_X, 600.0, MB_W, MB_H, "Back", 0, dt) || esc {
                    st.save();
                    rebinding = None;
                    page = Page::Main;
                }
                panel(ui, 420.0, 46.0, 720.0, 630.0);
                match tab {
                    0 => tab_graphics(ui, st, 450.0, 640.0),
                    1 => tab_display(ui, st, 450.0, 640.0),
                    _ => tab_controls(ui, st, &mut rebinding, 450.0),
                }
            }
            Page::Multi => {
                if menu_button(ui, &mut hov[0], MB_X, 600.0, MB_W, MB_H, "Back", 0, dt) || esc {
                    page = Page::Main;
                    join_focus = false;
                }
                panel(ui, 420.0, 46.0, 720.0, 630.0);
                let x = 450.0;
                ui.text("Multiplayer (up to 4 players)", x, 76.0, 30.0, WHITE);
                if ui.button(x, 130.0, 280.0, 50.0, "Host a game", true, true) {
                    go = Some(MenuChoice { new_game: !crate::game::slot_exists(st.slot), launch: Launch::Host, slot: st.slot });
                }
                let mut ly = 136.0;
                for l in wrap_lines(ui, "Start a game on this computer. Once you are on the island the game shows you a LAN code for friends on your network and an internet code for friends anywhere.", 17.0, 330.0) {
                    ui.text(&l, x + 300.0, ly, 17.0, NOTE);
                    ly += 21.0;
                }
                ui.text("Save slot (* = has a save)", x, 188.0, 16.0, rgb(0.8, 0.85, 0.9));
                for i in 0..crate::game::SLOTS {
                    let label = format!("{}{}", i + 1, if crate::game::slot_exists(i) { "*" } else { "" });
                    if ui.button(x + i as f32 * 56.0, 204.0, 50.0, 26.0, &label, true, st.slot == i) {
                        st.slot = i;
                    }
                }
                ui.rect(x, 238.0, 640.0, 1.0, rgba(1.0, 1.0, 1.0, 0.12));
                ui.text("Join a friend's game", x, 252.0, 24.0, WHITE);
                ui.text("Room code (or the host's IP address):", x, 292.0, 19.0, rgb(0.8, 0.85, 0.9));
                ui.rect(x, 322.0, 360.0, 48.0, Color::new(0.1, 0.12, 0.16, 1.0));
                ui.outline(x, 322.0, 360.0, 48.0, 2.0, if join_focus { COL_ACCENT } else { Color::new(1.0, 1.0, 1.0, 0.25) });
                if ui.clicked {
                    join_focus = ui.hover(x, 322.0, 360.0, 48.0);
                }
                let empty = join_text.is_empty() && !join_focus;
                let shown = if empty { "ABCDE-FGHJK".to_string() } else { format!("{}{}", join_text, if join_focus && (t * 2.0) as i32 % 2 == 0 { "|" } else { "" }) };
                ui.text(&shown, x + 14.0, 331.0, 28.0, if empty { Color::new(1.0, 1.0, 1.0, 0.3) } else { WHITE });
                let mut join_now = false;
                if join_focus {
                    while let Some(ch) = get_char_pressed() {
                        if (ch.is_ascii_alphanumeric() || ch == '-' || ch == '.' || ch == ':') && join_text.len() < 26 {
                            join_text.push(ch.to_ascii_uppercase());
                        }
                    }
                    if is_key_pressed(KeyCode::Backspace) {
                        join_text.pop();
                    }
                    if (is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)) && is_key_pressed(KeyCode::V) {
                        if let Some(c) = miniquad::window::clipboard_get() {
                            join_text = sanitize_code(&c);
                        }
                    }
                    if is_key_pressed(KeyCode::Enter) {
                        join_now = true;
                    }
                } else {
                    while get_char_pressed().is_some() {}
                }
                if ui.button(x + 376.0, 322.0, 130.0, 48.0, "Join", !join_text.is_empty(), true) {
                    join_now = true;
                }
                if join_now {
                    if join_text.is_empty() {
                        status = "Type a room code first.".into();
                    } else {
                        go = Some(MenuChoice { new_game: !crate::game::slot_exists(st.slot), launch: Launch::Join(join_text.clone()), slot: st.slot });
                    }
                }
                ui.text("Click the box and type, or press Ctrl+V to paste.", x, 380.0, 16.0, NOTE);
                if !status.is_empty() {
                    ui.text(&status, x, 410.0, 20.0, Color::new(1.0, 0.85, 0.5, 1.0));
                }
                let mut ly = 470.0;
                for line in [
                    "Hosting and joining work on the same network or over the internet - the host just shares the code the game shows. No router setup needed.",
                    "Windows may ask to allow the game through the firewall when you host.",
                ] {
                    for l in wrap_lines(ui, line, 17.0, 640.0) {
                        ui.text(&l, x, ly, 17.0, NOTE);
                        ly += 22.0;
                    }
                    ly += 8.0;
                }
            }
            Page::Character => {
                panel(ui, 190.0, 30.0, 900.0, 660.0);
                tab_character(ui, look, &mut st.name, &mut name_focus, 230.0, 820.0, t, mat);
                if ui.button(1090.0 - 140.0, 52.0, 120.0, 38.0, "Done", true, true) || esc {
                    st.name = crate::net::clean_name(&st.name);
                    name_focus = false;
                    st.look = look.encode();
                    st.save();
                    page = Page::Main;
                }
            }
        }

        // fade in
        let fade = 1.0 - t * 1.6;
        if fade > 0.0 {
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), rgba(0.0, 0.0, 0.0, fade));
        }
        if let Some(c) = go {
            st.look = look.encode();
            st.save();
            return c;
        }
        next_frame().await;
    }
}

// ===========================================================================
// Updates (GitHub releases, see update.rs)
// ===========================================================================

/// Small banner on the title screen: shows an available update, its download and restart.
pub fn update_banner(ui: &mut Ui) {
    use crate::update::{self, State};
    if update::take_auto_install() {
        if let Err(e) = update::apply_and_restart() {
            update::report(&e);
        }
    }
    let (x, y, w) = (845.0f32, 22.0f32, 405.0f32);
    let gold = Color::new(1.0, 0.9, 0.6, 1.0);
    let grey = Color::new(0.85, 0.88, 0.92, 0.8);
    let panel = |ui: &mut Ui, h: f32| {
        ui.rect(x, y, w, h, Color::new(0.04, 0.08, 0.11, 0.9));
        ui.outline(x, y, w, h, 1.5, Color::new(1.0, 0.9, 0.6, 0.45));
    };
    match update::state() {
        State::Available { tag, .. } => {
            // "Update to the latest version 0.5.1" with a small green bold UPDATE button
            let label = format!("Update to the latest version {}", tag.trim_start_matches(['v', 'V']));
            let lw = ui.text_w(&label, 20.0);
            let bw = 104.0;
            let total = lw + 16.0 + bw;
            let (bx, by) = (1262.0 - bw, 20.0);
            ui.rect(1262.0 - total - 14.0, by - 8.0, total + 28.0, 50.0, Color::new(0.03, 0.06, 0.09, 0.82));
            ui.text(&label, 1262.0 - total - 2.0, by + 6.0, 20.0, gold);
            if green_update_button(ui, bx, by, bw, 34.0) {
                update::download_and_install();
            }
        }
        State::Downloading { tag } => {
            let dots = ".".repeat(((get_time() * 2.0) as usize) % 4);
            let msg = format!("Downloading {}{}", tag.trim_start_matches(['v', 'V']), dots);
            let w2 = ui.text_w(&msg, 20.0);
            ui.rect(1262.0 - w2 - 32.0, 12.0, w2 + 44.0, 42.0, Color::new(0.03, 0.06, 0.09, 0.82));
            ui.text(&msg, 1262.0 - w2 - 14.0, 22.0, 20.0, gold);
        }
        State::Ready { tag } => {
            // the install normally starts by itself right after the download
            panel(ui, 70.0);
            ui.text(&format!("{} is ready", tag.trim_start_matches(['v', 'V'])), x + 14.0, y + 8.0, 20.0, gold);
            if ui.button(x + 14.0, y + 34.0, 160.0, 28.0, "Restart now", true, true) {
                if let Err(e) = update::apply_and_restart() {
                    update::report(&e);
                }
            }
        }
        State::Checking => ui.text("Checking for updates...", x + 150.0, y + 4.0, 16.0, grey),
        State::Failed(m) => {
            let m: String = m.chars().take(70).collect();
            ui.text(&m, x - 60.0, y + 4.0, 15.0, Color::new(1.0, 0.8, 0.5, 0.9));
        }
        _ => {}
    }
}

/// One row of the Display tab: current update status plus the button that fits it.
pub fn update_row(ui: &mut Ui, st: &Settings, x: f32, y: f32, w: f32) {
    use crate::update::{self, State};
    let bx = x + w - 210.0;
    let grey = Color::new(0.8, 0.85, 0.9, 0.9);
    match update::state() {
        State::Available { tag, .. } => {
            ui.text(&format!("Version {} is available", tag), x, y + 6.0, 20.0, Color::new(1.0, 0.9, 0.6, 1.0));
            if ui.button(bx, y, 210.0, 34.0, "Update now", true, true) {
                update::download_and_install();
            }
        }
        State::Downloading { .. } => ui.text("Downloading the update...", x, y + 6.0, 20.0, grey),
        State::Ready { .. } => {
            ui.text("Update downloaded", x, y + 6.0, 20.0, Color::new(0.6, 1.0, 0.7, 1.0));
            if ui.button(bx, y, 210.0, 34.0, "Restart now", true, true) {
                if let Err(e) = update::apply_and_restart() {
                    update::report(&e);
                }
            }
        }
        other => {
            let msg = match other {
                State::Checking => "Checking...".to_string(),
                State::UpToDate => format!("You're up to date (v{})", update::CURRENT),
                State::Failed(m) => m.chars().take(52).collect(),
                State::Disabled => "No update repository set (update_repo in island_settings.txt)".to_string(),
                _ => format!("Version v{}", update::CURRENT),
            };
            ui.text(&msg, x, y + 6.0, 18.0, grey);
            if ui.button(bx, y, 210.0, 34.0, "Check for updates", true, false) {
                update::check(&st.repo());
            }
        }
    }
}

/// Small green button with bold white "UPDATE" text.
fn green_update_button(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32) -> bool {
    let hot = ui.hover(x, y, w, h);
    let bg = if hot { Color::new(0.24, 0.74, 0.38, 1.0) } else { Color::new(0.16, 0.60, 0.29, 1.0) };
    ui.rect(x, y, w, h, bg);
    ui.outline(x, y, w, h, 1.5, Color::new(0.75, 1.0, 0.8, if hot { 0.9 } else { 0.5 }));
    // bold = the same text drawn a few times, a pixel apart
    let tw = ui.text_w("UPDATE", 22.0);
    for (dx, dy) in [(0.0f32, 0.0f32), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
        ui.text("UPDATE", x + (w - tw) * 0.5 + dx * 0.9, y + 5.0 + dy * 0.9, 22.0, WHITE);
    }
    hot && ui.clicked
}
