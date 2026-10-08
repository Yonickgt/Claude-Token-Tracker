//! Study buddies: the same 15 species, personalities and motion state machine as StudyList
//! (shared/buddy.ts + lib/buddyMotion.ts), drawn from the same 6x6 sprite atlases.
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};

pub const SPECIES: [&str; 15] = [
    "emberfox", "mossling", "sootpuff", "pondhopper", "cloudhare", "clawd", "sashpanda", "cerberus",
    "kitsune", "axolotl", "bao", "ghost", "jelly", "inkpip", "capling",
];
pub const MAX_BUDDIES: usize = 5;
pub const FRAMES: usize = 6;
pub const CELL: usize = 32;
// state rows: idle, walk, run, sit, sleep, special
const IDLE: usize = 0;
const WALK: usize = 1;
const RUN: usize = 2;
const SPECIAL: usize = 5;

pub struct Profile { pub walk: f32, pub run: f32, pub weights: [f32; 6], pub min_ms: f32, pub max_ms: f32, pub turn: f32, pub frame_ms: [f32; 6] }

pub fn profile(species: &str) -> Profile {
    let (walk, run, weights, min_ms, max_ms, turn, frame_ms) = match species {
        "emberfox" => (26., 62., [2., 4., 3., 1., 0., 2.], 1400., 3600., 0.35, [190., 110., 70., 240., 420., 130.]),
        "mossling" => (14., 30., [4., 3., 0.5, 3., 1., 2.], 2200., 5200., 0.5, [260., 170., 100., 300., 520., 180.]),
        "sootpuff" => (34., 84., [2., 2., 5., 1., 0.3, 2.], 700., 2000., 0.6, [150., 90., 55., 200., 400., 90.]),
        "pondhopper" => (11., 24., [3., 2., 0.3, 4., 4., 2.], 2600., 6200., 0.25, [300., 200., 120., 340., 600., 220.]),
        "cloudhare" => (19., 40., [4., 3., 1., 2., 1., 2.], 2000., 4800., 0.3, [240., 140., 85., 280., 520., 200.]),
        "clawd" => (15., 34., [4., 3., 1., 3., 1., 3.], 2000., 5200., 0.3, [260., 160., 95., 300., 540., 190.]),
        "sashpanda" => (15., 34., [4., 3., 1., 3., 1., 3.], 2000., 5200., 0.3, [260., 160., 95., 300., 540., 190.]),
        "cerberus" => (17., 44., [4.5, 3., 1.5, 2.5, 1., 2.], 2000., 5000., 0.35, [250., 135., 82., 280., 520., 200.]),
        "kitsune" => (17., 44., [4.5, 3., 1.5, 2.5, 1., 2.], 2000., 5000., 0.35, [250., 135., 82., 280., 520., 200.]),
        "axolotl" => (10., 24., [4., 2.5, 0.5, 3., 3., 1.5], 2400., 6000., 0.2, [300., 190., 110., 330., 600., 230.]),
        "bao" => (9., 20., [4.5, 2., 0.4, 3.5, 3., 1.5], 2400., 6200., 0.18, [310., 200., 120., 340., 620., 240.]),
        "ghost" => (13., 30., [4., 3.5, 1., 1.5, 1.5, 2.], 2200., 5600., 0.3, [280., 165., 100., 300., 560., 210.]),
        "jelly" => (11., 26., [4.5, 3., 0.6, 3., 2., 1.5], 2400., 6000., 0.22, [290., 185., 115., 320., 580., 225.]),
        "inkpip" => (20., 58., [3., 3.5, 3., 1., 0.6, 1.2], 900., 2600., 0.55, [190., 95., 58., 210., 400., 140.]),
        _ => (11., 26., [4.5, 3., 0.6, 3., 2., 1.5], 2400., 6000., 0.22, [290., 185., 115., 320., 580., 225.]), // capling
    };
    Profile { walk, run, weights, min_ms, max_ms, turn, frame_ms }
}

pub fn label(species: &str) -> String {
    match species {
        "emberfox" => "Ember Fox".into(), "sootpuff" => "Soot Puff".into(), "pondhopper" => "Pond Hopper".into(),
        "cloudhare" => "Cloud Hare".into(), "sashpanda" => "Sash Panda".into(),
        s => { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }
    }
}

macro_rules! atlases {
    ($($n:literal),*) => {
        fn atlas_bytes(s: &str) -> &'static [u8] {
            match s { $($n => include_bytes!(concat!("../assets/buddies/", $n, ".png")),)* _ => &[] }
        }
    };
}
atlases!("emberfox", "mossling", "sootpuff", "pondhopper", "cloudhare", "clawd", "sashpanda", "cerberus", "kitsune", "axolotl", "bao", "ghost", "jelly", "inkpip", "capling");

pub fn load_atlas(ctx: &egui::Context, species: &str) -> Option<TextureHandle> {
    let img = image::load_from_memory(atlas_bytes(species)).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    // nearest = crisp pixel art, like image-rendering: pixelated
    Some(ctx.load_texture(format!("buddy-{species}"), ColorImage::from_rgba_unmultiplied(size, img.as_raw()), TextureOptions::NEAREST))
}

/// mulberry32 seeded from the buddy id, so each buddy has its own reproducible temperament.
struct Rng(u32);
impl Rng {
    fn new(seed: &str) -> Self {
        let mut s: u32 = 0x811c9dc5;
        for c in seed.chars() { s = (s ^ c as u32).wrapping_mul(0x01000193) }
        Rng(s)
    }
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut v = (self.0 ^ (self.0 >> 15)).wrapping_mul(1 | self.0);
        v = v.wrapping_add((v ^ (v >> 7)).wrapping_mul(61 | v)) ^ v;
        ((v ^ (v >> 14)) as f64 / 4294967296.0) as f32
    }
}

pub struct Buddy {
    pub species: String,
    prof: Profile,
    rng: Rng,
    pub x: f32,
    pub dir: f32,
    pub state: usize,
    elapsed: f32,
    duration: f32,
    pub tex: Option<TextureHandle>,
}

impl Buddy {
    pub fn new(species: &str, slot: usize, track_max: f32) -> Self {
        let mut rng = Rng::new(&format!("{species}-{slot}"));
        let prof = profile(species);
        let (state, duration) = pick(&prof, &mut rng);
        let x = rng.next() * track_max.max(0.0);
        let dir = if rng.next() < 0.5 { -1.0 } else { 1.0 };
        Buddy { species: species.into(), prof, rng, x, dir, state, elapsed: 0.0, duration, tex: None }
    }

    pub fn advance(&mut self, dt_ms: f32, track_max: f32) {
        let max = track_max.max(0.0);
        self.elapsed += dt_ms;
        let speed = match self.state { WALK => self.prof.walk, RUN => self.prof.run, _ => 0.0 };
        if speed > 0.0 { self.x += speed * self.dir * dt_ms / 1000.0 }
        if self.x <= 0.0 { self.x = 0.0; if speed > 0.0 { self.dir = 1.0 } }
        else if self.x >= max { self.x = max; if speed > 0.0 { self.dir = -1.0 } }
        self.x = self.x.clamp(0.0, max);
        if self.elapsed >= self.duration {
            let (s, d) = pick(&self.prof, &mut self.rng);
            self.state = s; self.duration = d; self.elapsed = 0.0;
            if self.rng.next() < self.prof.turn { self.dir = -self.dir }
        }
    }

    /// Click: play the signature move once.
    pub fn special(&mut self) {
        self.state = SPECIAL;
        self.elapsed = 0.0;
        self.duration = self.prof.frame_ms[SPECIAL] * FRAMES as f32;
    }

    /// Ms until this buddy next needs a redraw: smooth motion while walking/running, otherwise only on frame changes.
    pub fn next_wake_ms(&self) -> f32 {
        if self.state == WALK || self.state == RUN { return 50.0 }
        let f = self.prof.frame_ms[self.state];
        (f - self.elapsed % f).max(16.0)
    }

    pub fn frame(&self) -> usize { ((self.elapsed / self.prof.frame_ms[self.state]) as usize) % FRAMES }
}

fn pick(p: &Profile, rng: &mut Rng) -> (usize, f32) {
    let total: f32 = p.weights.iter().map(|w| w.max(0.0)).sum();
    let (mut target, mut chosen) = (rng.next() * total, IDLE);
    for (i, w) in p.weights.iter().enumerate() {
        let w = w.max(0.0);
        if w <= 0.0 { continue }
        if target < w { chosen = i; break }
        target -= w;
        chosen = i;
    }
    let dur = p.min_ms + rng.next() * (p.max_ms - p.min_ms).max(0.0);
    if chosen == SPECIAL {
        // a signature move always plays whole loops
        return (chosen, p.frame_ms[SPECIAL] * FRAMES as f32 * if rng.next() < 0.5 { 1.0 } else { 2.0 });
    }
    (chosen, dur)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stays_on_track_and_every_atlas_decodes() {
        for s in SPECIES {
            let img = image::load_from_memory(atlas_bytes(s)).expect(s);
            assert_eq!((img.width(), img.height()), ((FRAMES * CELL) as u32, (6 * CELL) as u32));
            let mut b = Buddy::new(s, 0, 100.0);
            for _ in 0..2000 { b.advance(16.0, 100.0); assert!((0.0..=100.0).contains(&b.x)); }
        }
    }
}
