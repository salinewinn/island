//! The game itself: title screen flow, hotbar items, shop, tent, save, multiplayer glue.

use super::*;
use crate::items::*;
use crate::net::{Mode as NetMode, Net, NetEvent, PlayerNet};
use crate::people::*;
use crate::ui::*;
use std::collections::VecDeque;

const STARTER_PRICE: i32 = 20;
const LANTERN_PRICE: i32 = 15;
const BUCKET_PRICE: i32 = 10;

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------
fn visible_d(center: Vec3, radius: f32, cam: Vec3, fwd: Vec3, right: Vec3, max_d: f32) -> bool {
    if center.distance(cam) - radius > max_d {
        return false;
    }
    visible(center, radius, cam, fwd, right)
}

fn world_to_screen(cam: &Camera3D, eye: Vec3, fwd: Vec3, p: Vec3) -> Option<Vec2> {
    if (p - eye).dot(fwd) < 0.1 {
        return None;
    }
    let clip = cam.matrix() * vec4(p.x, p.y, p.z, 1.0);
    if clip.w <= 0.01 {
        return None;
    }
    let ndc = vec3(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);
    Some(vec2((ndc.x * 0.5 + 0.5) * screen_width(), (1.0 - (ndc.y * 0.5 + 0.5)) * screen_height()))
}

struct Remote {
    net: PlayerNet,
    pos: Vec3,
    yaw: f32,
    phase: f32,
    lure: Vec3,
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    None,
    Pause(usize),
    Shop,
    Tent,
}

static DBG_N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn dbg_log(s: &str) {
    if !std::env::args().any(|a| a == "--timing") {
        return;
    }
    let n = DBG_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if n > 60 {
        return;
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(std::env::temp_dir().join("island_load.log")) {
        let _ = writeln!(f, "{:.3}s  {}", get_time(), s);
    }
}

pub fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf {
            window_title: "Island".to_owned(),
            window_width: 1280,
            window_height: 720,
            sample_count: 4,
            platform: miniquad::conf::Platform { swap_interval: Some(if crate::ui::Settings::load().vsync { 1 } else { 0 }), ..Default::default() },
            ..Default::default()
        },
        draw_call_vertex_capacity: 24000,
        draw_call_index_capacity: 30000,
        ..Default::default()
    }
}

/// Model-detail multiplier for the quality setting (Low, Medium, High, Ultra).
fn detail_mult(level: usize) -> f32 {
    [0.5, 0.75, 1.0, 1.5][level.min(3)]
}

type Hotbar = [Option<ItemKind>; 5];

fn hb_has(h: &Hotbar, k: ItemKind) -> bool {
    h.iter().any(|s| *s == Some(k))
}
fn hb_give(h: &mut Hotbar, k: ItemKind) {
    if !hb_has(h, k) {
        if let Some(s) = h.iter_mut().find(|s| s.is_none()) {
            *s = Some(k);
        }
    }
}
fn hb_take(h: &mut Hotbar, k: ItemKind) {
    for s in h.iter_mut() {
        if *s == Some(k) {
            *s = None;
        }
    }
}

fn item_name(k: ItemKind, rod_tier: usize, bait: usize, fish: usize) -> String {
    match k {
        ItemKind::Rod => RODS[rod_tier.min(2)].0.to_string(),
        ItemKind::Coconut => "Coconut".into(),
        ItemKind::Lantern => "Lantern".into(),
        ItemKind::Bait => format!("Bait: {}", BAITS[bait.min(2)].0),
        ItemKind::Bucket => format!("Fish bucket ({} fish)", fish),
    }
}

fn fish_value(f: usize, kg: f32) -> f32 {
    if f == 6 {
        FISH_PRICE[6]
    } else {
        FISH_PRICE[f] * kg
    }
}

// ---------------------------------------------------------------------------
// Save file (only written from the tent)
// ---------------------------------------------------------------------------
fn save_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("island_save.txt")))
        .unwrap_or_else(|| std::path::PathBuf::from("island_save.txt"))
}

struct SaveData {
    coins: i32,
    rod_tier: usize,
    owned_rods: [bool; 3],
    hotbar: Hotbar,
    baits: [u32; 3],
    bait_sel: usize,
    coconuts: u32,
    lantern_on: bool,
    catches: Vec<(usize, f32)>,
    best: Option<(usize, f32)>,
    sky_t: f32,
}

impl SaveData {
    fn load() -> Option<SaveData> {
        let text = std::fs::read_to_string(save_path()).ok()?;
        let mut s = SaveData {
            coins: 50,
            rod_tier: 0,
            owned_rods: [false; 3],
            hotbar: [None; 5],
            baits: [0; 3],
            bait_sel: 0,
            coconuts: 0,
            lantern_on: false,
            catches: vec![],
            best: None,
            sky_t: 330.0,
        };
        let fish = |p: &str| -> Option<(usize, f32)> {
            let (a, b) = p.trim().split_once(':')?;
            let f: usize = a.trim().parse().ok()?;
            let kg: f32 = b.trim().parse().ok()?;
            if f < FISH_TABLE.len() && kg.is_finite() {
                Some((f, kg))
            } else {
                None
            }
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "coins" => s.coins = v.parse::<i32>().unwrap_or(50).clamp(0, 99_999_999),
                "rod_tier" => s.rod_tier = v.parse::<usize>().unwrap_or(0).min(2),
                "owned_rods" => {
                    for (i, p) in v.split(',').take(3).enumerate() {
                        s.owned_rods[i] = p.trim() == "1";
                    }
                }
                "hotbar" => {
                    for (i, p) in v.split(',').take(5).enumerate() {
                        s.hotbar[i] = p.trim().parse::<u8>().ok().and_then(ItemKind::from_u8);
                    }
                }
                "baits" => {
                    for (i, p) in v.split(',').take(3).enumerate() {
                        s.baits[i] = p.trim().parse::<u32>().unwrap_or(0).min(9999);
                    }
                }
                "bait_sel" => s.bait_sel = v.parse::<usize>().unwrap_or(0).min(2),
                "coconuts" => s.coconuts = v.parse::<u32>().unwrap_or(0).min(999),
                "lantern_on" => s.lantern_on = v == "1" || v == "true",
                "catches" => s.catches = v.split(',').filter_map(fish).collect(),
                "best" => s.best = fish(v),
                "sky_t" => s.sky_t = v.parse::<f32>().ok().filter(|t| t.is_finite()).unwrap_or(330.0),
                _ => {}
            }
        }
        Some(s)
    }

    fn write(&self) -> bool {
        let mut t = String::new();
        let b = |x: bool| if x { "1" } else { "0" };
        t += &format!("coins={}\nrod_tier={}\n", self.coins, self.rod_tier);
        t += &format!("owned_rods={},{},{}\n", b(self.owned_rods[0]), b(self.owned_rods[1]), b(self.owned_rods[2]));
        let hb: Vec<String> = self.hotbar.iter().map(|s| s.map(|k| (k as u8).to_string()).unwrap_or_else(|| "-".into())).collect();
        t += &format!("hotbar={}\n", hb.join(","));
        t += &format!("baits={},{},{}\nbait_sel={}\ncoconuts={}\nlantern_on={}\n", self.baits[0], self.baits[1], self.baits[2], self.bait_sel, self.coconuts, b(self.lantern_on));
        let c: Vec<String> = self.catches.iter().map(|(f, kg)| format!("{}:{:.2}", f, kg)).collect();
        t += &format!("catches={}\n", c.join(","));
        if let Some((f, kg)) = self.best {
            t += &format!("best={}:{:.2}\n", f, kg);
        }
        t += &format!("sky_t={:.1}\n", self.sky_t);
        std::fs::write(save_path(), t).is_ok()
    }
}

// ---------------------------------------------------------------------------
// Small UI helpers
// ---------------------------------------------------------------------------
fn wrap_lines(ui: &Ui, s: &str, size: f32, w: f32) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let t = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
        if ui.text_w(&t, size) > w && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = t;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// One shop row: name + description on the left, button on the right. Returns true when clicked.
#[allow(clippy::too_many_arguments)]
fn shop_row(ui: &mut Ui, x: f32, y: f32, w: f32, name: &str, desc: &str, btn: &str, on: bool, accent: bool) -> bool {
    ui.text(name, x, y, 22.0, WHITE);
    ui.text(desc, x, y + 26.0, 17.0, Color::new(0.7, 0.75, 0.8, 1.0));
    ui.button(x + w - 130.0, y, 130.0, 46.0, btn, on, accent)
}

// ---------------------------------------------------------------------------
// The game
// ---------------------------------------------------------------------------
pub async fn run() {
    std::panic::set_hook(Box::new(|info| {
        let _ = std::fs::write(std::env::temp_dir().join("island_panic.log"), format!("{info}"));
    }));

    clear_background(Color::new(0.04, 0.07, 0.10, 1.0));
    next_frame().await;

    let args: Vec<String> = std::env::args().collect();
    let argf = |name: &str, k: usize| -> Option<f32> {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + k)).and_then(|s| s.parse::<f32>().ok())
    };
    let has_arg = |name: &str| args.iter().any(|a| a == name);

    // ---- settings, materials, title screen ----
    let mut st = Settings::load();
    crate::update::cleanup_old();
    if st.auto_update {
        crate::update::check(&st.repo());
    }
    if std::env::args().any(|a| a == "--fakeupdate") {
        crate::update::debug_fake_available();
    }
    if st.fullscreen {
        set_fullscreen(true);
    } else if st.res != 0 {
        request_new_screen_size(RESOLUTIONS[st.res].0 as f32, RESOLUTIONS[st.res].1 as f32);
    }

    let detail = make_detail();
    let world_mat = load_material(
        ShaderSource::Glsl { vertex: WORLD_VS, fragment: WORLD_FS },
        MaterialParams {
            pipeline_params: PipelineParams { depth_write: true, depth_test: Comparison::LessOrEqual, ..Default::default() },
            uniforms: vec![
                UniformDesc::new("CamPos", UniformType::Float3),
                UniformDesc::new("Under", UniformType::Float1),
                UniformDesc::new("Lum", UniformType::Float1),
                UniformDesc::new("Bump", UniformType::Float1),
                UniformDesc::new("SunDir", UniformType::Float3),
                UniformDesc::new("SunCol", UniformType::Float3),
                UniformDesc::new("AmbCol", UniformType::Float3),
                UniformDesc::new("FogCol", UniformType::Float3),
                UniformDesc::new("P0", UniformType::Float3),
                UniformDesc::new("C0", UniformType::Float3),
                UniformDesc::new("P1", UniformType::Float3),
                UniformDesc::new("C1", UniformType::Float3),
                UniformDesc::new("P2", UniformType::Float3),
                UniformDesc::new("C2", UniformType::Float3),
                UniformDesc::new("P3", UniformType::Float3),
                UniformDesc::new("C3", UniformType::Float3),
            ],
            textures: vec!["Detail".to_owned()],
        },
    )
    .unwrap();
    world_mat.set_texture("Detail", detail.clone());
    let wvs = water_vs();
    let wfs = water_fs();
    let water_mat = load_material(
        ShaderSource::Glsl { vertex: &wvs, fragment: &wfs },
        MaterialParams {
            pipeline_params: PipelineParams {
                depth_write: true,
                depth_test: Comparison::LessOrEqual,
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
            uniforms: vec![
                UniformDesc::new("CamPos", UniformType::Float3),
                UniformDesc::new("SunDir", UniformType::Float3),
                UniformDesc::new("SunCol", UniformType::Float3),
                UniformDesc::new("HorCol", UniformType::Float3),
                UniformDesc::new("ZenCol", UniformType::Float3),
                UniformDesc::new("Lum", UniformType::Float1),
                UniformDesc::new("WQ", UniformType::Float1),
                UniformDesc::new("Time", UniformType::Float1),
            ],
            textures: vec!["Detail".to_owned()],
        },
    )
    .unwrap();
    water_mat.set_texture("Detail", detail.clone());

    let mut ui = Ui::new();
    let mut look = Look::decode(&st.look).unwrap_or_else(|| Look::default_for(0));
    let has_save = save_path().exists();
    let choice = if has_arg("--skipmenu") {
        MenuChoice { new_game: false, launch: Launch::Solo }
    } else {
        let c = main_menu(&mut ui, &mut st, &mut look, has_save, &world_mat).await;
        st.look = look.encode();
        st.save();
        c
    };
    let new_game = choice.new_game || has_arg("--newgame");
    let mut launch = choice.launch;
    if has_arg("--host") {
        launch = Launch::Host;
    }
    if let Some(i) = args.iter().position(|a| a == "--join") {
        if let Some(c) = args.get(i + 1) {
            launch = Launch::Join(c.clone());
        }
    }
    let save: Option<SaveData> = if new_game { None } else { SaveData::load() };

    // ---- build the world, one labelled step at a time ----
    DETAIL.with(|d| d.set(detail_mult(st.detail)));
    macro_rules! step {
        ($p:expr, $t:expr) => {{
            draw_loading($p, $t, get_time() as f32);
            if has_arg("--timing") {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(std::env::temp_dir().join("island_load.log")) {
                    let _ = writeln!(f, "{:.2}s  {}", get_time(), $t);
                }
            }
            next_frame().await;
        }};
    }
    step!(0.02, "Shaping the island...");
    rand::srand(20_261_010);
    let terrain = Terrain::gen();
    step!(0.18, "Building the dock and the camp...");
    let sa: f32 = 4.0;
    let dock = make_dock(&terrain, sa);
    AVOID.with(|a| *a.borrow_mut() = Some(dock.clone()));
    PATH.with(|p| p.set(Some((dock.origin, vec2(PLAZA_C.0, PLAZA_C.1)))));
    let mut pool: HashMap<(i32, i32), MB> = HashMap::new();
    let (dock_lamps, mut cols) = build_dock(&mut pool, &terrain, &dock);
    let plaza = build_plaza(&mut pool, &terrain);
    let (npc_pos, npc_yaw0) = (plaza.npc_pos, plaza.npc_yaw);
    cols.extend(plaza.cols.clone());
    step!(0.34, "Raising the terrain...");
    build_terrain(&terrain, &mut pool);
    step!(0.5, "Planting palms and bushes...");
    let sc = scatter(&terrain, &mut pool);
    cols.extend(sc.cols.iter().cloned());
    for t in &sc.trunks {
        cols.push(Col::Circle { c: *t, r: 0.42, y0: -20.0, top: 1.0e9, walk: false });
    }
    let cells = finish(pool);
    step!(0.68, "Filling the sea...");
    let water = build_water(&terrain);
    let sea_ring = build_sea_ring();
    step!(0.8, "Painting the sky...");
    let clouds = build_clouds();
    let stars: Vec<(Vec3, f32, f32)> = (0..650)
        .map(|_| {
            let d = rvec();
            (vec3(d.x, d.y.abs() * 0.95 + 0.05, d.z).normalize(), rf(12.0, 26.0), rf(0.6, 1.0))
        })
        .collect();
    step!(0.88, "Waking the creatures...");
    let mut crawlers = spawn_crawlers(&terrain, &sc.nests);
    let mut swimmers = spawn_swimmers(&terrain, &dock);

    // coconuts lying around the palms and on the beaches
    rand::srand(9100); // coconut ids must match on every player's PC
    let mut coconuts: Vec<Coconut> = vec![];
    let mut next_cid = 0u32;
    for p in &sc.palms {
        for _ in 0..rand::gen_range(1, 4) {
            let a = rf(0.0, TAU);
            let d = rf(0.6, 3.0);
            let (x, z) = (p.x + a.cos() * d, p.z + a.sin() * d);
            let h = terrain.at(x, z);
            if h > 0.4 && !dock.near(x, z, 1.0) {
                coconuts.push(Coconut { id: next_cid, pos: vec3(x, h + 0.085, z), vel: Vec3::ZERO, spin: rf(0.0, TAU), rest: true });
                next_cid += 1;
            }
        }
    }
    for _ in 0..25 {
        if let Some((p, _)) = sample_land(&terrain, 0.5, 1.6, 0.9, 1.0) {
            coconuts.push(Coconut { id: next_cid, pos: p + Vec3::Y * 0.085, vel: Vec3::ZERO, spin: rf(0.0, TAU), rest: true });
            next_cid += 1;
        }
    }

    let fly_cols = [vec3(0.95, 0.55, 0.10), vec3(0.20, 0.45, 0.95), vec3(0.98, 0.85, 0.25), vec3(0.95, 0.95, 0.92)];
    let mut flies: Vec<Fly> = (0..24)
        .filter_map(|i| {
            sample_land(&terrain, 1.5, 30.0, 0.0, 1.0).map(|(p, _)| Fly {
                pos: p + Vec3::Y * 1.0,
                heading: rf(0.0, TAU),
                phase: rf(0.0, 100.0),
                col: fly_cols[i % fly_cols.len()],
            })
        })
        .collect();
    step!(0.97, "Finishing touches...");

    let ground_at = |x: f32, z: f32, fy: f32| -> f32 {
        let g = terrain.at(x, z);
        let g = if dock.contains(x, z) && fy >= dock.deck_y - 0.45 { g.max(dock.deck_y) } else { g };
        match stand_on(&cols, x, z, fy) {
            Some(t) => g.max(t),
            None => g,
        }
    };

    // ---- player / inventory state (empty-handed new game, or the save file) ----
    let mut hotbar: Hotbar = [None; 5];
    let mut coins: i32 = 50;
    let mut rod_tier: usize = 0;
    let mut owned_rods = [false; 3];
    let mut baits: [u32; 3] = [0; 3];
    let mut bait_sel: usize = 0;
    let mut held = 0u32; // coconuts
    let mut lantern_on = false;
    let mut catches: Vec<(usize, f32)> = vec![];
    let mut best: Option<(usize, f32)> = None;
    let mut sky_start = argf("--time", 1).unwrap_or(330.0);
    let continuing = save.is_some();
    if let Some(s) = save {
        coins = s.coins;
        rod_tier = s.rod_tier;
        owned_rods = s.owned_rods;
        hotbar = s.hotbar;
        baits = s.baits;
        bait_sel = s.bait_sel;
        held = s.coconuts;
        lantern_on = s.lantern_on;
        catches = s.catches;
        best = s.best;
        if argf("--time", 1).is_none() {
            sky_start = s.sky_t;
        }
        if owned_rods.iter().any(|o| *o) {
            hb_give(&mut hotbar, ItemKind::Rod);
            if !owned_rods[rod_tier] {
                rod_tier = owned_rods.iter().position(|o| *o).unwrap_or(0);
            }
        }
    }
    if has_arg("--kit") {
        coins = 500;
        owned_rods = [true, true, false];
        rod_tier = 1;
        baits = [8, 4, 0];
        held = 3;
        lantern_on = true;
        for k in [ItemKind::Rod, ItemKind::Coconut, ItemKind::Lantern, ItemKind::Bait, ItemKind::Bucket] {
            hb_give(&mut hotbar, k);
        }
        catches = vec![(0, 1.4), (2, 2.2), (6, 0.5)];
    }

    // spawn: plaza (new game) or next to the tent and campfire (continue)
    let mut feet = vec3(plaza.spawn.x, ground_at(plaza.spawn.x, plaza.spawn.z, 10.0), plaza.spawn.z);
    let mut yaw = plaza.spawn_yaw;
    let mut pitch: f32 = -0.05;
    if continuing {
        let (t2, f2) = (vec2(plaza.tent_pos.x, plaza.tent_pos.z), vec2(plaza.fire_pos.x, plaza.fire_pos.z));
        let mid = (t2 + f2) * 0.5;
        let along = (t2 - f2).normalize_or_zero();
        let mut perp = vec2(-along.y, along.x);
        if perp.dot(vec2(PLAZA_C.0, PLAZA_C.1) - mid) < 0.0 {
            perp = -perp;
        }
        let sp = mid + perp * 2.6;
        feet = vec3(sp.x, ground_at(sp.x, sp.y, 10.0), sp.y);
        let to = f2 - sp;
        yaw = to.y.atan2(to.x);
    }

    if let Some(i) = args.iter().position(|a| a == "--cam") {
        let g = |k: usize| args.get(i + k).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
        feet = vec3(g(1), ground_at(g(1), g(2), 10.0), g(2));
        yaw = g(3).to_radians();
        pitch = g(4).to_radians();
    }
    if let Some(y) = argf("--look", 1) {
        yaw = y.to_radians();
        pitch = argf("--look", 2).unwrap_or(0.0).to_radians();
    }
    if let Some(d) = argf("--walk", 1) {
        feet.x += yaw.cos() * d;
        feet.z += yaw.sin() * d;
        feet.y = ground_at(feet.x, feet.z, 10.0).max(-1.4);
    }
    // debug: stand next to something and look at it
    let look_at_target = |feet: &mut Vec3, yaw: &mut f32, pitch: &mut f32, p: Vec3, dist: f32| {
        let dir = vec2(feet.x - p.x, feet.z - p.z).normalize_or_zero();
        let dir = if dir == Vec2::ZERO { vec2(1.0, 0.0) } else { dir };
        feet.x = p.x + dir.x * dist;
        feet.z = p.z + dir.y * dist;
        feet.y = ground_at(feet.x, feet.z, 10.0);
        let to = p - (*feet + Vec3::Y * 1.7);
        *yaw = to.z.atan2(to.x);
        *pitch = (to.y / to.length()).asin();
    };
    if has_arg("--shop") {
        look_at_target(&mut feet, &mut yaw, &mut pitch, npc_pos + Vec3::Y * 1.2, 2.6);
    }
    if has_arg("--tent") {
        look_at_target(&mut feet, &mut yaw, &mut pitch, plaza.tent_pos + Vec3::Y * 0.9, 3.0);
    }
    if has_arg("--plaza") {
        look_at_target(&mut feet, &mut yaw, &mut pitch, plaza.fire_pos + Vec3::Y * 0.5, 5.0);
    }
    if has_arg("--rock") {
        if let Some(Col::Circle { c, top, .. }) = sc.cols.iter().min_by(|a, b| {
            let da = if let Col::Circle { c, .. } = a { c.distance(vec2(feet.x, feet.z)) } else { 0.0 };
            let db = if let Col::Circle { c, .. } = b { c.distance(vec2(feet.x, feet.z)) } else { 0.0 };
            da.partial_cmp(&db).unwrap()
        }) {
            look_at_target(&mut feet, &mut yaw, &mut pitch, vec3(c.x, *top * 0.6, c.y), 3.5);
        }
    }
    if has_arg("--bush") {
        let me = vec2(feet.x, feet.z);
        if let Some(b) = sc.bushes.iter().min_by(|a, b| vec2(a.x, a.z).distance(me).partial_cmp(&vec2(b.x, b.z).distance(me)).unwrap()) {
            look_at_target(&mut feet, &mut yaw, &mut pitch, *b + Vec3::Y * 0.5, 2.2);
        }
    }
    let hide_hud = has_arg("--nohud");
    let freeze = has_arg("--freeze");
    let start_menu = argf("--menu", 1);

    let mut vy = 0.0f32;
    let mut grounded = true;
    let mut bob_t = 0.0f32;
    let mut swim = false;
    let mut swim_vel = Vec3::ZERO;
    let mut slot: usize = 0;
    if let Some(s) = argf("--slot", 1) {
        slot = (s as usize).min(4);
    }
    let mut fs = FS::Idle;
    let mut cast_anim = 0.0f32;
    let mut crank = 0.0f32;
    let mut meter_flash: Option<(f32, f32)> = None; // (value, time left)
    let mut slot_label_t = 0.0f32;
    let mut toasts: VecDeque<(String, f32)> = VecDeque::new();
    if continuing {
        toasts.push_back(("Welcome back to the island.".into(), 5.0));
    } else {
        toasts.push_back(("Visit the hermit's camp to buy a rod and bait, then head down the path to the dock.".into(), 9.0));
    }
    let mut shop_msg = String::from("Haven't had company in a long while. Got fish? I've got gear.");
    let mut tent_msg = String::new();
    let mut dusk_hint = false;
    let mut next_local_cid: u32 = 0;
    let _marlo_look = Look::marlo();
    let mut remote_looks: [Look; 4] = [Look::default_for(0), Look::default_for(1), Look::default_for(2), Look::default_for(3)];

    // ---- menus / network ----
    let mut screen = if let Some(m) = start_menu {
        Screen::Pause(m as usize)
    } else if has_arg("--shopui") {
        Screen::Shop
    } else if has_arg("--tentui") {
        Screen::Tent
    } else {
        Screen::None
    };
    let mut rebinding: Option<usize> = None;
    let mut join_text = String::new();
    let mut join_focus = false;
    let mut nw = Net::new();
    let was_join = matches!(launch, Launch::Join(_));
    match launch {
        Launch::Solo => {}
        Launch::Host => {
            if let Err(e) = nw.host() {
                nw.status = e;
            } else {
                nw.send_look(&st.look);
                let _ = std::fs::write(std::env::temp_dir().join("island_code.txt"), nw.code.clone());
            }
        }
        Launch::Join(code) => nw.join(&code),
    }
    if was_join && nw.mode == NetMode::Off && !nw.status.is_empty() {
        toasts.push_back((format!("Couldn't join: {}", nw.status), 9.0));
    }
    let mut remotes: [Option<Remote>; 4] = [None, None, None, None];
    let mut net_clock: Option<(f32, f32)> = None;
    let mut send_t = 0.0f32;
    let mut time_t = 0.0f32;
    let _ = npc_yaw0;
    let mut fps_smooth = 60.0f32;
    let mut sleep: Option<(f32, f32)> = None; // (elapsed, target cycle time)
    let mut intro: Option<f32> = if !continuing && !has_arg("--skipmenu") { Some(0.0) } else { None };
    let mut welcome_toast: Option<String> = if intro.is_some() { toasts.pop_back().map(|t| t.0) } else { None };

    let mut sfx = crate::audio::Sfx::load().await;
    let mut reel_acc = 0.0f32;
    let mut grabbed = false;
    let mut skip_mouse = 3;
    let mut frame_start = get_time();
    // sky clock (host / solo): sky_t = clock_base + time
    let mut clock_base = sky_start - get_time() as f32;

    let dbg_host = has_arg("--host");
    let mut dbg_code_written = false;
    // text chat (Enter or T to talk to the other players)
    let mut chat_open = false;
    let mut chat_text = String::new();
    let mut chat_log: VecDeque<(String, f32)> = VecDeque::new();
    loop {
        dbg_log("frame start");
        frame_start = frame_start.max(0.0);
        let dt = get_frame_time().min(0.05);
        let time = get_time() as f32;
        fps_smooth = mixf(fps_smooth, 1.0 / get_frame_time().max(0.0001), 0.05);
        frame_start = get_time();

        // castaway intro: skippable with any key or click
        if let Some(t) = intro {
            let nt = t + dt;
            let skip = nt > 0.4 && (get_last_key_pressed().is_some() || is_mouse_button_pressed(MouseButton::Left));
            if nt >= 8.0 || skip {
                intro = None;
                if let Some(m) = welcome_toast.take() {
                    toasts.push_back((m, 9.0));
                }
            } else {
                intro = Some(nt);
            }
        }

        // sleeping: fade out, jump the clock at the darkest point, fade back in
        if let Some((t, target)) = sleep {
            let nt = t + dt;
            if t < 0.6 && nt >= 0.6 {
                clock_base = target - time;
                let msg = if target < DAY_LEN * 0.5 { "You slept until morning. Good morning!" } else { "You slept until night." };
                toasts.push_back((msg.into(), 4.0));
            }
            sleep = if nt >= 1.2 { None } else { Some((nt, target)) };
        }

        // sky clock: follows the host when we're a client
        let sky_t = match net_clock {
            Some((t0, at)) if nw.mode == NetMode::Client => t0 + (time - at),
            _ => clock_base + time,
        };
        let sky = sky_at(sky_t);

        // ---------- network ----------
        for e in nw.poll() {
            match e {
                NetEvent::State(s) => {
                    let id = (s.id as usize).min(3);
                    if id != nw.my_id as usize {
                        let p = vec3(s.pos[0], s.pos[1], s.pos[2]);
                        match &mut remotes[id] {
                            Some(r) => {
                                r.net = s.clone();
                                r.lure = vec3(s.lure[0], s.lure[1], s.lure[2]);
                            }
                            None => {
                                remotes[id] = Some(Remote { net: s.clone(), pos: p, yaw: s.yaw, phase: 0.0, lure: vec3(s.lure[0], s.lure[1], s.lure[2]) });
                            }
                        }
                    }
                }
                NetEvent::Join(id) => {
                    toasts.push_back((format!("Player {} joined the island", id + 1), 4.0));
                    // newcomers learn what we look like
                    nw.send_look(&st.look);
                }
                NetEvent::Leave(id) => {
                    remotes[(id as usize).min(3)] = None;
                    toasts.push_back((format!("Player {} left", id + 1), 4.0));
                }
                NetEvent::Time(t) => net_clock = Some((t, time)),
                NetEvent::Pick(id) => coconuts.retain(|c| c.id != id),
                NetEvent::Spawn { id, p, v } => {
                    coconuts.push(Coconut { id, pos: vec3(p[0], p[1], p[2]), vel: vec3(v[0], v[1], v[2]), spin: 0.0, rest: false });
                }
                NetEvent::Look(id, code) => {
                    if let Some(l) = Look::decode(&code) {
                        remote_looks[(id as usize).min(3)] = l;
                    }
                }
                NetEvent::Msg(m) => {
                    // chat lines travel as "C|<name>|<text>", everything else is a game notice
                    if let Some(rest) = m.strip_prefix("C|") {
                        if let Some((who, text)) = rest.split_once('|') {
                            chat_log.push_back((format!("{}: {}", who, text), 14.0));
                        }
                    } else {
                        toasts.push_back((m, 5.0));
                    }
                }
                NetEvent::Welcome(id) => {
                    toasts.push_back((format!("Connected! You are Player {}", id + 1), 4.0));
                    nw.send_look(&st.look);
                }
                NetEvent::Disconnected(why) => {
                    toasts.push_back((why, 5.0));
                    remotes = [None, None, None, None];
                    net_clock = None;
                }
            }
        }
        if dbg_host && !dbg_code_written && !nw.inet_code.is_empty() {
            dbg_code_written = true;
            let _ = std::fs::write(std::env::temp_dir().join("island_code.txt"), format!("{}\n{}", nw.code, nw.inet_code));
        }
        if nw.mode == NetMode::Off && remotes.iter().any(|r| r.is_some()) {
            remotes = [None, None, None, None];
        }

        // ---------- window / menu control ----------
        // ---------- chat input ----------
        let mut chat_esc = false;
        if chat_open {
            while let Some(ch) = get_char_pressed() {
                if !ch.is_control() && chat_text.chars().count() < 120 {
                    chat_text.push(ch);
                }
            }
            if is_key_pressed(KeyCode::Backspace) {
                chat_text.pop();
            }
            if is_key_pressed(KeyCode::Escape) {
                chat_open = false;
                chat_text.clear();
                chat_esc = true;
            } else if is_key_pressed(KeyCode::Enter) {
                let msg = chat_text.trim().to_string();
                if !msg.is_empty() {
                    if nw.mode == NetMode::Host || nw.mode == NetMode::Client {
                        let who = format!("Player {}", nw.my_id + 1);
                        chat_log.push_back((format!("{}: {}", who, msg), 14.0));
                        nw.send_msg(&format!("C|{}|{}", who, msg.replace('|', "/")));
                    } else {
                        toasts.push_back(("Chat only works in a multiplayer game (Esc > Multiplayer)".into(), 4.0));
                    }
                }
                chat_open = false;
                chat_text.clear();
            }
        } else if screen == Screen::None && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::T)) {
            chat_open = true;
            while get_char_pressed().is_some() {}
        }
        if is_key_pressed(KeyCode::Escape) && rebinding.is_none() && !chat_esc {
            screen = match screen {
                Screen::None => Screen::Pause(0),
                _ => {
                    st.save();
                    Screen::None
                }
            };
            join_focus = false;
        }
        // the mouse is only locked to the game while it is the active window: alt-tab, the Windows
        // key, a second monitor or holding Left Alt all give the cursor back
        let focused = window_has_focus();
        let want_grab = screen == Screen::None && focused && !is_key_down(KeyCode::LeftAlt);
        if !want_grab {
            release_cursor_clip();
        }
        if want_grab != grabbed {
            grabbed = want_grab;
            set_cursor_grab(want_grab);
            show_mouse(!want_grab);
            if want_grab {
                skip_mouse = 3;
            }
        }
        if is_key_pressed(KeyCode::F11) {
            st.fullscreen = !st.fullscreen;
            set_fullscreen(st.fullscreen);
        }
        let in_menu_now = screen != Screen::None || chat_open;

        // ---------- fishing lock (can't look/move while fighting a fish) ----------
        let fighting = matches!(fs, FS::Bite { .. } | FS::Reeling { .. });

        let md = mouse_delta_position();
        if grabbed && !fighting {
            if skip_mouse > 0 {
                skip_mouse -= 1;
            } else {
                yaw -= md.x * 1.6 * st.sens;
                let dy = if st.invert_y { -md.y } else { md.y };
                pitch = (pitch + dy * 0.9 * st.sens).clamp(-1.5, 1.5);
            }
        } else if grabbed && skip_mouse > 0 {
            skip_mouse -= 1;
        }

        let fwd = vec3(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos());
        let fwd_flat = vec3(yaw.cos(), 0.0, yaw.sin());
        let right = vec3(-yaw.sin(), 0.0, yaw.cos());

        // ---------- hotbar selection ----------
        if !in_menu_now {
            for (i, a) in [Act::Slot1, Act::Slot2, Act::Slot3, Act::Slot4, Act::Slot5].iter().enumerate() {
                if st.pressed(*a) && slot != i {
                    slot = i;
                    slot_label_t = 1.6;
                    fs = FS::Idle;
                    sfx.play(crate::audio::Snd::Tick, 1.0);
                }
            }
            let wheel = mouse_wheel().1;
            if wheel.abs() > 0.1 && !fighting {
                slot = (slot as i32 + if wheel > 0.0 { -1 } else { 1 }).rem_euclid(5) as usize;
                slot_label_t = 1.6;
                sfx.play(crate::audio::Snd::Tick, 1.0);
                fs = FS::Idle;
            }
        }
        slot_label_t -= dt;
        let sel = hotbar[slot];

        // ---------- movement ----------
        let ctrl_ok = !in_menu_now && !fighting && sleep.is_none() && intro.is_none();
        let key_f = if ctrl_ok { st.down(Act::Forward) as i32 - st.down(Act::Back) as i32 } else { 0 };
        let key_r = if ctrl_ok { st.down(Act::Right) as i32 - st.down(Act::Left) as i32 } else { 0 };
        let sprint = ctrl_ok && st.down(Act::Sprint);
        let jump_down = ctrl_ok && st.down(Act::Jump);
        let jump_pressed = ctrl_ok && st.pressed(Act::Jump);
        let down_key = ctrl_ok && st.down(Act::SwimDown);
        let ground = ground_at(feet.x, feet.z, feet.y);
        let wy = water_h(&terrain, feet.x, feet.z, time);

        if !swim && ground < -1.15 && feet.y <= wy + 0.1 {
            swim = true;
            swim_vel = Vec3::ZERO;
            vy = 0.0;
        }
        if swim && ground > -0.8 {
            swim = false;
        }

        let mut moving = false;
        let prev_feet = feet;
        if swim {
            let mut tv = fwd * key_f as f32 + right * key_r as f32;
            let up_in = jump_down as i32 - down_key as i32;
            tv.y += up_in as f32;
            let spd = if sprint { 4.0 } else { 2.4 };
            tv = tv.normalize_or_zero() * spd;
            moving = tv.length_squared() > 0.0;
            let surface_feet = wy - 1.4;
            if up_in == 0 && key_f == 0 && feet.y < surface_feet - 0.05 {
                tv.y = 0.8;
            }
            swim_vel += (tv - swim_vel) * (dt * 3.5).min(1.0);
            feet += swim_vel * dt;
            if feet.y > surface_feet - 0.25 && up_in <= 0 {
                feet.y += (surface_feet - feet.y) * (dt * 5.0).min(1.0);
            }
            if feet.y > surface_feet {
                feet.y = surface_feet;
                swim_vel.y = swim_vel.y.min(0.0);
            }
            let gnd = terrain.at(feet.x, feet.z);
            if feet.y < gnd + 0.1 {
                feet.y = gnd + 0.1;
                swim_vel.y = swim_vel.y.max(0.0);
            }
            grounded = false;
        } else {
            let mut wish = fwd_flat * key_f as f32 + right * key_r as f32;
            let mut speed = if sprint { 8.0 } else { 4.0 };
            speed *= 1.0 - smooth(0.0, -1.2, ground) * 0.55;
            moving = wish.length_squared() > 0.0;
            if moving {
                wish = wish.normalize() * speed * dt;
                feet.x += wish.x;
                feet.z += wish.z;
                if grounded {
                    bob_t += speed * dt * 1.8;
                }
            }
        }
        collide(&cols, &mut feet, 0.35);
        feet.x = feet.x.clamp(-SWIM_LIMIT, SWIM_LIMIT);
        feet.z = feet.z.clamp(-SWIM_LIMIT, SWIM_LIMIT);

        if !swim {
            let ground = ground_at(feet.x, feet.z, feet.y);
            if grounded && jump_pressed {
                vy = 4.6;
                grounded = false;
            }
            vy -= 12.0 * dt;
            feet.y += vy * dt;
            if feet.y <= ground {
                feet.y = ground;
                vy = 0.0;
                grounded = true;
            } else if grounded && feet.y - ground < 0.4 {
                feet.y = ground;
                vy = 0.0;
            } else {
                grounded = false;
            }
        }
        let walk_speed = if dt > 0.0 { (vec2(feet.x - prev_feet.x, feet.z - prev_feet.z).length() / dt).min(9.0) } else { 0.0 };

        // ---------- camera ----------
        let bob = if moving && grounded { bob_t.sin() * 0.04 } else { 0.0 };
        let eye = feet + Vec3::Y * (1.7 + bob);
        let eye_wy = water_h(&terrain, eye.x, eye.z, time);
        let under = eye.y < eye_wy - 0.02 && terrain.at(eye.x, eye.z) < eye.y;
        {
            use crate::audio::{AmbCtx, Surface};
            let on_dock = dock.contains(feet.x, feet.z) && feet.y >= dock.deck_y - 0.3;
            let on_stuff = matches!(stand_on(&cols, feet.x, feet.z, feet.y), Some(t) if (feet.y - t).abs() < 0.25);
            let th = terrain.at(feet.x, feet.z);
            let ctx = AmbCtx {
                volume: st.volume,
                feet,
                fire: plaza.fire_pos,
                day_f: sky.day_f,
                under,
                swim,
                moving,
                grounded,
                sprint,
                vy,
                walk_speed,
                surface: if on_dock || on_stuff { Surface::Wood } else if th < 1.0 { Surface::Sand } else { Surface::Grass },
                wading: !swim && !on_dock && th < -0.25,
                on_dock,
                height: &|x, z| terrain.at(x, z),
            };
            sfx.update(&ctx, dt);
        }
        let camup = right.cross(fwd).normalize_or_zero();

        // ---------- interaction targets ----------
        let mut coco_target: Option<usize> = None;
        {
            let mut bestd = f32::MAX;
            for (i, c) in coconuts.iter().enumerate() {
                let to = c.pos - eye;
                let along = to.dot(fwd);
                if along > 0.3 && along < 3.2 {
                    let perp = (to - fwd * along).length();
                    if perp < 0.30 + along * 0.12 && along < bestd {
                        bestd = along;
                        coco_target = Some(i);
                    }
                }
            }
        }
        let npc_dist = vec2(feet.x - npc_pos.x, feet.z - npc_pos.z).length();
        let npc_target = npc_dist < 3.4 && {
            let to = (npc_pos + Vec3::Y * 1.2 - eye).normalize_or_zero();
            to.dot(fwd) > 0.75
        };
        let tent_dist = vec2(feet.x - plaza.tent_pos.x, feet.z - plaza.tent_pos.z).length();
        let tent_target = tent_dist < 3.4 && {
            let to = (plaza.tent_pos + Vec3::Y * 0.8 - eye).normalize_or_zero();
            to.dot(fwd) > 0.7
        };
        if !in_menu_now && sleep.is_none() && st.pressed(Act::Interact) {
            if npc_target {
                screen = Screen::Shop;
                shop_msg = "Haven't had company in a long while. Got fish? I've got gear.".into();
            } else if tent_target {
                screen = Screen::Tent;
                tent_msg.clear();
            } else if let Some(i) = coco_target {
                let id = coconuts[i].id;
                coconuts.swap_remove(i);
                nw.send_pick(id);
                held += 1;
                sfx.play(crate::audio::Snd::Pop, 1.0);
                toasts.push_back(("Picked up a coconut".into(), 2.0));
                coco_target = None;
            }
        }
        if !in_menu_now && sel == Some(ItemKind::Coconut) {
            if held > 0 && st.pressed(Act::Drink) {
                held -= 1;
                sfx.play(crate::audio::Snd::Gulp, 1.0);
                toasts.push_back(("You cracked one open and drank it. Delicious!".into(), 3.0));
            }
            if held > 0 && st.pressed(Act::Drop) {
                held -= 1;
                let id = (nw.my_id as u32 + 1) * 1_000_000 + next_local_cid;
                next_local_cid += 1;
                sfx.play(crate::audio::Snd::Thump, 0.6);
                let (p, v) = (eye + fwd * 0.6 - camup * 0.2, fwd * 1.5);
                coconuts.push(Coconut { id, pos: p, vel: v, spin: 0.0, rest: false });
                nw.send_spawn(id, p.to_array(), v.to_array());
            }
            if held > 0 && st.pressed(Act::Use) {
                held -= 1;
                let id = (nw.my_id as u32 + 1) * 1_000_000 + next_local_cid;
                next_local_cid += 1;
                sfx.play(crate::audio::Snd::Throw, 1.0);
                let (p, v) = (eye + fwd * 0.5 - camup * 0.15, fwd * 11.0 + Vec3::Y * 1.5);
                coconuts.push(Coconut { id, pos: p, vel: v, spin: 0.0, rest: false });
                nw.send_spawn(id, p.to_array(), v.to_array());
            }
        }
        if !in_menu_now && sel == Some(ItemKind::Lantern) && st.pressed(Act::Use) {
            lantern_on = !lantern_on;
            sfx.play(crate::audio::Snd::Lantern, 1.0);
        }
        if !in_menu_now && sel == Some(ItemKind::Bait) && st.pressed(Act::Use) {
            // cycle through the baits you own
            for k in 1..=3 {
                let n = (bait_sel + k) % 3;
                if baits[n] > 0 {
                    bait_sel = n;
                    break;
                }
            }
            toasts.push_back((format!("Bait: {} ({} left)", BAITS[bait_sel].0, baits[bait_sel]), 2.0));
        }
        for c in coconuts.iter_mut() {
            if c.pos.distance(feet) < 120.0 {
                update_coconut(c, &terrain, &dock, time, dt);
            }
        }

        // ---------- fishing ----------
        let rod = RODS[rod_tier];
        let bait_luck = if baits[bait_sel] > 0 { BAITS[bait_sel].1 } else { -0.15 };
        let bait_bite = if baits[bait_sel] > 0 { BAITS[bait_sel].2 } else { 1.5 };
        let mut bend = 0.025f32;
        let mut lure: Option<Vec3> = None;
        let mut bobber: Option<Vec3> = None;
        let tip_guess = eye + right * 0.30 + fwd * 0.28 - camup * 0.30 + (fwd + camup * 0.55).normalize() * 2.4;
        if cast_anim > 0.0 {
            cast_anim = (cast_anim - dt).max(0.0);
        }
        if let Some((_, t)) = &mut meter_flash {
            *t -= dt;
        }
        if matches!(meter_flash, Some((_, t)) if t <= 0.0) {
            meter_flash = None;
        }
        let rod_sel = sel == Some(ItemKind::Rod);
        let rod_active = rod_sel && !in_menu_now;
        if !rod_sel {
            fs = FS::Idle;
        }
        if rod_active || fighting {
            let use_pressed = st.pressed(Act::Use);
            let use_down = st.down(Act::Use);
            let use_released = st.released(Act::Use);
            match fs {
                FS::Idle => {
                    if use_pressed && !swim {
                        fs = FS::Charging { t: 0.0 };
                    } else if use_pressed {
                        toasts.push_back(("You can't cast while swimming".into(), 2.0));
                    }
                }
                FS::Charging { t } => {
                    if use_down {
                        fs = FS::Charging { t: t + dt };
                    } else if use_released || !use_down {
                        let q = triangle(t);
                        meter_flash = Some((q, 1.2));
                        let dist = mixf(6.0, 30.0, q) * rod.1;
                        let mut tgt = None;
                        let mut d = dist;
                        while d >= 3.0 {
                            let p = eye + fwd_flat * d;
                            if terrain.at(p.x, p.z) < -0.5 {
                                tgt = Some(vec3(p.x, 0.0, p.z));
                                break;
                            }
                            d -= 1.0;
                        }
                        // if the full cast would land on shore, don't use that shorter spot silently
                        let full = eye + fwd_flat * dist;
                        let landed_wet = terrain.at(full.x, full.z) < -0.5;
                        match tgt {
                            Some(tg) if landed_wet => {
                                if baits[bait_sel] > 0 {
                                    baits[bait_sel] -= 1;
                                    if baits[bait_sel] == 0 {
                                        if let Some(n) = (0..3).find(|n| baits[*n] > 0) {
                                            bait_sel = n;
                                        }
                                    }
                                }
                                cast_anim = 0.45;
                                sfx.play(crate::audio::Snd::Cast, 1.0);
                                fs = FS::Flying { u: 0.0, target: tg, q };
                            }
                            _ => {
                                toasts.push_back(("The cast fell on the shore - aim at the water, or hold longer for a farther cast".into(), 3.0));
                                fs = FS::Idle;
                            }
                        }
                    }
                }
                FS::Flying { u, target, q } => {
                    let dist = vec2(target.x - eye.x, target.z - eye.z).length();
                    let nu = u + dt / (0.5 + dist / 55.0);
                    let wy_t = water_h(&terrain, target.x, target.z, time);
                    if nu >= 1.0 {
                        let bite_at = rf(4.0, 10.0) * bait_bite * rod.4 / (1.0 + 0.35 * q);
                        fs = FS::Waiting { t: 0.0, bite_at, target, q };
                        sfx.play(crate::audio::Snd::LineSplash, 1.0);
                        lure = Some(vec3(target.x, wy_t, target.z));
                    } else {
                        let p = tip_guess.lerp(vec3(target.x, wy_t, target.z), nu) + Vec3::Y * (2.0 + dist * 0.08) * (PI * nu).sin();
                        lure = Some(p);
                        fs = FS::Flying { u: nu, target, q };
                    }
                }
                FS::Waiting { t, bite_at, target, q } => {
                    let wy_t = water_h(&terrain, target.x, target.z, time);
                    let bp = vec3(target.x, wy_t, target.z) + vec3((time * 0.6).sin() * 0.05, 0.0, (time * 0.5).cos() * 0.05);
                    lure = Some(bp);
                    bobber = Some(bp);
                    if use_pressed {
                        fs = FS::Idle;
                        toasts.push_back(("Reeled in early".into(), 2.0));
                    } else if t + dt >= bite_at {
                        fs = FS::Bite { t: 0.0, target, q };
                        sfx.play(crate::audio::Snd::Bite, 1.0);
                        toasts.push_back(("Something's biting! Click or pull the mouse back to hook it!".into(), 2.5));
                    } else {
                        fs = FS::Waiting { t: t + dt, bite_at, target, q };
                    }
                }
                FS::Bite { t, target, q } => {
                    let wy_t = water_h(&terrain, target.x, target.z, time);
                    let dip = (time * 16.0).sin().abs() * 0.12 + 0.05;
                    let bp = vec3(target.x + (time * 21.0).sin() * 0.08, wy_t - dip, target.z + (time * 17.0).cos() * 0.08);
                    lure = Some(bp);
                    bobber = Some(bp);
                    bend = 0.07 + (time * 22.0).sin().abs() * 0.03;
                    if use_pressed || -md.y > 0.03 {
                        let (fish, kg) = pick_fish(rod.3 + bait_luck + 0.5 * q);
                        fs = FS::Reeling { prog: 0.3, target, q, fish, kg, t: 0.0 };
                    } else if t + dt > 1.7 {
                        fs = FS::Idle;
                        sfx.play(crate::audio::Snd::Snap, 0.8);
                        toasts.push_back(("It got away...".into(), 2.5));
                    } else {
                        fs = FS::Bite { t: t + dt, target, q };
                    }
                }
                FS::Reeling { prog, target, q, fish, kg, t } => {
                    let pull = (-md.y).max(0.0) * 0.42 * rod.2 * (1.0 + 0.3 * q);
                    let struggle = 0.55 + 0.9 * (0.5 + 0.5 * (t * 1.9 + kg).sin()).powf(2.0);
                    let fish_pull = (0.07 + 0.011 * kg.min(15.0)) * struggle;
                    let np = (prog + pull - fish_pull * dt).min(1.0);
                    crank += pull * 55.0 + dt * 2.0;
                    if pull > 0.0005 {
                        reel_acc += pull.min(0.03) * 9.0 + dt * 1.5;
                        if reel_acc >= 1.0 {
                            reel_acc = 0.0;
                            sfx.play(crate::audio::Snd::Reel, 1.0);
                        }
                    }
                    bend = 0.06 + (1.0 - np) * 0.05 + (time * 25.0).sin().abs() * 0.015;
                    let wy_t = water_h(&terrain, target.x, target.z, time);
                    let ease = np * np * (3.0 - 2.0 * np);
                    let bx = mixf(target.x, eye.x + fwd_flat.x * 1.8, ease);
                    let bz = mixf(target.z, eye.z + fwd_flat.z * 1.8, ease);
                    let bp = vec3(bx + (time * 19.0).sin() * 0.06 * (1.0 - np), wy_t - 0.03, bz);
                    lure = Some(bp);
                    bobber = Some(bp);
                    if np >= 1.0 {
                        catches.push((fish, kg));
                        sfx.play(crate::audio::Snd::Caught, 1.0);
                        if best.map(|b| kg > b.1).unwrap_or(true) {
                            best = Some((fish, kg));
                        }
                        let who = if nw.mode != NetMode::Off { format!("Player {}", nw.my_id + 1) } else { "You".to_string() };
                        let txt = if fish == 6 {
                            format!("{} hooked an old boot. Gross.", who)
                        } else {
                            format!("{} caught a {} ({:.1} kg)!", who, FISH_TABLE[fish].0, kg)
                        };
                        toasts.push_back((txt.clone(), 4.5));
                        if nw.mode != NetMode::Off {
                            nw.send_msg(&txt.replace("You ", &format!("Player {} ", nw.my_id + 1)));
                        }
                        fs = FS::Show { t: 0.0, fish, kg };
                    } else if np <= 0.0 {
                        fs = FS::Idle;
                        sfx.play(crate::audio::Snd::Snap, 1.0);
                        toasts.push_back(("The fish broke free!".into(), 3.0));
                    } else {
                        fs = FS::Reeling { prog: np, target, q, fish, kg, t: t + dt };
                    }
                }
                FS::Show { t, fish, kg } => {
                    if t + dt > 3.0 {
                        fs = FS::Idle;
                    } else {
                        fs = FS::Show { t: t + dt, fish, kg };
                    }
                }
            }
        }
        let lure_net = lure.unwrap_or(Vec3::ZERO);

        // ---------- keep the hotbar in step with what you carry ----------
        if held > 0 {
            hb_give(&mut hotbar, ItemKind::Coconut);
        } else {
            hb_take(&mut hotbar, ItemKind::Coconut);
        }
        if baits.iter().sum::<u32>() > 0 {
            hb_give(&mut hotbar, ItemKind::Bait);
            if baits[bait_sel] == 0 {
                if let Some(n) = (0..3).find(|n| baits[*n] > 0) {
                    bait_sel = n;
                }
            }
        } else {
            hb_take(&mut hotbar, ItemKind::Bait);
        }
        let sel = hotbar[slot];

        // ---------- send our state ----------
        send_t += dt;
        time_t += dt;
        if nw.mode == NetMode::Host || nw.mode == NetMode::Client {
            if send_t > 1.0 / 15.0 {
                send_t = 0.0;
                nw.send_state(&PlayerNet {
                    id: nw.my_id,
                    pos: feet.to_array(),
                    yaw,
                    pitch,
                    slot: sel.map(|k| k as u8).unwrap_or(255),
                    fstate: if sel == Some(ItemKind::Lantern) { lantern_on as u8 } else { fs.code() },
                    lure: lure_net.to_array(),
                    tier: rod_tier as u8,
                    swim,
                    speed: walk_speed,
                });
            }
            if nw.mode == NetMode::Host && time_t > 1.0 {
                time_t = 0.0;
                nw.send_time(sky_t);
            }
        }
        // smooth remote players
        for r in remotes.iter_mut().flatten() {
            let target = vec3(r.net.pos[0], r.net.pos[1], r.net.pos[2]);
            r.pos = r.pos.lerp(target, (dt * 12.0).min(1.0));
            r.yaw = lerp_angle(r.yaw, r.net.yaw, dt * 14.0);
            r.phase += r.net.speed * dt * 2.4;
        }

        // ---------- lights ----------
        let lamp_on = (1.0 - sky.day_f).clamp(0.0, 1.0);
        let sway = if moving { (bob_t * 0.5).sin() * 0.02 } else { (time * 1.3).sin() * 0.006 };
        let lantern_hand = eye - right * 0.30 + fwd * 0.50 - camup * (0.38 + (time * 1.1).sin() * 0.005) + right * sway * 0.6;
        let lit_now = sel == Some(ItemKind::Lantern) && lantern_on;
        let flick = (1.0 + 0.05 * (time * 13.0).sin() + 0.04 * (time * 7.3 + 2.0).sin() + 0.02 * (time * 29.0).sin()).clamp(0.9, 1.1);
        let fire_flick = (1.0 + 0.12 * (time * 11.0).sin() + 0.08 * (time * 17.3 + 1.0).sin() + 0.05 * (time * 31.0).sin()).clamp(0.78, 1.2);
        if !dusk_hint && sky.day_f < 0.45 && sky.sun_el < 0.05 && sky_t.rem_euclid(CYCLE) > DAY_LEN * 0.5 {
            dusk_hint = true;
            if let Some(i) = hotbar.iter().position(|s| *s == Some(ItemKind::Lantern)) {
                let key = bind_label(st.bind([Act::Slot1, Act::Slot2, Act::Slot3, Act::Slot4, Act::Slot5][i]));
                toasts.push_back((format!("It's getting dark - equip your lantern ({})", key), 5.0));
            }
        }
        if sky.day_f > 0.8 {
            dusk_hint = false;
        }

        // ---------- render ----------
        clear_background(Color::new(0.04 * sky.lum, 0.22 * sky.lum, 0.40 * sky.lum, 1.0));
        let cam = Camera3D {
            position: eye,
            target: eye + fwd,
            up: Vec3::Y,
            fovy: st.fov.to_radians(),
            z_near: 0.03,
            z_far: 9000.0,
            ..Default::default()
        };
        set_camera(&cam);

        draw_sky(eye, &sky);
        draw_stars(eye, &sky, &stars);
        if sky.sun_el > -0.15 {
            let sc_col = vec3(1.0, 0.98, 0.88).lerp(vec3(1.0, 0.55, 0.25), sky.twi);
            draw_sphere(eye + sky.sun_dir * 5000.0, 140.0, None, col3(sc_col));
        }
        if sky.moon_el > -0.15 {
            draw_sphere(eye + sky.moon_dir * 5000.0, 115.0, None, Color::new(0.92, 0.95, 1.0, 1.0));
        }

        world_mat.set_uniform("CamPos", eye);
        world_mat.set_uniform("Under", if under { 1.0f32 } else { 0.0 });
        world_mat.set_uniform("Lum", sky.lum);
        world_mat.set_uniform("Bump", if st.bump { 1.0f32 } else { 0.0 });
        world_mat.set_uniform("SunDir", sky.light_dir);
        world_mat.set_uniform("SunCol", sky.light_col);
        world_mat.set_uniform("AmbCol", sky.amb);
        world_mat.set_uniform("FogCol", sky.horizon);
        world_mat.set_uniform("P0", lantern_hand + camup * 0.14);
        world_mat.set_uniform("C0", if lit_now { vec3(1.0, 0.76, 0.42) * 2.4 * flick } else { Vec3::ZERO });
        world_mat.set_uniform("P1", dock_lamps[0]);
        world_mat.set_uniform("C1", vec3(1.0, 0.72, 0.38) * 2.6 * lamp_on);
        world_mat.set_uniform("P2", dock_lamps[1]);
        world_mat.set_uniform("C2", vec3(1.0, 0.72, 0.38) * 2.6 * lamp_on);
        world_mat.set_uniform("P3", plaza.fire_pos + Vec3::Y * 0.7);
        world_mat.set_uniform("C3", vec3(1.0, 0.55, 0.2) * 3.0 * fire_flick);
        dbg_log("sky done");
        gl_use_material(&world_mat);
        for m in &clouds {
            draw_mesh(m);
        }
        for c in &cells {
            if visible_d(c.center, c.radius, eye, fwd, right, st.draw_dist) {
                for m in &c.meshes {
                    draw_mesh(m);
                }
            }
        }

        // dynamic things
        let mut dynmb = MB::new();
        for c in crawlers.iter_mut() {
            let d = c.pos.distance(feet);
            if d < 80.0 && !freeze {
                update_crawler(c, &terrain, feet, dt);
            }
            if d < 35.0 && visible(c.pos, 0.2, eye, fwd, right) {
                draw_crawler(c, &mut dynmb);
            }
        }
        for b in flies.iter_mut() {
            update_fly(b, &terrain, time, dt);
            if b.pos.distance(eye) < 60.0 && visible(b.pos, 0.2, eye, fwd, right) {
                draw_fly(b, time, &mut dynmb);
            }
        }
        for f in swimmers.iter_mut() {
            if f.pos.distance(feet) < 90.0 {
                update_swimmer(f, &terrain, dt);
            }
            if f.pos.distance(eye) < 40.0 && visible(f.pos, 0.4, eye, fwd, right) {
                let wag = (time * 7.0 * f.speed + f.phase).sin();
                fish_shape(&mut dynmb, f.pos, f.heading, f.size, f.col, wag);
            }
        }
        for (i, c) in coconuts.iter().enumerate() {
            if c.pos.distance(eye) < 70.0 && visible(c.pos, 0.2, eye, fwd, right) {
                draw_coconut(c, &mut dynmb, Some(i) == coco_target);
            }
        }
        // the campfire is always burning
        if plaza.fire_pos.distance(eye) < 160.0 {
            draw_campfire_flames(&mut dynmb, plaza.fire_pos + Vec3::Y * 0.1, time);
        }
        // other players
        for (i, r) in remotes.iter().enumerate() {
            if let Some(r) = r {
                if r.pos.distance(eye) < 120.0 {
                    let item = ItemKind::from_u8(r.net.slot);
                    let rod_out = item == Some(ItemKind::Rod) && r.net.fstate >= 2;
                    let lure_o = if rod_out { Some(r.lure) } else { None };
                    let walk = (r.net.speed / 4.0).min(1.0);
                    draw_person(
                        &mut dynmb,
                        r.pos,
                        &remote_looks[i],
                        &Pose {
                            yaw: r.yaw,
                            phase: r.phase,
                            walk,
                            item,
                            tier: r.net.tier.min(2),
                            lure: lure_o,
                            lit: item == Some(ItemKind::Lantern) && r.net.fstate == 1,
                            eye,
                            time,
                        },
                    );
                    if lure_o.is_some() {
                        blob(&mut dynmb, r.lure + Vec3::Y * 0.03, Vec3::splat(0.045), vec3(0.9, 0.12, 0.10), 0.0, 8, 5, 0.0, shade);
                    }
                }
            }
        }
        // what's in our hands
        match sel {
            Some(ItemKind::Rod) => {
                let mut rod_dir = (fwd + camup * 0.55 - right * 0.06).normalize();
                if let FS::Charging { t } = fs {
                    // pull the rod back while charging
                    let m = triangle(t);
                    rod_dir = (fwd + camup * (0.55 + 0.6 * m) - right * 0.06).normalize();
                }
                if cast_anim > 0.0 {
                    let u = 1.0 - cast_anim / 0.45;
                    let swing = if u < 0.45 { -(u / 0.45) } else { -1.0 + ((u - 0.45) / 0.55) * 1.2 };
                    rod_dir = (fwd + camup * (0.55 - swing * 0.9)).normalize();
                }
                let base = eye + right * 0.30 + fwd * 0.28 - camup * 0.30 + camup * sway;
                draw_rod(&mut dynmb, base, rod_dir, -camup, right, bend, eye, rod_tier, crank, lure, bobber, time);
                if let FS::Show { t, fish, kg } = fs {
                    let fc = FISH_TABLE[fish].4;
                    let wob = (t * 9.0).sin();
                    let pos = eye + fwd * 0.75 + right * 0.02 + camup * (0.05 + (t * 2.0).sin() * 0.01);
                    let szf = if fish == 6 { 1.4 } else { 1.0 + kg.sqrt() * 0.45 };
                    if fish == 6 {
                        blob(&mut dynmb, pos, vec3(0.14, 0.09, 0.07), vec3(0.25, 0.18, 0.12), 0.1, 6, 4, yaw, shade);
                    } else {
                        fish_shape(&mut dynmb, pos, yaw + PI * 0.5, szf * 0.9, vec3(fc[0], fc[1], fc[2]), wob);
                    }
                }
            }
            Some(ItemKind::Coconut) if held > 0 => {
                let bobv = (time * 1.6).sin() * 0.01 + if moving { (bob_t * 0.5).sin() * 0.015 } else { 0.0 };
                let hp = eye + fwd * 0.42 + right * 0.19 - camup * (0.17 - bobv);
                blob(&mut dynmb, hp, vec3(0.115, 0.085, 0.085), vec3(0.45, 0.32, 0.17), 0.0, 8, 5, yaw + 0.6, shade);
            }
            Some(ItemKind::Lantern) => {
                lantern_model(&mut dynmb, lantern_hand, 1.0, yaw, lantern_on);
            }
            Some(ItemKind::Bait) => {
                let hp = eye + fwd * 0.45 + right * 0.2 - camup * 0.2;
                draw_bait_tin(&mut dynmb, hp, camup);
            }
            Some(ItemKind::Bucket) => {
                let hp = eye + fwd * 0.5 + right * 0.2 - camup * 0.28;
                draw_bucket(&mut dynmb, hp, camup);
            }
            _ => {}
        }
        dbg_log("dynamic built");
        dynmb.flush();
        for m in &dynmb.meshes {
            draw_mesh(m);
        }
        gl_use_default_material();

        // water
        water_mat.set_uniform("CamPos", eye);
        water_mat.set_uniform("SunDir", sky.light_dir);
        water_mat.set_uniform("SunCol", sky.light_col);
        water_mat.set_uniform("HorCol", sky.horizon);
        water_mat.set_uniform("ZenCol", sky.zenith);
        water_mat.set_uniform("Lum", sky.lum);
        water_mat.set_uniform("WQ", st.water_q as f32);
        water_mat.set_uniform("Time", time);
        dbg_log("world drawn");
        gl_use_material(&water_mat);
        for w in &water {
            if visible_d(w.center, 26.0, eye, fwd, right, st.draw_dist + 100.0) {
                draw_mesh(&w.mesh);
            }
        }
        draw_mesh(&sea_ring);
        gl_use_default_material();

        // ---------- HUD ----------
        set_default_camera();
        ui.begin();
        if under {
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.28, 0.38, 0.22 * sky.lum + 0.08));
        }
        let (sw, sh) = (screen_width(), screen_height());
        let (cx, cy) = (sw * 0.5, sh * 0.5);
        let hs = (sh / 720.0).clamp(0.6, 2.5);
        let shadow_text = |s: &str, x: f32, y: f32, size: f32, c: Color| {
            draw_text(s, x + 1.5, y + 1.5, size, Color::new(0.0, 0.0, 0.0, 0.65));
            draw_text(s, x, y, size, c);
        };
        let slot_keys: [String; 5] = std::array::from_fn(|i| bind_label(st.bind([Act::Slot1, Act::Slot2, Act::Slot3, Act::Slot4, Act::Slot5][i])));
        if !hide_hud {
            if screen == Screen::None {
                draw_circle(cx, cy, 2.0, Color::new(1.0, 1.0, 1.0, 0.7));
            }
            // name tags
            for (i, r) in remotes.iter().enumerate() {
                if let Some(r) = r {
                    if let Some(p) = world_to_screen(&cam, eye, fwd, r.pos + Vec3::Y * 2.05) {
                        let label = format!("Player {}", i + 1);
                        let c = PLAYER_COLORS[i];
                        let tw = measure_text(&label, None, (22.0 * hs) as u16, 1.0).width;
                        shadow_text(&label, p.x - tw * 0.5, p.y, 22.0 * hs, Color::new(c[0] * 0.5 + 0.5, c[1] * 0.5 + 0.5, c[2] * 0.5 + 0.5, 1.0));
                    }
                }
            }
            // top-right: clock and money
            if st.show_clock {
                let hh = sky.clock.floor() as i32;
                let mm = ((sky.clock - sky.clock.floor()) * 60.0) as i32;
                let clock = format!("{:02}:{:02}", hh, mm);
                let tw = measure_text(&clock, None, (26.0 * hs) as u16, 1.0).width;
                shadow_text(&clock, sw - tw - 16.0, 30.0 * hs, 26.0 * hs, WHITE);
            }
            let money = format!("{} coins", coins);
            let tw = measure_text(&money, None, (26.0 * hs) as u16, 1.0).width;
            shadow_text(&money, sw - tw - 16.0, 30.0 * hs + if st.show_clock { 28.0 * hs } else { 0.0 }, 26.0 * hs, Color::new(0.5, 0.95, 0.5, 1.0));
            if st.show_fps {
                shadow_text(&format!("{:.0} FPS", fps_smooth), 14.0, 24.0 * hs, 22.0 * hs, Color::new(1.0, 1.0, 0.6, 1.0));
            }
            // always-visible multiplayer status so a failed join can't go unnoticed
            {
                let players = nw.names.iter().filter(|n| n.is_some()).count();
                let (msg, col) = match nw.mode {
                    NetMode::Host => (format!("ONLINE - hosting, {} player{} (code: Esc > Multiplayer)", players, if players == 1 { "" } else { "s" }), Color::new(0.5, 1.0, 0.6, 1.0)),
                    NetMode::Client => (format!("ONLINE - you are Player {}, {} players in this world", nw.my_id + 1, players), Color::new(0.5, 1.0, 0.6, 1.0)),
                    NetMode::Connecting => ("Connecting to the host...".to_string(), Color::new(1.0, 0.9, 0.5, 1.0)),
                    NetMode::Off if was_join => ("NOT CONNECTED - you are alone in your own world (Esc > Multiplayer for the reason)".to_string(), Color::new(1.0, 0.45, 0.4, 1.0)),
                    _ => (String::new(), WHITE),
                };
                if !msg.is_empty() {
                    shadow_text(&msg, 14.0, 24.0 * hs + if st.show_fps { 26.0 * hs } else { 0.0 }, 18.0 * hs, col);
                }
            }
            if st.show_hints {
                let l = |s: &str, y: f32| shadow_text(s, 14.0, y * hs + if st.show_fps { 26.0 * hs } else { 0.0 }, 22.0 * hs, WHITE);
                l("Hotbar 1-5 / mouse wheel: choose what is in your hand", 50.0);
                l("Hold click to charge a cast, release to throw. Pull the mouse back to reel.", 74.0);
                l("E interact   F drink   G drop   Esc menu", 98.0);
                let sp = crawlers.iter().filter(|c| c.kind == Kind::Spider && c.pos.distance(feet) < 8.0).count();
                shadow_text(&format!("Coconuts: {}   Fish: {}   Spiders nearby: {}", held, catches.len(), sp), 14.0, sh - 90.0 * hs, 22.0 * hs, WHITE);
            }
            // prompts
            if screen == Screen::None && sleep.is_none() {
                let ikey = bind_label(st.bind(Act::Interact));
                let prompt = if npc_target {
                    Some(format!("[{}] Trade at the counter", ikey))
                } else if tent_target {
                    Some(format!("[{}] Rest at the tent", ikey))
                } else if coco_target.is_some() {
                    Some(format!("[{}] Pick up coconut", ikey))
                } else {
                    None
                };
                if let Some(s) = prompt {
                    let tw = measure_text(&s, None, (28.0 * hs) as u16, 1.0).width;
                    shadow_text(&s, cx - tw * 0.5, cy + 60.0 * hs, 28.0 * hs, WHITE);
                }
            }
            // chat box and recent chat lines (bottom left, above the hotbar row)
            for c in chat_log.iter_mut() {
                c.1 -= dt;
            }
            while chat_log.len() > 40 || matches!(chat_log.front(), Some((_, t)) if *t <= -30.0) {
                chat_log.pop_front();
            }
            {
                let show: Vec<&(String, f32)> = chat_log.iter().filter(|(_, t)| chat_open || *t > 0.0).collect();
                let n = show.len().min(8);
                let base = sh - 120.0 * hs;
                for (i, (s, t)) in show.iter().rev().take(n).enumerate() {
                    let a = if chat_open { 1.0 } else { (*t / 1.5).clamp(0.0, 1.0) };
                    let y = base - i as f32 * 24.0 * hs - if chat_open { 34.0 * hs } else { 0.0 };
                    draw_text(s, 15.0, y + 1.5, 22.0 * hs, Color::new(0.0, 0.0, 0.0, 0.7 * a));
                    draw_text(s, 14.0, y, 22.0 * hs, Color::new(0.95, 0.97, 1.0, a));
                }
                if chat_open {
                    let by = sh - 118.0 * hs;
                    draw_rectangle(10.0, by, 520.0 * hs, 30.0 * hs, Color::new(0.0, 0.0, 0.0, 0.65));
                    draw_rectangle_lines(10.0, by, 520.0 * hs, 30.0 * hs, 2.0, Color::new(0.5, 0.9, 0.8, 0.9));
                    let cursor = if (get_time() * 2.0) as i32 % 2 == 0 { "|" } else { "" };
                    draw_text(&format!("{}{}", chat_text, cursor), 18.0, by + 22.0 * hs, 22.0 * hs, WHITE);
                    draw_text("Enter = send, Esc = cancel", 14.0, by - 6.0 * hs, 16.0 * hs, Color::new(1.0, 1.0, 1.0, 0.55));
                }
            }
            // toasts
            for t in toasts.iter_mut() {
                t.1 -= dt;
            }
            while matches!(toasts.front(), Some((_, t)) if *t <= 0.0) {
                toasts.pop_front();
            }
            while toasts.len() > 5 {
                toasts.pop_front();
            }
            for (i, (s, t)) in toasts.iter().enumerate() {
                let a = (t / 0.6).min(1.0);
                let tw = measure_text(s, None, (24.0 * hs) as u16, 1.0).width;
                let y = 120.0 * hs + i as f32 * 30.0 * hs;
                draw_text(s, cx - tw * 0.5 + 1.5, y + 1.5, 24.0 * hs, Color::new(0.0, 0.0, 0.0, 0.65 * a));
                draw_text(s, cx - tw * 0.5, y, 24.0 * hs, Color::new(1.0, 0.95, 0.72, a));
            }
            // cast power meter (hold to charge; green = far and better fish)
            let meter = match fs {
                FS::Charging { t } => Some((triangle(t), true)),
                _ => meter_flash.map(|(v, _)| (v, false)),
            };
            if let Some((m, live)) = meter {
                let (bw, bh) = (26.0 * hs, 260.0 * hs);
                let bx = cx + 140.0 * hs;
                let by = cy - bh * 0.5;
                draw_rectangle(bx - 4.0, by - 4.0, bw + 8.0, bh + 8.0, Color::new(0.0, 0.0, 0.0, 0.55));
                let n = 40;
                for k in 0..n {
                    let u = k as f32 / n as f32; // 0 = bottom
                    let col = if u < 0.5 { Color::new(0.9, 0.15 + 1.5 * u * 0.9, 0.12, 1.0) } else { Color::new(0.9 - (u - 0.5) * 1.5, 0.82, 0.12, 1.0) };
                    let seg = bh / n as f32;
                    draw_rectangle(bx, by + bh - (k + 1) as f32 * seg, bw, seg + 0.5, col);
                }
                // green zone marker
                draw_line(bx - 8.0, by + bh * 0.25, bx + bw + 8.0, by + bh * 0.25, 2.0, Color::new(1.0, 1.0, 1.0, 0.5));
                let my = by + bh - m * bh;
                draw_rectangle(bx - 10.0, my - 3.0, bw + 20.0, 6.0, WHITE);
                draw_triangle(vec2(bx - 12.0, my - 9.0), vec2(bx - 12.0, my + 9.0), vec2(bx - 1.0, my), WHITE);
                let lab = if live { "HOLD...".to_string() } else if m > 0.8 { "PERFECT!".to_string() } else if m > 0.5 { "Good".to_string() } else { "Weak".to_string() };
                shadow_text(&lab, bx - 6.0, by - 12.0, 24.0 * hs, WHITE);
            }
            // reel bar
            if let FS::Reeling { prog, .. } = fs {
                let (bw, bh) = (380.0 * hs, 22.0 * hs);
                let bx = cx - bw * 0.5;
                let by = sh - 150.0 * hs;
                draw_rectangle(bx - 4.0, by - 4.0, bw + 8.0, bh + 8.0, Color::new(0.0, 0.0, 0.0, 0.6));
                let col = if prog < 0.25 { Color::new(0.9, 0.25, 0.2, 1.0) } else { Color::new(0.3, 0.85, 0.5, 1.0) };
                draw_rectangle(bx, by, bw * prog, bh, col);
                shadow_text("Pull the mouse back to reel it in!", cx - 170.0 * hs, by - 12.0 * hs, 24.0 * hs, WHITE);
            } else if let FS::Bite { .. } = fs {
                shadow_text("FISH ON! Click or pull the mouse back!", cx - 190.0 * hs, sh - 160.0 * hs, 28.0 * hs, Color::new(1.0, 0.9, 0.4, 1.0));
            }

            // hotbar
            {
                let mut entries: [Option<HotbarEntry>; 5] = [None; 5];
                for i in 0..5 {
                    if let Some(k) = hotbar[i] {
                        let count = match k {
                            ItemKind::Coconut => Some(held),
                            ItemKind::Bait => Some(baits[bait_sel]),
                            ItemKind::Bucket => Some(catches.len() as u32),
                            _ => None,
                        };
                        entries[i] = Some(HotbarEntry { kind: k, count, tier: rod_tier, lit: lantern_on });
                    }
                }
                draw_hotbar(&entries, slot, &slot_keys, hs);
                let y0 = sh - 62.0 * hs - 6.0 * hs;
                if slot_label_t > 0.0 {
                    if let Some(k) = sel {
                        let label = item_name(k, rod_tier, bait_sel, catches.len());
                        let tw = measure_text(&label, None, (24.0 * hs) as u16, 1.0).width;
                        shadow_text(&label, cx - tw * 0.5, y0 - 18.0 * hs, 24.0 * hs, WHITE);
                    }
                }
                // bucket contents panel
                if sel == Some(ItemKind::Bucket) && screen == Screen::None {
                    let mut counts = [0u32; 7];
                    let mut total_val = 0.0;
                    for (f, kg) in &catches {
                        counts[*f] += 1;
                        total_val += fish_value(*f, *kg);
                    }
                    let px = cx - 150.0 * hs;
                    let py = y0 - 40.0 * hs - 8.0 * 22.0 * hs;
                    draw_rectangle(px, py, 300.0 * hs, 8.0 * 22.0 * hs + 10.0, Color::new(0.04, 0.06, 0.1, 0.8));
                    shadow_text(&format!("Bucket - worth about {:.0} coins", total_val), px + 10.0, py + 20.0 * hs, 20.0 * hs, Color::new(0.6, 0.95, 0.6, 1.0));
                    for (i, n) in counts.iter().enumerate() {
                        if *n > 0 {
                            shadow_text(&format!("{} x{}", FISH_TABLE[i].0, n), px + 10.0, py + (20.0 + (i as f32 + 1.0) * 21.0) * hs, 19.0 * hs, WHITE);
                        }
                    }
                    if let Some((f, kg)) = best {
                        shadow_text(&format!("Best catch: {} {:.1} kg", FISH_TABLE[f].0, kg), px + 10.0, py + 8.0 * 22.0 * hs, 18.0 * hs, Color::new(1.0, 0.9, 0.5, 1.0));
                    }
                }
            }
        }

        // ---------- menus ----------
        if screen != Screen::None {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.5));
        }
        match screen {
            Screen::None => {}
            Screen::Pause(tab) => {
                let mut tab = tab;
                ui.rect(180.0, 60.0, 920.0, 600.0, COL_PANEL);
                ui.outline(180.0, 60.0, 920.0, 600.0, 2.0, Color::new(1.0, 1.0, 1.0, 0.2));
                ui.text("ISLAND", 205.0, 78.0, 34.0, Color::new(0.9, 0.95, 1.0, 1.0));
                if ui.button(205.0, 135.0, 190.0, 44.0, "Resume", true, true) {
                    st.save();
                    screen = Screen::None;
                }
                let tabs = ["Graphics", "Display", "Controls", "Multiplayer", "Character"];
                for (i, t) in tabs.iter().enumerate() {
                    let y = 195.0 + i as f32 * 54.0;
                    if ui.button(205.0, y, 190.0, 44.0, t, tab != i, false) {
                        tab = i;
                        rebinding = None;
                        join_focus = false;
                    }
                }
                if ui.button(205.0, 600.0, 190.0, 44.0, "Quit game", true, false) {
                    st.save();
                    std::process::exit(0);
                }
                let (x, w) = (430.0f32, 640.0f32);
                let dim = Color::new(0.65, 0.7, 0.75, 1.0);
                match tab {
                    0 => tab_graphics(&mut ui, &mut st, x, w),
                    1 => tab_display(&mut ui, &mut st, x, w),
                    2 => tab_controls(&mut ui, &mut st, &mut rebinding, x),
                    4 => {
                        let before = look.clone();
                        tab_character(&mut ui, &mut look, x, w, time, &world_mat);
                        if look != before {
                            st.look = look.encode();
                            nw.send_look(&st.look);
                        }
                    }
                    _ => {
                        ui.text("Multiplayer (up to 4 players)", x, 80.0, 30.0, WHITE);
                        match nw.mode {
                            NetMode::Off | NetMode::Connecting => {
                                if ui.button(x, 130.0, 260.0, 46.0, "Host a game", nw.mode == NetMode::Off, true) {
                                    match nw.host() {
                                        Ok(()) => nw.send_look(&st.look),
                                        Err(e) => nw.status = e,
                                    }
                                }
                                ui.text("Join with a room code:", x, 200.0, 22.0, WHITE);
                                // text field
                                ui.rect(x, 234.0, 330.0, 44.0, Color::new(0.1, 0.12, 0.16, 1.0));
                                ui.outline(x, 234.0, 330.0, 44.0, 2.0, if join_focus { COL_ACCENT } else { Color::new(1.0, 1.0, 1.0, 0.25) });
                                if ui.clicked {
                                    join_focus = ui.hover(x, 234.0, 330.0, 44.0);
                                }
                                let shown = if join_text.is_empty() && !join_focus { "ABCDE-FGHJK-MNP".to_string() } else { format!("{}{}", join_text, if join_focus && (time * 2.0) as i32 % 2 == 0 { "|" } else { "" }) };
                                ui.text(&shown, x + 12.0, 242.0, 28.0, if join_text.is_empty() && !join_focus { Color::new(1.0, 1.0, 1.0, 0.3) } else { WHITE });
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
                                            join_text = c.trim().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '.' || *c == ':').take(26).collect::<String>().to_ascii_uppercase();
                                        }
                                    }
                                    if is_key_pressed(KeyCode::Enter) && nw.mode == NetMode::Off {
                                        nw.join(&join_text.clone());
                                    }
                                } else {
                                    while get_char_pressed().is_some() {}
                                }
                                if ui.button(x + 345.0, 234.0, 120.0, 44.0, "Join", nw.mode == NetMode::Off, true) {
                                    nw.join(&join_text.clone());
                                }
                                for (i, l) in wrap_lines(&ui, &nw.status, 20.0, w).iter().take(3).enumerate() {
                                    ui.text(l, x, 292.0 + i as f32 * 25.0, 20.0, Color::new(1.0, 0.85, 0.5, 1.0));
                                }
                                let notes = [
                                    "LAN code: friends on the same Wi-Fi/network or VPN (Tailscale, Hamachi).",
                                    "Internet code: friends anywhere in the world - no router setup, just share it.",
                                    "You can also type the host's IP address (for example 192.168.1.20) instead of a code.",
                                    "Windows may ask to allow the game through the firewall when you host.",
                                ];
                                let mut ny = 400.0;
                                for n in notes {
                                    for l in wrap_lines(&ui, n, 18.0, w) {
                                        ui.text(&l, x, ny, 18.0, dim);
                                        ny += 23.0;
                                    }
                                    ny += 6.0;
                                }
                            }
                            NetMode::Host | NetMode::Client => {
                                let mut ypos = 130.0;
                                if nw.mode == NetMode::Host {
                                    ui.text("LAN code - friends on the same Wi-Fi/network or VPN", x, 124.0, 19.0, Color::new(0.8, 0.85, 0.9, 1.0));
                                    ui.text(&nw.code, x, 150.0, 40.0, Color::new(1.0, 0.92, 0.55, 1.0));
                                    if ui.button(x + 490.0, 152.0, 150.0, 38.0, "Copy LAN code", true, false) {
                                        miniquad::window::clipboard_set(&nw.code);
                                        toasts.push_back(("LAN code copied".into(), 2.0));
                                    }
                                    ui.text("Internet code - friends anywhere in the world", x, 205.0, 19.0, Color::new(0.8, 0.85, 0.9, 1.0));
                                    if nw.inet_code.is_empty() {
                                        ui.text("(not available yet)", x, 231.0, 30.0, Color::new(1.0, 1.0, 1.0, 0.3));
                                    } else {
                                        ui.text(&nw.inet_code, x, 231.0, 40.0, Color::new(0.6, 0.95, 1.0, 1.0));
                                    }
                                    if ui.button(x + 490.0, 233.0, 150.0, 38.0, "Copy net code", !nw.inet_code.is_empty(), false) {
                                        miniquad::window::clipboard_set(&nw.inet_code);
                                        toasts.push_back(("Internet code copied".into(), 2.0));
                                    }
                                    for (i, l) in wrap_lines(&ui, &nw.inet_status, 17.0, w).iter().take(3).enumerate() {
                                        ui.text(l, x, 282.0 + i as f32 * 21.0, 17.0, Color::new(1.0, 0.85, 0.5, 1.0));
                                    }
                                    if nw.inet_code.is_empty() && !nw.inet_status.starts_with("Opening") {
                                        let busy = nw.inet_status.starts_with("Looking up");
                                        if ui.button(x, 350.0, 290.0, 36.0, "Look up my public address", !busy, false) {
                                            nw.lookup_public();
                                        }
                                        ui.text("Backup if the tunnel is down: needs a router port forward.", x + 300.0, 359.0, 16.0, dim);
                                    }
                                    ypos = 400.0;
                                } else {
                                    for (i, l) in wrap_lines(&ui, &nw.status, 22.0, w).iter().take(3).enumerate() {
                                        ui.text(l, x, 130.0 + i as f32 * 28.0, 22.0, Color::new(1.0, 0.85, 0.5, 1.0));
                                    }
                                    ypos = 240.0;
                                }
                                ui.text("Players", x, ypos, 24.0, WHITE);
                                for i in 0..4 {
                                    let name = nw.names[i].clone().unwrap_or_else(|| "(empty slot)".to_string());
                                    let c = PLAYER_COLORS[i];
                                    let ry = ypos + 34.0 + i as f32 * 32.0;
                                    ui.rect(x, ry, 22.0, 22.0, Color::new(c[0], c[1], c[2], if nw.names[i].is_some() { 1.0 } else { 0.25 }));
                                    let me = if nw.names[i].is_some() && i as u8 == nw.my_id { "  (you)" } else { "" };
                                    ui.text(&format!("{}{}", name, me), x + 34.0, ry, 21.0, if nw.names[i].is_some() { WHITE } else { Color::new(1.0, 1.0, 1.0, 0.35) });
                                }
                                let label = if nw.mode == NetMode::Host { "Stop hosting" } else { "Leave game" };
                                if ui.button(x, 600.0, 220.0, 44.0, label, true, false) {
                                    nw.leave();
                                    nw.status.clear();
                                    remotes = [None, None, None, None];
                                    net_clock = None;
                                }
                            }
                        }
                    }
                }
                if let Screen::Pause(_) = screen {
                    screen = Screen::Pause(tab);
                }
            }
            Screen::Shop => {
                let (px, py, pw, ph) = (190.0f32, 36.0f32, 900.0f32, 648.0f32);
                ui.rect(px, py, pw, ph, COL_PANEL);
                ui.outline(px, py, pw, ph, 2.0, Color::new(1.0, 0.9, 0.6, 0.35));
                ui.text("Trading Counter", px + 24.0, py + 14.0, 34.0, Color::new(1.0, 0.9, 0.6, 1.0));
                let ctext = format!("Coins: {}", coins);
                let cw = ui.text_w(&ctext, 28.0);
                ui.text(&ctext, px + pw - 24.0 - cw, py + 18.0, 28.0, Color::new(0.5, 0.95, 0.5, 1.0));
                ui.text(&format!("\"{}\"", shop_msg), px + 24.0, py + 62.0, 20.0, Color::new(0.85, 0.85, 0.9, 1.0));
                // sell
                let total: f32 = catches.iter().map(|(f, kg)| fish_value(*f, *kg)).sum();
                ui.rect(px + 24.0, py + 96.0, pw - 48.0, 56.0, Color::new(0.1, 0.13, 0.18, 1.0));
                ui.text(&format!("Your catch: {} fish, worth {:.0} coins", catches.len(), total), px + 40.0, py + 112.0, 22.0, WHITE);
                if ui.button(px + pw - 24.0 - 250.0 - 8.0, py + 102.0, 250.0, 44.0, "Sell all fish", !catches.is_empty(), true) {
                    coins += total.round() as i32;
                    sfx.play(crate::audio::Snd::Coins, 1.0);
                    shop_msg = format!("Fine catch! Here's {:.0} coins.", total);
                    catches.clear();
                }
                let (lx, rx, cw2) = (px + 24.0, px + 24.0 + 438.0, 414.0f32);
                let hcol = Color::new(0.8, 0.9, 1.0, 1.0);
                ui.text("Gear", lx, py + 168.0, 26.0, hcol);
                ui.text("Bait  (one is used up per cast)", rx, py + 168.0, 26.0, hcol);
                let any_rod = owned_rods.iter().any(|o| *o);
                for i in 0..3 {
                    let y = py + 208.0 + i as f32 * 68.0;
                    let r = RODS[i];
                    let desc = if i == 0 { "A simple, reliable rod".to_string() } else { format!("Cast +{:.0}%  Reel +{:.0}%  Better fish", (r.1 - 1.0) * 100.0, (r.2 - 1.0) * 100.0) };
                    let price = if i == 0 { STARTER_PRICE } else { r.5 };
                    if owned_rods[i] {
                        let equipped = rod_tier == i;
                        if shop_row(&mut ui, lx, y, cw2, r.0, &desc, if equipped { "Equipped" } else { "Equip" }, !equipped, false) {
                            rod_tier = i;
                            hb_give(&mut hotbar, ItemKind::Rod);
                            shop_msg = format!("The {} suits you!", r.0);
                        }
                    } else if i == 0 && any_rod {
                        shop_row(&mut ui, lx, y, cw2, r.0, "Only for first-time anglers", "Not needed", false, false);
                    } else if shop_row(&mut ui, lx, y, cw2, r.0, &desc, &format!("{} coins", price), coins >= price, true) {
                        coins -= price;
                        sfx.play(crate::audio::Snd::Coin, 1.0);
                        owned_rods[i] = true;
                        rod_tier = i;
                        hb_give(&mut hotbar, ItemKind::Rod);
                        shop_msg = format!("Enjoy your new {}!", r.0);
                    }
                }
                let has_lantern = hb_has(&hotbar, ItemKind::Lantern);
                let y = py + 208.0 + 3.0 * 68.0;
                if has_lantern {
                    shop_row(&mut ui, lx, y, cw2, "Lantern", "Lights your way after dark", "Owned", false, false);
                } else if shop_row(&mut ui, lx, y, cw2, "Lantern", "Lights your way after dark", &format!("{} coins", LANTERN_PRICE), coins >= LANTERN_PRICE, true) {
                    coins -= LANTERN_PRICE;
                    sfx.play(crate::audio::Snd::Coin, 1.0);
                    hb_give(&mut hotbar, ItemKind::Lantern);
                    shop_msg = "Click with it in hand to light it. Stay safe out there.".into();
                }
                let has_bucket = hb_has(&hotbar, ItemKind::Bucket);
                let y = py + 208.0 + 4.0 * 68.0;
                if has_bucket {
                    shop_row(&mut ui, lx, y, cw2, "Fish bucket", "See what you've caught so far", "Owned", false, false);
                } else if shop_row(&mut ui, lx, y, cw2, "Fish bucket", "See what you've caught so far", &format!("{} coins", BUCKET_PRICE), coins >= BUCKET_PRICE, true) {
                    coins -= BUCKET_PRICE;
                    sfx.play(crate::audio::Snd::Coin, 1.0);
                    hb_give(&mut hotbar, ItemKind::Bucket);
                    shop_msg = "Hold it up to see your catch.".into();
                }
                for i in 0..3 {
                    let y = py + 208.0 + i as f32 * 68.0;
                    let b = BAITS[i];
                    let desc = format!("Pack of {}   Luck +{:.0}%   Faster bites", b.4, b.1 * 100.0);
                    if shop_row(&mut ui, rx, y, cw2, &format!("{}  (you have {})", b.0, baits[i]), &desc, &format!("{} coins", b.3), coins >= b.3, true) {
                        coins -= b.3;
                        sfx.play(crate::audio::Snd::Coin, 1.0);
                        baits[i] += b.4;
                        bait_sel = i;
                        hb_give(&mut hotbar, ItemKind::Bait);
                        shop_msg = format!("{} {} - good choice.", b.4, b.0);
                    }
                }
                if ui.button(lx, py + ph - 62.0, 160.0, 42.0, "Close", true, false) {
                    screen = Screen::None;
                    st.save();
                }
            }
            Screen::Tent => {
                let (px, py, pw, ph) = (400.0f32, 150.0f32, 480.0f32, 420.0f32);
                ui.rect(px, py, pw, ph, COL_PANEL);
                ui.outline(px, py, pw, ph, 2.0, Color::new(1.0, 1.0, 1.0, 0.25));
                ui.text("Your tent", px + 24.0, py + 16.0, 34.0, Color::new(0.9, 0.95, 1.0, 1.0));
                let hh = sky.clock.floor() as i32;
                let mm = ((sky.clock - sky.clock.floor()) * 60.0) as i32;
                ui.text(&format!("The campfire crackles. It is {:02}:{:02}.", hh, mm), px + 24.0, py + 62.0, 20.0, Color::new(0.8, 0.85, 0.9, 1.0));
                let bx = px + 40.0;
                let bw = pw - 80.0;
                if ui.button(bx, py + 100.0, bw, 46.0, "Sleep until morning", true, false) {
                    if nw.mode == NetMode::Client {
                        tent_msg = "Only the host can change the time".into();
                        toasts.push_back((tent_msg.clone(), 3.0));
                    } else {
                        sleep = Some((0.0, 20.0));
                        sfx.play(crate::audio::Snd::Chime, 1.0);
                        screen = Screen::None;
                    }
                }
                if ui.button(bx, py + 160.0, bw, 46.0, "Sleep until night", true, false) {
                    if nw.mode == NetMode::Client {
                        tent_msg = "Only the host can change the time".into();
                        toasts.push_back((tent_msg.clone(), 3.0));
                    } else {
                        sleep = Some((0.0, DAY_LEN + 90.0));
                        sfx.play(crate::audio::Snd::Chime, 1.0);
                        screen = Screen::None;
                    }
                }
                if ui.button(bx, py + 220.0, bw, 46.0, "Save game", true, true) {
                    let data = SaveData { coins, rod_tier, owned_rods, hotbar, baits, bait_sel, coconuts: held, lantern_on, catches: catches.clone(), best, sky_t };
                    sfx.play(crate::audio::Snd::Chime, 0.8);
                    tent_msg = if data.write() { "Game saved.".into() } else { "Couldn't write the save file.".into() };
                    st.save();
                }
                if ui.button(bx, py + 280.0, bw, 46.0, "Close", true, false) {
                    screen = Screen::None;
                    st.save();
                }
                if !tent_msg.is_empty() {
                    ui.text_c(&tent_msg, px + pw * 0.5, py + 358.0, 22.0, Color::new(1.0, 0.9, 0.5, 1.0));
                }
            }
        }
        if screen == Screen::None && join_focus {
            join_focus = false;
        }

        // sleep fade
        if let Some((t, _)) = sleep {
            let a = (1.0 - ((t - 0.6).abs() / 0.6)).clamp(0.0, 1.0);
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, a));
        }

        if let Some(t) = intro {
            let a = 1.0 - smooth(2.0, 8.0, t);
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, a));
            let lines = ["Your boat ran out of fuel and the current carried you here.", "Salvage what you can. Fish to survive.", "Find the hermit's camp."];
            let out = 1.0 - smooth(6.8, 8.0, t);
            for (i, l) in lines.iter().enumerate() {
                let ta = ((t - (0.6 + 2.3 * i as f32)) / 0.9).clamp(0.0, 1.0) * out;
                if ta > 0.0 {
                    let tw = measure_text(l, None, (30.0 * hs) as u16, 1.0).width;
                    draw_text(l, cx - tw * 0.5, cy - 40.0 * hs + i as f32 * 46.0 * hs, 30.0 * hs, Color::new(0.95, 0.93, 0.85, ta));
                }
            }
        }

        // ---------- frame limiter ----------
        let cap = FPS_OPTS[st.fps.min(FPS_OPTS.len() - 1)];
        if cap > 0 && !st.vsync {
            let target = 1.0 / cap as f64;
            loop {
                let used = get_time() - frame_start;
                if used >= target {
                    break;
                }
                if target - used > 0.002 {
                    std::thread::sleep(std::time::Duration::from_micros(((target - used - 0.0015) * 1.0e6) as u64));
                }
            }
        }
        next_frame().await
    }
}

/// Is our window the one the user is working in? (false after alt-tab / Windows key / clicking
/// another monitor)
#[cfg(windows)]
fn window_has_focus() -> bool {
    extern "system" {
        fn GetForegroundWindow() -> isize;
        fn GetWindowThreadProcessId(h: isize, pid: *mut u32) -> u32;
        fn GetCurrentProcessId() -> u32;
    }
    unsafe {
        let h = GetForegroundWindow();
        if h == 0 {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, &mut pid);
        pid == GetCurrentProcessId()
    }
}

#[cfg(not(windows))]
fn window_has_focus() -> bool {
    true
}

/// Make sure Windows isn't still confining the cursor to the game window.
#[cfg(windows)]
fn release_cursor_clip() {
    extern "system" {
        fn ClipCursor(r: *const i32) -> i32;
    }
    unsafe {
        ClipCursor(std::ptr::null());
    }
}

#[cfg(not(windows))]
fn release_cursor_clip() {}
