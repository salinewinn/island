//! Realistic skinned player model (`assets/player.bin`, produced by `tools/export_player.py`).
//!
//! The game's own skeleton (`people::draw_person` solves hips, knees, ankles, shoulders, elbows,
//! wrists and the head every frame) drives 15 bones; vertices are blended onto them (linear blend
//! skinning).  Colours are baked into the vertices, so it goes through the normal vertex-lit mesh
//! builder like everything else.  Pass `--classic-people` to use the old procedural bodies instead.
//!
//! Model space (rest pose): x forward, y up, z = the character's right, feet at y = 0.

use super::*;
use std::sync::OnceLock;

const NB: usize = 15;
// rest joint indices (see tools/export_player.py); pairs are (right, left)
const HIP: usize = 0;
const KNEE: usize = 2;
const ANK: usize = 4;
const SH: usize = 8;
const EL: usize = 10;
const WR: usize = 12;
const TIP: usize = 14;
const PEL: usize = 16;
const CHEST: usize = 17;
const HEAD_B: usize = 18;
const HEAD_T: usize = 19;
/// how much further forward the rest head sits than the game's head (so the neck keeps its shape)
const HEAD_FWD: f32 = 0.032;

struct Model {
    pos: Vec<Vec3>,
    nrm: Vec<Vec3>,
    col: Vec<Vec3>,
    bi: Vec<[u8; 4]>,
    bw: Vec<[f32; 4]>,
    idx: Vec<[u16; 3]>,
    rest: Vec<Vec3>,
    /// per bone: (transpose of the rest frame, rest origin)
    rest_inv: [(Mat3, Vec3); NB],
}

/// Everything `draw_person` has solved for this frame.
pub struct Joints {
    /// pelvis / chest frame origins at the feet (ground) plane and their forward axes
    pub pel_base: Vec3,
    pub pel_f: Vec3,
    pub chest_base: Vec3,
    pub chest_f: Vec3,
    pub chest_r: Vec3,
    pub lean: f32,
    pub hip: [Vec3; 2],
    pub knee: [Vec3; 2],
    pub ank: [Vec3; 2],
    pub fd: [Vec3; 2],
    pub fu: [Vec3; 2],
    pub sh: [Vec3; 2],
    pub el: [Vec3; 2],
    pub wr: [Vec3; 2],
    pub hd: [Vec3; 2],
    pub hc: Vec3,
    pub fh: Vec3,
    pub uh: Vec3,
}

static MODEL: OnceLock<Option<Model>> = OnceLock::new();
static ENABLED: OnceLock<bool> = OnceLock::new();

fn model() -> Option<&'static Model> {
    MODEL.get_or_init(|| Model::parse(include_bytes!("../assets/player.bin"))).as_ref()
}

/// True when the realistic model should be used for (male) characters.
pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| !std::env::args().any(|a| a == "--classic-people") && model().is_some())
}

fn side(i: usize) -> f32 {
    if i == 0 {
        1.0
    } else {
        -1.0
    }
}

/// Orthonormal frame: x along `dir`, z as close to `hint` as possible.
fn bone_frame(dir: Vec3, hint: Vec3) -> Mat3 {
    let x = dir.try_normalize().unwrap_or(Vec3::X);
    let mut z = hint - x * hint.dot(x);
    if z.length_squared() < 1e-8 {
        z = if x.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        z -= x * z.dot(x);
    }
    let z = z.normalize();
    Mat3::from_cols(x, z.cross(x), z)
}

/// Direction the middle joint sticks out of the line between its neighbours; `fb` takes over when
/// the limb is straight, so the roll never flips.
fn pole(a: Vec3, b: Vec3, c: Vec3, fb: Vec3) -> Vec3 {
    let l1 = a.distance(b);
    let l2 = b.distance(c);
    let off = b - a.lerp(c, l1 / (l1 + l2).max(1e-6));
    (off + fb.normalize_or_zero() * 0.012).normalize_or_zero()
}

fn arm_fallback(f: Vec3, r: Vec3, i: usize) -> Vec3 {
    -f * 0.5 + r * (side(i) * 0.6)
}

/// Bone frames (rotation, origin) from a set of joints. Used for the rest pose and every frame.
#[allow(clippy::too_many_arguments)]
fn frames(
    pel: (Vec3, Vec3),
    chest: (Vec3, Vec3, Vec3),
    head: (Vec3, Vec3, Vec3),
    hip: [Vec3; 2],
    knee: [Vec3; 2],
    ank: [Vec3; 2],
    foot: [(Vec3, Vec3); 2],
    sh: [Vec3; 2],
    el: [Vec3; 2],
    wr: [Vec3; 2],
    hand_dir: [Vec3; 2],
) -> [(Mat3, Vec3); NB] {
    let mut out = [(Mat3::IDENTITY, Vec3::ZERO); NB];
    out[0] = (bone_frame(pel.1, Vec3::Y), pel.0);
    out[1] = (bone_frame(chest.1, Vec3::Y), chest.0);
    out[2] = (bone_frame(head.1, head.2), head.0);
    for i in 0..2 {
        let ap = pole(sh[i], el[i], wr[i], arm_fallback(chest.1, chest.2, i));
        out[3 + i] = (bone_frame(el[i] - sh[i], ap), sh[i]);
        out[5 + i] = (bone_frame(wr[i] - el[i], ap), el[i]);
        out[7 + i] = (bone_frame(hand_dir[i], ap), wr[i]);
        let lp = pole(hip[i], knee[i], ank[i], pel.1);
        out[9 + i] = (bone_frame(knee[i] - hip[i], lp), hip[i]);
        out[11 + i] = (bone_frame(ank[i] - knee[i], lp), knee[i]);
        out[13 + i] = (bone_frame(foot[i].0, foot[i].1), ank[i]);
    }
    out
}

impl Model {
    fn parse(b: &[u8]) -> Option<Model> {
        if b.len() < 16 || &b[0..4] != b"ISPL" {
            return None;
        }
        let u32_at = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as usize;
        let (nv, nt, nj) = (u32_at(4), u32_at(8), u32_at(12));
        let mut o = 16;
        let need = o + nj * 12 + nv * (12 + 12 + 3 + 4 + 4) + nt * 6;
        if nj != 20 || b.len() < need {
            return None;
        }
        let f32_at = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let v3_at = |o: usize| vec3(f32_at(o), f32_at(o + 4), f32_at(o + 8));
        let rest: Vec<Vec3> = (0..nj).map(|i| v3_at(o + i * 12)).collect();
        o += nj * 12;
        let pos: Vec<Vec3> = (0..nv).map(|i| v3_at(o + i * 12)).collect();
        o += nv * 12;
        let nrm: Vec<Vec3> = (0..nv).map(|i| v3_at(o + i * 12)).collect();
        o += nv * 12;
        let col: Vec<Vec3> = (0..nv).map(|i| vec3(b[o + i * 3] as f32, b[o + i * 3 + 1] as f32, b[o + i * 3 + 2] as f32) / 255.0).collect();
        o += nv * 3;
        let bi: Vec<[u8; 4]> = (0..nv).map(|i| [b[o + i * 4], b[o + i * 4 + 1], b[o + i * 4 + 2], b[o + i * 4 + 3]]).collect();
        o += nv * 4;
        let bw: Vec<[f32; 4]> = (0..nv).map(|i| [b[o + i * 4] as f32 / 255.0, b[o + i * 4 + 1] as f32 / 255.0, b[o + i * 4 + 2] as f32 / 255.0, b[o + i * 4 + 3] as f32 / 255.0]).collect();
        o += nv * 4;
        let idx: Vec<[u16; 3]> = (0..nt)
            .map(|i| {
                let p = o + i * 6;
                [u16::from_le_bytes([b[p], b[p + 1]]), u16::from_le_bytes([b[p + 2], b[p + 3]]), u16::from_le_bytes([b[p + 4], b[p + 5]])]
            })
            .collect();
        if idx.iter().any(|t| t.iter().any(|&v| v as usize >= nv)) || bi.iter().any(|a| a.iter().any(|&v| v as usize >= NB)) {
            return None;
        }
        // rest-pose frames, built exactly like the per-frame ones
        let r = |i: usize| -> [Vec3; 2] { [rest[i], rest[i + 1]] };
        let head_c = (rest[HEAD_B] + rest[HEAD_T]) * 0.5;
        let fr = frames(
            (rest[PEL], Vec3::X),
            (rest[CHEST], Vec3::X, Vec3::Z),
            (head_c, Vec3::X, Vec3::Y),
            r(HIP),
            r(KNEE),
            r(ANK),
            [(Vec3::X, Vec3::Y), (Vec3::X, Vec3::Y)],
            r(SH),
            r(EL),
            r(WR),
            [rest[TIP] - rest[WR], rest[TIP + 1] - rest[WR + 1]],
        );
        let mut rest_inv = [(Mat3::IDENTITY, Vec3::ZERO); NB];
        for k in 0..NB {
            rest_inv[k] = (fr[k].0.transpose(), fr[k].1);
        }
        Some(Model { pos, nrm, col, bi, bw, idx, rest, rest_inv })
    }

    /// Bone matrices for a pose: p' = A p + t.
    fn bones(&self, j: &Joints) -> [(Mat3, Vec3); NB] {
        let pel_f = j.pel_f;
        let foot = [(j.fd[0], j.fu[0]), (j.fd[1], j.fu[1])];
        let head_o = j.hc + j.chest_f * HEAD_FWD;
        let (rp, rc) = (self.rest[PEL], self.rest[CHEST]);
        let pel_o = j.pel_base + j.pel_f * rp.x + Vec3::Y * rp.y;
        // the game shears the torso forward with height when walking (`lean`)
        let chest_o = j.chest_base + j.chest_f * (rc.x + j.lean * (rc.y - 0.9).max(0.0)) + Vec3::Y * rc.y;
        let fr = frames(
            (pel_o, pel_f),
            (chest_o, j.chest_f, j.chest_r),
            (head_o, j.fh, j.uh),
            j.hip,
            j.knee,
            j.ank,
            foot,
            j.sh,
            j.el,
            j.wr,
            j.hd,
        );
        let mut out = [(Mat3::IDENTITY, Vec3::ZERO); NB];
        for k in 0..NB {
            let a = fr[k].0 * self.rest_inv[k].0;
            out[k] = (a, fr[k].1 - a * self.rest_inv[k].1);
        }
        out
    }
}

/// Draw the model for `j`. `post_p` / `post_n` are applied to every skinned position / normal
/// afterwards (used for the swimming pose).
pub fn draw(mb: &mut MB, j: &Joints, post_p: &dyn Fn(Vec3) -> Vec3, post_n: &dyn Fn(Vec3) -> Vec3) {
    let Some(m) = model() else { return };
    let bones = m.bones(j);
    let n = m.pos.len();
    let mut pos = Vec::with_capacity(n);
    let mut nrm = Vec::with_capacity(n);
    for v in 0..n {
        let (mut p, mut q) = (Vec3::ZERO, Vec3::ZERO);
        for s in 0..4 {
            let w = m.bw[v][s];
            if w <= 0.0 {
                continue;
            }
            let (a, t) = bones[m.bi[v][s] as usize];
            p += (a * m.pos[v] + t) * w;
            q += (a * m.nrm[v]) * w;
        }
        pos.push(post_p(p));
        nrm.push(post_n(q.normalize_or_zero()));
    }
    // emit in chunks that fit the mesh builder's per-mesh limits
    let mut map = vec![u32::MAX; n];
    let mut touched: Vec<u32> = Vec::new();
    for t in &m.idx {
        let fresh = t.iter().filter(|&&vi| map[vi as usize] == u32::MAX).count();
        if mb.v.len() + fresh + 3 > MAXV || mb.i.len() + 3 > MAXI {
            mb.flush();
            for &vi in &touched {
                map[vi as usize] = u32::MAX;
            }
            touched.clear();
        }
        for &vi in t {
            let vi = vi as usize;
            if map[vi] == u32::MAX {
                let l = Lit { c: col3(m.col[vi]), n: nrm[vi], w: 0.0 };
                map[vi] = mb.vert(pos[vi], l) as u32;
                touched.push(vi as u32);
            }
            mb.i.push(map[vi] as u16);
        }
    }
    let _ = &m.rest;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rest_joints(m: &Model) -> Joints {
        let r = |i: usize| [m.rest[i], m.rest[i + 1]];
        let head_c = (m.rest[HEAD_B] + m.rest[HEAD_T]) * 0.5;
        Joints {
            pel_base: Vec3::ZERO,
            pel_f: Vec3::X,
            chest_base: Vec3::ZERO,
            chest_f: Vec3::X,
            chest_r: Vec3::Z,
            lean: 0.0,
            hip: r(HIP),
            knee: r(KNEE),
            ank: r(ANK),
            fd: [Vec3::X; 2],
            fu: [Vec3::Y; 2],
            sh: r(SH),
            el: r(EL),
            wr: r(WR),
            hd: [m.rest[TIP] - m.rest[WR], m.rest[TIP + 1] - m.rest[WR + 1]],
            hc: head_c - Vec3::X * HEAD_FWD,
            fh: Vec3::X,
            uh: Vec3::Y,
        }
    }

    /// Writes the meshes `draw_person` produces for a few poses to %TEMP%/island_poses/*.ply, so
    /// they can be rendered in Blender.  `cargo test dump_poses -- --ignored`
    #[test]
    #[ignore]
    fn dump_poses() {
        use crate::items::ItemKind;
        use crate::people::{draw_person, Look, Pose};
        let dir = std::env::temp_dir().join("island_poses");
        std::fs::create_dir_all(&dir).unwrap();
        let look = Look::default_for(0);
        let base = |phase: f32, walk: f32| Pose {
            yaw: 0.0, phase, walk, item: None, tier: 0, lure: None, lit: false,
            eye: vec3(0.0, 1.6, 5.0), time: 1.0, swim: false, surf: 0.0, sit: false, fish: None,
        };
        let poses: Vec<(&str, Pose)> = vec![
            ("stand", base(0.0, 0.0)),
            ("walk_a", base(1.2, 1.0)),
            ("walk_b", base(4.3, 1.0)),
            ("rod", Pose { item: Some(ItemKind::Rod), ..base(0.0, 0.0) }),
            ("swim", Pose { swim: true, surf: 0.0, ..base(2.0, 0.8) }),
            ("sit", Pose { sit: true, ..base(0.0, 0.0) }),
        ];
        for (name, pose) in poses {
            let mut mb = MB::new();
            draw_person(&mut mb, Vec3::ZERO, &look, &pose);
            mb.flush();
            let (mut verts, mut faces) = (Vec::new(), Vec::new());
            for me in &mb.meshes {
                let o = verts.len();
                for v in &me.vertices {
                    verts.push((v.position, v.color, vec3(v.normal.x, v.normal.y, v.normal.z)));
                }
                for t in me.indices.chunks(3) {
                    faces.push([o + t[0] as usize, o + t[1] as usize, o + t[2] as usize]);
                }
            }
            let mut s = format!(
                "ply\nformat ascii 1.0\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nproperty float nx\nproperty float ny\nproperty float nz\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nelement face {}\nproperty list uchar int vertex_indices\nend_header\n",
                verts.len(), faces.len()
            );
            for (p, c, n) in &verts {
                s += &format!("{} {} {} {} {} {} {} {} {}\n", p.x, p.y, p.z, n.x, n.y, n.z, c[0], c[1], c[2]);
            }
            for f in &faces {
                s += &format!("3 {} {} {}\n", f[0], f[1], f[2]);
            }
            std::fs::write(dir.join(format!("{name}.ply")), s).unwrap();
            println!("POSE {name}: {} verts, {} tris, {} meshes", verts.len(), faces.len(), mb.meshes.len());
        }
    }

    #[test]
    fn model_loads() {
        let m = model().expect("assets/player.bin should parse");
        assert!(m.pos.len() > 5000 && m.idx.len() > 5000);
        let h = m.pos.iter().map(|p| p.y).fold(0.0, f32::max);
        assert!((1.6..2.0).contains(&h), "model height {h}");
    }

    #[test]
    fn rest_pose_is_identity() {
        let m = model().unwrap();
        let b = m.bones(&rest_joints(m));
        let mut worst = 0.0f32;
        for v in 0..m.pos.len() {
            let mut p = Vec3::ZERO;
            for s in 0..4 {
                let (a, t) = b[m.bi[v][s] as usize];
                p += (a * m.pos[v] + t) * m.bw[v][s];
            }
            worst = worst.max(p.distance(m.pos[v]));
        }
        assert!(worst < 1e-3, "rest pose drifted by {worst} m");
    }

    #[test]
    fn walking_pose_stays_sane() {
        let m = model().unwrap();
        let mut j = rest_joints(m);
        // swing the right leg forward and the left arm back
        j.knee[0] += Vec3::X * 0.20;
        j.ank[0] += Vec3::X * 0.30 + Vec3::Y * 0.10;
        j.el[1] += Vec3::new(-0.10, 0.05, 0.0);
        j.wr[1] += Vec3::new(-0.20, 0.10, 0.0);
        let b = m.bones(&j);
        for v in 0..m.pos.len() {
            let mut p = Vec3::ZERO;
            for s in 0..4 {
                let (a, t) = b[m.bi[v][s] as usize];
                p += (a * m.pos[v] + t) * m.bw[v][s];
            }
            assert!(p.is_finite() && p.distance(m.pos[v]) < 0.6, "vertex {v} flew off: {p:?}");
        }
    }
}
