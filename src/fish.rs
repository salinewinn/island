//! Fish: the species table, what you can catch, and a detailed animated fish model
//! (lofted body with scale/pattern colouring, ribbed fins, eyes, teeth, wiggling spine).

use super::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Pat {
    Plain,
    Stripes,
    Spots,
    Bands,
    Speckle,
    Blotch,
}

#[derive(Clone, Copy)]
pub struct Species {
    pub name: &'static str,
    pub kg: (f32, f32),
    /// relative chance to bite
    pub weight: f32,
    /// coins per kg (flat price for junk)
    pub price: f32,
    /// how strongly luck favours it
    pub rarity: f32,
    /// only found in the deep water
    pub deep: bool,
    /// body length (m) at its median weight
    pub len: f32,
    pub hb: f32,
    pub wb: f32,
    /// 0 torpedo, 1 disc, 2 round, 3 eel
    pub shape: u8,
    pub back: [f32; 3],
    pub belly: [f32; 3],
    pub fin: [f32; 3],
    pub mark: [f32; 3],
    pub pat: Pat,
    /// 0 forked, 1 round, 2 lunate, 3 shark
    pub tail: u8,
    /// 0 none, 1 small, 2 big
    pub teeth: u8,
    pub gape: f32,
    /// struggle strength (1 = normal)
    pub fight: f32,
    /// how much line strength it takes to land it
    pub line: f32,
    pub dors: [f32; 3],
    pub dors2: [f32; 3],
    pub anal: [f32; 3],
    pub pect: f32,
    pub tail_sz: f32,
    pub eye: [f32; 3],
    pub glow_eye: bool,
    pub lure: bool,
    pub sword: bool,
    pub spines: bool,
}

const D: Species = Species {
    name: "",
    kg: (1.0, 2.0),
    weight: 1.0,
    price: 10.0,
    rarity: 0.0,
    deep: false,
    len: 0.5,
    hb: 0.10,
    wb: 0.07,
    shape: 0,
    back: [0.4, 0.4, 0.4],
    belly: [0.8, 0.8, 0.8],
    fin: [0.5, 0.5, 0.5],
    mark: [0.1, 0.1, 0.1],
    pat: Pat::Plain,
    tail: 0,
    teeth: 0,
    gape: 0.35,
    fight: 1.0,
    line: 1.0,
    dors: [0.32, 0.55, 0.075],
    dors2: [0.0, 0.0, 0.0],
    anal: [0.62, 0.78, 0.04],
    pect: 0.085,
    tail_sz: 0.15,
    eye: [0.95, 0.9, 0.7],
    glow_eye: false,
    lure: false,
    sword: false,
    spines: false,
};

pub const BOOT: usize = 6;
pub const N_SPECIES: usize = 14;

pub const SPECIES: [Species; N_SPECIES] = [
    Species { name: "Sardine", kg: (0.1, 0.2), weight: 38.0, price: 14.0, len: 0.20, hb: 0.09, wb: 0.06, back: [0.26, 0.38, 0.52], belly: [0.84, 0.88, 0.92], fin: [0.45, 0.5, 0.55], mark: [0.08, 0.13, 0.2], pat: Pat::Speckle, fight: 0.5, dors: [0.38, 0.52, 0.06], ..D },
    Species { name: "Mackerel", kg: (0.6, 1.2), weight: 25.0, price: 11.0, len: 0.40, hb: 0.075, wb: 0.055, back: [0.12, 0.36, 0.42], belly: [0.86, 0.89, 0.85], fin: [0.2, 0.35, 0.4], mark: [0.04, 0.14, 0.2], pat: Pat::Stripes, fight: 0.8, dors2: [0.58, 0.68, 0.035], ..D },
    Species { name: "Red Snapper", kg: (1.5, 3.0), weight: 15.0, price: 16.0, rarity: 1.0, len: 0.58, hb: 0.15, wb: 0.075, back: [0.82, 0.20, 0.16], belly: [0.96, 0.64, 0.56], fin: [0.78, 0.14, 0.12], mark: [0.55, 0.08, 0.08], pat: Pat::Speckle, fight: 1.0, dors: [0.26, 0.62, 0.09], ..D },
    Species { name: "Grouper", kg: (4.0, 8.0), weight: 8.0, price: 13.0, rarity: 1.6, len: 0.80, hb: 0.19, wb: 0.13, back: [0.46, 0.38, 0.26], belly: [0.74, 0.66, 0.50], fin: [0.38, 0.30, 0.20], mark: [0.26, 0.20, 0.13], pat: Pat::Blotch, tail: 1, teeth: 1, fight: 1.3, line: 1.15, dors: [0.26, 0.66, 0.08], tail_sz: 0.17, ..D },
    Species { name: "Yellowfin Tuna", kg: (10.0, 25.0), weight: 5.0, price: 24.0, rarity: 2.4, len: 1.25, hb: 0.14, wb: 0.115, back: [0.10, 0.18, 0.40], belly: [0.88, 0.89, 0.84], fin: [0.95, 0.82, 0.12], mark: [0.95, 0.82, 0.12], pat: Pat::Plain, tail: 2, fight: 1.7, line: 1.3, dors: [0.34, 0.46, 0.12], dors2: [0.58, 0.70, 0.04], tail_sz: 0.20, ..D },
    Species { name: "Pufferfish", kg: (0.8, 1.3), weight: 6.0, price: 12.0, rarity: 0.5, len: 0.30, hb: 0.22, wb: 0.21, shape: 2, back: [0.76, 0.66, 0.36], belly: [0.96, 0.93, 0.80], fin: [0.82, 0.72, 0.45], mark: [0.30, 0.22, 0.10], pat: Pat::Spots, tail: 1, spines: true, fight: 0.8, dors: [0.50, 0.62, 0.06], pect: 0.12, tail_sz: 0.09, ..D },
    Species { name: "Old Boot", kg: (0.5, 0.5), weight: 3.0, price: 3.0, rarity: -1.0, len: 0.3, fight: 0.4, ..D },
    Species { name: "Barracuda", kg: (5.0, 12.0), weight: 7.0, price: 15.0, rarity: 0.8, len: 1.15, hb: 0.06, wb: 0.05, back: [0.30, 0.38, 0.42], belly: [0.90, 0.92, 0.91], fin: [0.30, 0.34, 0.38], mark: [0.10, 0.12, 0.15], pat: Pat::Bands, teeth: 2, gape: 0.5, fight: 1.5, line: 1.2, dors: [0.50, 0.58, 0.05], dors2: [0.74, 0.82, 0.05], anal: [0.76, 0.84, 0.05], ..D },
    Species { name: "Moonfish", kg: (18.0, 45.0), weight: 3.0, price: 19.0, rarity: 1.8, len: 0.90, hb: 0.36, wb: 0.075, shape: 1, back: [0.52, 0.28, 0.48], belly: [0.80, 0.50, 0.55], fin: [0.92, 0.24, 0.18], mark: [0.95, 0.92, 0.95], pat: Pat::Spots, fight: 1.6, line: 1.4, dors: [0.22, 0.52, 0.17], anal: [0.40, 0.66, 0.12], pect: 0.20, tail_sz: 0.12, ..D },
    Species { name: "Swordfish", kg: (45.0, 110.0), weight: 1.6, price: 28.0, rarity: 2.6, len: 2.5, hb: 0.085, wb: 0.07, back: [0.14, 0.18, 0.34], belly: [0.82, 0.82, 0.84], fin: [0.12, 0.14, 0.22], mark: [0.1, 0.1, 0.2], pat: Pat::Plain, tail: 2, sword: true, fight: 2.3, line: 1.7, dors: [0.30, 0.44, 0.10], tail_sz: 0.17, ..D },
    Species { name: "Grey Reef Shark", kg: (35.0, 90.0), weight: 1.5, price: 26.0, rarity: 2.4, len: 2.1, hb: 0.105, wb: 0.09, back: [0.40, 0.43, 0.47], belly: [0.93, 0.93, 0.91], fin: [0.32, 0.35, 0.38], mark: [0.2, 0.2, 0.22], pat: Pat::Plain, tail: 3, teeth: 2, gape: 0.55, fight: 2.4, line: 1.8, dors: [0.30, 0.44, 0.17], dors2: [0.70, 0.76, 0.05], anal: [0.68, 0.76, 0.05], pect: 0.16, tail_sz: 0.2, eye: [0.05, 0.05, 0.06], ..D },
    Species { name: "Deep Anglerfish", kg: (70.0, 170.0), weight: 2.4, price: 30.0, rarity: 2.0, deep: true, len: 1.5, hb: 0.30, wb: 0.27, shape: 2, back: [0.07, 0.06, 0.08], belly: [0.14, 0.11, 0.13], fin: [0.10, 0.08, 0.10], mark: [0.55, 0.50, 0.38], pat: Pat::Speckle, tail: 1, teeth: 2, gape: 0.9, fight: 2.6, line: 2.0, lure: true, glow_eye: true, eye: [0.5, 1.0, 0.8], dors: [0.40, 0.60, 0.08], tail_sz: 0.14, ..D },
    Species { name: "Abyssal Maw", kg: (220.0, 480.0), weight: 1.5, price: 34.0, rarity: 3.2, deep: true, len: 4.4, hb: 0.16, wb: 0.14, back: [0.035, 0.045, 0.07], belly: [0.50, 0.53, 0.56], fin: [0.06, 0.06, 0.09], mark: [0.3, 0.3, 0.32], pat: Pat::Plain, tail: 0, teeth: 2, gape: 0.9, fight: 3.4, line: 2.6, glow_eye: true, eye: [1.0, 0.15, 0.1], dors: [0.30, 0.50, 0.15], tail_sz: 0.22, ..D },
    Species { name: "Ghost Leviathan", kg: (350.0, 700.0), weight: 0.8, price: 42.0, rarity: 4.0, deep: true, len: 6.8, hb: 0.10, wb: 0.09, back: [0.70, 0.76, 0.78], belly: [0.92, 0.95, 0.96], fin: [0.60, 0.68, 0.72], mark: [0.4, 0.5, 0.55], pat: Pat::Bands, tail: 2, teeth: 2, gape: 0.7, fight: 2.5, line: 3.2, glow_eye: true, eye: [0.4, 1.0, 0.9], dors: [0.32, 0.46, 0.16], tail_sz: 0.2, ..D },
];

pub fn is_monster(i: usize) -> bool {
    SPECIES[i].deep
}

/// Value in coins.
pub fn fish_value(i: usize, kg: f32) -> i32 {
    let sp = &SPECIES[i];
    if i == BOOT { sp.price as i32 } else { (sp.price * kg).round() as i32 }
}

/// What bites: (species, weight in kg). `deep` = the lure landed in deep water.
pub fn pick_fish(luck: f32, deep: bool) -> (usize, f32) {
    let w: Vec<f32> = SPECIES
        .iter()
        .map(|f| {
            let mut x = f.weight * (1.0 + luck * f.rarity).max(0.05);
            if f.deep {
                x = if deep { x * 5.0 } else { 0.0 };
            } else if deep && f.rarity < 1.0 {
                x *= 0.4;
            }
            x
        })
        .collect();
    let total: f32 = w.iter().sum();
    let mut r = rf(0.0, total);
    for (i, f) in SPECIES.iter().enumerate() {
        if r < w[i] {
            let k = rf(0.0, 1.0).powf(1.4);
            return (i, mixf(f.kg.0, f.kg.1, k) * (1.0 + 0.2 * luck.max(0.0)));
        }
        r -= w[i];
    }
    (0, 0.15)
}

/// Visual length (m) of a fish of species `i` weighing `kg`.
pub fn fish_len(i: usize, kg: f32) -> f32 {
    let sp = &SPECIES[i];
    let mid = (sp.kg.0 + sp.kg.1) * 0.5;
    sp.len * (kg / mid.max(0.01)).powf(0.34).clamp(0.7, 1.45)
}

pub fn v3(a: [f32; 3]) -> Vec3 {
    vec3(a[0], a[1], a[2])
}

pub struct FishPose {
    pub heading: f32,
    pub pitch: f32,
    pub roll: f32,
    /// swim phase (radians)
    pub t: f32,
    /// tail-beat strength 0..1.5
    pub amp: f32,
    /// mouth open 0..1 (extra on top of the species gape)
    pub open: f32,
}

fn prof(shape: u8, u: f32) -> (f32, f32) {
    match shape {
        1 => {
            let a = (1.0 - (2.0 * (u * 0.9 + 0.04) - 1.0).powi(2)).max(0.0).sqrt();
            let end = 1.0 - smooth(0.78, 1.0, u) * 0.82;
            (a.powf(0.8) * end.max(0.12), a.powf(0.9) * (1.0 - smooth(0.6, 1.0, u) * 0.6))
        }
        2 => {
            let a = (PI * (0.02 + 0.96 * u)).sin().max(0.0);
            let end = 1.0 - smooth(0.7, 1.0, u) * 0.8;
            (a.powf(0.62) * end, a.powf(0.62) * end)
        }
        3 => {
            let rise = ((u / 0.2).min(1.0) * PI * 0.5).sin().powf(0.7);
            (rise * (1.0 - 0.55 * smooth(0.4, 1.0, u)), rise * (1.0 - 0.55 * smooth(0.4, 1.0, u)))
        }
        _ => {
            let rise = (((0.02 + 0.98 * (u / 0.30).min(1.0)) * PI * 0.5).sin()).powf(0.72);
            (rise * (1.0 - smooth(0.50, 1.0, u) * 0.87), rise * (1.0 - smooth(0.45, 1.0, u) * 0.82))
        }
    }
}

fn hsh(a: i32, b: i32, s: u32) -> f32 {
    hash2(a, b, s)
}

fn pat_color(sp: &Species, u: f32, sy: f32, k: i32, j: i32) -> Vec3 {
    let back = v3(sp.back);
    let belly = v3(sp.belly);
    let mark = v3(sp.mark);
    let mut c = belly.lerp(back, smooth(-0.3, 0.6, sy));
    match sp.pat {
        Pat::Plain => {}
        Pat::Stripes => {
            let s = 0.5 + 0.5 * (u * TAU * 7.5 + sy * 2.4).sin();
            c = c.lerp(mark, smooth(0.55, 0.9, s) * smooth(0.0, 0.5, sy));
        }
        Pat::Bands => {
            let b = (u * 5.5).fract();
            if (0.36..0.58).contains(&b) && sy > -0.35 {
                c = c.lerp(mark, 0.75 * smooth(-0.35, 0.1, sy));
            }
        }
        Pat::Spots => {
            if hsh(k, j / 2, 7) > 0.66 && sy > -0.5 {
                c = c.lerp(mark, 0.85);
            }
        }
        Pat::Speckle => {
            let h = hsh(k, j, 3);
            if h > 0.8 {
                c = c.lerp(mark, 0.5 * (h - 0.8) * 5.0);
            }
        }
        Pat::Blotch => {
            if hsh(k / 2, j / 2, 11) > 0.52 {
                c = c.lerp(mark, 0.6);
            }
        }
    }
    // gill slit, shiny gill cover, lips
    if (0.185..0.215).contains(&u) && sy.abs() < 0.85 {
        c *= 0.55;
    } else if (0.11..0.185).contains(&u) {
        c *= 1.07;
    }
    // scale glint
    c * (0.93 + 0.12 * hsh(k, j, 5))
}

fn fin_tri(mb: &mut MB, a: Vec3, b: Vec3, c: Vec3, col: Vec3) {
    let n = (b - a).cross(c - a).normalize_or_zero();
    let k = shade(col, n);
    let kb = shade(col * 0.85, -n);
    mb.tri(a, b, c, k, k, k);
    mb.tri(a, c, b, kb, kb, kb);
}

/// A ribbed fin: `n` strips between a base edge (b0..b1) and an outer edge (t0..t1).
fn fin_strip(mb: &mut MB, b0: Vec3, b1: Vec3, t0: Vec3, t1: Vec3, n: usize, col: Vec3) {
    for i in 0..n {
        let (f0, f1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
        let (p0, p1) = (b0.lerp(b1, f0), b0.lerp(b1, f1));
        let (q0, q1) = (t0.lerp(t1, f0), t0.lerp(t1, f1));
        let c = col * if i % 2 == 0 { 1.0 } else { 0.8 };
        fin_tri(mb, p0, q0, q1, c);
        fin_tri(mb, p0, q1, p1, c * 0.97);
    }
}

fn draw_boot(mb: &mut MB, pos: Vec3, heading: f32, len: f32, t: f32) {
    let leather = vec3(0.24, 0.17, 0.11);
    let mud = vec3(0.17, 0.15, 0.10);
    let s = len / 0.3;
    let f = vec3(heading.cos(), 0.0, heading.sin());
    let r = vec3(-heading.sin(), 0.0, heading.cos());
    let sway = (t * 2.0).sin() * 0.03 * s;
    let c = pos + r * sway;
    // foot, heel, sole and shaft
    blob(mb, c + f * 0.04 * s, vec3(0.13, 0.055, 0.06) * s, leather, 0.05, 8, 5, heading, shade);
    blob(mb, c - f * 0.07 * s - Vec3::Y * 0.01 * s, vec3(0.07, 0.06, 0.06) * s, leather * 0.9, 0.0, 7, 4, heading, shade);
    cyl(mb, c - f * 0.07 * s, c - f * 0.07 * s + Vec3::Y * 0.17 * s, 0.062 * s, 0.07 * s, leather * 1.05, 10);
    cyl(mb, c - f * 0.07 * s + Vec3::Y * 0.17 * s, c - f * 0.07 * s + Vec3::Y * 0.185 * s, 0.075 * s, 0.075 * s, mud, 10);
    cyl(mb, c - f * 0.17 * s - Vec3::Y * 0.045 * s, c + f * 0.17 * s - Vec3::Y * 0.045 * s, 0.045 * s, 0.045 * s, mud * 0.8, 6);
    // dripping weed
    for k in 0..3 {
        let a = c - f * 0.05 * s + r * (k as f32 - 1.0) * 0.04 * s + Vec3::Y * 0.12 * s;
        cyl(mb, a, a - Vec3::Y * (0.12 + 0.03 * k as f32) * s + r * (t * 3.0 + k as f32).sin() * 0.01, 0.006 * s, 0.003 * s, vec3(0.12, 0.30, 0.14), 4);
    }
}

pub fn draw_fish(mb: &mut MB, idx: usize, pos: Vec3, len: f32, p: &FishPose, high: bool) {
    if idx == BOOT {
        draw_boot(mb, pos, p.heading, len, p.t);
        return;
    }
    let sp = &SPECIES[idx];
    let f0 = vec3(p.heading.cos(), 0.0, p.heading.sin());
    let r0 = vec3(-p.heading.sin(), 0.0, p.heading.cos());
    let (sp_, cp) = p.pitch.sin_cos();
    let fwd = f0 * cp + Vec3::Y * sp_;
    let up0 = Vec3::Y * cp - f0 * sp_;
    let (sr, cr) = p.roll.sin_cos();
    let right = r0 * cr + up0 * sr;
    let up = up0 * cr - r0 * sr;

    let n = if high { 15 } else { 7 };
    let m = if high { 12 } else { 6 };
    let wig = if sp.shape == 3 { 1.6 } else { 1.0 };
    let off = |u: f32| p.amp * len * 0.10 * wig * (p.t - u * 4.4).sin() * smooth(0.0, 1.0, u).powf(1.25);
    let centre = |u: f32| pos + fwd * (len * (0.5 - u)) + right * off(u);
    let hh_of = |u: f32| (sp.hb * len * prof(sp.shape, u).0).max(0.0005);
    let ww_of = |u: f32| (sp.wb * len * prof(sp.shape, u).1).max(0.0005);

    // body
    let mut rings: Vec<Vec<(Vec3, Lit)>> = Vec::with_capacity(n);
    for k in 0..n {
        let u = k as f32 / (n - 1) as f32;
        let c = centre(u);
        let (hh, ww) = (hh_of(u), ww_of(u));
        let mut row = Vec::with_capacity(m);
        for j in 0..m {
            let th = TAU * j as f32 / m as f32;
            let (sy, cx) = th.sin_cos();
            let flat = if sy < 0.0 { 0.86 } else { 1.0 };
            let pt = c + right * (ww * cx) + up * (hh * sy * flat);
            let nrm = (right * (cx * hh) + up * (sy * ww)).normalize_or_zero();
            let col = pat_color(sp, u, sy, k as i32, j as i32);
            row.push((pt, shade(col, nrm)));
        }
        rings.push(row);
    }
    for k in 0..n - 1 {
        for j in 0..m {
            let j2 = (j + 1) % m;
            let (a, b, c, d) = (rings[k][j], rings[k + 1][j], rings[k + 1][j2], rings[k][j2]);
            mb.quadc(a.0, b.0, c.0, d.0, a.1, b.1, c.1, d.1);
        }
    }
    // tail-root cap
    {
        let c = centre(1.0);
        let l = shade(v3(sp.back) * 0.9, -fwd);
        for j in 0..m {
            mb.tri(c, rings[n - 1][j].0, rings[n - 1][(j + 1) % m].0, l, l, l);
        }
    }

    let finc = v3(sp.fin);
    let top = |u: f32, k: f32| centre(u) + up * (hh_of(u) * k);
    // dorsal fins
    for d in [sp.dors, sp.dors2] {
        if d[2] > 0.0 {
            let (u0, u1, h) = (d[0], d[1], d[2] * len);
            let ua = u0 + (u1 - u0) * 0.22;
            let sway = right * (p.amp * 0.01 * len * (p.t * 1.3).sin());
            fin_strip(mb, top(u0, 0.97), top(u1, 0.97), top(ua, 1.0) + up * h + sway - fwd * (h * 0.05), top(u1, 1.0) + up * (h * 0.30) + sway, if high { 6 } else { 2 }, finc);
        }
    }
    // anal fin
    if sp.anal[2] > 0.0 {
        let (u0, u1, h) = (sp.anal[0], sp.anal[1], sp.anal[2] * len);
        let bot = |u: f32| centre(u) - up * (hh_of(u) * 0.84);
        fin_strip(mb, bot(u0), bot(u1), bot(u0 + 0.02) - up * h, bot(u1) - up * (h * 0.4), if high { 5 } else { 2 }, finc * 0.92);
    }
    // pectoral and pelvic fins
    let flap = (p.t * 1.7).sin() * 0.25 * (0.4 + p.amp);
    for s in [-1.0f32, 1.0] {
        let pu = 0.235;
        let base = centre(pu) + right * (s * ww_of(pu) * 0.94) - up * (hh_of(pu) * 0.28);
        let l = sp.pect * len;
        let dir = (-fwd * 0.75 + right * (s * (0.5 + flap)) - up * (0.28 - flap * 0.5)).normalize_or_zero();
        let tip = base + dir * l;
        let base2 = base - fwd * (l * 0.34);
        fin_strip(mb, base, base2, tip - fwd * (l * 0.05), tip - fwd * (l * 0.5) - up * (l * 0.1), if high { 4 } else { 2 }, finc * 1.02);
        if high {
            let pel = centre(0.40) + right * (s * ww_of(0.40) * 0.45) - up * (hh_of(0.40) * 0.80);
            let dl = l * 0.6;
            let tip2 = pel - fwd * (dl * 0.8) + right * (s * dl * 0.3) - up * (dl * 0.55);
            fin_strip(mb, pel, pel - fwd * (dl * 0.4), tip2 + fwd * (dl * 0.1), tip2 - fwd * (dl * 0.3), 2, finc * 0.9);
        }
    }
    // tail
    {
        let root = centre(1.0);
        let swing = p.amp * 0.55 * (p.t - 4.9).sin();
        let tb = (-fwd * swing.cos() + right * swing.sin()).normalize_or_zero();
        let span = sp.tail_sz * len;
        let lt = span * 1.05;
        let rh = hh_of(1.0).max(span * 0.06);
        let rt = root + up * rh;
        let rb = root - up * rh;
        let rays = if high { 3 } else { 1 };
        match sp.tail {
            1 => {
                // round fan
                let tip = root + tb * lt;
                let steps = 6;
                let mut prev = rt;
                for i in 1..=steps {
                    let a = PI * (0.5 - i as f32 / steps as f32);
                    let q = root + tb * (lt * a.cos()) + up * (span * 0.55 * a.sin());
                    fin_tri(mb, rt.lerp(rb, (i - 1) as f32 / steps as f32), prev, q, finc * if i % 2 == 0 { 1.0 } else { 0.82 });
                    fin_tri(mb, rt.lerp(rb, (i - 1) as f32 / steps as f32), q, rt.lerp(rb, i as f32 / steps as f32), finc * 0.95);
                    prev = q;
                }
                let _ = tip;
            }
            3 => {
                // shark: long upper lobe, short lower lobe
                let ttop = root + tb * (lt * 1.25) + up * (span * 0.78);
                let tbot = root + tb * (lt * 0.7) - up * (span * 0.34);
                let notch = root + tb * (lt * 0.75) + up * (span * 0.1);
                fin_strip(mb, rt, root, ttop, notch, rays, finc);
                fin_strip(mb, rb, root, tbot, notch, rays, finc * 0.95);
            }
            2 => {
                // lunate crescent
                let ttop = root + tb * (lt * 1.35) + up * (span * 0.62);
                let tbot = root + tb * (lt * 1.35) - up * (span * 0.62);
                let notch = root + tb * (lt * 0.62);
                fin_strip(mb, rt, root, ttop, notch, rays, finc);
                fin_strip(mb, rb, root, tbot, notch, rays, finc * 0.95);
            }
            _ => {
                // forked
                let ttop = root + tb * (lt * 1.1) + up * (span * 0.55);
                let tbot = root + tb * (lt * 1.1) - up * (span * 0.55);
                let notch = root + tb * (lt * 0.55);
                fin_strip(mb, rt, root, ttop, notch, rays, finc);
                fin_strip(mb, rb, root, tbot, notch, rays, finc * 0.95);
            }
        }
    }

    // head details
    let nose = centre(0.0);
    let eye_u = if sp.shape == 2 { 0.16 } else { 0.115 };
    let er = (len * if sp.shape == 2 { 0.03 } else { 0.019 }).max(0.007);
    for s in [-1.0f32, 1.0] {
        let ec = centre(eye_u) + right * (s * ww_of(eye_u) * 0.93) + up * (hh_of(eye_u) * 0.28);
        if sp.glow_eye {
            blob(mb, ec, Vec3::splat(er * 1.3), v3(sp.eye), 0.0, 7, 5, 0.0, |c, _| glow(c));
        } else {
            blob(mb, ec, Vec3::splat(er), v3(sp.eye), 0.0, 7, 5, 0.0, shade);
            blob(mb, ec + right * (s * er * 0.55) + fwd * (er * 0.1), Vec3::splat(er * 0.62), vec3(0.02, 0.02, 0.03), 0.0, 6, 4, 0.0, shade);
        }
    }
    // mouth slit and teeth
    if sp.teeth > 0 || sp.shape != 1 {
        let gape = (sp.gape + p.open * 0.5) * if sp.teeth == 0 { 0.35 } else { 1.0 };
        let (ub, lb) = (0.0f32, 0.17f32);
        let ln = (n as f32 * 0.5) as usize;
        let _ = ln;
        for s in [-1.0f32, 1.0] {
            let lip_up = |u: f32| centre(u) + right * (s * ww_of(u) * 1.0) - up * (hh_of(u) * 0.10);
            let lip_dn = |u: f32| centre(u) + right * (s * ww_of(u) * 0.99) - up * (hh_of(u) * (0.10 + gape * 0.55));
            let steps = 4;
            for i in 0..steps {
                let (u0, u1) = (ub + (lb - ub) * i as f32 / steps as f32, ub + (lb - ub) * (i + 1) as f32 / steps as f32);
                let k = shade(vec3(0.20, 0.03, 0.05), right * s);
                mb.quadc(lip_up(u0), lip_up(u1), lip_dn(u1), lip_dn(u0), k, k, k, k);
            }
            if sp.teeth > 0 {
                let cnt = if sp.teeth == 2 { 7 } else { 4 };
                for i in 0..cnt {
                    let u = 0.012 + (lb - 0.025) * i as f32 / (cnt - 1) as f32;
                    let tl = len * if sp.teeth == 2 { 0.034 } else { 0.014 } * (1.0 - 0.35 * u / lb);
                    let tw = len * 0.006;
                    let w = vec3(0.92, 0.92, 0.84);
                    let a = lip_up(u);
                    fin_tri(mb, a - fwd * tw, a + fwd * tw, lip_up(u) - up * tl + right * (s * 0.002), w);
                    let b = lip_dn(u + 0.008);
                    fin_tri(mb, b - fwd * tw, b + fwd * tw, b + up * tl + right * (s * 0.002), w * 0.96);
                }
            }
        }
    }
    if sp.sword {
        let bl = len * 0.30;
        cyl(mb, nose, nose + fwd * bl, hh_of(0.03) * 0.35, 0.002, v3(sp.back) * 1.1, 6);
    }
    if sp.lure {
        let p0 = top(0.13, 1.0);
        let p1 = p0 + up * (len * 0.20) + fwd * (len * 0.05);
        let bulb = nose + fwd * (len * 0.20) + up * (len * 0.20 + (p.t * 1.3).sin() * len * 0.01);
        let mid = p1.lerp(bulb, 0.5) + up * (len * 0.04);
        cyl(mb, p0, p1, len * 0.007, len * 0.005, vec3(0.1, 0.08, 0.1), 4);
        cyl(mb, p1, mid, len * 0.005, len * 0.004, vec3(0.1, 0.08, 0.1), 4);
        cyl(mb, mid, bulb, len * 0.004, len * 0.003, vec3(0.1, 0.08, 0.1), 4);
        blob(mb, bulb, Vec3::splat(len * 0.032), vec3(0.55, 1.0, 0.85), 0.0, 8, 5, 0.0, |c, _| glow(c));
        blob(mb, bulb, Vec3::splat(len * 0.05), vec3(0.18, 0.5, 0.42), 0.0, 8, 5, 0.0, |c, _| glow(c * 0.7));
    }
    if sp.spines {
        let spc = v3(sp.belly) * 0.95;
        for q in 0..if high { 34 } else { 14 } {
            let u = 0.1 + 0.7 * hsh(q, 1, 9);
            let th = TAU * hsh(q, 2, 9);
            let (sy, cx) = th.sin_cos();
            let c = centre(u);
            let (hh, ww) = (hh_of(u), ww_of(u));
            let a = c + right * (ww * cx) + up * (hh * sy);
            let nrm = (right * (cx * hh) + up * (sy * ww)).normalize_or_zero();
            cyl(mb, a, a + nrm * (len * 0.06), len * 0.006, 0.0006, spc, 4);
        }
    }
}
