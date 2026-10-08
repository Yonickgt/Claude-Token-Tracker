// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod buddy;
mod glass;
mod menu;
mod tokens;

use buddy::Buddy;
use chrono::{Local, TimeZone};
use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, ResizeDirection, Sense,
    Stroke, StrokeKind, Vec2, ViewportCommand, WindowLevel,
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc},
    time::{Duration, Instant},
};
use tokens::{fmt_tok, now_ms, pct_left, Bar, Engine, Provider, Usage};

const RADIUS: f32 = 22.0; // used when glass is off; glass uses the system's 8px corner (glass::SYSTEM_RADIUS)
const HEADER_H: f32 = 38.0;
const COLLAPSED_H: f32 = 46.0;
const MIN_W: f32 = 215.0; // smallest size: the compact card with bar + buddies
const MIN_H: f32 = 172.0;

// ---------------------------------------------------------------- theme
#[derive(Clone, Copy)]
struct Theme {
    shell: Color32, card: Color32, ink: Color32, soft: Color32, muted: Color32, accent: Color32,
    accent_soft: Color32, track: Color32, good: Color32, warn: Color32, bad: Color32,
    dcard: Color32, dink: Color32, dsoft: Color32, dbad: Color32, blue: Color32, hairline: Color32, edge: Color32,
}
fn rgb(h: u32) -> Color32 { Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8) }
fn theme(dark: bool) -> Theme {
    if dark {
        // near-black with a faint violet cast, hairline border, white headings, grey body text
        Theme { shell: rgb(0x0e0d12), card: rgb(0x16151b), ink: rgb(0xffffff), soft: rgb(0xb4b3be), muted: rgb(0x8a8995), accent: rgb(0xdedcf0),
            accent_soft: rgb(0x2a2931), track: rgb(0x26252d), good: rgb(0x4cc27a), warn: rgb(0xf3892a), bad: rgb(0xff5b60),
            dcard: rgb(0x1d1c25), dink: rgb(0xffffff), dsoft: rgb(0x9a99a6), dbad: rgb(0xf0a6a0), blue: rgb(0x7c8cf0), hairline: rgb(0x26252c), edge: rgb(0x2c2b33) }
    } else {
        // StudyList "paper" + the dashboard's cream cards, brown accent, slate burn card
        Theme { shell: rgb(0xeae0d2), card: rgb(0xf7f3ec), ink: rgb(0x2d2d2d), soft: rgb(0x4a4542), muted: rgb(0x7d654a), accent: rgb(0xa68763),
            accent_soft: rgb(0xd9cab6), track: rgb(0xe4e0da), good: rgb(0x1f8a4f), warn: rgb(0xd9731a), bad: rgb(0xd13b40),
            dcard: rgb(0x26333f), dink: rgb(0xf4f1ea), dsoft: rgb(0xaab4bc), dbad: rgb(0xf0a6a0), blue: rgb(0x6f7fd1), hairline: rgb(0xd8ccb8), edge: rgb(0xf4eee4) }
    }
}

// ---------------------------------------------------------------- prefs
#[derive(Clone)]
struct Prefs {
    pos: Option<(f32, f32)>, size: (f32, f32), restore_h: f32, collapsed: bool, pinned: bool, top: bool, dark: bool, glass: bool,
    provider: Provider, buddies: Vec<String>,
}
impl Default for Prefs {
    fn default() -> Self {
        Prefs { pos: None, size: (440.0, 300.0), restore_h: 300.0, collapsed: false, pinned: false, top: true, dark: true, glass: false,
            provider: Provider::Claude, buddies: vec!["mossling".into(), "ghost".into(), "capling".into()] }
    }
}
fn prefs_path() -> PathBuf {
    let base = std::env::var("APPDATA").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    PathBuf::from(base).join("ClaudeTokenWidget").join("settings.json")
}
impl Prefs {
    fn load() -> Self {
        let mut p = Prefs::default();
        let Ok(text) = std::fs::read_to_string(prefs_path()) else { return p };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return p };
        let f = |k: &str| v[k].as_f64().map(|n| n as f32);
        if let (Some(x), Some(y)) = (f("x"), f("y")) { p.pos = Some((x, y)) }
        if let (Some(w), Some(h)) = (f("w"), f("h")) { p.size = (w.max(MIN_W), h.max(MIN_H)) }
        p.restore_h = f("restore_h").unwrap_or(p.size.1).max(MIN_H);
        p.collapsed = v["collapsed"].as_bool().unwrap_or(false);
        p.pinned = v["pinned"].as_bool().unwrap_or(false);
        p.top = v["top"].as_bool().unwrap_or(true);
        p.dark = !v["light"].as_bool().unwrap_or(false);
        p.glass = false; // glass look removed: always the solid card
        if v["provider"] == "codex" { p.provider = Provider::Codex }
        if let Some(a) = v["buddies"].as_array() {
            p.buddies = a.iter().filter_map(|s| s.as_str()).filter(|s| buddy::SPECIES.contains(s)).take(buddy::MAX_BUDDIES).map(String::from).collect();
        }
        p
    }
    fn save(&self) {
        let (x, y) = self.pos.unwrap_or((80.0, 80.0));
        let v = serde_json::json!({ "x": x, "y": y, "w": self.size.0, "h": self.size.1, "restore_h": self.restore_h,
            "collapsed": self.collapsed, "pinned": self.pinned, "top": self.top, "light": !self.dark, "glass": self.glass,
            "provider": if self.provider == Provider::Codex { "codex" } else { "claude" }, "buddies": self.buddies });
        let path = prefs_path();
        if let Some(d) = path.parent() { let _ = std::fs::create_dir_all(d); }
        let _ = std::fs::write(path, v.to_string());
    }
}

// ---------------------------------------------------------------- toasts (port of React Bits' SwipeToast)
const TOAST_MS: f32 = 4000.0; // fuse length: the toast closes itself after this
const SLIDE_MS: f32 = 400.0;
const EXIT_MS: f32 = 280.0;
const SWIPE_PX: f32 = 40.0; // a slow drag must pass this; a flick always dismisses

struct Toast {
    title: String,
    desc: String,
    fuse: Color32,
    born: Option<Instant>, // set when it first becomes visible (later toasts wait their turn)
    burned: f32,
    closing: Option<Instant>,
    pull: f32,   // raw downward drag in px (negative = pulled up, rubber-banded)
    frozen: f32, // drag offset at the moment it started closing
    dragging: bool,
}

fn rubberband(over: f32, dim: f32) -> f32 { let c = 0.55; (over * dim * c) / (dim + c * over.abs()) }
fn ease_out(x: f32) -> f32 { 1.0 - (1.0 - x).powi(4) }

/// The burning line: a horizontal gradient that fades at both ends and glows in the middle.
fn fuse_line(p: &egui::Painter, r: Rect, c: Color32, alpha: f32) {
    let stops = [(0.0, 0.0), (0.09, 0.18), (0.18, 0.5), (0.28, 0.84), (0.38, 1.0), (0.62, 1.0), (0.72, 0.84), (0.82, 0.5), (0.91, 0.18), (1.0, 0.0)];
    let mut m = egui::Mesh::default();
    for (f, a) in stops {
        let col = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * alpha * 255.0) as u8);
        let x = r.min.x + f * r.width();
        m.colored_vertex(Pos2::new(x, r.min.y), col);
        m.colored_vertex(Pos2::new(x, r.max.y), col);
    }
    for i in 0..(stops.len() as u32 - 1) { let b = i * 2; m.add_triangle(b, b + 1, b + 2); m.add_triangle(b + 1, b + 3, b + 2); }
    p.add(egui::Shape::mesh(m));
}

// ---------------------------------------------------------------- icon
/// The app icon (a cat on a light tile) as RGBA at `size` px, for the tray and the window.
fn app_icon(size: u32) -> (Vec<u8>, u32, u32) {
    let img = image::load_from_memory(include_bytes!("../assets/icon.png")).expect("icon.png").to_rgba8();
    let img = image::imageops::resize(&img, size, size, image::imageops::FilterType::Lanczos3);
    (img.into_raw(), size, size)
}

// ---------------------------------------------------------------- tray
static TRAY_TOGGLE: AtomicBool = AtomicBool::new(false);
static TRAY_QUIT: AtomicBool = AtomicBool::new(false);

/// Tray icon (the overflow area next to the clock): left-click shows/hides the widget, right-click has Show/Hide + Quit.
fn make_tray(ctx: &egui::Context) -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let (rgba, w, h) = app_icon(32);
    let menu = Menu::new();
    let toggle = MenuItem::with_id("toggle", "Show / Hide", true, None);
    let quit = MenuItem::with_id("quit", "Quit", true, None);
    menu.append_items(&[&toggle, &quit]).ok()?;
    let c = ctx.clone();
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            TRAY_TOGGLE.store(true, Ordering::Relaxed);
            c.request_repaint();
        }
    }));
    let c = ctx.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        match e.id.0.as_str() { "toggle" => TRAY_TOGGLE.store(true, Ordering::Relaxed), "quit" => TRAY_QUIT.store(true, Ordering::Relaxed), _ => {} }
        c.request_repaint();
    }));
    TrayIconBuilder::new().with_icon(Icon::from_rgba(rgba, w, h).ok()?).with_tooltip("Token Tracker").with_menu(Box::new(menu))
        .with_menu_on_left_click(false).build().ok()
}

// ---------------------------------------------------------------- app
struct App {
    _tray: Option<tray_icon::TrayIcon>,
    hidden: bool,
    toasts: VecDeque<Toast>,
    preview: usize,
    menu_st: menu::MenuState,
    prefs: Prefs,
    usage: Option<Usage>,
    ptx: mpsc::Sender<Provider>,
    urx: mpsc::Receiver<(Provider, Usage)>,
    avail: Vec<Provider>,
    buddies: Vec<Buddy>,
    last: Instant,
    last_save: Instant,
    dirty: bool,
    menu: bool,
    pill: bool,
    week: bool,
    fresh: bool,
    hwnd: Option<isize>,
    clip_px: (i32, i32),
}

fn home() -> PathBuf { PathBuf::from(std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default()) }

fn load_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut add = |key: &str, bytes: &'static [u8], fam: FontFamily| {
        fonts.font_data.insert(key.into(), Arc::new(FontData::from_static(bytes)));
        fonts.families.entry(fam).or_default().insert(0, key.into());
    };
    // body text: Inter Medium (the weight of React Bits' sidebar); titles, headings and big numerals: Inter Display Bold
    add("inter-medium", include_bytes!("../assets/fonts/Inter-Medium.ttf"), FontFamily::Proportional);
    add("inter-display-bold", include_bytes!("../assets/fonts/InterDisplay-Bold.ttf"), FontFamily::Monospace); // big numerals
    add("inter-display-bold", include_bytes!("../assets/fonts/InterDisplay-Bold.ttf"), FontFamily::Name("display".into())); // headings
    ctx.set_fonts(fonts);
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        load_fonts(&cc.egui_ctx);
        let mut prefs = Prefs::load();
        let (has_claude, has_codex) = (home().join(".claude/projects").exists(), home().join(".codex/sessions").exists());
        let mut avail = vec![];
        if has_claude || !has_codex { avail.push(Provider::Claude) }
        if has_codex { avail.push(Provider::Codex) }
        if !avail.contains(&prefs.provider) { prefs.provider = avail[0] } // system-specific default

        let (utx, urx) = mpsc::channel();
        let (ptx, prx) = mpsc::channel::<Provider>();
        let ctx = cc.egui_ctx.clone();
        let start = prefs.provider;
        std::thread::spawn(move || {
            let (mut engine, mut p) = (Engine::new(), start);
            loop {
                if utx.send((p, engine.compute(p))).is_err() { break }
                ctx.request_repaint();
                match prx.recv_timeout(Duration::from_secs(10)) {
                    Ok(np) => p = np,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => break,
                }
            }
        });
        let tray = make_tray(&cc.egui_ctx);
        let mut app = App { _tray: tray, hidden: false, prefs, usage: None, ptx, urx, avail, buddies: vec![], last: Instant::now(), last_save: Instant::now(),
            dirty: false, menu: false, pill: false, week: false, fresh: true, hwnd: None, clip_px: (0, 0), toasts: VecDeque::new(), preview: 0, menu_st: menu::MenuState::new() };
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(h) = cc.window_handle() { if let RawWindowHandle::Win32(w) = h.as_raw() { app.hwnd = Some(w.hwnd.get()) } }
        }
        if let Some(h) = app.hwnd { glass::apply(h, app.prefs.glass) } // explicit either way: also clears the system border
        app.rebuild_buddies(&cc.egui_ctx);
        app
    }

    fn rebuild_buddies(&mut self, ctx: &egui::Context) {
        let track = (self.prefs.size.0 - 24.0 - buddy::CELL as f32).max(0.0);
        self.buddies = self.prefs.buddies.iter().enumerate().map(|(i, s)| {
            let mut b = Buddy::new(s, i, track);
            b.tex = buddy::load_atlas(ctx, s);
            b
        }).collect();
    }

    fn touch(&mut self) { self.dirty = true }
    /// X hides to the tray (the app keeps running); Quit in the tray menu exits.
    fn hide(&mut self, ctx: &egui::Context) { self.hidden = true; ctx.send_viewport_cmd(ViewportCommand::Visible(false)); }
    fn show(&mut self, ctx: &egui::Context) {
        self.hidden = false;
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }
    fn set_provider(&mut self, p: Provider) {
        if p != self.prefs.provider { self.prefs.provider = p; self.usage = None; let _ = self.ptx.send(p); self.touch() }
    }
    fn toggle_collapse(&mut self, ctx: &egui::Context) {
        let (w, h) = self.prefs.size;
        self.prefs.collapsed = !self.prefs.collapsed;
        // the floor drops to header height while collapsed, and comes back on expand
        let (min_h, new_h) = if self.prefs.collapsed { self.prefs.restore_h = h; (COLLAPSED_H, COLLAPSED_H) } else { (MIN_H, self.prefs.restore_h.max(MIN_H)) };
        ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(Vec2::new(MIN_W, min_h)));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(w, new_h)));
        self.touch();
    }
}

impl eframe::App for App {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] { [0.0; 4] } // transparent: the rounded shell is painted by us

    fn ui(&mut self, ui: &mut egui::Ui, _f: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        while let Ok((p, u)) = self.urx.try_recv() {
            if p == self.prefs.provider {
                if let Some(prev) = self.usage.take() { self.notify_changes(&prev, &u) }
                self.usage = Some(u);
            }
        }

        if self.fresh {
            self.fresh = false;
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if self.prefs.top { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal }));
            // a saved spot on a monitor that's gone: pull the widget back into view
            if let (Some(mon), Some((x, y))) = (ctx.input(|i| i.viewport().monitor_size), self.prefs.pos) {
                if x > mon.x - 40.0 || y > mon.y - 40.0 || x < -self.prefs.size.0 + 40.0 || y < -20.0 {
                    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(80.0, 80.0)));
                }
            }
        }

        // remember geometry (debounced)
        if let Some(r) = ctx.input(|i| i.viewport().outer_rect) {
            let (x, y, w, h) = (r.min.x, r.min.y, r.width(), r.height());
            if self.prefs.pos != Some((x, y)) || (self.prefs.size.0 - w).abs() > 0.5 || (self.prefs.size.1 - h).abs() > 0.5 {
                self.prefs.pos = Some((x, y));
                self.prefs.size = (w, h);
                self.dirty = true;
            }
        }
        if self.dirty && self.last_save.elapsed() > Duration::from_secs(1) {
            self.prefs.save(); self.dirty = false; self.last_save = Instant::now();
        }

        if TRAY_QUIT.swap(false, Ordering::Relaxed) { ctx.send_viewport_cmd(ViewportCommand::Close) }
        if TRAY_TOGGLE.swap(false, Ordering::Relaxed) { if self.hidden { self.show(ctx) } else { self.hide(ctx) } }

        // flat mode: clip the window to the card's own rounded shape so nothing Windows draws at the window
        // edge shows outside it (glass mode relies on the system's corner rounding instead)
        if let Some(h) = self.hwnd { glass::strip_frame(h) }
        if let Some(h) = self.hwnd {
            if self.prefs.glass {
                if self.clip_px != (0, 0) { glass::clip(h, 0.0); self.clip_px = (0, 0) }
            } else {
                let ppp = ctx.pixels_per_point();
                let px = ctx.input(|i| i.viewport().outer_rect).map_or((0, 0), |r| ((r.width() * ppp) as i32, (r.height() * ppp) as i32));
                if px.0 > 0 && (px != self.clip_px || glass::region_missing(h)) { glass::clip(h, RADIUS * ppp); self.clip_px = px }
            }
        }

        let dt = (self.last.elapsed().as_secs_f32() * 1000.0).min(100.0);
        self.last = Instant::now();

        self.draw(ui, dt);

        // repaint only when something moves or a sprite frame changes; otherwise once a second keeps the countdown honest
        let wake = if self.buddies_visible(ctx) { self.buddies.iter().map(Buddy::next_wake_ms).fold(1000.0, f32::min) } else { 1000.0 };
        let wake = if self.toasts.is_empty() { wake } else { wake.min(16.0) }; // smooth toast motion
        ctx.request_repaint_after(Duration::from_millis(wake as u64));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) { self.prefs.save() }
}

impl App {
    fn push_toast(&mut self, title: String, desc: String, fuse: Color32) {
        if self.toasts.len() < 4 {
            self.toasts.push_back(Toast { title, desc, fuse, born: None, burned: 0.0, closing: None, pull: 0.0, frozen: 0.0, dragging: false });
        }
    }

    /// Fires when the session resets, or when "% left" drops below 75, 50, 20 or 10 (the lowest one crossed).
    fn notify_changes(&mut self, prev: &Usage, cur: &Usage) {
        let Some(ps) = &prev.session else { return };
        let now = now_ms();
        let Some(cs) = &cur.session else {
            self.push_toast("Session reset".into(), "A fresh 5h window starts with your next message".into(), rgb(0x4cc27a));
            return;
        };
        if cs.end > ps.end + 3_600_000 {
            self.push_toast("Session reset".into(), format!("Fresh 5h window · {} tokens", fmt_tok(cs.limit)), rgb(0x4cc27a));
            return;
        }
        let (lp, ln) = (pct_left(ps.used, ps.limit), pct_left(cs.used, cs.limit));
        if let Some(&t) = [10, 20, 50, 75].iter().find(|&&t| lp > t && ln <= t) {
            let title = if t <= 10 { format!("Only {t}% left") } else { format!("{t}% of session left") };
            let desc = format!("{} of {} tokens left · resets in {}", fmt_tok((cs.limit - cs.used).max(0.0)), fmt_tok(cs.limit), fmt_dur(cs.end - now));
            let fuse = match t { 75 => rgb(0xf5a524), 50 => rgb(0xf3892a), _ => rgb(0xff5b60) };
            self.push_toast(title, desc, fuse);
        }
    }

    /// Gear-menu "Preview notification": cycles through every alert so they can be seen on demand.
    fn preview_toast(&mut self) {
        let steps: [(&str, &str, u32); 5] = [
            ("75% of session left", "3.41M of 4.55M tokens left", 0xf5a524),
            ("50% of session left", "2.27M of 4.55M tokens left", 0xf3892a),
            ("20% of session left", "910.0K of 4.55M tokens left", 0xff5b60),
            ("Only 10% left", "455.0K of 4.55M tokens left", 0xff5b60),
            ("Session reset", "Fresh 5h window · 4.55M tokens", 0x4cc27a),
        ];
        let (t, d, c) = steps[self.preview % steps.len()];
        self.preview += 1;
        self.push_toast(t.into(), d.into(), rgb(c));
    }

    fn draw_toast(&mut self, ui: &mut egui::Ui, full: Rect, dt: f32) {
        let ctx = ui.ctx().clone();
        let collapsed = self.prefs.collapsed;
        let Some(t) = self.toasts.front_mut() else { return };
        let now = Instant::now();
        let born = *t.born.get_or_insert(now);
        let (h, w) = (if collapsed { 36.0 } else { 48.0 }, (full.width() - 24.0).min(356.0));
        let top = if collapsed { full.min.y + (full.height() - 36.0) / 2.0 } else { full.max.y - 10.0 - 48.0 };
        let rest = Rect::from_min_size(Pos2::new(full.center().x - w / 2.0, top), Vec2::new(w, h));

        // rise on entry
        let et = (now.duration_since(born).as_secs_f32() * 1000.0 / SLIDE_MS).min(1.0);
        let mut off = (1.0 - ease_out(et)) * (h + 24.0);
        let mut alpha = ease_out(et);
        // drag offset (rubber-banded when pulled up)
        let drag_off = if t.closing.is_some() { t.frozen } else if t.pull >= 0.0 { t.pull } else { rubberband(t.pull, 24.0) };
        off += drag_off;
        let mut done = false;
        if let Some(c) = t.closing {
            let ct = (now.duration_since(c).as_secs_f32() * 1000.0 / EXIT_MS).min(1.0);
            off += ease_out(ct) * (h + 24.0);
            alpha = 1.0 - ct;
            done = ct >= 1.0;
        }
        let card = rest.translate(Vec2::new(0.0, off));

        if t.closing.is_none() && et > 0.6 {
            let resp = ui.interact(card, egui::Id::new("toast"), Sense::drag());
            let hover = resp.hovered() || resp.dragged();
            if resp.drag_started() { t.dragging = true }
            if resp.dragged() { t.pull += resp.drag_delta().y }
            if resp.drag_stopped() {
                t.dragging = false;
                let v = ctx.input(|i| i.pointer.velocity().y); // px/s; a flick is > 110
                if t.pull > 0.0 && (t.pull >= SWIPE_PX || v > 110.0) { t.frozen = t.pull; t.closing = Some(now) }
            } else if !t.dragging && t.pull != 0.0 {
                t.pull *= (-dt / 90.0).exp(); // settle back
                if t.pull.abs() < 0.2 { t.pull = 0.0 }
            }
            if !hover && !t.dragging { t.burned += dt } // hovering freezes the line and the timer
            if t.burned >= TOAST_MS || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { t.frozen = t.pull.max(0.0); t.closing = Some(now) }
        }

        let p = ui.painter();
        let a = alpha.clamp(0.0, 1.0);
        let bg = rgb(0x27272a);
        p.rect_filled(card.translate(Vec2::new(0.0, 2.0)).expand(1.0), 13.0, Color32::from_black_alpha((70.0 * a) as u8));
        p.rect_filled(card, 12.0, bg.gamma_multiply(a));
        let ink = rgb(0xf5f5f5);
        let clip = p.with_clip_rect(card.shrink2(Vec2::new(10.0, 0.0)));
        let ty = if collapsed || t.desc.is_empty() { card.center().y } else { card.min.y + 17.0 };
        clip.text(Pos2::new(card.min.x + 14.0, ty), Align2::LEFT_CENTER, &t.title, FontId::proportional(13.0), ink.gamma_multiply(a));
        if !collapsed && !t.desc.is_empty() {
            clip.text(Pos2::new(card.min.x + 14.0, card.min.y + 33.0), Align2::LEFT_CENTER, &t.desc, FontId::proportional(11.5), ink.gamma_multiply(0.62 * a));
        }
        let frac = (1.0 - t.burned / TOAST_MS).clamp(0.0, 1.0);
        if frac > 0.0 { fuse_line(&p.with_clip_rect(card), Rect::from_min_size(Pos2::new(card.min.x, card.max.y - 2.0), Vec2::new(w * frac, 2.0)), t.fuse, a) }

        if done { self.toasts.pop_front(); }
    }

    fn buddies_visible(&self, ctx: &egui::Context) -> bool {
        let r = ctx.content_rect();
        !self.buddies.is_empty() && !self.prefs.collapsed && r.height() >= 110.0
    }

    fn draw(&mut self, ui: &mut egui::Ui, dt: f32) {
        let ctx = ui.ctx().clone();
        let full = ui.max_rect();
        let (w, h) = (full.width(), full.height());
        let t = theme(self.prefs.dark);
        let p = ui.painter().clone();
        let glass = self.prefs.glass;
        let shell = if glass { Color32::from_rgba_unmultiplied(t.shell.r(), t.shell.g(), t.shell.b(), if self.prefs.dark { 120 } else { 150 }) } else { t.shell };
        let radius = if glass { glass::SYSTEM_RADIUS } else { RADIUS };
        p.rect_filled(full, radius, shell);
        // no outline stroke: any rim shows as a light ring against bright backdrops
        let now = now_ms();
        let u = self.usage.clone();

        let tiny = !self.prefs.collapsed && (w < 190.0 || h < 120.0);
        let pinned = self.prefs.pinned;
        // registered first: later widgets (header drag, buttons, buddies) sit on top of it
        let bg = ui.interact(full, egui::Id::new("shell"), Sense::click_and_drag());

        // ---- header / tiny
        if tiny {
            self.draw_tiny(ui, full, &t, u.as_ref());
        } else {
            self.draw_header(ui, full, &t, u.as_ref());
        }

        // ---- body
        if !self.prefs.collapsed && !tiny {
            let body = Rect::from_min_max(Pos2::new(full.min.x + 10.0, full.min.y + HEADER_H), Pos2::new(full.max.x - 10.0, full.max.y - 10.0));
            let body = if self.buddies_visible(&ctx) { Rect::from_min_max(body.min, Pos2::new(body.max.x, body.max.y - 22.0)) } else { body };
            self.draw_body(ui, body, &t, u.as_ref(), now);
        }

        // ---- buddies (overlay along the bottom, like StudyList's shelf)
        if self.buddies_visible(&ctx) { self.draw_buddies(ui, full, dt) }
        self.draw_toast(ui, full, dt);

        // ---- settings gear + menu
        if !self.prefs.collapsed && !tiny { self.draw_gear(ui, full, &t) }

        // right-click anywhere opens settings too (the only way in tiny mode)
        if bg.secondary_clicked() { self.menu = !self.menu }
        if tiny && !pinned && bg.drag_started_by(egui::PointerButton::Primary) { ctx.send_viewport_cmd(ViewportCommand::StartDrag) }
        if self.menu { self.draw_menu(ui, full, &t) }

        if !pinned && !self.prefs.collapsed { self.resize_handles(ui, full) }
    }

    // ------------------------------------------------------------ chrome
    fn icon_btn(&self, ui: &egui::Ui, r: Rect, id: &str, t: &Theme, glyph: u8) -> egui::Response {
        let resp = ui.interact(r, egui::Id::new(id), Sense::click());
        let p = ui.painter();
        if resp.hovered() { p.rect_filled(r, 9.6, t.accent_soft); }
        let c = if resp.hovered() { t.soft } else { t.muted };
        let m = r.center();
        let s = Stroke::new(1.5, c);
        match glyph {
            0 => { p.line_segment([m + Vec2::new(-4.0, 2.0), m + Vec2::new(0.0, -2.0)], s); p.line_segment([m + Vec2::new(0.0, -2.0), m + Vec2::new(4.0, 2.0)], s); } // chevron up
            1 => { p.line_segment([m + Vec2::new(-4.0, -2.0), m + Vec2::new(0.0, 2.0)], s); p.line_segment([m + Vec2::new(0.0, 2.0), m + Vec2::new(4.0, -2.0)], s); } // chevron down
            _ => { p.line_segment([m + Vec2::new(-4.0, -4.0), m + Vec2::new(4.0, 4.0)], s); p.line_segment([m + Vec2::new(-4.0, 4.0), m + Vec2::new(4.0, -4.0)], s); } // close
        }
        resp
    }

    fn draw_header(&mut self, ui: &mut egui::Ui, full: Rect, t: &Theme, u: Option<&Usage>) {
        let ctx = ui.ctx().clone();
        let p = ui.painter().clone();
        let btn = 27.2;
        let y = full.min.y + (HEADER_H - btn) / 2.0 + 1.0;
        let mut right = full.max.x - 8.0;

        let close = Rect::from_min_size(Pos2::new(right - btn, y), Vec2::splat(btn));
        if self.icon_btn(ui, close, "close", t, 2).clicked() { self.hide(&ctx) }
        right -= btn + 2.0;
        let coll = Rect::from_min_size(Pos2::new(right - btn, y), Vec2::splat(btn));
        let glyph = if self.prefs.collapsed { 1 } else { 0 };
        if self.icon_btn(ui, coll, "collapse", t, glyph).clicked() { self.toggle_collapse(&ctx) }
        right -= btn + 4.0;

        // provider pill ("All work" slot): only when there is room and more than one tool to pick
        if !self.prefs.collapsed && full.width() >= 270.0 {
            let pill = Rect::from_min_size(Pos2::new(right - 104.0, y), Vec2::new(104.0, btn));
            let name = if self.prefs.provider == Provider::Codex { "Codex" } else { "Claude" };
            let resp = ui.interact(pill, egui::Id::new("pill"), Sense::click());
            p.rect_filled(pill, 13.6, if resp.hovered() { t.accent_soft } else { t.accent_soft.gamma_multiply(0.7) });
            p.text(Pos2::new(pill.min.x + 12.0, pill.center().y), Align2::LEFT_CENTER, name, FontId::proportional(12.0), t.soft);
            let m = Pos2::new(pill.max.x - 13.0, pill.center().y);
            let s = Stroke::new(1.3, t.muted);
            p.line_segment([m + Vec2::new(-3.5, -1.5), m + Vec2::new(0.0, 2.0)], s);
            p.line_segment([m + Vec2::new(0.0, 2.0), m + Vec2::new(3.5, -1.5)], s);
            if resp.clicked() && self.avail.len() > 1 { self.pill = !self.pill } else if resp.clicked() { self.pill = false }
            if self.pill {
                let (opts, mut chosen) = (self.avail.clone(), None);
                egui::Area::new(egui::Id::new("pill-list")).order(egui::Order::Foreground).fixed_pos(Pos2::new(pill.min.x, pill.max.y + 4.0)).show(&ctx, |ui| {
                    menu::paper_frame().show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                        for o in opts {
                            let label = if o == Provider::Codex { "Codex" } else { "Claude" };
                            if menu::paper_row(ui, label, o == self.prefs.provider, 104.0).clicked() { chosen = Some(o) }
                        }
                    });
                });
                if let Some(o) = chosen { self.set_provider(o); self.pill = false }
                if ctx.input(|i| i.pointer.any_pressed()) && !ctx.input(|i| i.pointer.hover_pos().is_some_and(|q| pill.contains(q) || Rect::from_min_size(Pos2::new(pill.min.x, pill.max.y), Vec2::new(110.0, 70.0)).contains(q))) { self.pill = false }
            }
            right -= 104.0 + 6.0;
        } else { self.pill = false }

        // title (also the drag handle, like StudyList's header)
        let title_rect = Rect::from_min_max(Pos2::new(full.min.x + 12.0, full.min.y), Pos2::new((right).max(full.min.x + 60.0), full.min.y + HEADER_H));
        let title = if self.prefs.collapsed {
            match u.and_then(|u| u.session.as_ref().map(|s| pct_left(s.used, s.limit))).or(u.map(|_| 100)) {
                Some(l) => format!("Token Tracker · {l}% left"), None => "Token Tracker".into() }
        } else { "Token Tracker".into() };
        p.with_clip_rect(title_rect).text(Pos2::new(title_rect.min.x, title_rect.center().y + 1.0), Align2::LEFT_CENTER, title,
            FontId::new(13.0, FontFamily::Name("display".into())), t.ink);
        let drag = ui.interact(title_rect, egui::Id::new("drag"), Sense::click_and_drag());
        if !self.prefs.pinned && drag.drag_started_by(egui::PointerButton::Primary) { ctx.send_viewport_cmd(ViewportCommand::StartDrag) }
        if drag.double_clicked() { self.toggle_collapse(&ctx) }
    }

    /// Smallest size: just the percentage, scaled to fill the window. X appears on hover.
    fn draw_tiny(&mut self, ui: &mut egui::Ui, full: Rect, t: &Theme, u: Option<&Usage>) {
        let ctx = ui.ctx().clone();
        let left = u.map(|u| u.session.as_ref().map_or(100, |s| pct_left(s.used, s.limit)));
        let txt = left.map_or("…".to_string(), |l| l.to_string());
        let col = left.map_or(t.muted, |l| tone(t, l));
        let area = full.shrink2(Vec2::new(8.0, 4.0));
        let g = fit_text(ui.painter(), &txt, FontFamily::Monospace, area.width() * 0.8, area.height() * 0.8, col);
        let pct = ui.painter().layout_no_wrap("%".into(), FontId::new((g.size().y * 0.4).max(9.0), FontFamily::Proportional), t.muted);
        let total_w = g.size().x + pct.size().x + 2.0;
        let x0 = full.center().x - total_w / 2.0;
        ui.painter().galley(Pos2::new(x0, full.center().y - g.size().y / 2.0), g.clone(), col);
        ui.painter().galley(Pos2::new(x0 + g.size().x + 2.0, full.center().y + g.size().y / 2.0 - pct.size().y - g.size().y * 0.12), pct, t.muted);
        if ui.rect_contains_pointer(full) && full.width() >= 70.0 {
            let r = Rect::from_min_size(Pos2::new(full.max.x - 24.0, full.min.y + 4.0), Vec2::splat(20.0));
            if self.icon_btn(ui, r, "close-t", t, 2).clicked() { self.hide(&ctx) }
        }
    }

    // ------------------------------------------------------------ body layouts
    fn draw_body(&mut self, ui: &mut egui::Ui, b: Rect, t: &Theme, u: Option<&Usage>, now: i64) {
        let Some(u) = u else {
            ui.painter().text(b.center(), Align2::CENTER_CENTER, "Reading usage…", FontId::proportional(13.0), t.muted);
            return;
        };
        if !u.logs_found {
            let name = if self.prefs.provider == Provider::Codex { "Codex" } else { "Claude" };
            ui.painter().text(b.center(), Align2::CENTER_CENTER, format!("No {name} logs found on this PC"), FontId::proportional(13.0), t.muted);
            return;
        }
        let (w, h) = (b.width(), b.height());
        let gap = 8.0;
        if w >= 500.0 && h >= 290.0 {
            // wide: session + burn on the left, stat strip + trend on the right (the dashboard page)
            let lw = (w * 0.36).clamp(190.0, 280.0);
            let left = Rect::from_min_size(b.min, Vec2::new(lw, h));
            let right = Rect::from_min_max(Pos2::new(b.min.x + lw + gap, b.min.y), b.max);
            let burn_h = (h * 0.34).clamp(88.0, 130.0);
            self.session_card(ui, Rect::from_min_max(left.min, Pos2::new(left.max.x, left.max.y - burn_h - gap)), t, u, now, true);
            self.burn_card(ui, Rect::from_min_max(Pos2::new(left.min.x, left.max.y - burn_h), left.max), t, u, now);
            let tiles_h = (h * 0.30).clamp(78.0, 110.0);
            self.tiles_card(ui, Rect::from_min_size(right.min, Vec2::new(right.width(), tiles_h)), t, u, 4);
            self.trend_card(ui, Rect::from_min_max(Pos2::new(right.min.x, right.min.y + tiles_h + gap), right.max), t, u);
        } else if w >= 300.0 && h >= 230.0 || h >= 330.0 {
            // stacked: add cards in priority order while they fit
            let mut y = b.min.y;
            let mut room = h;
            let sess_min = 120.0;
            let items: [(f32, u8); 3] = [(86.0, 0), (if w >= 400.0 { 78.0 } else { 118.0 }, 1), (150.0, 2)]; // burn, tiles, trend
            let mut plan: Vec<(u8, f32)> = vec![];
            let mut need = sess_min;
            for (min_h, kind) in items { if need + gap + min_h <= room { plan.push((kind, min_h)); need += gap + min_h } }
            let extra = room - need;
            let sess_h = sess_min + if plan.is_empty() { extra } else { extra * 0.4 };
            self.session_card(ui, Rect::from_min_size(Pos2::new(b.min.x, y), Vec2::new(w, sess_h)), t, u, now, true);
            y += sess_h + gap; room -= sess_h + gap;
            let _ = room;
            let spare = (extra - (sess_h - sess_min)).max(0.0);
            let last = plan.len().saturating_sub(1);
            for (i, (kind, min_h)) in plan.iter().enumerate() {
                let hh = min_h + if i == last { spare } else { 0.0 };
                let r = Rect::from_min_size(Pos2::new(b.min.x, y), Vec2::new(w, hh));
                match kind {
                    0 => self.burn_card(ui, r, t, u, now),
                    1 => self.tiles_card(ui, r, t, u, if w >= 400.0 { 4 } else { 2 }),
                    _ => self.trend_card(ui, r, t, u),
                }
                y += hh + gap;
            }
        } else {
            self.session_card(ui, b, t, u, now, h >= 150.0);
        }
    }

    fn card(&self, ui: &egui::Ui, r: Rect, fill: Color32) {
        // on glass the cards are thin frosted panes instead of solid fills
        let fill = if !self.prefs.glass { fill } else if fill == theme(self.prefs.dark).dcard { Color32::from_rgba_unmultiplied(24, 34, 52, 150) } else { Color32::from_white_alpha(if self.prefs.dark { 14 } else { 110 }) };
        let cr = if self.prefs.glass { 10.0 } else { 16.0 }; // inner corners stay smaller than the shell's
        ui.painter().rect_filled(r, cr, fill);
        ui.painter().rect_stroke(r.shrink(0.5), cr, Stroke::new(1.0, if self.prefs.dark { Color32::from_white_alpha(10) } else { Color32::from_white_alpha(90) }), StrokeKind::Inside);
    }

    fn session_card(&self, ui: &mut egui::Ui, r: Rect, t: &Theme, u: &Usage, now: i64, detail: bool) {
        self.card(ui, r, t.card);
        let p = ui.painter().clone();
        let pad = if r.height() < 110.0 { 8.0 } else { 14.0 };
        let inner = r.shrink(pad);
        let (used, limit) = u.session.as_ref().map_or((0.0, 5_000_000.0), |s| (s.used, s.limit));
        let left = if u.session.is_some() { pct_left(used, limit) } else { 100 };
        let col = tone(t, left);

        let show_label = r.height() >= 120.0;
        let show_window = detail && r.height() >= 125.0;
        let label_h = if show_label { 18.0 } else { 0.0 };
        let lower = 16.0 + 6.0 + 8.0 + if show_window { 22.0 } else { 0.0 };
        if show_label {
            p.text(inner.min, Align2::LEFT_TOP, if u.session.is_some() || self.prefs.provider == Provider::Codex { "5H SESSION" } else { "5H SESSION · IDLE" }, FontId::new(11.0, FontFamily::Proportional), t.accent);
        }
        if show_label && inner.width() >= 190.0 {
            let wk = format!("Week {}% left · {}", pct_left(u.week_used, u.week_limit), fmt_tok((u.week_limit - u.week_used).max(0.0)));
            p.text(Pos2::new(inner.max.x, inner.min.y), Align2::RIGHT_TOP, wk, FontId::new(11.0, FontFamily::Proportional), t.muted);
        }
        let num_area = Rect::from_min_max(Pos2::new(inner.min.x, inner.min.y + label_h), Pos2::new(inner.max.x, inner.max.y - lower));
        let g = fit_text(&p, &left.to_string(), FontFamily::Monospace, num_area.width() * 0.72, num_area.height().max(18.0), col);
        let pct_font = (g.size().y * 0.40).max(10.0);
        let pg = p.layout_no_wrap("%".into(), FontId::new(pct_font, FontFamily::Proportional), t.muted);
        p.galley(Pos2::new(num_area.min.x, num_area.center().y - g.size().y / 2.0), g.clone(), col);
        p.galley(Pos2::new(num_area.min.x + g.size().x + 3.0, num_area.center().y + g.size().y * 0.18 - pg.size().y / 2.0), pg, t.muted);

        // tokens left / bar / window
        let mut y = inner.max.y - lower + 2.0;
        let line = format!("{} of {} tokens left", fmt_tok((limit - used).max(0.0)), fmt_tok(limit));
        p.text(Pos2::new(inner.min.x, y), Align2::LEFT_TOP, line, FontId::new(12.5, FontFamily::Proportional), t.soft);
        y += 21.0;
        let bar = Rect::from_min_size(Pos2::new(inner.min.x, y), Vec2::new(inner.width(), 8.0));
        p.rect_filled(bar, 4.0, t.track);
        p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * left as f32 / 100.0, 8.0)), 4.0, t.accent);
        if show_window {
            y += 14.0;
            let txt = match &u.session {
                Some(s) => format!("{} – {}  ·  resets in {}", fmt_dt(s.start, "%a %H:%M"), fmt_dt(s.end, "%H:%M"), fmt_dur(s.end - now)),
                None => "No active session".to_string(),
            };
            p.text(Pos2::new(inner.min.x, y), Align2::LEFT_TOP, txt, FontId::new(11.5, FontFamily::Proportional), t.muted);
        }
    }

    fn burn_card(&self, ui: &mut egui::Ui, r: Rect, t: &Theme, u: &Usage, now: i64) {
        self.card(ui, r, t.dcard);
        let p = ui.painter().clone();
        let inner = r.shrink(12.0);
        p.text(inner.min, Align2::LEFT_TOP, "BURN RATE", FontId::new(10.5, FontFamily::Proportional), t.dsoft);
        p.text(Pos2::new(inner.max.x, inner.min.y), Align2::RIGHT_TOP, "live session", FontId::new(10.5, FontFamily::Proportional), t.dsoft);
        let Some(b) = &u.burn else {
            p.text(Pos2::new(inner.min.x, inner.center().y), Align2::LEFT_CENTER, "No active session", FontId::proportional(12.0), t.dsoft);
            return;
        };
        let big = (inner.height() * 0.34).clamp(16.0, 32.0);
        let g = p.layout_no_wrap(fmt_tok(b.per_hour), FontId::new(big, FontFamily::Monospace), t.dink);
        let gy = inner.min.y + 16.0;
        p.galley(Pos2::new(inner.min.x, gy), g.clone(), t.dink);
        p.text(Pos2::new(inner.min.x + g.size().x + 4.0, gy + g.size().y - 3.0), Align2::LEFT_BOTTOM, "/hour", FontId::proportional(11.5), t.dsoft);
        let remain = u.session.as_ref().map_or(0, |s| s.end - now);
        let (msg, c) = match b.eta_ms {
            Some(e) if (e as i64) < remain => (format!("Hits the limit in {}, before the window resets.", fmt_dur(e as i64)), t.dbad),
            Some(_) => ("On pace to last until the window resets.".to_string(), t.dsoft),
            None => ("Limit reached.".to_string(), t.dbad),
        };
        if inner.height() >= 70.0 {
            p.with_clip_rect(inner).text(Pos2::new(inner.min.x, gy + g.size().y + 4.0), Align2::LEFT_TOP, msg, FontId::proportional(11.5), c);
        }
        let used = u.session.as_ref().map_or(0.0, |s| s.used / s.limit).clamp(0.0, 1.0) as f32;
        let bar = Rect::from_min_size(Pos2::new(inner.min.x, inner.max.y - 5.0), Vec2::new(inner.width(), 5.0));
        p.rect_filled(bar, 2.5, Color32::from_white_alpha(28));
        p.rect_filled(Rect::from_min_size(bar.min, Vec2::new((bar.width() * used).max(5.0), 5.0)), 2.5, t.dink);
    }

    fn tiles_card(&self, ui: &mut egui::Ui, r: Rect, t: &Theme, u: &Usage, cols: usize) {
        self.card(ui, r, t.card);
        let p = ui.painter().clone();
        let tiles: [(&str, String, &str, Option<f32>); 4] = [
            ("CACHE HIT", format!("{:.0}%", u.cache_hit * 100.0), "of input came from cache", Some(u.cache_hit as f32)),
            ("OUTPUT RATIO", format!("{:.1}%", u.output_ratio * 100.0), "output per token read in", Some((u.output_ratio * 10.0).min(1.0) as f32)),
            ("PER MESSAGE", format!("${:.2}", u.cost_per_msg), "", None),
            ("TOOLS PER TURN", format!("{:.1}", u.tools_per_msg), "tool calls per reply", None),
        ];
        let rows = 4 / cols;
        let (cw, ch) = (r.width() / cols as f32, r.height() / rows as f32);
        for (i, (label, val, sub, bar)) in tiles.iter().enumerate() {
            let (cx, cy) = ((i % cols) as f32, (i / cols) as f32);
            let cell = Rect::from_min_size(Pos2::new(r.min.x + cx * cw, r.min.y + cy * ch), Vec2::new(cw, ch)).shrink2(Vec2::new(12.0, 8.0));
            if i % cols != 0 { p.line_segment([Pos2::new(cell.min.x - 12.0, r.min.y + cy * ch + 8.0), Pos2::new(cell.min.x - 12.0, r.min.y + (cy + 1.0) * ch - 8.0)], Stroke::new(1.0, t.hairline)); }
            let narrow = cell.width() < 130.0;
            let sub = match *label {
                "PER MESSAGE" => format!("{} messages", u.messages),
                "CACHE HIT" if narrow => "from cache".to_string(),
                "OUTPUT RATIO" if narrow => "out per token in".to_string(),
                "TOOLS PER TURN" if narrow => "calls per reply".to_string(),
                _ => sub.to_string(),
            };
            let vs = (cell.height() * 0.42).clamp(15.0, 28.0);
            let clip = p.with_clip_rect(cell);
            clip.text(cell.min, Align2::LEFT_TOP, label, FontId::new(10.0, FontFamily::Proportional), t.muted);
            clip.text(Pos2::new(cell.min.x, cell.min.y + 13.0), Align2::LEFT_TOP, val, FontId::new(vs, FontFamily::Monospace), t.ink);
            let mut y = cell.min.y + 13.0 + vs + 4.0;
            if let (Some(f), true) = (bar, cell.height() >= 70.0) {
                let b = Rect::from_min_size(Pos2::new(cell.min.x, y), Vec2::new(cell.width(), 4.0));
                p.rect_filled(b, 2.0, t.track);
                p.rect_filled(Rect::from_min_size(b.min, Vec2::new((b.width() * f.clamp(0.0, 1.0)).max(3.0), 4.0)), 2.0, t.accent);
                y += 8.0;
            }
            if y + 11.0 <= cell.max.y + 2.0 { clip.text(Pos2::new(cell.min.x, y), Align2::LEFT_TOP, sub, FontId::proportional(10.5), t.muted); }
        }
    }

    fn trend_card(&mut self, ui: &mut egui::Ui, r: Rect, t: &Theme, u: &Usage) {
        self.card(ui, r, t.card);
        let p = ui.painter().clone();
        let inner = r.shrink(12.0);
        p.text(inner.min, Align2::LEFT_TOP, "TOKEN TREND", FontId::new(10.5, FontFamily::Proportional), t.muted);
        // Day / Week toggle
        for (i, (label, is_week)) in [("Day", false), ("Week", true)].iter().enumerate() {
            let w = if *is_week { 46.0 } else { 40.0 };
            let x = inner.max.x - if *is_week { w } else { 46.0 + 4.0 + w };
            let pill = Rect::from_min_size(Pos2::new(x, inner.min.y - 4.0), Vec2::new(w, 22.0));
            let on = self.week == *is_week;
            let resp = ui.interact(pill, egui::Id::new(("trend", i)), Sense::click());
            p.rect_filled(pill, 11.0, if on { t.accent } else { t.track });
            p.text(pill.center(), Align2::CENTER_CENTER, label, FontId::new(11.0, FontFamily::Proportional), if on { Color32::WHITE } else { t.soft });
            if resp.clicked() { self.week = *is_week }
        }
        let bars: &[Bar] = if self.week { &u.week_bars } else { &u.day_bars };
        let total: f64 = bars.iter().map(|b| b.main + b.other).sum();
        let step = if self.week { 24 * 3_600_000 } else { 3_600_000 };
        let start = if self.week { u.week_day0 } else { u.day_start };

        let show_total = inner.height() >= 96.0;
        let top = inner.min.y + 18.0;
        // hovered bar's reading replaces the subtitle
        let area = Rect::from_min_max(Pos2::new(inner.min.x, top + if show_total { 40.0 } else { 0.0 }), Pos2::new(inner.max.x, inner.max.y - 14.0));
        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|q| area.expand2(Vec2::new(0.0, 6.0)).contains(*q));
        let n = bars.len();
        let gap = 3.0;
        let bw = (area.width() - gap * (n as f32 - 1.0)) / n as f32;
        let hov_i = hover.map(|q| (((q.x - area.min.x) / (bw + gap)) as usize).min(n - 1));
        if show_total {
            p.text(Pos2::new(inner.min.x, top), Align2::LEFT_TOP, fmt_tok(total), FontId::new(24.0, FontFamily::Monospace), t.ink);
            let sub = match hov_i {
                Some(i) => format!("{} · {}", fmt_dt(start + i as i64 * step, if self.week { "%a %d" } else { "%H:00" }), fmt_tok(bars[i].main + bars[i].other)),
                None => if self.week { "last 7 days".into() } else { "last 24 hours".into() },
            };
            p.text(Pos2::new(inner.min.x, top + 28.0), Align2::LEFT_TOP, sub, FontId::proportional(11.0), t.muted);
        }
        if area.height() < 23.0 { return }
        let max = bars.iter().map(|b| b.main + b.other).fold(1.0, f64::max);
        for (i, b) in bars.iter().enumerate() {
            let x = area.min.x + i as f32 * (bw + gap);
            let tot = b.main + b.other;
            let hh = (tot / max) as f32 * (area.height() - 2.0);
            let base = area.max.y;
            if tot <= 0.0 {
                p.rect_filled(Rect::from_min_size(Pos2::new(x, base - 2.0), Vec2::new(bw, 2.0)), 1.0, t.accent_soft);
                continue;
            }
            let hm = (b.main / max) as f32 * (area.height() - 2.0);
            let main_c = if Some(i) == hov_i { t.accent.gamma_multiply(1.15) } else { t.accent };
            p.rect_filled(Rect::from_min_max(Pos2::new(x, base - hm.max(2.0)), Pos2::new(x + bw, base)), 1.5, main_c);
            if b.other > 0.0 { p.rect_filled(Rect::from_min_max(Pos2::new(x, base - hh), Pos2::new(x + bw, base - hm)), 1.5, t.blue); }
        }
        // x-axis
        for i in 0..n {
            let ts = start + i as i64 * step;
            let show = self.week || Local.timestamp_millis_opt(ts).unwrap().format("%H").to_string().parse::<u32>().unwrap_or(1) % 6 == 0;
            if show {
                let x = area.min.x + i as f32 * (bw + gap);
                p.text(Pos2::new(x, area.max.y + 3.0), Align2::LEFT_TOP, fmt_dt(ts, if self.week { "%a" } else { "%H:00" }), FontId::proportional(10.0), t.muted);
            }
        }
    }

    // ------------------------------------------------------------ buddies
    fn draw_buddies(&mut self, ui: &mut egui::Ui, full: Rect, dt: f32) {
        let cell = buddy::CELL as f32;
        let track = (full.width() - 24.0 - cell).max(0.0);
        let y = full.max.y - 10.0 - cell + 4.0;
        for (i, b) in self.buddies.iter_mut().enumerate() {
            b.advance(dt, track);
            let r = Rect::from_min_size(Pos2::new(full.min.x + 12.0 + b.x, y), Vec2::splat(cell));
            if let Some(tex) = &b.tex {
                let (fx, fy) = (b.frame() as f32 / buddy::FRAMES as f32, b.state as f32 / 6.0);
                let fw = 1.0 / buddy::FRAMES as f32;
                let (u0, u1) = if b.dir < 0.0 { (fx + fw, fx) } else { (fx, fx + fw) }; // sheets face right: flip to face left
                let uv = Rect::from_min_max(Pos2::new(u0, fy), Pos2::new(u1, fy + 1.0 / 6.0));
                ui.painter().image(tex.id(), r, uv, Color32::WHITE);
            }
            if ui.interact(r, egui::Id::new(("buddy", i)), Sense::click()).clicked() { b.special() }
        }
    }

    // ------------------------------------------------------------ settings
    fn draw_gear(&mut self, ui: &mut egui::Ui, full: Rect, t: &Theme) {
        let r = Rect::from_min_size(Pos2::new(full.min.x + 3.2, full.max.y - 4.0 - 20.8), Vec2::splat(20.8));
        let resp = ui.interact(r, egui::Id::new("gear"), Sense::click());
        let c = if resp.hovered() || self.menu { t.soft } else { t.muted.gamma_multiply(0.65) };
        let p = ui.painter();
        if resp.hovered() || self.menu { p.rect_filled(r, 7.2, t.accent_soft); }
        let m = r.center();
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            let d = Vec2::new(a.cos(), a.sin());
            p.line_segment([m + d * 5.0, m + d * 7.0], Stroke::new(1.6, c));
        }
        p.circle_stroke(m, 5.0, Stroke::new(1.3, c));
        p.circle_stroke(m, 2.0, Stroke::new(1.3, c));
        if resp.clicked() { self.menu = !self.menu }
    }

    fn draw_menu(&mut self, ui: &mut egui::Ui, full: Rect, _t: &Theme) {
        let ctx = ui.ctx().clone();
        let (top, pinned, dark, glass) = (self.prefs.top, self.prefs.pinned, self.prefs.dark, self.prefs.glass);
        // a tree: switches are lit (accent line drawn along the branch) when on; buddies fold like a dropdown
        let item = |id: &str, label: &str, on: Option<bool>| menu::Item { id: id.into(), label: label.into(), on };
        let sections = vec![
            menu::Section { title: "Window".into(), items: vec![item("top", "Always on top", Some(top)), item("lock", "Lock position & size", Some(pinned))]  },
            menu::Section { title: "Appearance".into(), items: vec![item("dark", "Dark theme", Some(dark))] },
            menu::Section { title: "Notifications".into(), items: vec![item("preview", "Preview notification", None)] },
            menu::Section {
                title: format!("Study buddies · {}/{}", self.prefs.buddies.len(), buddy::MAX_BUDDIES),
                items: buddy::SPECIES.iter().map(|s| item(s, &buddy::label(s), Some(self.prefs.buddies.iter().any(|b| b == s)))).collect(),
            },
        ];
        let max_h = (full.height() - 44.0).max(96.0); // scrolls instead of overflowing a small window
        let mut clicked = None;
        let pos = Pos2::new(full.min.x + 6.0, (full.max.y - 30.0).max(full.min.y + 6.0));
        let resp = egui::Area::new(egui::Id::new("menu")).order(egui::Order::Foreground).pivot(Align2::LEFT_BOTTOM).fixed_pos(pos).show(&ctx, |ui| {
            menu::paper_frame().show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(max_h).auto_shrink([true, true]).show(ui, |ui| {
                    clicked = menu::branched(ui, egui::Id::new("branched"), &sections, &mut self.menu_st, 190.0);
                });
            });
        });
        let (mut top, mut pinned, mut dark, mut glass) = (top, pinned, dark, glass);
        let mut buddies = self.prefs.buddies.clone();
        let mut changed = false;
        if let Some(id) = clicked {
            changed = true;
            match id.as_str() {
                "top" => top = !top,
                "lock" => pinned = !pinned,
                "dark" => dark = !dark,
                "glass" => glass = !glass,
                "preview" => { self.preview_toast(); changed = false }
                s => {
                    if let Some(i) = buddies.iter().position(|b| b == s) { buddies.remove(i); }
                    else if buddies.len() < buddy::MAX_BUDDIES { buddies.push(s.into()) }
                }
            }
        }
        if changed {
            if top != self.prefs.top { ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if top { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal })) }
            let rebuild = buddies != self.prefs.buddies;
            self.prefs.top = top; self.prefs.pinned = pinned; self.prefs.dark = dark; self.prefs.buddies = buddies;
            if let Some(h) = self.hwnd { glass::apply(h, glass) }
            self.prefs.glass = glass;
            if rebuild { self.rebuild_buddies(&ctx) }
            self.touch();
        }
        // click outside closes (the gear and the menu itself don't count)
        let gear = Rect::from_min_size(Pos2::new(full.min.x + 3.2, full.max.y - 24.8), Vec2::splat(20.8));
        if ctx.input(|i| i.pointer.primary_pressed()) {
            if let Some(q) = ctx.input(|i| i.pointer.interact_pos()) { if !resp.response.rect.contains(q) && !gear.contains(q) { self.menu = false } }
        }
    }

    fn resize_handles(&self, ui: &egui::Ui, full: Rect) {
        let (e, c) = (5.0, 14.0);
        let ctx = ui.ctx();
        let zones: [(Rect, ResizeDirection, CursorIcon); 8] = [
            (Rect::from_min_max(Pos2::new(full.min.x + c, full.min.y), Pos2::new(full.max.x - c, full.min.y + e)), ResizeDirection::North, CursorIcon::ResizeVertical),
            (Rect::from_min_max(Pos2::new(full.min.x + c, full.max.y - e), Pos2::new(full.max.x - c, full.max.y)), ResizeDirection::South, CursorIcon::ResizeVertical),
            (Rect::from_min_max(Pos2::new(full.min.x, full.min.y + c), Pos2::new(full.min.x + e, full.max.y - c)), ResizeDirection::West, CursorIcon::ResizeHorizontal),
            (Rect::from_min_max(Pos2::new(full.max.x - e, full.min.y + c), Pos2::new(full.max.x, full.max.y - c)), ResizeDirection::East, CursorIcon::ResizeHorizontal),
            (Rect::from_min_size(full.min, Vec2::splat(c)), ResizeDirection::NorthWest, CursorIcon::ResizeNwSe),
            (Rect::from_min_size(Pos2::new(full.max.x - c, full.min.y), Vec2::splat(c)), ResizeDirection::NorthEast, CursorIcon::ResizeNeSw),
            (Rect::from_min_size(Pos2::new(full.min.x, full.max.y - c), Vec2::splat(c)), ResizeDirection::SouthWest, CursorIcon::ResizeNeSw),
            (Rect::from_min_size(Pos2::new(full.max.x - c, full.max.y - c), Vec2::splat(c)), ResizeDirection::SouthEast, CursorIcon::ResizeNwSe),
        ];
        for (i, (r, dir, cur)) in zones.into_iter().enumerate() {
            let resp = ui.interact(r, egui::Id::new(("resize", i)), Sense::drag());
            if resp.hovered() || resp.dragged() { ctx.set_cursor_icon(cur) }
            if resp.drag_started() { ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir)) }
        }
    }
}

// ---------------------------------------------------------------- helpers
fn tone(t: &Theme, left: i32) -> Color32 { if left >= 50 { t.good } else if left >= 20 { t.warn } else { t.bad } }
fn fmt_dt(ms: i64, f: &str) -> String { Local.timestamp_millis_opt(ms).unwrap().format(f).to_string() }
fn fmt_dur(ms: i64) -> String {
    let m = ms.max(0) / 60_000;
    if m >= 60 { format!("{}h {:02}m", m / 60, m % 60) } else { format!("{m}m") }
}
/// Lays out `text` at the largest size that fits (max_w x max_h), so the number scales with the window.
fn fit_text(p: &egui::Painter, text: &str, fam: FontFamily, max_w: f32, max_h: f32, col: Color32) -> Arc<egui::Galley> {
    let mut size = max_h.clamp(10.0, 160.0);
    let mut g = p.layout_no_wrap(text.into(), FontId::new(size, fam.clone()), col);
    if g.size().x > max_w { size = (size * max_w / g.size().x).max(8.0); g = p.layout_no_wrap(text.into(), FontId::new(size, fam), col) }
    g
}

fn main() -> eframe::Result {
    let prefs = Prefs::load();
    let mut vp = egui::ViewportBuilder::default()
        .with_title("Token Tracker")
        .with_decorations(false)
        .with_transparent(true)
        .with_taskbar(false)
        .with_icon({ let (rgba, width, height) = app_icon(64); egui::IconData { rgba, width, height } })
        .with_min_inner_size([MIN_W, if prefs.collapsed { COLLAPSED_H } else { MIN_H }])
        .with_inner_size([prefs.size.0, if prefs.collapsed { COLLAPSED_H } else { prefs.size.1 }]);
    if let Some((x, y)) = prefs.pos { vp = vp.with_position([x, y]) }
    if prefs.top { vp = vp.with_always_on_top() }
    eframe::run_native(
        "Token Tracker",
        eframe::NativeOptions { viewport: vp, renderer: eframe::Renderer::Glow, ..Default::default() },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
