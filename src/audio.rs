//! Procedural sound: every effect and ambience loop is synthesised at start-up into a 16-bit mono WAV
//! buffer (no asset files) and played through macroquad's audio.

use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound};
use macroquad::prelude::Vec3;
use std::cell::Cell;
use std::f32::consts::{PI, TAU};
use std::sync::atomic::{AtomicBool, Ordering};

pub const RATE: u32 = 22050;

static CLICK: AtomicBool = AtomicBool::new(false);
/// Called by `Ui::button` when a button is clicked; the next `Sfx::update` plays the click.
pub fn ui_click() {
    CLICK.store(true, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// tiny DSP toolbox
// ---------------------------------------------------------------------------
struct Rng(u32);
impl Rng {
    fn new(seed: u32) -> Rng {
        Rng(seed.wrapping_mul(2654435761).wrapping_add(0x9E3779B9) | 1)
    }
    fn u(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 16777216.0
    }
    /// white noise in -1..1
    fn n(&mut self) -> f32 {
        self.u() * 2.0 - 1.0
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.u()
    }
}

fn n_of(secs: f32) -> usize {
    (RATE as f32 * secs) as usize
}
fn noise(r: &mut Rng, n: usize) -> Vec<f32> {
    (0..n).map(|_| r.n()).collect()
}
fn lp(buf: &mut [f32], hz: f32) {
    let a = 1.0 - (-TAU * hz / RATE as f32).exp();
    let mut y = 0.0;
    for x in buf.iter_mut() {
        y += a * (*x - y);
        *x = y;
    }
}
fn lp_of(buf: &[f32], hz: f32) -> Vec<f32> {
    let mut v = buf.to_vec();
    lp(&mut v, hz);
    v
}
/// band-ish: lowpass at `hi` minus lowpass at `lo`
fn band(buf: &[f32], lo: f32, hi: f32) -> Vec<f32> {
    let a = lp_of(buf, hi);
    let b = lp_of(buf, lo);
    a.iter().zip(b.iter()).map(|(x, y)| x - y).collect()
}
fn normalize(b: &mut [f32], peak: f32) {
    let m = b.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    if m > 1e-9 {
        let k = peak / m;
        for x in b.iter_mut() {
            *x *= k;
        }
    }
}
/// short edge fades (no clicks) then normalise
fn finish(mut b: Vec<f32>) -> Vec<f32> {
    let n = b.len();
    let fi = 24.min(n / 4);
    let fo = 180.min(n / 4);
    for i in 0..fi {
        b[i] *= i as f32 / fi as f32;
    }
    for i in 0..fo {
        b[n - 1 - i] *= i as f32 / fo as f32;
    }
    normalize(&mut b, 0.9);
    b
}
fn mix(dst: &mut [f32], at_secs: f32, src: &[f32], gain: f32) {
    let o = n_of(at_secs);
    for (i, s) in src.iter().enumerate() {
        if let Some(d) = dst.get_mut(o + i) {
            *d += s * gain;
        }
    }
}
/// exponential pitch sweep from f0 to f1 (rate k), as a sine
fn sweep(t: f32, f0: f32, f1: f32, k: f32) -> f32 {
    (TAU * (f1 * t + (f0 - f1) * (1.0 - (-k * t).exp()) / k)).sin()
}
/// inharmonic bell/coin ping
fn bell(t: f32, f: f32, d: f32) -> f32 {
    if t < 0.0 {
        return 0.0;
    }
    ((TAU * f * t).sin() + 0.5 * (TAU * f * 2.76 * t).sin() * (-t * d * 1.6).exp() + 0.25 * (TAU * f * 5.4 * t).sin() * (-t * d * 3.0).exp()) * (-t * d).exp() * (t / 0.002).min(1.0)
}
fn render(secs: f32, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    (0..n_of(secs)).map(|i| f(i as f32 / RATE as f32)).collect()
}
/// equal-power crossfade of the tail into the head so a loop of length `n` has no seam
fn make_loop(raw: &[f32], n: usize) -> Vec<f32> {
    let f = raw.len() - n;
    let mut out = raw[..n].to_vec();
    for i in 0..f {
        let w = i as f32 / f as f32 * PI * 0.5;
        out[i] = raw[i] * w.sin() + raw[n + i] * w.cos();
    }
    out
}
fn whoosh(r: &mut Rng, secs: f32, f0: f32, f1: f32) -> Vec<f32> {
    let n = n_of(secs);
    let (mut y1, mut y2) = (0.0f32, 0.0f32);
    (0..n)
        .map(|i| {
            let p = i as f32 / n as f32;
            let fc = f0 + (f1 - f0) * p;
            let a = 1.0 - (-TAU * fc / RATE as f32).exp();
            y1 += a * (r.n() - y1);
            y2 += a * (y1 - y2);
            y2 * (PI * p).sin().powi(2)
        })
        .collect()
}
fn bubbles(r: &mut Rng, secs: f32, count: usize) -> Vec<f32> {
    let mut o = vec![0.0; n_of(secs)];
    for _ in 0..count {
        let at = r.range(0.0, secs * 0.7);
        let f0 = r.range(250.0, 500.0);
        let b = render(0.09, |t| sweep(t, f0 * 2.2, f0, 30.0) * (-t * 38.0).exp() * (t / 0.003).min(1.0));
        mix(&mut o, at, &b, r.range(0.4, 0.9));
    }
    o
}

// ---------------------------------------------------------------------------
// one-shots
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Snd {
    StepSand,
    StepGrass,
    StepWood,
    StepWater,
    Splash,
    Stroke,
    Cast,
    LineSplash,
    Reel,
    Bite,
    Caught,
    Snap,
    Pop,
    Throw,
    Thump,
    Gulp,
    Lantern,
    Tick,
    Coin,
    Coins,
    Chime,
    UiClick,
    Jump,
    Land,
    Creak,
    Bird,
    Thrash,
    Tension,
    Drag,
    Key,
    Paddle,
    Motor,
    Bloop,
    Flop,
}

/// Everything is played this much louder than the master slider says: the synthesised sounds are quiet.
const LOUD: f32 = 1.9;

pub const ALL: [Snd; 34] = [
    Snd::StepSand,
    Snd::StepGrass,
    Snd::StepWood,
    Snd::StepWater,
    Snd::Splash,
    Snd::Stroke,
    Snd::Cast,
    Snd::LineSplash,
    Snd::Reel,
    Snd::Bite,
    Snd::Caught,
    Snd::Snap,
    Snd::Pop,
    Snd::Throw,
    Snd::Thump,
    Snd::Gulp,
    Snd::Lantern,
    Snd::Tick,
    Snd::Coin,
    Snd::Coins,
    Snd::Chime,
    Snd::UiClick,
    Snd::Jump,
    Snd::Land,
    Snd::Creak,
    Snd::Bird,
    Snd::Thrash,
    Snd::Tension,
    Snd::Drag,
    Snd::Key,
    Snd::Paddle,
    Snd::Motor,
    Snd::Bloop,
    Snd::Flop,
];

fn variants(s: Snd) -> usize {
    match s {
        Snd::StepSand | Snd::StepGrass | Snd::StepWood | Snd::Creak | Snd::Bird => 3,
        Snd::StepWater | Snd::Splash | Snd::Reel | Snd::Coins | Snd::Thrash | Snd::Paddle | Snd::Flop => 2,
        _ => 1,
    }
}

/// designed length in seconds
fn secs(s: Snd) -> f32 {
    match s {
        Snd::StepSand => 0.14,
        Snd::StepGrass => 0.18,
        Snd::StepWood => 0.16,
        Snd::StepWater => 0.35,
        Snd::Splash => 0.7,
        Snd::Stroke => 0.5,
        Snd::Cast => 0.45,
        Snd::LineSplash => 0.45,
        Snd::Reel => 0.05,
        Snd::Bite => 0.35,
        Snd::Caught => 1.1,
        Snd::Snap => 0.5,
        Snd::Pop => 0.12,
        Snd::Throw => 0.3,
        Snd::Thump => 0.2,
        Snd::Gulp => 0.5,
        Snd::Lantern => 0.1,
        Snd::Tick => 0.05,
        Snd::Coin => 0.5,
        Snd::Coins => 1.1,
        Snd::Chime => 2.2,
        Snd::UiClick => 0.06,
        Snd::Jump => 0.15,
        Snd::Land => 0.22,
        Snd::Creak => 0.6,
        Snd::Bird => 0.5,
        Snd::Thrash => 0.55,
        Snd::Tension => 0.35,
        Snd::Drag => 0.5,
        Snd::Key => 1.2,
        Snd::Paddle => 0.6,
        Snd::Motor => 0.2,
        Snd::Bloop => 3.2,
        Snd::Flop => 0.3,
    }
}

/// playback level of each effect (before the master volume)
fn base_vol(s: Snd) -> f32 {
    match s {
        Snd::StepSand => 0.45,
        Snd::StepGrass => 0.40,
        Snd::StepWood => 0.55,
        Snd::StepWater => 0.55,
        Snd::Splash => 0.8,
        Snd::Stroke => 0.5,
        Snd::Cast => 0.6,
        Snd::LineSplash => 0.6,
        Snd::Reel => 0.35,
        Snd::Bite => 0.8,
        Snd::Caught => 0.7,
        Snd::Snap => 0.7,
        Snd::Pop => 0.6,
        Snd::Throw => 0.6,
        Snd::Thump => 0.6,
        Snd::Gulp => 0.7,
        Snd::Lantern => 0.6,
        Snd::Tick => 0.35,
        Snd::Coin => 0.6,
        Snd::Coins => 0.65,
        Snd::Chime => 0.6,
        Snd::UiClick => 0.4,
        Snd::Jump => 0.45,
        Snd::Land => 0.6,
        Snd::Creak => 0.45,
        Snd::Bird => 0.35,
        Snd::Thrash => 0.85,
        Snd::Tension => 0.45,
        Snd::Drag => 0.55,
        Snd::Key => 0.8,
        Snd::Paddle => 0.7,
        Snd::Motor => 0.5,
        Snd::Bloop => 0.9,
        Snd::Flop => 0.8,
    }
}

fn gen_oneshot(s: Snd, v: usize) -> Vec<f32> {
    let mut r = Rng::new(1000 + s as u32 * 97 + v as u32 * 13);
    let d = secs(s);
    let n = n_of(d);
    let vf = v as f32;
    let out: Vec<f32> = match s {
        Snd::StepSand => {
            let mut nz = noise(&mut r, n);
            lp(&mut nz, 2400.0 + vf * 500.0);
            let grit: Vec<f32> = (0..n).map(|_| if r.u() < 0.18 { r.n() } else { 0.0 }).collect();
            let grit = lp_of(&grit, 5000.0);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    let e = (t / 0.004).min(1.0) * (-t * 26.0).exp();
                    (nz[i] * 0.8 + grit[i] * 2.0) * e + (TAU * (85.0 + vf * 8.0) * t).sin() * (-t * 45.0).exp() * 0.5
                })
                .collect()
        }
        Snd::StepGrass => {
            let nz = noise(&mut r, n);
            let b = band(&nz, 1200.0 + vf * 300.0, 6500.0);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    b[i] * (t / 0.035).min(1.0) * (-t * 17.0).exp()
                })
                .collect()
        }
        Snd::StepWood => {
            let nz = lp_of(&noise(&mut r, n), 3200.0);
            let f = 140.0 + vf * 22.0;
            render(d, |t| (TAU * f * t).sin() * (-t * 34.0).exp() * 0.8 + (TAU * f * 2.1 * t).sin() * (-t * 55.0).exp() * 0.4)
                .iter()
                .enumerate()
                .map(|(i, x)| x + nz[i] * (-(i as f32 / RATE as f32) * 300.0).exp() * 0.9)
                .collect()
        }
        Snd::StepWater => {
            let mut nz = noise(&mut r, n);
            lp(&mut nz, 1100.0);
            let bub = bubbles(&mut r, d, 3);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    nz[i] * (t / 0.02).min(1.0) * (-t * 9.0).exp() * 1.5 + bub[i] * 0.5
                })
                .collect()
        }
        Snd::Splash => {
            let mut nz = noise(&mut r, n);
            lp(&mut nz, 3200.0);
            let bub = bubbles(&mut r, d, 7);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    nz[i] * (t / 0.006).min(1.0) * (-t * 6.0).exp() * 1.4 + bub[i] * 0.45
                })
                .collect()
        }
        Snd::Stroke => {
            let mut nz = noise(&mut r, n);
            lp(&mut nz, 1300.0);
            let bub = bubbles(&mut r, d, 3);
            (0..n)
                .map(|i| {
                    let p = i as f32 / n as f32;
                    nz[i] * (PI * p).sin().powi(2) * 1.5 + bub[i] * 0.3
                })
                .collect()
        }
        Snd::Cast => whoosh(&mut r, d, 350.0, 2800.0),
        Snd::LineSplash => {
            let mut nz = noise(&mut r, n);
            lp(&mut nz, 4500.0);
            let bub = bubbles(&mut r, d, 3);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    nz[i] * (t / 0.002).min(1.0) * (-t * 14.0).exp() + bub[i] * 0.4
                })
                .collect()
        }
        Snd::Reel => {
            let nz = lp_of(&noise(&mut r, n), 4500.0);
            let f = 1700.0 + vf * 350.0;
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    nz[i] * (-t * 170.0).exp() + (TAU * f * t).sin() * (-t * 210.0).exp() * 0.6
                })
                .collect()
        }
        Snd::Bite => {
            let mut o = vec![0.0; n];
            for (k, f) in [430.0f32, 560.0].iter().enumerate() {
                let kn = render(0.18, |t| {
                    (TAU * f * t).sin() * (-t * 42.0).exp() + (TAU * f * 2.3 * t).sin() * (-t * 70.0).exp() * 0.4 + (r.n()) * (-t * 400.0).exp() * 0.5
                });
                mix(&mut o, k as f32 * 0.16, &kn, 1.0);
            }
            o
        }
        Snd::Caught => {
            let mut o = vec![0.0; n];
            let notes = [523.25f32, 659.25, 783.99, 1046.5];
            for (k, f) in notes.iter().enumerate() {
                let len = if k == 3 { 0.7 } else { 0.3 };
                let dec = if k == 3 { 4.0 } else { 9.0 };
                let nt = render(len, |t| ((TAU * f * t).sin() + 0.3 * (TAU * f * 2.0 * t).sin() + 0.12 * (TAU * f * 3.0 * t).sin()) * (t / 0.006).min(1.0) * (-t * dec).exp());
                mix(&mut o, k as f32 * 0.11, &nt, if k == 3 { 1.2 } else { 0.8 });
            }
            o
        }
        Snd::Snap => {
            let nz = lp_of(&noise(&mut r, n), 6000.0);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    (sweep(t, 700.0, 140.0, 5.0) + 0.3 * sweep(t, 1400.0, 280.0, 5.0)) * (-t * 5.5).exp() * (t / 0.004).min(1.0) * 0.8 + nz[i] * (-t * 120.0).exp()
                })
                .collect()
        }
        Snd::Pop => {
            let nz = lp_of(&noise(&mut r, n), 3000.0);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    sweep(t, 900.0, 260.0, 45.0) * (-t * 38.0).exp() + nz[i] * (-t * 200.0).exp() * 0.5
                })
                .collect()
        }
        Snd::Throw => whoosh(&mut r, d, 300.0, 1700.0),
        Snd::Thump => {
            let nz = lp_of(&noise(&mut r, n), 600.0);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    sweep(t, 130.0, 52.0, 22.0) * (-t * 16.0).exp() + nz[i] * (-t * 50.0).exp() * 0.7
                })
                .collect()
        }
        Snd::Gulp => {
            let mut o = vec![0.0; n];
            for k in 0..3 {
                let f0 = 260.0 - k as f32 * 35.0;
                let g = render(0.14, |t| sweep(t, f0 * 1.6, f0 * 0.6, 14.0) * (PI * (t / 0.14)).sin().powi(2));
                mix(&mut o, 0.04 + k as f32 * 0.15, &g, 1.0 - k as f32 * 0.15);
            }
            o
        }
        Snd::Lantern => {
            let nz = lp_of(&noise(&mut r, n), 5000.0);
            let mut o = vec![0.0; n];
            for (k, f) in [1250.0f32, 900.0].iter().enumerate() {
                let c: Vec<f32> = render(0.04, |t| (TAU * f * t).sin() * (-t * 130.0).exp()).iter().enumerate().map(|(i, x)| x + nz[i] * (-(i as f32 / RATE as f32) * 350.0).exp()).collect();
                mix(&mut o, k as f32 * 0.05, &c, 1.0);
            }
            o
        }
        Snd::Tick => render(d, |t| (TAU * 1500.0 * t).sin() * (-t * 120.0).exp() + (TAU * 3100.0 * t).sin() * (-t * 200.0).exp() * 0.3),
        Snd::Coin => {
            let mut o = vec![0.0; n];
            mix(&mut o, 0.0, &render(0.5, |t| bell(t, 2637.0, 9.0)), 1.0);
            mix(&mut o, 0.07, &render(0.43, |t| bell(t, 3520.0, 8.0)), 0.8);
            o
        }
        Snd::Coins => {
            let mut o = vec![0.0; n];
            let base = [2400.0f32, 2637.0, 3136.0, 3520.0, 2093.0];
            for k in 0..7 {
                let f = base[(r.u() * 5.0) as usize % 5] * if v == 1 { 0.9 } else { 1.0 };
                let at = k as f32 * 0.1 + r.range(0.0, 0.05);
                mix(&mut o, at, &render(0.4, |t| bell(t, f, 11.0)), r.range(0.5, 1.0));
            }
            o
        }
        Snd::Chime => {
            let mut o = vec![0.0; n];
            for (k, f) in [659.25f32, 783.99, 987.77, 1318.5].iter().enumerate() {
                let at = k as f32 * 0.22;
                mix(&mut o, at, &render(2.0 - at * 0.5, |t| bell(t, *f, 2.6)), 1.0);
            }
            o
        }
        Snd::UiClick => {
            let nz = lp_of(&noise(&mut r, n), 4000.0);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    (TAU * 950.0 * t).sin() * (-t * 85.0).exp() + nz[i] * (-t * 300.0).exp() * 0.5
                })
                .collect()
        }
        Snd::Jump => {
            let mut nz = noise(&mut r, n);
            lp(&mut nz, 900.0);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    nz[i] * (PI * t / d).sin() * 1.2 + sweep(t, 160.0, 90.0, 20.0) * (-t * 22.0).exp() * 0.5
                })
                .collect()
        }
        Snd::Land => {
            let nz = lp_of(&noise(&mut r, n), 800.0);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    sweep(t, 120.0, 55.0, 25.0) * (-t * 14.0).exp() + nz[i] * (-t * 35.0).exp() * 0.9
                })
                .collect()
        }
        Snd::Creak => {
            // a slow stick-slip saw whose pitch wobbles, band-limited to sound like old planks
            let f0 = 85.0 + vf * 22.0;
            let mut ph = 0.0f32;
            let raw: Vec<f32> = (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    let f = f0 * (1.0 + 0.35 * (TAU * 2.2 * t + vf).sin() + 0.15 * (TAU * 7.0 * t).sin());
                    ph = (ph + f / RATE as f32).fract();
                    let saw = ph * 2.0 - 1.0;
                    saw * (0.55 + 0.45 * (TAU * 23.0 * t).sin().abs())
                })
                .collect();
            let f = lp_of(&raw, 650.0 + vf * 150.0);
            f.iter()
                .enumerate()
                .map(|(i, x)| {
                    let p = i as f32 / n as f32;
                    x * (PI * p).sin().powi(2)
                })
                .collect()
        }
        Snd::Thrash => {
            let mut nz = noise(&mut r, n);
            lp(&mut nz, 2600.0 - vf * 700.0);
            let bub = bubbles(&mut r, d, 6);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    nz[i] * (t / 0.01).min(1.0) * (-t * 5.5).exp() * 1.3 + bub[i] * 0.5 + sweep(t, 150.0, 60.0, 14.0) * (-t * 10.0).exp() * 0.6
                })
                .collect()
        }
        Snd::Tension => {
            // a taut line humming: a thin rising whine with a little grit
            let nz = lp_of(&noise(&mut r, n), 7000.0);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    let p = t / d;
                    let f = 520.0 + 380.0 * p + 18.0 * (TAU * 31.0 * t).sin();
                    ((TAU * f * t).sin() + 0.35 * (TAU * f * 2.0 * t).sin()) * (PI * p).sin().powf(0.6) * 0.7 + nz[i] * 0.12 * (PI * p).sin()
                })
                .collect()
        }
        Snd::Drag => {
            // reel drag: a fast ratchet that speeds up and a zipping tone
            let mut o = vec![0.0; n];
            let mut at = 0.0f32;
            let mut gap = 0.045f32;
            while at < d - 0.05 {
                let c = render(0.012, |t| (TAU * 2400.0 * t).sin() * (-t * 300.0).exp() + r.n() * (-t * 500.0).exp() * 0.6);
                mix(&mut o, at, &c, 1.0);
                at += gap;
                gap = (gap * 0.93).max(0.012);
            }
            o
        }
        Snd::Key => {
            let mut o = vec![0.0; n];
            for (k, f) in [1760.0f32, 2349.0, 2960.0].iter().enumerate() {
                mix(&mut o, 0.08 * k as f32, &render(0.7, |t| bell(t, *f, 4.0)), 0.9 - 0.15 * k as f32);
            }
            let nz = lp_of(&noise(&mut r, n_of(0.25)), 6000.0);
            let rattle: Vec<f32> = nz.iter().enumerate().map(|(i, x)| x * (-(i as f32 / RATE as f32) * 18.0).exp() * (0.5 + 0.5 * (i as f32 * 0.02).sin().abs())).collect();
            mix(&mut o, 0.0, &rattle, 0.5);
            o
        }
        Snd::Paddle => {
            // a wooden knock then the blade through water
            let mut nz = noise(&mut r, n);
            lp(&mut nz, 1500.0 + vf * 400.0);
            let bub = bubbles(&mut r, d, 3);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    let p = t / d;
                    (TAU * (170.0 + vf * 20.0) * t).sin() * (-t * 40.0).exp() * 0.8 + nz[i] * (PI * p).sin().powi(2) * 1.1 + bub[i] * 0.35
                })
                .collect()
        }
        Snd::Motor => {
            // one chug of an old outboard
            let nz = lp_of(&noise(&mut r, n), 900.0);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    let saw = ((t * 62.0).fract() * 2.0 - 1.0) + 0.5 * ((t * 124.0).fract() * 2.0 - 1.0);
                    (saw * 0.5 + nz[i] * 0.6) * (t / 0.006).min(1.0) * (-t * 20.0).exp()
                })
                .collect()
        }
        Snd::Bloop => {
            let tone = |t: f32| -> f32 {
                if t < 0.0 {
                    return 0.0;
                }
                let phase = TAU * (45.0 * t + 105.0 * (1.0 - (-t * 3.2).exp()) / 3.2);
                ((phase).sin() + 0.25 * (phase * 2.0).sin()) * (t / 0.04).min(1.0) * (-t * 2.4).exp()
            };
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    tone(t) * 0.8 + tone(t - 0.55) * 0.3 + tone(t - 1.1) * 0.12
                })
                .collect()
        }
        Snd::Flop => {
            let nz = lp_of(&noise(&mut r, n), 1800.0);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    nz[i] * (t / 0.004).min(1.0) * (-t * 22.0).exp() * 1.2 + sweep(t, 260.0 - vf * 40.0, 110.0, 25.0) * (-t * 18.0).exp() * 0.6
                })
                .collect()
        }
        Snd::Bird => match v {
            // three rising chirps
            0 => {
                let mut o = vec![0.0; n];
                for k in 0..3 {
                    let f0 = 2700.0 + k as f32 * 250.0;
                    let c = render(0.09, |t| sweep(t, f0, f0 * 1.5, -22.0) * (PI * t / 0.09).sin());
                    mix(&mut o, 0.02 + k as f32 * 0.13, &c, 1.0);
                }
                o
            }
            // fast trill
            1 => {
                let mut o = vec![0.0; n];
                for k in 0..9 {
                    let c = render(0.035, |t| (TAU * (3400.0 + 600.0 * (k % 2) as f32) * t).sin() * (PI * t / 0.035).sin());
                    mix(&mut o, 0.03 + k as f32 * 0.045, &c, 1.0);
                }
                o
            }
            // gull-like descending cry, twice
            _ => {
                let mut o = vec![0.0; n];
                for k in 0..2 {
                    let c = render(0.22, |t| {
                        let ph = sweep(t, 2300.0, 1200.0, 6.0);
                        let ph2 = sweep(t * 2.0, 2300.0, 1200.0, 6.0);
                        (ph + 0.4 * ph2) * (PI * t / 0.22).sin().powf(0.7)
                    });
                    mix(&mut o, 0.02 + k as f32 * 0.24, &c, 1.0 - k as f32 * 0.25);
                }
                o
            }
        },
    };
    finish(out)
}

// ---------------------------------------------------------------------------
// ambience loops
// ---------------------------------------------------------------------------
#[derive(Clone, Copy)]
pub enum Amb {
    Waves,
    Wind,
    Fire,
    Crickets,
    Under,
    Music,
}
const AMBS: [Amb; 6] = [Amb::Waves, Amb::Wind, Amb::Fire, Amb::Crickets, Amb::Under, Amb::Music];

pub fn loop_secs(a: Amb) -> f32 {
    match a {
        Amb::Waves => 6.0,
        Amb::Wind => 5.0,
        Amb::Fire => 4.0,
        Amb::Crickets => 4.0,
        Amb::Under => 4.0,
        Amb::Music => 16.0,
    }
}

fn gen_loop(a: Amb) -> Vec<f32> {
    let n = n_of(loop_secs(a));
    let xf = n_of(0.5);
    let mut r = Rng::new(77 + a as u32 * 31);
    let mut out = match a {
        Amb::Waves => {
            let raw = noise(&mut r, n + xf);
            let mut rumble = lp_of(&raw, 420.0);
            lp(&mut rumble, 700.0);
            let hiss = band(&raw, 700.0, 3200.0);
            let rumble = make_loop(&rumble, n);
            let hiss = make_loop(&hiss, n);
            let mut rm = rumble;
            normalize(&mut rm, 1.0);
            let mut hs = hiss;
            normalize(&mut hs, 1.0);
            (0..n)
                .map(|i| {
                    let p = i as f32 / n as f32;
                    let swell = 0.5 + 0.5 * (TAU * 2.0 * p).sin();
                    let crest = (0.5 + 0.5 * (TAU * 2.0 * p - 1.4).sin()).powi(2);
                    rm[i] * (0.35 + 0.65 * swell) + hs[i] * 0.45 * crest
                })
                .collect::<Vec<f32>>()
        }
        Amb::Wind => {
            let raw = noise(&mut r, n + xf);
            let b = band(&raw, 220.0, 1400.0);
            let mut b = make_loop(&b, n);
            normalize(&mut b, 1.0);
            (0..n)
                .map(|i| {
                    let p = i as f32 / n as f32;
                    b[i] * (0.55 + 0.35 * (TAU * p).sin() + 0.2 * (TAU * 3.0 * p + 1.0).sin())
                })
                .collect()
        }
        Amb::Fire => {
            let raw = noise(&mut r, n + xf);
            let mut rum = lp_of(&raw, 150.0);
            lp(&mut rum, 220.0);
            let mut rum = make_loop(&rum, n);
            normalize(&mut rum, 0.45);
            let mut o = rum;
            // crackles stay away from the seam so it stays silent-ish and continuous
            for _ in 0..34 {
                let at = r.range(0.12, loop_secs(a) - 0.2);
                let len = r.range(0.012, 0.05);
                let k = r.range(90.0, 380.0);
                let amp = r.range(0.25, 1.0).powi(2) + 0.1;
                let cut = r.range(2500.0, 7000.0);
                let nz = lp_of(&noise(&mut r, n_of(len)), cut);
                let b: Vec<f32> = nz.iter().enumerate().map(|(i, x)| x * (-(i as f32 / RATE as f32) * k).exp()).collect();
                mix(&mut o, at, &b, amp * 2.0);
            }
            o
        }
        Amb::Crickets => {
            let mut o = vec![0.0; n];
            let secs_total = loop_secs(a);
            for (f, period, off, amp) in [(4100.0f32, 0.52f32, 0.15f32, 0.55f32), (4650.0, 0.63, 0.31, 0.45), (5250.0, 0.81, 0.07, 0.38), (3800.0, 1.1, 0.5, 0.3)] {
                let mut start = off;
                while start + 0.2 < secs_total - 0.1 {
                    for p in 0..3 {
                        let pulse = render(0.022, |t| (TAU * f * t).sin() * (PI * t / 0.022).sin());
                        mix(&mut o, start + p as f32 * 0.04, &pulse, amp);
                    }
                    start += period;
                }
            }
            o
        }
        Amb::Music => {
            // a calm, slow loop: soft pads under a gently plucked arpeggio (Am - F - C - G)
            let total = loop_secs(a);
            let mut o = vec![0.0f32; n];
            let chords: [[f32; 3]; 4] = [[220.0, 261.63, 329.63], [174.61, 220.0, 261.63], [196.0, 261.63, 329.63], [196.0, 246.94, 293.66]];
            let wrap = |o: &mut Vec<f32>, at: f32, src: &[f32], gain: f32| {
                let len = o.len();
                let s0 = n_of(at.rem_euclid(total));
                for (i, s) in src.iter().enumerate() {
                    o[(s0 + i) % len] += s * gain;
                }
            };
            for (ci, ch) in chords.iter().enumerate() {
                let start = ci as f32 * 4.0;
                for f in ch.iter() {
                    for det in [-1.0f32, 1.0] {
                        let fr = f * 0.5 * (1.0 + det * 0.0025);
                        let pad = render(5.0, |t| ((TAU * fr * t).sin() + 0.28 * (TAU * fr * 2.0 * t).sin()) * (PI * t / 5.0).sin().powi(2));
                        wrap(&mut o, start - 0.5, &pad, 0.07);
                    }
                }
                let order = [0usize, 1, 2, 1, 2, 1, 0, 1];
                for (j, &oi) in order.iter().enumerate() {
                    let f = ch[oi] * if j % 4 == 2 { 2.0 } else { 1.0 } * if j % 4 == 0 { 1.0 } else { 2.0 } * 0.5;
                    let at = start + j as f32 * 0.5;
                    let pl = render(1.4, |t| ((TAU * f * t).sin() + 0.3 * (TAU * f * 2.0 * t).sin() * (-t * 6.0).exp()) * (t / 0.004).min(1.0) * (-t * 3.2).exp());
                    wrap(&mut o, at, &pl, if j % 2 == 0 { 0.10 } else { 0.06 });
                }
            }
            o
        }
        Amb::Under => {
            let raw = noise(&mut r, n + xf);
            let mut a1 = lp_of(&raw, 260.0);
            lp(&mut a1, 320.0);
            let mut a1 = make_loop(&a1, n);
            normalize(&mut a1, 1.0);
            let secs_total = loop_secs(a);
            (0..n)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    let p = t / secs_total;
                    a1[i] * (0.7 + 0.3 * (TAU * p).sin()) + (TAU * 55.0 * t).sin() * 0.5 + (TAU * 82.5 * t).sin() * 0.2 * (TAU * p).cos()
                })
                .collect()
        }
    };
    normalize(&mut out, 0.9);
    out
}

fn to_wav(pcm: &[f32]) -> Vec<u8> {
    let n = pcm.len();
    let mut out = Vec::with_capacity(44 + n * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + n * 2) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&((n * 2) as u32).to_le_bytes());
    for p in pcm {
        out.extend_from_slice(&((p.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    out
}

// ---------------------------------------------------------------------------
// runtime
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq)]
pub enum Surface {
    Sand,
    Grass,
    Wood,
}

pub struct AmbCtx<'a> {
    pub volume: f32,
    pub feet: Vec3,
    pub fire: Vec3,
    pub day_f: f32,
    pub under: bool,
    pub swim: bool,
    pub moving: bool,
    pub grounded: bool,
    pub sprint: bool,
    pub vy: f32,
    pub walk_speed: f32,
    pub surface: Surface,
    pub wading: bool,
    pub on_dock: bool,
    /// terrain height lookup (x, z)
    pub height: &'a dyn Fn(f32, f32) -> f32,
}

pub struct Sfx {
    bank: Vec<Vec<Sound>>,
    loops: Vec<Option<Sound>>,
    vols: [f32; 6],
    master: f32,
    rng: Cell<u32>,
    probe_t: f32,
    water_d: f32,
    step_d: f32,
    stroke_t: f32,
    air: f32,
    was_grounded: bool,
    was_swim: bool,
    creak_t: f32,
    bird_t: f32,
}

impl Drop for Sfx {
    /// leaving the world (back to the main menu) must silence its ambience
    fn drop(&mut self) {
        for s in self.loops.iter().flatten() {
            stop_sound(s);
        }
    }
}

impl Sfx {
    pub async fn load() -> Sfx {
        let mut bank: Vec<Vec<Sound>> = Vec::new();
        for s in ALL {
            let mut v = Vec::new();
            for k in 0..variants(s) {
                if let Ok(snd) = load_sound_from_bytes(&to_wav(&gen_oneshot(s, k))).await {
                    v.push(snd);
                }
            }
            bank.push(v);
        }
        let mut loops = Vec::new();
        for a in AMBS {
            let snd = load_sound_from_bytes(&to_wav(&gen_loop(a))).await.ok();
            if let Some(s) = &snd {
                play_sound(s, PlaySoundParams { looped: true, volume: 0.0 });
            }
            loops.push(snd);
        }
        CLICK.store(false, Ordering::Relaxed);
        Sfx {
            bank,
            loops,
            vols: [0.0; 6],
            master: 0.7,
            rng: Cell::new(0x1234_5679),
            probe_t: 0.0,
            water_d: 60.0,
            step_d: 1.2,
            stroke_t: 0.0,
            air: 0.0,
            was_grounded: true,
            was_swim: false,
            creak_t: 2.0,
            bird_t: 4.0,
        }
    }

    fn rnd(&self) -> f32 {
        let mut x = self.rng.get();
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng.set(x);
        (x >> 8) as f32 / 16777216.0
    }

    /// Play a one-shot (a random variant) at `scale` x its base level x master volume.
    pub fn play(&self, which: Snd, scale: f32) {
        let v = self.master * base_vol(which) * scale * LOUD;
        if v < 0.002 {
            return;
        }
        let list = &self.bank[which as usize];
        if list.is_empty() {
            return;
        }
        let i = ((self.rnd() * list.len() as f32) as usize).min(list.len() - 1);
        play_sound(&list[i], PlaySoundParams { looped: false, volume: v.min(1.0) });
    }

    fn probe(&self, c: &AmbCtx) -> f32 {
        if (c.height)(c.feet.x, c.feet.z) < -0.5 {
            return 0.0;
        }
        let mut best = 80.0f32;
        for k in 0..8 {
            let a = k as f32 * TAU / 8.0;
            let (dx, dz) = (a.cos(), a.sin());
            for d in [2.0f32, 5.0, 9.0, 14.0, 20.0, 28.0, 38.0, 50.0] {
                if d >= best {
                    break;
                }
                if (c.height)(c.feet.x + dx * d, c.feet.z + dz * d) < -0.5 {
                    best = d;
                    break;
                }
            }
        }
        best
    }

    pub fn update(&mut self, c: &AmbCtx, dt: f32) {
        let dt = dt.clamp(0.0005, 0.1);
        self.master = c.volume.clamp(0.0, 1.0);
        if CLICK.swap(false, Ordering::Relaxed) {
            self.play(Snd::UiClick, 1.0);
        }
        self.probe_t -= dt;
        if self.probe_t <= 0.0 {
            self.probe_t = 0.3;
            self.water_d = self.probe(c);
        }

        // ---- ambience levels ----
        let wd = if c.swim || c.wading { 0.0 } else { self.water_d };
        let mut waves = (1.0 - wd / 50.0).clamp(0.0, 1.0).powf(1.3) * 0.75;
        if c.on_dock {
            waves = waves.max(0.6);
        }
        if c.swim {
            waves = 0.85;
        }
        let hill = ((c.feet.y - 2.0) / 12.0).clamp(0.0, 1.0);
        let wind = 0.25 + 0.6 * hill;
        let fd = c.feet.distance(c.fire);
        let fire = if fd < 14.0 { (1.0 - fd / 14.0).powi(2) * 0.85 } else { 0.0 };
        let night = (1.0 - c.day_f).clamp(0.0, 1.0).powf(1.5);
        let inland = ((self.water_d - 12.0) / 25.0).clamp(0.0, 1.0);
        let crick = 0.4 * night * inland;
        let music = if c.under { 0.0 } else { 0.30 - 0.10 * night };
        let mut t = [waves * 0.55, wind * 0.32, fire * 0.7, crick, 0.0, music];
        if c.under {
            for x in t.iter_mut() {
                *x *= 0.12;
            }
            t[4] = 0.65;
        }
        let k = (dt * 4.0).min(1.0);
        for i in 0..6 {
            self.vols[i] += (t[i] - self.vols[i]) * k;
            if let Some(s) = &self.loops[i] {
                set_sound_volume(s, (self.vols[i] * self.master * LOUD).clamp(0.0, 1.0));
            }
        }

        // ---- birds by day ----
        if c.day_f > 0.7 && !c.under {
            self.bird_t -= dt;
            if self.bird_t <= 0.0 {
                self.bird_t = 6.0 + self.rnd() * 10.0;
                self.play(Snd::Bird, 0.6 + 0.6 * self.rnd());
            }
        }

        // ---- movement sounds ----
        if c.swim && !self.was_swim {
            self.play(Snd::Splash, 1.0);
        }
        if c.swim && c.moving {
            self.stroke_t += dt;
            if self.stroke_t >= if c.sprint { 0.7 } else { 1.1 } {
                self.stroke_t = 0.0;
                self.play(Snd::Stroke, 0.8);
            }
        } else {
            self.stroke_t = 0.6;
        }
        let stride = if c.sprint { 2.6 } else { 2.0 };
        if c.grounded && c.moving && !c.swim && c.walk_speed > 0.5 {
            self.step_d += c.walk_speed * dt;
            if self.step_d >= stride {
                self.step_d -= stride;
                let sc = (if c.sprint { 1.0 } else { 0.7 }) * (0.85 + 0.3 * self.rnd());
                let snd = if c.wading {
                    Snd::StepWater
                } else {
                    match c.surface {
                        Surface::Sand => Snd::StepSand,
                        Surface::Grass => Snd::StepGrass,
                        Surface::Wood => Snd::StepWood,
                    }
                };
                self.play(snd, sc);
            }
        } else {
            self.step_d = self.step_d.max(1.2).min(stride);
        }
        // jump / landing
        if self.was_grounded && !c.grounded && !c.swim && c.vy > 2.0 {
            self.play(Snd::Jump, 1.0);
        }
        if !c.grounded && !c.swim {
            self.air += dt;
        } else {
            if c.grounded && !self.was_grounded && self.air > 0.25 {
                self.play(if c.wading { Snd::Splash } else { Snd::Land }, (self.air * 1.2).clamp(0.5, 1.2));
            }
            self.air = 0.0;
        }
        // dock planks
        if c.on_dock && c.grounded && !c.swim {
            self.creak_t -= dt * if c.moving { 1.0 } else { 0.3 };
            if self.creak_t <= 0.0 {
                self.creak_t = 3.0 + self.rnd() * 4.0;
                self.play(Snd::Creak, 1.0);
            }
        }
        self.was_grounded = c.grounded;
        self.was_swim = c.swim;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(name: &str, b: &[f32]) {
        assert!(!b.is_empty(), "{name} empty");
        assert!(b.iter().all(|x| x.is_finite()), "{name} not finite");
        let pk = b.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(pk >= 0.2 && pk <= 1.0, "{name} peak {pk}");
    }

    #[test]
    fn oneshots_valid() {
        for s in ALL {
            for v in 0..variants(s) {
                let b = gen_oneshot(s, v);
                check(&format!("{:?}/{}", s, v), &b);
                assert_eq!(b.len(), n_of(secs(s)), "{:?} length", s);
                assert!(b.len() > 500 && b.len() < n_of(3.5), "{:?} duration", s);
                // fades make the edges silent
                assert!(b[0].abs() < 0.05 && b[b.len() - 1].abs() < 0.05, "{:?} edges", s);
                // wav size
                assert_eq!(to_wav(&b).len(), 44 + b.len() * 2);
            }
        }
    }

    #[test]
    fn loops_valid_and_seamless() {
        for a in AMBS {
            let b = gen_loop(a);
            check(&format!("loop{}", a as usize), &b);
            assert_eq!(b.len(), n_of(loop_secs(a)));
            let seam = (b[0] - b[b.len() - 1]).abs();
            assert!(seam < 0.12, "loop {} seam {}", a as usize, seam);
            // biggest step inside the loop, for reference: the seam must not be a spike
            let mx = b.windows(2).fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()));
            assert!(seam <= mx.max(0.12), "loop {} seam spike", a as usize);
        }
    }
}
