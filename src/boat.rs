//! The rowing boat: physics (paddle strokes, a motor once you have the key), shore collision and
//! a detailed wooden model with oars that really move.

use super::*;

pub struct Boat {
    /// waterline position (y follows the swell)
    pub pos: Vec3,
    pub yaw: f32,
    pub speed: f32,
    turn: f32,
    pub row_t: f32,
    /// 0..1 how hard the oars are working
    pub row_amt: f32,
    pub pitch: f32,
    pub roll: f32,
    /// set while the engine is making power (for sound and the prop)
    pub engine: f32,
    /// extra rolling after a hit (a swing that dies away)
    pub rock: f32,
}

/// Height, pitch and roll of a boat floating at (x, z) heading `yaw`.
pub fn attitude(t: &Terrain, x: f32, z: f32, yaw: f32, time: f32) -> (f32, f32, f32) {
    let f = vec2(yaw.cos(), yaw.sin());
    let r = vec2(-yaw.sin(), yaw.cos());
    let h = |dx: f32, dz: f32| water_h(t, x + dx, z + dz, time);
    let (fb, bb) = (h(f.x * 1.4, f.y * 1.4), h(-f.x * 1.4, -f.y * 1.4));
    let (rb, lb) = (h(r.x * 0.7, r.y * 0.7), h(-r.x * 0.7, -r.y * 0.7));
    let y = (fb + bb + rb + lb) * 0.25;
    ((y + 0.02).max(-0.05), ((fb - bb) / 2.8).atan(), ((rb - lb) / 1.4).atan())
}

impl Boat {
    pub fn new(pos: Vec3, yaw: f32) -> Boat {
        Boat { pos, yaw, speed: 0.0, turn: 0.0, row_t: 0.0, row_amt: 0.0, pitch: 0.0, roll: 0.0, engine: 0.0, rock: 0.0 }
    }

    pub fn forward(&self) -> Vec3 {
        vec3(self.yaw.cos(), 0.0, self.yaw.sin())
    }

    /// Where the rider's feet go (the model's seated pose lowers the body by itself).
    pub fn seat(&self) -> Vec3 {
        self.pos - Vec3::Y * 0.14 - self.forward() * 0.1
    }

    /// `throttle` -1..1 (W/S), `steer` -1..1 (A/D, positive = right), `solid(x,z)` = can't float there.
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, dt: f32, throttle: f32, steer: f32, boost: bool, motor: bool, power: f32, t: &Terrain, time: f32, solid: &dyn Fn(f32, f32) -> bool) {
        let dt = dt.min(0.05);
        let working = throttle.abs() > 0.05;
        if motor {
            // outboard: strong steady push, much faster with boost
            let acc = (if boost { 7.0 } else { 4.2 }) * power;
            self.speed += throttle * acc * dt;
            self.speed -= self.speed * 0.45 * dt;
            self.speed = self.speed.clamp(-3.0, (if boost { 11.5 } else { 7.5 }) * power);
            self.engine += ((if working || self.speed.abs() > 0.5 { 1.0 } else { 0.25 }) - self.engine) * (dt * 3.0).min(1.0);
            self.row_amt += (0.0 - self.row_amt) * (dt * 4.0).min(1.0);
        } else {
            // paddling: each stroke gives a push, so the boat surges along
            if working {
                self.row_t += dt * 4.4;
            } else {
                self.row_t += (self.row_t.rem_euclid(TAU) - self.row_t) * 0.0; // keep phase
            }
            let push = if throttle > 0.0 { 0.5 + 3.0 * self.row_t.sin().max(0.0) } else { 0.8 + 1.4 * self.row_t.sin().max(0.0) };
            self.speed += throttle * push * dt;
            self.speed -= self.speed * 0.60 * dt;
            self.speed = self.speed.clamp(-1.8, 3.4);
            self.row_amt += ((if working { 1.0 } else { 0.0 }) - self.row_amt) * (dt * 5.0).min(1.0);
            self.engine += (0.0 - self.engine) * (dt * 5.0).min(1.0);
        }
        // steering: sharper once the boat is moving, but you can still spin slowly on the spot
        let mv = (self.speed.abs() / 3.0).min(1.0);
        let want = steer * (0.45 + 0.75 * mv) * if self.speed < -0.2 { -1.0 } else { 1.0 };
        self.turn += (want - self.turn) * (dt * 3.5).min(1.0);
        self.yaw += self.turn * dt;
        // move, sliding along the shore instead of sticking to it
        let f = vec2(self.yaw.cos(), self.yaw.sin());
        let step = f * self.speed * dt;
        let ok = |x: f32, z: f32| -> bool {
            if x.abs() > SWIM_LIMIT || z.abs() > SWIM_LIMIT {
                return false;
            }
            for (a, b) in [(0.0f32, 0.0f32), (1.6, 0.0), (-1.6, 0.0), (0.0, 0.6), (0.0, -0.6)] {
                let p = vec2(x, z) + f * a + vec2(-f.y, f.x) * b;
                if solid(p.x, p.y) {
                    return false;
                }
            }
            true
        };
        let (nx, nz) = (self.pos.x + step.x, self.pos.z + step.y);
        if ok(nx, nz) {
            self.pos.x = nx;
            self.pos.z = nz;
        } else if ok(nx, self.pos.z) {
            self.pos.x = nx;
            self.speed *= 0.97;
        } else if ok(self.pos.x, nz) {
            self.pos.z = nz;
            self.speed *= 0.97;
        } else {
            self.speed *= -0.25;
        }
        self.rock -= self.rock * (dt * 0.9).min(1.0);
        let (y, p, r) = attitude(t, self.pos.x, self.pos.z, self.yaw, time);
        self.pos.y = y;
        self.pitch += (p - self.pitch) * (dt * 4.0).min(1.0);
        self.roll += (r - self.roll) * (dt * 4.0).min(1.0);
    }
}

/// Where to moor the boat at the start: in the water beside the dock, away from the posts.
pub fn start_boat(t: &Terrain, dock: &Dock) -> Boat {
    let yaw = dock.dir.y.atan2(dock.dir.x);
    for k in 0..40 {
        let s = dock.len * 0.55 + k as f32 * 0.8;
        for lat in [3.4f32, -3.4] {
            let p = dock.point(s, lat, 0.0);
            let ok = [(0.0f32, 0.0f32), (1.8, 0.0), (-1.8, 0.0), (0.0, 1.0), (0.0, -1.0)].iter().all(|(a, b)| {
                let q = vec2(p.x, p.z) + dock.dir * *a + vec2(-dock.dir.y, dock.dir.x) * *b;
                t.at(q.x, q.y) < -1.5 && !dock.near(q.x, q.y, 0.9)
            });
            if ok {
                return Boat::new(vec3(p.x, 0.0, p.z), yaw);
            }
        }
    }
    let p = dock.point(dock.len + 4.0, 3.4, 0.0);
    Boat::new(vec3(p.x, 0.0, p.z), yaw)
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------
pub struct BoatView {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub row_t: f32,
    pub row_amt: f32,
    pub motor: bool,
    /// engine power 0..1 (prop spins)
    pub engine: f32,
    /// somebody is in it (oars out) or not (oars shipped)
    pub manned: bool,
    pub time: f32,
}

struct Frame {
    o: Vec3,
    f: Vec3,
    u: Vec3,
    r: Vec3,
}
impl Frame {
    fn p(&self, x: f32, y: f32, z: f32) -> Vec3 {
        self.o + self.f * x + self.u * y + self.r * z
    }
    fn v(&self, d: Vec3) -> Vec3 {
        self.f * d.x + self.u * d.y + self.r * d.z
    }
}

fn tri_out(mb: &mut MB, a: Vec3, b: Vec3, c: Vec3, col: Vec3, cen: Vec3, flip: bool) {
    let mut n = (b - a).cross(c - a).normalize_or_zero();
    let out = (a + b + c) / 3.0 - cen;
    if (n.dot(out) < 0.0) != flip {
        n = -n;
    }
    let k = shade(col, n);
    mb.tri(a, b, c, k, k, k);
}

fn quad_out(mb: &mut MB, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: Vec3, cen: Vec3, flip: bool) {
    tri_out(mb, a, b, c, col, cen, flip);
    tri_out(mb, a, c, d, col, cen, flip);
}

fn plan(t: f32) -> f32 {
    // half-beam factor along the hull: pointed bow, full midship, blunt stern
    if t < 0.45 {
        ((t / 0.45) * PI * 0.5).sin().powf(0.8)
    } else {
        1.0 - 0.52 * smooth(0.55, 1.0, t)
    }
}

pub fn draw_boat(mb: &mut MB, v: &BoatView) {
    mb.ensure(9000, 22000);
    let f0 = vec3(v.yaw.cos(), 0.0, v.yaw.sin());
    let r0 = vec3(-v.yaw.sin(), 0.0, v.yaw.cos());
    let (sp, cp) = v.pitch.sin_cos();
    let fwd = f0 * cp + Vec3::Y * sp;
    let up0 = Vec3::Y * cp - f0 * sp;
    let (sr, cr) = v.roll.sin_cos();
    let fr = Frame { o: v.pos, f: fwd, u: up0 * cr - r0 * sr, r: r0 * cr + up0 * sr };

    let (l, beam) = (3.7f32, 0.74f32);
    let wood_a = vec3(0.50, 0.34, 0.19);
    let wood_b = vec3(0.43, 0.29, 0.16);
    let paint = vec3(0.13, 0.33, 0.46);
    let inner = vec3(0.58, 0.43, 0.26);
    let rail = vec3(0.66, 0.50, 0.30);
    let n_st = 15usize;
    let n_pt = 9usize;
    let gun = |t: f32| 0.40 + 0.14 * (1.0 - t).powi(3) + 0.03 * t;
    let keel = |t: f32| -0.24 + 0.30 * (1.0 - t).powi(2) + 0.12 * smooth(0.82, 1.0, t);
    // hull sections: (outer, inner) points from starboard gunwale round the keel to port
    let section = |k: usize, shrink: f32| -> Vec<Vec3> {
        let t = k as f32 / (n_st - 1) as f32;
        let x = l * (0.5 - t);
        let b = (beam * plan(t) - shrink).max(0.0);
        let (g, kl) = (gun(t), keel(t) + shrink);
        (0..n_pt)
            .map(|j| {
                let phi = -PI * 0.5 + PI * j as f32 / (n_pt - 1) as f32;
                let s = phi.sin();
                let z = b * s.signum() * s.abs().powf(0.85);
                let y = g - (g - kl) * phi.cos().powf(0.9);
                fr.p(x, y, z)
            })
            .collect()
    };
    let outer: Vec<Vec<Vec3>> = (0..n_st).map(|k| section(k, 0.0)).collect();
    let innr: Vec<Vec<Vec3>> = (0..n_st).map(|k| section(k, 0.045)).collect();
    let cen = fr.p(0.0, 0.1, 0.0);
    for k in 0..n_st - 1 {
        for j in 0..n_pt - 1 {
            let band = if j == 0 || j == n_pt - 2 { paint } else if (j + k / 3) % 2 == 0 { wood_a } else { wood_b };
            quad_out(mb, outer[k][j], outer[k + 1][j], outer[k + 1][j + 1], outer[k][j + 1], band * (0.94 + 0.1 * hash2(k as i32, j as i32, 4)), cen, false);
            quad_out(mb, innr[k][j], innr[k + 1][j], innr[k + 1][j + 1], innr[k][j + 1], inner * (0.92 + 0.12 * hash2(k as i32, j as i32, 5)), cen, true);
        }
        // gunwale cap joins the outer and inner rims on both sides
        for e in [0usize, n_pt - 1] {
            let up_n = fr.u;
            let kc = shade(rail, up_n);
            mb.quadc(outer[k][e], outer[k + 1][e], innr[k + 1][e], innr[k][e], kc, kc, kc, kc);
        }
    }
    // stern transom (outer and inner) and the pointed bow stem
    {
        let last = n_st - 1;
        let c = (outer[last][0] + outer[last][n_pt - 1]) * 0.5 - fr.u * 0.1;
        let kn = shade(wood_b * 1.05, -fr.f);
        let ki = shade(inner * 0.9, fr.f);
        for j in 0..n_pt - 1 {
            mb.tri(c, outer[last][j], outer[last][j + 1], kn, kn, kn);
            let ci = (innr[last][0] + innr[last][n_pt - 1]) * 0.5 - fr.u * 0.1;
            mb.tri(ci, innr[last][j], innr[last][j + 1], ki, ki, ki);
        }
        let bow_top = fr.p(l * 0.5 + 0.05, gun(0.0) + 0.12, 0.0);
        let bow_bot = fr.p(l * 0.5 - 0.02, keel(0.0) + 0.02, 0.0);
        cyl(mb, bow_bot, bow_top, 0.035, 0.03, wood_b * 0.8, 6);
        blob(mb, bow_top, Vec3::splat(0.04), rail, 0.0, 6, 4, 0.0, shade);
    }
    // floor boards
    for i in 0..6 {
        let x0 = -1.35 + i as f32 * 0.42;
        let t0 = 0.5 - x0 / l;
        let w = (beam * plan(t0) - 0.12).max(0.1) * 0.82;
        let col = if i % 2 == 0 { inner * 0.8 } else { inner * 0.68 };
        let (a, b, c, d) = (fr.p(x0, -0.13, -w), fr.p(x0 + 0.38, -0.13, -w), fr.p(x0 + 0.38, -0.13, w), fr.p(x0, -0.13, w));
        let kc = shade(col, fr.u);
        mb.quadc(a, b, c, d, kc, kc, kc, kc);
    }
    // three thwarts (seats) with little brackets
    for sx in [0.95f32, -0.12, -1.15] {
        let t = 0.5 - sx / l;
        let w = beam * plan(t) - 0.04;
        let top = 0.24;
        let c = fr.p(sx, top - 0.02, 0.0);
        bbox(mb, c, fr.f, fr.r, vec3(0.13, 0.022, w), rail * 0.95);
        for s in [-1.0f32, 1.0] {
            cyl(mb, fr.p(sx, top - 0.04, s * (w - 0.05)), fr.p(sx, top - 0.20, s * (w - 0.02)), 0.014, 0.014, wood_b, 5);
        }
    }
    // oarlocks and oars
    let manned = v.manned;
    for s in [-1.0f32, 1.0] {
        let t = 0.5 - (-0.12) / l;
        let b = beam * plan(t);
        let pv = fr.p(-0.12, gun(t) + 0.03, s * (b + 0.04));
        cyl(mb, pv - fr.u * 0.03, pv + fr.u * 0.07, 0.012, 0.012, vec3(0.3, 0.3, 0.32), 5);
        ring_lock(mb, pv + fr.u * 0.07, fr.f, 0.03);
        // oar direction in boat space: out to the side, swung fore and aft by the stroke
        let stroke = v.row_t.cos();
        let swing = if manned { 0.20 + 0.55 * stroke * v.row_amt } else { 0.95 };
        let lift = if manned { 0.0 + 0.30 * v.row_t.sin().max(0.0) * v.row_amt } else { 0.55 };
        let dir = vec3(swing.sin(), -0.26 + lift * 1.2, s * swing.cos()).normalize();
        let blade_c = pv + fr.v(dir) * 1.55;
        let grip = pv - fr.v(dir) * 0.55;
        cyl(mb, grip, blade_c, 0.022, 0.026, vec3(0.62, 0.48, 0.30), 6);
        cyl(mb, grip, grip + fr.v(dir) * 0.14, 0.03, 0.03, vec3(0.16, 0.12, 0.09), 6);
        // blade: flat paddle shape lying along the oar
        let ax = fr.v(dir);
        let side = ax.cross(fr.u).normalize_or_zero();
        let bc = blade_c + ax * 0.22;
        let hb = (side * 0.075, ax * 0.24);
        let kb = shade(vec3(0.70, 0.55, 0.34), fr.u);
        let kb2 = shade(vec3(0.55, 0.42, 0.26), -fr.u);
        let (a, b2, c2, d2) = (bc - hb.0 - hb.1, bc + hb.0 - hb.1, bc + hb.0 + hb.1, bc - hb.0 + hb.1);
        mb.quadc(a, b2, c2, d2, kb, kb, kb, kb);
        mb.quadc(a, d2, c2, b2, kb2, kb2, kb2, kb2);
        // a light drip line when the blade is out of the water
        let _ = (a, d2);
    }
    // bow rope coil and a bailer
    blob(mb, fr.p(1.35, -0.07, 0.18), vec3(0.13, 0.05, 0.13), vec3(0.62, 0.52, 0.34), 0.0, 8, 4, v.yaw, shade);
    blob(mb, fr.p(-1.55, -0.06, -0.25), vec3(0.09, 0.07, 0.09), vec3(0.60, 0.15, 0.12), 0.0, 6, 4, v.yaw, shade);
    // outboard motor (once you have the key)
    if v.motor {
        let red = vec3(0.70, 0.12, 0.10);
        let dark = vec3(0.10, 0.10, 0.12);
        let mx = -l * 0.5 - 0.06;
        bbox(mb, fr.p(mx + 0.06, 0.33, 0.0), fr.f, fr.r, vec3(0.05, 0.10, 0.12), dark);
        blob(mb, fr.p(mx - 0.08, 0.50, 0.0), vec3(0.19, 0.17, 0.13), red, 0.0, 9, 6, v.yaw, shade);
        cyl(mb, fr.p(mx - 0.08, 0.40, 0.0), fr.p(mx - 0.10, -0.32, 0.0), 0.045, 0.04, dark * 1.4, 7);
        blob(mb, fr.p(mx - 0.10, -0.36, 0.0), vec3(0.15, 0.065, 0.065), red * 0.9, 0.0, 8, 5, v.yaw, shade);
        // propeller spins with the engine
        let a = v.time * (6.0 + 40.0 * v.engine);
        for k in 0..3 {
            let ang = a + k as f32 * TAU / 3.0;
            let tip = fr.p(mx - 0.26, -0.36 + ang.sin() * 0.11, ang.cos() * 0.11);
            cyl(mb, fr.p(mx - 0.24, -0.36, 0.0), tip, 0.012, 0.006, vec3(0.7, 0.7, 0.72), 4);
        }
        // tiller handle reaching into the boat
        cyl(mb, fr.p(mx - 0.02, 0.50, 0.0), fr.p(mx + 0.75, 0.46, 0.10), 0.016, 0.014, dark, 5);
        blob(mb, fr.p(mx + 0.78, 0.46, 0.10), Vec3::splat(0.03), dark, 0.0, 6, 4, 0.0, shade);
    }
}

fn ring_lock(mb: &mut MB, c: Vec3, axis: Vec3, r: f32) {
    let u = axis.cross(Vec3::Y).normalize_or_zero();
    let mut prev = c + u * r;
    for i in 1..=8 {
        let a = TAU * i as f32 / 8.0;
        let p = c + (u * a.cos() + Vec3::Y * a.sin()) * r;
        cyl(mb, prev, p, 0.006, 0.006, vec3(0.32, 0.32, 0.34), 3);
        prev = p;
    }
}
