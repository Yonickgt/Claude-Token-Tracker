//! Token engine: reads Claude Code / Codex logs and turns them into 5h-session + week numbers.
//! A Rust port of StudyList's shared/tokens.ts (counted = input + output + cache writes).
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Timelike};
use serde_json::Value;
use std::{collections::HashMap, fs, io::{BufRead, BufReader}, path::{Path, PathBuf}, sync::Arc, time::SystemTime};

const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Provider { Claude, Codex }

#[derive(Clone, Default)]
pub struct Row { pub ts: i64, pub model: Arc<str>, pub inp: f64, pub out: f64, pub cw: f64, pub cr: f64, pub tools: usize }
impl Row { fn tok(&self) -> f64 { self.inp + self.out + self.cw } }

#[derive(Clone, Copy, Default)]
struct Meter { ts: i64, s_pct: f64, s_reset: i64, w_pct: f64, w_reset: i64 }
#[derive(Clone, Copy, Default)]
struct Reading { m: Meter, s_tok: f64, w_tok: f64 }

#[derive(Clone, Default)]
pub struct Session { pub start: i64, pub end: i64, pub used: f64, pub limit: f64 }
#[derive(Clone, Default)]
pub struct Burn { pub per_hour: f64, pub eta_ms: Option<f64> }
/// One trend bar: tokens from the main model and from every other model (drawn blue).
#[derive(Clone, Default)]
pub struct Bar { pub main: f64, pub other: f64 }

#[derive(Clone, Default)]
pub struct Usage {
    pub logs_found: bool,
    pub calibrated: bool,
    pub session: Option<Session>,
    pub week_used: f64,
    pub week_limit: f64,
    pub cache_hit: f64,
    pub output_ratio: f64,
    pub cost_per_msg: f64,
    pub messages: usize,
    pub tools_per_msg: f64,
    pub burn: Option<Burn>,
    pub total_24h: f64,
    pub day_bars: Vec<Bar>,  // 24 hourly, oldest first
    pub week_bars: Vec<Bar>, // 7 daily, oldest first
    pub day_start: i64,      // epoch ms of the first hourly bar
    pub week_day0: i64,      // epoch ms of the first daily bar
}

// ---- pricing (USD / 1M tokens: in, out, cache write, cache read). First substring wins. ----
const PRICES: &[(&str, [f64; 4])] = &[
    ("fable", [10.0, 50.0, 12.5, 0.25]), ("mythos", [10.0, 50.0, 12.5, 0.25]),
    ("opus-5-5", [4.0, 20.0, 5.0, 0.2]), ("opus", [5.0, 25.0, 6.25, 0.5]),
    ("sonnet-5", [2.0, 10.0, 2.5, 0.2]), ("sonnet", [3.0, 15.0, 3.75, 0.3]),
    ("haiku", [1.0, 5.0, 1.25, 0.1]),
];
fn cost(r: &Row) -> f64 {
    PRICES.iter().find(|(k, _)| r.model.contains(k)).map_or(0.0, |(_, p)| {
        (r.inp * p[0] + r.out * p[1] + r.cw * p[2] + r.cr * p[3]) / 1e6
    })
}

thread_local! { static MODELS: std::cell::RefCell<HashMap<String, Arc<str>>> = Default::default(); }
/// One shared allocation per model name instead of one String per message.
fn intern(s: &str) -> Arc<str> {
    MODELS.with(|m| m.borrow_mut().entry(s.to_string()).or_insert_with(|| Arc::from(s)).clone())
}
fn hash_key(s: &str) -> u64 { use std::hash::{Hash, Hasher}; let mut h = std::collections::hash_map::DefaultHasher::new(); s.hash(&mut h); h.finish() }

fn num(v: &Value) -> f64 { v.as_f64().filter(|n| n.is_finite()).unwrap_or(0.0) }
fn ts_of(v: &Value) -> Option<i64> {
    DateTime::parse_from_rfc3339(v.as_str()?).ok().map(|d| d.timestamp_millis())
}

// ---- parsing ----
type Entry = (Option<u64>, Row);

/// Only the last 8 days matter (the week window + 7-day trend), so older lines are skipped unparsed.
fn parse_claude_line(line: &str, cutoff: i64, out: &mut Vec<Entry>) {
    if !line.contains("\"usage\"") { return }
    let Ok(d) = serde_json::from_str::<Value>(line) else { return };
    let (m, u) = (&d["message"], &d["message"]["usage"]);
    if !m.is_object() || !u.is_object() { return }
    let Some(ts) = ts_of(&d["timestamp"]).filter(|t| *t >= cutoff) else { return };
    let model = m["model"].as_str().unwrap_or("?");
    let (inp, o, cw, cr) = (num(&u["input_tokens"]), num(&u["output_tokens"]),
        num(&u["cache_creation_input_tokens"]), num(&u["cache_read_input_tokens"]));
    if model.starts_with('<') || inp + o + cw + cr == 0.0 { return }
    let model = intern(model);
    let tools = m["content"].as_array().map_or(0, |c| c.iter().filter(|b| b["type"] == "tool_use").count());
    let key = m["id"].as_str().filter(|s| !s.is_empty())
        .map(|id| hash_key(&format!("{id}|{}", d["requestId"].as_str().unwrap_or(""))));
    out.push((key, Row { ts, model, inp, out: o, cw, cr, tools }));
}

/// Codex carries its session id + model from line to line, so that state survives between reads.
#[derive(Default, Clone)]
struct CodexState { session: String, model: String }

fn parse_codex_line(line: &str, cutoff: i64, st: &mut CodexState, entries: &mut Vec<Entry>, meters: &mut Vec<Meter>) {
    if !(line.contains("token_count") || line.contains("session_meta") || line.contains("turn_context")) { return }
    let Ok(d) = serde_json::from_str::<Value>(line) else { return };
    let p = &d["payload"];
    let Some(ts) = ts_of(&d["timestamp"]) else { return };
    if !p.is_object() { return }
    match d["type"].as_str() {
        Some("session_meta") => {
            if let Some(id) = p["id"].as_str() { st.session = id.into() }
            if let Some(m) = p["base_instructions"]["provenance"]["model"].as_str().filter(|s| !s.is_empty()) { st.model = m.into() }
        }
        Some("turn_context") => if let Some(m) = p["model"].as_str().filter(|s| !s.is_empty()) { st.model = m.into() },
        Some("event_msg") if p["type"] == "token_count" => {
            let last = &p["info"]["last_token_usage"];
            if last.is_object() {
                let total = num(&p["info"]["total_token_usage"]["total_tokens"]);
                let (cached, cw, o) = (num(&last["cached_input_tokens"]), num(&last["cache_write_input_tokens"]), num(&last["output_tokens"]));
                let mut inp = (num(&last["input_tokens"]) - cached - cw).max(0.0);
                if inp + o + cw + cached == 0.0 { inp = num(&last["total_tokens"]) }
                if inp + o + cw + cached > 0.0 && ts >= cutoff {
                    let key = (total > 0.0).then(|| hash_key(&format!("{}|{total}", st.session)));
                    let model = if st.model.is_empty() { "?" } else { &st.model };
                    entries.push((key, Row { ts, model: intern(model), inp, out: o, cw, cr: cached, tools: 0 }));
                }
            }
            let rl = &p["rate_limits"];
            if ts >= cutoff && rl["limit_id"] == "codex" && rl["primary"].is_object() && rl["secondary"].is_object() {
                meters.push(Meter { ts, s_pct: num(&rl["primary"]["used_percent"]), s_reset: (num(&rl["primary"]["resets_at"]) * 1000.0) as i64,
                    w_pct: num(&rl["secondary"]["used_percent"]), w_reset: (num(&rl["secondary"]["resets_at"]) * 1000.0) as i64 });
            }
        }
        _ => {}
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() { walk(&p, out) } else if p.extension().is_some_and(|x| x == "jsonl") { out.push(p) }
    }
}


/// What we know about one log: bytes already consumed (only whole lines), and what they held.
#[derive(Default)]
struct FileState { offset: u64, entries: Vec<Entry>, meters: Vec<Meter>, codex: CodexState }
type FileCache = HashMap<PathBuf, FileState>;

/// Feeds every complete new line after `offset` to `f`; returns the new offset. A half-written last line waits.
fn read_new_lines(path: &Path, mut offset: u64, mut f: impl FnMut(&str)) -> u64 {
    use std::io::{Seek, SeekFrom};
    let Ok(mut file) = fs::File::open(path) else { return offset };
    if file.seek(SeekFrom::Start(offset)).is_err() { return offset }
    let (mut r, mut buf) = (BufReader::new(file), String::new());
    loop {
        buf.clear();
        match r.read_line(&mut buf) {
            Ok(n) if n > 0 && buf.ends_with('\n') => { offset += n as u64; f(&buf) }
            _ => break, // EOF, a partial line, or invalid UTF-8
        }
    }
    offset
}

/// Reads every log, parsing only what was appended since the last pass. Returns deduped rows.
fn read_rows(p: Provider, cache: &mut FileCache) -> (Vec<Row>, Vec<Meter>, bool) {
    let home = PathBuf::from(std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default());
    let root = home.join(if p == Provider::Claude { ".claude/projects" } else { ".codex/sessions" });
    let mut paths = vec![];
    walk(&root, &mut paths);
    paths.sort();
    let cutoff = now_ms() - 8 * DAY;
    let (mut seen, mut rows, mut meters): (HashMap<u64, usize>, Vec<Row>, Vec<Meter>) = (HashMap::new(), vec![], vec![]);
    let mut keep = vec![];
    for path in &paths {
        let Ok(md) = fs::metadata(path) else { continue };
        // a log can't hold a message newer than its last write: old files are never even opened
        let modified = md.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if DateTime::<chrono::Utc>::from(modified).timestamp_millis() < cutoff { continue }
        keep.push(path.clone());
        let st = cache.entry(path.clone()).or_default();
        if md.len() < st.offset { *st = FileState::default() } // truncated or replaced: start over
        if md.len() > st.offset {
            let FileState { entries, meters, codex, .. } = st;
            let new_off = read_new_lines(path, st.offset, |line| {
                if p == Provider::Codex { parse_codex_line(line, cutoff, codex, entries, meters) } else { parse_claude_line(line, cutoff, entries) }
            });
            st.offset = new_off;
        }
        meters.extend_from_slice(&st.meters);
        for (key, row) in &st.entries {
            if let Some(k) = key {
                // the same message is logged once per content block: count its tokens once, pool its tool calls
                if let Some(&i) = seen.get(k) {
                    // streamed message: output grows across lines, so keep the biggest reading
                    let tools = rows[i].tools + row.tools;
                    if row.tok() > rows[i].tok() { rows[i] = row.clone() }
                    rows[i].tools = tools; continue
                }
                seen.insert(*k, rows.len());
            }
            rows.push(row.clone());
        }
    }
    cache.retain(|k, _| keep.contains(k));
    rows.sort_by_key(|r| r.ts);
    (rows, meters, !paths.is_empty())
}

// ---- windows ----
struct Block { start: i64, end: i64, tok: f64 }

fn build_blocks(rows: &[Row], pinned: Option<(i64, i64)>) -> Vec<Block> {
    let mut out: Vec<Block> = vec![];
    for r in rows {
        if let Some((a, b)) = pinned.filter(|(a, b)| r.ts >= *a && r.ts < *b) {
            match out.last_mut() { Some(l) if l.start == a => l.tok += r.tok(), _ => out.push(Block { start: a, end: b, tok: r.tok() }) }
        } else if let Some(l) = out.last_mut().filter(|l| r.ts < l.end) {
            l.tok += r.tok();
        } else {
            let start = r.ts.div_euclid(HOUR) * HOUR;
            out.push(Block { start, end: start + 5 * HOUR, tok: r.tok() });
        }
    }
    out
}

/// Start of the weekly window containing `now`: latest local `week_day` (Mon=0) at `week_hour`.
fn week_start(now: i64, week_day: i64, week_hour: u32) -> i64 {
    let n = Local.timestamp_millis_opt(now).unwrap();
    let mut d = n.date_naive().and_hms_opt(week_hour, 0, 0).unwrap();
    d -= Duration::days((n.weekday().num_days_from_monday() as i64 - week_day + 7) % 7);
    let mut t = Local.from_local_datetime(&d).earliest().unwrap().timestamp_millis();
    if t > now { t -= 7 * DAY }
    t
}

fn window_tokens(rows: &[Row], since: i64, until: i64) -> f64 {
    rows.iter().filter(|r| r.ts >= since && r.ts <= until).map(Row::tok).sum()
}

fn fit_limit(rs: &[Reading], weekly: bool, fallback: f64) -> f64 {
    let best = rs.iter().rev().take(300).filter_map(|r| {
        let (t, pct) = if weekly { (r.w_tok, r.m.w_pct) } else { (r.s_tok, r.m.s_pct) };
        (t > 0.0 && pct >= 10.0).then(|| t * 100.0 / pct)
    }).fold(0.0, f64::max);
    if best > 0.0 { best.round().max(1.0) } else { fallback }
}

// ---- live meter (Claude only): the real /usage percentages ----
fn readings_path() -> PathBuf {
    let base = std::env::var("APPDATA").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    PathBuf::from(base).join("ClaudeTokenWidget").join("readings.jsonl")
}
fn load_readings() -> Vec<Reading> {
    fs::read_to_string(readings_path()).unwrap_or_default().lines().filter_map(|l| {
        let v: Value = serde_json::from_str(l).ok()?;
        Some(Reading { m: Meter { ts: v["ts"].as_i64()?, s_pct: num(&v["sPct"]), s_reset: v["sReset"].as_i64()?, w_pct: num(&v["wPct"]), w_reset: v["wReset"].as_i64()? },
            s_tok: num(&v["sTok"]), w_tok: num(&v["wTok"]) })
    }).collect()
}
fn fetch_meter() -> Option<Meter> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()?;
    let creds: Value = serde_json::from_str(&fs::read_to_string(PathBuf::from(home).join(".claude/.credentials.json")).ok()?).ok()?;
    let token = creds["claudeAiOauth"]["accessToken"].as_str()?;
    let agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(10))).build().new_agent();
    let body: Value = agent.get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", &format!("Bearer {token}")).header("anthropic-beta", "oauth-2025-04-20")
        .call().ok()?.body_mut().read_json().ok()?;
    let win = |v: &Value| -> Option<(f64, i64)> {
        if v.is_null() { return Some((0.0, 0)) }
        Some((v["utilization"].as_f64()?, DateTime::parse_from_rfc3339(v["resets_at"].as_str()?).ok()?.timestamp_millis()))
    };
    let (s, w) = (win(&body["five_hour"])?, win(&body["seven_day"])?);
    Some(Meter { ts: now_ms(), s_pct: s.0, s_reset: s.1, w_pct: w.0, w_reset: w.1 })
}
pub fn now_ms() -> i64 { Local::now().timestamp_millis() }

/// Owns the per-file cache and reading history; `compute` is called from the worker thread.
pub struct Engine { cache: FileCache, readings: Vec<Reading>, last_poll: i64 }
impl Engine {
    pub fn new() -> Self { Self { cache: HashMap::new(), readings: load_readings(), last_poll: 0 } }

    pub fn compute(&mut self, p: Provider) -> Usage {
        let (rows, meters, logs_found) = read_rows(p, &mut self.cache);
        let now = now_ms();
        let readings: Vec<Reading> = if p == Provider::Codex {
            let mut ms = meters; ms.sort_by_key(|m| m.ts);
            let mut out: Vec<Reading> = vec![];
            for m in ms {
                if out.last().is_some_and(|l| l.m.s_pct == m.s_pct && l.m.w_pct == m.w_pct && l.m.s_reset == m.s_reset) { continue }
                out.push(Reading { m, s_tok: window_tokens(&rows, m.s_reset - 5 * HOUR, m.ts), w_tok: window_tokens(&rows, m.w_reset - 168 * HOUR, m.ts) });
            }
            out
        } else {
            if now - self.last_poll >= 30_000 {
                self.last_poll = now;
                if let Some(m) = fetch_meter() { self.push_reading(m, &rows) }
            }
            self.readings.clone()
        };
        build_usage(&rows, &readings, now, logs_found)
    }

    fn push_reading(&mut self, m: Meter, rows: &[Row]) {
        let r = Reading { m, s_tok: if m.s_reset > 0 { window_tokens(rows, m.s_reset - 5 * HOUR, i64::MAX) } else { 0.0 },
            w_tok: if m.w_reset > 0 { window_tokens(rows, m.w_reset - 168 * HOUR, i64::MAX) } else { 0.0 } };
        if let Some(l) = self.readings.last_mut().filter(|l| l.m.s_pct == m.s_pct && l.m.w_pct == m.w_pct && m.ts - l.m.ts < 600_000) {
            l.m.ts = m.ts; return;
        }
        self.readings.push(r);
        let line = format!("{{\"ts\":{},\"sPct\":{},\"sReset\":{},\"sTok\":{},\"wPct\":{},\"wReset\":{},\"wTok\":{}}}\n", m.ts, m.s_pct, m.s_reset, r.s_tok, m.w_pct, m.w_reset, r.w_tok);
        let path = readings_path();
        if let Some(dir) = path.parent() { let _ = fs::create_dir_all(dir); }
        use std::io::Write;
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) { let _ = f.write_all(line.as_bytes()); }
    }
}

fn build_usage(rows: &[Row], readings: &[Reading], now: i64, logs_found: bool) -> Usage {
    let (mut limit5, mut limit_w) = (5_000_000.0, 50_000_000.0);
    let (mut s_end, mut w_day, mut w_hour) = (0i64, 4i64, 9u32);
    let (mut live_s, mut live_w) = (None, None);
    let calibrated = if let Some(l) = readings.last() {
        limit5 = fit_limit(readings, false, limit5);
        limit_w = fit_limit(readings, true, limit_w);
        s_end = l.m.s_reset;
        if l.m.w_reset > now {
            let d = Local.timestamp_millis_opt(l.m.w_reset).unwrap();
            w_day = d.weekday().num_days_from_monday() as i64; w_hour = d.hour();
        }
        live_s = Some(if l.m.s_reset > now { l.m.s_pct } else { 0.0 });
        live_w = Some(if l.m.w_reset > now { l.m.w_pct } else { 0.0 });
        true
    } else { false };
    let shown = |used: f64, limit: f64, pct: Option<f64>| pct.map_or(used, |p| (p / 100.0 * limit).round());

    let blocks = build_blocks(rows, (s_end > 0).then(|| (s_end - 5 * HOUR, s_end)));
    let ws = week_start(now, w_day, w_hour);
    let wk: Vec<&Row> = rows.iter().filter(|r| r.ts >= ws).collect();
    let sum = |f: &dyn Fn(&Row) -> f64| wk.iter().map(|r| f(r)).sum::<f64>();
    let (inp, cw, cr, out) = (sum(&|r| r.inp), sum(&|r| r.cw), sum(&|r| r.cr), sum(&|r| r.out));
    let ratio = |a: f64, b: f64| if b > 0.0 { a / b } else { 0.0 };

    let live = blocks.last().filter(|b| now < b.end);
    let session = live.map(|b| Session { start: b.start, end: b.end, limit: limit5,
        used: shown(b.tok, limit5, live_s) });
    let burn = session.as_ref().map(|s| {
        let per_hour = s.used * HOUR as f64 / ((now - s.start).max(15 * 60_000)) as f64;
        Burn { per_hour: per_hour.round(), eta_ms: (per_hour > 0.0 && s.used < s.limit).then(|| (s.limit - s.used) / per_hour * HOUR as f64) }
    });

    // trend: the busiest model (in the window) is "main", everything else stacks on top in blue
    let cur_hour = now.div_euclid(HOUR) * HOUR;
    let day_start = cur_hour - 23 * HOUR;
    let week_day0 = Local.timestamp_millis_opt(now).unwrap().date_naive().and_hms_opt(0, 0, 0)
        .and_then(|d| Local.from_local_datetime(&d).earliest()).map_or(now, |d| d.timestamp_millis()) - 6 * DAY;
    let mut by_model: HashMap<&str, f64> = HashMap::new();
    for r in rows.iter().filter(|r| r.ts >= week_day0) { *by_model.entry(&*r.model).or_default() += r.tok() }
    let main = by_model.into_iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|(m, _)| m.to_string()).unwrap_or_default();
    let (mut day_bars, mut week_bars) = (vec![Bar::default(); 24], vec![Bar::default(); 7]);
    for r in rows {
        let add = |b: &mut Bar| if &*r.model == main { b.main += r.tok() } else { b.other += r.tok() };
        if r.ts >= day_start { if let Some(b) = day_bars.get_mut(((r.ts - day_start) / HOUR) as usize) { add(b) } }
        if r.ts >= week_day0 { if let Some(b) = week_bars.get_mut(((r.ts - week_day0) / DAY) as usize) { add(b) } }
    }
    Usage {
        logs_found, calibrated, session,
        week_used: shown(sum(&Row::tok), limit_w, live_w), week_limit: limit_w,
        cache_hit: ratio(cr, cr + cw + inp), output_ratio: ratio(out, inp + cw + cr),
        cost_per_msg: ratio(sum(&cost), wk.len() as f64), messages: wk.len(),
        tools_per_msg: ratio(sum(&|r| r.tools as f64), wk.len() as f64),
        burn, total_24h: day_bars.iter().map(|b| b.main + b.other).sum(),
        day_bars, week_bars, day_start, week_day0,
    }
}

// ---- formatting helpers shared with the UI ----
pub fn pct_left(used: f64, limit: f64) -> i32 { ((1.0 - used / limit) * 100.0).round().max(0.0) as i32 }
pub fn fmt_tok(n: f64) -> String {
    if n >= 1e9 { format!("{:.2}B", n / 1e9) } else if n >= 1e6 { format!("{:.2}M", n / 1e6) }
    else if n >= 1e3 { format!("{:.1}K", n / 1e3) } else { format!("{}", n.round()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_and_limit() {
        let r = |ts: i64| Row { ts, model: "claude-opus-4".into(), inp: 10.0, out: 5.0, cw: 0.0, cr: 0.0, tools: 1 };
        let rows = vec![r(HOUR + 5), r(2 * HOUR), r(7 * HOUR)];
        let b = build_blocks(&rows, None);
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].start, b[0].end), (HOUR, 6 * HOUR));
        let rd = Reading { m: Meter { s_pct: 50.0, ..Default::default() }, s_tok: 1000.0, w_tok: 0.0 };
        assert_eq!(fit_limit(&[rd], false, 1.0), 2000.0);
        assert_eq!(pct_left(25.0, 100.0), 75);
        assert_eq!(fmt_tok(2_890_000.0), "2.89M");
    }
}
