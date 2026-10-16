//! The horror layer: foggy and blood-moon nights, sanity, hallucinations and the night stalkers.
//! Everything here is plain logic (no drawing, no globals) so it can be tested; the game loop feeds
//! it the situation and reacts to what it reports.

use super::*;

// ---------------------------------------------------------------------------
// Nights: fog and blood moons
// ---------------------------------------------------------------------------
pub struct Weather {
    /// 0..1 how thick the fog is
    pub fog: f32,
    /// 0..1 how red the moon is
    pub blood: f32,
    /// 0..1 how deep into the night it is
    pub night: f32,
    /// which day this is (0 = the first day)
    pub day: i64,
}

fn day_hash(day: i64, salt: u32) -> u32 {
    let mut x = (day as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (salt as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 32;
    x as u32
}

/// The night that follows day `day` has a blood moon (never the first two nights).
pub fn blood_night(day: i64) -> bool {
    day >= 2 && (day == 4 || day_hash(day, 7) % 4 == 0)
}

/// The night that follows day `day` is foggy.
pub fn fog_night(day: i64) -> bool {
    day == 1 || day_hash(day, 3) % 3 == 0
}

/// Weather at game-clock time `t` (the shared sky clock, so every player sees the same).
pub fn weather_at(t: f32) -> Weather {
    let day = (t / CYCLE).floor() as i64;
    let tc = t - day as f32 * CYCLE;
    let night = smooth(DAY_LEN - 40.0, DAY_LEN + 30.0, tc) * (1.0 - smooth(CYCLE - 60.0, CYCLE, tc));
    let fog = if fog_night(day) { smooth(DAY_LEN - 100.0, DAY_LEN + 40.0, tc) * (1.0 - smooth(CYCLE - 90.0, CYCLE - 5.0, tc)) } else { 0.0 };
    let blood = if blood_night(day) { night } else { 0.0 };
    Weather { fog, blood, night, day }
}

/// Fog thickness multiplier for the shaders (1 = the normal haze; thick fog leaves a few metres).
pub fn fog_k(fog: f32) -> f32 {
    1.0 + fog * 2500.0
}

/// Greys the sky out in fog and turns the night blood red.
pub fn apply_weather(sky: &mut Sky, w: &Weather) {
    let night = 1.0 - sky.day_f;
    if w.fog > 0.0 {
        let fog_col = vec3(0.64, 0.67, 0.71).lerp(vec3(0.13, 0.15, 0.19), night);
        sky.horizon = sky.horizon.lerp(fog_col, w.fog * 0.92);
        sky.zenith = sky.zenith.lerp(fog_col * 0.8, w.fog * 0.8);
        sky.light_col *= 1.0 - 0.55 * w.fog;
    }
    if w.blood > 0.0 {
        let b = w.blood;
        sky.horizon = sky.horizon.lerp(vec3(0.36, 0.025, 0.03), b * 0.85);
        sky.zenith = sky.zenith.lerp(vec3(0.13, 0.004, 0.012), b * 0.85);
        let l = sky.light_col.length();
        sky.light_col = sky.light_col.lerp(vec3(1.0, 0.16, 0.13) * l * 0.9, b);
        sky.amb = sky.amb.lerp(vec3(0.24, 0.04, 0.05), b * 0.55);
    }
}

// ---------------------------------------------------------------------------
// Sanity
// ---------------------------------------------------------------------------
pub struct SanityIn {
    pub dt: f32,
    pub night: f32,
    pub fog: f32,
    pub blood: f32,
    /// in deep water, swimming or on a boat
    pub deep: bool,
    /// metres to the nearest hunting monster (a big number when there is none)
    pub stalker_dist: f32,
    /// carrying a lit lantern or sitting under a boat searchlight
    pub lit: bool,
    pub near_fire: bool,
    pub day_f: f32,
    /// another player is close by
    pub company: bool,
}

/// One step of the sanity meter (0..100). The dark, the deep and the hunters wear it down; daylight,
/// firelight and company bring it back.
pub fn sanity_step(s: f32, i: &SanityIn) -> f32 {
    let mut drain = 0.0;
    if i.night > 0.3 {
        drain += 0.06 * i.night * (1.0 + 0.8 * i.fog + 0.7 * i.blood);
        if !i.lit && !i.near_fire {
            drain += 0.05;
        }
    }
    if i.deep {
        drain += 0.22 * (1.0 + i.night);
    }
    if i.stalker_dist < 40.0 {
        drain += 0.6 * (1.0 - i.stalker_dist / 40.0);
    }
    if i.lit {
        drain *= 0.6;
    }
    if i.company {
        drain *= 0.5;
    }
    let mut gain = 0.0;
    if i.day_f > 0.5 {
        gain += 0.5 * i.day_f;
    }
    if i.near_fire {
        gain += 1.4;
    }
    (s + (gain - drain) * i.dt).clamp(0.0, 100.0)
}

/// Chance that a bite is only in your head.
pub fn phantom_chance(sanity: f32) -> f32 {
    if sanity >= 50.0 {
        0.0
    } else {
        (50.0 - sanity) / 50.0 * 0.4
    }
}

/// The lantern gutters when your mind goes: a brightness multiplier 0..1.
pub fn lantern_flicker(time: f32, sanity: f32) -> f32 {
    if sanity >= 35.0 {
        return 1.0;
    }
    let k = (35.0 - sanity) / 35.0;
    let n = ((time * 7.0).sin() * 43758.547).fract().abs();
    let slow = ((time * 0.9).sin() * 0.5 + 0.5).powf(3.0);
    if n < 0.15 * k * (0.4 + slow) {
        0.1
    } else {
        1.0 - 0.25 * k * ((time * 19.0).sin() * 0.5 + 0.5)
    }
}

// ---------------------------------------------------------------------------
// Hallucinations
// ---------------------------------------------------------------------------
pub struct Shadow {
    pub pos: Vec3,
    pub age: f32,
    /// seconds you have been looking right at it
    pub gaze: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HauntEvent {
    ShadowAppeared,
    ShadowVanished,
    FakeFin,
}

pub struct Haunt {
    pub shadow: Option<Shadow>,
    next_shadow: f32,
    next_fin: f32,
    /// (position, time left, heading)
    pub fake_fin: Option<(Vec3, f32, f32)>,
}

impl Haunt {
    pub fn new() -> Haunt {
        Haunt { shadow: None, next_shadow: 25.0, next_fin: 30.0, fake_fin: None }
    }

    /// `ground(x, z)` is the terrain height. `yaw` is where the player is looking.
    pub fn update(&mut self, dt: f32, sanity: f32, feet: Vec3, yaw: f32, near_water: bool, ground: &dyn Fn(f32, f32) -> f32) -> Vec<HauntEvent> {
        let mut ev = vec![];
        // a figure at the edge of your vision that is gone when you look straight at it
        if let Some(s) = &mut self.shadow {
            s.age += dt;
            let to = vec2(s.pos.x - feet.x, s.pos.z - feet.z);
            let ang = angle_between(vec2(yaw.cos(), yaw.sin()), to);
            if ang < 0.42 {
                s.gaze += dt;
            } else {
                s.gaze = (s.gaze - dt).max(0.0);
            }
            if s.gaze > 0.45 || s.age > 28.0 || sanity > 70.0 {
                self.shadow = None;
                ev.push(HauntEvent::ShadowVanished);
            }
        } else if sanity < 45.0 {
            self.next_shadow -= dt;
            if self.next_shadow <= 0.0 {
                self.next_shadow = rf(18.0, 45.0) * (0.4 + sanity / 100.0);
                let side = if rf(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
                let a = yaw + side * rf(1.0, 1.5);
                let d = rf(14.0, 24.0);
                let (x, z) = (feet.x + a.cos() * d, feet.z + a.sin() * d);
                let g = ground(x, z);
                let y = if g < -0.4 { 0.0 } else { g };
                self.shadow = Some(Shadow { pos: vec3(x, y, z), age: 0.0, gaze: 0.0 });
                ev.push(HauntEvent::ShadowAppeared);
            }
        }
        // a fin that is not there
        if let Some((p, t, h)) = &mut self.fake_fin {
            *t -= dt;
            *p += vec3(h.cos(), 0.0, h.sin()) * (2.5 * dt);
            if *t <= 0.0 {
                self.fake_fin = None;
            }
        } else if sanity < 30.0 && near_water {
            self.next_fin -= dt;
            if self.next_fin <= 0.0 {
                self.next_fin = rf(30.0, 70.0);
                for _ in 0..8 {
                    let a = yaw + rf(-0.9, 0.9);
                    let d = rf(10.0, 22.0);
                    let (x, z) = (feet.x + a.cos() * d, feet.z + a.sin() * d);
                    if ground(x, z) < -2.0 {
                        self.fake_fin = Some((vec3(x, 0.0, z), 4.0, rf(0.0, TAU)));
                        ev.push(HauntEvent::FakeFin);
                        break;
                    }
                }
            }
        }
        ev
    }
}

fn angle_between(a: Vec2, b: Vec2) -> f32 {
    let (la, lb) = (a.length(), b.length());
    if la < 1e-6 || lb < 1e-6 {
        return PI;
    }
    (a.dot(b) / (la * lb)).clamp(-1.0, 1.0).acos()
}

// ---------------------------------------------------------------------------
// The night stalkers
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StalkState {
    Circle,
    Charge,
    Retreat,
    /// bursting out of the water next to the shore (jump scare)
    Surface,
}

pub struct Stalker {
    pub pos: Vec3,
    pub heading: f32,
    pub sp: usize,
    pub len: f32,
    pub state: StalkState,
    pub ang: f32,
    pub timer: f32,
    pub phase: f32,
}

pub struct StalkIn<'a> {
    pub dt: f32,
    pub time: f32,
    pub player: Vec3,
    /// in the water or on a boat: the hunters can reach you
    pub exposed: bool,
    /// on the dock or the beach next to the water
    pub near_water: bool,
    pub night: f32,
    pub blood: f32,
    pub fog: f32,
    /// burning flares (the hunters keep away)
    pub flares: &'a [Vec3],
    /// a searchlight: (position, direction)
    pub light: Option<(Vec3, Vec3)>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StalkEvent {
    /// it rammed you; `boat` = you were in a boat
    Hit { boat: bool },
    /// it burst out of the water beside you on the shore
    Lunge(Vec3),
    Spawned,
}

#[derive(Default)]
pub struct StalkerCtl {
    pub list: Vec<Stalker>,
    /// seconds spent exposed at night: the longer you stay out, the closer they come
    pub threat: f32,
    cooldown: f32,
    lunge_cd: f32,
    spawn_cd: f32,
}

/// How big and which monster a stalker is: huge at night, bigger still under a blood moon.
pub fn stalker_len(sp: usize, night: f32, blood: f32) -> f32 {
    crate::fish::SPECIES[sp].len * (1.15 + 0.55 * night + 0.4 * blood)
}

impl StalkerCtl {
    /// `depth(x, z)` is the terrain height (negative = sea floor).
    pub fn update(&mut self, depth: &dyn Fn(f32, f32) -> f32, inp: &StalkIn) -> Vec<StalkEvent> {
        let mut ev = vec![];
        let dt = inp.dt;
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.lunge_cd = (self.lunge_cd - dt).max(0.0);
        self.spawn_cd = (self.spawn_cd - dt).max(0.0);
        let out_there = inp.exposed || inp.near_water;
        if inp.night < 0.12 {
            self.list.clear();
            self.threat = 0.0;
            return ev;
        }
        if inp.night > 0.35 && out_there {
            self.threat += dt * (1.0 + 0.6 * inp.blood + 0.3 * inp.fog);
        } else {
            self.threat = (self.threat - dt * 0.4).max(0.0);
        }
        let want = if inp.night > 0.35 && out_there {
            1 + usize::from(inp.blood > 0.5 && self.threat > 25.0) + usize::from(inp.blood > 0.5 && self.threat > 70.0)
        } else {
            0
        };
        if self.list.len() < want && self.spawn_cd <= 0.0 {
            for _ in 0..12 {
                let a = rf(0.0, TAU);
                let r = rf(48.0, 60.0);
                let (x, z) = (inp.player.x + a.cos() * r, inp.player.z + a.sin() * r);
                if depth(x, z) < -5.0 {
                    let sp = 11 + (self.list.len() + (inp.time as usize / 60)) % 3;
                    let len = stalker_len(sp, inp.night, inp.blood);
                    self.list.push(Stalker { pos: vec3(x, -2.0, z), heading: a + PI, sp, len, state: StalkState::Circle, ang: a, timer: 0.0, phase: rf(0.0, TAU) });
                    self.spawn_cd = 6.0;
                    ev.push(StalkEvent::Spawned);
                    break;
                }
            }
        }
        // fewer wanted than we have: the extras swim off and are gone
        self.list.truncate(want);
        let flares = inp.flares;
        let light = inp.light;
        let threat = self.threat;
        let mut cooldown = self.cooldown;
        let mut lunge_cd = self.lunge_cd;
        let mut threat_drop = 0.0;
        for s in self.list.iter_mut() {
            let to = vec2(inp.player.x - s.pos.x, inp.player.z - s.pos.z);
            let dist = to.length();
            let scared = flares.iter().any(|f| vec2(f.x - s.pos.x, f.z - s.pos.z).length() < 75.0)
                || light.is_some_and(|(lp, ld)| {
                    let v = s.pos - lp;
                    let ang = angle_between(vec2(ld.x, ld.z), vec2(v.x, v.z));
                    v.length() < 45.0 && ang < 0.32
                });
            s.len = stalker_len(s.sp, inp.night, inp.blood);
            let mv = |s: &mut Stalker, dir: Vec2, speed: f32| {
                if dir.length_squared() < 1e-6 {
                    return;
                }
                let d = dir.normalize();
                let (nx, nz) = (s.pos.x + d.x * speed * dt, s.pos.z + d.y * speed * dt);
                if depth(nx, nz) < -2.5 {
                    s.pos.x = nx;
                    s.pos.z = nz;
                }
                s.heading = lerp_angle(s.heading, d.y.atan2(d.x), (dt * 3.0).min(1.0));
            };
            match s.state {
                StalkState::Circle => {
                    let r_target = mixf(48.0, 8.0, smooth(0.0, 160.0, threat));
                    s.ang += dt * 3.2 * (1.0 + 0.4 * inp.blood) / r_target.max(6.0);
                    let want_pos = vec2(inp.player.x + s.ang.cos() * r_target, inp.player.z + s.ang.sin() * r_target);
                    let dir = want_pos - vec2(s.pos.x, s.pos.z);
                    let before = vec2(s.pos.x, s.pos.z);
                    mv(s, dir, 5.5);
                    if (vec2(s.pos.x, s.pos.z) - before).length() < 1e-4 && dir.length() > 1.0 {
                        s.ang += 0.12; // blocked by shallows: go round
                    }
                    s.pos.y += (-(1.3 + s.len * 0.07) - s.pos.y) * (dt * 1.5).min(1.0);
                    if scared {
                        s.state = StalkState::Retreat;
                        s.timer = 12.0;
                    } else if threat >= 150.0 && inp.exposed && cooldown <= 0.0 && inp.night > 0.45 {
                        s.state = StalkState::Charge;
                    } else if threat >= 150.0 && !inp.exposed && inp.near_water && lunge_cd <= 0.0 && inp.night > 0.45 {
                        // a jump scare: it erupts from the water right beside the shore
                        for k in 0..16 {
                            let a = k as f32 * TAU / 16.0 + s.phase;
                            let d = rf(3.0, 6.0);
                            let (x, z) = (inp.player.x + a.cos() * d, inp.player.z + a.sin() * d);
                            if depth(x, z) < -1.2 {
                                s.pos = vec3(x, -1.5, z);
                                s.heading = (inp.player.z - z).atan2(inp.player.x - x);
                                s.state = StalkState::Surface;
                                s.timer = 1.8;
                                lunge_cd = 45.0;
                                ev.push(StalkEvent::Lunge(s.pos));
                                break;
                            }
                        }
                    }
                }
                StalkState::Charge => {
                    mv(s, to, 11.0 + 3.0 * inp.blood);
                    s.pos.y += (-0.5 - s.pos.y) * (dt * 3.0).min(1.0);
                    if scared || !inp.exposed {
                        s.state = StalkState::Retreat;
                        s.timer = if scared { 12.0 } else { 6.0 };
                    } else if dist < s.len * 0.35 + 1.4 {
                        ev.push(StalkEvent::Hit { boat: inp.player.y > -0.6 });
                        s.state = StalkState::Retreat;
                        s.timer = 22.0;
                        cooldown = 25.0;
                        threat_drop = 70.0;
                    }
                }
                StalkState::Retreat => {
                    mv(s, -to, 8.0);
                    s.pos.y += (-(1.5 + s.len * 0.07) - s.pos.y) * (dt * 1.5).min(1.0);
                    s.timer -= dt;
                    if s.timer <= 0.0 && dist > 25.0 {
                        s.state = StalkState::Circle;
                    } else if s.timer <= -10.0 {
                        s.state = StalkState::Circle;
                    }
                }
                StalkState::Surface => {
                    s.timer -= dt;
                    let rise = if s.timer > 0.9 { 0.35 } else { -1.6 };
                    s.pos.y += (rise - s.pos.y) * (dt * 5.0).min(1.0);
                    if s.timer <= 0.0 {
                        s.state = StalkState::Retreat;
                        s.timer = 10.0;
                    }
                }
            }
        }
        self.cooldown = cooldown;
        self.lunge_cd = lunge_cd;
        self.threat = (self.threat - threat_drop).max(0.0);
        ev
    }

    /// Distance to the nearest stalker (a big number when there are none).
    pub fn nearest(&self, p: Vec3) -> f32 {
        self.list.iter().map(|s| vec2(s.pos.x - p.x, s.pos.z - p.z).length()).fold(1.0e9, f32::min)
    }
}

/// 0..1 how afraid the player should be right now.
pub fn dread_target(stalker_dist: f32, night: f32, sanity: f32, blood: f32) -> f32 {
    let near = if stalker_dist < 1.0e8 { 1.0 - ((stalker_dist - 6.0) / 55.0).clamp(0.0, 1.0) } else { 0.0 };
    let mind = ((100.0 - sanity) / 100.0).powf(1.4) * 0.9;
    let dark = night * 0.12 + blood * 0.15;
    near.max(mind).max(dark)
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------
/// A black, faceless figure with two pale eyes (the "shadow person" you see out of the corner of your eye).
pub fn draw_shadow(mb: &mut MB, s: &Shadow, face_to: Vec3, time: f32) {
    let to = vec2(face_to.x - s.pos.x, face_to.z - s.pos.z);
    let h = to.y.atan2(to.x);
    let f = vec3(h.cos(), 0.0, h.sin());
    let r = vec3(-h.sin(), 0.0, h.cos());
    let black = vec3(0.012, 0.012, 0.016);
    let p = s.pos;
    let sway = (time * 0.8).sin() * 0.03;
    let fade = smooth(0.0, 1.2, s.age);
    let k = black * (0.6 + 0.4 * fade);
    // legs, torso, long arms, head
    for sd in [-1.0f32, 1.0] {
        cyl(mb, p + r * (sd * 0.11), p + r * (sd * 0.10) + Vec3::Y * 0.92, 0.085, 0.07, k, 8);
        cyl(mb, p + Vec3::Y * 1.42 + r * (sd * 0.22), p + Vec3::Y * 0.50 + r * (sd * 0.27) + f * sway, 0.05, 0.035, k, 6);
    }
    blob(mb, p + Vec3::Y * 1.18, vec3(0.22, 0.34, 0.15), k, 0.0, 9, 6, h, shade);
    blob(mb, p + Vec3::Y * 1.67 + f * sway, vec3(0.115, 0.14, 0.105), k, 0.0, 9, 6, h, shade);
    for sd in [-1.0f32, 1.0] {
        blob(mb, p + Vec3::Y * 1.70 + f * (0.09 + sway) + r * (sd * 0.045), Vec3::splat(0.014), vec3(0.85, 0.9, 0.95), 0.0, 5, 3, 0.0, |c, _| glow(c));
    }
}

/// A dorsal fin cutting the surface with a wake behind it.
pub fn draw_fin(mb: &mut MB, pos: Vec3, heading: f32, size: f32, time: f32) {
    let f = vec3(heading.cos(), 0.0, heading.sin());
    let r = vec3(-heading.sin(), 0.0, heading.cos());
    let col = vec3(0.05, 0.06, 0.08);
    let base = vec3(pos.x, 0.0, pos.z);
    let h = size * 0.55;
    let (a, b, c) = (base + f * (size * 0.22), base - f * (size * 0.22), base - f * (size * 0.05) + Vec3::Y * h);
    for off in [-0.02f32, 0.02] {
        let o = r * off;
        let n = (b - a).cross(c - a).normalize_or_zero();
        let l = shade(col, if off < 0.0 { n } else { -n });
        mb.tri(a + o, b + o, c + o, l, l, l);
    }
    // wake: pale foam trailing behind in a V
    for k in 1..9 {
        let u = k as f32;
        for sd in [-1.0f32, 1.0] {
            let p = base - f * (size * 0.25 + u * size * 0.18) + r * (sd * u * size * 0.07);
            blob(mb, p + Vec3::Y * 0.02, vec3(0.10 + 0.03 * u, 0.012, 0.10 + 0.03 * u) * (size * 0.4), vec3(0.88, 0.93, 0.95) * (1.0 - u * 0.07), 0.0, 6, 3, (time + u).sin(), shade);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nights_are_dark_foggy_and_sometimes_blood_red() {
        // noon of day 0 is clear and day-like
        let w = weather_at(DAY_LEN * 0.4);
        assert!(w.fog == 0.0 && w.blood == 0.0 && w.night == 0.0);
        // the first two nights never have a blood moon; the second night is always foggy
        assert!(!blood_night(0) && !blood_night(1));
        assert!(fog_night(1));
        let mid_night_1 = CYCLE + DAY_LEN + NIGHT_LEN * 0.5;
        let w = weather_at(mid_night_1);
        assert!(w.fog > 0.95 && w.night > 0.95 && w.day == 1);
        // a blood moon is guaranteed by the fifth night and then reddens the whole night
        assert!(blood_night(4));
        let w = weather_at(4.0 * CYCLE + DAY_LEN + NIGHT_LEN * 0.5);
        assert!(w.blood > 0.95);
        // blood moons stay a minority
        let n = (0..400).filter(|d| blood_night(*d)).count();
        assert!((60..160).contains(&n), "{n} blood nights in 400");
        // fog fades by morning
        assert!(weather_at(CYCLE + CYCLE - 2.0).fog < 0.05);
    }

    #[test]
    fn sanity_drains_in_the_dark_deep_and_recovers_by_fire_and_day() {
        let base = SanityIn { dt: 1.0, night: 0.0, fog: 0.0, blood: 0.0, deep: false, stalker_dist: 1e9, lit: false, near_fire: false, day_f: 1.0, company: false };
        assert!(sanity_step(50.0, &base) > 50.0, "day heals");
        let night = SanityIn { night: 1.0, day_f: 0.0, ..base };
        let n1 = sanity_step(80.0, &night);
        assert!(n1 < 80.0);
        let deep = SanityIn { night: 1.0, day_f: 0.0, deep: true, ..night };
        assert!(sanity_step(80.0, &deep) < n1, "deep water is worse");
        let near = SanityIn { stalker_dist: 5.0, ..deep };
        assert!(sanity_step(80.0, &near) < sanity_step(80.0, &deep), "hunters are worse");
        let lit = SanityIn { lit: true, ..night };
        assert!(sanity_step(80.0, &lit) > n1, "a lantern helps");
        let fire = SanityIn { near_fire: true, ..night };
        assert!(sanity_step(40.0, &fire) > 40.0, "a campfire heals");
        let company = SanityIn { company: true, ..night };
        assert!(sanity_step(80.0, &company) > n1, "company helps");
        // always within bounds
        assert_eq!(sanity_step(0.0, &near), 0.0);
        assert_eq!(sanity_step(100.0, &base), 100.0);
        // a whole blood-moon night out in the open without light is survivable but costly
        let mut s = 100.0;
        for _ in 0..600 {
            s = sanity_step(s, &SanityIn { night: 1.0, blood: 1.0, fog: 1.0, day_f: 0.0, ..base });
        }
        assert!(s < 20.0, "should be rattled, got {s}");
    }

    #[test]
    fn phantom_bites_and_flicker_only_when_rattled() {
        assert_eq!(phantom_chance(80.0), 0.0);
        assert!(phantom_chance(10.0) > phantom_chance(40.0));
        assert!(phantom_chance(0.0) <= 0.41);
        assert_eq!(lantern_flicker(3.3, 90.0), 1.0);
        let dips = (0..2000).filter(|i| lantern_flicker(*i as f32 * 0.037, 5.0) < 0.5).count();
        assert!(dips > 20, "{dips}");
    }

    fn deep(_x: f32, _z: f32) -> f32 {
        -20.0
    }

    #[test]
    fn stalkers_close_in_over_time_then_strike_and_leave_by_day() {
        let _g = crate::RAND_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        rand::srand(42);
        let mut c = StalkerCtl::default();
        let player = vec3(0.0, -0.1, 0.0);
        let mut hits = 0;
        let mut min_d = 1e9f32;
        let mut spawned = false;
        for i in 0..40_000 {
            let t = i as f32 * 0.05;
            let ev = c.update(&deep, &StalkIn { dt: 0.05, time: t, player, exposed: true, near_water: true, night: 1.0, blood: 0.0, fog: 0.0, flares: &[], light: None });
            spawned |= ev.contains(&StalkEvent::Spawned);
            hits += ev.iter().filter(|e| matches!(e, StalkEvent::Hit { .. })).count();
            if t > 60.0 {
                min_d = min_d.min(c.nearest(player));
            }
            if hits > 0 {
                break;
            }
        }
        assert!(spawned);
        assert!(hits > 0, "it never attacked");
        assert!(min_d < 20.0, "it should have come close, got {min_d}");
        assert!(c.threat < 160.0);
        // daylight sends them away
        let ev = c.update(&deep, &StalkIn { dt: 0.05, time: 0.0, player, exposed: true, near_water: true, night: 0.0, blood: 0.0, fog: 0.0, flares: &[], light: None });
        assert!(ev.is_empty() && c.list.is_empty());
    }

    #[test]
    fn flares_keep_stalkers_away_and_the_shore_is_safe_except_for_a_scare() {
        let _g = crate::RAND_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        rand::srand(7);
        let player = vec3(0.0, -0.1, 0.0);
        // a burning flare right above the player: no hits ever
        let mut c = StalkerCtl::default();
        c.threat = 400.0;
        let flare = [player];
        let mut hits = 0;
        for i in 0..6000 {
            let ev = c.update(&deep, &StalkIn { dt: 0.05, time: i as f32 * 0.05, player, exposed: true, near_water: true, night: 1.0, blood: 0.0, fog: 0.0, flares: &flare, light: None });
            hits += ev.iter().filter(|e| matches!(e, StalkEvent::Hit { .. })).count();
        }
        assert_eq!(hits, 0, "a flare should hold them off");
        // standing on the beach: never a Hit, but eventually a lunge scare
        let mut c = StalkerCtl::default();
        let (mut lunges, mut hits) = (0, 0);
        for i in 0..30_000 {
            let ev = c.update(&deep, &StalkIn { dt: 0.05, time: i as f32 * 0.05, player: vec3(0.0, 1.0, 0.0), exposed: false, near_water: true, night: 1.0, blood: 0.0, fog: 0.0, flares: &[], light: None });
            lunges += ev.iter().filter(|e| matches!(e, StalkEvent::Lunge(_))).count();
            hits += ev.iter().filter(|e| matches!(e, StalkEvent::Hit { .. })).count();
        }
        assert_eq!(hits, 0);
        assert!(lunges >= 1, "no lunge scare on the shore");
    }

    #[test]
    fn shadows_vanish_when_looked_at() {
        let _g = crate::RAND_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        rand::srand(5);
        let mut h = Haunt::new();
        let ground = |_x: f32, _z: f32| 1.0;
        let mut appeared = false;
        for _ in 0..4000 {
            if h.update(0.05, 20.0, Vec3::ZERO, 0.0, false, &ground).contains(&HauntEvent::ShadowAppeared) {
                appeared = true;
                break;
            }
        }
        assert!(appeared);
        // turn to face it: gone within a second or so
        let s = h.shadow.as_ref().unwrap().pos;
        let yaw = s.z.atan2(s.x);
        let mut gone = false;
        for _ in 0..40 {
            if h.update(0.05, 20.0, Vec3::ZERO, yaw, false, &ground).contains(&HauntEvent::ShadowVanished) {
                gone = true;
                break;
            }
        }
        assert!(gone, "the figure should vanish when stared at");
        // a sane player never sees them
        let mut h = Haunt::new();
        for _ in 0..20_000 {
            assert!(h.update(0.05, 90.0, Vec3::ZERO, 0.0, false, &ground).is_empty());
        }
    }
}
