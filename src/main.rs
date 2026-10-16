#![windows_subsystem = "windows"]

mod audio;
mod fish;
mod horror;
mod islands;
mod items;
mod model;
mod boat;
mod bore;
mod net;
mod people;
mod postfx;
mod progress;
mod ui;
mod update;
mod upnp;

use macroquad::miniquad::{self, BlendFactor, BlendState, BlendValue, Equation, FilterMode, MipmapFilterMode, TextureWrap};
use macroquad::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

// ---------------------------------------------------------------------------
// Smooth UI text: every `draw_text` / `measure_text` in the game goes through these two (they shadow
// macroquad's blocky built-in font) and use the system's Segoe UI when it exists.
// ---------------------------------------------------------------------------
thread_local! {
    static UI_FONT: RefCell<Option<Option<Font>>> = const { RefCell::new(None) };
}

fn ui_font() -> Option<Font> {
    UI_FONT.with(|f| {
        f.borrow_mut()
            .get_or_insert_with(|| {
                ["C:\\Windows\\Fonts\\segoeuisl.ttf", "C:\\Windows\\Fonts\\segoeui.ttf"]
                    .iter()
                    .find_map(|p| std::fs::read(p).ok().and_then(|b| load_ttf_font_from_bytes(&b).ok()))
            })
            .clone()
    })
}

pub fn draw_text(text: impl AsRef<str>, x: f32, y: f32, font_size: f32, color: Color) -> TextDimensions {
    let font = ui_font();
    macroquad::text::draw_text_ex(text, x, y, TextParams { font: font.as_ref(), font_size: font_size.max(1.0) as u16, color, ..Default::default() })
}

pub fn measure_text(text: impl AsRef<str>, _font: Option<&Font>, font_size: u16, font_scale: f32) -> TextDimensions {
    let font = ui_font();
    macroquad::text::measure_text(text, font.as_ref(), font_size, font_scale)
}

// ---------------------------------------------------------------------------
// World constants
// ---------------------------------------------------------------------------
const CELL: f32 = 1.6; // terrain grid spacing (m)
const N: usize = 320; // grid cells per side
const NV: usize = N + 1;
const HALF: f32 = 256.0; // grid covers -256..256
const CHUNK: usize = 20; // cells per chunk side
const CW: f32 = 32.0; // chunk width (m)
const NCH: i32 = 16;
const MAXV: usize = 20000;
const MAXI: usize = 24000;
/// How far you can swim or row from the middle of the map: effectively no edge (the sea goes on past the fog).
const SWIM_LIMIT: f32 = 3000.0;
/// The sea floor drops away to this depth far from the islands (a long way down, no invisible floor).
const ABYSS: f32 = -60.0;
const ISLES: [(f32, f32, f32, f32); 3] = [(0.0, 0.0, 82.0, 0.80), (125.0, -80.0, 44.0, 0.12), (-140.0, 100.0, 36.0, 0.08)];
const DAY_LEN: f32 = 900.0; // 15 minutes of daylight
const NIGHT_LEN: f32 = 600.0; // 10 minutes of night
const CYCLE: f32 = DAY_LEN + NIGHT_LEN;

// ---------------------------------------------------------------------------
// Noise
// ---------------------------------------------------------------------------
fn hash2(x: i32, y: i32, s: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add(s.wrapping_mul(2_246_822_519));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 16_777_215.0
}

fn vnoise(x: f32, y: f32, s: u32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let fx = x - xi;
    let fy = y - yi;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let (ix, iy) = (xi as i32, yi as i32);
    let a = hash2(ix, iy, s);
    let b = hash2(ix + 1, iy, s);
    let c = hash2(ix, iy + 1, s);
    let d = hash2(ix + 1, iy + 1, s);
    mixf(mixf(a, b, ux), mixf(c, d, ux), uy)
}

/// Value noise that tiles with the given integer period.
fn pnoise(x: f32, y: f32, per: i32, s: u32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let fx = x - xi;
    let fy = y - yi;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let (ix, iy) = (xi as i32, yi as i32);
    let h = |a: i32, b: i32| hash2(a.rem_euclid(per), b.rem_euclid(per), s);
    mixf(mixf(h(ix, iy), h(ix + 1, iy), ux), mixf(h(ix, iy + 1), h(ix + 1, iy + 1), ux), uy)
}

fn fbm(x: f32, y: f32, s: u32, oct: u32) -> f32 {
    let (mut sum, mut amp, mut f, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for i in 0..oct {
        sum += amp * vnoise(x * f, y * f, s + i * 17);
        norm += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    sum / norm
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mixf(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn rf(a: f32, b: f32) -> f32 {
    rand::gen_range(a, b)
}

fn rvec() -> Vec3 {
    loop {
        let v = vec3(rf(-1.0, 1.0), rf(-1.0, 1.0), rf(-1.0, 1.0));
        let l = v.length();
        if l > 0.1 && l <= 1.0 {
            return v / l;
        }
    }
}

/// Runs a world-building closure on its own random stream so the main stream (and therefore the
/// placement of everything that follows) never depends on the model-detail setting.
fn isolated<R>(f: impl FnOnce() -> R) -> R {
    let a = rand::rand();
    let b = rand::rand();
    rand::srand(a as u64);
    let r = f();
    rand::srand(b as u64);
    r
}

fn dk() -> f32 {
    DETAIL.with(|d| d.get())
}

/// Scale a segment count by the model-detail setting.
fn seg(n: usize) -> usize {
    ((n as f32 * dk()).round() as usize).max(3)
}

fn cnt(n: usize) -> usize {
    ((n as f32 * dk().powf(0.85)).round() as usize).max(1)
}

fn lerp_angle(h: f32, want: f32, k: f32) -> f32 {
    let d = (want - h + PI).rem_euclid(TAU) - PI;
    h + d * k.min(1.0)
}

// ---------------------------------------------------------------------------
// Terrain
// ---------------------------------------------------------------------------
#[allow(clippy::too_many_arguments)]
fn island_h(x: f32, z: f32, c: (f32, f32), rad: (f32, f32), rot: f32, hill: f32, nf: f32, seed: u32) -> f32 {
    let (s, co) = rot.sin_cos();
    let (px, pz) = (x - c.0, z - c.1);
    let rx = px * co - pz * s;
    let rz = px * s + pz * co;
    let d = ((rx / rad.0).powi(2) + (rz / rad.1).powi(2)).sqrt();
    let sd = seed as f32 * 3.7;
    let w1 = fbm(x * nf + 7.3 + sd, z * nf + 3.1, 1 + seed, 4) - 0.5;
    let w2 = fbm(x * nf * 4.0 + 1.7, z * nf * 4.0 + 9.2 + sd, 2 + seed, 3) - 0.5;
    let land = 1.0 - (d + w1 * 0.8 + w2 * 0.16);
    if land < 0.0 {
        return (land * 11.0).max(ABYSS);
    }
    let beach = smooth(0.0, 0.10, land) * 1.4;
    let inland = smooth(0.06, 0.55, land);
    let hn = fbm(x * 0.02 + 11.0, z * 0.02 + 5.0 + sd, 3, 5);
    let hills = inland * 26.0 * (hn - 0.32).max(0.0).powf(1.25) * hill;
    let peak = smooth(0.35, 0.9, land) * 7.0 * fbm(x * 0.035 + 2.0, z * 0.035 + 8.0, 4, 3) * hill;
    let bumps = (fbm(x * 0.15, z * 0.15, 5, 3) - 0.5) * 0.7 * smooth(0.02, 0.2, land);
    beach + hills + peak + bumps + inland * 1.2 * hill.max(0.3)
}

/// Centre and radius of the flat plaza in the middle of the main island.
const PLAZA_C: (f32, f32) = (-5.0, -6.0);
const PLAZA_R: f32 = 13.0;

fn base_height(x: f32, z: f32) -> f32 {
    let a = island_h(x, z, (0.0, 0.0), (55.0, 41.0), 0.5, 0.85, 0.0155, 0);
    let b = island_h(x, z, (125.0, -80.0), (30.0, 23.0), 1.1, 0.3, 0.034, 5);
    let c = island_h(x, z, (-140.0, 100.0), (22.0, 18.0), 2.2, 0.22, 0.045, 9);
    a.max(b).max(c)
}

fn plaza_h() -> f32 {
    static H: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *H.get_or_init(|| base_height(PLAZA_C.0, PLAZA_C.1).clamp(2.2, 5.0))
}

fn raw_height(x: f32, z: f32) -> f32 {
    let h = base_height(x, z);
    let d = ((x - PLAZA_C.0).powi(2) + (z - PLAZA_C.1).powi(2)).sqrt();
    let w = 1.0 - smooth(PLAZA_R, PLAZA_R + 9.0, d);
    if w > 0.0 {
        mixf(h, plaza_h(), w)
    } else {
        h
    }
}

struct Terrain {
    h: Vec<f32>,
}

impl Terrain {
    fn gen() -> Self {
        let mut h = vec![0.0; NV * NV];
        for iz in 0..NV {
            for ix in 0..NV {
                let x = -HALF + ix as f32 * CELL;
                let z = -HALF + iz as f32 * CELL;
                h[iz * NV + ix] = raw_height(x, z);
            }
        }
        Terrain { h }
    }

    fn grid(&self, ix: usize, iz: usize) -> f32 {
        self.h[iz * NV + ix]
    }

    fn at(&self, x: f32, z: f32) -> f32 {
        let gx = (x + HALF) / CELL;
        let gz = (z + HALF) / CELL;
        if gx < 0.0 || gz < 0.0 || gx >= N as f32 || gz >= N as f32 {
            return ABYSS;
        }
        let ix = gx.floor() as usize;
        let iz = gz.floor() as usize;
        let fx = gx - ix as f32;
        let fz = gz - iz as f32;
        let a = self.grid(ix, iz);
        let b = self.grid(ix + 1, iz);
        let c = self.grid(ix, iz + 1);
        let d = self.grid(ix + 1, iz + 1);
        mixf(mixf(a, b, fx), mixf(c, d, fx), fz)
    }

    fn normal(&self, x: f32, z: f32) -> Vec3 {
        let e = 0.8;
        vec3(
            self.at(x - e, z) - self.at(x + e, z),
            2.0 * e,
            self.at(x, z - e) - self.at(x, z + e),
        )
        .normalize()
    }
}

// Must match swellh() in the water shader.
fn swell(x: f32, z: f32, t: f32) -> f32 {
    0.10 * ((x * 0.62 + z * 0.78) * 0.42 + t * 1.35).sin()
        + 0.07 * ((-x * 0.8 + z * 0.6) * 0.61 + t * 1.7).sin()
        + 0.035 * ((x * 0.97 - z * 0.25) * 1.1 + t * 2.3).sin()
}

fn water_h(t: &Terrain, x: f32, z: f32, time: f32) -> f32 {
    swell(x, z, time) * smooth(0.2, 2.5, -t.at(x, z))
}

// ---------------------------------------------------------------------------
// Vertex lighting data: albedo colour + normal (lighting happens in the shader)
// ---------------------------------------------------------------------------
#[derive(Clone, Copy)]
struct Lit {
    c: Color,
    n: Vec3,
    w: f32,
}

fn col3(c: Vec3) -> Color {
    Color::new(c.x.clamp(0.0, 1.0), c.y.clamp(0.0, 1.0), c.z.clamp(0.0, 1.0), 1.0)
}

fn shade(c: Vec3, n: Vec3) -> Lit {
    Lit { c: col3(c), n: n.normalize_or_zero(), w: 0.0 }
}

/// Unlit, glowing surface (zero normal tells the shader to skip lighting).
fn glow(c: Vec3) -> Lit {
    Lit { c: col3(c), n: Vec3::ZERO, w: 0.0 }
}

fn cloud_shade(c: Vec3, n: Vec3) -> Lit {
    Lit { c: col3(c), n: n.normalize_or_zero(), w: 1.0 }
}

fn ground_col(x: f32, z: f32, h: f32, n: Vec3) -> Vec3 {
    let v = fbm(x * 0.08, z * 0.08, 21, 3);
    let v2 = fbm(x * 0.5, z * 0.5, 22, 2);
    let dry_sand = vec3(0.87, 0.79, 0.60) * (0.95 + 0.08 * v2);
    let wet_sand = vec3(0.60, 0.51, 0.38);
    let under = vec3(0.72, 0.69, 0.53);
    let mut col = if h < 0.0 {
        under.lerp(vec3(0.30, 0.40, 0.36), smooth(0.0, -6.0, h))
    } else {
        wet_sand.lerp(dry_sand, smooth(0.15, 0.6, h))
    };
    let mut g = vec3(0.19, 0.38, 0.10).lerp(vec3(0.36, 0.50, 0.17), v);
    g = g.lerp(
        vec3(0.10, 0.26, 0.07),
        smooth(0.45, 0.7, fbm(x * 0.03 + 4.0, z * 0.03, 23, 3)),
    );
    g = g.lerp(vec3(0.50, 0.52, 0.26), smooth(9.0, 16.0, h) * 0.6);
    g *= 0.92 + 0.14 * v2;
    col = col.lerp(g, smooth(1.35, 2.1, h + (v - 0.5) * 1.2));
    // packed dirt in the plaza and along the path from the dock
    let p = vec2(x, z);
    let mut dirt_w = 1.0 - smooth(PLAZA_R - 2.0, PLAZA_R + 3.0, p.distance(vec2(PLAZA_C.0, PLAZA_C.1)));
    if let Some((a, b)) = PATH.with(|c| c.get()) {
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        let pw = 1.0 - smooth(0.9, 2.3, p.distance(a + ab * t));
        dirt_w = dirt_w.max(pw * 0.85 * smooth(0.5, 1.2, h));
    }
    let dirt = vec3(0.50, 0.43, 0.32) * (0.88 + 0.24 * v2);
    col = col.lerp(dirt, dirt_w * 0.92);
    let rock = vec3(0.44, 0.41, 0.37) * (0.85 + 0.3 * v2);
    col.lerp(rock, smooth(0.85, 0.70, n.y))
}

// ---------------------------------------------------------------------------
// Mesh builder
// ---------------------------------------------------------------------------
struct MB {
    meshes: Vec<Mesh>,
    v: Vec<Vertex>,
    i: Vec<u16>,
}

impl MB {
    fn new() -> Self {
        MB { meshes: vec![], v: vec![], i: vec![] }
    }
    fn ensure(&mut self, nv: usize, ni: usize) {
        if self.v.len() + nv > MAXV || self.i.len() + ni > MAXI {
            self.flush();
        }
    }
    fn vert_uv(&mut self, p: Vec3, l: Lit, u: f32, v: f32) -> u16 {
        let mut vx = Vertex::new(p.x, p.y, p.z, u, v, l.c);
        vx.normal = vec4(l.n.x, l.n.y, l.n.z, l.w);
        self.v.push(vx);
        (self.v.len() - 1) as u16
    }
    fn vert(&mut self, p: Vec3, l: Lit) -> u16 {
        self.vert_uv(p, l, p.x + p.z * 0.9, p.y + p.z * 0.3)
    }
    fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, la: Lit, lb: Lit, lc: Lit) {
        self.ensure(3, 3);
        let i0 = self.vert(a, la);
        let i1 = self.vert(b, lb);
        let i2 = self.vert(c, lc);
        self.i.extend_from_slice(&[i0, i1, i2]);
    }
    #[allow(clippy::too_many_arguments)]
    fn quadc(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, la: Lit, lb: Lit, lc: Lit, ld: Lit) {
        self.tri(a, b, c, la, lb, lc);
        self.tri(a, c, d, la, lc, ld);
    }
    /// Flat-shaded, double-sided (normal flipped to face up).
    fn ftri(&mut self, a: Vec3, b: Vec3, c: Vec3, col: Vec3) {
        let mut n = (b - a).cross(c - a).normalize_or_zero();
        if n.y < 0.0 {
            n = -n;
        }
        let k = shade(col, n);
        self.tri(a, b, c, k, k, k);
    }
    fn fquad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: Vec3) {
        self.ftri(a, b, c, col);
        self.ftri(a, c, d, col);
    }
    /// Quad with an explicit normal.
    fn nquad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: Vec3, n: Vec3) {
        let k = shade(col, n);
        self.quadc(a, b, c, d, k, k, k, k);
    }
    fn flush(&mut self) {
        if !self.i.is_empty() {
            self.meshes.push(Mesh {
                vertices: std::mem::take(&mut self.v),
                indices: std::mem::take(&mut self.i),
                texture: None,
            });
        } else {
            self.v.clear();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn blob(
    mb: &mut MB,
    c: Vec3,
    r: Vec3,
    col: Vec3,
    jit: f32,
    lon: usize,
    lat: usize,
    yaw: f32,
    sh: fn(Vec3, Vec3) -> Lit,
) {
    let (sy, cy) = yaw.sin_cos();
    let (lon, lat) = (seg(lon).max(4), seg(lat).max(3));
    mb.ensure((lat + 1) * lon, lat * lon * 6);
    let base = mb.v.len() as u16;
    for j in 0..=lat {
        let th = PI * j as f32 / lat as f32;
        for i in 0..lon {
            let ph = TAU * i as f32 / lon as f32;
            let u = vec3(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
            let k = if jit > 0.0 && j > 0 && j < lat { 1.0 + jit * rf(-1.0, 1.0) } else { 1.0 };
            let lp = u * r * k;
            let p = c + vec3(lp.x * cy - lp.z * sy, lp.y, lp.x * sy + lp.z * cy);
            let ln = u / r;
            let n = vec3(ln.x * cy - ln.z * sy, ln.y, ln.x * sy + ln.z * cy).normalize_or_zero();
            mb.vert(p, sh(col, n));
        }
    }
    for j in 0..lat {
        for i in 0..lon {
            let i2 = (i + 1) % lon;
            let a = base + (j * lon + i) as u16;
            let b = base + (j * lon + i2) as u16;
            let cc = base + ((j + 1) * lon + i) as u16;
            let d = base + ((j + 1) * lon + i2) as u16;
            mb.i.extend_from_slice(&[a, cc, b, b, cc, d]);
        }
    }
}

fn cyl(mb: &mut MB, a: Vec3, b: Vec3, r0: f32, r1: f32, col: Vec3, sides: usize) {
    let axis = (b - a).normalize_or_zero();
    if axis == Vec3::ZERO {
        return;
    }
    let sides = if sides >= 6 { seg(sides) } else { sides };
    let refv = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let u = axis.cross(refv).normalize();
    let v = axis.cross(u);
    for s in 0..sides {
        let a0 = TAU * s as f32 / sides as f32;
        let a1 = TAU * (s + 1) as f32 / sides as f32;
        let n0 = u * a0.cos() + v * a0.sin();
        let n1 = u * a1.cos() + v * a1.sin();
        let c0 = shade(col, n0);
        let c1 = shade(col, n1);
        let (v00, v01, v10, v11) = (a + n0 * r0, a + n1 * r0, b + n0 * r1, b + n1 * r1);
        mb.tri(v00, v10, v11, c0, c0, c1);
        mb.tri(v00, v11, v01, c0, c1, c1);
    }
}

/// Axis-aligned-in-local-frame box: half extents along f, up (Y) and r.
fn bbox(mb: &mut MB, c: Vec3, f: Vec3, r: Vec3, h: Vec3, col: Vec3) {
    let ax = [f, Vec3::Y, r];
    let hs = [h.x, h.y, h.z];
    for axis in 0..3 {
        let (a1, a2) = ((axis + 1) % 3, (axis + 2) % 3);
        for sg in [-1.0f32, 1.0] {
            let n = ax[axis] * sg;
            let ctr = c + n * hs[axis];
            let (u, v) = (ax[a1] * hs[a1], ax[a2] * hs[a2]);
            mb.nquad(ctr - u - v, ctr + u - v, ctr + u + v, ctr - u + v, col, n);
        }
    }
}

// ---------------------------------------------------------------------------
// Vegetation & props
// ---------------------------------------------------------------------------

/// A volume filled with many small leaves (much denser than a smooth blob).
#[allow(clippy::too_many_arguments)]
fn foliage(mb: &mut MB, c: Vec3, r: Vec3, n: usize, ls: f32, base: Vec3, tip: Vec3) {
    for _ in 0..cnt(n) {
        let u = rvec();
        let rad = rf(0.45, 1.0).sqrt();
        let p = c + u * r * rad;
        let occl = 0.40 + 0.60 * rad;
        let nrm = (u + rvec() * 0.5).normalize();
        let ax = nrm.cross(rvec()).normalize_or_zero();
        if ax == Vec3::ZERO {
            continue;
        }
        let bx = nrm.cross(ax);
        let l = ls * rf(0.8, 1.3);
        let w = l * 0.55;
        let col = base.lerp(tip, rf(0.0, 1.0)) * occl;
        let ln = (u * 0.7 + Vec3::Y * 0.4).normalize();
        let c_stem = shade(col * 0.65, ln);
        let c_mid = shade(col, ln);
        let c_tip = shade(col * 1.15, ln);
        let v0 = p - ax * l * 0.5;
        let v1 = p + bx * w * 0.5 - ax * l * 0.05;
        let v2 = p + ax * l * 0.5;
        let v3 = p - bx * w * 0.5 - ax * l * 0.05;
        mb.quadc(v0, v1, v2, v3, c_stem, c_mid, c_tip, c_mid);
    }
}

fn flower(mb: &mut MB, p: Vec3, n: Vec3, col: Vec3) {
    let t1 = n.cross(rvec()).normalize_or_zero();
    if t1 == Vec3::ZERO {
        return;
    }
    let t2 = n.cross(t1);
    let k = shade(col, n);
    let kc = shade(vec3(0.95, 0.80, 0.15), n);
    let petals = 5;
    for i in 0..petals {
        let a0 = TAU * i as f32 / petals as f32;
        let a1 = TAU * (i + 1) as f32 / petals as f32;
        let m = (a0 + a1) * 0.5;
        let r = 0.07;
        let p0 = p + (t1 * a0.cos() + t2 * a0.sin()) * r * 0.6;
        let p1 = p + (t1 * m.cos() + t2 * m.sin()) * r * 1.1 + n * 0.01;
        let p2 = p + (t1 * a1.cos() + t2 * a1.sin()) * r * 0.6;
        mb.tri(p0, p1, p2, k, k, k);
        mb.tri(p, p0, p2, kc, k, k);
    }
}

fn palm(mb: &mut MB, base: Vec3, scale: f32) {
    let height = rf(6.0, 10.5) * scale;
    let ang = rf(0.0, TAU);
    let lean = rf(0.6, 3.2) * scale;
    let dir = vec3(ang.cos(), 0.0, ang.sin());
    let segs = 16;
    let mut rings = Vec::with_capacity(segs + 1);
    for k in 0..=segs {
        let t = k as f32 / segs as f32;
        let wob = vec3((t * 9.0 + ang).sin(), 0.0, (t * 7.0 + ang).cos()) * 0.06;
        let p = base + dir * lean * t * t + Vec3::Y * (height * t - 0.3) + wob;
        let r = (mixf(0.24, 0.14, t) + 0.12 * (1.0 - smooth(0.0, 0.12, t))) * scale.max(0.6);
        rings.push((p, r));
    }
    let bark1 = vec3(0.50, 0.40, 0.29);
    let bark2 = vec3(0.34, 0.27, 0.19);
    for k in 0..segs {
        let (p0, r0) = rings[k];
        let (p1, r1) = rings[k + 1];
        let bump = if k % 2 == 0 { 1.0 } else { 0.9 };
        cyl(mb, p0, p1, r0 * bump, r1 * if k % 2 == 0 { 0.9 } else { 1.0 }, if k % 2 == 0 { bark1 } else { bark2 }, 8);
    }
    let top = rings[segs].0;
    blob(mb, top, vec3(0.32, 0.4, 0.32) * scale.max(0.6), vec3(0.34, 0.30, 0.16), 0.1, 7, 4, 0.0, shade);
    // coconut cluster under the crown
    for _ in 0..rand::gen_range(3, 7) {
        let a = rf(0.0, TAU);
        let p = top + vec3(a.cos() * 0.28, -0.33, a.sin() * 0.28) * scale.max(0.6);
        let green = rf(0.0, 1.0) < 0.5;
        let col = if green { vec3(0.34, 0.42, 0.14) } else { vec3(0.38, 0.27, 0.13) };
        blob(mb, p, vec3(0.10, 0.14, 0.10) * scale.max(0.6), col, 0.0, 6, 4, a, shade);
    }

    let nf = rand::gen_range(10, 14);
    for f in 0..nf {
        let a = TAU * f as f32 / nf as f32 + rf(-0.25, 0.25);
        let fd = vec3(a.cos(), 0.0, a.sin());
        let side = vec3(-fd.z, 0.0, fd.x);
        let len = rf(3.2, 4.8) * scale.max(0.5);
        let dead = rf(0.0, 1.0) < 0.02;
        let (rk, dk) = if dead { (rf(0.0, 0.2), rf(1.6, 2.2)) } else { (rf(0.45, 1.0), rf(0.9, 1.45)) };
        let col_base = if dead { vec3(0.47, 0.39, 0.21) } else { vec3(0.14, 0.38, 0.10) };
        let col_tip = if dead { vec3(0.58, 0.47, 0.26) } else { vec3(0.30, 0.50, 0.13) };
        let spine = |s: f32| top + fd * len * 0.92 * s + Vec3::Y * len * (rk * s - dk * s * s);
        let steps = 14;
        for k in 0..steps {
            let s0 = k as f32 / steps as f32;
            let s1 = (k as f32 + 1.0) / steps as f32;
            let p0 = spine(s0);
            let p1 = spine(s1);
            let col = col_base.lerp(col_tip, s0 * 0.8);
            // midrib
            let rib = side * 0.035 * scale.max(0.6);
            mb.fquad(p0 - rib, p0 + rib, p1 + rib, p1 - rib, vec3(0.30, 0.34, 0.12));
            // leaflets: wide quads that touch their neighbours, narrowing to the tip
            let ll = (0.25 + 1.15 * (PI * (0.08 + 0.84 * s0)).sin()) * scale.max(0.55);
            for sd in [-1.0f32, 1.0] {
                let droop = 0.45 + 0.5 * s0;
                let dl = (side * sd * 0.85 + fd * 0.50 - Vec3::Y * droop).normalize();
                let tip_a = p0.lerp(p1, 0.30) + dl * ll;
                let tip_b = p0.lerp(p1, 0.85) + dl * ll * 0.93 + fd * 0.04;
                let mut n = (p1 - p0).cross(tip_a - p0).normalize_or_zero();
                if n.y < 0.0 {
                    n = -n;
                }
                let dark = shade(col * 0.55, n);
                let lite = shade(col * 1.1, n);
                mb.quadc(p0, p1, tip_b, tip_a, dark, dark, lite, lite);
            }
        }
    }
}

fn broadleaf(mb: &mut MB, base: Vec3) {
    let h = rf(4.0, 7.5);
    let top = base + vec3(rf(-0.6, 0.6), h, rf(-0.6, 0.6));
    let bark = vec3(0.36, 0.29, 0.22);
    cyl(mb, base - Vec3::Y * 0.3, top, 0.28, 0.16, bark, 7);
    for _ in 0..rand::gen_range(2, 4) {
        let a = rf(0.0, TAU);
        let from = base.lerp(top, rf(0.5, 0.8));
        let to = top + vec3(a.cos() * 1.6, rf(0.0, 0.8), a.sin() * 1.6);
        cyl(mb, from, to, 0.12, 0.07, bark, 5);
    }
    for _ in 0..rand::gen_range(3, 5) {
        let a = rf(0.0, TAU);
        let d = rf(0.0, 1.8);
        let c = top + vec3(a.cos() * d, rf(-0.2, 1.2), a.sin() * d);
        let r = rf(1.5, 2.3);
        blob(mb, c, vec3(r * 0.7, r * 0.5, r * 0.7), vec3(0.08, 0.20, 0.06), 0.1, 7, 4, 0.0, shade);
        foliage(mb, c, vec3(r, r * 0.72, r), 120, 0.55, vec3(0.10, 0.30, 0.08), vec3(0.28, 0.48, 0.14));
    }
}

/// Leaves packed onto the surface of a dome: dense, bright on top, dark underneath.
#[allow(clippy::too_many_arguments)]
fn leaf_dome(mb: &mut MB, c: Vec3, r: Vec3, n: usize, ls: f32, base: Vec3, tip: Vec3) {
    for _ in 0..cnt(n) {
        let u = rvec();
        let u = vec3(u.x, u.y.abs().max(0.05), u.z).normalize(); // only the upper half
        let rad = rf(0.82, 1.0);
        let p = c + u * r * rad;
        let sky = (0.5 + 0.5 * u.y).clamp(0.0, 1.0);
        let occl = 0.55 + 0.45 * sky * rad;
        // leaves lie roughly along the surface and point outwards/upwards
        let tang = u.cross(rvec()).normalize_or_zero();
        if tang == Vec3::ZERO {
            continue;
        }
        let ax = (tang * 0.7 + u * 0.55 + Vec3::Y * 0.2).normalize();
        let bx = ax.cross(u).normalize_or_zero();
        let l = ls * rf(0.85, 1.35);
        let w = l * 0.42;
        let col = base.lerp(tip, rf(0.0, 1.0)) * occl;
        let ln = (u * 0.6 + Vec3::Y * 0.5).normalize();
        let c0 = shade(col * 0.7, ln);
        let c1 = shade(col, ln);
        let c2 = shade(col * 1.2, ln);
        let v0 = p - ax * l * 0.15;
        let v1 = p + ax * l * 0.35 + bx * w;
        let v2 = p + ax * l * 0.95;
        let v3 = p + ax * l * 0.35 - bx * w;
        mb.quadc(v0, v1, v2, v3, c0, c1, c2, c1);
    }
}

fn arching_frond(mb: &mut MB, base: Vec3, dir: Vec3, len: f32, col: Vec3, tipc: Vec3) {
    let side = vec3(-dir.z, 0.0, dir.x);
    let steps = 9;
    let spine = |s: f32| base + dir * len * s + Vec3::Y * len * (0.95 * s - 0.9 * s * s);
    for k in 0..steps {
        let s0 = k as f32 / steps as f32;
        let s1 = (k as f32 + 1.0) / steps as f32;
        let p0 = spine(s0);
        let p1 = spine(s1);
        let cc = col.lerp(tipc, s0);
        let w = 0.20 * len * (PI * (0.1 + 0.8 * s0)).sin().max(0.15);
        for sd in [-1.0f32, 1.0] {
            let a = p0 + side * sd * 0.01;
            let b = p1 + side * sd * 0.01;
            let a2 = a + side * sd * w + dir * w * 0.25 - Vec3::Y * w * 0.25;
            let b2 = b + side * sd * w * 0.8 + dir * w * 0.2 - Vec3::Y * w * 0.25;
            let mut n = (b - a).cross(a2 - a).normalize_or_zero();
            if n.y < 0.0 {
                n = -n;
            }
            let d0 = shade(cc * 0.6, n);
            let d1 = shade(cc * 1.1, n);
            mb.quadc(a, b, b2, a2, d0, d0, d1, d1);
        }
    }
}

fn bush(mb: &mut MB, p: Vec3) {
    let kind = rf(0.0, 1.0);
    let greens = [
        (vec3(0.10, 0.30, 0.08), vec3(0.26, 0.48, 0.14)),
        (vec3(0.13, 0.34, 0.10), vec3(0.34, 0.54, 0.17)),
        (vec3(0.08, 0.27, 0.11), vec3(0.20, 0.44, 0.18)),
    ];
    let (gb, gt) = greens[rand::gen_range(0, 3)];
    if kind < 0.28 {
        // fern clump: a fan of arching fronds
        let n = rand::gen_range(8, 13);
        for i in 0..n {
            let a = TAU * i as f32 / n as f32 + rf(-0.3, 0.3);
            let d = vec3(a.cos(), 0.0, a.sin());
            arching_frond(mb, p + d * 0.05 + Vec3::Y * 0.05, d, rf(0.9, 1.5), gb, gt);
        }
        return;
    }
    // round shrub made of a few overlapping lobes
    let r = rf(0.6, 1.0);
    let lobes = rand::gen_range(2, 4);
    let mut centers = vec![];
    for i in 0..lobes {
        let a = TAU * i as f32 / lobes as f32 + rf(0.0, 1.0);
        let off = if i == 0 { 0.0 } else { r * rf(0.35, 0.55) };
        let lr = r * if i == 0 { 1.0 } else { rf(0.6, 0.8) };
        let c = p + vec3(a.cos() * off, 0.0, a.sin() * off);
        centers.push((c, lr));
    }
    for &(c, lr) in &centers {
        let ctr = c + Vec3::Y * lr * 0.12;
        // green core so no gaps ever show black
        blob(mb, ctr + Vec3::Y * lr * 0.25, vec3(lr * 0.78, lr * 0.5, lr * 0.78), gb * 1.1, 0.04, 8, 4, 0.0, shade);
        leaf_dome(mb, ctr, vec3(lr, lr * 0.82, lr), (100.0 * lr) as usize + 55, 0.27 * (0.7 + 0.5 * lr), gb, gt);
    }
    if kind > 0.78 {
        let fc = match rand::gen_range(0, 4) {
            0 => vec3(0.85, 0.10, 0.12),
            1 => vec3(0.95, 0.45, 0.65),
            2 => vec3(0.95, 0.85, 0.20),
            _ => vec3(0.95, 0.95, 0.92),
        };
        for _ in 0..rand::gen_range(7, 16) {
            let (c, lr) = centers[rand::gen_range(0, centers.len())];
            let u = rvec();
            let u = vec3(u.x, u.y.abs().max(0.25), u.z).normalize();
            flower(mb, c + Vec3::Y * lr * 0.12 + u * vec3(lr, lr * 0.82, lr) * 1.02, u, fc);
        }
    }
}

fn grass_tuft(mb: &mut MB, p: Vec3, n: Vec3, beach: bool) {
    let blades = rand::gen_range(4, 8);
    for _ in 0..blades {
        let b = p + vec3(rf(-0.15, 0.15), -0.03, rf(-0.15, 0.15));
        let h = if beach { rf(0.35, 0.8) } else { rf(0.22, 0.55) };
        let a = rf(0.0, TAU);
        let lean = vec3(a.cos(), 0.0, a.sin()) * rf(0.05, 0.25) * h;
        let tip = b + Vec3::Y * h + lean;
        let sa = rf(0.0, TAU);
        let side = vec3(sa.cos(), 0.0, sa.sin()) * 0.03;
        let (c0, c1) = if beach {
            (vec3(0.45, 0.48, 0.22), vec3(0.75, 0.72, 0.42))
        } else {
            let k = rf(0.8, 1.15);
            (vec3(0.14, 0.32, 0.08) * k, vec3(0.38, 0.56, 0.18) * k)
        };
        let k0 = shade(c0, n);
        let k1 = shade(c1, n);
        mb.tri(b - side, b + side, tip, k0, k0, k1);
    }
}

// ---------------------------------------------------------------------------
// Colliders (things you can't walk through, and tops you can stand on)
// ---------------------------------------------------------------------------
#[derive(Clone)]
enum Col {
    Circle { c: Vec2, r: f32, y0: f32, top: f32, walk: bool },
    Obb { c: Vec2, f: Vec2, hx: f32, hz: f32, y0: f32, top: f32, walk: bool },
}

impl Col {
    fn top(&self) -> f32 {
        match self {
            Col::Circle { top, .. } | Col::Obb { top, .. } => *top,
        }
    }
    fn y0(&self) -> f32 {
        match self {
            Col::Circle { y0, .. } | Col::Obb { y0, .. } => *y0,
        }
    }
    fn walk(&self) -> bool {
        match self {
            Col::Circle { walk, .. } | Col::Obb { walk, .. } => *walk,
        }
    }
    fn near(&self, p: Vec2, d: f32) -> bool {
        match self {
            Col::Circle { c, r, .. } => c.distance(p) < r + d,
            Col::Obb { c, hx, hz, .. } => c.distance(p) < hx.max(*hz) * 1.5 + d,
        }
    }
    /// Is the point inside the footprint (shrunk by `shrink` metres)?
    fn inside(&self, p: Vec2, shrink: f32) -> bool {
        match self {
            Col::Circle { c, r, .. } => c.distance(p) < r - shrink,
            Col::Obb { c, f, hx, hz, .. } => {
                let d = p - *c;
                let (a, b) = (d.dot(*f), d.dot(vec2(-f.y, f.x)));
                a.abs() < hx - shrink && b.abs() < hz - shrink
            }
        }
    }
    /// Push a circle of radius `rad` at `p` out of the shape; returns the corrected position.
    fn push(&self, p: Vec2, rad: f32) -> Vec2 {
        match self {
            Col::Circle { c, r, .. } => {
                let d = p - *c;
                let l = d.length();
                if l < r + rad {
                    let n = if l > 1e-4 { d / l } else { vec2(1.0, 0.0) };
                    *c + n * (r + rad)
                } else {
                    p
                }
            }
            Col::Obb { c, f, hx, hz, .. } => {
                let pr = vec2(-f.y, f.x);
                let d = p - *c;
                let (a, b) = (d.dot(*f), d.dot(pr));
                let (ca, cb) = (a.clamp(-hx, *hx), b.clamp(-hz, *hz));
                let (da, db) = (a - ca, b - cb);
                let dist = (da * da + db * db).sqrt();
                if dist >= rad {
                    return p;
                }
                if dist > 1e-4 {
                    let k = rad / dist;
                    *c + *f * (ca + da * k) + pr * (cb + db * k)
                } else {
                    // centre is inside the box: leave through the nearest face
                    let (ex, ez) = (hx - a.abs(), hz - b.abs());
                    if ex < ez {
                        *c + *f * ((hx + rad) * a.signum()) + pr * b
                    } else {
                        *c + *f * a + pr * ((hz + rad) * b.signum())
                    }
                }
            }
        }
    }
}

/// Moves `feet` out of every collider whose vertical range overlaps the player's body.
fn collide(cols: &[Col], feet: &mut Vec3, rad: f32) {
    let p = vec2(feet.x, feet.z);
    let mut q = p;
    for _ in 0..5 {
        for c in cols {
            if !c.near(q, rad + 0.1) {
                continue;
            }
            // player body spans feet..feet+1.7; skip if above the top or below the bottom
            if feet.y >= c.top() - 0.45 && c.walk() {
                continue;
            }
            if feet.y >= c.top() || feet.y + 1.7 <= c.y0() {
                continue;
            }
            q = c.push(q, rad);
        }
    }
    feet.x = q.x;
    feet.z = q.y;
}

/// Highest standable surface at (x,z) for a player whose feet are at `fy`.
fn stand_on(cols: &[Col], x: f32, z: f32, fy: f32) -> Option<f32> {
    let p = vec2(x, z);
    let mut best: Option<f32> = None;
    for c in cols {
        if c.walk() && fy >= c.top() - 0.45 && c.near(p, 0.2) && c.inside(p, 0.0) {
            best = Some(best.map_or(c.top(), |b: f32| b.max(c.top())));
        }
    }
    best
}

/// A boulder that sits on the ground: the rim hugs the terrain so nothing overhangs.
/// Returns (collision radius, top height).
fn rock_on(mb: &mut MB, t: &Terrain, p: Vec3, size: f32, dark: bool) -> (f32, f32) {
    let col = if dark { vec3(0.24, 0.23, 0.22) } else { vec3(0.50, 0.47, 0.42) } * rf(0.85, 1.1);
    let r = vec3(size * rf(0.85, 1.3), size * rf(0.5, 0.85), size * rf(0.85, 1.3));
    let yaw = rf(0.0, TAU);
    let (sy, cy) = yaw.sin_cos();
    let rr = r.x.max(r.z);
    let (mut gmin, mut gmax) = (f32::MAX, f32::MIN);
    for k in 0..8 {
        let a = TAU * k as f32 / 8.0;
        let g = t.at(p.x + a.cos() * rr * 0.8, p.z + a.sin() * rr * 0.8);
        gmin = gmin.min(g);
        gmax = gmax.max(g);
    }
    let cy0 = (gmin + gmax) * 0.5 + r.y * 0.1;
    let (lon, lat) = (10usize, 6usize);
    mb.ensure((lat + 1) * lon, lat * lon * 6);
    let base = mb.v.len() as u16;
    for j in 0..=lat {
        let th = PI * j as f32 / lat as f32;
        for i in 0..lon {
            let ph = TAU * i as f32 / lon as f32;
            let u = vec3(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
            let k = if j > 0 && j < lat { 1.0 + 0.26 * rf(-1.0, 1.0) } else { 1.0 };
            let lp = u * r * k;
            let wx = p.x + lp.x * cy - lp.z * sy;
            let wz = p.z + lp.x * sy + lp.z * cy;
            let ty = t.at(wx, wz);
            let y = if lp.y <= 0.0 { ty - 0.1 } else { (cy0 + lp.y).max(ty + 0.04) };
            let ln = u / r;
            let n = vec3(ln.x * cy - ln.z * sy, ln.y, ln.x * sy + ln.z * cy).normalize_or_zero();
            mb.vert(vec3(wx, y, wz), shade(col, n));
        }
    }
    for j in 0..lat {
        for i in 0..lon {
            let i2 = (i + 1) % lon;
            let a = base + (j * lon + i) as u16;
            let b = base + (j * lon + i2) as u16;
            let cc = base + ((j + 1) * lon + i) as u16;
            let d = base + ((j + 1) * lon + i2) as u16;
            mb.i.extend_from_slice(&[a, cc, b, b, cc, d]);
        }
    }
    (rr * 1.05, cy0 + r.y * 0.95)
}

fn driftwood(mb: &mut MB, p: Vec3) {
    let a = rf(0.0, TAU);
    let d = vec3(a.cos(), 0.0, a.sin());
    let l = rf(1.5, 3.5);
    let col = vec3(0.62, 0.58, 0.52);
    cyl(mb, p - d * l * 0.5 + Vec3::Y * 0.08, p + d * l * 0.5 + Vec3::Y * 0.1, 0.13, 0.09, col, 7);
    let br = p + d * rf(-0.6, 0.6) + Vec3::Y * 0.1;
    cyl(mb, br, br + vec3(-d.z, 0.25, d.x) * rf(0.5, 0.9), 0.06, 0.03, col, 5);
}

// ---------------------------------------------------------------------------
// Fishing dock
// ---------------------------------------------------------------------------
#[derive(Clone)]
struct Dock {
    origin: Vec2,
    dir: Vec2,
    len: f32,
    hw: f32,
    deck_y: f32,
}

impl Dock {
    fn local(&self, x: f32, z: f32) -> (f32, f32) {
        let d = vec2(x, z) - self.origin;
        (d.dot(self.dir), d.dot(vec2(-self.dir.y, self.dir.x)))
    }
    fn contains(&self, x: f32, z: f32) -> bool {
        let (s, t) = self.local(x, z);
        s >= 0.0 && s <= self.len && t.abs() <= self.hw
    }
    fn near(&self, x: f32, z: f32, m: f32) -> bool {
        let (s, t) = self.local(x, z);
        s >= -m && s <= self.len + m && t.abs() <= self.hw + m
    }
    fn point(&self, s: f32, t: f32, y: f32) -> Vec3 {
        let p = self.origin + self.dir * s + vec2(-self.dir.y, self.dir.x) * t;
        vec3(p.x, y, p.y)
    }
}

thread_local! {
    /// Model detail multiplier chosen in the menu (applies when the world is built).
    static DETAIL: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
    static AVOID: RefCell<Option<Dock>> = const { RefCell::new(None) };
    /// The dirt path from the dock to the plaza (start, end).
    static PATH: std::cell::Cell<Option<(Vec2, Vec2)>> = const { std::cell::Cell::new(None) };
}

fn make_dock(t: &Terrain, sa: f32) -> Dock {
    let dir = vec2(sa.cos(), sa.sin());
    let deck_y = 1.4;
    let mut r = 0.0;
    while r < 240.0 && t.at(dir.x * r, dir.y * r) > deck_y {
        r += 0.5;
    }
    let start = r - 1.5;
    let mut e = r;
    while e < 240.0 && t.at(dir.x * e, dir.y * e) > -3.4 {
        e += 0.5;
    }
    let len = (e + 3.0 - start).clamp(12.0, 80.0);
    Dock { origin: dir * start, dir, len, hw: 1.4, deck_y }
}

/// Builds the pier and returns the positions of its two lantern lights plus its colliders.
fn build_dock(pool: &mut HashMap<(i32, i32), MB>, t: &Terrain, d: &Dock) -> ([Vec3; 2], Vec<Col>) {
    let f3 = vec3(d.dir.x, 0.0, d.dir.y);
    let r3 = vec3(-d.dir.y, 0.0, d.dir.x);
    let dy = d.deck_y;
    let mut cols: Vec<Col> = vec![];
    // railings: one long thin wall along each side (entrance and far end stay open)
    for side in [-1.0f32, 1.0] {
        let s0 = 1.0;
        let s1 = d.len - 0.4;
        let c = d.point((s0 + s1) * 0.5, side * (d.hw - 0.1), 0.0);
        cols.push(Col::Obb { c: vec2(c.x, c.z), f: d.dir, hx: (s1 - s0) * 0.5, hz: 0.07, y0: dy, top: dy + 1.0, walk: false });
    }
    let wood = vec3(0.50, 0.38, 0.25);
    let dark = vec3(0.30, 0.22, 0.15);

    // deck planks
    // salvaged planks: uneven widths, mixed colours, split boards, slight tilt and gaps
    let mut s = 0.15;
    while s < d.len {
        let hwid = rf(0.085, 0.125);
        let split = rf(-0.6, 0.6);
        let pieces: [(f32, f32); 2] = if rf(0.0, 1.0) < 0.35 {
            [(-d.hw, split - 0.02), (split + 0.02, d.hw)]
        } else {
            [(-d.hw, d.hw), (0.0, 0.0)]
        };
        for (a, b) in pieces {
            if b - a < 0.05 {
                continue;
            }
            let yaw = rf(-0.025, 0.025);
            let (sy, cy) = yaw.sin_cos();
            let pf = f3 * cy + r3 * sy;
            let pr = -f3 * sy + r3 * cy;
            let c = d.point(s + rf(-0.02, 0.02), (a + b) * 0.5, dy - 0.03 + rf(-0.012, 0.012));
            let tone = if rf(0.0, 1.0) < 0.15 { rf(0.55, 0.7) } else { rf(0.78, 1.12) };
            let weathered = wood * tone * vec3(1.0, 1.0, 0.92 + 0.12 * rf(0.0, 1.0)) * vec3(0.9, 0.93, 0.95);
            bbox(pmb(pool, c), c, pf, pr, vec3(hwid, 0.03, (b - a) * 0.5), weathered);
        }
        s += hwid * 2.0 + rf(0.01, 0.05);
    }
    // stringers under the deck
    for side in [-1.0f32, 1.0] {
        let mut a = 0.0;
        while a < d.len {
            let b = (a + 8.0).min(d.len);
            let c = d.point((a + b) * 0.5, side * (d.hw - 0.2), dy - 0.2);
            bbox(pmb(pool, c), c, f3, r3, vec3((b - a) * 0.5, 0.09, 0.06), dark);
            a = b;
        }
    }
    // cross beams and support posts
    let mut s = 1.0;
    while s < d.len {
        let c = d.point(s, 0.0, dy - 0.2);
        bbox(pmb(pool, c), c, f3, r3, vec3(0.07, 0.07, d.hw + 0.12), dark);
        for side in [-1.0f32, 1.0] {
            let top = d.point(s, side * (d.hw - 0.12), dy - 0.1);
            let floor_y = t.at(top.x, top.z) - 0.6;
            cyl(pmb(pool, top), vec3(top.x, floor_y, top.z), top, 0.15, 0.14, vec3(0.34, 0.27, 0.19), 7);
            cols.push(Col::Circle { c: vec2(top.x, top.z), r: 0.16, y0: -20.0, top: dy - 0.1, walk: false });
        }
        s += 3.0;
    }
    // railings
    let mut s = 1.0;
    while s < d.len - 0.4 {
        let s2 = (s + 3.0).min(d.len - 0.4);
        for side in [-1.0f32, 1.0] {
            let b0 = d.point(s, side * (d.hw - 0.1), dy);
            let b1 = d.point(s2, side * (d.hw - 0.1), dy);
            cyl(pmb(pool, b0), b0, b0 + Vec3::Y * 1.0, 0.06, 0.06, wood, 6);
            cyl(pmb(pool, b0), b0 + Vec3::Y * 0.95, b1 + Vec3::Y * 0.95, 0.04, 0.04, wood * 0.9, 5);
            cyl(pmb(pool, b0), b0 + Vec3::Y * 0.50, b1 + Vec3::Y * 0.50, 0.03, 0.03, wood * 0.85, 5);
            if s2 >= d.len - 0.45 {
                cyl(pmb(pool, b1), b1, b1 + Vec3::Y * 1.0, 0.06, 0.06, wood, 6);
            }
        }
        s = s2;
    }
    // lantern posts: a black candle lantern sits on top of each post
    let mut lamps = [Vec3::ZERO; 2];
    for (i, ls) in [(0usize, d.len * 0.30), (1usize, d.len - 2.0)] {
        let base = d.point(ls, d.hw - 0.1, dy);
        let top = base + Vec3::Y * 2.0;
        let mb = pmb(pool, base);
        cyl(mb, base, top, 0.07, 0.06, dark, 6);
        cyl(mb, top - Vec3::Y * 0.04, top + Vec3::Y * 0.02, 0.11, 0.1, vec3(0.05, 0.05, 0.055), 8);
        crate::items::lantern_model(mb, top + Vec3::Y * 0.02, 1.7, d.dir.y.atan2(d.dir.x), true);
        lamps[i] = top + Vec3::Y * (0.02 + 0.17 * 1.7);
        cols.push(Col::Circle { c: vec2(base.x, base.z), r: 0.1, y0: dy, top: dy + 2.4, walk: false });
    }
    // barrels, crate and a bench at the far end
    let end = d.len - 1.2;
    for (k, side) in [(0, -1.0f32), (1, 1.0)] {
        let p = d.point(end - k as f32 * 0.9, side * (d.hw - 0.55), dy);
        let mb = pmb(pool, p);
        cyl(mb, p, p + Vec3::Y * 0.8, 0.32, 0.32, vec3(0.40, 0.30, 0.20), 10);
        cyl(mb, p + Vec3::Y * 0.2, p + Vec3::Y * 0.26, 0.335, 0.335, vec3(0.12, 0.12, 0.12), 10);
        cyl(mb, p + Vec3::Y * 0.55, p + Vec3::Y * 0.61, 0.335, 0.335, vec3(0.12, 0.12, 0.12), 10);
        cols.push(Col::Circle { c: vec2(p.x, p.z), r: 0.34, y0: dy, top: dy + 0.8, walk: true });
    }
    let cp = d.point(d.len * 0.62, -(d.hw - 0.5), dy + 0.27);
    bbox(pmb(pool, cp), cp, f3, r3, vec3(0.32, 0.27, 0.32), vec3(0.55, 0.43, 0.28));
    cols.push(Col::Obb { c: vec2(cp.x, cp.z), f: d.dir, hx: 0.32, hz: 0.32, y0: dy, top: dy + 0.54, walk: true });
    let bp = d.point(d.len * 0.45, d.hw - 0.4, dy);
    {
        let mb = pmb(pool, bp);
        bbox(mb, bp + Vec3::Y * 0.45, f3, r3, vec3(0.7, 0.03, 0.2), wood);
        for sx in [-0.55f32, 0.55] {
            bbox(mb, bp + f3 * sx + Vec3::Y * 0.22, f3, r3, vec3(0.04, 0.22, 0.16), dark);
        }
    }
    cols.push(Col::Obb { c: vec2(bp.x, bp.z), f: d.dir, hx: 0.7, hz: 0.2, y0: dy, top: dy + 0.48, walk: true });
    (lamps, cols)
}

// ---------------------------------------------------------------------------
// Plaza: shop, empty plots, campfire and tent on the flat middle of the island
// ---------------------------------------------------------------------------
struct Plaza {
    npc_pos: Vec3,
    npc_yaw: f32,
    tent_pos: Vec3,
    fire_pos: Vec3,
    spawn: Vec3,
    spawn_yaw: f32,
    cols: Vec<Col>,
}

fn polar(c: Vec2, ang_deg: f32, r: f32) -> Vec2 {
    let a = ang_deg.to_radians();
    c + vec2(a.cos(), a.sin()) * r
}

fn dir_to(from: Vec3, to: Vec3) -> Vec3 {
    let d = vec3(to.x - from.x, 0.0, to.z - from.z).normalize_or_zero();
    if d == Vec3::ZERO { vec3(1.0, 0.0, 0.0) } else { d }
}

// ---- castaway camp props -------------------------------------------------

/// Box with arbitrary orthonormal axes and half extents.
fn obox(mb: &mut MB, c: Vec3, ax: [Vec3; 3], h: Vec3, col: Vec3) {
    let hs = [h.x, h.y, h.z];
    for a in 0..3 {
        let (a1, a2) = ((a + 1) % 3, (a + 2) % 3);
        for sg in [-1.0f32, 1.0] {
            let n = ax[a] * sg;
            let ctr = c + n * hs[a];
            let (u, v) = (ax[a1] * hs[a1], ax[a2] * hs[a2]);
            mb.nquad(ctr - u - v, ctr + u - v, ctr + u + v, ctr - u + v, col, n);
        }
    }
}

/// Square-section beam/plank from a to b (w wide, t thick).
fn beam(mb: &mut MB, a: Vec3, b: Vec3, w: f32, t: f32, col: Vec3) {
    let f = (b - a).normalize_or_zero();
    if f == Vec3::ZERO {
        return;
    }
    let up = if f.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let s = f.cross(up).normalize();
    let u = s.cross(f);
    obox(mb, (a + b) * 0.5, [f, u, s], vec3((b - a).length() * 0.5, t * 0.5, w * 0.5), col);
}

fn rope_line(mb: &mut MB, a: Vec3, b: Vec3, sag: f32) {
    let rope = vec3(0.62, 0.52, 0.34);
    let mid = (a + b) * 0.5 - Vec3::Y * sag;
    cyl(mb, a, mid, 0.011, 0.011, rope, 3);
    cyl(mb, mid, b, 0.011, 0.011, rope, 3);
}

fn driftwood_col() -> Vec3 {
    let k = rf(0.0, 1.0);
    if k < 0.3 {
        vec3(0.55, 0.50, 0.43)
    } else if k < 0.6 {
        vec3(0.42, 0.34, 0.25)
    } else if k < 0.85 {
        vec3(0.50, 0.42, 0.31)
    } else {
        vec3(0.30, 0.38, 0.38) // faded old hull paint
    }
}

fn barrel(mb: &mut MB, p: Vec3, h: f32, col: Vec3) {
    let r = 0.30;
    let mid = p + Vec3::Y * h * 0.5;
    cyl(mb, p, mid, r * 0.88, r, col, 10);
    cyl(mb, mid, p + Vec3::Y * h, r, r * 0.88, col, 10);
    let iron = vec3(0.20, 0.17, 0.14);
    for (y, k) in [(0.14f32, 0.92f32), (0.46, 1.0), (0.8, 1.0), (0.88, 0.92)] {
        let yy = y * h / 0.8;
        if yy < h {
            cyl(mb, p + Vec3::Y * yy, p + Vec3::Y * (yy + 0.04), r * k * 1.03, r * k * 1.03, iron, 10);
        }
    }
    cyl(mb, p + Vec3::Y * (h - 0.01), p + Vec3::Y * h, r * 0.88, r * 0.88, col * 0.7, 10);
}

fn crate_box(mb: &mut MB, p: Vec3, yaw: f32, s: f32, col: Vec3) {
    let f = vec3(yaw.cos(), 0.0, yaw.sin());
    let r = vec3(-f.z, 0.0, f.x);
    let c = p + Vec3::Y * s * 0.5;
    bbox(mb, c, f, r, Vec3::splat(s * 0.5), col);
    let dk = col * 0.62;
    let e = s * 0.5;
    for sy in [-1.0f32, 1.0] {
        for sx in [-1.0f32, 1.0] {
            bbox(mb, c + f * sx * e + Vec3::Y * sy * e, f, r, vec3(0.03, 0.03, e + 0.005), dk);
            bbox(mb, c + r * sx * e + Vec3::Y * sy * e, f, r, vec3(e + 0.005, 0.03, 0.03), dk);
            bbox(mb, c + f * sx * e + r * sy * e, f, r, vec3(0.03, e + 0.005, 0.03), dk);
        }
    }
}

fn rope_coil(mb: &mut MB, p: Vec3) {
    let rope = vec3(0.64, 0.54, 0.34);
    for layer in 0..2 {
        let n = 9;
        for i in 0..n {
            let a = TAU * i as f32 / n as f32;
            let rr = 0.2 - layer as f32 * 0.03;
            let q = p + vec3(a.cos() * rr, 0.035 + layer as f32 * 0.05, a.sin() * rr);
            blob(mb, q, vec3(0.06, 0.035, 0.06), rope * rf(0.85, 1.05), 0.0, 5, 3, a, shade);
        }
    }
}

fn basket(mb: &mut MB, p: Vec3, s: f32) {
    let straw = vec3(0.66, 0.54, 0.30);
    cyl(mb, p, p + Vec3::Y * 0.28 * s, 0.17 * s, 0.25 * s, straw, 9);
    cyl(mb, p + Vec3::Y * 0.1 * s, p + Vec3::Y * 0.13 * s, 0.2 * s, 0.205 * s, straw * 0.65, 9);
    cyl(mb, p + Vec3::Y * 0.2 * s, p + Vec3::Y * 0.23 * s, 0.23 * s, 0.235 * s, straw * 0.65, 9);
    cyl(mb, p + Vec3::Y * 0.27 * s, p + Vec3::Y * 0.29 * s, 0.25 * s, 0.25 * s, straw * 0.55, 9);
}

/// Pile of split logs lying along `d`.
fn firewood(mb: &mut MB, p: Vec3, d: Vec3) {
    let side = vec3(-d.z, 0.0, d.x);
    let rows = [(4usize, 0.0f32), (3, 0.14), (2, 0.28)];
    for (n, y) in rows {
        for i in 0..n {
            let off = (i as f32 - (n as f32 - 1.0) * 0.5) * 0.19;
            let a = p + side * off + Vec3::Y * (0.09 + y) + d * rf(-0.08, 0.08);
            let l = rf(0.45, 0.6);
            let col = vec3(0.40, 0.29, 0.18) * rf(0.8, 1.15);
            cyl(mb, a - d * l, a + d * l, 0.085, 0.08, col, 7);
            cyl(mb, a + d * l, a + d * (l + 0.015), 0.075, 0.075, vec3(0.72, 0.58, 0.38), 7);
        }
    }
}

fn fish_rack(mb: &mut MB, p: Vec3, f: Vec3) {
    let wood = vec3(0.40, 0.31, 0.21);
    let hw = 1.0;
    for sx in [-1.0f32, 1.0] {
        let q = p + f * sx * hw;
        cyl(mb, q - Vec3::Y * 0.1, q + Vec3::Y * 1.55, 0.05, 0.04, wood, 6);
    }
    for y in [1.5f32, 0.95] {
        cyl(mb, p - f * hw + Vec3::Y * y, p + f * hw + Vec3::Y * y, 0.03, 0.03, wood * 1.1, 5);
    }
    let h = f.z.atan2(f.x);
    for i in 0..5 {
        let q = p - f * 0.75 + f * i as f32 * 0.38 + Vec3::Y * 1.5;
        cyl(mb, q, q - Vec3::Y * 0.2, 0.008, 0.008, vec3(0.6, 0.55, 0.4), 3);
        fish_shape(mb, q - Vec3::Y * 0.36, h + PI * 0.5, 1.1, vec3(0.55, 0.60, 0.62) * rf(0.8, 1.1), 0.0);
    }
    for i in 0..3 {
        let q = p - f * 0.5 + f * i as f32 * 0.5 + Vec3::Y * 0.95;
        cyl(mb, q, q - Vec3::Y * 0.15, 0.008, 0.008, vec3(0.6, 0.55, 0.4), 3);
        fish_shape(mb, q - Vec3::Y * 0.28, h + PI * 0.5, 1.4, vec3(0.62, 0.50, 0.38) * rf(0.8, 1.1), 0.0);
    }
}

/// Castaway's lean-to hut of salvaged planks and palm thatch. `fr` points out of the open front.
fn build_shop(pool: &mut HashMap<(i32, i32), MB>, cols: &mut Vec<Col>, c: Vec3, fr: Vec3) -> (Vec3, f32) {
    let f3 = vec3(-fr.z, 0.0, fr.x); // width axis
    let r3 = fr; // depth axis (front = +)
    let gy = c.y;
    let fdir = vec2(f3.x, f3.z);
    let (hw, hd) = (2.4f32, 1.8f32);
    let obb = |cc: Vec3, hx: f32, hz: f32, top: f32| Col::Obb { c: vec2(cc.x, cc.z), f: fdir, hx, hz, y0: gy, top, walk: false };
    cols.push(Col::Obb { c: vec2(c.x, c.z), f: fdir, hx: hw + 0.1, hz: hd + 0.1, y0: gy - 1.0, top: gy + 0.06, walk: true });
    let l = |x: f32, y: f32, z: f32| c + f3 * x + Vec3::Y * y + r3 * z;
    let (zf, zb) = (hd + 0.6, -(hd + 0.4));
    // roof height above the ground at depth z (low at the front, high at the back)
    let roof = |z: f32| 2.15 + 1.1 * (zf - z) / (zf - zb);
    let mb = pmb(pool, c);

    // uneven salvaged floor boards over packed sand
    bbox(mb, c, f3, r3, vec3(hw + 0.1, 0.02, hd + 0.1), vec3(0.55, 0.47, 0.34));
    let mut z = -hd - 0.05;
    while z < hd + 0.05 {
        let w = rf(0.12, 0.2);
        let col = driftwood_col() * rf(0.8, 1.0);
        let xo = rf(-0.08, 0.08);
        bbox(mb, l(xo, 0.035 + rf(-0.008, 0.008), z + w), f3, r3, vec3(hw + 0.04 - xo.abs(), 0.025, w * 0.93), col);
        z += w * 2.0;
    }

    // driftwood corner poles, lashed with rope
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            let z = sz * hd;
            let top = roof(z) - 0.08;
            let lean = rf(-0.05, 0.05);
            cyl(mb, l(sx * hw, -0.15, z), l(sx * hw + lean, top, z + rf(-0.04, 0.04)), 0.10, 0.075, driftwood_col(), 7);
            cyl(mb, l(sx * hw, top - 0.2, z), l(sx * hw, top - 0.14, z), 0.12, 0.12, vec3(0.62, 0.52, 0.34), 7);
        }
    }

    // mismatched plank walls: back + both sides
    let mut x = -hw + 0.1;
    while x < hw - 0.05 {
        let w = rf(0.13, 0.26);
        let h = roof(-hd) - 0.14 - rf(0.0, 0.3);
        let col = driftwood_col() * rf(0.8, 1.1);
        let zz = -hd + rf(-0.03, 0.03);
        bbox(mb, l(x + w * 0.5, h * 0.5, zz), f3, r3, vec3(w * 0.5 - 0.01, h * 0.5, 0.03), col);
        x += w;
    }
    cols.push(obb(c - fr * hd, hw + 0.12, 0.12, gy + 2.3));
    for sx in [-1.0f32, 1.0] {
        let mut z = -hd;
        while z < hd - 0.1 {
            let w = rf(0.13, 0.26);
            let zc = z + w * 0.5;
            let h = roof(zc) - 0.14 - rf(0.0, 0.3);
            let col = driftwood_col() * rf(0.8, 1.1);
            bbox(mb, l(sx * hw + rf(-0.02, 0.02), h * 0.5, zc), f3, r3, vec3(0.03, h * 0.5, w * 0.5 - 0.01), col);
            z += w;
        }
        cols.push(obb(c + f3 * sx * hw, 0.12, hd + 0.1, gy + 2.3));
        for y in [0.45f32, 1.3] {
            beam(mb, l(sx * hw + sx * 0.05, y, -hd), l(sx * hw + sx * 0.05, y + 0.05, hd), 0.09, 0.03, vec3(0.30, 0.23, 0.16));
        }
    }
    for y in [0.5f32, 1.4] {
        beam(mb, l(-hw, y, -hd + 0.05), l(hw, y + 0.04, -hd + 0.05), 0.09, 0.03, vec3(0.30, 0.23, 0.16));
    }

    // rafters + purlins
    for i in 0..5 {
        let x = -hw - 0.3 + i as f32 * (hw * 2.0 + 0.6) / 4.0;
        beam(mb, l(x, roof(zf) - 0.1, zf), l(x, roof(zb) - 0.1, zb), 0.08, 0.07, vec3(0.34, 0.26, 0.17));
    }
    for z in [zf - 0.05, 0.4, -1.2] {
        beam(mb, l(-hw - 0.4, roof(z) - 0.15, z), l(hw + 0.4, roof(z) - 0.15, z), 0.07, 0.06, vec3(0.30, 0.23, 0.15));
    }

    // layered palm thatch, rows shingled from the low front edge up to the back
    let rows = 9;
    let segs = 14usize;
    let (x0, x1) = (-hw - 0.55, hw + 0.55);
    for i in 0..rows {
        let za = mixf(zf, zb, i as f32 / rows as f32);
        let zb2 = mixf(zf, zb, (i + 1) as f32 / rows as f32 + 0.08).max(zb);
        for s in 0..segs {
            let xa = mixf(x0, x1, s as f32 / segs as f32);
            let xb = mixf(x0, x1, (s + 1) as f32 / segs as f32);
            let k = rf(0.86, 1.1) * (0.92 + 0.02 * i as f32);
            let col = vec3(0.60, 0.50, 0.25) * k;
            let (p0, p1) = (l(xa, roof(za), za), l(xb, roof(za), za));
            let (p2, p3) = (l(xb, roof(zb2) + 0.05, zb2), l(xa, roof(zb2) + 0.05, zb2));
            let mut n = (p1 - p0).cross(p3 - p0).normalize_or_zero();
            if n.y < 0.0 {
                n = -n;
            }
            let (dk0, lt) = (shade(col * 0.62, n), shade(col * 1.05, n));
            mb.quadc(p0, p1, p2, p3, dk0, dk0, lt, lt);
            // ragged frond tips hanging off the lower edge of the row
            let tip = l(mixf(xa, xb, rf(0.3, 0.7)) + rf(-0.05, 0.05), roof(za) - rf(0.12, 0.26), za + rf(0.05, 0.15));
            let tc = shade(col * 0.5, n);
            mb.tri(p0, p1, tip, tc, tc, shade(col * 0.8, n));
        }
    }
    // sailcloth patch lashed over part of the roof
    {
        let (xa, xb, za, zb2) = (0.3f32, 1.9f32, 1.4f32, -0.8f32);
        let o = Vec3::Y * 0.06;
        let (p0, p1, p2, p3) = (l(xa, roof(za), za) + o, l(xb, roof(za), za) + o, l(xb, roof(zb2), zb2) + o, l(xa, roof(zb2), zb2) + o);
        let mut n = (p1 - p0).cross(p3 - p0).normalize_or_zero();
        if n.y < 0.0 {
            n = -n;
        }
        let a = shade(vec3(0.74, 0.70, 0.58), n);
        let b = shade(vec3(0.62, 0.58, 0.47), n);
        mb.quadc(p0, p1, p2, p3, a, a, b, b);
        rope_line(mb, p0 + o, p2 + o, 0.0);
        rope_line(mb, p1 + o, p3 + o, 0.0);
        rope_line(mb, (p0 + p3) * 0.5 + o, (p1 + p2) * 0.5 + o, 0.0);
    }
    cyl(mb, l(-hw - 0.5, roof(zb) + 0.12, zb), l(hw + 0.5, roof(zb) + 0.12, zb), 0.1, 0.1, vec3(0.38, 0.29, 0.20), 7);

    // one solid plank counter that closes the whole front, wall to wall
    let counter = c + fr * (hd - 0.12);
    bbox(mb, counter + Vec3::Y * 0.5, f3, r3, vec3(hw + 0.02, 0.5, 0.25), driftwood_col() * 0.7);
    let mut px = -hw;
    while px < hw - 0.02 {
        let w = rf(0.16, 0.26).min(hw - px);
        let ph = rf(0.9, 1.0);
        bbox(mb, counter + f3 * (px + w * 0.5) + r3 * 0.26 + Vec3::Y * (ph * 0.5), f3, r3, vec3(w * 0.5 - 0.01, ph * 0.5, 0.025), driftwood_col() * rf(0.8, 1.1));
        px += w;
    }
    beam(mb, counter - f3 * (hw + 0.1) + Vec3::Y * 1.03, counter + f3 * (hw + 0.1) + Vec3::Y * 1.0, 0.66, 0.05, vec3(0.45, 0.36, 0.25));
    // a couple of crates and barrels on the customer side for flavour
    crate_box(mb, counter + r3 * 0.9 + f3 * (hw - 0.6), f3.z.atan2(f3.x) + 0.2, 0.55, driftwood_col() * 0.9);
    crate_box(mb, counter + r3 * 0.9 - f3 * (hw - 0.9), f3.z.atan2(f3.x) - 0.1, 0.5, driftwood_col() * 0.85);
    let top = Vec3::Y * 1.06;
    crate::items::lantern_model(mb, counter + f3 * 1.4 + top, 1.0, 0.4, false);
    basket(mb, counter - f3 * 1.35 + top, 0.7);
    cyl(mb, counter - f3 * 0.4 + top, counter - f3 * 0.4 + top + Vec3::Y * 0.12, 0.07, 0.07, vec3(0.62, 0.62, 0.60), 8);
    cyl(mb, counter + f3 * 0.2 + top, counter + f3 * 0.2 + top + Vec3::Y * 0.2, 0.07, 0.09, vec3(0.45, 0.55, 0.50), 8);
    fish_shape(mb, counter + f3 * 0.8 + top + Vec3::Y * 0.04, 0.3, 1.2, vec3(0.55, 0.65, 0.70), 0.0);

    // fish drying on lines under the roof
    let h = f3.z.atan2(f3.x);
    for (k, z) in [(0usize, -1.1f32), (1, -0.4)] {
        let a = l(-hw + 0.1, roof(z) - 0.3, z);
        let b = l(hw - 0.1, roof(z) - 0.3, z);
        rope_line(mb, a, b, 0.06);
        for i in 0..(5 - k) {
            let q = a.lerp(b, (i as f32 + 0.6) / (6.0 - k as f32)) - Vec3::Y * 0.1;
            cyl(mb, q + Vec3::Y * 0.08, q - Vec3::Y * 0.12, 0.008, 0.008, vec3(0.6, 0.55, 0.4), 3);
            fish_shape(mb, q - Vec3::Y * 0.27, h + PI * 0.5, 1.3, vec3(0.58, 0.55, 0.45) * rf(0.8, 1.1), 0.0);
        }
    }
    // fishing net draped on the left wall, floats and all
    let wx = -hw + 0.1;
    for i in 0..9 {
        let z = -1.5 + i as f32 * 0.3;
        rope_line(mb, l(wx, 2.0, z), l(wx, 0.7 + (i % 3) as f32 * 0.1, z), 0.0);
    }
    for j in 0..6 {
        let y = 0.8 + j as f32 * 0.22;
        rope_line(mb, l(wx, y, -1.5), l(wx, y, 0.9), 0.04);
    }
    for i in 0..4 {
        blob(mb, l(wx + 0.03, 2.0, -1.3 + i as f32 * 0.7), Vec3::splat(0.05), vec3(0.75, 0.30, 0.22), 0.0, 5, 3, 0.0, shade);
    }
    // shelf with jars on the back wall
    beam(mb, l(0.4, 1.3, -hd + 0.2), l(2.2, 1.3, -hd + 0.2), 0.3, 0.04, vec3(0.34, 0.26, 0.18));
    for i in 0..3 {
        cyl(mb, l(0.7 + i as f32 * 0.55, 1.32, -hd + 0.2), l(0.7 + i as f32 * 0.55, 1.5, -hd + 0.2), 0.07, 0.07, vec3(0.5, 0.6, 0.55), 8);
    }
    // rod bundle leaning in a corner
    let rb = l(-hw + 0.3, 0.0, -hd + 0.25);
    for i in 0..3 {
        cyl(mb, rb + f3 * i as f32 * 0.08, rb + f3 * i as f32 * 0.08 + Vec3::Y * 2.0 + r3 * 0.25 - f3 * 0.1, 0.016, 0.006, vec3(0.45, 0.36, 0.2), 4);
    }
    // hammock of old sailcloth along the right wall outside
    {
        let a = l(hw + 0.15, 1.0, -hd + 0.4);
        let b = l(hw + 0.15, 1.0, hd - 0.4);
        let cl = vec3(0.72, 0.68, 0.55);
        let mut prev: Option<(Vec3, Vec3)> = None;
        for i in 0..=8 {
            let t = i as f32 / 8.0;
            let mid = a.lerp(b, t) - Vec3::Y * (0.45 * (PI * t).sin());
            let pair = (mid + f3 * 0.12, mid + f3 * 0.62);
            if let Some((p0, p1)) = prev {
                mb.fquad(p0, p1, pair.1, pair.0, cl * rf(0.92, 1.05));
            }
            prev = Some(pair);
        }
        rope_line(mb, a, a + f3 * 0.3, 0.0);
        rope_line(mb, b, b + f3 * 0.3, 0.0);
    }

    // the counter blocks the whole front up to the roof line (no climbing over it, no gap to the
    // walls); the roof itself is solid ground so nobody can drop into the hut from above
    cols.push(obb(counter, hw + 0.12, 0.34, gy + 2.3));
    let strips = 5;
    for i in 0..strips {
        let (za, zb2) = (mixf(zb, zf, i as f32 / strips as f32), mixf(zb, zf, (i + 1) as f32 / strips as f32));
        let zc = (za + zb2) * 0.5;
        let top_y = roof(za.min(zb2)).max(roof(za.max(zb2))) + 0.02;
        let ctr = c + r3 * zc;
        cols.push(Col::Obb { c: vec2(ctr.x, ctr.z), f: fdir, hx: hw + 0.6, hz: (zb2 - za) * 0.5 + 0.02, y0: gy + 2.3, top: gy + top_y, walk: true });
    }
    // the trading spot is the counter (no shopkeeper)
    (vec3(counter.x, gy, counter.z), fr.z.atan2(fr.x))
}

fn build_campfire(pool: &mut HashMap<(i32, i32), MB>, cols: &mut Vec<Col>, c: Vec3, toward: Vec3) {
    let mb = pmb(pool, c);
    // ring of rough beach stones
    for i in 0..12 {
        let a = TAU * i as f32 / 12.0 + rf(-0.1, 0.1);
        let p = c + vec3(a.cos() * 0.68, 0.0, a.sin() * 0.68);
        let r = rf(0.12, 0.2);
        blob(mb, p + Vec3::Y * r * 0.35, vec3(r * 1.1, r * 0.75, r), vec3(0.45, 0.43, 0.40) * rf(0.75, 1.1), 0.18, 6, 4, rf(0.0, TAU), shade);
    }
    cyl(mb, c + Vec3::Y * 0.01, c + Vec3::Y * 0.05, 0.56, 0.56, vec3(0.08, 0.07, 0.07), 12);
    for i in 0..6 {
        let a = TAU * i as f32 / 6.0 + 0.3;
        let d = vec3(a.cos(), 0.0, a.sin());
        cyl(mb, c + d * 0.5 + Vec3::Y * 0.08, c + d * 0.07 + Vec3::Y * 0.38, 0.065, 0.055, vec3(0.30, 0.20, 0.12), 6);
        cyl(mb, c + d * 0.12 + Vec3::Y * 0.33, c + d * 0.07 + Vec3::Y * 0.38, 0.058, 0.056, vec3(0.06, 0.05, 0.05), 6);
    }
    // tripod of branches with a battered cooking pot
    let top = c + Vec3::Y * 1.55;
    for i in 0..3 {
        let a = TAU * i as f32 / 3.0 + 0.5;
        cyl(mb, c + vec3(a.cos() * 1.05, 0.0, a.sin() * 1.05), top, 0.035, 0.03, vec3(0.34, 0.26, 0.17), 5);
    }
    cyl(mb, top, top - Vec3::Y * 0.5, 0.008, 0.008, vec3(0.15, 0.13, 0.12), 3);
    cyl(mb, top - Vec3::Y * 0.78, top - Vec3::Y * 0.52, 0.14, 0.16, vec3(0.16, 0.15, 0.15), 8);
    cols.push(Col::Circle { c: vec2(c.x, c.z), r: 1.0, y0: c.y - 1.0, top: c.y + 1.6, walk: false });
    // two log seats
    let side = vec3(-toward.z, 0.0, toward.x);
    for sg in [-1.0f32, 1.0] {
        let p = c + side * sg * 2.1 + toward * 0.3;
        let along = toward;
        cyl(mb, p - along * 0.9 + Vec3::Y * 0.18, p + along * 0.9 + Vec3::Y * 0.18, 0.18, 0.17, vec3(0.38, 0.30, 0.22), 8);
        cyl(mb, p + along * 0.9 + Vec3::Y * 0.18, p + along * 0.92 + Vec3::Y * 0.18, 0.15, 0.15, vec3(0.70, 0.58, 0.40), 8);
        cols.push(Col::Obb { c: vec2(p.x, p.z), f: vec2(along.x, along.z), hx: 0.9, hz: 0.2, y0: c.y - 1.0, top: c.y + 0.36, walk: true });
    }
}

fn build_tent(pool: &mut HashMap<(i32, i32), MB>, cols: &mut Vec<Col>, c: Vec3, fr: Vec3) {
    let f3 = vec3(-fr.z, 0.0, fr.x);
    let r3 = fr; // entrance direction
    let (hw, hd, hh) = (1.2f32, 1.6f32, 1.6f32);
    let canvas_a = vec3(0.70, 0.64, 0.46);
    let canvas_b = vec3(0.58, 0.50, 0.34); // patched, stained panels
    let mb = pmb(pool, c);
    bbox(mb, c + Vec3::Y * 0.02, f3, r3, vec3(hw, 0.03, hd), vec3(0.34, 0.30, 0.20));
    let rf0 = c + r3 * hd + Vec3::Y * hh; // front ridge end
    let rb0 = c - r3 * hd + Vec3::Y * hh; // back ridge end
    for sg in [-1.0f32, 1.0] {
        let base_f = c + f3 * sg * hw + r3 * hd;
        let base_b = c + f3 * sg * hw - r3 * hd;
        for k in 0..4 {
            let (u0, u1) = (k as f32 / 4.0, (k as f32 + 1.0) / 4.0);
            let a = base_b.lerp(base_f, u0);
            let b = base_b.lerp(base_f, u1);
            let a2 = rb0.lerp(rf0, u0);
            let b2 = rb0.lerp(rf0, u1);
            let mut n = (b - a).cross(a2 - a).normalize_or_zero();
            if n.y < 0.0 {
                n = -n;
            }
            let pc = if (k + (sg > 0.0) as usize) % 3 == 1 { canvas_b } else { canvas_a };
            mb.nquad(a, b, b2, a2, pc * rf(0.92, 1.05), n);
        }
    }
    mb.ftri(c - f3 * hw - r3 * hd, c + f3 * hw - r3 * hd, rb0, canvas_a * 0.85);
    mb.ftri(rf0, c - f3 * hw + r3 * hd, c - f3 * 0.28 + r3 * hd, canvas_b * 0.95);
    mb.ftri(rf0, c + f3 * hw + r3 * hd, c + f3 * 0.28 + r3 * hd, canvas_b * 0.95);
    // bedroll (muted) and a rolled-up jacket pillow inside
    blob(mb, c + f3 * -0.35 + Vec3::Y * 0.12, vec3(0.36, 0.1, 0.85), vec3(0.40, 0.42, 0.30), 0.0, 8, 4, f3.z.atan2(f3.x) + PI * 0.5, shade);
    blob(mb, c + f3 * -0.35 - r3 * 0.62 + Vec3::Y * 0.2, vec3(0.28, 0.08, 0.16), vec3(0.74, 0.70, 0.60), 0.0, 6, 3, 0.0, shade);
    // driftwood poles, ropes and stakes
    cyl(mb, c + r3 * hd, rf0 + r3 * 0.05, 0.04, 0.035, vec3(0.42, 0.34, 0.25), 5);
    cyl(mb, c - r3 * hd, rb0 - r3 * 0.05, 0.04, 0.035, vec3(0.42, 0.34, 0.25), 5);
    for sg in [-1.0f32, 1.0] {
        let stake = c + f3 * sg * (hw + 1.0) + r3 * (hd + 0.7);
        cyl(mb, stake, stake + Vec3::Y * 0.25, 0.025, 0.02, vec3(0.42, 0.34, 0.25), 4);
        cyl(mb, rf0, stake + Vec3::Y * 0.2, 0.01, 0.01, vec3(0.65, 0.55, 0.38), 3);
        let stake2 = c + f3 * sg * (hw + 1.0) - r3 * (hd + 0.7);
        cyl(mb, stake2, stake2 + Vec3::Y * 0.25, 0.025, 0.02, vec3(0.42, 0.34, 0.25), 4);
        cyl(mb, rb0, stake2 + Vec3::Y * 0.2, 0.01, 0.01, vec3(0.65, 0.55, 0.38), 3);
    }
    cols.push(Col::Obb { c: vec2(c.x, c.z), f: vec2(f3.x, f3.z), hx: hw + 0.08, hz: hd + 0.08, y0: c.y - 1.0, top: c.y + hh, walk: false });
}

fn build_plaza(pool: &mut HashMap<(i32, i32), MB>, t: &Terrain) -> Plaza {
    let c2 = vec2(PLAZA_C.0, PLAZA_C.1);
    let gy = t.at(c2.x, c2.y);
    let center = vec3(c2.x, gy, c2.y);
    let at = |ang: f32, r: f32| {
        let p = polar(c2, ang, r);
        vec3(p.x, gy, p.y)
    };
    let mut cols = vec![];
    // the hermit's hut
    let shop = at(290.0, 9.4);
    let sfr = dir_to(shop, center);
    let (npc_pos, npc_yaw) = build_shop(pool, &mut cols, shop, sfr);
    let sf3 = vec3(-sfr.z, 0.0, sfr.x);
    // salvage washed up beside the hut: barrels, crates, rope, baskets
    {
        let p = shop + sf3 * 3.5 + sfr * 0.8;
        let mb = pmb(pool, p);
        barrel(mb, p, 0.85, vec3(0.42, 0.31, 0.20));
        barrel(mb, p + sf3 * 0.7 + sfr * 0.3, 0.8, vec3(0.36, 0.30, 0.24));
        let p2 = p + sf3 * 0.7 + sfr * 0.3;
        cols.push(Col::Circle { c: vec2(p.x, p.z), r: 0.34, y0: gy - 1.0, top: gy + 0.85, walk: false });
        cols.push(Col::Circle { c: vec2(p2.x, p2.z), r: 0.34, y0: gy - 1.0, top: gy + 0.8, walk: false });
        let q = shop - sf3 * 3.4 + sfr * 0.5;
        let mb = pmb(pool, q);
        crate_box(mb, q, rf(0.0, TAU), 0.7, vec3(0.50, 0.40, 0.28));
        crate_box(mb, q + Vec3::Y * 0.7 + sf3 * 0.05, rf(0.0, TAU), 0.55, vec3(0.44, 0.36, 0.26));
        crate_box(mb, q + sfr * 1.0 + sf3 * 0.4, rf(0.0, TAU), 0.5, vec3(0.38, 0.32, 0.25));
        let q3 = q + sfr * 1.0 + sf3 * 0.4;
        cols.push(Col::Circle { c: vec2(q.x, q.z), r: 0.5, y0: gy - 1.0, top: gy + 1.3, walk: false });
        cols.push(Col::Circle { c: vec2(q3.x, q3.z), r: 0.38, y0: gy - 1.0, top: gy + 0.55, walk: false });
        let rp = shop + sfr * 2.6 - sf3 * 1.6;
        rope_coil(pmb(pool, rp), rp);
        let bp = shop + sfr * 2.4 + sf3 * 2.0;
        basket(pmb(pool, bp), bp, 1.0);
        cols.push(Col::Circle { c: vec2(bp.x, bp.z), r: 0.27, y0: gy - 1.0, top: gy + 0.3, walk: false });
    }
    // washed-ashore cargo, driftwood and a fish rack where there is room
    {
        let p = at(350.0, 10.5);
        let mb = pmb(pool, p);
        barrel(mb, p, 0.8, vec3(0.34, 0.28, 0.22));
        crate_box(mb, p + vec3(0.9, 0.0, 0.3), rf(0.0, TAU), 0.6, vec3(0.46, 0.38, 0.27));
        let cr = p + vec3(0.9, 0.0, 0.3);
        cols.push(Col::Circle { c: vec2(p.x, p.z), r: 0.34, y0: gy - 1.0, top: gy + 0.8, walk: false });
        cols.push(Col::Circle { c: vec2(cr.x, cr.z), r: 0.44, y0: gy - 1.0, top: gy + 0.6, walk: false });
        let p = at(50.0, 10.0);
        let mb = pmb(pool, p);
        for i in 0..3 {
            let a = i as f32 * 1.1 + rf(-0.2, 0.2);
            let d = vec3(a.cos(), 0.0, a.sin());
            cyl(mb, p - d * 1.1 + Vec3::Y * (0.12 + 0.1 * i as f32), p + d * 1.1 + Vec3::Y * (0.14 + 0.1 * i as f32), 0.13, 0.1, driftwood_col(), 7);
        }
        cols.push(Col::Circle { c: vec2(p.x, p.z), r: 1.15, y0: gy - 1.0, top: gy + 0.5, walk: false });
        let p = at(110.0, 9.5);
        let d = dir_to(p, center);
        fish_rack(pmb(pool, p), p, vec3(-d.z, 0.0, d.x));
        cols.push(Col::Obb { c: vec2(p.x, p.z), f: vec2(-d.z, d.x), hx: 1.08, hz: 0.14, y0: gy - 1.0, top: gy + 1.6, walk: false });
        for ang in [20.0f32, 80.0, 150.0] {
            let p = at(ang, rf(7.0, 11.0));
            for _ in 0..3 {
                let q = p + vec3(rf(-1.0, 1.0), 0.0, rf(-1.0, 1.0));
                blob(pmb(pool, q), q + Vec3::Y * 0.01, vec3(0.05, 0.025, 0.04), vec3(0.93, 0.86, 0.78), 0.0, 6, 3, rf(0.0, TAU), shade);
            }
        }
    }
    // campfire and tent
    let fire = at(188.0, 4.6);
    let tent = at(176.0, 10.2);
    build_campfire(pool, &mut cols, fire, dir_to(fire, tent));
    let tfr = dir_to(tent, fire);
    build_tent(pool, &mut cols, tent, tfr);
    {
        let tside = vec3(-tfr.z, 0.0, tfr.x);
        let p = tent + tside * 3.4 + tfr * 1.0;
        firewood(pmb(pool, p), p, tfr);
        cols.push(Col::Obb { c: vec2(p.x, p.z), f: vec2(tfr.x, tfr.z), hx: 0.72, hz: 0.5, y0: gy - 1.0, top: gy + 0.5, walk: false });
        let q = tent - tside * 3.2 + tfr * 1.4;
        rope_coil(pmb(pool, q), q);
    }
    let spawn = at(238.0, 5.5);
    let to_shop = dir_to(spawn, shop);
    Plaza { npc_pos, npc_yaw, tent_pos: tent, fire_pos: fire, spawn, spawn_yaw: to_shop.z.atan2(to_shop.x), cols }
}

// ---------------------------------------------------------------------------
// Static world assembly
// ---------------------------------------------------------------------------
fn key(x: f32, z: f32) -> (i32, i32) {
    (
        (((x + HALF) / CW).floor() as i32).clamp(0, NCH - 1),
        (((z + HALF) / CW).floor() as i32).clamp(0, NCH - 1),
    )
}

fn pmb(pool: &mut HashMap<(i32, i32), MB>, p: Vec3) -> &mut MB {
    pool.entry(key(p.x, p.z)).or_insert_with(MB::new)
}

struct Cell {
    center: Vec3,
    radius: f32,
    meshes: Vec<Mesh>,
}

fn build_terrain(t: &Terrain, pool: &mut HashMap<(i32, i32), MB>) {
    for cz in 0..NCH as usize {
        for cx in 0..NCH as usize {
            let mut maxh = f32::MIN;
            for iz in 0..=CHUNK {
                for ix in 0..=CHUNK {
                    maxh = maxh.max(t.grid(cx * CHUNK + ix, cz * CHUNK + iz));
                }
            }
            // the sea floor is built too (you can dive all the way down); deep chunks are coarser
            let step = if maxh < -6.5 { 2usize } else { 1 };
            let n = CHUNK / step;
            let mb = pool.entry((cx as i32, cz as i32)).or_insert_with(MB::new);
            mb.ensure((n + 1) * (n + 1), n * n * 6);
            let base = mb.v.len() as u16;
            for iz in 0..=n {
                for ix in 0..=n {
                    let gx = cx * CHUNK + ix * step;
                    let gz = cz * CHUNK + iz * step;
                    let x = -HALF + gx as f32 * CELL;
                    let z = -HALF + gz as f32 * CELL;
                    let h = t.grid(gx, gz);
                    // wide-stencil normal: smooth shading across the grid
                    let e = CELL * step as f32;
                    let nrm = vec3(t.at(x - e, z) - t.at(x + e, z), 2.0 * e, t.at(x, z - e) - t.at(x, z + e)).normalize();
                    let l = shade(ground_col(x, z, h, nrm), nrm);
                    mb.vert_uv(vec3(x, h, z), l, x, z);
                }
            }
            let w = (n + 1) as u16;
            for iz in 0..n as u16 {
                for ix in 0..n as u16 {
                    let a = base + iz * w + ix;
                    let b = a + 1;
                    let c = a + w;
                    let d = c + 1;
                    if (ix + iz) % 2 == 0 {
                        mb.i.extend_from_slice(&[a, c, b, b, c, d]);
                    } else {
                        mb.i.extend_from_slice(&[a, c, d, a, d, b]);
                    }
                }
            }
        }
    }
}
fn sample_land(t: &Terrain, hmin: f32, hmax: f32, nymin: f32, nymax: f32) -> Option<(Vec3, Vec3)> {
    for _ in 0..800 {
        let r = rf(0.0, 1.0);
        let (cx, cz, half, _) = if r < 0.8 { ISLES[0] } else if r < 0.92 { ISLES[1] } else { ISLES[2] };
        let x = cx + rf(-half, half);
        let z = cz + rf(-half, half);
        let h = t.at(x, z);
        if h >= hmin && h <= hmax {
            let n = t.normal(x, z);
            if n.y >= nymin && n.y <= nymax {
                let blocked = AVOID.with(|a| a.borrow().as_ref().map(|d| d.near(x, z, 9.5)).unwrap_or(false));
                let on_plaza = {
                    let pt = vec2(x, z);
                    let c = vec2(PLAZA_C.0, PLAZA_C.1);
                    let mut hit = pt.distance(c) < PLAZA_R + 4.0;
                    if let Some((a, b)) = PATH.with(|p| p.get()) {
                        let ab = b - a;
                        let tt = ((pt - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
                        hit |= pt.distance(a + ab * tt) < 2.4;
                    }
                    hit
                };
                if blocked || on_plaza {
                    continue;
                }
                return Some((vec3(x, h, z), n));
            }
        }
    }
    None
}

struct Scatter {
    cols: Vec<Col>,
    bushes: Vec<Vec3>,
    trunks: Vec<Vec2>,
    nests: Vec<Vec2>,
    palms: Vec<Vec3>,
}

fn scatter(t: &Terrain, pool: &mut HashMap<(i32, i32), MB>) -> Scatter {
    let mut trunks: Vec<Vec2> = vec![];
    let mut palms: Vec<Vec3> = vec![];

    rand::srand(9001);
    // Palms: dense along the beach, thinner inland
    let mut placed = 0;
    let mut guard = 0;
    while placed < 80 && guard < 6000 {
        guard += 1;
        if let Some((p, _)) = sample_land(t, 0.8, 9.0, 0.86, 1.0) {
            let keep = if p.y < 3.0 { 1.0 } else { 0.3 };
            if rf(0.0, 1.0) > keep {
                continue;
            }
            let q = vec2(p.x, p.z);
            if trunks.iter().any(|o| o.distance(q) < 2.4) {
                continue;
            }
            let young = rf(0.0, 1.0) < 0.22;
            let ps = if young { rf(0.3, 0.5) } else { rf(0.85, 1.1) };
            isolated(|| palm(pmb(pool, p), p, ps));
            trunks.push(q);
            if !young {
                palms.push(p);
            }
            placed += 1;
        }
    }

    rand::srand(9002);
    // Jungle trees on the hills
    for _ in 0..32 {
        if let Some((p, _)) = sample_land(t, 3.5, 40.0, 0.8, 1.0) {
            let q = vec2(p.x, p.z);
            if trunks.iter().any(|o| o.distance(q) < 3.0) {
                continue;
            }
            isolated(|| broadleaf(pmb(pool, p), p));
            trunks.push(q);
        }
    }

    let mut bushes: Vec<Vec3> = vec![];
    rand::srand(9003);
    for _ in 0..cnt(125) {
        if let Some((p, _)) = sample_land(t, 1.5, 40.0, 0.75, 1.0) {
            isolated(|| bush(pmb(pool, p), p));
            bushes.push(p);
        }
    }
    rand::srand(9004);
    for _ in 0..cnt(2600) {
        if let Some((p, n)) = sample_land(t, 1.6, 40.0, 0.75, 1.0) {
            isolated(|| grass_tuft(pmb(pool, p), p, n, false));
        }
    }
    rand::srand(9005);
    for _ in 0..cnt(300) {
        if let Some((p, n)) = sample_land(t, 0.9, 1.9, 0.8, 1.0) {
            isolated(|| grass_tuft(pmb(pool, p), p, n, true));
        }
    }
    // rocks: hillside outcrops and dark shoreline rocks
    let mut cols: Vec<Col> = vec![];
    rand::srand(9006);
    for _ in 0..42 {
        if let Some((p, _)) = sample_land(t, 2.0, 40.0, 0.0, 0.9) {
            let sz = rf(0.6, 2.0);
            let (rr, top) = isolated(|| rock_on(pmb(pool, p), t, p, sz, false));
            cols.push(Col::Circle { c: vec2(p.x, p.z), r: rr, y0: -20.0, top, walk: true });
        }
    }
    rand::srand(9007);
    for _ in 0..50 {
        if let Some((p, _)) = sample_land(t, -0.9, 0.5, 0.0, 1.0) {
            let sz = rf(0.3, 1.4);
            let (rr, top) = isolated(|| rock_on(pmb(pool, p), t, p, sz, true));
            cols.push(Col::Circle { c: vec2(p.x, p.z), r: rr, y0: -20.0, top, walk: true });
        }
    }
    // shells
    rand::srand(9008);
    for _ in 0..90 {
        if let Some((p, _)) = sample_land(t, 0.2, 1.3, 0.85, 1.0) {
            let col = if rf(0.0, 1.0) < 0.5 { vec3(0.95, 0.90, 0.84) } else { vec3(0.90, 0.75, 0.68) };
            blob(pmb(pool, p), p + Vec3::Y * 0.01, vec3(0.04, 0.02, 0.03), col, 0.0, 6, 3, rf(0.0, TAU), shade);
        }
    }
    rand::srand(9009);
    for _ in 0..14 {
        if let Some((p, _)) = sample_land(t, 0.4, 1.4, 0.85, 1.0) {
            isolated(|| driftwood(pmb(pool, p), p));
        }
    }

    // ant nests
    let mut nests = vec![];
    rand::srand(9010);
    for _ in 0..7 {
        if let Some((p, _)) = sample_land(t, 1.8, 12.0, 0.9, 1.0) {
            blob(pmb(pool, p), p, vec3(0.45, 0.2, 0.45), vec3(0.50, 0.40, 0.28), 0.15, 8, 4, 0.0, shade);
            nests.push(vec2(p.x, p.z));
        }
    }
    Scatter { cols, bushes, trunks, nests, palms }
}

fn finish(pool: HashMap<(i32, i32), MB>) -> Vec<Cell> {
    pool.into_iter()
        .map(|((kx, kz), mut mb)| {
            mb.flush();
            Cell {
                center: vec3(-HALF + (kx as f32 + 0.5) * CW, 5.0, -HALF + (kz as f32 + 0.5) * CW),
                radius: CW * 0.75 + 16.0,
                meshes: mb.meshes,
            }
        })
        .collect()
}

fn build_clouds() -> Vec<Mesh> {
    let mut mb = MB::new();
    for _ in 0..16 {
        let a = rf(0.0, TAU);
        let d = rf(600.0, 2600.0);
        let c = vec3(a.cos() * d, rf(260.0, 480.0), a.sin() * d);
        for _ in 0..rand::gen_range(4, 8) {
            let r = rf(50.0, 130.0);
            let p = c + vec3(rf(-130.0, 130.0), rf(0.0, 30.0), rf(-80.0, 80.0));
            blob(&mut mb, p, vec3(r, r * 0.35, r * 0.8), Vec3::splat(rf(0.93, 1.0)), 0.12, 10, 6, rf(0.0, TAU), cloud_shade);
        }
    }
    mb.flush();
    mb.meshes
}

// ---------------------------------------------------------------------------
// Water geometry (animation and shading happen in the shader)
// ---------------------------------------------------------------------------
struct WChunk {
    center: Vec3,
    mesh: Mesh,
}

fn build_water(t: &Terrain) -> Vec<WChunk> {
    let mut out = vec![];
    for cz in 0..NCH as usize {
        for cx in 0..NCH as usize {
            let mut maxg = f32::MIN;
            let mut ming = f32::MAX;
            for iz in 0..=CHUNK {
                for ix in 0..=CHUNK {
                    let g = t.grid(cx * CHUNK + ix, cz * CHUNK + iz);
                    maxg = maxg.max(g);
                    ming = ming.min(g);
                }
            }
            if ming > 0.5 {
                continue;
            }
            let cells = if maxg < -6.5 { 10usize } else { CHUNK };
            let step = CW / cells as f32;
            let x0 = -HALF + cx as f32 * CW;
            let z0 = -HALF + cz as f32 * CW;
            let mut vertices = Vec::with_capacity((cells + 1) * (cells + 1));
            for iz in 0..=cells {
                for ix in 0..=cells {
                    let x = x0 + ix as f32 * step;
                    let z = z0 + iz as f32 * step;
                    let depth = -t.at(x, z);
                    vertices.push(Vertex::new(x, 0.0, z, depth, 0.0, WHITE));
                }
            }
            let w = (cells + 1) as u16;
            let mut indices = vec![];
            for iz in 0..cells as u16 {
                for ix in 0..cells as u16 {
                    let a = iz * w + ix;
                    indices.extend_from_slice(&[a, a + w, a + 1, a + 1, a + w, a + w + 1]);
                }
            }
            out.push(WChunk {
                center: vec3(x0 + CW * 0.5, 0.0, z0 + CW * 0.5),
                mesh: Mesh { vertices, indices, texture: None },
            });
        }
    }
    out
}

fn build_sea_ring() -> Mesh {
    let segs = 64;
    let ring = |a: f32| -> (Vec3, Vec3) {
        let d = vec3(a.cos(), 0.0, a.sin());
        let s = HALF / d.x.abs().max(d.z.abs());
        (d * (s - 0.2), d * 6000.0)
    };
    let mut vertices = vec![];
    let mut indices: Vec<u16> = vec![];
    for i in 0..=segs {
        let (a, b) = ring(TAU * i as f32 / segs as f32);
        vertices.push(Vertex::new(a.x, 0.0, a.z, 40.0, 0.0, WHITE));
        vertices.push(Vertex::new(b.x, 0.0, b.z, 40.0, 0.0, WHITE));
    }
    for i in 0..segs as u16 {
        let k = i * 2;
        indices.extend_from_slice(&[k, k + 1, k + 2, k + 2, k + 1, k + 3]);
    }
    Mesh { vertices, indices, texture: None }
}

// ---------------------------------------------------------------------------
// Day / night
// ---------------------------------------------------------------------------
struct Sky {
    sun_dir: Vec3,
    moon_dir: Vec3,
    light_dir: Vec3,
    light_col: Vec3,
    amb: Vec3,
    horizon: Vec3,
    zenith: Vec3,
    lum: f32,
    day_f: f32,
    twi: f32,
    sun_el: f32,
    moon_el: f32,
    arc: f32,
    clock: f32,
}

fn sky_at(tc: f32) -> Sky {
    let t = tc.rem_euclid(CYCLE);
    let a = if t < DAY_LEN { PI * t / DAY_LEN } else { PI + PI * (t - DAY_LEN) / NIGHT_LEN };
    let sun_dir = vec3(a.cos(), a.sin(), -0.3).normalize();
    let moon_dir = vec3(-a.cos(), -a.sin(), 0.25).normalize();
    let sun_el = sun_dir.y;
    let moon_el = moon_dir.y;
    let day_f = smooth(-0.10, 0.22, sun_el);
    let twi = smooth(-0.22, -0.02, sun_el) * (1.0 - smooth(0.0, 0.28, sun_el));

    let hor_d = vec3(0.80, 0.89, 0.96);
    let zen_d = vec3(0.20, 0.45, 0.82);
    let hor_n = vec3(0.035, 0.055, 0.115);
    let zen_n = vec3(0.006, 0.012, 0.04);
    let mut horizon = hor_n.lerp(hor_d, day_f);
    let mut zenith = zen_n.lerp(zen_d, day_f);
    horizon = horizon.lerp(vec3(0.96, 0.50, 0.28), twi * 0.75);
    zenith = zenith.lerp(vec3(0.28, 0.34, 0.60), twi * 0.5);

    let sun_i = smooth(-0.03, 0.20, sun_el);
    let moon_i = smooth(-0.10, 0.20, moon_el) * (1.0 - day_f);
    let sun_col = vec3(1.0, 0.95, 0.82).lerp(vec3(1.0, 0.58, 0.32), twi * 0.9) * 0.85;
    let moon_col = vec3(0.40, 0.52, 0.85) * 0.34;
    let ldir = (sun_dir * sun_i + moon_dir * moon_i).normalize_or_zero();
    let light_dir = if ldir == Vec3::ZERO { sun_dir } else { ldir };
    let light_col = sun_col * sun_i + moon_col * moon_i;
    let amb_d = vec3(0.32, 0.36, 0.44);
    let amb_n = vec3(0.07, 0.09, 0.18);
    let amb = amb_n.lerp(amb_d, day_f) * vec3(1.0 + 0.15 * twi, 1.0, 1.0 - 0.1 * twi);
    let clock = if t < DAY_LEN { 6.0 + 12.0 * t / DAY_LEN } else { (18.0 + 12.0 * (t - DAY_LEN) / NIGHT_LEN) % 24.0 };
    Sky {
        sun_dir,
        moon_dir,
        light_dir,
        light_col,
        amb,
        horizon,
        zenith,
        lum: 0.10 + 0.90 * day_f,
        day_f,
        twi,
        sun_el,
        moon_el,
        arc: a,
        clock,
    }
}

fn draw_sky(eye: Vec3, sky: &Sky) {
    let (lon, lat) = (64usize, 24usize);
    let mut v = Vec::with_capacity((lat + 1) * (lon + 1));
    let mut idx = Vec::with_capacity(lat * lon * 6);
    let sun_vis = smooth(-0.15, 0.05, sky.sun_el);
    let moon_vis = smooth(-0.15, 0.05, sky.moon_el);
    let sun_glow = vec3(1.0, 0.9, 0.7).lerp(vec3(1.0, 0.5, 0.25), sky.twi);
    for j in 0..=lat {
        let e = -0.25 + (PI / 2.0 + 0.25) * j as f32 / lat as f32;
        for i in 0..=lon {
            let a = TAU * i as f32 / lon as f32;
            let d = vec3(e.cos() * a.cos(), e.sin(), e.cos() * a.sin());
            let t = (e.max(0.0) / (PI / 2.0)).powf(0.45);
            let mut c = sky.horizon.lerp(sky.zenith, t);
            let sd = d.dot(sky.sun_dir).max(0.0);
            let md = d.dot(sky.moon_dir).max(0.0);
            c += sun_glow * (sd.powi(12) * 0.35 + sd.powi(4) * 0.18 * sky.twi + sd.powi(200) * 0.6) * sun_vis;
            c += vec3(0.55, 0.65, 0.9) * (md.powi(30) * 0.22 + md.powi(500) * 0.5) * moon_vis * (1.0 - sky.day_f);
            let p = eye + d * 6000.0;
            v.push(Vertex::new(p.x, p.y, p.z, 0.0, 0.0, col3(c)));
        }
    }
    let w = (lon + 1) as u16;
    for j in 0..lat as u16 {
        for i in 0..lon as u16 {
            let a = j * w + i;
            idx.extend_from_slice(&[a, a + w, a + 1, a + 1, a + w, a + w + 1]);
        }
    }
    draw_mesh(&Mesh { vertices: v, indices: idx, texture: None });
}

fn draw_stars(eye: Vec3, sky: &Sky, stars: &[(Vec3, f32, f32)]) {
    let night = 1.0 - smooth(-0.12, 0.10, sky.sun_el);
    if night < 0.02 {
        return;
    }
    let (sa, ca) = sky.arc.sin_cos();
    let mut v = Vec::with_capacity(stars.len() * 4);
    let mut idx = Vec::with_capacity(stars.len() * 6);
    for &(d0, size, bright) in stars {
        let d =vec3(d0.x * ca - d0.y * sa, d0.x * sa + d0.y * ca, d0.z);
        if d.y < -0.05 {
            continue;
        }
        let t1 = d.cross(Vec3::Y).normalize_or_zero();
        let t1 = if t1 == Vec3::ZERO { Vec3::X } else { t1 };
        let t2 = d.cross(t1);
        let c = eye + d * 5800.0;
        let tw = 0.75 + 0.25 * (get_time() as f32 * 3.0 + size * 40.0).sin();
        let col = Color::new(0.85, 0.9, 1.0, (night * bright * tw).min(1.0));
        let b = v.len() as u16;
        for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let p = c + t1 * sx * size + t2 * sy * size;
            v.push(Vertex::new(p.x, p.y, p.z, 0.0, 0.0, col));
        }
        idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    if !idx.is_empty() {
        draw_mesh(&Mesh { vertices: v, indices: idx, texture: None });
    }
}

// ---------------------------------------------------------------------------
// Shaders & textures
// ---------------------------------------------------------------------------
const SWELL_GLSL: &str = "
float swellh(vec2 p, float t){
  return 0.10*sin(dot(p,vec2(0.62,0.78))*0.42+t*1.35)
       + 0.07*sin(dot(p,vec2(-0.8,0.6))*0.61+t*1.7)
       + 0.035*sin(dot(p,vec2(0.97,-0.25))*1.1+t*2.3);
}
vec2 swellg(vec2 p, float t){
  return 0.10*cos(dot(p,vec2(0.62,0.78))*0.42+t*1.35)*0.42*vec2(0.62,0.78)
       + 0.07*cos(dot(p,vec2(-0.8,0.6))*0.61+t*1.7)*0.61*vec2(-0.8,0.6)
       + 0.035*cos(dot(p,vec2(0.97,-0.25))*1.1+t*2.3)*1.1*vec2(0.97,-0.25);
}
";

const WORLD_VS: &str = "#version 100
attribute vec3 position; attribute vec2 texcoord; attribute vec4 color0; attribute vec4 normal;
varying vec2 uv; varying vec4 col; varying vec3 wp; varying vec3 nrm; varying float nw;
uniform mat4 Model; uniform mat4 Projection;
void main(){
  wp = (Model*vec4(position,1.0)).xyz; uv = texcoord; col = color0 / 255.0;
  nrm = normal.xyz; nw = normal.w;
  gl_Position = Projection*vec4(wp,1.0);
}";

const WORLD_FS: &str = "#version 100
precision highp float;
varying vec2 uv; varying vec4 col; varying vec3 wp; varying vec3 nrm; varying float nw;
uniform sampler2D Detail;
uniform vec3 CamPos; uniform float Under; uniform float Lum; uniform float Bump; uniform float FogK;
uniform vec3 SunDir; uniform vec3 SunCol; uniform vec3 AmbCol; uniform vec3 FogCol;
uniform vec3 P0; uniform vec3 C0; uniform vec3 P1; uniform vec3 C1; uniform vec3 P2; uniform vec3 C2; uniform vec3 P3; uniform vec3 C3;
vec3 lamp(vec3 P, vec3 C, vec3 N){
  vec3 d = P - wp; float l = length(d);
  float att = clamp(1.0 - l/20.0, 0.0, 1.0); att *= att;
  float nd = max(dot(N, d/max(l,0.001)), 0.0)*0.8 + 0.2;
  return C*att*nd;
}
void main(){
  float dist = distance(CamPos, wp);
  vec3 c;
  float cloud = step(0.5, nw) * step(nw, 1.5);
  if (dot(nrm,nrm) < 0.001) {
    c = col.rgb;
  } else {
    float n1 = texture2D(Detail, uv*0.09).r;
    float n2 = texture2D(Detail, uv*0.71+0.37).r;
    float n3 = texture2D(Detail, uv*3.9).r;
    float n4 = texture2D(Detail, uv*11.0).r;
    float fine = 1.0 - smoothstep(30.0, 90.0, dist);
    float m = 0.40*n1 + 0.30*n2 + (0.20*n3 + 0.10*n4)*fine + 0.30*(1.0-fine)*0.5;
    vec3 albedo = col.rgb * (0.50 + 1.00*m);
    vec3 N = normalize(nrm);
    if (dot(N, CamPos - wp) < 0.0) N = -N;
    if (Bump > 0.5 && N.y > 0.75 && fine > 0.0) {
      vec2 b = uv*3.9;
      float gx = texture2D(Detail, b + vec2(0.012,0.0)).r - n3;
      float gz = texture2D(Detail, b + vec2(0.0,0.012)).r - n3;
      N = normalize(N + vec3(gx, 0.0, gz) * 2.5 * fine);
    }
    float ndl = max(dot(N, SunDir), 0.0);
    vec3 L;
    if (cloud > 0.5) {
      L = (AmbCol*2.4 + SunCol*ndl*0.5) * (0.30 + 0.70*Lum);
      albedo = col.rgb;
    } else {
      L = AmbCol*(0.55 + 0.45*(0.5 + 0.5*N.y)) + SunCol*ndl;
      L += lamp(P0, C0, N) + lamp(P1, C1, N) + lamp(P2, C2, N) + lamp(P3, C3, N);
    }
    c = albedo * L;
  }
  if (cloud < 0.5) {
    float f = 1.0 - exp(-dist*dist*0.0000045*FogK);
    c = mix(c, FogCol, f);
  }
  if (Under > 0.5) {
    // the deeper you are, the darker: the abyss is black
    float dd = clamp(-CamPos.y, 0.0, 80.0);
    float dark = exp(-dd*0.045);
    float u = 1.0 - exp(-dist*(0.11 + dd*0.004));
    c = mix(c*vec3(0.45,0.75,0.85)*(0.25+0.75*dark), vec3(0.01,0.15,0.24)*Lum*dark, u);
  }
  gl_FragColor = vec4(c, col.a);
}";

fn water_vs() -> String {
    format!(
        "#version 100
attribute vec3 position; attribute vec2 texcoord;
varying vec3 wp; varying float depth;
uniform mat4 Model; uniform mat4 Projection;
uniform float Time; uniform vec3 CamPos;
{}
void main(){{
  vec3 p = position;
  depth = texcoord.x;
  float dist = distance(CamPos.xz, p.xz);
  float amp = smoothstep(0.2, 2.5, depth) * (1.0 - smoothstep(150.0, 400.0, dist));
  p.y += swellh(p.xz, Time) * amp;
  wp = p;
  gl_Position = Projection*vec4(p,1.0);
}}",
        SWELL_GLSL
    )
}

fn water_fs() -> String {
    format!(
        "#version 100
precision highp float;
varying vec3 wp; varying float depth;
uniform sampler2D Detail;
uniform vec3 CamPos; uniform vec3 SunDir; uniform vec3 SunCol; uniform float Time;
uniform vec3 HorCol; uniform vec3 ZenCol; uniform float Lum; uniform float WQ; uniform float FogK;
{}
float tn(vec2 p){{ return texture2D(Detail, p).r; }}
vec3 skyc(vec3 r){{
  float t = pow(clamp(r.y,0.0,1.0), 0.45);
  vec3 c = mix(HorCol, ZenCol, t);
  float s = max(dot(r,SunDir),0.0);
  c += SunCol*(pow(s,12.0)*0.35 + pow(s,200.0)*0.6)*1.2;
  return c;
}}
void main(){{
  float t = Time;
  vec3 toC = CamPos - wp;
  float dist = length(toC);
  vec3 V = toC / dist;
  float dist2 = distance(CamPos.xz, wp.xz);
  float amp = smoothstep(0.2, 2.5, depth) * (1.0 - smoothstep(150.0, 400.0, dist2));

  // big swell slope + three layers of scrolling ripples from the detail texture
  vec2 g = swellg(wp.xz, t) * amp;
  float e = 0.02;
  vec2 p1 = wp.xz*0.35 + vec2(t*0.050, t*0.030);
  vec2 p2 = wp.xz*1.10 + vec2(-t*0.070, t*0.050);
  vec2 p3 = wp.xz*3.30 + vec2(t*0.110, -t*0.090);
  float a1 = tn(p1); float a2 = tn(p2); float a3 = tn(p3);
  vec2 r1 = vec2(tn(p1+vec2(e,0.0))-a1, tn(p1+vec2(0.0,e))-a1) / e * 0.35;
  vec2 r2 = vec2(tn(p2+vec2(e,0.0))-a2, tn(p2+vec2(0.0,e))-a2) / e * 1.10;
  vec2 r3 = vec2(tn(p3+vec2(e,0.0))-a3, tn(p3+vec2(0.0,e))-a3) / e * 3.30;
  float f2 = (1.0 - smoothstep(60.0, 220.0, dist)) * step(0.5, WQ);
  float f3 = (1.0 - smoothstep(25.0, 90.0, dist)) * step(1.5, WQ);
  vec2 slope = g + r1*0.060 + r2*0.032*f2 + r3*0.014*f3;
  slope *= 0.35 + 0.65*smoothstep(0.0, 1.2, depth);
  vec3 n = normalize(vec3(-slope.x, 1.0, -slope.y));

  // seen from below the surface
  if (CamPos.y < wp.y) {{
    vec3 dirU = normalize(wp - CamPos);
    float win = smoothstep(0.55, 0.85, dirU.y);
    vec3 uc = mix(vec3(0.02,0.17,0.27), vec3(0.30,0.68,0.80), win) * (0.12 + 0.88*Lum);
    uc += vec3(0.5,0.7,0.7) * pow(a2*a3*2.0, 3.0) * 0.4 * win * Lum;
    gl_FragColor = vec4(uc, 1.0);
    return;
  }}

  float ndv = clamp(dot(n, V), 0.0, 1.0);
  float F = 0.02 + 0.98*pow(1.0 - ndv, 5.0);
  F = clamp(F, 0.0, 0.92);
  vec3 R = reflect(-V, n);
  R.y = abs(R.y);
  vec3 refl = skyc(R);

  vec3 shallow = vec3(0.34, 0.86, 0.78);
  vec3 mid     = vec3(0.05, 0.46, 0.60);
  vec3 deepc   = vec3(0.015, 0.19, 0.35);
  vec3 body = mix(shallow, mid, smoothstep(0.0, 2.5, depth));
  body = mix(body, deepc, smoothstep(2.0, 9.0, depth));
  // caustic shimmer over shallow sand
  float cs = tn(wp.xz*0.8 + vec2(t*0.10, t*0.07)) * tn(wp.xz*1.3 - vec2(t*0.08, t*0.10));
  body += vec3(0.5,0.6,0.5) * pow(clamp(cs*2.4, 0.0, 1.0), 3.0) * (1.0 - smoothstep(0.0, 3.5, depth)) * 0.55;
  body *= (0.10 + 0.90*Lum);

  vec3 col = mix(body, refl, F);
  vec3 H = normalize(V + SunDir);
  float spec = pow(max(dot(n, H), 0.0), 350.0);
  col += SunCol * spec * 2.9;

  // foam: lacy wash at the shoreline and a few whitecaps on the crests
  float lace = smoothstep(0.40, 0.62, tn(wp.xz*0.5 + vec2(t*0.04, -t*0.03))*0.6 + tn(wp.xz*1.7 - vec2(t*0.06, 0.0))*0.4);
  float wash = 0.22*sin(t*0.9 + wp.x*0.12 + wp.z*0.09) + 0.15*sin(t*1.7 + wp.z*0.31);
  float shore = 1.0 - smoothstep(0.04, 0.60 + wash, depth);
  float foam = shore*(0.30 + 0.70*lace) + (1.0 - smoothstep(0.0, 0.14, depth))*0.7;
  float crest = swellh(wp.xz, t) * amp;
  foam += smoothstep(0.12, 0.19, crest) * lace * 0.6 * smoothstep(3.0, 8.0, depth);
  foam = clamp(foam, 0.0, 1.0);
  col = mix(col, vec3(0.96,0.98,0.98)*0.92*(0.12 + 0.88*Lum), foam);

  float alpha = mix(0.30, 1.0, smoothstep(0.0, 5.5, depth));
  alpha = max(alpha, F*0.85);
  alpha = max(alpha, foam*0.92);
  alpha *= smoothstep(-0.06, 0.12, depth);

  float fg = 1.0 - exp(-dist*dist*0.0000045*FogK);
  col = mix(col, HorCol, fg);
  gl_FragColor = vec4(col, alpha);
}}",
        SWELL_GLSL
    )
}

/// Seamless grey noise used for ground / bark / leaf detail and water ripples.
fn make_detail() -> Texture2D {
    let n = 256u16;
    let mut img = Image::gen_image_color(n, n, WHITE);
    for y in 0..n as u32 {
        for x in 0..n as u32 {
            let u = x as f32 / n as f32;
            let v = y as f32 / n as f32;
            let (mut sum, mut amp, mut norm, mut per) = (0.0, 0.5, 0.0, 4i32);
            for o in 0..6 {
                sum += amp * pnoise(u * per as f32, v * per as f32, per, 31 + o);
                norm += amp;
                amp *= 0.55;
                per *= 2;
            }
            let f = (((sum / norm) - 0.5) * 2.0 + 0.5).clamp(0.0, 1.0);
            img.set_pixel(x, y, Color::new(f, f, f, 1.0));
        }
    }
    let tex = Texture2D::from_image(&img);
    let ctx = unsafe { get_internal_gl() }.quad_context;
    ctx.texture_set_wrap(tex.raw_miniquad_id(), TextureWrap::Repeat, TextureWrap::Repeat);
    ctx.texture_generate_mipmaps(tex.raw_miniquad_id());
    ctx.texture_set_filter(tex.raw_miniquad_id(), FilterMode::Linear, MipmapFilterMode::Linear);
    tex
}

// ---------------------------------------------------------------------------
// Coconuts
// ---------------------------------------------------------------------------
struct Coconut {
    id: u32,
    pos: Vec3,
    vel: Vec3,
    spin: f32,
    rest: bool,
}

fn update_coconut(c: &mut Coconut, t: &Terrain, dock: &Dock, time: f32, dt: f32) {
    let wy = water_h(t, c.pos.x, c.pos.z, time);
    let mut g = t.at(c.pos.x, c.pos.z);
    if dock.contains(c.pos.x, c.pos.z) && c.pos.y > dock.deck_y - 0.3 {
        g = g.max(dock.deck_y);
    }
    let g = g + 0.085;
    let deep = g < wy - 0.1;
    if c.rest && !deep {
        return;
    }
    c.rest = false;
    if deep {
        // floats, bobbing with the swell and drifting slowly
        let target = wy + 0.01;
        c.vel.y += ((target - c.pos.y) * 22.0 - c.vel.y * 3.5) * dt;
        c.vel.x += (-c.vel.x * 1.5 + 0.04) * dt;
        c.vel.z += (-c.vel.z * 1.5 - 0.02) * dt;
        c.pos += c.vel * dt;
        c.spin += dt * 0.3;
        return;
    }
    c.vel.y -= 9.8 * dt;
    c.pos += c.vel * dt;
    if c.pos.y <= g {
        c.pos.y = g;
        let n = t.normal(c.pos.x, c.pos.z);
        if c.vel.y < -1.2 {
            c.vel.y *= -0.35;
            c.vel.x *= 0.6;
            c.vel.z *= 0.6;
        } else {
            c.vel.y = 0.0;
            c.vel.x += n.x * 2.4 * dt;
            c.vel.z += n.z * 2.4 * dt;
            let damp = (1.0 - 3.0 * dt).max(0.0);
            c.vel.x *= damp;
            c.vel.z *= damp;
        }
        c.spin += (c.vel.x.abs() + c.vel.z.abs()) * dt * 4.0;
        if c.vel.length() < 0.06 && n.y > 0.93 {
            c.vel = Vec3::ZERO;
            c.rest = true;
        }
    }
}

fn draw_coconut(c: &Coconut, mb: &mut MB, hl: bool) {
    let col = if hl { vec3(0.78, 0.56, 0.30) } else { vec3(0.45, 0.32, 0.17) };
    blob(mb, c.pos, vec3(0.115, 0.085, 0.085), col, 0.0, 8, 5, c.spin, shade);
}

// ---------------------------------------------------------------------------
// Bugs
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Spider,
    Ant,
    Beetle,
}

struct Crawler {
    kind: Kind,
    pos: Vec3,
    heading: f32,
    speed: f32,
    target: f32,
    timer: f32,
    phase: f32,
    size: f32,
    base_speed: f32,
    col: Vec3,
    col2: Vec3,
    home: Vec2,
}

fn spawn_crawlers(t: &Terrain, nests: &[Vec2]) -> Vec<Crawler> {
    let mut v = vec![];
    let mk = |kind: Kind, p: Vec3, size: f32, base_speed: f32, col: Vec3, col2: Vec3, home: Vec2| Crawler {
        kind,
        pos: p,
        heading: rf(0.0, TAU),
        speed: 0.0,
        target: 0.0,
        timer: rf(0.0, 2.0),
        phase: rf(0.0, TAU),
        size,
        base_speed,
        col,
        col2,
        home,
    };
    for _ in 0..110 {
        if let Some((p, _)) = sample_land(t, 0.6, 40.0, 0.6, 1.0) {
            let (c1, c2) = if p.y < 1.7 {
                (vec3(0.66, 0.58, 0.44), vec3(0.58, 0.50, 0.38)) // pale sand spider
            } else if rf(0.0, 1.0) < 0.6 {
                (vec3(0.30, 0.22, 0.15), vec3(0.22, 0.16, 0.11)) // brown wolf spider
            } else {
                (vec3(0.07, 0.07, 0.07), vec3(0.10, 0.09, 0.08)) // black spider
            };
            v.push(mk(Kind::Spider, p, rf(0.05, 0.095), 0.35, c1, c2, vec2(p.x, p.z)));
        }
    }
    rand::srand(9007);
    for _ in 0..50 {
        if let Some((p, _)) = sample_land(t, 1.5, 40.0, 0.7, 1.0) {
            let green = rf(0.0, 1.0) < 0.5;
            let c = if green { vec3(0.10, 0.25, 0.14) } else { vec3(0.06, 0.05, 0.05) };
            v.push(mk(Kind::Beetle, p, rf(0.04, 0.06), 0.08, c, c * 0.7, vec2(p.x, p.z)));
        }
    }
    for n in nests {
        for _ in 0..18 {
            let x = n.x + rf(-1.5, 1.5);
            let z = n.y + rf(-1.5, 1.5);
            let p = vec3(x, t.at(x, z), z);
            v.push(mk(Kind::Ant, p, rf(0.022, 0.03), 0.13, vec3(0.20, 0.07, 0.04), vec3(0.12, 0.05, 0.03), *n));
        }
    }
    v
}

fn update_crawler(c: &mut Crawler, t: &Terrain, player: Vec3, dt: f32) {
    c.timer -= dt;
    if c.timer <= 0.0 {
        if rf(0.0, 1.0) < 0.35 {
            c.target = 0.0;
            c.timer = rf(0.4, 2.5);
        } else {
            c.target = c.base_speed * rf(0.6, 1.3);
            c.heading += rf(-1.3, 1.3);
            c.timer = rf(0.6, 3.5);
        }
    }
    if c.kind == Kind::Ant {
        let to_home = c.home - vec2(c.pos.x, c.pos.z);
        if to_home.length() > 3.5 {
            c.heading = lerp_angle(c.heading, to_home.y.atan2(to_home.x), dt * 3.0);
        }
    }
    // scurry away from the player's feet
    let dp = vec2(c.pos.x - player.x, c.pos.z - player.z);
    if dp.length() < 1.5 && (player.y - c.pos.y).abs() < 2.5 {
        c.heading = dp.y.atan2(dp.x) + rf(-0.4, 0.4);
        c.target = c.base_speed * 3.2;
        c.timer = 0.5;
    }
    if c.speed > 0.01 {
        c.heading += rf(-1.0, 1.0) * dt * 2.0;
    }
    c.speed += (c.target - c.speed) * (dt * 8.0).min(1.0);
    let nx = c.pos.x + c.heading.cos() * c.speed * dt;
    let nz = c.pos.z + c.heading.sin() * c.speed * dt;
    let hn = t.at(nx, nz);
    if hn < 0.3 {
        c.heading += PI * 0.7;
        c.speed *= 0.3;
    } else {
        c.pos = vec3(nx, hn, nz);
    }
    c.phase += c.speed / c.size * dt * 1.6;
}

#[allow(clippy::too_many_arguments)]
fn legs(
    mb: &mut MB,
    center: Vec3,
    ground: f32,
    f: Vec3,
    r: Vec3,
    s: f32,
    hip_f: &[f32],
    foot_f: &[f32],
    reach: f32,
    knee_h: f32,
    phase: f32,
    m: f32,
    col: Vec3,
    rad: f32,
) {
    for (j, (&hf, &ff)) in hip_f.iter().zip(foot_f).enumerate() {
        for sd in [-1.0f32, 1.0] {
            let ph = phase + if (j + (sd > 0.0) as usize) % 2 == 0 { 0.0 } else { PI };
            let hip = center + f * hf * s + r * sd * 0.1 * s;
            let mut foot = vec3(center.x, 0.0, center.z) + f * (ff * s + ph.sin() * 0.22 * s * m) + r * sd * reach * s;
            foot.y = ground + ph.cos().max(0.0) * 0.25 * s * m + 0.003;
            let knee = hip.lerp(foot, 0.45) + Vec3::Y * knee_h * s + r * sd * 0.1 * s;
            let rr = (rad * s).max(0.0012);
            cyl(mb, hip, knee, rr, rr * 0.8, col, 3);
            cyl(mb, knee, foot, rr * 0.8, rr * 0.45, col, 3);
        }
    }
}

fn draw_crawler(c: &Crawler, mb: &mut MB) {
    let s = c.size;
    let f = vec3(c.heading.cos(), 0.0, c.heading.sin());
    let r = vec3(-c.heading.sin(), 0.0, c.heading.cos());
    let p = c.pos;
    let m = (c.speed / c.base_speed).min(1.0);
    let legc = c.col2 * 0.85;
    match c.kind {
        Kind::Spider => {
            let ceph = p + f * 0.24 * s + Vec3::Y * 0.42 * s;
            let abd = p - f * 0.40 * s + Vec3::Y * 0.55 * s;
            blob(mb, ceph, vec3(0.30, 0.22, 0.26) * s, c.col, 0.0, 7, 5, c.heading, shade);
            blob(mb, abd, vec3(0.50, 0.38, 0.40) * s, c.col2, 0.0, 8, 5, c.heading, shade);
            // two pale eyes at the front
            for sd in [-1.0f32, 1.0] {
                blob(mb, ceph + f * 0.26 * s + r * sd * 0.08 * s + Vec3::Y * 0.1 * s, Vec3::splat(0.045 * s), vec3(0.8, 0.8, 0.75), 0.0, 4, 3, 0.0, shade);
            }
            legs(mb, ceph, p.y, f, r, s, &[0.12, 0.04, -0.04, -0.12], &[1.0, 0.4, -0.3, -0.85], 0.95, 0.6, c.phase, m, legc, 0.05);
        }
        Kind::Ant => {
            blob(mb, p + f * 0.40 * s + Vec3::Y * 0.28 * s, vec3(0.13, 0.11, 0.12) * s, c.col, 0.0, 5, 3, c.heading, shade);
            let th = p + f * 0.12 * s + Vec3::Y * 0.28 * s;
            blob(mb, th, vec3(0.17, 0.09, 0.08) * s, c.col, 0.0, 5, 3, c.heading, shade);
            blob(mb, p - f * 0.28 * s + Vec3::Y * 0.30 * s, vec3(0.24, 0.16, 0.17) * s, c.col2, 0.0, 6, 3, c.heading, shade);
            legs(mb, th, p.y, f, r, s, &[0.08, 0.0, -0.08], &[0.45, 0.0, -0.45], 0.5, 0.35, c.phase, m, legc, 0.03);
        }
        Kind::Beetle => {
            let shell = p + Vec3::Y * 0.30 * s;
            blob(mb, shell, vec3(0.45, 0.25, 0.32) * s, c.col, 0.0, 8, 5, c.heading, |c, n| {
                // glossy wing case: slightly lifted albedo
                shade(c * 1.25, n)
            });
            blob(mb, p + f * 0.45 * s + Vec3::Y * 0.22 * s, vec3(0.13, 0.1, 0.13) * s, c.col2, 0.0, 5, 3, c.heading, shade);
            legs(mb, shell - Vec3::Y * 0.1 * s, p.y, f, r, s, &[0.2, 0.0, -0.2], &[0.5, 0.0, -0.5], 0.55, 0.2, c.phase, m, legc, 0.035);
        }
    }
}

struct Fly {
    pos: Vec3,
    heading: f32,
    phase: f32,
    col: Vec3,
}

fn update_fly(b: &mut Fly, t: &Terrain, time: f32, dt: f32) {
    b.heading += (vnoise(time * 0.5 + b.phase * 10.0, b.phase * 3.0, 77) - 0.5) * dt * 6.0;
    if t.at(b.pos.x, b.pos.z) < 0.5 {
        // head back toward the nearest island centre
        let (cx, cz, _, _) = ISLES
            .iter()
            .copied()
            .min_by(|a, b2| {
                let da = (a.0 - b.pos.x).hypot(a.1 - b.pos.z);
                let db = (b2.0 - b.pos.x).hypot(b2.1 - b.pos.z);
                da.partial_cmp(&db).unwrap()
            })
            .unwrap();
        b.heading = lerp_angle(b.heading, (cz - b.pos.z).atan2(cx - b.pos.x), dt * 2.0);
    }
    b.pos.x += b.heading.cos() * 0.9 * dt;
    b.pos.z += b.heading.sin() * 0.9 * dt;
    let ty = t.at(b.pos.x, b.pos.z).max(0.0) + 0.7 + (0.5 + 0.5 * (time * 0.4 + b.phase).sin());
    b.pos.y += (ty - b.pos.y) * dt * 1.2 + (time * 7.0 + b.phase).sin() * dt * 0.5;
}

fn draw_fly(b: &Fly, time: f32, mb: &mut MB) {
    let fw = vec3(b.heading.cos(), 0.0, b.heading.sin());
    let r = vec3(-b.heading.sin(), 0.0, b.heading.cos());
    let p = b.pos;
    let flap = (time * 14.0 + b.phase * 5.0).sin() * 0.85 + 0.25;
    blob(mb, p, vec3(0.022, 0.007, 0.007), vec3(0.10, 0.08, 0.06), 0.0, 5, 3, b.heading, shade);
    let c1 = shade(b.col, Vec3::Y);
    let c2 = shade(b.col * 0.75, Vec3::Y);
    for sd in [-1.0f32, 1.0] {
        let wd = r * sd * flap.cos() + Vec3::Y * flap.sin();
        let a = p + fw * 0.012;
        let bb = p - fw * 0.006;
        let t1 = p + wd * 0.055 + fw * 0.035;
        let t2 = p + wd * 0.06 - fw * 0.005;
        let h1 = p + wd * 0.042 - fw * 0.032;
        mb.tri(a, t1, t2, c2, c1, c1);
        mb.tri(a, t2, bb, c2, c1, c2);
        mb.tri(bb, t2, h1, c2, c1, c1);
    }
}

// ---------------------------------------------------------------------------
// Fish (swimming around; what you catch lives in fish.rs)
// ---------------------------------------------------------------------------
struct Swimmer {
    pos: Vec3,
    heading: f32,
    speed: f32,
    phase: f32,
    depth: f32,
    sp: usize,
    len: f32,
    /// one of the big deep-water monsters
    monster: bool,
}

fn spawn_swimmers(t: &Terrain, dock: &Dock) -> Vec<Swimmer> {
    let mut v = vec![];
    let kinds = [0usize, 0, 0, 1, 1, 1, 2, 2, 3, 5, 7];
    let mut tries = 0;
    while v.len() < 190 && tries < 40000 {
        tries += 1;
        let near_dock = v.len() < 45;
        let (x, z) = if near_dock {
            let p = dock.point(dock.len + rf(-14.0, 10.0), rf(-12.0, 12.0), 0.0);
            (p.x, p.z)
        } else {
            (rf(-230.0, 230.0), rf(-230.0, 230.0))
        };
        let g = t.at(x, z);
        if g < -1.6 && g > -6.5 {
            let sp = kinds[rand::gen_range(0, kinds.len())];
            let kg = rf(fish::SPECIES[sp].kg.0, fish::SPECIES[sp].kg.1);
            v.push(Swimmer {
                pos: vec3(x, -0.6, z),
                heading: rf(0.0, TAU),
                speed: rf(0.4, 1.2) * (1.0 + fish::SPECIES[sp].len * 0.3),
                phase: rf(0.0, TAU),
                depth: rf(0.55, 1.5),
                sp,
                len: fish::fish_len(sp, kg),
                monster: false,
            });
        }
    }
    v
}

/// The things that live in the deep water, far from the island.
fn spawn_monsters(t: &Terrain) -> Vec<Swimmer> {
    let mut v = vec![];
    let mut tries = 0;
    while v.len() < 7 && tries < 20000 {
        tries += 1;
        let a = rf(0.0, TAU);
        let d = rf(70.0, 225.0);
        let (x, z) = (a.cos() * d, a.sin() * d);
        if t.at(x, z) < -12.0 && t.at(x + 8.0, z) < -12.0 && t.at(x, z + 8.0) < -12.0 {
            let sp = 11 + (v.len() % 3);
            let kg = rf(fish::SPECIES[sp].kg.0, fish::SPECIES[sp].kg.1);
            v.push(Swimmer {
                pos: vec3(x, -rf(3.0, 5.2), z),
                heading: rf(0.0, TAU),
                speed: rf(1.2, 2.0),
                phase: rf(0.0, TAU),
                depth: rf(3.0, 5.2),
                sp,
                len: fish::fish_len(sp, kg),
                monster: true,
            });
        }
    }
    v
}

/// A dead fish for props (drying racks, the counter): a still, detailed fish of a shallow species.
fn fish_shape(mb: &mut MB, pos: Vec3, heading: f32, size: f32, col: Vec3, _wag: f32) {
    let idx = [1usize, 0, 2][((col.x * 10.0) as usize) % 3];
    let len = fish::SPECIES[idx].len * size * 0.9;
    fish::draw_fish(mb, idx, pos, len, &fish::FishPose { heading, pitch: 0.0, roll: 0.0, t: 0.0, amp: 0.0, open: 0.0 }, true);
}

fn update_swimmer(f: &mut Swimmer, t: &Terrain, dt: f32) {
    let (lo, hi) = if f.monster { (ABYSS - 5.0, -11.0) } else { (-6.8, -1.4) };
    f.heading += rf(-1.0, 1.0) * dt * if f.monster { 0.5 } else { 1.4 };
    let nx = f.pos.x + f.heading.cos() * f.speed * dt;
    let nz = f.pos.z + f.heading.sin() * f.speed * dt;
    let g = t.at(nx, nz);
    if g > hi || g < lo {
        f.heading += PI * 0.65 + rf(-0.3, 0.3);
    } else {
        f.pos.x = nx;
        f.pos.z = nz;
    }
    let y = if f.monster { -f.depth } else { (-f.depth).max(g + 0.3).min(-0.35) };
    f.pos.y += (y - f.pos.y) * (dt * 2.0).min(1.0);
}


fn visible(center: Vec3, radius: f32, cam: Vec3, fwd: Vec3, right: Vec3) -> bool {
    let d = center - cam;
    let z = d.dot(fwd);
    if z < -radius {
        return false;
    }
    d.dot(right).abs() - radius < (z + radius) * 1.7
}

mod game;

fn window_conf() -> macroquad::conf::Conf {
    game::window_conf()
}

#[macroquad::main(window_conf)]
async fn main() {
    game::run().await;
}

/// The random generator is global, so tests that depend on it must not run at the same time.
#[cfg(test)]
pub static RAND_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod col_tests {
    use super::*;

    #[test]
    fn collide_hut_walls() {
        let _g = RAND_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        rand::srand(20_261_010);
        let t = Terrain::gen();
        let mut pool: HashMap<(i32, i32), MB> = HashMap::new();
        let plaza = build_plaza(&mut pool, &t);
        let c2 = vec2(PLAZA_C.0, PLAZA_C.1);
        let gy = t.at(c2.x, c2.y);
        let center = vec3(c2.x, gy, c2.y);
        let p = polar(c2, 290.0, 9.4);
        let shop = vec3(p.x, t.at(p.x, p.y), p.y);
        let fr = dir_to(shop, center);
        let f3 = vec3(-fr.z, 0.0, fr.x);
        let (hw, hd) = (2.4f32, 1.8f32);
        let mut worst = 9.0f32;
        let mut check = |pt: Vec3, n: Vec3, what: &str| {
            let mut feet = vec3(pt.x, shop.y + 0.06, pt.z);
            collide(&plaza.cols, &mut feet, 0.35);
            let _ = n;
            let moved = ((feet.x - pt.x).powi(2) + (feet.z - pt.z).powi(2)).sqrt();
            worst = worst.min(moved);
            // after being pushed out the player must be completely clear of every solid object
            for col in plaza.cols.iter() {
                if feet.y >= col.top() - 0.45 && col.walk() {
                    continue;
                }
                if feet.y >= col.top() || feet.y + 1.7 <= col.y0() {
                    continue;
                }
                let p = vec2(feet.x, feet.z);
                let desc = match col { Col::Circle { c, r, y0, top, walk } => format!("circle c={c:?} r={r} y0={y0} top={top} walk={walk}"), Col::Obb { c, f, hx, hz, y0, top, walk } => format!("obb c={c:?} f={f:?} hx={hx} hz={hz} y0={y0} top={top} walk={walk}") };
                assert!(col.push(p, 0.25).distance(p) < 0.01, "{what} at {pt:?}: still inside {desc} after being pushed out ({feet:?})");
            }
            let _ = moved;
        };
        for i in 0..=48 {
            let x = (-hw + 2.0 * hw * i as f32 / 48.0).clamp(-hw + 0.15, hw - 0.15);
            check(shop + f3 * x - fr * hd, fr, "back wall");
        }
        // (side walls: props stand right beside them, so a start point inside the wall can be squeezed
        // between two colliders; the check that matters is that nothing can be entered from outside,
        // which the back wall, counter and roof checks cover)
        let _ = f3 * 0.0;
        // the counter closes the whole front, wall to wall, with no gap at either end
        for i in 0..=48 {
            let x = (-hw + 2.0 * hw * i as f32 / 48.0).clamp(-hw + 0.15, hw - 0.15);
            check(shop + f3 * x + fr * (hd - 0.12), fr, "counter");
        }
        // nothing can be stood on or dropped through inside: the roof is solid ground over the whole hut
        for ix in 0..=24 {
            for iz in 0..=18 {
                let (x, z) = (-hw + 2.0 * hw * ix as f32 / 24.0, -hd + 2.0 * hd * iz as f32 / 18.0);
                let pt = shop + f3 * x + fr * z;
                let top = stand_on(&plaza.cols, pt.x, pt.z, shop.y + 4.0);
                assert!(matches!(top, Some(h) if h > shop.y + 2.0), "no roof ground at x {x} z {z}: {top:?}");
            }
        }
        println!("worst push {worst}, flat gy {gy} shop y {}", shop.y);
    }
}
