//! The game itself: title screen flow, hotbar items, shop, tent, save, multiplayer glue.

use super::*;
use crate::boat::{self, Boat, BoatView};
use crate::fish::{self, FishPose, SPECIES};
use crate::items::*;
use crate::net::{Mode as NetMode, Net, NetEvent, PlayerNet};
use crate::people::*;
use crate::ui::*;
use std::collections::VecDeque;

const STARTER_PRICE: i32 = 20;
const LANTERN_PRICE: i32 = 15;

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
    /// where their boat is (smoothed)
    bpos: Vec3,
    byaw: f32,
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

/// a caught fish riding in a hotbar slot: (species, kg)
type FishSlots = [Option<(usize, f32)>; 5];

fn hb_has(h: &Hotbar, k: ItemKind) -> bool {
    h.iter().any(|s| *s == Some(k))
}
fn hb_free(h: &Hotbar) -> Option<usize> {
    h.iter().position(|s| s.is_none())
}
/// can this item be put in the hotbar (it is already there, or there is a free slot)?
fn hb_can_give(h: &Hotbar, k: ItemKind) -> bool {
    hb_has(h, k) || hb_free(h).is_some()
}
fn hb_give(h: &mut Hotbar, k: ItemKind) -> bool {
    if hb_has(h, k) {
        return true;
    }
    if let Some(s) = h.iter_mut().find(|s| s.is_none()) {
        *s = Some(k);
        return true;
    }
    false
}
fn hb_take(h: &mut Hotbar, k: ItemKind) {
    for s in h.iter_mut() {
        if *s == Some(k) {
            *s = None;
        }
    }
}

fn item_name(k: ItemKind, rod_tier: usize, bait: usize, fish: Option<(usize, f32)>) -> String {
    match k {
        ItemKind::Rod => RODS[rod_tier.min(N_RODS - 1)].0.to_string(),
        ItemKind::Coconut => "Coconut".into(),
        ItemKind::Lantern => "Lantern".into(),
        ItemKind::Bait => format!("Bait: {}", BAITS[bait.min(N_BAITS - 1)].0),
        ItemKind::Bucket => "Bucket".into(),
        ItemKind::Fish => match fish {
            Some((f, kg)) if f == fish::BOOT => format!("{} (worth {} coins)", SPECIES[f].name, fish::fish_value(f, kg)),
            Some((f, kg)) => format!("{}  {:.1} kg  (worth {} coins)", SPECIES[f].name, kg, fish::fish_value(f, kg)),
            None => "Fish".into(),
        },
    }
}

/// Add (or take away) money for this player and everybody else in the game.
fn add_coins(coins: &mut i32, nw: &mut Net, d: i32) {
    *coins = (*coins as i64 + d as i64).clamp(0, 99_999_999) as i32;
    nw.coin_delta(d);
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
    owned_rods: [bool; N_RODS],
    hotbar: Hotbar,
    fish_slots: FishSlots,
    baits: [u32; N_BAITS],
    bait_sel: usize,
    coconuts: u32,
    lantern_on: bool,
    best: Option<(usize, f32)>,
    sky_t: f32,
    upg: [bool; 3],
    has_key: bool,
}

impl SaveData {
    fn load() -> Option<SaveData> {
        let text = std::fs::read_to_string(save_path()).ok()?;
        let mut s = SaveData {
            coins: 50,
            rod_tier: 0,
            owned_rods: [false; N_RODS],
            hotbar: [None; 5],
            fish_slots: [None; 5],
            baits: [0; N_BAITS],
            bait_sel: 0,
            coconuts: 0,
            lantern_on: false,
            best: None,
            sky_t: 330.0,
            upg: [false; 3],
            has_key: false,
        };
        let fish = |p: &str| -> Option<(usize, f32)> {
            let (a, b) = p.trim().split_once(':')?;
            let f: usize = a.trim().parse().ok()?;
            let kg: f32 = b.trim().parse().ok()?;
            if f < SPECIES.len() && kg.is_finite() && kg > 0.0 {
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
                "rod_tier" => s.rod_tier = v.parse::<usize>().unwrap_or(0).min(N_RODS - 1),
                "owned_rods" => {
                    for (i, p) in v.split(',').take(N_RODS).enumerate() {
                        s.owned_rods[i] = p.trim() == "1";
                    }
                }
                "hotbar" => {
                    for (i, p) in v.split(',').take(5).enumerate() {
                        s.hotbar[i] = p.trim().parse::<u8>().ok().and_then(ItemKind::from_u8).filter(|k| *k != ItemKind::Bucket);
                    }
                }
                "fish" => {
                    for (i, p) in v.split(',').take(5).enumerate() {
                        s.fish_slots[i] = fish(p);
                    }
                }
                "baits" => {
                    for (i, p) in v.split(',').take(N_BAITS).enumerate() {
                        s.baits[i] = p.trim().parse::<u32>().unwrap_or(0).min(9999);
                    }
                }
                "bait_sel" => s.bait_sel = v.parse::<usize>().unwrap_or(0).min(N_BAITS - 1),
                "coconuts" => s.coconuts = v.parse::<u32>().unwrap_or(0).min(999),
                "lantern_on" => s.lantern_on = v == "1" || v == "true",
                "upg" => {
                    for (i, p) in v.split(',').take(3).enumerate() {
                        s.upg[i] = p.trim() == "1";
                    }
                }
                "key" => s.has_key = v == "1" || v == "true",
                "best" => s.best = fish(v),
                "sky_t" => s.sky_t = v.parse::<f32>().ok().filter(|t| t.is_finite()).unwrap_or(330.0),
                _ => {}
            }
        }
        // a fish item needs its fish, and a fish needs its item
        for i in 0..5 {
            if s.hotbar[i] == Some(ItemKind::Fish) && s.fish_slots[i].is_none() {
                s.hotbar[i] = None;
            }
            if s.hotbar[i] != Some(ItemKind::Fish) {
                s.fish_slots[i] = None;
            }
        }
        Some(s)
    }

    fn write(&self) -> bool {
        let mut t = String::new();
        let b = |x: bool| if x { "1" } else { "0" };
        t += &format!("coins={}\nrod_tier={}\n", self.coins, self.rod_tier);
        let or: Vec<&str> = self.owned_rods.iter().map(|x| b(*x)).collect();
        t += &format!("owned_rods={}\n", or.join(","));
        let hb: Vec<String> = self.hotbar.iter().map(|s| s.map(|k| (k as u8).to_string()).unwrap_or_else(|| "-".into())).collect();
        t += &format!("hotbar={}\n", hb.join(","));
        let fs: Vec<String> = self.fish_slots.iter().map(|s| s.map(|(f, kg)| format!("{}:{:.2}", f, kg)).unwrap_or_else(|| "-".into())).collect();
        t += &format!("fish={}\n", fs.join(","));
        let bs: Vec<String> = self.baits.iter().map(|n| n.to_string()).collect();
        t += &format!("baits={}\nbait_sel={}\ncoconuts={}\nlantern_on={}\n", bs.join(","), self.bait_sel, self.coconuts, b(self.lantern_on));
        t += &format!("upg={},{},{}\nkey={}\n", b(self.upg[0]), b(self.upg[1]), b(self.upg[2]), b(self.has_key));
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
/// Runs the game until the player quits to the desktop. Quitting to the main menu starts a new session.
pub async fn run() {
    let mut first = true;
    while session(first).await {
        first = false;
    }
}

/// One visit to the island: main menu, loading, playing. Returns true to go back to the main menu.
async fn session(first: bool) -> bool {
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
    if first {
        crate::update::cleanup_old();
        if st.auto_update {
            crate::update::check(&st.repo());
        }
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
    let choice = if has_arg("--skipmenu") && first {
        MenuChoice { new_game: false, launch: Launch::Solo }
    } else {
        let c = main_menu(&mut ui, &mut st, &mut look, has_save, &world_mat).await;
        st.look = look.encode();
        st.save();
        c
    };
    let new_game = choice.new_game || (has_arg("--newgame") && first);
    let mut launch = choice.launch;
    if has_arg("--host") && first {
        launch = Launch::Host;
    }
    if let (true, Some(i)) = (first, args.iter().position(|a| a == "--join")) {
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
    let mut monsters = spawn_monsters(&terrain);
    let mut boat = boat::start_boat(&terrain, &dock);

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
    let mut fish_slots: FishSlots = [None; 5];
    let mut coins: i32 = 50;
    let mut rod_tier: usize = 0;
    let mut owned_rods = [false; N_RODS];
    let mut baits: [u32; N_BAITS] = [0; N_BAITS];
    let mut bait_sel: usize = 0;
    let mut held = 0u32; // coconuts
    let mut lantern_on = false;
    let mut best: Option<(usize, f32)> = None;
    let mut upg = [false; 3]; // braided line, steel leader, pro reel
    let mut has_key = false; // the boat key (found inside the first deep-water monster you land)
    let mut sky_start = argf("--time", 1).unwrap_or(330.0);
    let continuing = save.is_some();
    if let Some(s) = save {
        coins = s.coins;
        rod_tier = s.rod_tier;
        owned_rods = s.owned_rods;
        hotbar = s.hotbar;
        fish_slots = s.fish_slots;
        baits = s.baits;
        bait_sel = s.bait_sel;
        held = s.coconuts;
        lantern_on = s.lantern_on;
        best = s.best;
        upg = s.upg;
        has_key = s.has_key;
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
        owned_rods = [true, true, true, true];
        rod_tier = 3;
        baits = [8, 4, 3, 3, 3];
        held = 3;
        lantern_on = true;
        has_key = has_arg("--key");
        for k in [ItemKind::Rod, ItemKind::Coconut, ItemKind::Lantern, ItemKind::Bait] {
            hb_give(&mut hotbar, k);
        }
        if has_arg("--fish") {
            hotbar[4] = Some(ItemKind::Fish);
            fish_slots[4] = Some((2, 2.2));
        }
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
    if has_arg("--dock") {
        let p = dock.point(dock.len - 1.2, 0.0, dock.deck_y);
        feet = vec3(p.x, dock.deck_y, p.z);
        yaw = dock.dir.y.atan2(dock.dir.x);
        pitch = -0.12;
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
    let mut riding = false;
    let mut board_once = has_arg("--board");
    let mut frame_no = 0u32;
    let autopull = has_arg("--autopull");
    let mut leave: Option<bool> = None; // Some(true) = back to the main menu, Some(false) = quit to the desktop
    let mut shop_tab = 0usize;
    let mut name_focus = false;
    let mut swim_t = 0.0f32; // first-person swim stroke phase
    let mut thrash_snd = 0.0f32;
    let mut tension_snd = 0.0f32;
    let mut monster_cd = 0.0f32;
    let mut was_thrashing = false;
    let mut engine_t = 0.0f32;
    let mut last_stroke = 0.0f32;
    let mut fish_flash = 0.0f32; // brief highlight when a catch is stored
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
    nw.set_name(&st.name);
    nw.set_balance(coins);
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
    let mut known_name = [false; 4];
    let mut net_clock: Option<(f32, f32)> = None;
    let mut send_t = 0.0f32;
    let mut time_t = 0.0f32;
    let _ = npc_yaw0;
    let mut fps_smooth = 60.0f32;
    let mut sleep: Option<(f32, f32)> = None; // (elapsed, target cycle time)
    let mut intro: Option<f32> = if !continuing && !(has_arg("--skipmenu") && first) { Some(0.0) } else { None };
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
        frame_no += 1;
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
                                remotes[id] = Some(Remote { net: s.clone(), pos: p, yaw: s.yaw, phase: 0.0, lure: vec3(s.lure[0], s.lure[1], s.lure[2]), bpos: vec3(s.boat[0], s.boat[1], s.boat[2]), byaw: s.byaw });
                            }
                        }
                    }
                }
                NetEvent::Join(_) => {
                    // newcomers learn what we look like (their name arrives in a moment)
                    nw.send_look(&st.look);
                }
                NetEvent::Name(id, name) => {
                    let i = (id as usize).min(3);
                    if i != nw.my_id as usize && !known_name[i] {
                        known_name[i] = true;
                        toasts.push_back((format!("{} joined the island", name), 4.0));
                    }
                }
                NetEvent::Coins(v) => coins = v,
                NetEvent::Leave(id) => {
                    let i = (id as usize).min(3);
                    let who = nw.names[i].clone().unwrap_or_else(|| format!("Player {}", i + 1));
                    remotes[i] = None;
                    known_name[i] = false;
                    toasts.push_back((format!("{} left", who), 4.0));
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
                    toasts.push_back((format!("Connected! You are player {}", id + 1), 4.0));
                    nw.send_look(&st.look);
                }
                NetEvent::Disconnected(why) => {
                    toasts.push_back((why, 5.0));
                    known_name = [false; 4];
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
                        let who = nw.names[nw.my_id as usize].clone().unwrap_or_else(|| format!("Player {}", nw.my_id + 1));
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
            if name_focus {
                name_focus = false;
                st.name = crate::net::clean_name(&st.name);
                nw.set_name(&st.name);
            }
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
        let fighting = matches!(fs, FS::Bite { .. } | FS::Reeling(_));
        let holding = matches!(fs, FS::Holding { .. });

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
                if !st.pressed(*a) {
                    continue;
                }
                if let FS::Holding { fish, kg, .. } = fs {
                    // the catch goes into the slot you press, if it is free
                    if hotbar[i].is_none() {
                        hotbar[i] = Some(ItemKind::Fish);
                        fish_slots[i] = Some((fish, kg));
                        slot = i;
                        slot_label_t = 2.5;
                        fish_flash = 1.0;
                        fs = FS::Idle;
                        sfx.play(crate::audio::Snd::Pop, 1.0);
                        toasts.push_back((format!("{} stored in slot {}", SPECIES[fish].name, i + 1), 3.0));
                    } else {
                        toasts.push_back((format!("Slot {} is full - press a free slot, or G to let the fish go", i + 1), 3.0));
                    }
                } else if slot != i {
                    slot = i;
                    slot_label_t = 1.6;
                    fs = FS::Idle;
                    sfx.play(crate::audio::Snd::Tick, 1.0);
                }
            }
            let wheel = mouse_wheel().1;
            if wheel.abs() > 0.1 && !fighting && !holding {
                slot = (slot as i32 + if wheel > 0.0 { -1 } else { 1 }).rem_euclid(5) as usize;
                slot_label_t = 1.6;
                sfx.play(crate::audio::Snd::Tick, 1.0);
                fs = FS::Idle;
            }
        }
        slot_label_t -= dt;
        let sel = hotbar[slot];

        // ---------- movement ----------
        if let (true, Some(k)) = (first, args.iter().position(|a| a == "--fightdemo")) {
            // debug: start a fight with species N (weight in kg after it)
            if frame_no == 30 {
                let sp = args.get(k + 1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(2).min(fish::N_SPECIES - 1);
                let kg = args.get(k + 2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(2.0);
                let tg = dock.point(dock.len + 14.0, 0.0, 0.0);
                let mut f = Fight::new(sp, kg, vec3(tg.x, 0.0, tg.z), 0.8);
                f.prog = 0.55;
                f.thrash = if has_arg("--thrash") { 0.9 } else { 0.0 };
                f.thr_left = if has_arg("--thrash") { 99.0 } else { 0.0 };
                f.tension = 0.7;
                fs = FS::Reeling(f);
            }
        }
        if let (true, Some(k)) = (first, args.iter().position(|a| a == "--hold")) {
            if frame_no == 30 {
                let sp = args.get(k + 1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(2).min(fish::N_SPECIES - 1);
                fs = FS::Holding { t: 0.5, fish: sp, kg: args.get(k + 2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(2.0) };
            }
        }
        if has_arg("--leavetest") && first && frame_no == 240 {
            leave = Some(true);
        }
        if board_once {
            board_once = false;
            riding = true;
            boat.speed = 0.0;
            feet = boat.seat();
            yaw = boat.yaw;
            swim = false;
        }
        let ctrl_ok = !in_menu_now && !fighting && sleep.is_none() && intro.is_none();
        let key_f = if ctrl_ok { st.down(Act::Forward) as i32 - st.down(Act::Back) as i32 } else { 0 };
        let key_r = if ctrl_ok { st.down(Act::Right) as i32 - st.down(Act::Left) as i32 } else { 0 };
        let sprint = ctrl_ok && st.down(Act::Sprint);
        let jump_down = ctrl_ok && st.down(Act::Jump);
        let jump_pressed = ctrl_ok && st.pressed(Act::Jump);
        let down_key = ctrl_ok && st.down(Act::SwimDown);
        let ground = ground_at(feet.x, feet.z, feet.y);
        let wy = water_h(&terrain, feet.x, feet.z, time);

        if !riding && !swim && ground < -1.15 && feet.y <= wy + 0.1 {
            swim = true;
            swim_vel = Vec3::ZERO;
            vy = 0.0;
        }
        if swim && ground > -0.8 {
            swim = false;
        }

        let mut moving = false;
        let prev_feet = feet;
        if !riding {
            // an empty boat just floats where you left it
            boat.update(dt, 0.0, 0.0, false, has_key, &terrain, time, &|x, z| terrain.at(x, z) > -0.8 || dock.near(x, z, 1.1));
        }
        if riding {
            let throttle = key_f as f32;
            let steer = key_r as f32;
            boat.update(dt, throttle, steer, sprint, has_key, &terrain, time, &|x, z| terrain.at(x, z) > -0.8 || dock.near(x, z, 1.1));
            feet = boat.seat();
            vy = 0.0;
            grounded = true;
            swim = false;
            moving = boat.speed.abs() > 0.3;
        } else if swim {
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
        if !riding {
            collide(&cols, &mut feet, 0.35);
        }
        feet.x = feet.x.clamp(-SWIM_LIMIT, SWIM_LIMIT);
        feet.z = feet.z.clamp(-SWIM_LIMIT, SWIM_LIMIT);

        if !swim && !riding {
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
        let bob = if moving && grounded && !riding { bob_t.sin() * 0.04 } else { 0.0 };
        let eye = feet + Vec3::Y * (if riding { 1.22 } else { 1.7 } + bob);
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
        let boat_dist = vec2(feet.x - boat.pos.x, feet.z - boat.pos.z).length();
        let boat_target = !riding && boat_dist < 3.8 && {
            let to = (boat.pos + Vec3::Y * 0.3 - eye).normalize_or_zero();
            to.dot(fwd) > 0.55
        };
        if !in_menu_now && sleep.is_none() && st.pressed(Act::Interact) {
            if riding {
                if !fighting {
                    // step out: onto the shore or the dock if it is close, otherwise into the water
                    let bf = boat.forward();
                    let br = vec3(-bf.z, 0.0, bf.x);
                    let mut spot: Option<Vec3> = None;
                    'exit: for dist in [1.9f32, 2.7, 3.5] {
                        for d in [br, -br, bf, -bf] {
                            let p = boat.pos + d * dist;
                            let g = ground_at(p.x, p.z, 10.0);
                            if g > -0.6 {
                                spot = Some(vec3(p.x, g, p.z));
                                break 'exit;
                            }
                        }
                    }
                    riding = false;
                    vy = 0.0;
                    match spot {
                        Some(p) => {
                            feet = p;
                            grounded = true;
                            swim = false;
                        }
                        None => {
                            let p = boat.pos + br * 1.5;
                            feet = vec3(p.x, water_h(&terrain, p.x, p.z, time) - 1.35, p.z);
                            swim = true;
                            swim_vel = Vec3::ZERO;
                            grounded = false;
                        }
                    }
                    fs = FS::Idle;
                    sfx.play(crate::audio::Snd::Thump, 0.6);
                }
            } else if boat_target {
                riding = true;
                boat.speed = 0.0;
                feet = boat.seat();
                swim = false;
                grounded = true;
                fs = FS::Idle;
                sfx.play(crate::audio::Snd::Thump, 0.8);
                toasts.push_back((format!("W/S row, A/D steer{}", if has_key { ", Shift = full throttle" } else { "" }), 4.0));
            } else if npc_target {
                screen = Screen::Shop;
                shop_msg = "Haven't had company in a long while. Got fish? I've got gear.".into();
            } else if tent_target {
                screen = Screen::Tent;
                tent_msg.clear();
            } else if let Some(i) = coco_target {
                if !hb_can_give(&hotbar, ItemKind::Coconut) {
                    toasts.push_back(("Your hotbar is full - free a slot first".into(), 3.0));
                } else {
                    let id = coconuts[i].id;
                    coconuts.swap_remove(i);
                    nw.send_pick(id);
                    held += 1;
                    sfx.play(crate::audio::Snd::Pop, 1.0);
                    toasts.push_back(("Picked up a coconut".into(), 2.0));
                    coco_target = None;
                }
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
            for k in 1..=N_BAITS {
                let n = (bait_sel + k) % N_BAITS;
                if baits[n] > 0 {
                    bait_sel = n;
                    break;
                }
            }
            toasts.push_back((format!("Bait: {} ({} left)", BAITS[bait_sel].0, baits[bait_sel]), 2.0));
        }
        // let go of a fish you are carrying
        if !in_menu_now && sel == Some(ItemKind::Fish) && st.pressed(Act::Drop) {
            if let Some((f, _)) = fish_slots[slot] {
                toasts.push_back((format!("You let the {} go", SPECIES[f].name.to_lowercase()), 2.5));
            }
            hotbar[slot] = None;
            fish_slots[slot] = None;
            sfx.play(crate::audio::Snd::Splash, 0.5);
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
        // how much force the line takes, and how strongly the reel pulls
        let line_cap = 10.0 * rod.8 * if upg[0] { 1.25 } else { 1.0 } * if upg[1] { 1.6 } else { 1.0 };
        let reel_pow = rod.2 * if upg[2] { 1.25 } else { 1.0 };
        let mut bend = 0.025f32;
        let mut lure: Option<Vec3> = None;
        let mut bobber: Option<Vec3> = None;
        // a fish on the line that we draw: (species, kg, position, thrash, progress)
        let mut fish_on_line: Option<(usize, f32, Vec3, f32, f32)> = None;
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
        if !rod_sel && !holding {
            fs = FS::Idle;
        }
        thrash_snd -= dt;
        tension_snd -= dt;
        fish_flash = (fish_flash - dt).max(0.0);
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
                                        if let Some(n) = (0..N_BAITS).find(|n| baits[*n] > 0) {
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
                        let deep = terrain.at(target.x, target.z) < -5.5;
                        let (fish_i, kg) = fish::pick_fish(rod.3 + bait_luck + 0.5 * q, deep);
                        fs = FS::Reeling(Fight::new(fish_i, kg, target, q));
                        sfx.play(crate::audio::Snd::Thrash, 0.8);
                        if fish::is_monster(fish_i) {
                            toasts.push_back(("The line goes tight... something HUGE is down there!".into(), 4.0));
                        } else if kg > 8.0 {
                            toasts.push_back(("It's a big one!".into(), 3.0));
                        }
                    } else if t + dt > 1.7 {
                        fs = FS::Idle;
                        sfx.play(crate::audio::Snd::Snap, 0.8);
                        toasts.push_back(("It got away...".into(), 2.5));
                    } else {
                        fs = FS::Bite { t: t + dt, target, q };
                    }
                }
                FS::Reeling(mut f) => {
                    // reeling: pull the mouse back (or hold the button for a slow steady reel)
                    let mut pull = ((-md.y).max(0.0) / dt.max(0.001)).min(4.5);
                    if use_down || autopull {
                        pull = pull.max(if autopull { 0.9 } else { 1.1 });
                    }
                    let out = fight_step(&mut f, &FightIn { pull, reel: reel_pow, cap: line_cap }, if freeze { 0.0 } else { dt });
                    crank += pull * dt * 9.0 + dt * 1.5;
                    if pull > 0.05 {
                        reel_acc += (pull * 0.2).min(0.5) * dt * 9.0 + dt * 1.5;
                        if reel_acc >= 1.0 {
                            reel_acc = 0.0;
                            sfx.play(crate::audio::Snd::Reel, 1.0);
                        }
                    }
                    // the fish thrashes: splash, drag zip, and the line hums when it is taut
                    let thrashing = f.thrash > 0.2;
                    if thrashing && !was_thrashing {
                        sfx.play(crate::audio::Snd::Thrash, 0.6 + 0.4 * f.thrash);
                        sfx.play(crate::audio::Snd::Drag, 0.8);
                        thrash_snd = 0.4;
                    } else if thrashing && thrash_snd <= 0.0 {
                        sfx.play(crate::audio::Snd::Thrash, 0.5);
                        thrash_snd = 0.45;
                    }
                    was_thrashing = thrashing;
                    if f.tension > 0.78 && tension_snd <= 0.0 {
                        sfx.play(crate::audio::Snd::Tension, (f.tension - 0.5).min(1.0));
                        tension_snd = 0.55;
                    }
                    bend = 0.05 + f.tension * 0.09 + f.thrash * (0.03 + 0.03 * (time * 38.0).sin().abs());
                    let wy_t = water_h(&terrain, f.target.x, f.target.z, time);
                    let np = f.prog;
                    let ease = np * np * (3.0 - 2.0 * np);
                    let near = eye + fwd_flat * 1.8;
                    let along = vec3(mixf(f.target.x, near.x, ease), 0.0, mixf(f.target.z, near.z, ease));
                    // the fish darts from side to side when it thrashes
                    let rgt = vec3(-(f.target.z - eye.z), 0.0, f.target.x - eye.x).normalize_or_zero();
                    let dart = rgt * (f.dir * f.thrash * (time * 14.0).sin() * 1.2 * (1.0 - np)) + rgt * ((time * 1.3 + f.kg).sin() * 0.5 * (1.0 - np));
                    let bp = vec3(along.x + dart.x, wy_t - 0.03, along.z + dart.z);
                    lure = Some(bp);
                    bobber = Some(bp);
                    fish_on_line = Some((f.fish, f.kg, bp, f.thrash, np));
                    match out {
                        FightOut::Going => fs = FS::Reeling(f),
                        FightOut::Landed => {
                            sfx.play(crate::audio::Snd::Caught, 1.0);
                            sfx.play(crate::audio::Snd::Flop, 0.8);
                            if best.map(|b| f.kg > b.1).unwrap_or(true) && f.fish != fish::BOOT {
                                best = Some((f.fish, f.kg));
                            }
                            let who = nw.names[nw.my_id as usize].clone().filter(|_| nw.mode != NetMode::Off).unwrap_or_else(|| "You".to_string());
                            let txt = if f.fish == fish::BOOT {
                                format!("{} hooked an old boot. Gross.", who)
                            } else {
                                format!("{} caught a {} ({:.1} kg)!", who, SPECIES[f.fish].name, f.kg)
                            };
                            toasts.push_back((txt.clone(), 4.5));
                            if nw.mode != NetMode::Off {
                                nw.send_msg(&txt);
                            }
                            // the first deep-water monster you land has the boat key in its jaws
                            if fish::is_monster(f.fish) && !has_key {
                                has_key = true;
                                sfx.play(crate::audio::Snd::Key, 1.0);
                                toasts.push_back(("Something glints in its jaws... an old BOAT KEY! Your boat has an engine now.".into(), 8.0));
                            }
                            fs = FS::Holding { t: 0.0, fish: f.fish, kg: f.kg };
                            toasts.push_back((
                                if hb_free(&hotbar).is_some() {
                                    "Press a free hotbar number (1-5) to keep it, or G to let it go".to_string()
                                } else {
                                    "Your hotbar is full - make room (sell something) or press G to let it go".to_string()
                                },
                                6.0,
                            ));
                        }
                        FightOut::Snapped => {
                            fs = FS::Idle;
                            sfx.play(crate::audio::Snd::Snap, 1.0);
                            toasts.push_back(("SNAP! The line broke. Ease off when it thrashes, or get stronger line.".into(), 4.0));
                        }
                        FightOut::Escaped => {
                            fs = FS::Idle;
                            sfx.play(crate::audio::Snd::Snap, 0.7);
                            toasts.push_back(("It pulled all the line out and got away...".into(), 3.5));
                        }
                    }
                }
                FS::Holding { t, fish, kg } => {
                    if st.pressed(Act::Drop) {
                        fs = FS::Idle;
                        sfx.play(crate::audio::Snd::Splash, 0.6);
                        toasts.push_back((format!("You let the {} go", SPECIES[fish].name.to_lowercase()), 2.5));
                    } else {
                        fs = FS::Holding { t: t + dt, fish, kg };
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
                if let Some(n) = (0..N_BAITS).find(|n| baits[*n] > 0) {
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
                    fstate: match sel {
                        Some(ItemKind::Lantern) => lantern_on as u8,
                        Some(ItemKind::Fish) => fish_slots[slot].map(|f| f.0 as u8).unwrap_or(0),
                        _ => fs.code(),
                    },
                    lure: lure_net.to_array(),
                    tier: rod_tier as u8,
                    swim,
                    speed: walk_speed,
                    boat: boat.pos.to_array(),
                    byaw: boat.yaw,
                    bstate: if riding { if has_key { 3 } else { 2 } } else { 1 },
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
            let bt = vec3(r.net.boat[0], r.net.boat[1], r.net.boat[2]);
            r.bpos = if r.bpos.distance(bt) > 30.0 { bt } else { r.bpos.lerp(bt, (dt * 10.0).min(1.0)) };
            r.byaw = lerp_angle(r.byaw, r.net.byaw, dt * 10.0);
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
            let d = f.pos.distance(eye);
            if d < 40.0 && visible(f.pos, f.len * 0.6 + 0.2, eye, fwd, right) {
                let fp = FishPose { heading: f.heading, pitch: 0.0, roll: 0.0, t: time * (5.0 + 3.0 * f.speed) + f.phase, amp: 0.6, open: 0.0 };
                fish::draw_fish(&mut dynmb, f.sp, f.pos, f.len, &fp, d < 14.0);
            }
        }
        // the things that live in the deep water: huge dark shapes under the surface
        let in_deep = terrain.at(feet.x, feet.z) < -5.0;
        let mut monster_d = f32::MAX;
        for m in monsters.iter_mut() {
            let d = m.pos.distance(eye);
            if d < 400.0 {
                // curious: they slowly turn towards anyone out in the deep water
                if in_deep && d < 80.0 {
                    let want = (feet.z - m.pos.z).atan2(feet.x - m.pos.x);
                    m.heading = lerp_angle(m.heading, want, dt * 0.35);
                }
                update_swimmer(m, &terrain, dt);
            }
            monster_d = monster_d.min(d);
            if d < 170.0 && visible(m.pos, m.len * 0.6, eye, fwd, right) {
                let fp = FishPose { heading: m.heading, pitch: 0.0, roll: 0.0, t: time * 2.2 + m.phase, amp: 0.55, open: 0.4 };
                fish::draw_fish(&mut dynmb, m.sp, m.pos, m.len, &fp, d < 45.0);
            }
        }
        monster_cd -= dt;
        if monster_d < 55.0 && monster_cd <= 0.0 && (swim || riding) {
            sfx.play(crate::audio::Snd::Bloop, 1.0);
            monster_cd = 30.0;
            if monster_d < 35.0 {
                toasts.push_back(("Something enormous passes beneath you...".into(), 4.0));
            }
        }
        // boats: ours, and everybody else's
        let own_view = BoatView { pos: boat.pos, yaw: boat.yaw, pitch: boat.pitch, roll: boat.roll, row_t: boat.row_t, row_amt: boat.row_amt, motor: has_key, engine: boat.engine, manned: riding, time };
        if boat.pos.distance(eye) < 260.0 && (riding || visible(boat.pos, 3.0, eye, fwd, right)) {
            boat::draw_boat(&mut dynmb, &own_view);
        }
        if riding && boat.engine > 0.3 {
            engine_t -= dt;
            if engine_t <= 0.0 {
                engine_t = 0.15 - 0.05 * boat.engine;
                sfx.play(crate::audio::Snd::Motor, 0.5 + 0.4 * boat.engine);
            }
        }
        if riding && boat.row_amt > 0.3 && !has_key {
            let ph = (boat.row_t / TAU).floor();
            if ph != last_stroke {
                last_stroke = ph;
                sfx.play(crate::audio::Snd::Paddle, 0.9);
            }
        }
        for (i, r) in remotes.iter().enumerate() {
            let _ = i;
            if let Some(r) = r {
                if r.net.bstate >= 1 && r.bpos.distance(eye) < 260.0 {
                    let (y, p, rl) = boat::attitude(&terrain, r.bpos.x, r.bpos.z, r.byaw, time);
                    let moving_b = r.net.bstate >= 2 && r.net.speed > 0.3;
                    let v = BoatView { pos: vec3(r.bpos.x, y, r.bpos.z), yaw: r.byaw, pitch: p, roll: rl, row_t: time * 4.4, row_amt: if moving_b { 1.0 } else { 0.0 }, motor: r.net.bstate == 3, engine: if moving_b { 0.8 } else { 0.2 }, manned: r.net.bstate >= 2, time };
                    boat::draw_boat(&mut dynmb, &v);
                }
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
                    let in_boat = r.net.bstate >= 2;
                    let r_fish = if item == Some(ItemKind::Fish) {
                        let sp = (r.net.fstate as usize).min(fish::N_SPECIES - 1);
                        Some((sp, (SPECIES[sp].kg.0 + SPECIES[sp].kg.1) * 0.5))
                    } else {
                        None
                    };
                    draw_person(
                        &mut dynmb,
                        r.pos,
                        &remote_looks[i],
                        &Pose {
                            yaw: r.yaw,
                            phase: r.phase,
                            walk,
                            item,
                            tier: r.net.tier.min(3),
                            lure: lure_o,
                            lit: item == Some(ItemKind::Lantern) && r.net.fstate == 1,
                            eye,
                            time,
                            swim: r.net.swim && !in_boat,
                            surf: water_h(&terrain, r.pos.x, r.pos.z, time),
                            sit: in_boat,
                            fish: r_fish,
                        },
                    );
                    if lure_o.is_some() {
                        blob(&mut dynmb, r.lure + Vec3::Y * 0.03, Vec3::splat(0.045), vec3(0.9, 0.12, 0.10), 0.0, 8, 5, 0.0, shade);
                    }
                }
            }
        }
        // debug: see our own body a few metres ahead (swimming, sitting...)
        if has_arg("--ghost") {
            let gp = feet + fwd_flat * 3.4;
            let pose = Pose { yaw: yaw + PI, phase: time * 5.0, walk: if swim { 0.8 } else { 0.0 }, item: sel, tier: rod_tier as u8, lure: None, lit: false, eye, time, swim, surf: water_h(&terrain, gp.x, gp.z, time), sit: riding, fish: fish_slots[slot] };
            draw_person(&mut dynmb, gp, &look, &pose);
        }
        // debug: every species in a row, shrunk to fit, for checking the models
        if has_arg("--fishshow") {
            for i in 0..fish::N_SPECIES {
                let (row, col) = (i / 5, i % 5);
                let len = fish::fish_len(i, (SPECIES[i].kg.0 + SPECIES[i].kg.1) * 0.5).min(1.15);
                let p = eye + fwd_flat * (3.2 + row as f32 * 1.6) + right * ((col as f32 - 2.0) * 1.15) + Vec3::Y * (-0.2 + row as f32 * 0.35);
                let fp = FishPose { heading: yaw + PI * 0.5, pitch: 0.0, roll: 0.0, t: time * 4.0, amp: 0.6, open: 0.5 };
                fish::draw_fish(&mut dynmb, i, p, len, &fp, true);
            }
        }
        // what's in our hands (not while swimming)
        match if swim { None } else { sel } {
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
                if let FS::Holding { t, fish: fi, kg } = fs {
                    // the catch, flapping in front of you until you keep it or let it go
                    let len = fish::fish_len(fi, kg).min(1.2);
                    let pos = eye + fwd * (0.55 + len * 0.45) + camup * (0.02 + (t * 2.0).sin() * 0.012);
                    let fp = FishPose { heading: yaw + 1.25, pitch: 0.0, roll: (t * 7.0).sin() * 0.25 * (1.0 - smooth(0.0, 3.0, t)), t: t * 15.0, amp: (1.1 - t * 0.3).max(0.25), open: 0.6 };
                    fish::draw_fish(&mut dynmb, fi, pos, len, &fp, true);
                }
            }
            Some(ItemKind::Fish) => {
                if let Some((fi, kg)) = fish_slots[slot] {
                    let len = fish::fish_len(fi, kg).min(0.95);
                    let hp = eye + fwd * 0.75 + right * 0.26 + camup * 0.02;
                    let sway = (time * 1.4).sin() * 0.04 + if moving { (bob_t * 0.5).sin() * 0.03 } else { 0.0 };
                    let fp = FishPose { heading: yaw + PI * 0.5 + sway, pitch: 1.2, roll: 0.0, t: time * 2.2, amp: 0.25, open: 0.3 };
                    fish::draw_fish(&mut dynmb, fi, hp - camup * (len * 0.42), len, &fp, true);
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
            _ => {}
        }
        // the fish on the end of the line (thrashing, and lifted out of the water at the end)
        if let Some((fi, kg, pos, thr, prog)) = fish_on_line {
            let len_full = fish::fish_len(fi, kg);
            let len = len_full.min(mixf(len_full, 1.8, smooth(0.55, 1.0, prog)));
            let lift = smooth(0.82, 1.0, prog) * (0.25 + 0.2 * len);
            let heading = (pos.z - eye.z).atan2(pos.x - eye.x);
            let c = pos + Vec3::Y * (lift + 0.10 + 0.25 * thr - len * 0.08);
            let fp = FishPose { heading, pitch: thr * (time * 17.0).sin() * 0.45, roll: thr * (time * 21.0).sin() * 1.0, t: time * (7.0 + 11.0 * thr), amp: 0.6 + 1.1 * thr, open: 0.5 + 0.5 * thr };
            let d = c.distance(eye);
            if d < 120.0 {
                fish::draw_fish(&mut dynmb, fi, c, len, &fp, d < 25.0);
            }
            if thr > 0.25 {
                for k in 0..7 {
                    let a = k as f32 * 0.9 + time * 7.0;
                    let j = vec3(a.cos() * (0.25 + 0.5 * len.min(1.5)), 0.05 + (a * 2.3).sin().abs() * 0.35, a.sin() * (0.25 + 0.5 * len.min(1.5)));
                    blob(&mut dynmb, pos + j, Vec3::splat(0.03 + 0.02 * ((a * 3.0).sin() * 0.5 + 0.5)), vec3(0.92, 0.97, 1.0), 0.0, 5, 3, 0.0, shade);
                }
            }
        }
        // first-person swim strokes
        if swim && !riding && screen == Screen::None {
            swim_t += dt * (2.2 + 4.0 * (walk_speed / 2.4).min(1.5));
            fp_swim_arms(&mut dynmb, &look, eye, fwd, right, camup, swim_t, (walk_speed / 2.4).min(1.0));
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
                        let label = nw.names[i].clone().unwrap_or_else(|| format!("Player {}", i + 1));
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
            let money = if nw.mode == NetMode::Off { format!("{} coins", coins) } else { format!("{} coins (shared)", coins) };
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
                    NetMode::Client => (format!("ONLINE - you are {}, {} players in this world", nw.names[nw.my_id as usize].clone().unwrap_or_else(|| format!("Player {}", nw.my_id + 1)), players), Color::new(0.5, 1.0, 0.6, 1.0)),
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
                let nfish = hotbar.iter().filter(|s| **s == Some(ItemKind::Fish)).count();
                shadow_text(&format!("Coconuts: {}   Fish: {}   Spiders nearby: {}", held, nfish, sp), 14.0, sh - 90.0 * hs, 22.0 * hs, WHITE);
            }
            // prompts
            if screen == Screen::None && sleep.is_none() {
                let ikey = bind_label(st.bind(Act::Interact));
                let prompt = if riding {
                    if fighting { None } else { Some(format!("[{}] Leave the boat", ikey)) }
                } else if boat_target {
                    Some(format!("[{}] Board the boat", ikey))
                } else if npc_target {
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
            // the fight: line tension, how far the fish is, and what it is doing
            if let FS::Reeling(f) = fs {
                let (bw, bh) = (420.0 * hs, 20.0 * hs);
                let bx = cx - bw * 0.5;
                let ty = sh - 196.0 * hs;
                draw_rectangle(bx - 4.0, ty - 4.0, bw + 8.0, bh + 8.0, Color::new(0.0, 0.0, 0.0, 0.6));
                draw_rectangle(bx, ty, bw * 0.35, bh, Color::new(0.25, 0.35, 0.50, 0.85));
                draw_rectangle(bx + bw * 0.35, ty, bw * 0.45, bh, Color::new(0.20, 0.62, 0.35, 0.85));
                draw_rectangle(bx + bw * 0.80, ty, bw * 0.20, bh, Color::new(0.78, 0.20, 0.18, 0.9));
                let mx = bx + bw * f.tension.min(1.0);
                draw_rectangle(mx - 3.0, ty - 6.0 * hs, 6.0, bh + 12.0 * hs, WHITE);
                shadow_text("LINE TENSION", bx, ty - 10.0 * hs, 17.0 * hs, Color::new(0.85, 0.9, 0.95, 1.0));
                if f.overload > 0.04 {
                    let fl = if (time * 10.0) as i32 % 2 == 0 { 1.0 } else { 0.5 };
                    shadow_text("SNAPPING!", bx + bw - 100.0 * hs, ty - 10.0 * hs, 20.0 * hs, Color::new(1.0, 0.3 * fl, 0.25 * fl, 1.0));
                }
                // how much line is in (far -> landed)
                let by = sh - 150.0 * hs;
                draw_rectangle(bx - 4.0, by - 4.0, bw + 8.0, bh + 8.0, Color::new(0.0, 0.0, 0.0, 0.6));
                let col = if f.prog < 0.2 { Color::new(0.9, 0.25, 0.2, 1.0) } else { Color::new(0.3, 0.85, 0.5, 1.0) };
                draw_rectangle(bx, by, bw * f.prog, bh, col);
                shadow_text("FISH", bx, by - 10.0 * hs, 17.0 * hs, Color::new(0.85, 0.9, 0.95, 1.0));
                shadow_text("LANDED", bx + bw - 62.0 * hs, by - 10.0 * hs, 17.0 * hs, Color::new(0.85, 0.9, 0.95, 1.0));
                // its energy: a tired fish thrashes less and pulls less
                let eh = 8.0 * hs;
                draw_rectangle(bx, by + bh + 8.0 * hs, bw, eh, Color::new(0.0, 0.0, 0.0, 0.5));
                draw_rectangle(bx, by + bh + 8.0 * hs, bw * f.stamina, eh, Color::new(0.95, 0.7, 0.2, 0.95));
                shadow_text("energy", bx + bw + 8.0 * hs, by + bh + 16.0 * hs, 15.0 * hs, Color::new(0.95, 0.8, 0.45, 1.0));
                let size = if f.kg < 1.0 { "small" } else if f.kg < 6.0 { "decent" } else if f.kg < 25.0 { "heavy" } else if f.kg < 100.0 { "huge" } else { "a MONSTER" };
                let msg = if f.thrash > 0.2 {
                    "THRASHING! Ease off the reel!".to_string()
                } else {
                    format!("Pull the mouse back (or hold click) to reel it in - it feels {}", size)
                };
                let col = if f.thrash > 0.2 { Color::new(1.0, 0.35, 0.3, 1.0) } else { WHITE };
                let tw = measure_text(&msg, None, (26.0 * hs) as u16, 1.0).width;
                shadow_text(&msg, cx - tw * 0.5, ty - 38.0 * hs, 26.0 * hs, col);
            } else if let FS::Bite { .. } = fs {
                shadow_text("FISH ON! Click or pull the mouse back!", cx - 190.0 * hs, sh - 160.0 * hs, 28.0 * hs, Color::new(1.0, 0.9, 0.4, 1.0));
            } else if let FS::Holding { fish: fi, kg, .. } = fs {
                let line1 = if fi == fish::BOOT { format!("An old boot (worth {} coins)", fish::fish_value(fi, kg)) } else { format!("{} - {:.1} kg (worth {} coins)", SPECIES[fi].name, kg, fish::fish_value(fi, kg)) };
                let line2 = if hb_free(&hotbar).is_some() { "Press a free hotbar number to keep it   -   G lets it go".to_string() } else { "Hotbar full! Sell something at the shop, or press G to let it go".to_string() };
                for (k, (s, sz, c)) in [(line1, 30.0, Color::new(1.0, 0.92, 0.55, 1.0)), (line2, 22.0, WHITE)].iter().enumerate() {
                    let tw = measure_text(s, None, (*sz * hs) as u16, 1.0).width;
                    shadow_text(s, cx - tw * 0.5, sh - 215.0 * hs + k as f32 * 30.0 * hs, *sz * hs, *c);
                }
                // pulse the free slots
                let (ss, gap) = (64.0 * hs, 9.0 * hs);
                let total = 5.0 * ss + 4.0 * gap;
                let x0 = ((sw - total) * 0.5).round();
                let ty = sh - 3.0 * hs - 20.0 * hs - 4.0 * hs - ss;
                let pulse = 0.5 + 0.5 * (time * 6.0).sin();
                for i in 0..5 {
                    if hotbar[i].is_none() {
                        let x = (x0 + i as f32 * (ss + gap)).round();
                        draw_rectangle_lines(x - 2.0 * hs, ty - 2.0 * hs, ss + 4.0 * hs, ss + 4.0 * hs, 3.0 * hs, Color::new(1.0, 0.9, 0.35, 0.35 + 0.6 * pulse));
                    }
                }
            }

            // hotbar
            {
                let mut entries: [Option<HotbarEntry>; 5] = [None; 5];
                for i in 0..5 {
                    if let Some(k) = hotbar[i] {
                        let count = match k {
                            ItemKind::Coconut => Some(held),
                            ItemKind::Bait => Some(baits[bait_sel]),
                            _ => None,
                        };
                        entries[i] = Some(HotbarEntry { kind: k, count, tier: rod_tier, lit: lantern_on, fish: fish_slots[i].map(|f| f.0).unwrap_or(0) });
                    }
                }
                draw_hotbar(&entries, slot, &slot_keys, hs);
                let y0 = sh - 62.0 * hs - 6.0 * hs;
                if slot_label_t > 0.0 {
                    if let Some(k) = sel {
                        let label = item_name(k, rod_tier, bait_sel, fish_slots[slot]);
                        let tw = measure_text(&label, None, (24.0 * hs) as u16, 1.0).width;
                        shadow_text(&label, cx - tw * 0.5, y0 - 18.0 * hs, 24.0 * hs, WHITE);
                    }
                }
            }        }

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
                if ui.button(205.0, 536.0, 190.0, 44.0, "Main menu", true, false) {
                    leave = Some(true);
                }
                if ui.button(205.0, 592.0, 190.0, 44.0, "Quit to desktop", true, false) {
                    leave = Some(false);
                }
                ui.text("Unsaved progress is lost", 205.0, 644.0, 15.0, Color::new(1.0, 1.0, 1.0, 0.4));
                let (x, w) = (430.0f32, 640.0f32);
                let dim = Color::new(0.65, 0.7, 0.75, 1.0);
                match tab {
                    0 => tab_graphics(&mut ui, &mut st, x, w),
                    1 => tab_display(&mut ui, &mut st, x, w),
                    2 => tab_controls(&mut ui, &mut st, &mut rebinding, x),
                    4 => {
                        let before = look.clone();
                        let was_focus = name_focus;
                        tab_character(&mut ui, &mut look, &mut st.name, &mut name_focus, x, w, time, &world_mat);
                        if was_focus && !name_focus {
                            st.name = crate::net::clean_name(&st.name);
                            st.save();
                            nw.set_name(&st.name);
                        }
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
                                        Ok(()) => {
                                            nw.send_look(&st.look);
                                            nw.set_balance(coins);
                                        }
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
                let ctext = if nw.mode == NetMode::Off { format!("Coins: {}", coins) } else { format!("Coins: {} (shared)", coins) };
                let cw = ui.text_w(&ctext, 28.0);
                ui.text(&ctext, px + pw - 24.0 - cw, py + 18.0, 28.0, Color::new(0.5, 0.95, 0.5, 1.0));
                ui.text(&format!("\"{}\"", shop_msg), px + 24.0, py + 62.0, 20.0, Color::new(0.85, 0.85, 0.9, 1.0));
                // tabs
                for (i, name) in ["Sell fish", "Rods", "Bait", "Gear"].iter().enumerate() {
                    if ui.button(px + 24.0 + i as f32 * 172.0, py + 96.0, 164.0, 40.0, name, shop_tab != i, false) {
                        shop_tab = i;
                    }
                }
                let (lx, cw2) = (px + 24.0, pw - 48.0);
                let top = py + 156.0;
                let have_rod = owned_rods.iter().any(|o| *o);
                match shop_tab {
                    0 => {
                        // sell the fish you carry in your hotbar
                        let mut row = 0;
                        let mut total = 0;
                        let mut sold: Option<usize> = None;
                        for i in 0..5 {
                            if let (Some(ItemKind::Fish), Some((f, kg))) = (hotbar[i], fish_slots[i]) {
                                let v = fish::fish_value(f, kg);
                                total += v;
                                let y = top + row as f32 * 62.0;
                                let name = if f == fish::BOOT { SPECIES[f].name.to_string() } else { format!("{}  ({:.1} kg)", SPECIES[f].name, kg) };
                                if shop_row(&mut ui, lx, y, cw2, &format!("Slot {}: {}", i + 1, name), &format!("Worth {} coins", v), "Sell", true, true) {
                                    sold = Some(i);
                                }
                                row += 1;
                            }
                        }
                        if let Some(i) = sold {
                            if let Some((f, kg)) = fish_slots[i] {
                                let v = fish::fish_value(f, kg);
                                add_coins(&mut coins, &mut nw, v);
                                sfx.play(crate::audio::Snd::Coins, 1.0);
                                shop_msg = if fish::is_monster(f) { format!("By the tides... a {}! {} coins, and gladly.", SPECIES[f].name, v) } else { format!("Fine catch! Here's {} coins.", v) };
                            }
                            hotbar[i] = None;
                            fish_slots[i] = None;
                        }
                        if row == 0 {
                            ui.text("You aren't carrying any fish.", lx, top, 24.0, Color::new(0.8, 0.85, 0.9, 1.0));
                            ui.text("Catch one, press a free hotbar number to keep it, then bring it here.", lx, top + 34.0, 19.0, Color::new(0.65, 0.7, 0.75, 1.0));
                        }
                        if let Some((f, kg)) = best {
                            ui.text(&format!("Your best catch: {} ({:.1} kg)", SPECIES[f].name, kg), lx, py + ph - 108.0, 20.0, Color::new(1.0, 0.9, 0.5, 1.0));
                        }
                        if ui.button(px + pw - 24.0 - 330.0, py + ph - 62.0, 330.0, 42.0, &format!("Sell all fish ({} coins)", total), row > 0, true) {
                            for i in 0..5 {
                                if hotbar[i] == Some(ItemKind::Fish) {
                                    hotbar[i] = None;
                                    fish_slots[i] = None;
                                }
                            }
                            add_coins(&mut coins, &mut nw, total);
                            sfx.play(crate::audio::Snd::Coins, 1.0);
                            shop_msg = format!("A fine haul. Here's {} coins.", total);
                        }
                    }
                    1 => {
                        for i in 0..N_RODS {
                            let y = top + i as f32 * 74.0;
                            let r = RODS[i];
                            let desc = if i == 0 { "A simple, reliable rod".to_string() } else { format!("Cast +{:.0}%  Reel +{:.0}%  Line strength x{:.1}  Better fish", (r.1 - 1.0) * 100.0, (r.2 - 1.0) * 100.0, r.8) };
                            let price = if i == 0 { STARTER_PRICE } else { r.5 };
                            if owned_rods[i] {
                                let equipped = rod_tier == i;
                                if shop_row(&mut ui, lx, y, cw2, r.0, &desc, if equipped { "Equipped" } else { "Equip" }, !equipped && hb_can_give(&hotbar, ItemKind::Rod), false) {
                                    rod_tier = i;
                                    hb_give(&mut hotbar, ItemKind::Rod);
                                    shop_msg = format!("The {} suits you!", r.0);
                                }
                            } else if i == 0 && have_rod {
                                shop_row(&mut ui, lx, y, cw2, r.0, "Only for first-time anglers", "Not needed", false, false);
                            } else if shop_row(&mut ui, lx, y, cw2, r.0, &desc, &format!("{} coins", price), coins >= price && hb_can_give(&hotbar, ItemKind::Rod), true) {
                                add_coins(&mut coins, &mut nw, -price);
                                sfx.play(crate::audio::Snd::Coin, 1.0);
                                owned_rods[i] = true;
                                rod_tier = i;
                                hb_give(&mut hotbar, ItemKind::Rod);
                                shop_msg = format!("Enjoy your new {}!", r.0);
                            }
                        }
                        if !hb_can_give(&hotbar, ItemKind::Rod) {
                            ui.text("Your hotbar is full - free a slot for the rod.", lx, py + ph - 100.0, 19.0, Color::new(1.0, 0.7, 0.5, 1.0));
                        }
                    }
                    2 => {
                        ui.text("One is used up per cast. Better bait means better and bigger fish.", lx, top - 6.0, 19.0, Color::new(0.7, 0.75, 0.8, 1.0));
                        for i in 0..N_BAITS {
                            let y = top + 22.0 + i as f32 * 62.0;
                            let b = BAITS[i];
                            let extra = if i == N_BAITS - 1 { "   Draws the deep monsters" } else { "" };
                            let desc = format!("Pack of {}   Luck +{:.0}%   Faster bites{}", b.4, b.1 * 100.0, extra);
                            if shop_row(&mut ui, lx, y, cw2, &format!("{}  (you have {})", b.0, baits[i]), &desc, &format!("{} coins", b.3), coins >= b.3 && hb_can_give(&hotbar, ItemKind::Bait), true) {
                                add_coins(&mut coins, &mut nw, -b.3);
                                sfx.play(crate::audio::Snd::Coin, 1.0);
                                baits[i] += b.4;
                                bait_sel = i;
                                hb_give(&mut hotbar, ItemKind::Bait);
                                shop_msg = format!("{} {} - good choice.", b.4, b.0);
                            }
                        }
                    }
                    _ => {
                        let has_lantern = hb_has(&hotbar, ItemKind::Lantern);
                        if has_lantern {
                            shop_row(&mut ui, lx, top, cw2, "Lantern", "Lights your way after dark", "Owned", false, false);
                        } else if shop_row(&mut ui, lx, top, cw2, "Lantern", "Lights your way after dark", &format!("{} coins", LANTERN_PRICE), coins >= LANTERN_PRICE && hb_can_give(&hotbar, ItemKind::Lantern), true) {
                            add_coins(&mut coins, &mut nw, -LANTERN_PRICE);
                            sfx.play(crate::audio::Snd::Coin, 1.0);
                            hb_give(&mut hotbar, ItemKind::Lantern);
                            shop_msg = "Click with it in hand to light it. Stay safe out there.".into();
                        }
                        for i in 0..3 {
                            let y = top + (i as f32 + 1.0) * 66.0;
                            let (name, desc, price) = UPGRADES[i];
                            if upg[i] {
                                shop_row(&mut ui, lx, y, cw2, name, desc, "Owned", false, false);
                            } else if shop_row(&mut ui, lx, y, cw2, name, desc, &format!("{} coins", price), coins >= price, true) {
                                add_coins(&mut coins, &mut nw, -price);
                                sfx.play(crate::audio::Snd::Coin, 1.0);
                                upg[i] = true;
                                shop_msg = format!("{} - that'll hold a bigger fish.", name);
                            }
                        }
                        let y = top + 4.0 * 66.0 + 8.0;
                        ui.text("Rowing boat", lx, y, 22.0, WHITE);
                        let boat_note = if has_key { "You found the boat key: the outboard motor works (hold Shift for full throttle)." } else { "Moored by the dock. Rowing is free. Land a deep-water monster to find the boat key." };
                        ui.text(boat_note, lx, y + 26.0, 17.0, Color::new(0.7, 0.75, 0.8, 1.0));
                    }
                }
                if ui.button(lx, py + ph - 62.0, 160.0, 42.0, "Close", true, false) {
                    screen = Screen::None;
                    st.save();
                }
            }            Screen::Tent => {
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
                    let data = SaveData { coins, rod_tier, owned_rods, hotbar, fish_slots, baits, bait_sel, coconuts: held, lantern_on, best, sky_t, upg, has_key };
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

        // leave the island: back to the main menu, or all the way out
        if let Some(to_menu) = leave {
            st.save();
            release_cursor_clip();
            set_cursor_grab(false);
            show_mouse(true);
            return to_menu;
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
