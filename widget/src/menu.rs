//! Settings panel in the "branched menu" style (React Bits BranchedMenu): a left rail, folding section
//! headers, and child rows hanging off a trunk on curved branches. An accent line draws along the branch
//! of every item that is switched on, and a marker glides beside the section last touched.
//! Painted by hand (no widgets), on the paper-coloured popover from StudyList's dropdowns.
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

pub const PAPER: Color32 = Color32::from_rgb(0xea, 0xdf, 0xd0);
pub const INK: Color32 = Color32::from_rgb(0x2d, 0x2d, 0x2d);
const BROWN: Color32 = Color32::from_rgb(0x87, 0x6e, 0x55);
const LINE: Color32 = Color32::from_rgb(0xc9, 0xb9, 0xa2);

// geometry, in points (the component's defaults, with a slightly tighter row)
const NAV_PAD: f32 = 14.0; // rail -> content
const PAD: f32 = 6.0; // padding above/below the children
const ROW: f32 = 30.0;
const INDENT: f32 = 40.0; // where child text starts
const TRUNK: f32 = 14.0; // trunk x, inside the tree
const RADIUS: f32 = 10.0;
const LW: f32 = 1.5;
const HEAD_H: f32 = 34.0;

pub struct Item {
    pub id: String,
    pub label: String,
    /// `Some(on)` for a switch, `None` for an action.
    pub on: Option<bool>,
}
pub struct Section { pub title: String, pub items: Vec<Item> }
pub struct MenuState { pub open: Vec<bool>, pub last: Option<usize> }
impl MenuState { pub fn new() -> Self { MenuState { open: vec![true, false, false, false], last: Some(0) } } }

fn ease_out(x: f32) -> f32 { 1.0 - (1.0 - x).powi(3) }
fn bold() -> FontFamily { FontFamily::Name("display".into()) }

/// The popover every dropdown sits on.
pub fn paper_frame() -> egui::Frame {
    egui::Frame::NONE.fill(PAPER).corner_radius(18.0).inner_margin(8.0)
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(90)))
        .shadow(egui::Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(70) })
}

/// One row of a dropdown list: the chosen row is a dark pill, the rest are brown text.
pub fn paper_row(ui: &mut egui::Ui, label: &str, on: bool, w: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 30.0), Sense::click());
    let p = ui.painter();
    if on { p.rect_filled(rect, 12.0, INK); } else if resp.hovered() { p.rect_filled(rect, 12.0, Color32::from_black_alpha(14)); }
    p.text(Pos2::new(rect.min.x + 12.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::new(13.0, bold()), if on { Color32::WHITE } else { BROWN });
    resp
}

/// Points of a branch: down the trunk from `top_y`, round the corner, along to `end_x`.
fn branch_pts(trunk_x: f32, top_y: f32, row_y: f32, end_x: f32) -> Vec<Pos2> {
    let r = RADIUS.min(ROW / 2.0 - 2.0);
    let mut v = vec![Pos2::new(trunk_x, top_y), Pos2::new(trunk_x, row_y - r)];
    let c = Pos2::new(trunk_x + r, row_y - r); // arc centre
    for i in 1..=8 {
        let a = std::f32::consts::PI - (i as f32 / 8.0) * std::f32::consts::FRAC_PI_2;
        v.push(Pos2::new(c.x + r * a.cos(), c.y + r * a.sin()));
    }
    v.push(Pos2::new(end_x, row_y));
    v
}

/// Strokes the first `frac` (by length) of a polyline: the accent "draws itself" along the branch.
fn draw_path(p: &egui::Painter, pts: &[Pos2], frac: f32, stroke: Stroke) {
    let mut left = pts.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>() * frac;
    let mut cur = vec![pts[0]];
    for w in pts.windows(2) {
        let d = w[0].distance(w[1]);
        if left <= 0.0 { break }
        if left >= d { cur.push(w[1]); left -= d } else { cur.push(w[0] + (w[1] - w[0]) * (left / d)); break }
    }
    if cur.len() > 1 { p.add(Shape::line(cur, stroke)); }
}

/// Draws the tree; returns the id of the item clicked this frame, if any.
pub fn branched(ui: &mut egui::Ui, id: egui::Id, sections: &[Section], st: &mut MenuState, w: f32) -> Option<String> {
    let ctx = ui.ctx().clone();
    st.open.resize(sections.len(), false);
    let origin = ui.cursor().min;
    let mut clicked = None;
    let mut marker_y = None;

    for (si, sec) in sections.iter().enumerate() {
        // header: click folds / unfolds
        let (hr, hresp) = ui.allocate_exact_size(Vec2::new(w, HEAD_H), Sense::click());
        let col = if st.open[si] || hresp.hovered() { INK } else { INK.gamma_multiply(0.55) };
        ui.painter().text(Pos2::new(hr.min.x + NAV_PAD, hr.center().y), Align2::LEFT_CENTER, &sec.title, FontId::new(14.0, bold()), col);
        if hresp.clicked() {
            // accordion: a small window has room for one open section at a time
            let opening = !st.open[si];
            st.open.iter_mut().for_each(|o| *o = false);
            st.open[si] = opening;
            if opening { st.last = Some(si); } // the marker follows the section you opened
        }
        let open_t = ease_out(ctx.animate_bool_with_time(id.with(("open", si)), st.open[si], 0.3));
        if st.last == Some(si) && st.open[si] { marker_y = Some(hr.center().y) }
        if open_t < 0.01 { continue }

        // body: folds by clipping, content stays pinned to the top like the CSS grid-rows trick
        let full_h = PAD * 2.0 + sec.items.len() as f32 * ROW;
        let (body, _) = ui.allocate_exact_size(Vec2::new(w, full_h * open_t), Sense::hover());
        let p = ui.painter().with_clip_rect(body);
        let (ox, oy) = (body.min.x + NAV_PAD, body.min.y);
        let (trunk_x, end_x) = (ox + TRUNK, ox + INDENT - 8.0);
        let row_y = |k: usize| oy + PAD + k as f32 * ROW + ROW / 2.0;
        let r = RADIUS.min(ROW / 2.0 - 2.0);
        let base = Stroke::new(LW, LINE.gamma_multiply(open_t));
        let last = sec.items.len() - 1;
        p.line_segment([Pos2::new(trunk_x, oy), Pos2::new(trunk_x, row_y(last) - r)], base);
        for k in 0..sec.items.len() {
            let pts = branch_pts(trunk_x, row_y(k) - r - 1.0, row_y(k), end_x);
            draw_path(&p, &pts[1..], 1.0, base);
        }

        for (k, item) in sec.items.iter().enumerate() {
            let row = Rect::from_min_size(Pos2::new(ox, oy + PAD + k as f32 * ROW), Vec2::new(w - NAV_PAD, ROW));
            let resp = ui.interact(row.intersect(body), id.with(("row", si, k)), Sense::click());
            let on = item.on == Some(true);
            let prog = ease_out(ctx.animate_bool_with_time(id.with(("on", si, k)), on, 0.4));
            if prog > 0.0 {
                let pts = branch_pts(trunk_x, oy, row_y(k), end_x);
                draw_path(&p, &pts, prog, Stroke::new(LW, INK));
            }
            let (col, font) = if on {
                (INK, FontId::new(13.0, bold()))
            } else {
                (if resp.hovered() { INK } else { INK.gamma_multiply(0.55) }, FontId::new(13.0, FontFamily::Proportional))
            };
            p.text(Pos2::new(ox + INDENT, row_y(k)), Align2::LEFT_CENTER, &item.label, font, col);
            if resp.clicked() && open_t > 0.95 { st.last = Some(si); clicked = Some(item.id.clone()); }
        }
    }

    // rail (fades out toward the bottom) and the marker that glides to the section last touched
    let bottom = ui.cursor().min.y;
    let x = origin.x + 1.0;
    let mut m = egui::Mesh::default();
    let h = (bottom - origin.y - 8.0).max(1.0);
    for (f, a) in [(0.0, 1.0), (0.55, 1.0), (1.0, 0.0)] {
        let c = LINE.gamma_multiply(a);
        let y = origin.y + 8.0 + f * h;
        m.colored_vertex(Pos2::new(x - 1.0, y), c);
        m.colored_vertex(Pos2::new(x + 1.0, y), c);
    }
    for i in 0..2u32 { let b = i * 2; m.add_triangle(b, b + 1, b + 2); m.add_triangle(b + 1, b + 3, b + 2); }
    ui.painter().add(Shape::mesh(m));
    let shown = ctx.animate_bool_with_time(id.with("marker-on"), marker_y.is_some(), 0.15);
    if let Some(y) = marker_y {
        let y = ctx.animate_value_with_time(id.with("marker-y"), y, 0.22);
        ui.painter().rect_filled(Rect::from_min_size(Pos2::new(x - 1.0, y - 8.0), Vec2::new(2.0, 16.0)), 1.0, INK.gamma_multiply(shown));
    }
    clicked
}
