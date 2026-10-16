//! Items, fishing gear models and held-item geometry.

use super::*;

// ---------------------------------------------------------------------------
// Fishing data
// ---------------------------------------------------------------------------
pub const N_RODS: usize = 5;
pub const N_BAITS: usize = 7;
pub const N_UPG: usize = 4;

/// (name, cast range x, reel speed x, luck, bite-time x, price, blank colour, wrap colour, line strength)
pub const RODS: [(&str, f32, f32, f32, f32, i32, [f32; 3], [f32; 3], f32); N_RODS] = [
    ("Starter Rod", 1.0, 1.0, 0.0, 1.0, 0, [0.11, 0.33, 0.33], [0.85, 0.15, 0.12], 1.0),
    ("Fiberglass Rod", 1.25, 1.35, 0.25, 0.85, 120, [0.15, 0.40, 0.18], [0.95, 0.85, 0.25], 1.8),
    ("Carbon Pro Rod", 1.5, 1.8, 0.55, 0.7, 400, [0.06, 0.06, 0.09], [0.95, 0.30, 0.10], 3.5),
    ("Titan Rod", 1.8, 2.3, 0.9, 0.6, 1500, [0.50, 0.53, 0.60], [0.15, 0.80, 0.95], 6.5),
    ("Deep Sea Rod", 2.1, 2.8, 1.3, 0.5, 3800, [0.08, 0.10, 0.22], [0.85, 0.12, 0.70], 11.0),
];

/// (name, luck, bite-time x, pack price, pack size)
pub const BAITS: [(&str, f32, f32, i32, u32); N_BAITS] = [
    ("Worm", 0.10, 0.95, 10, 10),
    ("Shrimp", 0.30, 0.75, 60, 10),
    ("Squid Strip", 0.50, 0.65, 110, 8),
    ("Lucky Lure", 0.65, 0.55, 150, 5),
    ("Abyss Glow Lure", 1.10, 0.50, 420, 3),
    ("Silver Spoon", 1.40, 0.48, 520, 6),
    ("Drowned Lure", 2.20, 0.35, 3000, 2),
];

/// Gear upgrades: (name, description, price)
pub const UPGRADES: [(&str, &str, i32); N_UPG] = [
    ("Braided Line", "Stronger line: +25% before it snaps", 180),
    ("Steel Leader", "Heavy-duty leader: +60% line strength", 650),
    ("Pro Reel", "Smooth drag: +25% reeling power", 420),
    ("Master Reel", "Machined gears: another +25% reeling power", 2400),
];

/// Everything about one fish fight. Drive it with `fight_step`.
#[derive(Clone, Copy)]
pub struct Fight {
    /// 0 = the fish is at the far end of the line, 1 = landed
    pub prog: f32,
    pub tension: f32,
    pub stamina: f32,
    pub fish: usize,
    pub kg: f32,
    pub t: f32,
    /// how hard the fish is thrashing right now (0..1)
    pub thrash: f32,
    pub thr_left: f32,
    pub next: f32,
    pub overload: f32,
    pub target: Vec3,
    pub q: f32,
    pub dir: f32,
}

impl Fight {
    pub fn new(fish: usize, kg: f32, target: Vec3, q: f32) -> Fight {
        Fight { prog: 0.3, tension: 0.15, stamina: 1.0, fish, kg, t: 0.0, thrash: 0.0, thr_left: 0.0, next: rf(0.8, 1.6), overload: 0.0, target, q, dir: 1.0 }
    }
    /// how heavy it feels: grows with the square root of its weight
    pub fn heavy(&self) -> f32 {
        self.kg.max(0.05).sqrt()
    }
}

pub struct FightIn {
    /// how hard you are reeling right now (mouse pulled back), ~0..4 per second
    pub pull: f32,
    /// the rod and reel's reeling power
    pub reel: f32,
    /// how much force the line takes before it snaps
    pub cap: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FightOut {
    Going,
    Landed,
    Snapped,
    Escaped,
}

/// One step of the fight. Heavier fish are harder to pull in, pull back harder and thrash harder;
/// reeling during a thrash makes the line tension spike (too much for too long snaps it).
pub fn fight_step(f: &mut Fight, inp: &FightIn, dt: f32) -> FightOut {
    let sp = &crate::fish::SPECIES[f.fish];
    let heavy = f.heavy();
    let force = sp.fight * heavy;
    let ratio = force / inp.cap.max(0.1);
    f.t += dt;
    // thrash schedule
    if f.thr_left > 0.0 {
        f.thr_left -= dt;
        if f.thr_left <= 0.0 {
            f.thrash = 0.0;
            f.next = rf(1.0, 2.6) / (0.7 + 0.3 * sp.fight) * (0.7 + 0.6 * f.stamina);
        }
    } else {
        f.next -= dt;
        if f.next <= 0.0 {
            f.thr_left = rf(0.55, 1.15);
            f.thrash = rf(0.65, 1.0) * (0.4 + 0.6 * f.stamina);
            f.dir = if rf(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
        }
    }
    let pull = inp.pull.max(0.0);
    // line tension
    let base = 0.10 + 0.10 * ratio.min(1.5);
    let pull_load = pull * 0.16 * (0.55 + ratio * 0.9);
    let thr_load = f.thrash * (0.30 + 0.55 * ratio.min(2.5));
    let target_t = (base + pull_load + thr_load).min(1.7);
    f.tension += (target_t - f.tension) * (dt * 7.0).min(1.0);
    if f.tension > 1.0 {
        f.overload += dt * (f.tension - 0.9) * 4.0;
    } else {
        f.overload = (f.overload - dt * 1.5).max(0.0);
    }
    if f.overload > 0.35 {
        return FightOut::Snapped;
    }
    // the fish tires when it is kept under steady tension
    if f.tension > 0.3 && pull > 0.4 {
        f.stamina = (f.stamina - dt * 0.17 / (1.0 + 0.25 * heavy)).max(0.0);
    } else if f.tension < 0.2 {
        f.stamina = (f.stamina + dt * 0.015).min(1.0);
    }
    // line gained / lost: bigger fish take much more effort per pull
    let eff = inp.reel / (1.0 + heavy * 0.20 * (1.0 - 0.45 * (1.0 - f.stamina)));
    let gain = pull * 0.22 * eff * (1.0 + 0.3 * f.q) * dt;
    let drag = (0.045 + 0.0010 * force.min(120.0)) * (0.30 + 0.70 * f.stamina) * (1.0 + 1.1 * f.thrash) * dt;
    f.prog = (f.prog + gain - drag).min(1.0);
    if f.prog >= 1.0 {
        FightOut::Landed
    } else if f.prog <= 0.0 {
        FightOut::Escaped
    } else {
        FightOut::Going
    }
}

#[derive(Clone, Copy)]
pub enum FS {
    Idle,
    Charging { t: f32 },
    Flying { u: f32, target: Vec3, q: f32 },
    Waiting { t: f32, bite_at: f32, target: Vec3, q: f32 },
    Bite { t: f32, target: Vec3, q: f32 },
    Reeling(Fight),
    /// the catch is in your hands: press a free hotbar number to keep it, or release it
    Holding { t: f32, fish: usize, kg: f32 },
}

impl FS {
    pub fn code(&self) -> u8 {
        match self {
            FS::Idle | FS::Holding { .. } => 0,
            FS::Charging { .. } => 1,
            FS::Flying { .. } | FS::Waiting { .. } => 2,
            FS::Bite { .. } => 3,
            FS::Reeling(_) => 4,
        }
    }
}

pub fn triangle(t: f32) -> f32 {
    let ph = (t / 1.9).fract();
    if ph < 0.5 { ph * 2.0 } else { 2.0 - ph * 2.0 }
}


// ---------------------------------------------------------------------------
// Fishing rod geometry (first person)
// ---------------------------------------------------------------------------
pub fn rod_pt(base: Vec3, dir: Vec3, s: f32, bend: f32) -> Vec3 {
    base + dir * s - Vec3::Y * (bend * s * s)
}

/// A thin line that stays visible at a distance.
pub fn thin(mb: &mut MB, a: Vec3, b: Vec3, eye: Vec3, col: Vec3) {
    let d = ((a + b) * 0.5).distance(eye);
    let r = (0.0016f32).max(d * 0.0004);
    cyl(mb, a, b, r, r, col, 3);
}

pub fn line_between(mb: &mut MB, a: Vec3, b: Vec3, sag: f32, eye: Vec3) {
    let segs = 10;
    let mut prev = a;
    for i in 1..=segs {
        let u = i as f32 / segs as f32;
        let p = a.lerp(b, u) - Vec3::Y * sag * 4.0 * u * (1.0 - u);
        thin(mb, prev, p, eye, vec3(0.92, 0.92, 0.85));
        prev = p;
    }
}

pub fn ring(mb: &mut MB, c: Vec3, axis: Vec3, rad: f32, thick: f32, col: Vec3, n: usize) {
    let u = axis.cross(if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X }).normalize_or_zero();
    let v = axis.cross(u);
    let mut prev = c + u * rad;
    for i in 1..=n {
        let a = TAU * i as f32 / n as f32;
        let p = c + (u * a.cos() + v * a.sin()) * rad;
        cyl(mb, prev, p, thick, thick, col, 3);
        prev = p;
    }
}

pub fn draw_lure(mb: &mut MB, p: Vec3) {
    let gold = vec3(0.90, 0.68, 0.18);
    let steel = vec3(0.60, 0.60, 0.63);
    cyl(mb, p + Vec3::Y * 0.055, p + Vec3::Y * 0.035, 0.004, 0.004, steel, 4);
    ring(mb, p + Vec3::Y * 0.06, Vec3::X, 0.007, 0.0013, steel, 6);
    blob(mb, p, vec3(0.011, 0.034, 0.009), gold, 0.0, 7, 5, 0.0, shade);
    blob(mb, p + Vec3::Y * 0.01, vec3(0.012, 0.012, 0.008), vec3(0.8, 0.12, 0.1), 0.0, 5, 3, 0.0, shade);
    for a in [0.0f32, 2.1, 4.2] {
        let d = vec3(a.cos(), 0.0, a.sin()) * 0.013;
        cyl(mb, p - Vec3::Y * 0.03, p - Vec3::Y * 0.056 + d, 0.0016, 0.0012, steel, 3);
        cyl(mb, p - Vec3::Y * 0.056 + d, p - Vec3::Y * 0.046 + d * 0.3, 0.0012, 0.0006, steel, 3);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_rod(mb: &mut MB, base: Vec3, dir: Vec3, down: Vec3, right: Vec3, bend: f32, eye: Vec3, tier: usize, crank: f32, lure_pos: Option<Vec3>, bobber: Option<Vec3>, time: f32) {
    let (blank, wrap) = (RODS[tier].6, RODS[tier].7);
    let blank = vec3(blank[0], blank[1], blank[2]);
    let wrapc = vec3(wrap[0], wrap[1], wrap[2]);
    let black = vec3(0.035, 0.035, 0.04);
    let steel = vec3(0.62, 0.64, 0.68);
    let grip_a = vec3(0.07, 0.07, 0.08);
    let grip_b = vec3(0.14, 0.14, 0.16);
    let fwd = dir;

    // foam grip with ridges, butt cap and fore grip
    let mut s = -0.06f32;
    let mut k = 0;
    while s < 0.30 {
        let s1 = s + 0.045;
        let rad = if k % 2 == 0 { 0.0235 } else { 0.0215 };
        cyl(mb, rod_pt(base, dir, s, bend), rod_pt(base, dir, s1, bend), rad, rad, if k % 2 == 0 { grip_a } else { grip_b }, 10);
        s = s1;
        k += 1;
    }
    blob(mb, rod_pt(base, dir, -0.07, bend), vec3(0.026, 0.026, 0.026), black, 0.0, 8, 4, 0.0, shade);
    // reel seat rings
    cyl(mb, rod_pt(base, dir, 0.17, bend), rod_pt(base, dir, 0.20, bend), 0.027, 0.027, steel, 10);
    cyl(mb, rod_pt(base, dir, 0.30, bend), rod_pt(base, dir, 0.33, bend), 0.025, 0.025, steel, 10);

    // tapered blank in short pieces so it can flex
    let rad_at = |x: f32| mixf(0.0175, 0.0042, ((x - 0.32) / 2.08).clamp(0.0, 1.0));
    let mut s = 0.32f32;
    while s < 2.4 {
        let s1 = (s + 0.16f32).min(2.4);
        let mid = (s + s1) * 0.5;
        let col = if (1.30..1.38).contains(&mid) { black } else { blank * (0.9 + 0.2 * ((mid * 6.0).sin() * 0.5 + 0.5)) };
        cyl(mb, rod_pt(base, dir, s, bend), rod_pt(base, dir, s1, bend), rad_at(s), rad_at(s1), col, 10);
        s = s1;
    }
    // guides: wrapped feet + ring
    let guides = [0.62f32, 0.98, 1.32, 1.64, 1.94, 2.2, 2.4];
    let mut prev = rod_pt(base, dir, 0.34, bend) + down * 0.075;
    // reel
    let rp = rod_pt(base, dir, 0.245, bend);
    let rc = rp + down * 0.085;
    cyl(mb, rp, rc, 0.011, 0.011, steel, 6);
    cyl(mb, rc - right * 0.026, rc + right * 0.026, 0.047, 0.047, vec3(0.10, 0.10, 0.12), 14); // body
    cyl(mb, rc + right * 0.026, rc + right * 0.036, 0.034, 0.034, vec3(0.8, 0.65, 0.2), 14); // drag knob
    let spool_c = rc + fwd * 0.045 - right * 0.0;
    cyl(mb, spool_c - right * 0.022, spool_c + right * 0.022, 0.033, 0.033, steel, 12); // spool
    cyl(mb, spool_c - right * 0.016, spool_c + right * 0.016, 0.031, 0.031, vec3(0.88, 0.88, 0.82), 12); // line on spool
    cyl(mb, spool_c - right * 0.028, spool_c - right * 0.022, 0.036, 0.036, vec3(0.2, 0.2, 0.22), 12);
    // bail arm
    let bail_a = spool_c - right * 0.04;
    let bail_b = spool_c + right * 0.04 + fwd * 0.02;
    cyl(mb, bail_a, bail_a + fwd * 0.05 + Vec3::Y * 0.03, 0.0035, 0.0035, steel, 4);
    cyl(mb, bail_b, bail_b + fwd * 0.05 + Vec3::Y * 0.03, 0.0035, 0.0035, steel, 4);
    cyl(mb, bail_a + fwd * 0.05 + Vec3::Y * 0.03, bail_b + fwd * 0.05 + Vec3::Y * 0.03, 0.003, 0.003, steel, 4);
    // crank handle that spins as you reel
    let ca = crank;
    let crank_end = rc + right * 0.05 + (down * ca.cos() + fwd * ca.sin()) * 0.065;
    cyl(mb, rc + right * 0.036, rc + right * 0.05, 0.007, 0.007, steel, 5);
    cyl(mb, rc + right * 0.05, crank_end + right * 0.0, 0.006, 0.006, steel, 5);
    blob(mb, crank_end + right * 0.012, vec3(0.012, 0.012, 0.012), vec3(0.08, 0.08, 0.09), 0.0, 6, 3, 0.0, shade);

    for (gi, &g) in guides.iter().enumerate() {
        let p = rod_pt(base, dir, g, bend);
        let drop = if gi == guides.len() - 1 { 0.0 } else { 0.034 - gi as f32 * 0.0018 };
        // thread wrap
        cyl(mb, p - dir * 0.025, p + dir * 0.025, rad_at(g) * 1.18, rad_at(g) * 1.18, wrapc, 8);
        if gi == guides.len() - 1 {
            ring(mb, p + dir * 0.01, right, 0.008, 0.0016, steel, 8);
            prev = p + down * 0.0;
            continue;
        }
        let foot = p + down * drop;
        cyl(mb, p, foot, 0.0028, 0.0028, steel, 4);
        let ringc = foot + down * 0.004;
        ring(mb, ringc, right, 0.0095 - gi as f32 * 0.0004, 0.0014, steel, 8);
        thin(mb, prev, ringc + down * 0.008, eye, vec3(0.92, 0.92, 0.86));
        prev = ringc + down * 0.008;
    }
    let tip = rod_pt(base, dir, 2.4, bend);
    thin(mb, prev, tip, eye, vec3(0.92, 0.92, 0.86));
    match lure_pos {
        None => {
            let sway = (time * 1.7).sin() * 0.04;
            let lp = tip + vec3(sway, -0.50, 0.0);
            thin(mb, tip, lp + Vec3::Y * 0.06, eye, vec3(0.92, 0.92, 0.86));
            draw_lure(mb, lp);
        }
        Some(lp) => {
            line_between(mb, tip, lp, 0.35 + tip.distance(lp) * 0.02, eye);
            if let Some(bp) = bobber {
                blob(mb, bp + Vec3::Y * 0.03, Vec3::splat(0.045), vec3(0.9, 0.12, 0.10), 0.0, 8, 5, 0.0, shade);
                blob(mb, bp - Vec3::Y * 0.02, Vec3::splat(0.04), vec3(0.95, 0.95, 0.95), 0.0, 8, 4, 0.0, shade);
                thin(mb, bp, bp - Vec3::Y * 0.9, eye, vec3(0.85, 0.85, 0.8));
                draw_lure(mb, bp - Vec3::Y * 0.95);
            }
        }
    }
}


/// What can sit in a hotbar slot. The numeric value is also what is sent over the network.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemKind {
    Rod = 0,
    Coconut = 1,
    Lantern = 2,
    Bait = 3,
    /// no longer sold (fish are carried in the hotbar now); only kept so old saves still load
    Bucket = 4,
    /// a caught fish; which one is stored next to the hotbar slot
    Fish = 5,
    /// shoots a flare that lights the sky and keeps the hunters away (ammo is counted separately)
    FlareGun = 6,
}

impl ItemKind {
    pub fn from_u8(v: u8) -> Option<ItemKind> {
        match v {
            0 => Some(ItemKind::Rod),
            1 => Some(ItemKind::Coconut),
            2 => Some(ItemKind::Lantern),
            3 => Some(ItemKind::Bait),
            4 => Some(ItemKind::Bucket),
            5 => Some(ItemKind::Fish),
            6 => Some(ItemKind::FlareGun),
            _ => None,
        }
    }
}

#[cfg(test)]
mod fight_tests {
    use super::*;

    /// A steady reeler who eases off while the fish thrashes. Returns the outcome and the time taken.
    fn play(fish: usize, kg: f32, rod: usize, line_mult: f32, reel_mult: f32, patient: bool) -> (FightOut, f32) {
        rand::srand(1234 + fish as u64 * 7 + kg as u64);
        let mut f = Fight::new(fish, kg, Vec3::ZERO, 0.8);
        let cap = 10.0 * RODS[rod].8 * line_mult;
        let reel = RODS[rod].2 * reel_mult;
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        loop {
            let base_pull = if f.stamina < 0.5 { 1.9 } else { 1.3 };
            let pull = if patient && f.thrash > 0.2 { 0.0 } else if patient && f.tension > 0.78 { 0.5 } else { base_pull };
            let out = fight_step(&mut f, &FightIn { pull, reel, cap }, dt);
            t += dt;
            if out != FightOut::Going {
                return (out, t);
            }
            if t > 400.0 {
                return (FightOut::Escaped, t);
            }
        }
    }

    #[test]
    fn calibration_table() {
        let _g = crate::RAND_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let rods = [(0usize, 1.0f32, 1.0f32), (1, 1.0, 1.0), (2, 1.25, 1.0), (3, 2.0, 1.25)];
        for (fish, kg) in [(0usize, 0.15f32), (1, 0.9), (2, 2.2), (3, 6.0), (4, 18.0), (7, 8.0), (8, 30.0), (9, 70.0), (10, 60.0), (11, 110.0), (12, 350.0), (13, 520.0)] {
            let mut row = format!("{:16} {:6.1}kg ", crate::fish::SPECIES[fish].name, kg);
            for (rod, lm, rm) in rods {
                let (mut landed, mut snapped, mut esc, mut tsum) = (0, 0, 0, 0.0f32);
                for s in 0..12u64 {
                    rand::srand(77 + s * 13 + fish as u64);
                    let (o, t) = play(fish, kg, rod, lm, rm, true);
                    match o {
                        FightOut::Landed => {
                            landed += 1;
                            tsum += t;
                        }
                        FightOut::Snapped => snapped += 1,
                        _ => esc += 1,
                    }
                }
                row += &format!(" | rod{} L{:2} S{:2} E{:2} {:4.0}s", rod, landed, snapped, esc, if landed > 0 { tsum / landed as f32 } else { 0.0 });
            }
            println!("{row}");
        }
    }

    #[test]
    fn fights_are_fair_and_scale_with_size() {
        let _g = crate::RAND_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // small fish are easy with the starter rod
        for (fish, kg) in [(0usize, 0.15f32), (1, 0.9), (2, 2.2)] {
            let (o, t) = play(fish, kg, 0, 1.0, 1.0, true);
            assert_eq!(o, FightOut::Landed, "fish {fish} should be easy, took {t}");
            assert!(t < 30.0, "fish {fish} took {t}s");
        }
        // bigger fish take longer
        let (_, t_small) = play(1, 0.9, 0, 1.0, 1.0, true);
        let (_, t_big) = play(8, 30.0, 0, 1.0, 1.0, true);
        assert!(t_big > t_small * 1.5, "{t_big} vs {t_small}");
        // a deep-water monster beats everything short of the best gear...
        for rod in 0..3 {
            let (o, _) = play(12, 350.0, rod, 1.0, 1.0, true);
            assert_ne!(o, FightOut::Landed, "rod {rod}");
        }
        // ...but the best rod with every upgrade lands it, after a real fight
        let (o, t) = play(12, 350.0, 3, 2.0, 1.25, true);
        assert_eq!(o, FightOut::Landed, "took {t}");
        assert!((15.0..150.0).contains(&t), "took {t}");
        // yanking the reel through every thrash snaps the line; easing off lands the fish
        let (mut snaps, mut landed_patient) = (0, 0);
        for s in 0..12u64 {
            rand::srand(900 + s);
            if play(8, 30.0, 0, 1.0, 1.0, false).0 == FightOut::Snapped {
                snaps += 1;
            }
            rand::srand(900 + s);
            if play(8, 30.0, 0, 1.0, 1.0, true).0 == FightOut::Landed {
                landed_patient += 1;
            }
        }
        assert!(snaps >= 8, "only {snaps} snaps");
        assert!(landed_patient >= 10, "only {landed_patient} landed");
    }
}

// ---------------------------------------------------------------------------
// Small mesh helpers shared by the item models
// ---------------------------------------------------------------------------
fn perp(axis: Vec3) -> (Vec3, Vec3) {
    let refv = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let u = axis.cross(refv).normalize_or_zero();
    (u, axis.cross(u))
}

/// Open tube with a fixed side count (never scaled by the detail setting) and a per-vertex
/// lighting callback `f(t along axis, normal, side index)`.
fn tube(mb: &mut MB, a: Vec3, b: Vec3, r0: f32, r1: f32, sides: usize, f: &dyn Fn(f32, Vec3, usize) -> Lit) {
    let axis = (b - a).normalize_or_zero();
    if axis == Vec3::ZERO {
        return;
    }
    let (u, v) = perp(axis);
    for s in 0..sides {
        let a0 = TAU * s as f32 / sides as f32;
        let a1 = TAU * (s + 1) as f32 / sides as f32;
        let n0 = u * a0.cos() + v * a0.sin();
        let n1 = u * a1.cos() + v * a1.sin();
        let (l00, l01, l10, l11) = (f(0.0, n0, s), f(0.0, n1, s), f(1.0, n0, s), f(1.0, n1, s));
        let (v00, v01, v10, v11) = (a + n0 * r0, a + n1 * r0, b + n0 * r1, b + n1 * r1);
        mb.tri(v00, v10, v11, l00, l10, l11);
        mb.tri(v00, v11, v01, l00, l11, l01);
    }
}

/// Flat disc / cone cap (centre `c`, plane normal `n`), lighting callback gets the radial fraction.
fn disc(mb: &mut MB, c: Vec3, n: Vec3, r: f32, sides: usize, f: &dyn Fn(f32) -> Lit) {
    let (u, v) = perp(n.normalize_or_zero());
    let lc = f(0.0);
    let le = f(1.0);
    for s in 0..sides {
        let a0 = TAU * s as f32 / sides as f32;
        let a1 = TAU * (s + 1) as f32 / sides as f32;
        let p0 = c + (u * a0.cos() + v * a0.sin()) * r;
        let p1 = c + (u * a1.cos() + v * a1.sin()) * r;
        mb.tri(c, p0, p1, lc, le, le);
    }
}

fn tout(mb: &mut MB, a: Vec3, b: Vec3, c: Vec3, col: Vec3, cen: Vec3) {
    let mut n = (b - a).cross(c - a).normalize_or_zero();
    if n.dot((a + b + c) / 3.0 - cen) < 0.0 {
        n = -n;
    }
    let k = shade(col, n);
    mb.tri(a, b, c, k, k, k);
}

fn qout(mb: &mut MB, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: Vec3, cen: Vec3) {
    tout(mb, a, b, c, col, cen);
    tout(mb, a, c, d, col, cen);
}

/// Thin two-sided triangle with a correct normal on each face.
fn dtri(mb: &mut MB, a: Vec3, b: Vec3, c: Vec3, col: Vec3) {
    let n = (b - a).cross(c - a).normalize_or_zero();
    let k = shade(col, n);
    let kb = shade(col * 0.85, -n);
    let o = n * 0.0004;
    mb.tri(a + o, b + o, c + o, k, k, k);
    mb.tri(a - o, c - o, b - o, kb, kb, kb);
}

/// Little glowing octahedron (embers, coals).
fn gem(mb: &mut MB, c: Vec3, r: Vec3, top: Vec3, bot: Vec3) {
    let (lt, lb) = (glow(top), glow(bot));
    let t = c + Vec3::Y * r.y;
    let b = c - Vec3::Y * r.y;
    let ring = [c + Vec3::X * r.x, c + Vec3::Z * r.z, c - Vec3::X * r.x, c - Vec3::Z * r.z];
    for i in 0..4 {
        let (p, q) = (ring[i], ring[(i + 1) % 4]);
        let le = glow((top + bot) * 0.5);
        mb.tri(t, p, q, lt, le, le);
        mb.tri(b, q, p, lb, le, le);
    }
}

fn hn(x: f32) -> f32 {
    ((x * 12.9898).sin() * 43758.547).fract().abs()
}

// ---------------------------------------------------------------------------
// Lantern
// ---------------------------------------------------------------------------
/// A square black-metal candle lantern standing on `base` (centre of its bottom plate).
/// About 0.31 m tall at scale 1 (handle included). Opaque renderer: glass is suggested by
/// thin frame bars with open gaps through which the candle shows.
pub fn lantern_model(mb: &mut MB, base: Vec3, scale: f32, yaw: f32, lit: bool) {
    let (sy, cy) = yaw.sin_cos();
    let f = vec3(cy, 0.0, sy);
    let r = vec3(-sy, 0.0, cy);
    let p = |x: f32, y: f32, z: f32| base + vec3(x * cy - z * sy, y, x * sy + z * cy) * scale;
    let bx = |mb: &mut MB, c: Vec3, h: Vec3, col: Vec3| bbox(mb, p(c.x, c.y, c.z), f, r, h * scale, col);

    // old worn black iron: slightly warm, with a few rusty parts
    let iron = vec3(0.052, 0.048, 0.046);
    let iron2 = vec3(0.095, 0.080, 0.072);
    let cream = vec3(0.82, 0.74, 0.55);

    // base plate with a round raised tray
    bx(mb, vec3(0.0, 0.006, 0.0), vec3(0.064, 0.006, 0.064), iron);
    bx(mb, vec3(0.0, 0.0135, 0.0), vec3(0.058, 0.0015, 0.058), iron2);
    tube(mb, p(0.0, 0.015, 0.0), p(0.0, 0.0235, 0.0), 0.047 * scale, 0.050 * scale, 14, &|_, n, _| shade(iron2, n));
    disc(mb, p(0.0, 0.0148, 0.0), Vec3::Y, 0.047 * scale, 14, &|_| shade(iron, Vec3::Y));
    // four corner posts, three rails, mullions
    let (k, y0, y1) = (0.052f32, 0.015f32, 0.166f32);
    for (sx, sz) in [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        bx(mb, vec3(sx * k, (y0 + y1) * 0.5, sz * k), vec3(0.0048, (y1 - y0) * 0.5, 0.0048), iron);
    }
    for (ry, th) in [(0.022f32, 0.0045f32), (0.098, 0.0030), (0.1625, 0.0045)] {
        for s in [-1.0f32, 1.0] {
            bx(mb, vec3(0.0, ry, s * k), vec3(k, th, th), iron2);
            bx(mb, vec3(s * k, ry, 0.0), vec3(th, th, k), iron2);
        }
    }
    // slim mullion in the middle of each face and a thin glass-edge wire
    for s in [-1.0f32, 1.0] {
        bx(mb, vec3(0.0, 0.130, s * k), vec3(0.0016, 0.032, 0.0016), iron);
        bx(mb, vec3(s * k, 0.130, 0.0), vec3(0.0016, 0.032, 0.0016), iron);
        bx(mb, vec3(0.0, 0.060, s * k), vec3(0.0016, 0.036, 0.0016), iron);
        bx(mb, vec3(s * k, 0.060, 0.0), vec3(0.0016, 0.036, 0.0016), iron);
    }
    // top collar
    bx(mb, vec3(0.0, 0.171, 0.0), vec3(0.062, 0.0055, 0.062), iron);
    // pyramid roof (frustum), vent neck, hat and finial
    let (h0, h1, ya, yb) = (0.066f32, 0.020f32, 0.1765f32, 0.2155f32);
    let cen = p(0.0, (ya + yb) * 0.5, 0.0);
    let bot = [p(h0, ya, h0), p(-h0, ya, h0), p(-h0, ya, -h0), p(h0, ya, -h0)];
    let top = [p(h1, yb, h1), p(-h1, yb, h1), p(-h1, yb, -h1), p(h1, yb, -h1)];
    for i in 0..4 {
        let j = (i + 1) % 4;
        qout(mb, bot[i], bot[j], top[j], top[i], iron, cen);
    }
    // thin trim along roof edges
    for i in 0..4 {
        cyl(mb, bot[i], top[i], 0.0022 * scale, 0.0022 * scale, iron2, 4);
    }
    bx(mb, vec3(0.0, 0.2215, 0.0), vec3(0.0115, 0.0065, 0.0115), iron);
    let hb = 0.030f32;
    let (yh0, yh1) = (0.2275f32, 0.247f32);
    let hat_c = p(0.0, yh0 + 0.008, 0.0);
    let hb_pts = [p(hb, yh0, hb), p(-hb, yh0, hb), p(-hb, yh0, -hb), p(hb, yh0, -hb)];
    let apex = p(0.0, yh1, 0.0);
    for i in 0..4 {
        tout(mb, hb_pts[i], hb_pts[(i + 1) % 4], apex, iron, hat_c);
    }
    // underside of the hat so it reads as a cap
    let under = shade(iron, -Vec3::Y);
    mb.tri(hb_pts[0], hb_pts[2], hb_pts[1], under, under, under);
    mb.tri(hb_pts[0], hb_pts[3], hb_pts[2], under, under, under);
    blob(mb, p(0.0, yh1 + 0.002, 0.0), Vec3::splat(0.0075 * scale), iron2, 0.0, 6, 4, 0.0, shade);
    // arched ring handle (vertical, in the lantern's x/y plane)
    let rc = p(0.0, yh1 + 0.0105 + 0.027, 0.0);
    ring(mb, rc, (r + f * 0.10).normalize(), 0.027 * scale, 0.0032 * scale, iron2, 16);
    // rust blooms on the roof, collar and base
    let rustc = vec3(0.30, 0.14, 0.07);
    blob(mb, p(0.034, 0.200, 0.026), vec3(0.012, 0.0035, 0.009) * scale, rustc, 0.0, 6, 3, 0.0, shade);
    blob(mb, p(-0.030, 0.19, -0.040), vec3(0.010, 0.0035, 0.008) * scale, rustc * 0.9, 0.0, 6, 3, 0.7, shade);
    bx(mb, vec3(0.058, 0.171, -0.02), vec3(0.0065, 0.0035, 0.016), rustc);
    bx(mb, vec3(-0.02, 0.012, 0.0645), vec3(0.025, 0.0045, 0.0012), rustc * 0.9);

    // candle on the tray
    let c_top = 0.098f32;
    let c_r = 0.0165 * scale;
    let cream_lit = |t: f32, n: Vec3| -> Lit {
        if lit {
            // glowing wax: brightest near the flame
            glow(cream * (0.55 + 0.45 * t) * 1.05)
        } else {
            shade(cream * (0.82 + 0.18 * t), n)
        }
    };
    tube(mb, p(0.0, 0.0235, 0.0), p(0.0, c_top, 0.0), c_r * 1.07, c_r, 12, &|t, n, _| cream_lit(t, n));
    disc(mb, p(0.0, c_top, 0.0), Vec3::Y, c_r, 12, &|rad| {
        if lit { glow(vec3(1.0, 0.86, 0.55) * (1.0 - 0.25 * rad)) } else { shade(cream * 0.92, Vec3::Y) }
    });
    // melted pool of wax at the foot and a few drips
    blob(mb, p(0.0, 0.0245, 0.0), vec3(0.030, 0.0045, 0.030) * scale, cream * 0.95, 0.0, 10, 3, 0.0, shade);
    for (a, l) in [(0.5f32, 0.020f32), (2.4, 0.032), (4.2, 0.014)] {
        let (dx, dz) = (a.cos() * 0.0158, a.sin() * 0.0158);
        cyl(mb, p(dx, c_top - 0.001, dz), p(dx, c_top - l, dz), 0.0033 * scale, 0.0026 * scale, cream, 4);
        blob(mb, p(dx, c_top - l, dz), Vec3::splat(0.0033 * scale), cream, 0.0, 6, 3, 0.0, shade);
    }
    // dark wick
    cyl(mb, p(0.0, c_top, 0.0), p(0.0, c_top + 0.012, 0.0), 0.0013 * scale, 0.0010 * scale, vec3(0.06, 0.045, 0.04), 4);

    if lit {
        let t = macroquad::time::get_time() as f32;
        let flick = 1.0 + 0.07 * (t * 13.0).sin() + 0.05 * (t * 21.0 + 1.0).sin();
        let lean = vec3((t * 6.0).sin() * 0.0012, 0.0, (t * 5.1).cos() * 0.0012);
        let wick = p(0.0, c_top + 0.010, 0.0);
        // outer teardrop: yellow at the bottom to orange at the tip; pointed
        let height = 0.046 * scale * flick;
        let rmax = 0.0105 * scale;
        let rings = 8usize;
        let sides = 8usize;
        let bot_c = wick + Vec3::Y * 0.004 * scale;
        let prof = |u: f32| -> f32 { (PI * u.powf(0.62)).sin().max(0.0).powf(0.8) };
        let col_o = |u: f32| -> Vec3 {
            let a = vec3(1.0, 0.86, 0.35);
            let b = vec3(1.0, 0.48, 0.07);
            let c = vec3(0.95, 0.28, 0.04);
            if u < 0.5 { a.lerp(b, u * 2.0) } else { b.lerp(c, (u - 0.5) * 2.0) }
        };
        let ring_pt = |u: f32, a: f32, rm: f32, h: f32| -> Vec3 {
            let c = bot_c + Vec3::Y * (h * u) + lean * (u * u) * scale * 6.0;
            c + vec3(a.cos(), 0.0, a.sin()) * (rm * prof(u))
        };
        for k in 0..rings {
            let (u0, u1) = (k as f32 / rings as f32, (k + 1) as f32 / rings as f32);
            for s in 0..sides {
                let (a0, a1) = (TAU * s as f32 / sides as f32, TAU * (s + 1) as f32 / sides as f32);
                let (p00, p01) = (ring_pt(u0, a0, rmax, height), ring_pt(u0, a1, rmax, height));
                let (p10, p11) = (ring_pt(u1, a0, rmax, height), ring_pt(u1, a1, rmax, height));
                let (c0, c1) = (glow(col_o(u0)), glow(col_o(u1)));
                mb.tri(p00, p10, p11, c0, c1, c1);
                if k > 0 {
                    mb.tri(p00, p11, p01, c0, c1, c0);
                }
            }
        }
        // bright white-yellow core bulb poking out of the flame base (opaque, so it sits low and wide)
        blob(mb, wick + Vec3::Y * 0.0095 * scale, vec3(0.0068, 0.0095, 0.0068) * scale, vec3(1.0, 0.98, 0.78), 0.0, 8, 5, 0.0, |c, _| glow(c));
        // faint soft glow: a dim flattened halo resting on the wax top
        disc(mb, p(0.0, c_top + 0.0006, 0.0), Vec3::Y, c_r * 1.05, 12, &|rad| glow(vec3(1.0, 0.75, 0.38) * (1.0 - 0.3 * rad)));
    }
}

// ---------------------------------------------------------------------------
// Campfire flames (rebuilt every frame)
// ---------------------------------------------------------------------------
#[allow(clippy::too_many_arguments)]
fn tongue(mb: &mut MB, base: Vec3, h: f32, w: f32, lean: Vec3, ph: f32, heat: f32) {
    const N: usize = 6;
    let grad = |s: f32| -> Vec3 {
        let a = vec3(0.85, 0.08, 0.02);
        let b = vec3(1.0, 0.40, 0.05);
        let c = vec3(1.0, 0.72, 0.14);
        let d = vec3(1.0, 0.92, 0.50);
        let g = if s < 0.25 {
            a.lerp(b, s / 0.25)
        } else if s < 0.6 {
            b.lerp(c, (s - 0.25) / 0.35)
        } else {
            c.lerp(d, (s - 0.6) / 0.4)
        };
        g.lerp(vec3(1.0, 0.97, 0.78), heat * (0.35 + 0.5 * s))
    };
    let side = vec3(-lean.z, 0.0, lean.x).normalize_or_zero();
    let mut rings: Vec<[Vec3; 3]> = Vec::with_capacity(N + 1);
    let mut cols: Vec<Lit> = Vec::with_capacity(N + 1);
    let mut tip = base;
    for k in 0..=N {
        let s = k as f32 / N as f32;
        let c = base + Vec3::Y * (h * s) + lean * (s * s) + side * ((s * 4.0 + ph).sin() * 0.035 * h * s);
        let prof = (1.0 - s).max(0.0).powf(0.7) * (0.55 + 0.45 * (s * 3.5).min(1.0));
        let rad = w * prof;
        let mut ring = [Vec3::ZERO; 3];
        for (j, rp) in ring.iter_mut().enumerate() {
            let a = ph + TAU * j as f32 / 3.0;
            *rp = c + vec3(a.cos(), 0.0, a.sin()) * rad;
        }
        if k == N {
            tip = c;
        }
        rings.push(ring);
        cols.push(glow(grad(s)));
    }
    for k in 0..N {
        for j in 0..3 {
            let j2 = (j + 1) % 3;
            if k == N - 1 {
                mb.tri(rings[k][j], tip, rings[k][j2], cols[k], cols[k + 1], cols[k]);
            } else {
                mb.tri(rings[k][j], rings[k + 1][j], rings[k + 1][j2], cols[k], cols[k + 1], cols[k + 1]);
                mb.tri(rings[k][j], rings[k + 1][j2], rings[k][j2], cols[k], cols[k + 1], cols[k]);
            }
        }
    }
}

/// Animated campfire flames drawn around `pos` (centre of the log pile at log height).
/// Always lit. Rebuild every frame with the running game time.
pub fn draw_campfire_flames(mb: &mut MB, pos: Vec3, time: f32) {
    let base_y = pos.y + 0.07;
    // glowing coals between the logs
    for i in 0..6 {
        let a = i as f32 * 1.05 + 0.4;
        let rr = 0.06 + 0.12 * hn(i as f32 + 3.0);
        let pulse = 0.65 + 0.35 * (time * (2.5 + i as f32 * 0.7) + i as f32 * 1.9).sin();
        let c = pos + vec3(a.cos() * rr, 0.045, a.sin() * rr);
        gem(mb, c, vec3(0.045, 0.026, 0.040), vec3(1.0, 0.45, 0.10) * pulse, vec3(0.55, 0.06, 0.02) * pulse);
    }
    // outer red/orange tongues
    for k in 0..6 {
        let kf = k as f32;
        let a = kf * (TAU / 6.0) + 0.35 + 0.15 * (time * 0.7 + kf).sin();
        let out = vec3(a.cos(), 0.0, a.sin());
        let hgt = (0.50 + 0.22 * hn(kf * 1.7)) * (0.82 + 0.22 * (time * (6.5 + kf * 1.3) + kf * 2.1).sin() + 0.08 * (time * 17.0 + kf).sin());
        let sway = vec3((time * (2.1 + kf * 0.31) + kf).sin(), 0.0, (time * (1.7 + kf * 0.27) + kf * 1.3).cos()) * 0.07;
        let lean = -out * 0.07 + sway;
        let wid = 0.082 * (0.9 + 0.2 * (time * (8.0 + kf) + kf).sin());
        tongue(mb, base_y_pos(pos, base_y) + out * 0.15, hgt, wid, lean, kf * 0.9, 0.0);
    }
    // mid yellow-orange tongues
    for k in 0..3 {
        let kf = k as f32;
        let a = kf * (TAU / 3.0) + 1.0 + 0.2 * (time * 0.9 + kf * 2.0).sin();
        let out = vec3(a.cos(), 0.0, a.sin());
        let hgt = (0.66 + 0.15 * hn(kf * 4.1 + 9.0)) * (0.85 + 0.2 * (time * (7.7 + kf * 1.1) + kf * 3.1).sin());
        let sway = vec3((time * 2.7 + kf * 2.0).sin(), 0.0, (time * 2.2 + kf).cos()) * 0.06;
        tongue(mb, base_y_pos(pos, base_y) + out * 0.065, hgt, 0.07, sway, kf * 1.3 + 0.5, 0.45);
    }
    // white-hot central core
    let hgt = 0.74 * (0.86 + 0.18 * (time * 9.3).sin() + 0.06 * (time * 23.0).sin());
    let sway = vec3((time * 3.1).sin(), 0.0, (time * 2.6).cos()) * 0.05;
    tongue(mb, base_y_pos(pos, base_y), hgt, 0.055, sway, 0.2, 1.0);
    // small low red licks near the logs
    for k in 0..3 {
        let kf = k as f32;
        let a = kf * (TAU / 3.0) + 2.4;
        let h = 0.20 + 0.08 * (time * (9.0 + kf * 2.0) + kf).sin();
        tongue(mb, base_y_pos(pos, base_y) + vec3(a.cos(), 0.0, a.sin()) * 0.24, h, 0.045, vec3(a.cos(), 0.0, a.sin()) * -0.03, kf, 0.0);
    }
    // rising embers
    for i in 0..8 {
        let fi = i as f32;
        let sp = 0.28 + 0.22 * hn(fi * 2.3);
        let ph = (time * sp + hn(fi * 5.7)).fract();
        let a0 = hn(fi * 3.1) * TAU;
        let r0 = 0.05 + 0.15 * hn(fi * 7.9);
        let drift = vec3((time * 1.3 + fi * 2.0).sin(), 0.0, (time * 1.1 + fi * 1.4).cos()) * (0.12 * ph);
        let p = pos + vec3(a0.cos() * r0, 0.25 + ph * 1.5, a0.sin() * r0) + drift;
        let sz = 0.017 * (1.0 - ph).powf(0.8) * (0.7 + 0.5 * hn(fi + 0.5));
        let top = vec3(1.0, 0.85, 0.35).lerp(vec3(1.0, 0.35, 0.08), ph);
        let bot = vec3(1.0, 0.5, 0.1).lerp(vec3(0.6, 0.1, 0.03), ph);
        gem(mb, p, vec3(sz, sz * 1.5, sz), top, bot);
    }
}

#[inline]
fn base_y_pos(pos: Vec3, y: f32) -> Vec3 {
    vec3(pos.x, y, pos.z)
}

// ---------------------------------------------------------------------------
// Bait tin and fish bucket (held items)
// ---------------------------------------------------------------------------
fn held_frame(up: Vec3) -> (Vec3, Vec3, Vec3) {
    let u = if up.length_squared() < 1e-6 { Vec3::Y } else { up.normalize() };
    let ref_dir = if u.x.abs() < 0.85 { Vec3::X } else { Vec3::Z };
    let a = (ref_dir - u * ref_dir.dot(u)).normalize_or_zero();
    (u, a, u.cross(a))
}

fn worm(mb: &mut MB, o: Vec3, d: Vec3, w: Vec3, len: f32, ph: f32, col: Vec3, rad: f32) {
    let mut prev = o;
    for k in 1..=5 {
        let s = k as f32 / 5.0;
        let q = o + d * (len * s) + w * ((s * 5.0 + ph).sin() * len * 0.14);
        cyl(mb, prev, q, rad, rad * (1.0 - 0.18 * s), col, 4);
        prev = q;
    }
    blob(mb, prev, Vec3::splat(rad * 1.05), col, 0.0, 5, 3, 0.0, shade);
}

pub fn draw_bait_tin(mb: &mut MB, pos: Vec3, up: Vec3) {
    let (u, a, b) = held_frame(up);
    // old salvaged tin can: dull grey tin, streaked with rust, a few dents
    let tin = vec3(0.46, 0.45, 0.42);
    let tin_d = vec3(0.30, 0.27, 0.24);
    let rust = vec3(0.42, 0.22, 0.11);
    let rad = 0.046f32;
    let hh = 0.034f32;
    let lo = pos - u * hh;
    let hi = pos + u * hh;
    let wear = move |t: f32, s: usize, base: Vec3| -> Vec3 {
        let h = hn(s as f32 * 3.7 + (t * 2.5).floor() * 1.9 + 0.3);
        if h > 0.55 { base.lerp(rust, 0.45 + 0.5 * h) } else { base * (0.8 + 0.4 * h) }
    };
    // body in three slightly dented sections with pressed ribs between them
    let secs = [(-1.0f32, -0.34f32, 0.0f32, 0.0f32), (-0.34, 0.34, 0.0035, 0.0), (0.34, 1.0, -0.002, 0.0)];
    for (i, (t0, t1, dent, _)) in secs.iter().enumerate() {
        let c0 = pos + u * (hh * t0);
        let c1 = pos + u * (hh * t1);
        let off = (a * 0.5 + b * 0.3) * (dent * 0.6);
        let rr = rad - dent.abs();
        tube(mb, c0 + off, c1 + off, rr, rr, 14, &|t, n, s| shade(wear(t, s + i * 5, tin), n));
    }
    for t in [-0.34f32, 0.34] {
        let c = pos + u * (hh * t);
        tube(mb, c - u * 0.0022, c + u * 0.0022, rad * 1.03, rad * 1.03, 14, &|_, n, s| shade(wear(0.0, s + 40, tin_d), n));
    }
    disc(mb, lo, -u, rad, 14, &|_| shade(tin_d, -u));
    tube(mb, hi - u * 0.003, hi + u * 0.002, rad * 1.045, rad * 1.045, 14, &|_, n, s| shade(wear(0.0, s + 60, tin * 0.9), n));
    tube(mb, lo, lo + u * 0.004, rad * 1.04, rad * 1.04, 14, &|_, n, s| shade(wear(0.0, s + 80, tin_d), n));
    // dark damp earth with the bait on top
    let soil_c = hi - u * 0.004;
    disc(mb, soil_c, u, rad * 0.93, 14, &|r| shade(vec3(0.17, 0.11, 0.07) * (1.0 - 0.2 * r), u));
    let worm_col = vec3(0.60, 0.36, 0.32);
    let worm_col2 = vec3(0.48, 0.28, 0.23);
    worm(mb, soil_c + u * 0.004 - a * 0.030 - b * 0.020, a * 0.9 + b * 0.35, b, 0.052, 0.0, worm_col, 0.0042);
    worm(mb, soil_c + u * 0.0045 - a * 0.028 + b * 0.010, a * 0.95 - b * 0.2, b, 0.050, 1.7, worm_col2, 0.0040);
    worm(mb, soil_c + u * 0.004 - a * 0.020 + b * 0.034, a * 0.8 - b * 0.55, b, 0.045, 3.1, worm_col, 0.0040);
    worm(mb, soil_c + u * 0.0055 - a * 0.005 - b * 0.004, a * 0.5 + b * 0.8, a, 0.040, 4.4, worm_col2, 0.0038);
    // two curled shrimp
    for (o, ph) in [(a * 0.022 + b * 0.012, 0.0f32), (a * 0.016 - b * 0.026, 2.0)] {
        let shr = vec3(0.72, 0.45, 0.36);
        let mut prev = soil_c + u * 0.007 + o;
        let ang0 = ph;
        for k in 1..=5 {
            let t = k as f32 / 5.0;
            let ang = ang0 + t * 2.4;
            let q = soil_c + u * (0.007 + 0.008 * (t * PI).sin()) + o + (a * ang.cos() + b * ang.sin()) * (0.026 * t);
            cyl(mb, prev, q, 0.0058 * (1.0 - 0.45 * t) + 0.002, 0.0058 * (1.0 - 0.45 * (t + 0.2)) + 0.002, shr, 5);
            prev = q;
        }
        dtri(mb, prev, prev + (a * (ang0 + 2.0).cos() + b * (ang0 + 2.0).sin()) * 0.012 + u * 0.003, prev + (a * (ang0 + 3.0).cos() + b * (ang0 + 3.0).sin()) * 0.012, shr * 0.9);
        blob(mb, soil_c + u * 0.008 + o, vec3(0.0065, 0.0065, 0.0065), vec3(0.15, 0.1, 0.1), 0.0, 5, 3, 0.0, shade);
    }
    // lid, hinged on one side and propped open about 65 degrees
    let hinge = hi + a * (rad * 1.02) + u * 0.001;
    let th = 1.15f32;
    let ld = -a * th.cos() + u * th.sin(); // direction from hinge across the lid
    let ln = (a * th.sin() + u * th.cos()).normalize(); // lid normal
    let lc = hinge + ld * rad * 1.02;
    let lidc = tin.lerp(rust, 0.35);
    tube(mb, lc - ln * 0.003, lc + ln * 0.003, rad * 1.045, rad * 1.045, 14, &|_, n, s| shade(wear(0.0, s + 100, lidc), n));
    disc(mb, lc + ln * 0.003, ln, rad * 1.045, 14, &|r| shade(lidc * (1.0 - 0.15 * r), ln));
    disc(mb, lc - ln * 0.003, -ln, rad * 1.045, 14, &|r| shade(tin_d * (1.0 - 0.1 * r), -ln));
    tube(mb, lc + ln * 0.003, lc + ln * 0.0045, rad * 0.55, rad * 0.55, 10, &|_, n, _| shade(rust * 0.9, n));
    // little pull tab
    blob(mb, lc + ln * 0.006 + ld * (rad * 0.65), vec3(0.008, 0.003, 0.008), tin_d, 0.0, 6, 3, 0.0, shade);
    cyl(mb, hinge - b * 0.012, hinge + b * 0.012, 0.0035, 0.0035, tin_d, 5);
}

pub fn draw_bucket(mb: &mut MB, pos: Vec3, up: Vec3) {
    let (u, a, b) = held_frame(up);
    let (wood_a, wood_b) = (vec3(0.40, 0.31, 0.21), vec3(0.31, 0.24, 0.17));
    let steel = vec3(0.26, 0.19, 0.14); // rusty iron hoops
    let hh = 0.062f32;
    let (r_bot, r_top) = (0.048f32, 0.066f32);
    let lo = pos - u * hh;
    let hi = pos + u * hh;
    let rat = |t: f32| mixf(r_bot, r_top, t);
    // staves
    tube(mb, lo, hi, r_bot, r_top, 14, &|_, n, s| shade(if s % 2 == 0 { wood_a } else { wood_b }, n));
    disc(mb, lo, -u, r_bot, 14, &|_| shade(wood_b * 0.7, -u));
    // iron hoops and rim
    for t in [0.2f32, 0.78] {
        let c0 = lo + u * (2.0 * hh * (t - 0.04));
        let c1 = lo + u * (2.0 * hh * (t + 0.04));
        tube(mb, c0, c1, rat(t - 0.04) * 1.035, rat(t + 0.04) * 1.035, 14, &|_, n, _| shade(steel, n));
    }
    tube(mb, hi - u * 0.005, hi + u * 0.003, r_top * 1.03, r_top * 1.03, 14, &|_, n, _| shade(steel, n));
    // water surface and the inner wall
    tube(mb, hi, hi - u * 0.02, r_top * 0.97, r_top * 0.95, 14, &|t, n, s| shade((if s % 2 == 0 { wood_a } else { wood_b }) * (0.55 - 0.15 * t), -n));
    disc(mb, hi - u * 0.02, u, r_top * 0.95, 14, &|r| shade(vec3(0.17, 0.26, 0.26) * (1.0 - 0.15 * r), u));
    // fish: body disappearing in the water and a forked tail poking out
    let silver = vec3(0.45, 0.50, 0.52);
    let fa = hi - u * 0.02 + a * 0.020 + b * 0.008;
    let fb = hi + u * 0.045 + a * 0.028 + b * 0.012;
    cyl(mb, fa, fb, 0.0145, 0.0065, silver, 8);
    cyl(mb, fa + a * 0.001, fb + a * 0.001, 0.0148, 0.0068, vec3(0.22, 0.28, 0.32), 4);
    let root = fb;
    let ncol = vec3(0.62, 0.38, 0.22);
    let l1 = root + u * 0.040 + b * 0.026;
    let l2 = root + u * 0.040 - b * 0.026;
    let notch = root + u * 0.026;
    let rl = root + b * 0.005;
    let rr = root - b * 0.005;
    dtri(mb, rl, rr, notch, ncol);
    dtri(mb, rl, notch, l1, ncol);
    dtri(mb, rr, l2, notch, ncol);
    dtri(mb, rl, l1, root + u * 0.012 + b * 0.012, ncol * 1.1);
    dtri(mb, rr, l2, root + u * 0.012 - b * 0.012, ncol * 1.1);
    // wire/rope bail handle arching over the top
    let rope = vec3(0.55, 0.46, 0.30);
    let mut prev = hi + b * (r_top * 1.03);
    for k in 1..=14 {
        let t = PI * k as f32 / 14.0;
        let q = hi + b * (r_top * 1.03 * t.cos()) + u * (0.078 * t.sin()) + a * ((t * 2.0).sin() * 0.003);
        // twisted rope look: alternate light/dark, thick strands
        cyl(mb, prev, q, 0.0042, 0.0042, if k % 2 == 0 { rope } else { rope * 0.72 }, 5);
        prev = q;
    }
    // rope knots through holes under the rim
    for s in [-1.0f32, 1.0] {
        blob(mb, hi + b * (s * r_top * 1.05) - u * 0.008, vec3(0.0075, 0.0075, 0.0075), rope * 0.8, 0.0, 6, 3, 0.0, shade);
    }
}

/// A chunky orange flare pistol held at `pos`, the barrel pointing along `dir`.
pub fn draw_flare_gun(mb: &mut MB, pos: Vec3, dir: Vec3, up: Vec3) {
    let f = dir.normalize_or_zero();
    let s = f.cross(up).normalize_or_zero();
    if f == Vec3::ZERO || s == Vec3::ZERO {
        return;
    }
    let u = s.cross(f);
    let orange = vec3(0.92, 0.42, 0.08);
    let black = vec3(0.07, 0.07, 0.08);
    cyl(mb, pos, pos + f * 0.20, 0.030, 0.032, orange, 10);
    cyl(mb, pos + f * 0.20, pos + f * 0.22, 0.036, 0.036, black, 10);
    obox(mb, pos - f * 0.04 - u * 0.03, [f, u, s], vec3(0.085, 0.036, 0.030), orange * 0.9);
    beam(mb, pos - f * 0.07 - u * 0.04, pos - f * 0.12 - u * 0.17, 0.05, 0.04, black);
    ring(mb, pos - f * 0.025 - u * 0.075, s, 0.03, 0.005, black, 8);
    blob(mb, pos - f * 0.085 + u * 0.005, Vec3::splat(0.022), vec3(0.75, 0.75, 0.78), 0.0, 5, 3, 0.0, shade);
}
