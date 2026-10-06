"use client";

import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import dynamic from "next/dynamic";
import { useUsage } from "@/lib/app-state";
import { FAMILIES, FAMILY_SHADE, familyLabel, familyOf, fmtLeft, fmtTok, modelLabel, projected } from "@/lib/usage";
import { COUNT_DELAY, CountUp, DemoChip, Empty, useNow } from "@/components/ui";
import { cn } from "@/lib/utils";
import JellyRadio from "@/components/JellyRadio";

/** True once `threshold` of the element has been on screen. Stays true, so scrolling away never resets a chart. */
export function useInView<T extends HTMLElement>(threshold: number) {
  const ref = useRef<T>(null);
  const [seen, setSeen] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const io = new IntersectionObserver(([e]) => e.isIntersecting && (setSeen(true), io.disconnect()), { threshold });
    io.observe(el);
    return () => io.disconnect();
  }, [threshold]);
  return [ref, seen] as const;
}

export function CardHead({ title, right }: { title: string; right?: React.ReactNode }) {
  return (
    <div className="card-head">
      <h2 className="text-[18px] font-semibold leading-6 tracking-[-0.01em]">{title}</h2>
      <div className="flex items-center gap-2">{right}</div>
    </div>
  );
}

/* ---- Week left: the headline for the longer window. */
export function WeekLeft() {
  const { usage, summary } = useUsage();
  const now = useNow(30_000);
  const w = usage!.week;
  return (
    <section className="card enter p-6" aria-label="Week left">
      <span className="text-[14px] font-bold uppercase leading-[18px] tracking-[0.06em] text-ink-2">Week left</span>
      <div className="mt-3 text-[40px] font-semibold leading-[44px] tracking-[-0.03em]" aria-live="polite">
        <CountUp to={summary!.weekLeft} suffix="%" />
      </div>
      <p className="mt-2 text-[13px] text-ink-2">
        <span className="num">{fmtTok(Math.max(0, w.limit - w.used))}</span> of <span className="num">{fmtTok(w.limit)}</span> tokens left.
        Resets in <span className="num">{fmtLeft(w.end - now)}</span>.
      </p>
    </section>
  );
}

// Hover-only donut: its recharts chunk loads with the charts, never with the first paint of these widgets.
const DayDonut = dynamic(() => import("./charts").then((m) => m.DayDonut), { ssr: false });

/* ---- API-equivalent cost. Honesty label on the card: this is a list-price estimate, not a bill. */
export function CostEstimate() {
  const { usage } = useUsage();
  const u = usage!;
  return (
    <section className="card enter flex flex-col p-6" aria-label="API-equivalent cost">
      <div className="flex items-start justify-between gap-3">
        <span className="text-[14px] font-bold uppercase leading-[18px] tracking-[0.06em] text-ink-2">API-equivalent cost</span>
        <DemoChip text="Estimate" title="Token counts times published list prices. Not what your plan charges." />
      </div>
      <div className="mt-3 flex flex-wrap items-baseline gap-x-2 text-[30px] font-semibold leading-[36px] tracking-[-0.03em]" aria-live="polite">
        <span className="text-[18px] tracking-[-0.02em] text-ink-3">$</span>
        <CountUp to={Math.round(u.week.cost)} />
      </div>
      <p className="mt-2 text-[13px] text-ink-2">
        This week’s <span className="num">{fmtTok(u.week.used)}</span> counted tokens at list price. All time: <span className="num">${Math.round(u.totals.cost).toLocaleString()}</span>.
      </p>
      <details className="mt-auto pt-4 text-[12px] text-ink-3">
        <summary className="cursor-pointer hover:text-ink">How this is worked out</summary>
        <p className="mt-2 leading-[18px]">
          Each message’s input, output, cache-write and cache-read tokens are priced at that model’s published rate per million tokens.
          It shows how heavy the usage is; a subscription plan is not billed this way.
        </p>
      </details>
    </section>
  );
}

/* ---- Where tokens go. Numbers first (the claim), then two plain ranked bar lists: which model, which project. */
export function WhereTokensGo() {
  const { usage } = useUsage();
  const router = useRouter();
  const [ref, seen] = useInView<HTMLElement>(0.25);
  const w = usage!.week;
  const { models, projects } = useMemo(() => {
    const rank = (m: Record<string, number>) => Object.entries(m).sort((a, b) => b[1] - a[1]);
    return { models: rank(w.models), projects: rank(w.projects).slice(0, 5) };
  }, [w]);
  const pct = (a: number, b: number) => (b ? Math.round((a / b) * 100) : 0);
  const topP = projects[0];

  return (
    // Its own container: the stat row and the two bar lists split by the card's width, not the page's.
    // On Usage it spans both rows of the page grid and shares them: header + stats = row 1, bar lists = row 2.
    <section ref={ref} className="card enter @container grid content-start gap-y-5 @4xl:col-start-2 @4xl:row-span-2 @4xl:row-start-1 @4xl:grid-rows-subgrid">
      {w.used === 0 ? (
        <div>
          <CardHead title="Where tokens go" right={<span className="text-[12px] text-ink-3">this week</span>} />
          <Empty title="Nothing used this week" hint="Once Claude Code runs, the models and projects behind the tokens show up here." />
        </div>
      ) : (
        <>
          <div>
            <CardHead title="Where tokens go" right={<span className="text-[12px] text-ink-3">this week</span>} />
            <div className="grid gap-px border-b border-line bg-line @2xl:grid-cols-3">
              <Stat label="Tokens counted" value={fmtTok(w.used)} sub={<>of {fmtTok(w.limit)} · <CountUp to={pct(w.used, w.limit)} suffix="%" /> of the weekly limit</>} />
              <Stat label="Output tokens" value={fmtTok(w.output)} sub={<><CountUp to={pct(w.output, w.used)} suffix="%" /> of counted tokens, the costly kind</>} />
              {topP && <Stat label={`Busiest project · ${topP[0]}`} value={<CountUp to={pct(topP[1], w.used)} suffix="%" />} sub={<>{fmtTok(topP[1])} tokens this week</>} />}
            </div>
          </div>
          <div className="grid content-start gap-6 px-5 pb-5 @3xl:grid-cols-2">
            <RankList title="By model" note="tokens counted" seen={seen}
              rows={models.map(([m, n]) => ({ key: m, label: modelLabel(m), code: familyLabel(familyOf(m)), value: n, share: n / models[0][1], text: fmtTok(n), go: () => router.push(`/log?model=${encodeURIComponent(m)}`) }))} />
            <RankList title="By project" note="top 5 this week" seen={seen}
              rows={projects.map(([p, n]) => ({ key: p, label: p, value: n, share: n / projects[0][1], text: fmtTok(n), go: () => router.push(`/log?project=${encodeURIComponent(p)}`) }))} />
          </div>
        </>
      )}
    </section>
  );
}

export function Stat({ label, value, sub }: { label: string; value: React.ReactNode; sub: React.ReactNode }) {
  return (
    <div className="bg-surface px-5 py-4">
      <span className="label">{label}</span>
      <div className="num mt-1 text-[30px] font-semibold leading-[34px] tracking-[-0.03em]">{value}</div>
      <p className="mt-1 text-[12px] text-ink-3">{sub}</p>
    </div>
  );
}

export type RankRow = { key: string; label: string; code?: string | null; value: number; share: number; text: string; go: () => void };
export function RankList({ title, note, rows, seen }: { title: string; note: string; rows: RankRow[]; seen: boolean }) {
  return (
    <div className="min-w-0">
      <p className="mb-2 flex items-baseline justify-between gap-3"><span className="label">{title}</span><span className="text-[11px] text-ink-3">{note}</span></p>
      <ul className="grid gap-0.5">
        {rows.map((r, i) => (
          <li key={r.key}>
            <button onClick={r.go} className="grid h-9 w-full grid-cols-[112px_minmax(0,1fr)_56px] items-center gap-3 rounded-lg px-2 text-left hover:bg-surface-2">
              <span className="flex min-w-0 items-baseline gap-1.5 text-[13px]">
                <span className="truncate" title={r.label}>{r.label}</span>{r.code && <span className="num text-[10px] text-ink-3">{r.code}</span>}
              </span>
              <span className="h-2 rounded-full bg-surface-2">
                <span className="block h-full rounded-full transition-[width] duration-700 ease-out"
                  style={{ width: seen ? `${Math.max(r.share * 100, r.value ? 2 : 0)}%` : "0%", background: i === 0 ? "var(--b-900)" : "var(--b-300)", transitionDelay: `${COUNT_DELAY + i * 80}ms` }} />
              </span>
              <span className={cn("num text-right text-[13px]", i === 0 && "font-semibold")}>{r.text}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

/* ---- Burn rate: one row per window. Band = used so far, dot = where it lands at the current pace,
   black tick = the limit. Answers "will I make it to the reset?" at a glance. Amber dot = the pace passes the limit. */
type Burn = { name: string; used: number; limit: number; proj: number };
export function BurnRate() {
  const { usage } = useUsage();
  const now = useNow(30_000);
  const [ref, seen] = useInView<HTMLElement>(0.2);
  const u = usage!;
  const rows: Burn[] = [
    ...(u.session ? [{ name: "5h session", used: u.session.used, limit: u.session.limit, proj: projected(u.session.used, u.session.start, u.session.end, now) }] : []),
    { name: "Week", used: u.week.used, limit: u.week.limit, proj: projected(u.week.used, u.week.start, u.week.end, now) },
  ];
  const over = rows.filter((r) => r.proj > r.limit);
  const title = over.length === 0 ? "On pace to stay under both limits" : `On pace to pass the ${over.map((r) => (r.name === "Week" ? "weekly" : "5h")).join(" and ")} limit`;
  const axis = Math.max(1.25, ...rows.map((r) => (r.proj / r.limit) * 1.1)); // in multiples of the limit
  const pos = (v: number, limit: number) => Math.min((v / limit / axis) * 100, 100);
  const cols = "grid-cols-[112px_minmax(0,1fr)_112px]";
  return (
    <section ref={ref} className="card enter">
      <CardHead title={title} right={<DemoChip text="Projection" title="Straight-line pace so far. A burst after a quiet spell will beat it." />} />
      {u.week.used === 0 ? (
        <Empty title="No usage this week yet" hint="Once Claude Code runs, this shows whether the current pace reaches each limit before it resets." />
      ) : (
        <div className="grid gap-1 p-5">
          <div className={cn("grid gap-3", cols)}><span /><span /><span className="label text-right">Pace vs limit</span></div>
          {rows.map((r, i) => {
            const delay = COUNT_DELAY + i * 80;
            const hot = r.proj > r.limit;
            return (
              <div key={r.name} className={cn("grid h-10 items-center gap-3", cols)}>
                <span className="truncate text-[13px]">{r.name}</span>
                <div className="relative h-2 rounded-full bg-surface-2" title={`${fmtTok(r.used)} used · on pace for ${fmtTok(r.proj)} · limit ${fmtTok(r.limit)}`}>
                  <span className="absolute inset-y-0 left-0 rounded-full transition-[clip-path] duration-700 ease-out"
                    style={{ width: `${pos(r.used, r.limit)}%`, background: "var(--b-500)", opacity: 0.6, clipPath: seen ? "inset(0 0 0 0)" : "inset(0 100% 0 0)", transitionDelay: `${delay}ms` }} />
                  <i className="absolute -top-1 h-4 w-0.5 -translate-x-1/2 rounded-full bg-ink" style={{ left: `${pos(r.limit, r.limit)}%` }} />
                  {i === 0 && <span className="absolute -top-6 -translate-x-1/2 text-[10px] font-semibold uppercase tracking-[0.08em] text-ink-2" style={{ left: `${pos(r.limit, r.limit)}%` }}>limit</span>}
                  <i className="absolute top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-[var(--surface)] transition-opacity duration-300"
                    style={{ left: `${pos(r.proj, r.limit)}%`, background: hot ? "var(--review)" : "var(--b-900)", opacity: seen ? 1 : 0, transitionDelay: `${delay + 500}ms` }} />
                </div>
                <span className="num text-right text-[13px]">
                  <span className={cn("font-semibold", hot && "text-review-ink")}>{Math.round((r.proj / r.limit) * 100)}%</span>
                  <span className="text-ink-3"> vs 100</span>
                </span>
              </div>
            );
          })}
          <div className={cn("grid gap-3", cols)}>
            <span />
            <div className="num flex justify-between text-[11px] text-ink-3"><span>0%</span><span>{Math.round(axis * 100)}%</span></div>
          </div>
          <p className="mt-2 text-[12px] text-ink-3">Band: used so far. Dot: where the current pace ends up at the reset. {over.length ? "Amber means the pace passes the limit." : "Neither pace reaches its limit."}</p>
        </div>
      )}
    </section>
  );
}

/* §3 — pixel-block trend, from the real hourly series. Weekly: 7 days in 3h blocks; Monthly: 30 days in 12h; Yearly: 52 weeks. */
const ROWS = 34;
// days covered and hours per column: all three land near 55 columns so the grid always reads as a skyline.
const RANGES = { Weekly: { days: 7, hours: 3 }, Monthly: { days: 30, hours: 12 }, Yearly: { days: 364, hours: 168 } } as const;
const fmt = (d: Date, o: Intl.DateTimeFormatOptions) => d.toLocaleString("en", o);
const POWERS = Array.from({ length: 11 }, (_, k) => 10 ** k);
export function TrendChart() {
  const { usage } = useUsage();
  const hourly = usage!.hourly;
  const [range, setRange] = useState<keyof typeof RANGES>("Weekly");
  const [hover, setHover] = useState<number | null>(null);
  // The blocks stay empty until a third of the card is on screen, then fill in bottom-up, left to right. Replays on a range change.
  const [card, seen] = useInView<HTMLElement>(1 / 3);
  const [filled, setFilled] = useState(false);
  const lag = useRef(COUNT_DELAY); // the 1s beat is for the first reveal only; range switches refill straight away
  useEffect(() => {
    if (!seen) return;
    setFilled(false);
    let b = 0;
    const a = requestAnimationFrame(() => { b = requestAnimationFrame(() => setFilled(true)); });
    return () => { cancelAnimationFrame(a); cancelAnimationFrame(b); };
  }, [seen, range]);
  const [grp, setGrp] = useState<string | null>(null); // hovered day name (Weekly) or month name (Monthly / Yearly)
  const { data, labels } = useMemo(() => {
    const { days, hours } = RANGES[range];
    const start = new Date(); start.setHours(0, 0, 0, 0); start.setDate(start.getDate() - (days - 1));
    const span = hours * 3600_000;
    const n = (days * 24) / hours;
    const out = Array.from({ length: n }, (_, i) => ({ d: new Date(start.getTime() + i * span), n: FAMILIES.map(() => 0) }));
    hourly.forEach(([t, model, tokens]) => {
      const i = Math.floor((t - start.getTime()) / span);
      if (i >= 0 && i < n) out[i].n[FAMILIES.indexOf(familyOf(model))] += tokens;
    });
    const labels = out.map((x, i) => {
      const day = fmt(x.d, { weekday: "short", day: "numeric", month: "short" });
      const newDay = x.d.getHours() === 0;
      const monthGroup = { group: `${x.d.getFullYear()}-${x.d.getMonth()}`, groupTitle: fmt(x.d, { month: "long", year: "numeric" }) };
      if (range === "Weekly") return { tick: newDay ? fmt(x.d, { weekday: "short" }) : "", full: `${day}, ${String(x.d.getHours()).padStart(2, "0")}:00`, group: String(Math.floor(i / 8)), groupTitle: day, month: "" };
      if (range === "Monthly") {
        // Row 1: day numbers, anchored on the 1st then every 5th. Row 2: month name at the start and at each month change.
        const dn = x.d.getDate();
        const show = newDay && (dn === 1 || (dn % 5 === 0 && dn <= 25));
        const month = i === 0 || (newDay && dn === 1) ? fmt(x.d, { month: "long" }) : "";
        return { tick: show ? String(dn) : "", month, full: `${day}, ${newDay ? "AM" : "PM"}`, ...monthGroup };
      }
      const m = fmt(x.d, { month: "short" });
      // Year suffix on the first month and on each January, so the reader always knows which year they are in.
      const yy = i === 0 || x.d.getMonth() === 0 ? ` ${String(x.d.getFullYear()).slice(-2)}` : "";
      return { tick: i === 0 || out[i - 1].d.getMonth() !== x.d.getMonth() ? m + yy : "", full: `Week of ${x.d.getDate()} ${m}`, month: "", ...monthGroup };
    });
    return { data: out.map((x) => x.n), labels };
  }, [hourly, range]);
  const cols = data.length;
  const totalOf = (n: number[]) => n.reduce((a, b) => a + b, 0);
  const max = Math.max(1, ...data.map(totalOf));
  // Tokens per block is 1, 2, 5, 10, 20, 50 ... of a power of ten; the axis ticks are round multiples, at most 5 of them.
  const niceUnit = (x: number) => { const p = 10 ** Math.floor(Math.log10(x)), m = x / p; return (m <= 1 ? 1 : m <= 2 ? 2 : m <= 5 ? 5 : 10) * p; };
  const unit = niceUnit(Math.max(1, max / (ROWS - 2))); // leaves headroom above the tallest column
  const top = ROWS * unit;
  const step = [5, 10, 20, 25, 50].flatMap((m) => POWERS.map((p) => m * p)).sort((a, b) => a - b).find((v) => top / v <= 5)!;
  const ticks = Array.from({ length: Math.floor(top / step) + 1 }, (_, k) => k * step);
  const famTotals = FAMILIES.map((_, k) => data.reduce((a, n) => a + n[k], 0));
  const grand = totalOf(famTotals);
  // A spacer track between groups (Weekly: days, Monthly / Yearly: months) so each period reads as its own block.
  const GAP = 20;
  const newDay = (i: number) => i > 0 && labels[i].group !== labels[i - 1].group;
  const gaps = data.filter((_, i) => newDay(i)).length;
  const grid = { gridTemplateColumns: data.map((_, i) => (newDay(i) ? `${GAP}px ` : "") + "minmax(0, 1fr)").join(" ") };
  const spacer = (i: number) => (newDay(i) ? <span aria-hidden /> : null);
  // Hovering a day name (Weekly) or a month name (Monthly / Yearly) breaks that period down; the rest fades.
  const inGrp = (i: number) => grp != null && labels[i].group === grp;
  const grpCols = grp == null ? [] : labels.flatMap((l, i) => (l.group === grp ? [i] : []));
  const groupHover = (i: number) => ({
    onMouseEnter: () => { setGrp(labels[i].group); setHover(null); },
    onMouseLeave: () => setGrp(null),
  });
  // Centre of column i: track width w = (100% - spacers - 3px gutters) / cols, offset by the tracks and gutters before it.
  const xAt = (i: number) => {
    const g = data.slice(0, i + 1).filter((_, j) => newDay(j)).length;
    const w = `((100% - ${gaps * GAP + 3 * (cols + gaps - 1)}px) / ${cols})`;
    return `calc(${w} * ${i + 0.5} + ${g * GAP + 3 * (i + g)}px)`;
  };
  return (
    <section ref={card} className="card enter">
      <CardHead
        title="Token trend"
        right={
          <JellyRadio
            ariaLabel="Range"
            size="sm"
            items={Object.keys(RANGES)}
            value={range}
            onChange={(k) => { lag.current = 0; setRange(k as keyof typeof RANGES); setHover(null); setGrp(null); }}
            chipColor="var(--surface-2)"
            activeColor="var(--ink)"
            textColor="var(--ink-2)"
            activeTextColor="var(--paper)"
            className="-my-[var(--jr-pad-y)]"
          />
        }
      />
      <div className="flex flex-wrap items-end justify-between gap-x-10 gap-y-4 px-6 pt-6">
        <div>
          <span className="label">Tokens used</span>
          <div className="num mt-1 text-[40px] font-semibold leading-[44px] tracking-[-0.03em]"><CountUp to={grand} /></div>
        </div>
        <ul className="flex flex-wrap gap-x-5 gap-y-2" aria-hidden>
          {FAMILIES.map((f, k) => (
            <li key={f} className="flex items-center gap-2 text-[13px] text-ink-2">
              <i className="size-3 rounded-[3px]" style={{ background: FAMILY_SHADE[f] }} />
              {familyLabel(f)} <span className="num text-ink-3">{fmtTok(famTotals[k])}</span>
            </li>
          ))}
        </ul>
      </div>
      <p className="px-6 pt-3 text-[12px] text-ink-3">Each block is about {fmtTok(unit)} tokens. Darker means a more capable model: Fable and Opus are the darkest.</p>
      <div className="flex gap-4 px-6 pb-6 pt-5">
        <div className="relative h-[340px] w-12 shrink-0 text-right text-[11px] text-ink-3" aria-hidden>
          {ticks.map((v) => <span key={v} className="num absolute right-0 translate-y-1/2 leading-3" style={{ bottom: `${(v / top) * 100}%` }}>{fmtTok(v)}</span>)}
        </div>
        <div className="relative min-w-0 flex-1" onMouseLeave={() => setHover(null)}>
          <div className="grid h-[340px] gap-[3px]" style={grid} role="img" aria-label="Tokens over time, stacked by model family.">
            {data.map((n, x) => {
              let cum = 0;
              const bounds = n.map((v) => (cum += v, Math.round(cum / unit)));
              return (
                <Fragment key={x}>
                  {spacer(x)}
                  <div onMouseEnter={() => setHover(x)}
                    className={cn("flex cursor-default flex-col-reverse gap-[3px] transition-opacity duration-150", grp != null ? (!inGrp(x) && "opacity-25") : hover != null && hover !== x && "opacity-45")}>
                    {Array.from({ length: ROWS }, (_, r) => {
                      const fam = bounds.findIndex((b) => r < b);
                      return <i key={r} className="w-full flex-1 rounded-[2px] transition-[background-color] duration-300 ease-out"
                        style={{ background: fam === -1 || !filled ? "var(--b-050)" : FAMILY_SHADE[FAMILIES[fam]], transitionDelay: filled ? `${lag.current + x * 14 + r * 9}ms` : "0ms" }} />;
                    })}
                  </div>
                </Fragment>
              );
            })}
          </div>
          <div className="mt-3 grid gap-[3px] text-[11px] uppercase tracking-wide text-ink-3" style={grid} aria-hidden>
            {labels.map((l, i) => (
              <Fragment key={i}>
                {spacer(i)}
                {/* Weekly / Yearly: every column under a day or month name is a hover target for its breakdown. */}
                <span className={cn("num relative h-4", range !== "Monthly" && "cursor-pointer")} {...(range !== "Monthly" ? groupHover(i) : {})}>
                  <span className={cn("absolute left-0 whitespace-nowrap transition-colors", range !== "Monthly" && inGrp(i) && "text-ink")}>{l.tick}</span>
                </span>
              </Fragment>
            ))}
          </div>
          {labels.some((l) => l.month) && (
            <div className="mt-1 grid gap-[3px] text-[11px] font-semibold uppercase tracking-wide text-ink-2" style={grid} aria-hidden>
              {labels.map((l, i) => (
                <Fragment key={i}>
                  {spacer(i)}
                  <span className="relative h-5 cursor-pointer" {...groupHover(i)}>
                    {l.month && <span className={cn("absolute left-0 top-0 whitespace-nowrap border-l-2 border-ink-3 pl-1.5 leading-5 transition-colors", inGrp(i) ? "text-ink" : "text-ink-2")}>{l.month}</span>}
                  </span>
                </Fragment>
              ))}
            </div>
          )}
          {grp != null && grpCols.length > 0 && (() => {
            const first = grpCols[0], last = grpCols[grpCols.length - 1];
            const mid = (first + last) / 2 / cols;
            const shift = mid < 0.18 ? "0%" : mid > 0.82 ? "-100%" : "-50%";
            return <DayDonut
              title={labels[first].groupTitle}
              counts={FAMILIES.map((_, k) => grpCols.reduce((a, i) => a + data[i][k], 0))}
              left={shift === "0%" ? xAt(first) : shift === "-100%" ? xAt(last) : `calc((${xAt(first)} + ${xAt(last)}) / 2)`}
              shift={shift} />;
          })()}
          {hover != null && (
            <>
              <div className="pointer-events-none absolute top-0 h-[340px] border-l border-dashed border-ink-3" style={{ left: xAt(hover) }} />
              <div className="pointer-events-none absolute top-3 z-10 min-w-48 rounded-xl bg-surface px-4 py-3 text-[13px] shadow-[var(--shadow-pop)] ring-1 ring-line-strong"
                style={{ left: xAt(hover), transform: hover > cols * 0.6 ? "translateX(calc(-100% - 14px))" : "translateX(14px)" }}>
                <div className="mb-2 rounded-md bg-surface-2 px-2 py-1 text-[12px] font-medium text-ink-2">{labels[hover].full}</div>
                {[...FAMILIES].reverse().map((f) => (
                  <div key={f} className="flex items-center gap-2 py-0.5">
                    <i className="size-2.5 rounded-[3px]" style={{ background: FAMILY_SHADE[f] }} />
                    <span className="flex-1 text-ink-2">{familyLabel(f)}</span>
                    <span className="num font-semibold">{fmtTok(data[hover][FAMILIES.indexOf(f)])}</span>
                  </div>
                ))}
                <div className="mt-1 flex justify-between border-t border-line pt-1.5 font-semibold"><span>Total</span><span className="num">{fmtTok(totalOf(data[hover]))}</span></div>
              </div>
            </>
          )}
        </div>
      </div>
      {/* In a div: a table ignores sr-only's 1px height and would stretch the page. */}
      <div className="sr-only"><table>
        <caption>Tokens over time by model family</caption>
        <thead><tr><th>Period</th>{FAMILIES.map((f) => <th key={f}>{familyLabel(f)}</th>)}</tr></thead>
        <tbody>{data.map((n, i) => <tr key={i}><td>{labels[i].full}</td>{n.map((v, k) => <td key={k}>{v}</td>)}</tr>)}</tbody>
      </table></div>
    </section>
  );
}

