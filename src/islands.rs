//! The other islands: the two traders (boat yard and salvage), the wrecked camps of the people who
//! came before, and the diary pages they left behind.

use super::*;
use crate::people::Look;
use crate::progress::N_PAGES;

// ---------------------------------------------------------------------------
// The diary pages
// ---------------------------------------------------------------------------
pub struct Page {
    pub title: &'static str,
    pub text: &'static str,
}

pub const PAGES: [Page; N_PAGES] = [
    Page {
        title: "Captain's log - day 1",
        text: "We put the Marigold on the reef in the dark. Fuel gone, radio drowned, six of us alive. There is fruit, fresh water and more fish than I have ever seen. We will be fine. We will be home in a week. I have started a camp on the high ground so we can see the sea.",
    },
    Page {
        title: "Captain's log - day 6",
        text: "The fish out past the dock are enormous. Hendry pulled in a thing the size of a rowboat and we ate like kings. Mara says the sea sounded like singing last night, low and slow, from out where the water goes black. I told her it was the wind. It was not the wind.",
    },
    Page {
        title: "Mara's diary",
        text: "DO NOT FISH AT NIGHT. I wrote it on the rock so nobody can say they didn't know. The big ones come up when the moon is out. They don't take the line, they follow the boat. They follow the light. They follow you home. Stay by the fire. The fire is the only thing they will not cross.",
    },
    Page {
        title: "Hendry's note",
        text: "Took the dinghy out to the deep water for a bigger catch. Something hit us from underneath. Twice. It put a hole in the hull you could climb through and I swam back with my hands shaking so hard I could not hold a cup. Don't tell the kids. Steel would have held it. Everything else just breaks.",
    },
    Page {
        title: "A torn page",
        text: "The moon went red tonight. The sea went flat as glass and every bird on the island stopped at once. Not quiet. Stopped. Even the insects. Then the water by the dock started to move against the tide, and there were shapes in it, and they were not swimming away from us, they were [the rest of the page has been torn out]",
    },
    Page {
        title: "Captain's log - the last entry",
        text: "Four gone. The things do not like fire. A flare sent one away for an hour, and a strong lamp on the water makes them turn aside. Stay lit. Stay close to land. The boat key went down with the dinghy. I watched the biggest one take it in its jaws. Whoever finds this: the key is down there with it. Do not go out on the red moon.",
    },
    Page {
        title: "A child's drawing",
        text: "A crayon drawing, curling with damp: a little boat, a huge fish with far too many teeth, and a tiny stick figure holding a lantern. Underneath, in careful capitals: DADDY SAYS THE LANTERN KEEPS THEM AWAY. MY LANTERN IS THE BIGGEST. There is a second, smaller figure beside the first, drawn in black, with no face.",
    },
    Page {
        title: "Scratched into the rock",
        text: "IT KNOWS YOUR NAME. IT KNOWS WHEN YOU ARE ALONE. IT IS PATIENT. IT WAS HERE BEFORE THE ISLAND. WE WERE NEVER THE ONES FISHING. Beneath the words, hundreds of tally marks, in groups of five, going down and down into the sand.",
    },
];

// ---------------------------------------------------------------------------
// Traders
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Good {
    Rod(usize),
    Bait(usize),
    /// fishing upgrades (line, leader, reels)
    Upgrade(usize),
    /// boat parts
    Boat(usize),
    Gas,
    Flares,
    FlareGun,
    Repair,
}

pub const BOATYARD: &[Good] = &[Good::Rod(4), Good::Upgrade(3), Good::Bait(5), Good::Boat(crate::progress::ENGINE), Good::Boat(crate::progress::TURBO), Good::Boat(crate::progress::BIG_TANK), Good::Gas, Good::Repair];
pub const SALVAGE: &[Good] = &[Good::Boat(crate::progress::REINFORCED), Good::Boat(crate::progress::STEEL), Good::Boat(crate::progress::SEARCHLIGHT), Good::FlareGun, Good::Flares, Good::Bait(6)];

pub struct Vendor {
    pub id: usize,
    pub name: &'static str,
    /// the counter
    pub pos: Vec3,
    /// where the customer stands
    pub trade: Vec3,
    /// the way the stall faces (towards the customers)
    pub yaw: f32,
    /// where the trader stands (behind the counter)
    pub person: Vec3,
    pub look: Look,
    pub greeting: &'static str,
    pub night_line: &'static str,
    pub stock: &'static [Good],
}

pub struct PageSite {
    pub id: usize,
    pub pos: Vec3,
}

pub struct Islands {
    pub vendors: Vec<Vendor>,
    pub pages: Vec<PageSite>,
    pub cols: Vec<Col>,
}

fn flat_spot(t: &Terrain, c: Vec2, rad: f32, hmin: f32, hmax: f32, avoid: &[Vec3], sep: f32) -> Option<Vec3> {
    for _ in 0..6000 {
        let a = rf(0.0, TAU);
        let r = rf(0.0, rad);
        let (x, z) = (c.x + a.cos() * r, c.y + a.sin() * r);
        let h = t.at(x, z);
        if h < hmin || h > hmax {
            continue;
        }
        let flat = [(2.5, 0.0), (-2.5, 0.0), (0.0, 2.5), (0.0, -2.5)].iter().all(|(dx, dz)| (t.at(x + dx, z + dz) - h).abs() < 0.55);
        if flat && avoid.iter().all(|p| vec2(p.x - x, p.z - z).length() > sep) {
            return Some(vec3(x, h, z));
        }
    }
    None
}

fn awning(mb: &mut MB, base: Vec3, f3: Vec3, fr: Vec3, c1: Vec3, c2: Vec3) {
    // slanted striped canvas: higher at the back, with a scalloped front edge
    let (hw, front, back) = (1.7f32, 1.2f32, 0.9f32);
    let stripes = 8;
    for k in 0..stripes {
        let (u0, u1) = (k as f32 / stripes as f32, (k + 1) as f32 / stripes as f32);
        let x0 = -hw + 2.0 * hw * u0;
        let x1 = -hw + 2.0 * hw * u1;
        let fa = base + f3 * x0 + fr * front + Vec3::Y * 2.35;
        let fb = base + f3 * x1 + fr * front + Vec3::Y * 2.35;
        let ba = base + f3 * x0 - fr * back + Vec3::Y * 2.85;
        let bb = base + f3 * x1 - fr * back + Vec3::Y * 2.85;
        let mut n = (fb - fa).cross(ba - fa).normalize_or_zero();
        if n.y < 0.0 {
            n = -n;
        }
        let col = if k % 2 == 0 { c1 } else { c2 };
        mb.nquad(fa, fb, bb, ba, col, n);
        mb.nquad(ba, bb, fb, fa, col * 0.75, -n);
        // scallop
        let mid = (fa + fb) * 0.5 - Vec3::Y * 0.2;
        mb.ftri(fa, fb, mid, col * 0.95);
    }
    for sx in [-1.0f32, 1.0] {
        for (fz, h) in [(front, 2.35f32), (-back, 2.85)] {
            let p = base + f3 * (sx * (hw - 0.05)) + fr * fz;
            cyl(mb, p, p + Vec3::Y * h, 0.05, 0.045, vec3(0.38, 0.29, 0.20), 6);
        }
    }
}

fn fuel_can(mb: &mut MB, p: Vec3, yaw: f32) {
    let f = vec3(yaw.cos(), 0.0, yaw.sin());
    let r = vec3(-f.z, 0.0, f.x);
    bbox(mb, p + Vec3::Y * 0.22, f, r, vec3(0.17, 0.22, 0.08), vec3(0.72, 0.12, 0.10));
    cyl(mb, p + Vec3::Y * 0.44, p + Vec3::Y * 0.52, 0.035, 0.03, vec3(0.1, 0.1, 0.1), 6);
    crate::items::ring(mb, p + Vec3::Y * 0.5 + f * 0.08, r, 0.07, 0.012, vec3(0.1, 0.1, 0.1), 8);
    bbox(mb, p + Vec3::Y * 0.22 + f * 0.0, f, r, vec3(0.172, 0.04, 0.082), vec3(0.95, 0.9, 0.2));
}

fn build_stall(pool: &mut HashMap<(i32, i32), MB>, cols: &mut Vec<Col>, pos: Vec3, fr: Vec3, kind: usize) {
    let f3 = vec3(-fr.z, 0.0, fr.x);
    let wood = vec3(0.45, 0.34, 0.22);
    let top = vec3(0.56, 0.44, 0.30);
    let mb = pmb(pool, pos);
    // the counter
    bbox(mb, pos + Vec3::Y * 0.5, f3, fr, vec3(1.35, 0.5, 0.36), wood);
    bbox(mb, pos + Vec3::Y * 1.02, f3, fr, vec3(1.42, 0.04, 0.45), top);
    for k in 0..6 {
        let x = -1.2 + k as f32 * 0.48;
        bbox(mb, pos + f3 * x + fr * 0.37 + Vec3::Y * 0.5, f3, fr, vec3(0.005, 0.46, 0.005), wood * 0.6);
    }
    cols.push(Col::Obb { c: vec2(pos.x, pos.z), f: vec2(f3.x, f3.z), hx: 1.45, hz: 0.5, y0: pos.y - 1.0, top: pos.y + 1.05, walk: false });
    for sx in [-1.0f32, 1.0] {
        let p = pos + f3 * (sx * 1.65) + fr * 1.2;
        cols.push(Col::Circle { c: vec2(p.x, p.z), r: 0.1, y0: pos.y - 1.0, top: pos.y + 2.4, walk: false });
        let p = pos + f3 * (sx * 1.65) - fr * 0.9;
        cols.push(Col::Circle { c: vec2(p.x, p.z), r: 0.1, y0: pos.y - 1.0, top: pos.y + 2.9, walk: false });
    }
    match kind {
        0 => {
            // the boat yard: red and white awning, fuel cans, an engine on a stand, oars and rope
            awning(mb, pos, f3, fr, vec3(0.78, 0.16, 0.14), vec3(0.92, 0.9, 0.84));
            for i in 0..4 {
                let p = pos + f3 * (-2.1 + (i % 2) as f32 * 0.5) + fr * (0.3 + (i / 2) as f32 * 0.5);
                fuel_can(mb, p, rf(0.0, TAU));
                cols.push(Col::Circle { c: vec2(p.x, p.z), r: 0.22, y0: pos.y - 1.0, top: pos.y + 0.5, walk: false });
            }
            let e = pos + f3 * 2.3 + fr * 0.4;
            cyl(mb, e, e + Vec3::Y * 0.5, 0.05, 0.05, vec3(0.3, 0.3, 0.3), 6);
            for k in 0..3 {
                let a = TAU * k as f32 / 3.0;
                cyl(mb, e + Vec3::Y * 0.02, e + vec3(a.cos() * 0.4, 0.0, a.sin() * 0.4), 0.025, 0.02, vec3(0.3, 0.3, 0.3), 5);
            }
            blob(mb, e + Vec3::Y * 0.78, vec3(0.2, 0.26, 0.17), vec3(0.74, 0.14, 0.11), 0.0, 9, 6, 0.3, shade);
            cyl(mb, e + Vec3::Y * 0.62, e + Vec3::Y * 0.2, 0.05, 0.045, vec3(0.12, 0.12, 0.13), 6);
            blob(mb, e + Vec3::Y * 0.17, vec3(0.13, 0.06, 0.06), vec3(0.7, 0.12, 0.1), 0.0, 8, 5, 0.3, shade);
            cols.push(Col::Circle { c: vec2(e.x, e.z), r: 0.42, y0: pos.y - 1.0, top: pos.y + 1.0, walk: false });
            for k in 0..2 {
                let a = pos - f3 * (1.85 + k as f32 * 0.08) - fr * 0.6;
                cyl(mb, a, a + vec3(0.2 * (k as f32 - 0.5), 2.0, 0.0), 0.022, 0.022, vec3(0.62, 0.48, 0.30), 6);
            }
            rope_coil(mb, pos + f3 * 1.0 + fr * 1.2);
            // a lure board on the front of the counter
            for k in 0..5 {
                let p = pos + f3 * (-0.8 + k as f32 * 0.4) + fr * 0.42 + Vec3::Y * 0.9;
                blob(mb, p, vec3(0.025, 0.07, 0.015), vec3(0.85, 0.7, 0.2) * rf(0.8, 1.1), 0.0, 6, 4, 0.0, shade);
            }
        }
        _ => {
            // the salvage dealer: dark green and gold awning, shelves, jars, lanterns, flare crates
            awning(mb, pos, f3, fr, vec3(0.13, 0.30, 0.22), vec3(0.78, 0.66, 0.30));
            for lvl in 0..3 {
                bbox(mb, pos - fr * 0.9 + Vec3::Y * (0.5 + lvl as f32 * 0.62), f3, fr, vec3(1.3, 0.025, 0.2), wood);
            }
            for sx in [-1.0f32, 1.0] {
                bbox(mb, pos - fr * 0.9 + f3 * (sx * 1.3) + Vec3::Y * 1.0, f3, fr, vec3(0.03, 1.0, 0.2), wood * 0.8);
            }
            for lvl in 0..3 {
                for k in 0..5 {
                    let p = pos - fr * 0.9 + f3 * (-1.05 + k as f32 * 0.52) + Vec3::Y * (0.53 + lvl as f32 * 0.62);
                    let jar = vec3(0.30, 0.5, 0.42) * rf(0.7, 1.2);
                    cyl(mb, p, p + Vec3::Y * rf(0.16, 0.26), 0.07, 0.065, jar, 8);
                }
            }
            for k in 0..3 {
                let p = pos + f3 * (-1.0 + k as f32 * 1.0) + fr * 0.2 + Vec3::Y * 1.06;
                crate::items::lantern_model(mb, p, 0.9, 0.0, false);
            }
            // a crate of flares
            let c = pos + f3 * 2.2 + fr * 0.4;
            crate_box(mb, c, 0.2, 0.62, vec3(0.30, 0.34, 0.26));
            for k in 0..5 {
                let o = f3 * ((k as f32 - 2.0) * 0.1);
                cyl(mb, c + o + Vec3::Y * 0.55, c + o + Vec3::Y * 0.75, 0.032, 0.03, vec3(0.9, 0.35, 0.08), 7);
                cyl(mb, c + o + Vec3::Y * 0.75, c + o + Vec3::Y * 0.78, 0.034, 0.034, vec3(0.1, 0.1, 0.1), 7);
            }
            cols.push(Col::Circle { c: vec2(c.x, c.z), r: 0.45, y0: pos.y - 1.0, top: pos.y + 0.8, walk: false });
            barrel(mb, pos - f3 * 2.2 + fr * 0.2, 0.85, vec3(0.30, 0.34, 0.30));
            let b = pos - f3 * 2.2 + fr * 0.2;
            cols.push(Col::Circle { c: vec2(b.x, b.z), r: 0.34, y0: pos.y - 1.0, top: pos.y + 0.85, walk: false });
        }
    }
}

/// Wrecked camp of the people who were here before: a collapsed tent, a cold fire, a broken crate
/// and a few bones.
fn build_camp(pool: &mut HashMap<(i32, i32), MB>, cols: &mut Vec<Col>, c: Vec3, fr: Vec3, bones: bool) -> [Vec3; 3] {
    let f3 = vec3(-fr.z, 0.0, fr.x);
    let canvas = vec3(0.46, 0.45, 0.34);
    let mb = pmb(pool, c);
    // the half-collapsed tent: one pole still up, canvas slumped over it
    let pole = c + Vec3::Y * 1.5 + f3 * 0.2;
    cyl(mb, c + f3 * 0.2, pole, 0.04, 0.035, vec3(0.38, 0.30, 0.22), 5);
    let corners = [c + f3 * 1.5 + fr * 1.2, c - f3 * 1.3 + fr * 1.4, c - f3 * 1.2 - fr * 1.4, c + f3 * 1.6 - fr * 1.1];
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        let mut n = (b - pole).cross(a - pole).normalize_or_zero();
        if n.y < 0.0 {
            n = -n;
        }
        mb.nquad(a, b, b + (pole - b) * 0.15, a + (pole - a) * 0.15, canvas * (0.85 + 0.1 * i as f32), n);
        mb.ftri(a + (pole - a) * 0.15, b + (pole - b) * 0.15, pole, canvas * (0.8 + 0.08 * i as f32));
    }
    // a rip in the side, a snapped pole lying in the sand and a scrap of canvas
    mb.ftri(corners[1] + Vec3::Y * 0.02, corners[2] + Vec3::Y * 0.02, (corners[1] + corners[2]) * 0.5 + fr * 0.5 + Vec3::Y * 0.02, canvas * 0.6);
    cyl(mb, c + f3 * 2.4 + fr * 0.3 + Vec3::Y * 0.05, c + f3 * 3.5 - fr * 0.7 + Vec3::Y * 0.08, 0.04, 0.035, vec3(0.36, 0.28, 0.20), 5);
    cols.push(Col::Obb { c: vec2(c.x, c.z), f: vec2(f3.x, f3.z), hx: 1.5, hz: 1.4, y0: c.y - 1.0, top: c.y + 0.7, walk: false });
    // the cold fire
    let fire = c + fr * 3.6 + f3 * 0.6;
    for i in 0..9 {
        let a = TAU * i as f32 / 9.0;
        let p = fire + vec3(a.cos() * 0.55, 0.0, a.sin() * 0.55);
        blob(mb, p + Vec3::Y * 0.08, vec3(0.16, 0.11, 0.14), vec3(0.34, 0.32, 0.30) * rf(0.8, 1.1), 0.2, 6, 4, a, shade);
    }
    cyl(mb, fire + Vec3::Y * 0.01, fire + Vec3::Y * 0.04, 0.45, 0.45, vec3(0.04, 0.04, 0.04), 10);
    for i in 0..4 {
        let a = i as f32 * 1.6;
        let d = vec3(a.cos(), 0.0, a.sin());
        cyl(mb, fire + d * 0.35 + Vec3::Y * 0.1, fire - d * 0.1 + Vec3::Y * 0.2, 0.06, 0.05, vec3(0.06, 0.05, 0.05), 6);
    }
    cols.push(Col::Circle { c: vec2(fire.x, fire.z), r: 0.7, y0: c.y - 1.0, top: c.y + 0.3, walk: false });
    // a broken crate, loose planks and a barrel
    let cr = c - f3 * 2.8 + fr * 1.2;
    crate_box(mb, cr, 0.4, 0.62, vec3(0.40, 0.33, 0.24));
    beam(mb, cr + vec3(0.6, 0.04, 0.2), cr + vec3(1.4, 0.06, -0.3), 0.14, 0.03, vec3(0.38, 0.30, 0.22));
    beam(mb, cr + vec3(0.3, 0.04, -0.6), cr + vec3(-0.5, 0.06, -1.1), 0.14, 0.03, vec3(0.36, 0.29, 0.21));
    cols.push(Col::Circle { c: vec2(cr.x, cr.z), r: 0.45, y0: c.y - 1.0, top: c.y + 0.7, walk: false });
    let br = c + f3 * 3.2 + fr * 2.6;
    barrel(mb, br, 0.8, vec3(0.30, 0.27, 0.22));
    cols.push(Col::Circle { c: vec2(br.x, br.z), r: 0.34, y0: c.y - 1.0, top: c.y + 0.8, walk: false });
    if bones {
        let b = c + fr * 2.3 - f3 * 1.8;
        let bone = vec3(0.86, 0.83, 0.74);
        blob(mb, b + Vec3::Y * 0.1, vec3(0.085, 0.095, 0.1), bone, 0.0, 7, 5, 0.5, shade);
        blob(mb, b + Vec3::Y * 0.05 + f3 * 0.0 + fr * 0.07, vec3(0.06, 0.04, 0.05), bone * 0.95, 0.0, 6, 4, 0.5, shade);
        for sx in [-0.035f32, 0.035] {
            blob(mb, b + Vec3::Y * 0.12 + fr * 0.075 + f3 * sx, Vec3::splat(0.022), vec3(0.04, 0.03, 0.03), 0.0, 5, 3, 0.0, shade);
        }
        for k in 0..6 {
            let a = k as f32 * 1.05;
            let d = vec3(a.cos(), 0.0, a.sin());
            cyl(mb, b + d * 0.25 + Vec3::Y * 0.025, b + d * (0.25 + rf(0.18, 0.3)) + Vec3::Y * 0.03, 0.014, 0.012, bone * 0.95, 4);
        }
    }
    // where the pages lie: on the crate, by the cold fire and under the tent canvas
    [cr + Vec3::Y * 0.64, fire + f3 * 0.9 + Vec3::Y * 0.03, c + f3 * 1.0 + fr * 1.7 + Vec3::Y * 0.03]
}

/// A weathered board stuck in the sand, covered in scratches (a warning).
fn carved_board(pool: &mut HashMap<(i32, i32), MB>, cols: &mut Vec<Col>, p: Vec3, fr: Vec3) {
    let f3 = vec3(-fr.z, 0.0, fr.x);
    let mb = pmb(pool, p);
    let wood = vec3(0.42, 0.37, 0.30);
    bbox(mb, p + Vec3::Y * 0.85, f3, fr, vec3(0.55, 0.85, 0.04), wood);
    for k in 0..14 {
        let (x, y) = (rf(-0.45, 0.45), rf(0.2, 1.5));
        let a = rf(-0.6, 0.6);
        let d = f3 * a.cos() + Vec3::Y * a.sin();
        cyl(mb, p + f3 * x + Vec3::Y * y + fr * 0.045 - d * 0.1, p + f3 * x + Vec3::Y * y + fr * 0.045 + d * 0.1, 0.006, 0.006, vec3(0.08, 0.06, 0.05), 3);
        let _ = k;
    }
    cols.push(Col::Obb { c: vec2(p.x, p.z), f: vec2(f3.x, f3.z), hx: 0.58, hz: 0.1, y0: p.y - 1.0, top: p.y + 1.7, walk: false });
}

pub fn build_islands(pool: &mut HashMap<(i32, i32), MB>, t: &Terrain, player_tent: Vec3) -> Islands {
    let mut cols = vec![];
    let mut vendors = vec![];
    let mut pages = vec![];
    let isle = |i: usize| vec2(ISLES[i].0, ISLES[i].1);
    let to_origin = |p: Vec3| dir_to(p, Vec3::ZERO);
    let mut taken: Vec<Vec3> = vec![];

    // ---- island 1 (east): the boat yard and a wrecked camp ----
    if let Some(p) = flat_spot(t, isle(1), 14.0, 1.0, 7.0, &[], 0.0) {
        let fr = to_origin(p);
        build_stall(pool, &mut cols, p, fr, 0);
        vendors.push(Vendor {
            id: 0,
            name: "Dunmore",
            pos: p,
            trade: p + fr * 1.5,
            yaw: fr.z.atan2(fr.x),
            person: p - fr * 0.95,
            look: Look { female: false, skin: 2, eyes: 3, hair_style: 3, hair_color: 3, beard: 2, top_style: 0, top_color: 4, sleeve_color: 4, bottom_style: 0, bottom_color: 12, shoe_style: 0, hat: 2 },
            greeting: "Boats are my trade. Engines, tanks, fuel and the better rods. Mind the deep water.",
            night_line: "You're out at night? Fine. Quick, then. I don't like the water this late.",
            stock: BOATYARD,
        });
        taken.push(p);
    }
    if let Some(p) = flat_spot(t, isle(1), 24.0, 0.8, 7.0, &taken, 14.0) {
        let fr = rand_dir();
        let pts = build_camp(pool, &mut cols, p, fr, true);
        for (i, id) in [0usize, 1, 3].iter().enumerate() {
            pages.push(PageSite { id: *id, pos: pts[i] });
        }
        taken.push(p);
    }
    // ---- island 2 (west): the salvage dealer, a camp and the carved rock ----
    let mut taken2: Vec<Vec3> = vec![];
    if let Some(p) = flat_spot(t, isle(2), 10.0, 1.0, 7.0, &[], 0.0) {
        let fr = to_origin(p);
        build_stall(pool, &mut cols, p, fr, 1);
        vendors.push(Vendor {
            id: 1,
            name: "Old Mara",
            pos: p,
            trade: p + fr * 1.5,
            yaw: fr.z.atan2(fr.x),
            person: p - fr * 0.95,
            look: Look { female: true, skin: 1, eyes: 6, hair_style: 7, hair_color: 6, beard: 0, top_style: 3, top_color: 9, sleeve_color: 9, bottom_style: 3, bottom_color: 13, shoe_style: 3, hat: 1 },
            greeting: "Salvage from the wrecks. Hull plate, lamps, flares. Things that keep you alive.",
            night_line: "Lights low, dear. Keep your voice down. They listen.",
            stock: SALVAGE,
        });
        taken2.push(p);
    }
    if let Some(p) = flat_spot(t, isle(2), 16.0, 0.8, 7.0, &taken2, 11.0) {
        let fr = rand_dir();
        let pts = build_camp(pool, &mut cols, p, fr, false);
        for (i, id) in [2usize, 4].iter().enumerate() {
            pages.push(PageSite { id: *id, pos: pts[i] });
        }
        // the scratched board stands on dry ground a little way from the camp
        let mut near_camp = taken2.clone();
        near_camp.push(p);
        let board = flat_spot(t, vec2(p.x, p.z), 14.0, 1.0, 7.0, &near_camp, 5.0).unwrap_or(p + fr * 3.0);
        let board = vec3(board.x, t.at(board.x, board.z), board.z);
        carved_board(pool, &mut cols, board, -fr);
        let b = board + fr * 0.7;
        pages.push(PageSite { id: 7, pos: vec3(b.x, t.at(b.x, b.z) + 0.03, b.z) });
    }
    // ---- the main island: a far cove, and a page left in your own tent ----
    {
        let away = vec2(PLAZA_C.0, PLAZA_C.1);
        let far = away + vec2(1.0, 0.8).normalize() * 52.0;
        if let Some(p) = flat_spot(t, far, 14.0, 0.8, 6.0, &[], 0.0) {
            let fr = rand_dir();
            let pts = build_camp(pool, &mut cols, p, fr, true);
            pages.push(PageSite { id: 5, pos: pts[1] });
        }
        let q = player_tent + vec3(1.5, 0.0, 1.5);
        pages.push(PageSite { id: 6, pos: vec3(q.x, t.at(q.x, q.z) + 0.03, q.z) });
    }
    Islands { vendors, pages, cols }
}

fn rand_dir() -> Vec3 {
    let a = rf(0.0, TAU);
    vec3(a.cos(), 0.0, a.sin())
}

/// A page of paper on the ground, curling a little, with a faint pale gleam so it can be found.
pub fn draw_page(mb: &mut MB, p: Vec3, time: f32, found: bool) {
    let h = (p.x * 3.1 + p.z * 1.7).sin() * PI;
    let f = vec3(h.cos(), 0.0, h.sin());
    let r = vec3(-f.z, 0.0, f.x);
    let col = if found { vec3(0.62, 0.60, 0.54) } else { vec3(0.93, 0.91, 0.82) };
    let curl = Vec3::Y * 0.012;
    let (a, b, c, d) = (p - f * 0.11 - r * 0.08, p + f * 0.11 - r * 0.08 + curl, p + f * 0.11 + r * 0.08, p - f * 0.11 + r * 0.08 + curl);
    mb.ftri(a, b, c, col);
    mb.ftri(a, c, d, col * 0.97);
    for k in 0..5 {
        let y = -0.06 + k as f32 * 0.03;
        cyl(mb, p + f * -0.08 + r * y + Vec3::Y * 0.004, p + f * 0.07 + r * y + Vec3::Y * 0.004, 0.003, 0.003, vec3(0.2, 0.18, 0.16), 3);
    }
    if !found {
        let g = 0.5 + 0.5 * (time * 2.0 + p.x).sin();
        blob(mb, p + Vec3::Y * (0.25 + 0.04 * g), Vec3::splat(0.012 + 0.006 * g), vec3(1.0, 0.95, 0.75), 0.0, 5, 3, 0.0, |c, _| glow(c));
    }
}

/// A trader standing behind the counter.
pub fn draw_vendor(mb: &mut MB, v: &Vendor, eye: Vec3, time: f32) {
    use crate::people::{draw_person, Pose};
    let to = vec2(eye.x - v.person.x, eye.z - v.person.z);
    let near = to.length() < 12.0;
    let yaw = if near { to.y.atan2(to.x) } else { v.yaw };
    draw_person(mb, v.person, &v.look, &Pose { yaw, phase: 0.0, walk: 0.0, item: None, tier: 0, lure: None, lit: false, eye, time, swim: false, surf: 0.0, sit: false, fish: None });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_traders_and_every_page_are_placed_on_dry_land() {
        let _g = crate::RAND_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        rand::srand(20_261_010);
        let t = Terrain::gen();
        let mut pool: HashMap<(i32, i32), MB> = HashMap::new();
        let plaza = build_plaza(&mut pool, &t);
        let isl = build_islands(&mut pool, &t, plaza.tent_pos);
        // the boat yard on the east island and the salvage dealer on the west one
        assert_eq!(isl.vendors.len(), 2, "a trader could not be placed");
        assert!(isl.vendors.iter().any(|v| v.name == "Dunmore" && v.stock == BOATYARD));
        assert!(isl.vendors.iter().any(|v| v.name == "Old Mara" && v.stock == SALVAGE));
        for v in &isl.vendors {
            assert!(t.at(v.pos.x, v.pos.z) > 0.5, "{} is standing in the water", v.name);
            assert!((v.trade - v.pos).length() > 1.0);
        }
        // all eight diary pages exist exactly once and lie on land
        let mut seen = [0u32; N_PAGES];
        for p in &isl.pages {
            seen[p.id] += 1;
            assert!(t.at(p.pos.x, p.pos.z) > 0.0, "page {} is under water", p.id);
        }
        assert!(seen.iter().all(|n| *n == 1), "pages placed: {seen:?}");
        // the boat yard sells the engine; the salvage dealer the armour
        assert!(BOATYARD.contains(&Good::Boat(crate::progress::ENGINE)));
        assert!(SALVAGE.contains(&Good::Boat(crate::progress::STEEL)));
        assert!(!isl.cols.is_empty());
    }
}
