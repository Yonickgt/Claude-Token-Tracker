/* Usage model + the ONE set of selectors every number comes from. Shapes mirror server.py /api/usage. */

export type Config = { limit5h: number; limitWeek: number; weekDay: number; weekHour: number; sessionEnd: number };
export type Session = {
  id: string; start: number; end: number; active: boolean; messages: number; tokens: number;
  models: Record<string, number>; projects: Record<string, number>;
};
export type Usage = {
  now: number;
  config: Config;
  session: (Session & { used: number; limit: number }) | null;
  week: {
    start: number; end: number; used: number; limit: number; models: Record<string, number>; projects: Record<string, number>;
    input: number; output: number; cacheWrite: number; cacheRead: number; cost: number;
  };
  sessions: Session[];
  hourly: [number, string, number][]; // [hour start ms, model, tokens]
  flow: [string, string, string, number][]; // [project, model, kind, tokens] for the week
  totals: { all: number; cacheRead: number; messages: number; cost: number };
};
export type LogRow = { ts: number; model: string; project: string; input: number; output: number; cacheWrite: number; cacheRead: number };

/* ---- Model families: the chart categories. Dark = the one that matters most; fixed forever (DESIGN_STYLE §2.4). */
export const FAMILIES = ["fable", "opus", "sonnet", "haiku", "other"] as const;
export type Family = (typeof FAMILIES)[number];
export const FAMILY_SHADE: Record<Family, string> = {
  fable: "var(--b-900)", opus: "var(--b-700)", sonnet: "var(--b-500)", haiku: "var(--b-300)", other: "var(--b-100)",
};
const FAMILY_LABEL: Record<Family, string> = { fable: "Fable", opus: "Opus", sonnet: "Sonnet", haiku: "Haiku", other: "Other" };
export const familyLabel = (f: Family) => FAMILY_LABEL[f];
export const familyOf = (model: string): Family => (FAMILIES.find((f) => f !== "other" && model.includes(f)) ?? (model.includes("mythos") ? "fable" : "other"));
/** "claude-opus-5-5" → "Opus 5.5"; unknown ids pass through. */
export const modelLabel = (m: string) => {
  const x = m.replace(/^claude-/, "").replace(/-\d{8}$/, "").match(/^([a-z]+)-(\d+)(?:-(\d+))?$/);
  return x ? `${x[1][0].toUpperCase()}${x[1].slice(1)} ${x[2]}${x[3] ? `.${x[3]}` : ""}` : m;
};

/* ---- Limit levels. NEAR is the threshold where attention starts (the HITL_THRESHOLD analogue). */
export const NEAR = 0.8;
export type Level = "ok" | "near" | "over";
export const levelOf = (used: number, limit: number): Level => (used >= limit ? "over" : used >= limit * NEAR ? "near" : "ok");
export const LEVEL_WORD: Record<Level, string> = { ok: "Under limit", near: "Near limit", over: "Over limit" };

export const pctLeft = (used: number, limit: number) => Math.max(0, Math.round((1 - used / limit) * 100));
export const pctUsed = (used: number, limit: number) => Math.round((used / limit) * 100);

export function fmtTok(n: number) {
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(2)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return String(Math.round(n));
}
export function fmtLeft(ms: number) {
  const s = Math.max(0, Math.floor(ms / 1000));
  const d = Math.floor(s / 86400), h = Math.floor((s % 86400) / 3600), m = Math.floor((s % 3600) / 60);
  return d ? `${d}d ${h}h` : `${h}h ${String(m).padStart(2, "0")}m`;
}
const p2 = (n: number) => String(n).padStart(2, "0");
export const hhmm = (ms: number) => { const d = new Date(ms); return `${p2(d.getHours())}:${p2(d.getMinutes())}`; };
export const ddmmyy = (ms: number) => { const d = new Date(ms); return `${p2(d.getDate())}/${p2(d.getMonth() + 1)}/${p2(d.getFullYear() % 100)}`; };
/** "Tue 14:00 – 19:00": how a session reads everywhere. */
export const sessionName = (s: Pick<Session, "start" | "end">) =>
  `${new Date(s.start).toLocaleDateString("en", { weekday: "short" })} ${hhmm(s.start)} – ${hhmm(s.end)}`;
export const topKey = (m: Record<string, number>) => Object.entries(m).sort((a, b) => b[1] - a[1])[0]?.[0] ?? null;

/** Everything a person might type to find a session. One haystack, so the table and ⌘K agree. */
export const searchText = (s: Session) =>
  [sessionName(s), ddmmyy(s.start), ...Object.keys(s.projects), ...Object.keys(s.models).map(modelLabel)].join(" ").toLowerCase();

export const familyTotals = (models: Record<string, number>) => {
  const out = Object.fromEntries(FAMILIES.map((f) => [f, 0])) as Record<Family, number>;
  Object.entries(models).forEach(([m, n]) => (out[familyOf(m)] += n));
  return out;
};

/** Linear pace: where a window ends up if the rate so far holds. ponytail: ignores sleep/work rhythm; upgrade to a per-hour profile. */
export function projected(used: number, start: number, end: number, now: number) {
  const elapsed = Math.max(now - start, 15 * 60_000); // under 15 min the rate is noise
  return Math.round(used * Math.min((end - start) / elapsed, 20));
}

export function summarise(u: Usage) {
  const lim = u.config.limit5h;
  const sessions = u.sessions;
  const count = (f: (s: Session) => boolean) => sessions.filter(f).length;
  return {
    total: sessions.length,
    active: count((s) => s.active),
    near: count((s) => levelOf(s.tokens, lim) === "near"),
    over: count((s) => levelOf(s.tokens, lim) === "over"),
    ok: count((s) => levelOf(s.tokens, lim) === "ok"),
    sessionLeft: u.session ? pctLeft(u.session.used, u.session.limit) : 100,
    weekLeft: pctLeft(u.week.used, u.week.limit),
    sessionLevel: u.session ? levelOf(u.session.used, u.session.limit) : ("ok" as Level),
    weekLevel: levelOf(u.week.used, u.week.limit),
  };
}
export type Summary = ReturnType<typeof summarise>;
