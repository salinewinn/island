//! Human character models and customization.
//!
//! Bodies are built from lofted elliptical rings (`loft`) with smooth normals, vertex colour
//! variation and a few blobs.  Model space = world space with the feet as origin; the finished
//! person is scaled about the feet (female body = 0.935).

use super::*;
use crate::items::*;

// ---------------------------------------------------------------------------
// Legacy entry point (still used by game.rs until it switches to draw_person)
// ---------------------------------------------------------------------------
pub struct Outfit {
    pub shirt: Vec3,
    pub pants: Vec3,
    pub skin: Vec3,
    pub hair: Vec3,
    pub hat: Option<Vec3>,
}

pub const PLAYER_COLORS: [[f32; 3]; 4] = [[0.20, 0.50, 0.85], [0.85, 0.30, 0.25], [0.30, 0.70, 0.35], [0.88, 0.70, 0.20]];

#[allow(clippy::too_many_arguments)]
pub fn draw_person_old(mb: &mut MB, feet: Vec3, yaw: f32, _o: &Outfit, phase: f32, walk: f32, slot: Option<u8>, tier: u8, lure: Option<Vec3>, eye: Vec3, time: f32) {
    let look = match slot {
        None => Look::marlo(),
        Some(s) => Look::default_for(s),
    };
    let pose = Pose { yaw, phase, walk, item: slot.and_then(ItemKind::from_u8), tier, lure, lit: true, eye, time };
    draw_person(mb, feet, &look, &pose);
}

// ===========================================================================
// Customization data
// ===========================================================================
const SKIN: [(&str, [f32; 3]); 6] = [
    ("Fair", [0.90, 0.72, 0.62]),
    ("Light", [0.86, 0.66, 0.52]),
    ("Olive", [0.78, 0.58, 0.42]),
    ("Weathered tan", [0.66, 0.46, 0.31]),
    ("Brown", [0.46, 0.30, 0.21]),
    ("Deep brown", [0.31, 0.20, 0.14]),
];
const EYES: [(&str, [f32; 3]); 7] = [
    ("Blue-grey", [0.45, 0.58, 0.68]),
    ("Blue", [0.22, 0.45, 0.82]),
    ("Green", [0.28, 0.55, 0.32]),
    ("Hazel", [0.50, 0.38, 0.18]),
    ("Brown", [0.30, 0.17, 0.08]),
    ("Dark brown", [0.12, 0.07, 0.04]),
    ("Grey", [0.55, 0.58, 0.60]),
];
const HAIRC: [(&str, [f32; 3]); 8] = [
    ("Light brown", [0.42, 0.30, 0.18]),
    ("Dark brown", [0.17, 0.10, 0.07]),
    ("Black", [0.05, 0.045, 0.05]),
    ("Blonde", [0.78, 0.62, 0.30]),
    ("Ginger", [0.58, 0.24, 0.10]),
    ("Grey", [0.62, 0.62, 0.62]),
    ("White", [0.86, 0.86, 0.84]),
    ("Chestnut", [0.30, 0.15, 0.09]),
];
const CLOTH: [(&str, [f32; 3]); 21] = [
    ("Cream", [0.90, 0.85, 0.70]),
    ("White", [0.88, 0.88, 0.86]),
    ("Black", [0.07, 0.07, 0.08]),
    ("Charcoal", [0.22, 0.23, 0.25]),
    ("Grey", [0.50, 0.51, 0.52]),
    ("Navy", [0.10, 0.16, 0.34]),
    ("Denim blue", [0.24, 0.34, 0.52]),
    ("Sky blue", [0.36, 0.60, 0.84]),
    ("Blue", [0.18, 0.42, 0.82]),
    ("Teal", [0.12, 0.45, 0.48]),
    ("Forest green", [0.14, 0.34, 0.18]),
    ("Olive", [0.38, 0.40, 0.18]),
    ("Khaki", [0.64, 0.56, 0.36]),
    ("Sand", [0.78, 0.68, 0.50]),
    ("Brown", [0.34, 0.22, 0.13]),
    ("Mustard", [0.78, 0.58, 0.14]),
    ("Orange", [0.85, 0.40, 0.12]),
    ("Red", [0.72, 0.14, 0.12]),
    ("Dark red", [0.42, 0.07, 0.08]),
    ("Pink", [0.88, 0.55, 0.62]),
    ("Purple", [0.40, 0.20, 0.50]),
];
const HAIR_STYLES: [&str; 10] = ["Short crop", "Tousled", "Side part", "Buzz cut", "Long", "Bob", "Ponytail", "Bun", "Curly", "Bald"];
const BEARDS: [&str; 4] = ["None", "Stubble", "Short beard", "Full beard"];
const TOPS: [&str; 4] = ["T-shirt", "Raglan long sleeve", "Tank top", "Rough linen shirt"];
const BOTTOMS: [&str; 4] = ["Cargo pants", "Jeans", "Shorts", "Rolled-up trousers"];
const SHOES: [&str; 4] = ["Boots", "Sneakers", "Sandals", "Barefoot"];
const HATS: [&str; 4] = ["None", "Straw hat", "Cap", "Sailor's cap"];

fn v3(a: [f32; 3]) -> Vec3 {
    vec3(a[0], a[1], a[2])
}
/// worn, slightly muted clothing colour
fn cl(i: u8) -> Vec3 {
    let c = v3(CLOTH[(i as usize).min(CLOTH.len() - 1)].1);
    let g = (c.x + c.y + c.z) / 3.0;
    (c * 0.86 + Vec3::splat(g) * 0.14) * (0.97 - 0.17 * smooth(0.5, 0.88, g))
}

/// Everything about a character's appearance that the player can customize.
/// Every field is an index into a list of options (see `field_*`).
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub female: bool,
    pub skin: u8,
    pub eyes: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    pub beard: u8,
    pub top_style: u8,
    pub top_color: u8,
    pub sleeve_color: u8,
    pub bottom_style: u8,
    pub bottom_color: u8,
    pub shoe_style: u8,
    pub hat: u8,
}

impl Look {
    /// A distinct default look per player slot (0..3).
    pub fn default_for(slot: u8) -> Look {
        match slot % 4 {
            0 => Look { female: false, skin: 1, eyes: 0, hair_style: 1, hair_color: 0, beard: 1, top_style: 1, top_color: 0, sleeve_color: 18, bottom_style: 0, bottom_color: 12, shoe_style: 0, hat: 0 },
            1 => Look { female: true, skin: 0, eyes: 4, hair_style: 5, hair_color: 1, beard: 0, top_style: 2, top_color: 8, sleeve_color: 8, bottom_style: 1, bottom_color: 6, shoe_style: 1, hat: 0 },
            2 => Look { female: false, skin: 3, eyes: 3, hair_style: 0, hair_color: 2, beard: 2, top_style: 0, top_color: 15, sleeve_color: 15, bottom_style: 2, bottom_color: 13, shoe_style: 2, hat: 0 },
            _ => Look { female: true, skin: 4, eyes: 5, hair_style: 6, hair_color: 2, beard: 0, top_style: 0, top_color: 9, sleeve_color: 9, bottom_style: 3, bottom_color: 13, shoe_style: 3, hat: 2 },
        }
    }
    /// The Hermit (shopkeeper): an old castaway.
    pub fn marlo() -> Look {
        Look { female: false, skin: 3, eyes: 6, hair_style: 4, hair_color: 5, beard: 3, top_style: 3, top_color: 13, sleeve_color: 14, bottom_style: 3, bottom_color: 11, shoe_style: 3, hat: 1 }
    }
    /// Compact text form, safe for network lines and the save file (digits and dots only).
    pub fn encode(&self) -> String {
        format!(
            "{}.{}.{}.{}.{}.{}.{}.{}.{}.{}.{}.{}.{}",
            self.female as u8, self.skin, self.eyes, self.hair_style, self.hair_color, self.beard, self.top_style, self.top_color, self.sleeve_color, self.bottom_style, self.bottom_color, self.shoe_style, self.hat
        )
    }
    pub fn decode(s: &str) -> Option<Look> {
        let v: Vec<u8> = s.trim().split('.').map(|p| p.parse::<u8>().ok()).collect::<Option<Vec<u8>>>()?;
        if v.len() != 13 {
            return None;
        }
        let c = |i: usize, x: u8| x.min((nopt(i) - 1) as u8);
        Some(Look {
            female: v[0] != 0,
            skin: c(1, v[1]),
            eyes: c(2, v[2]),
            hair_style: c(3, v[3]),
            hair_color: c(4, v[4]),
            beard: c(5, v[5]),
            top_style: c(6, v[6]),
            top_color: c(7, v[7]),
            sleeve_color: c(8, v[8]),
            bottom_style: c(9, v[9]),
            bottom_color: c(10, v[10]),
            shoe_style: c(11, v[11]),
            hat: c(12, v[12]),
        })
    }
    pub fn randomize(&mut self) {
        for i in 0..field_count() {
            let n = nopt(i).max(1);
            field_set(self, i, rand::gen_range(0, n as i32) as usize);
        }
    }
}

fn nopt(i: usize) -> usize {
    [2, SKIN.len(), EYES.len(), HAIR_STYLES.len(), HAIRC.len(), BEARDS.len(), TOPS.len(), CLOTH.len(), CLOTH.len(), BOTTOMS.len(), CLOTH.len(), SHOES.len(), HATS.len()][i.min(12)]
}

/// Customization fields shown in the character screen, in order.
pub fn field_count() -> usize {
    13
}
pub fn field_label(i: usize) -> &'static str {
    ["Body type", "Skin tone", "Eye colour", "Hairstyle", "Hair colour", "Facial hair", "Top", "Top colour", "Sleeve / accent colour", "Bottoms", "Bottoms colour", "Shoes", "Hat"][i.min(12)]
}
/// Names of the options for field `i` (may depend on the current body type).
pub fn field_options(i: usize, _look: &Look) -> Vec<String> {
    let l = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let p = |a: &[(&str, [f32; 3])]| a.iter().map(|s| s.0.to_string()).collect::<Vec<_>>();
    match i {
        0 => l(&["Masculine", "Feminine"]),
        1 => p(&SKIN),
        2 => p(&EYES),
        3 => l(&HAIR_STYLES),
        4 => p(&HAIRC),
        5 => l(&BEARDS),
        6 => l(&TOPS),
        7 | 8 | 10 => p(&CLOTH),
        9 => l(&BOTTOMS),
        11 => l(&SHOES),
        _ => l(&HATS),
    }
}
pub fn field_get(l: &Look, i: usize) -> usize {
    match i {
        0 => l.female as usize,
        1 => l.skin as usize,
        2 => l.eyes as usize,
        3 => l.hair_style as usize,
        4 => l.hair_color as usize,
        5 => l.beard as usize,
        6 => l.top_style as usize,
        7 => l.top_color as usize,
        8 => l.sleeve_color as usize,
        9 => l.bottom_style as usize,
        10 => l.bottom_color as usize,
        11 => l.shoe_style as usize,
        _ => l.hat as usize,
    }
}
pub fn field_set(l: &mut Look, i: usize, v: usize) {
    let v8 = v.min(nopt(i) - 1) as u8;
    match i {
        0 => l.female = v != 0,
        1 => l.skin = v8,
        2 => l.eyes = v8,
        3 => l.hair_style = v8,
        4 => l.hair_color = v8,
        5 => l.beard = v8,
        6 => l.top_style = v8,
        7 => l.top_color = v8,
        8 => l.sleeve_color = v8,
        9 => l.bottom_style = v8,
        10 => l.bottom_color = v8,
        11 => l.shoe_style = v8,
        _ => l.hat = v8,
    }
}

/// How the person is posed / what they hold this frame.
pub struct Pose {
    pub yaw: f32,
    /// walk-cycle phase (radians) and how strongly they are walking (0..1)
    pub phase: f32,
    pub walk: f32,
    pub item: Option<ItemKind>,
    /// rod tier (0..2) and where the lure/bobber is when a line is out
    pub tier: u8,
    pub lure: Option<Vec3>,
    /// lantern flame on?
    pub lit: bool,
    pub eye: Vec3,
    pub time: f32,
}

// ===========================================================================
// Geometry helpers
// ===========================================================================
#[derive(Clone, Copy)]
struct R {
    c: Vec3,
    u: Vec3,
    v: Vec3,
    ru: f32,
    rv: f32,
    col: Vec3,
}

/// Ring perpendicular to `dir`; `hint` picks the direction of the `ru` axis.
fn ring(c: Vec3, dir: Vec3, hint: Vec3, ru: f32, rv: f32, col: Vec3) -> R {
    let d = dir.normalize_or_zero();
    let mut u = (hint - d * hint.dot(d)).normalize_or_zero();
    if u == Vec3::ZERO {
        u = d.any_orthonormal_vector();
    }
    R { c, u, v: d.cross(u), ru, rv, col }
}

struct Lf {
    sides: usize,
    cap0: bool,
    cap1: bool,
    start: f32,
    sweep: f32,
}
impl Lf {
    fn c(sides: usize) -> Lf {
        Lf { sides, cap0: false, cap1: false, start: 0.0, sweep: TAU }
    }
    fn caps(mut self, a: bool, b: bool) -> Lf {
        self.cap0 = a;
        self.cap1 = b;
        self
    }
    fn arc(mut self, start: f32, sweep: f32) -> Lf {
        self.start = start;
        self.sweep = sweep;
        self
    }
}

fn id_s(_: usize, _: f32, p: Vec3, _: Vec3) -> Vec3 {
    p
}
fn id_c(_: usize, _: f32, c: Vec3) -> Vec3 {
    c
}

/// Sweep elliptical rings along their centres.  Normals are smoothed from the grid.
fn loft(mb: &mut MB, rings: &[R], o: &Lf, shape: &dyn Fn(usize, f32, Vec3, Vec3) -> Vec3, colf: &dyn Fn(usize, f32, Vec3) -> Vec3) {
    let n = rings.len();
    if n < 2 {
        return;
    }
    let closed = o.sweep >= TAU - 1e-3;
    let sd = o.sides.max(3);
    let m = if closed { sd } else { sd + 1 };
    mb.ensure(n * m + 2 * (m + 1), (n - 1) * sd * 6 + 2 * sd * 3);
    let mut pos = Vec::with_capacity(n * m);
    let mut cols = Vec::with_capacity(n * m);
    let mut ths = Vec::with_capacity(m);
    for j in 0..m {
        ths.push(o.start + o.sweep * j as f32 / sd as f32);
    }
    for (i, r) in rings.iter().enumerate() {
        for &th in &ths {
            let (s, c) = th.sin_cos();
            let p = r.c + r.u * (r.ru * c) + r.v * (r.rv * s);
            pos.push(shape(i, th, p, r.c));
            cols.push(colf(i, th, r.col));
        }
    }
    let mut vlen = vec![0.0f32; n];
    for i in 1..n {
        vlen[i] = vlen[i - 1] + rings[i].c.distance(rings[i - 1].c);
    }
    let base = mb.v.len();
    for i in 0..n {
        for j in 0..m {
            let jp = if closed { (j + 1) % m } else { (j + 1).min(m - 1) };
            let jm = if closed { (j + m - 1) % m } else { j.saturating_sub(1) };
            let tang = pos[i * m + jp] - pos[i * m + jm];
            let along = pos[(i + 1).min(n - 1) * m + j] - pos[i.saturating_sub(1) * m + j];
            let mut nr = tang.cross(along);
            if nr.length_squared() < 1e-14 {
                nr = if i == 0 {
                    rings[0].c - rings[1].c
                } else if i == n - 1 {
                    rings[n - 1].c - rings[n - 2].c
                } else {
                    pos[i * m + j] - rings[i].c
                };
            }
            let (s, c) = ths[j].sin_cos();
            let lit = Lit { c: col3(cols[i * m + j]), n: nr.normalize_or_zero(), w: 0.0 };
            mb.vert_uv(pos[i * m + j], lit, rings[i].ru * c + 0.37 * (sd as f32), vlen[i] + rings[i].rv * s);
        }
    }
    for i in 0..n - 1 {
        for j in 0..sd {
            let jn = if closed { (j + 1) % m } else { j + 1 };
            let a = (base + i * m + j) as u16;
            let b = (base + i * m + jn) as u16;
            let c = (base + (i + 1) * m + j) as u16;
            let d = (base + (i + 1) * m + jn) as u16;
            mb.i.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    for (do_cap, e, ax) in [(o.cap0, 0usize, rings[0].c - rings[1].c), (o.cap1, n - 1, rings[n - 1].c - rings[n - 2].c)] {
        if !do_cap || rings[e].ru.max(rings[e].rv) < 1e-4 {
            continue;
        }
        let nrm = ax.normalize_or_zero();
        let cc = (0..m).fold(Vec3::ZERO, |a, j| a + pos[e * m + j]) / m as f32;
        let ci = mb.vert_uv(cc, Lit { c: col3(rings[e].col), n: nrm, w: 0.0 }, cc.x, cc.z);
        let b0 = mb.v.len();
        for j in 0..m {
            mb.vert_uv(pos[e * m + j], Lit { c: col3(cols[e * m + j]), n: nrm, w: 0.0 }, pos[e * m + j].x, pos[e * m + j].z);
        }
        for j in 0..sd {
            let jn = if closed { (j + 1) % m } else { j + 1 };
            mb.i.extend_from_slice(&[ci, (b0 + j) as u16, (b0 + jn) as u16]);
        }
    }
}

/// Piecewise smooth lookup in a table whose first column is the key.
fn tbl(tb: &[[f32; 4]], x: f32) -> [f32; 4] {
    let n = tb.len();
    if x <= tb[0][0] {
        return tb[0];
    }
    if x >= tb[n - 1][0] {
        return tb[n - 1];
    }
    let mut i = 0;
    while i + 2 < n && x >= tb[i + 1][0] {
        i += 1;
    }
    let (a, b) = (tb[i], tb[i + 1]);
    let t = (x - a[0]) / (b[0] - a[0]);
    let p0 = tb[i.saturating_sub(1)];
    let p3 = tb[(i + 2).min(n - 1)];
    let mut o = [x, 0.0, 0.0, 0.0];
    let (t2, t3) = (t * t, t * t * t);
    for k in 1..4 {
        let m0 = (b[k] - p0[k]) * 0.5;
        let m1 = (p3[k] - a[k]) * 0.5;
        o[k] = (2.0 * t3 - 3.0 * t2 + 1.0) * a[k] + (t3 - 2.0 * t2 + t) * m0 + (-2.0 * t3 + 3.0 * t2) * b[k] + (t3 - t2) * m1;
    }
    o
}

fn hs(a: f32, b: f32) -> f32 {
    let s = (a * 12.9898 + b * 78.233).sin() * 43758.547;
    s - s.floor()
}
fn gs(x: f32, s: f32) -> f32 {
    (-x * x / (2.0 * s * s)).exp()
}
fn sd(i: usize) -> f32 {
    if i == 0 {
        1.0
    } else {
        -1.0
    }
}
fn tint(c: Vec3, k: f32) -> Vec3 {
    c * vec3(1.0 + 0.05 * k, 1.0 - 0.07 * k, 1.0 - 0.08 * k)
}
fn angdiff(th: f32, to: f32) -> f32 {
    ((th - to + PI).rem_euclid(TAU) - PI).abs()
}

struct Fr {
    o: Vec3,
    f: Vec3,
    r: Vec3,
}
impl Fr {
    fn p(&self, x: f32, y: f32, z: f32) -> Vec3 {
        self.o + self.f * x + Vec3::Y * y + self.r * z
    }
}
fn frame(o: Vec3, yaw: f32) -> Fr {
    Fr { o, f: vec3(yaw.cos(), 0.0, yaw.sin()), r: vec3(-yaw.sin(), 0.0, yaw.cos()) }
}

/// Two-bone IK: returns (middle joint, end point).
fn ik2(a: Vec3, t: Vec3, l1: f32, l2: f32, pole: Vec3) -> (Vec3, Vec3) {
    let mut d = t - a;
    let mut dist = d.length();
    let maxd = (l1 + l2) * 0.998;
    if dist > maxd {
        d *= maxd / dist;
        dist = maxd;
    }
    let dist = dist.max(0.05);
    let dir = d / dist;
    let aa = (l1 * l1 - l2 * l2 + dist * dist) / (2.0 * dist);
    let h = (l1 * l1 - aa * aa).max(0.0).sqrt();
    let pp = (pole - dir * pole.dot(dir)).normalize_or_zero();
    (a + dir * aa + pp * h, a + d)
}

fn pt3(a: Vec3, b: Vec3, c: Vec3, t: f32) -> Vec3 {
    let l1 = a.distance(b).max(1e-4);
    let l2 = b.distance(c).max(1e-4);
    let s = t * (l1 + l2);
    if s <= l1 {
        a.lerp(b, s / l1)
    } else if s <= l1 + l2 {
        b.lerp(c, (s - l1) / l2)
    } else {
        c + (c - b) / l2 * (s - l1 - l2)
    }
}

fn ts_between(a: f32, b: f32) -> Vec<f32> {
    let mut v = vec![a];
    let mut t = (a * 10.0).floor() / 10.0 + 0.1;
    while t < b - 0.03 {
        if t > a + 0.03 {
            v.push(t);
        }
        t += 0.1;
    }
    v.push(b);
    v
}

fn limb(a: Vec3, b: Vec3, c: Vec3, ts: &[f32], hint: Vec3, rad: &dyn Fn(f32) -> (f32, f32), col: &dyn Fn(f32) -> Vec3) -> Vec<R> {
    ts.iter()
        .map(|&t| {
            let p = pt3(a, b, c, t);
            let d = pt3(a, b, c, t + 0.04) - pt3(a, b, c, (t - 0.04).max(0.0));
            let (ru, rv) = rad(t);
            ring(p, d, hint, ru, rv, col(t))
        })
        .collect()
}

// ===========================================================================
// Body data
// ===========================================================================
// y, half width, half depth, forward offset
const TT_M: [[f32; 4]; 13] = [
    [0.72, 0.070, 0.060, 0.0],
    [0.80, 0.122, 0.086, 0.0],
    [0.88, 0.156, 0.100, 0.0],
    [0.96, 0.167, 0.106, 0.0],
    [1.04, 0.158, 0.100, 0.0],
    [1.12, 0.150, 0.094, 0.002],
    [1.20, 0.156, 0.098, 0.004],
    [1.28, 0.172, 0.108, 0.006],
    [1.36, 0.188, 0.114, 0.006],
    [1.43, 0.196, 0.108, 0.002],
    [1.47, 0.180, 0.095, -0.004],
    [1.495, 0.120, 0.080, -0.006],
    [1.512, 0.070, 0.065, -0.006],
];
const TT_F: [[f32; 4]; 13] = [
    [0.72, 0.065, 0.058, 0.0],
    [0.80, 0.118, 0.084, 0.0],
    [0.88, 0.160, 0.098, 0.0],
    [0.96, 0.172, 0.104, -0.002],
    [1.04, 0.146, 0.090, 0.0],
    [1.12, 0.130, 0.082, 0.002],
    [1.20, 0.140, 0.092, 0.008],
    [1.28, 0.156, 0.108, 0.014],
    [1.36, 0.164, 0.112, 0.016],
    [1.43, 0.172, 0.100, 0.004],
    [1.47, 0.160, 0.088, -0.004],
    [1.495, 0.112, 0.074, -0.006],
    [1.512, 0.064, 0.060, -0.006],
];
const TY: [f32; 16] = [0.80, 0.86, 0.92, 0.98, 1.04, 1.10, 1.16, 1.22, 1.28, 1.34, 1.40, 1.44, 1.465, 1.485, 1.50, 1.512];
// t, half width, half depth
const LEG_R: [[f32; 4]; 8] = [[0.0, 0.086, 0.090, 0.0], [0.15, 0.082, 0.086, 0.0], [0.35, 0.068, 0.070, 0.0], [0.5, 0.057, 0.058, 0.0], [0.62, 0.056, 0.061, 0.0], [0.8, 0.046, 0.047, 0.0], [0.95, 0.037, 0.040, 0.0], [1.0, 0.036, 0.039, 0.0]];
const ARM_R: [[f32; 4]; 7] = [[0.0, 0.058, 0.060, 0.0], [0.12, 0.054, 0.056, 0.0], [0.3, 0.046, 0.048, 0.0], [0.5, 0.037, 0.038, 0.0], [0.62, 0.038, 0.037, 0.0], [0.8, 0.031, 0.028, 0.0], [1.0, 0.026, 0.021, 0.0]];
// y, half width, half depth, forward offset (head local)
const HT: [[f32; 4]; 13] = [
    [-0.120, 0.018, 0.030, 0.058],
    [-0.108, 0.040, 0.052, 0.042],
    [-0.092, 0.056, 0.070, 0.026],
    [-0.070, 0.066, 0.084, 0.012],
    [-0.045, 0.072, 0.092, 0.004],
    [-0.015, 0.075, 0.096, 0.000],
    [0.015, 0.077, 0.098, -0.002],
    [0.045, 0.076, 0.098, -0.004],
    [0.070, 0.072, 0.094, -0.007],
    [0.092, 0.063, 0.083, -0.011],
    [0.108, 0.047, 0.064, -0.013],
    [0.119, 0.026, 0.038, -0.014],
    [0.124, 0.000, 0.000, -0.014],
];
const HY: [f32; 16] = [-0.12, -0.108, -0.092, -0.075, -0.058, -0.04, -0.02, 0.0, 0.02, 0.04, 0.06, 0.078, 0.094, 0.108, 0.118, 0.124];
const HAIRLINE: [[f32; 4]; 6] = [[0.0, 0.068, 0.0, 0.0], [0.6, 0.052, 0.0, 0.0], [1.1, -0.002, 0.0, 0.0], [1.6, -0.010, 0.0, 0.0], [2.3, -0.045, 0.0, 0.0], [3.15, -0.085, 0.0, 0.0]];
const BTOP: [[f32; 4]; 4] = [[0.0, -0.052, 0.0, 0.0], [0.5, -0.035, 0.0, 0.0], [1.0, -0.018, 0.0, 0.0], [1.57, 0.012, 0.0, 0.0]];
const SOLE_SEC: [[f32; 4]; 9] = [
    [-0.088, 0.040, 0.020, 0.0],
    [-0.075, 0.080, 0.032, 0.0],
    [-0.040, 0.112, 0.040, 0.0],
    [0.0, 0.112, 0.043, 0.0],
    [0.04, 0.092, 0.046, 0.0],
    [0.09, 0.066, 0.049, 0.0],
    [0.14, 0.050, 0.048, 0.0],
    [0.19, 0.040, 0.040, 0.0],
    [0.225, 0.024, 0.024, 0.0],
];

fn head_prof(fem: bool, y: f32) -> (f32, f32, f32) {
    let r = tbl(&HT, y);
    let (mut w, mut d, x0) = (r[1], r[2], r[3]);
    if fem {
        w *= 0.95 - 0.07 * smooth(-0.03, -0.11, y);
        d *= 0.97;
    }
    (w, d, x0)
}

struct Sk {
    fem: bool,
    old: bool,
    lk: Look,
    lean: f32,
    br: f32,
    skin: Vec3,
    hair: Vec3,
    eye: Vec3,
    tc: Vec3,
    sc: Vec3,
    bc: Vec3,
    feet: Vec3,
    yaw: f32,
    bob: f32,
    tw_p: f32,
    tw_c: f32,
    tt: &'static [[f32; 4]],
    fpel: Fr,
    fch: Fr,
    hip: [Vec3; 2],
    knee: [Vec3; 2],
    ank: [Vec3; 2],
    fd: [Vec3; 2],
    fu: [Vec3; 2],
    sh: [Vec3; 2],
    el: [Vec3; 2],
    wr: [Vec3; 2],
    hd: [Vec3; 2],
    hn: [Vec3; 2],
    curl: [f32; 2],
    hc: Vec3,
    fh: Vec3,
    uh: Vec3,
    rh: Vec3,
}

fn lx(s: &Sk, y: f32) -> f32 {
    s.lean * (y - 0.9).max(0.0)
}
fn t_ring(s: &Sk, y: f32, grow: f32, col: Vec3) -> R {
    let t = tbl(s.tt, y);
    let tw = mixf(s.tw_p, s.tw_c, smooth(0.95, 1.35, y));
    let fr = frame(s.feet + Vec3::Y * s.bob, s.yaw + tw);
    let g = grow + s.br * 0.003 * smooth(1.1, 1.35, y) * (1.0 - smooth(1.42, 1.5, y));
    R { c: fr.p(t[3] + lx(s, y), y, 0.0), u: fr.r, v: fr.f, ru: t[1] + g, rv: t[2] + g, col }
}
fn hp(s: &Sk, x: f32, y: f32, z: f32) -> Vec3 {
    s.hc + s.fh * x + s.uh * y + s.rh * z
}
fn face_x(s: &Sk, y: f32, z: f32) -> f32 {
    let (w, d, x0) = head_prof(s.fem, y);
    x0 + d * (1.0 - (z / w).powi(2)).max(0.0).sqrt()
}
fn head_ring(s: &Sk, y: f32, grow: f32, col: Vec3) -> R {
    let (w, d, x0) = head_prof(s.fem, y);
    R { c: hp(s, x0, y, 0.0), u: s.rh, v: s.fh, ru: w + grow, rv: d + grow, col }
}
/// point on the skull surface at height y / ring angle th, pushed out by `off`
fn skull_pt(s: &Sk, y: f32, th: f32, off: f32) -> Vec3 {
    let (w, d, x0) = head_prof(s.fem, y);
    hp(s, x0 + (d + off) * th.sin(), y, (w + off) * th.cos())
}
fn head_k(s: &Sk, y: f32, th: f32) -> f32 {
    let fr = angdiff(th, PI * 0.5);
    let brow = 0.045 * gs(y - 0.034, 0.012) * gs(fr, 0.65);
    let cheek = if s.old { -0.04 } else { 0.03 } * gs(y + 0.03, 0.025) * gs(fr - 0.9, 0.35);
    let chin = 0.04 * gs(y + 0.105, 0.015) * gs(fr, 0.5);
    1.0 + brow + cheek + chin
}

// ===========================================================================
// Parts
// ===========================================================================
fn strand(mb: &mut MB, pts: &[Vec3], rad: &[(f32, f32)], hint: &dyn Fn(Vec3) -> Vec3, col: Vec3, sides: usize, cap1: bool) {
    let n = pts.len();
    let rings: Vec<R> = (0..n)
        .map(|i| {
            let d = pts[(i + 1).min(n - 1)] - pts[i.saturating_sub(1)];
            ring(pts[i], d, hint(pts[i]), rad[i].0, rad[i].1, col)
        })
        .collect();
    loft(mb, &rings, &Lf::c(sides).caps(true, cap1), &id_s, &|i, th, c| c * (0.9 + 0.12 * (th * 3.0 + i as f32).sin()));
}

fn torso(mb: &mut MB, s: &Sk) {
    let rings: Vec<R> = TY.iter().map(|&y| t_ring(s, y, 0.0, s.skin)).collect();
    loft(mb, &rings, &Lf::c(16).caps(true, true), &id_s, &|i, th, c| {
        let y = TY[i];
        let c = tint(c, 0.4 * smooth(1.40, 1.46, y) * gs(angdiff(th, PI * 0.5), 1.2));
        c * (1.0 - 0.07 * smooth(1.48, 1.512, y)) * (0.985 + 0.015 * (th * 2.0 + i as f32 * 0.5).sin())
    });
}

fn shirt(mb: &mut MB, s: &Sk) {
    let style = s.lk.top_style;
    let fem = s.fem;
    let (hem, grow, flare) = match style {
        0 => (if fem { 0.90 } else { 0.85 }, 0.014, 0.03),
        1 => (if fem { 0.90 } else { 0.85 }, 0.012, 0.02),
        2 => (if fem { 1.00 } else { 0.93 }, 0.012, 0.0),
        _ => (if fem { 0.80 } else { 0.78 }, 0.024, 0.09),
    };
    let top_y = if style == 2 { 1.43 } else { 1.512 };
    let mut ys = vec![hem];
    ys.extend(TY.iter().filter(|&&y| y > hem + 0.03 && y < top_y - 0.001));
    ys.push(top_y);
    let n = ys.len();
    let body = s.tc;
    let acc = s.sc;
    let rings: Vec<R> = ys.iter().map(|&y| t_ring(s, y, grow, body)).collect();
    let sides = if style == 2 { 32 } else { 18 };
    let skin = s.skin;
    loft(
        mb,
        &rings,
        &Lf::c(sides).caps(false, style == 2 && false),
        &|i, th, p, c| {
            let mut p = p;
            if i == 0 && flare > 0.0 {
                p = c + (p - c) * (1.0 + flare);
                if style == 3 {
                    p.y -= 0.035 * hs(th * 5.0, 1.0);
                }
            }
            if style == 2 && i + 1 >= n {
                let sn = th.sin();
                p.y -= 0.045 * sn.max(0.0).powi(2) + 0.02 * (-sn).max(0.0).powi(2);
            }
            p
        },
        &|i, th, c| {
            let y = ys[i];
            let mut c = c;
            match style {
                1 => {
                    if y > 1.34 {
                        let thr = 1.0 - (y - 1.34) / 0.172 * 0.8;
                        let m = smooth(thr - 0.10, thr + 0.04, th.cos().abs());
                        c = c.lerp(acc, m);
                    }
                }
                2 => {
                    let jj = (th / TAU * 32.0).round() as i32;
                    c *= if jj % 2 == 0 { 0.93 } else { 1.05 };
                    c *= 0.9 + 0.1 * smooth(hem, hem + 0.06, y);
                }
                3 => {
                    c *= 0.97 + 0.04 * (th * 3.0 + i as f32).sin();
                    if y > 1.40 {
                        let hw = 0.55 * smooth(1.40, 1.512, y);
                        let m = (1.0 - smooth(hw - 0.12, hw + 0.02, angdiff(th, PI * 0.5))) * smooth(1.40, 1.44, y);
                        c = c.lerp(skin * 0.95, m);
                    }
                    if (0.95..1.07).contains(&y) && angdiff(th, 0.7) < 0.26 {
                        c = acc * 0.9;
                    }
                    c *= mixf(0.82, 1.0, smooth(hem, hem + 0.12, y));
                }
                _ => {
                    c *= 0.985 + 0.02 * (th * 2.0 + i as f32 * 0.7).sin();
                }
            }
            c * mixf(0.88, 1.0, smooth(hem, hem + 0.05, y))
        },
    );
    // collar
    let fr = frame(s.feet + Vec3::Y * s.bob, s.yaw + s.tw_c);
    let ccol = match style {
        0 => body * 0.85,
        1 => acc,
        _ => body * 0.92,
    };
    if style <= 1 || style == 3 {
        let (a, b, c) = if style == 3 { (0.084, 0.078, 0.07) } else { (0.083, 0.071, 0.066) };
        let col = ccol;
        let rs = vec![
            R { c: fr.p(-0.006 + lx(s, 1.5), 1.50, 0.0), u: fr.r, v: fr.f, ru: a, rv: a * 0.94, col },
            R { c: fr.p(-0.006 + lx(s, 1.5), 1.52, 0.0), u: fr.r, v: fr.f, ru: b, rv: b * 0.96, col },
            R { c: fr.p(-0.006 + lx(s, 1.5), if style == 3 { 1.54 } else { 1.545 }, 0.0), u: fr.r, v: fr.f, ru: c, rv: c * 0.96, col: col * 0.92 },
        ];
        loft(mb, &rs, &Lf::c(18), &id_s, &|i, th, c| {
            if style == 3 && angdiff(th, PI * 0.5) < 0.6 && i > 0 {
                skin * 0.9
            } else {
                c * (0.96 + 0.06 * ((th * 9.0).sin()))
            }
        });
    }
    if style == 3 {
        // cord lacing in the neck opening
        for q in 0..3 {
            let y = 1.44 + 0.022 * q as f32;
            let (a, b) = (fr.p(0.12 - q as f32 * 0.008, y, -0.018), fr.p(0.12 - q as f32 * 0.008, y + 0.012, 0.018));
            cyl(mb, a, b, 0.0025, 0.0025, acc * 0.8, 3);
        }
    }
    if style == 2 {
        // straps over the shoulders
        for e in [-1.0f32, 1.0] {
            let pts: Vec<Vec3> = [(0.100, 1.40, 0.072), (0.100, 1.45, 0.082), (0.078, 1.488, 0.088), (0.04, 1.502, 0.088), (-0.04, 1.502, 0.088), (-0.078, 1.488, 0.088), (-0.100, 1.45, 0.080), (-0.098, 1.41, 0.070)]
                .iter()
                .map(|&(x, y, z)| fr.p(lx(s, 1.45) + x * if fem { 0.92 } else { 1.0 }, y, z * e * if fem { 0.88 } else { 1.0 }))
                .collect();
            let rad = vec![(0.0045, 0.0125); 8];
            let c0 = body * 1.04;
            strand(mb, &pts, &rad, &|p| p - fr.p(0.0, 1.4, 0.0), c0, 6, true);
        }
    }
}

fn pants(mb: &mut MB, s: &Sk) {
    let st = s.lk.bottom_style;
    let top = if s.fem { 1.05 } else { 1.06 };
    let mut ys = vec![0.74, 0.80, 0.88, 0.96, 1.04];
    ys.push(top);
    let bc = s.bc;
    let rings: Vec<R> = ys.iter().map(|&y| t_ring(s, y, 0.006, bc)).collect();
    loft(mb, &rings, &Lf::c(16).caps(true, true), &id_s, &|i, th, c| c * (0.975 + 0.03 * (th + i as f32).sin()) * if i + 1 == ys.len() { 0.82 } else { 1.0 });
    // belt for cropped tops
    if s.lk.top_style == 2 {
        let rs: Vec<R> = [1.02f32, 1.052].iter().map(|&y| t_ring(s, y, 0.012, vec3(0.20, 0.13, 0.08))).collect();
        loft(mb, &rs, &Lf::c(16), &id_s, &id_c);
        let b = t_ring(s, 1.036, 0.014, Vec3::ZERO);
        blob(mb, b.c + s.fpel.f * (b.rv + 0.004), vec3(0.006, 0.014, 0.018), vec3(0.7, 0.62, 0.4), 0.0, 6, 4, s.yaw, shade);
    }
    let lw = if s.fem { 0.93 } else { 1.0 };
    for i in 0..2 {
        let (h, k, a) = (s.hip[i], s.knee[i], s.ank[i]);
        let sdv = sd(i);
        let rad = |t: f32| {
            let r = tbl(&LEG_R, t);
            (r[1] * lw, r[2] * lw)
        };
        // skin leg where it is visible
        let t0 = match st {
            2 => Some(0.3),
            3 => Some(0.55),
            _ => {
                if s.lk.shoe_style >= 1 {
                    Some(0.9)
                } else {
                    None
                }
            }
        };
        if let Some(t0) = t0 {
            let ts = ts_between(t0, 1.0);
            let rs = limb(h, k, a, &ts, s.fpel.r, &|t| rad(t), &|t| tint(s.skin * (0.99 + 0.015 * (t * 7.0 + sdv).sin()), 0.8 * gs(t - 0.5, 0.06) + 0.3 * gs(t - 0.9, 0.05)));
            loft(mb, &rs, &Lf::c(12), &id_s, &id_c);
        }
        let (tend, g, fl) = match st {
            0 => (1.0, 0.012, 0.020),
            1 => (1.0, 0.005, 0.012),
            2 => (0.40, 0.016, 0.0),
            _ => (0.74, 0.012, 0.0),
        };
        let mut ts = ts_between(0.0, tend);
        if tend >= 1.0 {
            ts.push(1.06);
        }
        let mut rs = limb(h, k, a, &ts, s.fpel.r, &|t| {
            let (u, v) = rad(t.min(1.0));
            let e = g + fl * smooth(0.85, 1.05, t);
            (u + e, v + e)
        }, &|t| {
            let mut c = bc * (0.985 + 0.02 * (t * 9.0 + sdv).sin());
            if st == 1 {
                c *= 1.0 + 0.10 * gs(t - 0.28, 0.1) + 0.06 * gs(t - 0.52, 0.05);
            }
            c *= 1.0 - 0.07 * gs(t - 0.5, 0.05);
            c.lerp(vec3(0.30, 0.24, 0.17), 0.30 * smooth(0.85, 1.06, t))
        });
        if tend < 0.95 {
            // turned-up cuff
            if let Some(last) = rs.last().copied() {
                let mut c2 = last;
                c2.ru += 0.012;
                c2.rv += 0.012;
                c2.col = last.col * 0.85;
                let mut c1 = c2;
                c1.c += (last.c - rs[rs.len() - 2].c).normalize_or_zero() * 0.035;
                rs.push(c2);
                rs.push(c1);
            }
        }
        loft(mb, &rs, &Lf::c(12).caps(false, tend < 0.95), &id_s, &id_c);
        {
            let r0_ = rad(0.0);
            blob(mb, h, Vec3::splat(r0_.0 + 0.012), bc * 0.98, 0.0, 8, 5, s.yaw, shade);
            let rk = rad(0.5);
            let (kc, kr) = if tend > 0.55 { (bc, rk.0 + g + 0.002) } else { (s.skin, rk.0 + 0.002) };
            blob(mb, k, Vec3::splat(kr), kc, 0.0, 8, 5, s.yaw, shade);
        }
        if st == 0 {
            // cargo pocket on the outer thigh
            let c = pt3(h, k, a, 0.24) + s.fpel.r * sdv * (0.086 * lw + 0.022);
            blob(mb, c, vec3(0.052, 0.072, 0.016), bc * 0.92, 0.0, 8, 5, s.yaw, shade);
            blob(mb, c + Vec3::Y * 0.04 + s.fpel.r * sdv * 0.003, vec3(0.053, 0.022, 0.017), bc * 0.80, 0.0, 8, 4, s.yaw, shade);
            blob(mb, c + Vec3::Y * 0.035 + s.fpel.r * sdv * 0.014 + s.fpel.f * 0.0, Vec3::splat(0.006), vec3(0.45, 0.4, 0.3), 0.0, 5, 3, 0.0, shade);
        }
    }
}

fn shoe(mb: &mut MB, s: &Sk, i: usize) {
    let kind = s.lk.shoe_style;
    let sdv = sd(i);
    let (fd, fu) = (s.fd[i], s.fu[i]);
    let wa = fd.cross(fu);
    let sole = s.ank[i] - fu * 0.085;
    let p = |x: f32, y: f32, z: f32| sole + fd * x + fu * y + wa * z;
    let (hk, wk) = match kind {
        0 => (1.0, 1.0),
        1 => (0.80, 1.0),
        _ => (0.74, 0.92),
    };
    let col = match kind {
        0 => vec3(0.36, 0.21, 0.11),
        1 => vec3(0.80, 0.80, 0.78),
        _ => s.skin * vec3(0.98, 0.95, 0.93),
    };
    let xs: Vec<f32> = SOLE_SEC.iter().map(|r| r[0]).collect();
    let ymin = 0.014f32;
    let sec = |x: f32| {
        let r = tbl(&SOLE_SEC, x);
        ((r[1] * hk).max(0.03), r[2] * wk)
    };
    let rings: Vec<R> = xs
        .iter()
        .map(|&x| {
            let (yt, hw) = sec(x);
            ring(p(x, (ymin + yt) * 0.5, 0.0), fd, wa, hw, (yt - ymin) * 0.5, col)
        })
        .collect();
    loft(mb, &rings, &Lf::c(14).caps(true, true), &id_s, &|_, th, c| if kind == 0 { c * (0.94 + 0.1 * (th * 6.0).sin().abs()) } else { c });
    if kind <= 2 {
        let (scol, th) = match kind {
            0 => (vec3(0.80, 0.70, 0.52), 0.011),
            1 => (vec3(0.93, 0.93, 0.9), 0.012),
            _ => (vec3(0.40, 0.27, 0.16), 0.005),
        };
        let heel = if kind == 0 { 0.006 } else { 0.0 };
        let rs: Vec<R> = xs
            .iter()
            .map(|&x| {
                let (_, hw) = sec(x);
                ring(p(x, th, 0.0), fd, wa, hw + 0.006, th + if x < -0.03 { heel } else { 0.0 }, scol)
            })
            .collect();
        loft(mb, &rs, &Lf::c(12).caps(true, true), &id_s, &|i, _, c| c * if i % 2 == 0 { 1.0 } else { 0.9 });
    }
    if kind == 0 {
        let up = (s.knee[i] - s.ank[i]).normalize_or_zero();
        let rs: Vec<R> = [(-0.03f32, 0.044f32, 0.047f32), (0.0, 0.044, 0.047), (0.06, 0.045, 0.046), (0.12, 0.046, 0.046), (0.135, 0.049, 0.049)]
            .iter()
            .enumerate()
            .map(|(q, &(h, a, b))| ring(s.ank[i] + up * h - fd * 0.005, up, wa, a, b, if q == 4 { col * 0.7 } else { col }))
            .collect();
        loft(mb, &rs, &Lf::c(12), &id_s, &id_c);
    }
    if kind <= 1 {
        for q in 0..4 {
            let x = 0.02 + 0.03 * q as f32;
            let yt = sec(x).0 + 0.003;
            cyl(mb, p(x, yt, -0.022), p(x, yt, 0.022), 0.0026, 0.0026, vec3(0.88, 0.85, 0.72), 3);
        }
    }
    if kind == 2 {
        for x in [0.03f32, 0.11] {
            let (yt, hw) = sec(x);
            let cy = (ymin + yt) * 0.5;
            let ry = (yt - ymin) * 0.5;
            let pts: Vec<Vec3> = (0..=6)
                .map(|q| {
                    let phi = PI - PI * q as f32 / 6.0;
                    p(x, cy + (ry + 0.003) * phi.sin(), (hw + 0.003) * phi.cos())
                })
                .collect();
            let rs: Vec<R> = (0..pts.len())
                .map(|q| ring(pts[q], pts[(q + 1).min(6)] - pts[q.saturating_sub(1)], fd, 0.0075, 0.002, vec3(0.40, 0.27, 0.16)))
                .collect();
            loft(mb, &rs, &Lf::c(6).caps(true, true), &id_s, &id_c);
        }
    }
    if kind >= 2 {
        let yaw = fd.z.atan2(fd.x);
        for q in 0..5 {
            let r = if q == 0 { 0.0115 } else { 0.0085 - 0.0007 * q as f32 };
            blob(mb, p(0.208 - 0.005 * q as f32, 0.016 + 0.0 * hk, sdv * (-0.022 + 0.0138 * q as f32)), vec3(r * 1.3, r * 0.9, r), s.skin * 1.02, 0.0, 6, 4, yaw, shade);
        }
    }
}

fn hand(mb: &mut MB, w: Vec3, d: Vec3, nh: Vec3, side: f32, curl: f32, skin: Vec3) {
    let d = d.normalize_or_zero();
    if d == Vec3::ZERO {
        return;
    }
    let mut n = (nh - d * nh.dot(d)).normalize_or_zero();
    if n == Vec3::ZERO {
        n = d.any_orthonormal_vector();
    }
    let t = d.cross(n) * side;
    let knuckle = skin * vec3(1.04, 0.95, 0.92);
    let rs: Vec<R> = [(0.0f32, 0.029f32, 0.020f32), (0.35, 0.039, 0.019), (0.7, 0.042, 0.016), (1.0, 0.040, 0.013)]
        .iter()
        .map(|&(q, ru, rv)| ring(w + d * (0.088 * q), d, t, ru, rv, if q > 0.9 { knuckle } else { skin }))
        .collect();
    loft(mb, &rs, &Lf::c(8).caps(false, true), &id_s, &id_c);
    let lens = [0.072f32, 0.080, 0.074, 0.060];
    let offs = [0.031f32, 0.0105, -0.0105, -0.030];
    for f in 0..4 {
        let c = curl * (1.0 + 0.08 * f as f32);
        let angs = [c * 1.1, c * 2.1, c * 2.7];
        let mut p = w + d * 0.088 + t * offs[f];
        let mut rr = vec![ring(p, d * angs[0].cos() + n * angs[0].sin(), t, 0.0082, 0.0080, knuckle)];
        for k in 0..3 {
            let dir = d * angs[k].cos() + n * angs[k].sin();
            p += dir * lens[f] / 3.0 * if k == 0 { 1.15 } else { 0.95 };
            let r = 0.0082 * (1.0 - 0.14 * (k + 1) as f32);
            rr.push(ring(p, dir, t, r, r * 0.95, skin));
        }
        loft(mb, &rr, &Lf::c(6).caps(false, true), &id_s, &id_c);
    }
    // thumb
    let dir0 = (d * 0.55 + t * 0.8).normalize();
    let mut p = w + d * 0.03 + t * 0.034;
    let mut rr = vec![ring(p, dir0, n, 0.0115, 0.011, skin)];
    for (k, l) in [0.042f32, 0.036].iter().enumerate() {
        let a = curl * 0.8 * (k + 1) as f32;
        let dir = dir0 * a.cos() + n * a.sin();
        p += dir * *l;
        rr.push(ring(p, dir, n, 0.0105 - 0.0018 * k as f32, 0.0098 - 0.0016 * k as f32, skin));
    }
    loft(mb, &rr, &Lf::c(6).caps(false, true), &id_s, &id_c);
}

fn arm(mb: &mut MB, s: &Sk, i: usize) {
    let (sh0, el, wr) = (s.sh[i], s.el[i], s.wr[i]);
    let sh = sh0 - s.fch.r * sd(i) * 0.035 + Vec3::Y * 0.006;
    let lw = if s.fem { 0.88 } else { 1.0 };
    let style = s.lk.top_style;
    let (send, grow) = match style {
        0 => (0.36f32, 0.012f32),
        1 => (0.985, 0.008),
        2 => (0.0, 0.0),
        _ => (0.52, 0.02),
    };
    let rad = |t: f32| {
        let r = tbl(&ARM_R, t);
        (r[1] * lw, r[2] * lw)
    };
    let hint = s.fch.r;
    if send < 0.97 {
        let ts = ts_between(if send > 0.0 { send - 0.04 } else { 0.0 }, 1.0);
        let rs = limb(sh, el, wr, &ts, hint, &rad, &|t| tint(s.skin * (0.99 + 0.015 * (t * 6.0).sin()), 0.5 * gs(t - 0.5, 0.06) + 0.4 * gs(t - 1.0, 0.1)));
        loft(mb, &rs, &Lf::c(10).caps(send <= 0.0, false), &id_s, &id_c);
    }
    let sleeve_col = match style {
        0 | 1 => s.sc,
        _ => s.tc,
    };
    if send > 0.0 {
        let ts = ts_between(0.0, send);
        let mut rs = limb(sh, el, wr, &ts, hint, &|t| {
            let (u, v) = rad(t);
            (u + grow, v + grow)
        }, &|t| sleeve_col * (0.985 + 0.02 * (t * 8.0).sin()));
        let last = *rs.last().unwrap();
        let mut cuff = last;
        cuff.ru += 0.005;
        cuff.rv += 0.005;
        cuff.col = last.col * if style == 3 { 1.05 } else { 0.85 };
        let mut c2 = cuff;
        c2.c += (last.c - rs[rs.len() - 2].c).normalize_or_zero() * if style == 3 { 0.03 } else { 0.012 };
        rs.push(cuff);
        rs.push(c2);
        loft(mb, &rs, &Lf::c(10).caps(true, false), &id_s, &|i, th, c| if style == 3 { c * (0.92 + 0.08 * ((th * 5.0 + i as f32).sin())) } else { c });
    }
    let bc = if send > 0.0 { sleeve_col } else { s.skin };
    let sr = 0.066 * lw.max(0.92) + if send > 0.0 { grow } else { 0.0 };
    blob(mb, sh0 + Vec3::Y * 0.004, vec3(sr * 1.05, sr, sr), bc, 0.0, 10, 6, s.yaw, shade);
    blob(mb, sh0 - s.fch.r * sd(i) * 0.03 + Vec3::Y * 0.012, vec3(sr * 0.95, sr * 0.9, sr * 0.95), bc, 0.0, 8, 5, s.yaw, shade);
    {
        let er = rad(0.5).0 + 0.002 + if send > 0.55 { grow } else { 0.0 };
        blob(mb, el, Vec3::splat(er), if send > 0.55 { sleeve_col } else { s.skin }, 0.0, 8, 5, s.yaw, shade);
    }
    hand(mb, wr, s.hd[i], s.hn[i], sd(i), s.curl[i], s.skin);
}

fn face(mb: &mut MB, s: &Sk) {
    let fem = s.fem;
    let skin = s.skin;
    let hairc = s.hair;
    let nk = s.fh * 0.0;
    let _ = nk;
    // neck
    let n0 = s.fch.p(lx(s, 1.455), 1.455, 0.0);
    let n1 = hp(s, -0.012, -0.09, 0.0);
    let nr = if fem { 0.043 } else { 0.053 };
    let pts = [n0, n0.lerp(n1, 0.33), n0.lerp(n1, 0.66), n1];
    let rs: Vec<R> = pts
        .iter()
        .enumerate()
        .map(|(q, &p)| ring(p, n1 - n0, s.fch.r, nr * (1.0 - 0.07 * q as f32 / 3.0) * 1.0, nr * 1.04 * (1.0 - 0.07 * q as f32 / 3.0), skin * (1.0 - 0.1 * (q as f32 / 3.0).powi(2))))
        .collect();
    loft(mb, &rs, &Lf::c(12), &id_s, &id_c);
    // head
    let hrings: Vec<R> = HY.iter().map(|&y| head_ring(s, y, 0.0, skin)).collect();
    let blush = if fem { 0.7 } else { 0.5 };
    loft(
        mb,
        &hrings,
        &Lf::c(22),
        &|i, th, p, c| c + (p - c) * head_k(s, HY[i], th),
        &|i, th, c| {
            let y = HY[i];
            let fr = angdiff(th, PI * 0.5);
            let mut c = c * (0.99 + 0.02 * (th * 3.0 + i as f32).sin());
            c = tint(c, blush * gs(y + 0.03, 0.03) * gs(fr - 0.85, 0.4) + 0.3 * gs(y + 0.045, 0.012) * gs(fr, 0.15));
            c *= 1.0 - 0.07 * gs(y - 0.0, 0.012) * gs(fr, 0.7) - 0.06 * smooth(-0.1, -0.12, y);
            if s.old {
                c *= 1.0 - 0.05 * gs(y + 0.02, 0.02) * gs(fr - 1.0, 0.3);
            }
            c
        },
    );
    // ears
    for e in [-1.0f32, 1.0] {
        blob(mb, hp(s, -0.005, -0.004, e * 0.0775), vec3(0.015, 0.027, 0.007), skin * 0.97, 0.0, 7, 5, s.yaw, shade);
        blob(mb, hp(s, -0.003, -0.004, e * 0.081), vec3(0.008, 0.016, 0.003), skin * 0.78, 0.0, 6, 4, s.yaw, shade);
    }
    // nose
    let nose: [(f32, f32, f32, f32); 5] = [(0.090, 0.022, 0.004, 0.004), (0.099, 0.004, 0.0075, 0.0065), (0.109, -0.012, 0.0095, 0.008), (0.116, -0.026, 0.0125, 0.0095), (0.113, -0.035, 0.0145, 0.008)];
    let pts: Vec<Vec3> = nose.iter().map(|n| hp(s, n.0, n.1, 0.0)).collect();
    let rs: Vec<R> = (0..5).map(|q| ring(pts[q], pts[(q + 1).min(4)] - pts[q.saturating_sub(1)], s.rh, nose[q].2, nose[q].3, tint(skin, if q >= 3 { 0.7 } else { 0.1 }))).collect();
    loft(mb, &rs, &Lf::c(8).caps(true, true), &id_s, &id_c);
    for e in [-1.0f32, 1.0] {
        blob(mb, hp(s, 0.104, -0.032, e * 0.0115), vec3(0.008, 0.006, 0.0065), tint(skin, 0.8), 0.0, 6, 4, s.yaw, shade);
        blob(mb, hp(s, 0.1135, -0.0385, e * 0.0058), vec3(0.004, 0.0016, 0.0028), vec3(0.18, 0.08, 0.07), 0.0, 5, 3, s.yaw, shade);
    }
    // eyes
    let es = if fem { 1.1 } else { 1.0 };
    for e in [-1.0f32, 1.0] {
        let z = e * 0.034;
        let xs = face_x(s, 0.012, 0.034) * 1.0 - 0.006;
        blob(mb, hp(s, xs, 0.012, z), vec3(0.0095, 0.0105 * es, 0.0155 * es), vec3(0.93, 0.92, 0.9), 0.0, 8, 6, s.yaw, shade);
        blob(mb, hp(s, xs + 0.0092, 0.012, z), vec3(0.0026, 0.0072 * es, 0.0072 * es), s.eye, 0.0, 8, 5, s.yaw, shade);
        blob(mb, hp(s, xs + 0.0112, 0.012, z), vec3(0.0012, 0.0034 * es, 0.0034 * es), vec3(0.02, 0.02, 0.025), 0.0, 6, 4, s.yaw, shade);
        let lid = skin * 0.97;
        blob(mb, hp(s, xs + 0.003, 0.012 + 0.0085 * es, z), vec3(0.0085, 0.0045, 0.0168 * es), lid, 0.0, 8, 4, s.yaw, shade);
        blob(mb, hp(s, xs + 0.003, 0.012 - 0.0095 * es, z), vec3(0.0075, 0.0034, 0.0158 * es), lid * 0.96, 0.0, 8, 4, s.yaw, shade);
        blob(mb, hp(s, xs + 0.0085, 0.012 + 0.0062 * es, z), vec3(0.0076, 0.0013 * if fem { 1.8 } else { 1.0 }, 0.0162 * es), hairc * 0.35, 0.0, 8, 3, s.yaw, shade);
        // eyebrow
        let (by, thick) = if fem { (0.043, 0.0038) } else { (0.040, 0.0066) };
        let bp: Vec<Vec3> = (0..5)
            .map(|q| {
                let u = q as f32 / 4.0;
                let zz = 0.012 + 0.040 * u;
                let y = by + 0.008 * (PI * u * 0.85 + 0.2).sin() - 0.004 * u * u;
                hp(s, face_x(s, y, zz) + 0.003, y, e * zz)
            })
            .collect();
        let brow_c = if s.old { hairc * 0.9 } else { hairc * 0.7 };
        let rs: Vec<R> = (0..5)
            .map(|q| {
                let tp = 1.0 - 0.45 * (q as f32 / 4.0).powi(2);
                ring(bp[q], bp[(q + 1).min(4)] - bp[q.saturating_sub(1)], s.uh, thick * tp, 0.0045 * tp, brow_c)
            })
            .collect();
        loft(mb, &rs, &Lf::c(5).caps(true, true), &id_s, &id_c);
    }
    // lips
    let lipc = tint(skin, 1.0).lerp(vec3(0.62, 0.22, 0.24), if fem { 0.42 } else { 0.22 });
    for (yc, th, bulge, col) in [(-0.0615f32, 0.0045f32, -0.0012f32, lipc * 0.97), (-0.0715, 0.0058, 0.0016, lipc)] {
        let pts: Vec<Vec3> = (0..7)
            .map(|q| {
                let u = q as f32 / 6.0 * 2.0 - 1.0;
                let z = u * 0.026;
                let y = yc + bulge * u.abs() * -1.0 + bulge * (1.0 - u * u) * 0.0 - 0.0016 * u * u * if yc > -0.065 { -1.0 } else { 1.0 };
                hp(s, face_x(s, y, z) + 0.0012, y, z)
            })
            .collect();
        let rs: Vec<R> = (0..7)
            .map(|q| {
                let u = q as f32 / 6.0 * 2.0 - 1.0;
                let tp = (1.0 - u * u).max(0.05).sqrt();
                ring(pts[q], pts[(q + 1).min(6)] - pts[q.saturating_sub(1)], s.uh, th * tp, 0.0058 * tp, col)
            })
            .collect();
        loft(mb, &rs, &Lf::c(6).caps(true, true), &id_s, &id_c);
    }
    let mp: Vec<Vec3> = (0..5)
        .map(|q| {
            let z = (q as f32 / 4.0 * 2.0 - 1.0) * 0.027;
            hp(s, face_x(s, -0.0665, z) + 0.0005, -0.0665, z)
        })
        .collect();
    for q in 0..4 {
        cyl(mb, mp[q], mp[q + 1], 0.0012, 0.0012, vec3(0.28, 0.12, 0.12), 3);
    }
}

fn hair_and_beard(mb: &mut MB, s: &Sk) {
    let lk = &s.lk;
    let hairc = s.hair;
    let style = lk.hair_style;
    let hat = lk.hat > 0;
    let skin = s.skin;
    // beard first (so the moustache etc. are consistent)
    if !s.fem && lk.beard > 0 {
        let (thick, len) = match lk.beard {
            1 => (0.012f32, 0.0f32),
            2 => (0.050, 0.02),
            _ => (0.085, 0.10),
        };
        let bcol = if lk.beard == 1 { hairc.lerp(skin, 0.45) * 0.85 } else { hairc };
        let mut rs: Vec<R> = HY.iter().take(8).map(|&y| head_ring(s, y, 0.0, bcol)).collect();
        let n0 = rs.len();
        if len > 0.0 {
            for q in 1..=3 {
                let t = q as f32 / 3.0;
                rs.push(R { c: hp(s, 0.050 - 0.015 * t, -0.12 - len * t, 0.0), u: s.rh, v: s.fh, ru: 0.030 - 0.017 * t, rv: 0.042 - 0.02 * t, col: bcol });
            }
        }
        let ncount = rs.len();
        loft(
            mb,
            &rs,
            &Lf::c(22).caps(false, len > 0.0),
            &|i, th, p, c| {
                if i >= n0 {
                    let fr = angdiff(th, PI * 0.5);
                    let k = 1.0 + 0.5 * (1.0 - smooth(0.7, 1.3, fr)) * 0.0;
                    return c + (p - c) * k;
                }
                let y = HY[i];
                let fr = angdiff(th, PI * 0.5);
                let top = tbl(&BTOP, fr)[1] * (1.0) + if fr > 1.57 { 0.0 } else { 0.0 };
                let mut cov = 1.0 - smooth(top - 0.008, top + 0.008, y);
                cov *= 1.0 - smooth(1.8, 2.3, fr);
                let (w, _, _) = head_prof(s.fem, y);
                let z = w * th.cos();
                let hole = (1.0 - smooth(0.022, 0.034, z.abs())) * (1.0 - smooth(0.013, 0.022, (y + 0.064).abs())) * (1.0 - smooth(0.7, 1.0, fr));
                cov *= 1.0 - hole;
                if len > 0.0 && i == 0 {
                    cov = cov.max(0.7);
                }
                let k = mixf(0.90, 1.0 + thick, cov) * head_k(s, y, th) * if i == 0 { 1.0 } else { 1.0 };
                c + (p - c) * k
            },
            &|i, th, c| {
                let j = hs(i as f32 * 1.7, (th * 9.0).floor());
                c * (0.78 + 0.4 * j) * if i < ncount && lk.beard == 1 { 1.0 } else { 1.0 }
            },
        );
        if lk.beard >= 2 {
            let pts: Vec<Vec3> = (0..7)
                .map(|q| {
                    let u = q as f32 / 6.0 * 2.0 - 1.0;
                    let z = u * 0.034;
                    let y = -0.050 - 0.012 * u * u;
                    hp(s, face_x(s, y, z) + 0.004, y, z)
                })
                .collect();
            let rs: Vec<R> = (0..7)
                .map(|q| {
                    let u = q as f32 / 6.0 * 2.0 - 1.0;
                    ring(pts[q], pts[(q + 1).min(6)] - pts[q.saturating_sub(1)], s.uh, 0.0085 * (1.0 - 0.3 * u * u), 0.008, hairc * 0.95)
                })
                .collect();
            loft(mb, &rs, &Lf::c(6).caps(true, true), &id_s, &|i, th, c| c * (0.85 + 0.25 * hs(i as f32, th * 4.0)));
        }
    }
    if style >= 9 {
        return;
    }
    let base_t = [0.07f32, 0.10, 0.09, 0.03, 0.08, 0.08, 0.07, 0.07, 0.13][style as usize];
    let base_t = if hat && (style == 1 || style == 8) { 0.06 } else { base_t };
    let rough = if style == 1 || style == 8 || (style == 4 && s.old) { 1.0 } else { 0.25 };
    let first = 2usize;
    let rs: Vec<R> = HY[first..].iter().map(|&y| head_ring(s, y, 0.0, hairc)).collect();
    let buzz = style == 3;
    loft(
        mb,
        &rs,
        &Lf::c(22),
        &|i, th, p, c| {
            let y = HY[i + first];
            let fr = angdiff(th, PI * 0.5);
            let yh = tbl(&HAIRLINE, fr)[1];
            let cov = smooth(yh - 0.010, yh + 0.014, y);
            let t = base_t * (0.6 + 0.7 * smooth(0.0, 0.1, y)) * (1.0 + rough * 0.7 * (hs(th * 3.0, y * 40.0) - 0.5));
            c + (p - c) * head_k(s, y, th) * mixf(0.93, 1.0 + t, cov)
        },
        &|i, th, c| {
            let y = HY[i + first];
            let st = 0.86 + 0.16 * (th * 13.0 + i as f32 * 0.7).sin() + 0.08 * (th * 31.0 + i as f32 * 1.9).sin();
            let c = c * st * (1.0 + 0.08 * smooth(0.05, 0.11, y));
            if buzz {
                c.lerp(skin, 0.38)
            } else {
                c
            }
        },
    );
    let sweep_hint = |p: Vec3| p - s.hc;
    match style {
        1 => {
            for q in 0..9 {
                let th = 0.4 + q as f32 * 0.72;
                let y = 0.10 - 0.012 * (q % 3) as f32 + 0.0;
                if hat && q > 2 {
                    break;
                }
                let p0 = skull_pt(s, y, th, 0.012);
                let out = (p0 - hp(s, -0.005, y * 0.5, 0.0)).normalize_or_zero();
                let swirl = s.rh * (hs(q as f32, 1.0) - 0.5) * 1.2 + s.fh * (hs(q as f32, 2.0) - 0.5);
                let d1 = (out * 0.6 + s.uh * 0.7 + swirl).normalize();
                let d2 = (out * 0.8 + s.uh * 0.2 + swirl * 1.5).normalize();
                let pts = [p0, p0 + d1 * 0.03, p0 + d1 * 0.03 + d2 * 0.028];
                strand(mb, &pts, &[(0.009, 0.020), (0.008, 0.016), (0.004, 0.006)], &sweep_hint, hairc * (0.9 + 0.2 * hs(q as f32, 3.0)), 6, true);
            }
        }
        2 | 5 => {
            let pts: Vec<Vec3> = [(0.112f32, 1.75f32), (0.100, 1.50), (0.078, 1.15), (0.050, 0.85), (0.025, 0.65)].iter().map(|&(y, th)| skull_pt(s, y, th, 0.011)).collect();
            strand(mb, &pts, &[(0.010, 0.020), (0.011, 0.026), (0.011, 0.026), (0.010, 0.020), (0.006, 0.010)], &sweep_hint, hairc * 1.02, 8, true);
        }
        6 => {
            let pts = [hp(s, -0.10, 0.045, 0.0), hp(s, -0.14, 0.0, 0.0), hp(s, -0.165, -0.07, 0.0), hp(s, -0.165, -0.15, 0.0), hp(s, -0.15, -0.24, 0.01)];
            strand(mb, &pts, &[(0.03, 0.028), (0.036, 0.034), (0.032, 0.03), (0.024, 0.022), (0.006, 0.006)], &|_| s.rh, hairc, 10, true);
            blob(mb, pts[0] - s.fh * 0.0, vec3(0.032, 0.03, 0.03), vec3(0.15, 0.1, 0.1), 0.0, 8, 4, s.yaw, shade);
        }
        7 => {
            blob(mb, hp(s, -0.05, 0.125, 0.0), vec3(0.052, 0.045, 0.052), hairc, 0.0, 10, 6, s.yaw, shade);
            blob(mb, hp(s, -0.05, 0.095, 0.0), vec3(0.036, 0.012, 0.036), vec3(0.2, 0.12, 0.1), 0.0, 8, 3, s.yaw, shade);
        }
        8 => {
            for q in 0..14 {
                let th = q as f32 * 2.4;
                let y = 0.115 - (q as f32 / 14.0) * 0.17;
                if angdiff(th, PI * 0.5) < 0.9 && y < 0.075 {
                    continue;
                }
                let r = 0.034 + 0.008 * hs(q as f32, 7.0);
                blob(mb, skull_pt(s, y, th, 0.022), Vec3::splat(r), hairc * (0.85 + 0.25 * hs(q as f32, 9.0)), 0.0, 7, 5, s.yaw, shade);
            }
        }
        _ => {}
    }
    if style == 4 || style == 5 {
        let long = style == 4;
        let n = 9usize;
        let y_end = if long { -0.37 } else { -0.105 };
        let rs: Vec<R> = (0..n)
            .map(|q| {
                let t = q as f32 / (n - 1) as f32;
                let y = mixf(-0.01, y_end, t);
                let (cx, ru, rv) = if long { (-0.012 - 0.063 * smooth(0.0, 1.0, t), 0.089 + 0.014 * t, 0.099 - 0.037 * t) } else { (-0.012 - 0.012 * t, 0.090 - 0.002 * t, 0.099 - 0.01 * t) };
                let (ru, rv) = if !long && t > 0.85 { (ru * 0.95, rv * 0.95) } else { (ru, rv) };
                R { c: hp(s, cx, y, 0.0), u: s.rh, v: s.fh, ru, rv, col: hairc }
            })
            .collect();
        let (start, sweep) = if long { (2.55, 4.0) } else { (2.09, TAU - 2.09) };
        let rough = s.old && long;
        loft(
            mb,
            &rs,
            &Lf::c(18).arc(start, sweep).caps(false, false),
            &|i, th, p, c| {
                if rough && i + 2 >= n {
                    let j = hs(th * 5.0, i as f32);
                    return p + (p - c) * 0.10 * (j - 0.3) - s.uh * 0.05 * j * (i + 2 - n + 1) as f32;
                }
                p
            },
            &|i, th, c| {
                let st = 0.82 + 0.2 * (th * 17.0 + i as f32 * 0.4).sin() + 0.1 * (th * 41.0).sin();
                let m = if rough { 0.9 + 0.2 * hs(i as f32, (th * 6.0).floor()) } else { 1.0 };
                c * st * m * (1.0 - 0.15 * (i as f32 / n as f32))
            },
        );
    }
}

fn hat_loft(mb: &mut MB, s: &Sk, prof: &[(f32, f32)], sides: usize, shape: &dyn Fn(usize, f32, Vec3, Vec3) -> Vec3, colf: &dyn Fn(usize, f32, Vec3) -> Vec3, col: Vec3) {
    let o = hp(s, -0.008, 0.032, 0.0);
    let rs: Vec<R> = prof.iter().map(|&(r, y)| R { c: o + s.uh * y, u: s.rh, v: s.fh, ru: r, rv: r * 1.1, col }).collect();
    loft(mb, &rs, &Lf::c(sides).caps(false, true), shape, colf);
}

fn hat(mb: &mut MB, s: &Sk) {
    match s.lk.hat {
        1 => {
            let prof = [(0.100, 0.0), (0.17, -0.004), (0.25, -0.012), (0.30, -0.02), (0.305, -0.014), (0.26, -0.002), (0.19, 0.008), (0.125, 0.018), (0.112, 0.04), (0.110, 0.075), (0.100, 0.10), (0.075, 0.112), (0.04, 0.116), (0.0, 0.117)];
            let old = s.old;
            let up = s.uh;
            hat_loft(
                mb,
                s,
                &prof,
                28,
                &|i, th, p, c| {
                    if (3..=5).contains(&i) {
                        let j = hs(th * 4.0, i as f32);
                        let droop = if old { 0.025 * (th * 1.0 + 1.0).sin().max(0.0) } else { 0.01 * (th + 2.0).sin().max(0.0) };
                        return c + (p - c) * (1.0 + if old { 0.05 } else { 0.015 } * (j - 0.5)) - up * droop * (i as f32 - 2.0) * 0.5;
                    }
                    p
                },
                &|i, th, c| {
                    let w = 0.88 + 0.16 * ((th * 36.0).sin() * (i as f32 * 2.3).cos()).abs() + 0.06 * hs(i as f32, (th * 10.0).floor());
                    c * w * if old { 0.88 } else { 1.0 }
                },
                vec3(0.80, 0.68, 0.40),
            );
            hat_loft(mb, s, &[(0.1138, 0.022), (0.1146, 0.034), (0.1138, 0.046)], 28, &id_s, &id_c, vec3(0.40, 0.14, 0.10));
        }
        2 => {
            let c = s.tc * 0.9;
            hat_loft(mb, s, &[(0.098, 0.0), (0.105, 0.02), (0.103, 0.05), (0.093, 0.08), (0.072, 0.10), (0.04, 0.11), (0.0, 0.113)], 22, &id_s, &|i, th, c| c * (0.95 + 0.07 * ((th * 6.0).sin().abs() * (i as f32 > 3.0) as i32 as f32)), c);
            let o = hp(s, -0.008, 0.032, 0.0);
            let pts = [o + s.fh * 0.085, o + s.fh * 0.12 - s.uh * 0.005, o + s.fh * 0.16 - s.uh * 0.015, o + s.fh * 0.19 - s.uh * 0.03];
            strand(mb, &pts, &[(0.004, 0.05), (0.004, 0.056), (0.004, 0.05), (0.004, 0.034)], &|_| s.uh, c * 0.9, 8, true);
            blob(mb, o + s.uh * 0.115, Vec3::splat(0.008), c, 0.0, 5, 3, 0.0, shade);
        }
        3 => {
            let w = vec3(0.88, 0.86, 0.80);
            hat_loft(
                mb,
                s,
                &[(0.118, 0.0), (0.121, 0.02), (0.119, 0.045), (0.108, 0.052), (0.104, 0.056), (0.100, 0.075), (0.095, 0.095), (0.05, 0.099), (0.0, 0.1)],
                24,
                &id_s,
                &|i, _, c| if i < 2 { vec3(0.10, 0.14, 0.26) } else { c },
                w,
            );
        }
        _ => {}
    }
}

// ===========================================================================
// Held items
// ===========================================================================
fn draw_rod(mb: &mut MB, base: Vec3, dir: Vec3, tier: u8, lure: Option<Vec3>, eye: Vec3, time: f32) {
    let t = (tier as usize).min(2);
    let col = vec3(RODS[t].6[0], RODS[t].6[1], RODS[t].6[2]);
    cyl(mb, base - dir * 0.04, base + dir * 0.32, 0.0225, 0.021, vec3(0.08, 0.08, 0.09), 8);
    cyl(mb, base + dir * 0.17, base + dir * 0.20, 0.026, 0.026, vec3(0.62, 0.64, 0.68), 8);
    let rc = rod_pt(base, dir, 0.245, 0.03) - Vec3::Y * 0.07;
    blob(mb, rc, vec3(0.03, 0.045, 0.045), vec3(0.12, 0.12, 0.14), 0.0, 8, 5, 0.0, shade);
    let mut prev = rod_pt(base, dir, 0.3, 0.03);
    for k in 1..=6 {
        let sx = 0.3 + k as f32 * 0.35;
        let p = rod_pt(base, dir, sx, 0.03);
        let rad = mixf(0.0175, 0.0042, k as f32 / 6.0);
        cyl(mb, prev, p, rad, rad * 0.9, col, 6);
        prev = p;
    }
    let tip = rod_pt(base, dir, 2.4, 0.03);
    match lure {
        Some(lp) => line_between(mb, tip, lp, 0.4, eye),
        None => {
            let lp = tip + vec3((time * 1.7).sin() * 0.03, -0.45, 0.0);
            thin(mb, tip, lp, eye, vec3(0.9, 0.9, 0.85));
            draw_lure(mb, lp);
        }
    }
}

// ===========================================================================
// draw_person
// ===========================================================================
pub fn draw_person(mb: &mut MB, feet: Vec3, look: &Look, pose: &Pose) {
    mb.ensure(9000, 22000);
    let v0 = mb.v.len();
    let fem = look.female;
    let k: f32 = if fem { 0.935 } else { 1.0 };
    let old = look.hair_color == 5 || look.hair_color == 6;
    let walk = pose.walk.clamp(0.0, 1.0);
    let ph = pose.phase;
    let tm = pose.time;
    let breathe = (tm * 1.6).sin() * (1.0 - walk * 0.7);
    let bob = 0.018 * walk * (2.0 * ph).cos() - 0.012 * walk;
    let lat = -0.02 * walk * ph.cos() + 0.008 * (tm * 0.5).sin() * (1.0 - walk);
    let lean_x = 0.07 * walk;
    let tw_p = -0.10 * walk * ph.sin();
    let tw_c = 0.07 * walk * ph.sin();
    let f0 = vec3(pose.yaw.cos(), 0.0, pose.yaw.sin());
    let r0 = vec3(-pose.yaw.sin(), 0.0, pose.yaw.cos());
    let base = feet + Vec3::Y * bob + r0 * lat;
    let fpel = frame(base, pose.yaw + tw_p);
    let fch = frame(base, pose.yaw + tw_c);
    let y = Vec3::Y;
    // legs
    let (mut hip, mut knee, mut ank, mut fd, mut fu) = ([Vec3::ZERO; 2], [Vec3::ZERO; 2], [Vec3::ZERO; 2], [Vec3::ZERO; 2], [Vec3::ZERO; 2]);
    for i in 0..2 {
        let s = sd(i);
        let lp = ph + if i == 0 { 0.0 } else { PI };
        let sp = lp.sin();
        let lift = lp.cos().max(0.0).powf(0.8) * 0.12 * walk;
        let fy = pose.yaw + s * 0.10;
        let ff = vec3(fy.cos(), 0.0, fy.sin());
        let g0 = feet + r0 * (s * 0.092) + f0 * ((0.5 * sp + 0.5 * (lp.sin().asin() * 0.6366)) * 0.30 * walk) + y * lift;
        let pitch = (0.32 * smooth(0.55, 1.0, sp) - 0.55 * smooth(0.5, 1.0, -sp) + 0.15 * lp.cos().max(0.0)) * walk;
        let d = ff * pitch.cos() + y * pitch.sin();
        let u = y * pitch.cos() - ff * pitch.sin();
        let a0 = if pitch < 0.0 { 0.14 } else { -0.05 };
        let target = g0 + ff * a0 - d * a0 + u * 0.085;
        hip[i] = base + y * 0.955 + fpel.r * (s * 0.088);
        let (kn, an) = ik2(hip[i], target, 0.445, 0.435, ff + fpel.r * s * 0.15);
        knee[i] = kn;
        ank[i] = an;
        fd[i] = d;
        fu[i] = u;
    }
    // head frame
    let stoop = if old { 0.02 } else { 0.0 };
    let hyaw = pose.yaw + tw_c * 0.3 + (tm * 0.37).sin() * 0.25 * (1.0 - walk);
    let hf = vec3(hyaw.cos(), 0.0, hyaw.sin());
    let pt: f32 = 0.05 + if old { 0.08 } else { 0.0 };
    let fh = hf * pt.cos() - y * pt.sin();
    let uh = y * pt.cos() + hf * pt.sin();
    let rh = vec3(-hyaw.sin(), 0.0, hyaw.cos());
    let hc = fch.p(0.012 + stoop + lean_x * 0.8, 1.705 + 0.003 * breathe, 0.0);
    // arms
    let shx = if fem { 0.158 } else { 0.178 };
    let mut sh = [Vec3::ZERO; 2];
    let mut wr = [Vec3::ZERO; 2];
    let mut el = [Vec3::ZERO; 2];
    let mut hd = [Vec3::ZERO; 2];
    let mut hn = [Vec3::ZERO; 2];
    let mut curl = [0.3f32; 2];
    let mut tg: [Option<(Vec3, Vec3, Vec3, f32)>; 2] = [None, None];
    for i in 0..2 {
        sh[i] = fch.p(lean_x * 0.52, 1.425 + 0.004 * breathe, sd(i) * shx);
    }
    let it = pose.item;
    let mut rod: Option<(Vec3, Vec3)> = None;
    let mut coco: Option<Vec3> = None;
    let mut lant: Option<Vec3> = None;
    let mut tin: Option<Vec3> = None;
    let mut bucket: Option<Vec3> = None;
    let palm_of = |w: Vec3, d: Vec3| w + d.normalize_or_zero() * 0.075;
    match it {
        Some(ItemKind::Rod) => {
            let dg = (fch.f * 0.8 - y * 0.35).normalize();
            let tr = sh[0] + fch.f * 0.28 - y * 0.36 - fch.r * 0.07;
            let rdir = (fch.f + y * 0.55).normalize();
            let rb = palm_of(tr, dg) + y * 0.02 - rdir * 0.08;
            let dgl = (fch.f * 0.8 - y * 0.2).normalize();
            let tl = rb + rdir * 0.30 - dgl * 0.075 - y * 0.02;
            tg[0] = Some((tr, dg, y, 0.95));
            tg[1] = Some((tl, dgl, y, 0.95));
            rod = Some((rb, rdir));
        }
        Some(ItemKind::Coconut) => {
            let dg = (fch.f * 0.7 - y * 0.3).normalize();
            let tr = sh[0] + fch.f * 0.30 - y * 0.30 - fch.r * 0.04;
            tg[0] = Some((tr, dg, y, 0.55));
            coco = Some(palm_of(tr, dg) + y * 0.07);
        }
        Some(ItemKind::Lantern) => {
            let dg = (-y * 0.9 + fch.f * 0.2).normalize();
            let tl = sh[1] + fch.f * 0.08 - y * 0.50 - fch.r * 0.0;
            tg[1] = Some((tl, dg, fch.f, 1.0));
            lant = Some(palm_of(tl, dg));
        }
        Some(ItemKind::Bait) => {
            let dg = (fch.f * 0.7 - y * 0.3).normalize();
            let tl = sh[1] + fch.f * 0.28 - y * 0.30 + fch.r * 0.05;
            tg[1] = Some((tl, dg, y, 0.5));
            tin = Some(palm_of(tl, dg) + y * 0.05);
        }
        Some(ItemKind::Bucket) => {
            let tr = sh[0] + fch.f * 0.03 - y * 0.53 + fch.r * 0.10;
            tg[0] = Some((tr, -y, fch.f, 1.0));
            bucket = Some(palm_of(tr, -y) - y * 0.17);
        }
        None => {}
    }
    for i in 0..2 {
        let s = sd(i);
        let th = -s * 0.36 * ph.sin() * walk + (tm * 0.9 + s).sin() * 0.02 * (1.0 - walk);
        let r = 0.567 - 0.017 * (th.max(0.0) / 0.36);
        let hang = sh[i] + (fch.f * th.sin() - y * th.cos()) * r + fch.r * s * 0.045;
        let pole = -fch.f * 0.5 - y * 0.3 + fch.r * s * 0.6;
        if let Some((t, d, n, c)) = tg[i] {
            let (e, w) = ik2(sh[i], t, 0.30, 0.27, pole);
            el[i] = e;
            wr[i] = w;
            hd[i] = d;
            hn[i] = n;
            curl[i] = c;
        } else {
            let (e, w) = ik2(sh[i], hang, 0.30, 0.27, pole);
            el[i] = e;
            wr[i] = w;
            hd[i] = ((w - e).normalize_or_zero() * 0.7 - y * 0.3).normalize();
            hn[i] = -fch.r * s;
            curl[i] = 0.3 + 0.1 * th.max(0.0);
        }
    }
    let sk = Sk {
        fem,
        old,
        lk: look.clone(),
        lean: lean_x,
        br: breathe,
        skin: v3(SKIN[(look.skin as usize).min(5)].1) * 0.94,
        hair: v3(HAIRC[(look.hair_color as usize).min(7)].1),
        eye: v3(EYES[(look.eyes as usize).min(6)].1),
        tc: cl(look.top_color),
        sc: cl(look.sleeve_color),
        bc: cl(look.bottom_color),
        feet: feet + r0 * lat,
        yaw: pose.yaw,
        bob,
        tw_p,
        tw_c,
        tt: if fem { &TT_F } else { &TT_M },
        fpel,
        fch,
        hip,
        knee,
        ank,
        fd,
        fu,
        sh,
        el,
        wr,
        hd,
        hn,
        curl,
        hc,
        fh,
        uh,
        rh,
    };
    torso(mb, &sk);
    shirt(mb, &sk);
    pants(mb, &sk);
    for i in 0..2 {
        shoe(mb, &sk, i);
        arm(mb, &sk, i);
    }
    face(mb, &sk);
    hair_and_beard(mb, &sk);
    if look.hat > 0 {
        hat(mb, &sk);
    }
    // scale the whole body about the feet
    if (k - 1.0).abs() > 1e-4 {
        let start = if mb.v.len() >= v0 { v0 } else { 0 };
        for v in mb.v[start..].iter_mut() {
            v.position = feet + (v.position - feet) * k;
        }
    }
    let w = |p: Vec3| feet + (p - feet) * k;
    if let Some((b, d)) = rod {
        draw_rod(mb, w(b), d, pose.tier, pose.lure, pose.eye, tm);
    }
    if let Some(p) = coco {
        blob(mb, w(p), vec3(0.105, 0.085, 0.085) * k, vec3(0.42, 0.30, 0.16), 0.0, 9, 6, pose.yaw, shade);
    }
    if let Some(p) = lant {
        lantern_model(mb, w(p) - Vec3::Y * 0.30, 1.0, pose.yaw, pose.lit);
    }
    if let Some(p) = tin {
        draw_bait_tin(mb, w(p), Vec3::Y);
    }
    if let Some(p) = bucket {
        draw_bucket(mb, w(p), Vec3::Y);
    }
}

/// Rotating full-body preview for the character menu, drawn inside the pixel rectangle `rect = (x, y, w, h)`.
pub fn draw_preview(look: &Look, yaw: f32, time: f32, mat: &Material, rect: (f32, f32, f32, f32)) {
    let (x, y, w, h) = rect;
    draw_rectangle(x, y, w, h, Color::new(0.06, 0.08, 0.10, 0.92));
    draw_rectangle_lines(x, y, w, h, 2.0, Color::new(0.30, 0.38, 0.45, 0.9));
    let k = if look.female { 0.935 } else { 1.0 };
    let asp = (w / h).max(0.1);
    let fovy = 0.52f32;
    let t = (fovy * 0.5).tan();
    let d = (2.1 * k * 0.5 / t).max(1.2 * 0.5 / (t * asp));
    let target = vec3(0.0, 0.98 * k, 0.0);
    let cam = target + vec3(0.0, 0.2, d);
    let pose = Pose { yaw: PI * 0.5 + yaw, phase: 0.0, walk: 0.0, item: None, tier: 0, lure: None, lit: false, eye: cam, time };
    let mut mb = MB::new();
    cyl(&mut mb, vec3(0.0, -0.03, 0.0), vec3(0.0, 0.0, 0.0), 0.6, 0.6, vec3(0.16, 0.19, 0.22), 28);
    draw_person(&mut mb, Vec3::ZERO, look, &pose);
    mb.flush();
    let dpi = screen_dpi_scale();
    let vp = ((x * dpi) as i32, ((screen_height() - y - h) * dpi) as i32, (w * dpi) as i32, (h * dpi) as i32);
    set_camera(&Camera3D { position: cam, target, up: Vec3::Y, fovy, aspect: Some(asp), projection: Projection::Perspective, render_target: None, viewport: Some(vp), z_near: 0.05, z_far: 60.0 });
    unsafe {
        let mut gl = get_internal_gl();
        gl.flush();
        gl.quad_context.clear(None, Some(1.0), None);
    }
    mat.set_uniform("CamPos", cam);
    mat.set_uniform("Under", 0.0f32);
    mat.set_uniform("Lum", 1.0f32);
    mat.set_uniform("Bump", 0.0f32);
    mat.set_uniform("SunDir", vec3(0.45, 0.55, 0.70).normalize());
    mat.set_uniform("SunCol", vec3(1.0, 0.95, 0.85) * 0.9);
    mat.set_uniform("AmbCol", vec3(0.38, 0.40, 0.46));
    mat.set_uniform("FogCol", vec3(0.5, 0.55, 0.6));
    for n in ["P0", "P1", "P2", "P3", "C0", "C1", "C2", "C3"] {
        mat.set_uniform(n, Vec3::ZERO);
    }
    gl_use_material(mat);
    for m in &mb.meshes {
        draw_mesh(m);
    }
    gl_use_default_material();
    set_default_camera();
}
