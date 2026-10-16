//! Progression that isn't tied to one screen: fisher level and XP, the fish journal, boat upgrades,
//! fuel, flares and the diary pages found. Saved with the game.

use crate::fish::N_SPECIES;

pub const N_BOAT_UPG: usize = 6;
/// Boat parts (all sold by the traders on the other islands): (name, description, price)
pub const BOAT_UPGRADES: [(&str, &str, i32); N_BOAT_UPG] = [
    ("Outboard Engine", "A real engine for the boat. Needs the boat key, and it burns gasoline.", 3000),
    ("Reinforced Hull", "Takes half the damage from rams and bites.", 2500),
    ("Steel Plating", "A hull that almost nothing can break (needs the Reinforced Hull).", 5000),
    ("Searchlight", "A strong lamp on the bow. Hunters shy away from its beam. Press F on the boat.", 1200),
    ("Big Fuel Tank", "Carries 50% more gasoline.", 2500),
    ("Turbo Engine", "A much faster outboard (needs the Outboard Engine).", 5000),
];
pub const ENGINE: usize = 0;
pub const REINFORCED: usize = 1;
pub const STEEL: usize = 2;
pub const SEARCHLIGHT: usize = 3;
pub const BIG_TANK: usize = 4;
pub const TURBO: usize = 5;

/// One can of gasoline fills the tank.
pub const GAS_PRICE: i32 = 2000;
pub const FLARE_PACK: (u32, i32) = (5, 300);
pub const FLARE_GUN_PRICE: i32 = 800;
pub const REPAIR_PRICE: i32 = 150;
pub const TANK: f32 = 100.0;

/// How many diary pages there are to find.
pub const N_PAGES: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub boat: [bool; N_BOAT_UPG],
    /// gasoline left (0..tank_size)
    pub fuel: f32,
    pub flares: u32,
    /// hull integrity 0..100
    pub hp: f32,
    /// bit i set = diary page i found
    pub pages: u32,
    pub xp: u32,
    /// per species: (times caught, heaviest in kg)
    pub journal: Vec<(u32, f32)>,
}

impl Default for Progress {
    fn default() -> Self {
        Progress { boat: [false; N_BOAT_UPG], fuel: 0.0, flares: 0, hp: 100.0, pages: 0, xp: 0, journal: vec![(0, 0.0); N_SPECIES] }
    }
}

impl Progress {
    pub fn tank_size(&self) -> f32 {
        if self.boat[BIG_TANK] {
            TANK * 1.5
        } else {
            TANK
        }
    }

    /// How much damage gets through the hull (1 = all of it).
    pub fn hull_factor(&self) -> f32 {
        if self.boat[STEEL] {
            0.15
        } else if self.boat[REINFORCED] {
            0.5
        } else {
            1.0
        }
    }

    pub fn level(&self) -> u32 {
        level_of(self.xp)
    }

    pub fn pages_found(&self) -> u32 {
        self.pages.count_ones()
    }

    pub fn has_page(&self, i: usize) -> bool {
        self.pages & (1 << i) != 0
    }

    /// Is this upgrade available to buy yet?
    pub fn can_buy(&self, i: usize, has_key: bool) -> Result<(), &'static str> {
        if self.boat[i] {
            return Err("Owned");
        }
        match i {
            ENGINE if !has_key => Err("Needs the boat key"),
            STEEL if !self.boat[REINFORCED] => Err("Needs the Reinforced Hull"),
            TURBO if !self.boat[ENGINE] => Err("Needs the Outboard Engine"),
            _ => Ok(()),
        }
    }

    /// Record a catch. Returns (xp gained, first time this species was caught).
    pub fn record_catch(&mut self, species: usize, kg: f32, value: i32, monster: bool) -> (u32, bool) {
        let e = &mut self.journal[species.min(N_SPECIES - 1)];
        let first = e.0 == 0;
        e.0 += 1;
        if kg > e.1 {
            e.1 = kg;
        }
        let gain = (value.max(1) as u32 / 4).max(2) + if monster { 150 } else { 0 } + if first { 50 } else { 0 };
        self.xp += gain;
        (gain, first)
    }

    pub fn species_caught(&self) -> usize {
        self.journal.iter().filter(|e| e.0 > 0).count()
    }

    pub fn write(&self) -> String {
        let b = |x: bool| if x { "1" } else { "0" };
        let up: Vec<&str> = self.boat.iter().map(|x| b(*x)).collect();
        let j: Vec<String> = self.journal.iter().map(|(n, kg)| format!("{}:{:.2}", n, kg)).collect();
        format!("boat={}\nfuel={:.1}\nflares={}\nhp={:.1}\npages={}\nxp={}\njournal={}\n", up.join(","), self.fuel, self.flares, self.hp, self.pages, self.xp, j.join(","))
    }

    /// Reads one `key=value` line; returns true when the key was ours.
    pub fn read(&mut self, k: &str, v: &str) -> bool {
        match k {
            "boat" => {
                for (i, p) in v.split(',').take(N_BOAT_UPG).enumerate() {
                    self.boat[i] = p.trim() == "1";
                }
            }
            "fuel" => self.fuel = v.parse::<f32>().ok().filter(|x| x.is_finite()).unwrap_or(0.0).clamp(0.0, 1000.0),
            "flares" => self.flares = v.parse::<u32>().unwrap_or(0).min(999),
            "hp" => self.hp = v.parse::<f32>().ok().filter(|x| x.is_finite()).unwrap_or(100.0).clamp(0.0, 100.0),
            "pages" => self.pages = v.parse::<u32>().unwrap_or(0) & ((1 << N_PAGES) - 1),
            "xp" => self.xp = v.parse::<u32>().unwrap_or(0).min(10_000_000),
            "journal" => {
                for (i, p) in v.split(',').take(N_SPECIES).enumerate() {
                    if let Some((n, kg)) = p.split_once(':') {
                        self.journal[i] = (n.trim().parse().unwrap_or(0), kg.trim().parse::<f32>().ok().filter(|x| x.is_finite()).unwrap_or(0.0));
                    }
                }
            }
            _ => return false,
        }
        true
    }
}

/// Level for an amount of XP: 1 at the start, 2 at 100, 3 at 400, 4 at 900...
pub fn level_of(xp: u32) -> u32 {
    ((xp as f32 / 100.0).sqrt() as u32 + 1).min(50)
}

/// XP needed to reach `level`.
pub fn xp_for(level: u32) -> u32 {
    let l = level.max(1) - 1;
    100 * l * l
}

/// 0..1 progress through the current level.
pub fn level_progress(xp: u32) -> f32 {
    let l = level_of(xp);
    let (a, b) = (xp_for(l), xp_for(l + 1));
    ((xp - a.min(xp)) as f32 / (b - a).max(1) as f32).clamp(0.0, 1.0)
}

/// Small permanent bonuses per level: (luck, reel power multiplier).
pub fn level_perks(level: u32) -> (f32, f32) {
    let l = (level.max(1) - 1) as f32;
    (0.02 * l, 1.0 + 0.015 * l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_grow_and_progress_is_sane() {
        assert_eq!(level_of(0), 1);
        assert_eq!(level_of(99), 1);
        assert_eq!(level_of(100), 2);
        assert_eq!(level_of(399), 2);
        assert_eq!(level_of(400), 3);
        for lv in 1..30 {
            assert_eq!(level_of(xp_for(lv)), lv);
            assert!(xp_for(lv + 1) > xp_for(lv));
        }
        assert!((level_progress(250) - 0.5).abs() < 0.01);
        let (luck, reel) = level_perks(11);
        assert!((luck - 0.2).abs() < 1e-5 && reel > 1.1);
        assert_eq!(level_perks(1), (0.0, 1.0));
    }

    #[test]
    fn catches_fill_the_journal_and_pay_xp() {
        let mut p = Progress::default();
        let (g1, first) = p.record_catch(2, 2.0, 32, false);
        assert!(first && g1 >= 50);
        let (g2, first2) = p.record_catch(2, 3.5, 56, false);
        assert!(!first2 && g2 < g1);
        assert_eq!(p.journal[2], (2, 3.5));
        assert_eq!(p.species_caught(), 1);
        let (g3, _) = p.record_catch(12, 400.0, 13_600, true);
        assert!(g3 > 3000, "a monster is worth a lot of xp");
        assert!(p.level() >= 7);
    }

    #[test]
    fn boat_parts_have_prerequisites_and_armour_stacks() {
        let mut p = Progress::default();
        assert_eq!(p.can_buy(ENGINE, false), Err("Needs the boat key"));
        assert_eq!(p.can_buy(ENGINE, true), Ok(()));
        assert!(p.can_buy(STEEL, true).is_err() && p.can_buy(TURBO, true).is_err());
        p.boat[REINFORCED] = true;
        p.boat[ENGINE] = true;
        assert_eq!(p.can_buy(STEEL, true), Ok(()));
        assert_eq!(p.can_buy(TURBO, true), Ok(()));
        assert_eq!(p.can_buy(ENGINE, true), Err("Owned"));
        assert_eq!(p.hull_factor(), 0.5);
        p.boat[STEEL] = true;
        assert!(p.hull_factor() < 0.2);
        assert_eq!(p.tank_size(), TANK);
        p.boat[BIG_TANK] = true;
        assert_eq!(p.tank_size(), TANK * 1.5);
        // prices of the big upgrades reach 5000 and gas costs about 2000
        assert!(BOAT_UPGRADES.iter().any(|u| u.2 >= 5000));
        assert_eq!(GAS_PRICE, 2000);
    }

    #[test]
    fn saves_round_trip_and_survive_garbage() {
        let mut p = Progress::default();
        p.boat[ENGINE] = true;
        p.boat[SEARCHLIGHT] = true;
        p.fuel = 42.5;
        p.flares = 7;
        p.hp = 63.0;
        p.pages = 0b1010_0110;
        p.xp = 1234;
        p.record_catch(4, 18.0, 432, false);
        let text = p.write();
        let mut q = Progress::default();
        for line in text.lines() {
            let (k, v) = line.split_once('=').unwrap();
            assert!(q.read(k, v), "{k}");
        }
        assert_eq!(p.boat, q.boat);
        assert_eq!(p.pages, q.pages);
        assert_eq!(p.journal[4], q.journal[4]);
        assert!((q.fuel - 42.5).abs() < 0.1 && q.flares == 7 && (q.hp - 63.0).abs() < 0.1);
        let mut bad = Progress::default();
        bad.read("fuel", "nan");
        bad.read("hp", "9999");
        bad.read("pages", "4294967295");
        bad.read("journal", "x:y,3:inf,1:2");
        assert!(bad.hp <= 100.0 && bad.pages < (1 << N_PAGES));
        assert!(!bad.read("coins", "5"));
    }
}
