"use client";

import { useMemo } from "react";
import { useRouter } from "next/navigation";
import { Cell, Pie, PieChart, ResponsiveContainer, Sankey } from "recharts";
import type { LinkProps as SankeyLinkProps, NodeProps as SankeyNodeProps } from "recharts/types/chart/Sankey";
import { useUsage } from "@/lib/app-state";
import { FAMILIES, FAMILY_SHADE, familyLabel, familyOf, familyTotals, fmtTok, modelLabel } from "@/lib/usage";
import { COUNT_DELAY, CountUp, Empty } from "@/components/ui";
import { CardHead, useInView } from "./widgets";

/* The recharts widgets live here so recharts (about 330 KB) is only fetched when a chart is shown. */

/* §4 — model mix. recharts owns the arcs; legend rows are the real hit targets. */
export function ModelMix() {
  const { usage } = useUsage();
  const router = useRouter();
  const per = familyTotals(usage!.week.models);
  const total = usage!.week.used;
  const data = FAMILIES.map((f) => ({ f, n: per[f] })).filter((d) => d.n > 0);
  const open = (f: string) => router.push(`/log?family=${f}`);
  return (
    <section className="card enter">
      <CardHead title="Model mix" right={<span className="text-[12px] text-ink-3">this week</span>} />
      <div className="flex flex-wrap items-center justify-center gap-6 p-5">
        <div className="relative size-[176px] shrink-0">
          <PieChart width={176} height={176}>
            <Pie data={data.length ? data : [{ f: "other", n: 1 }]} dataKey="n" innerRadius={58} outerRadius={84} minAngle={6} paddingAngle={data.length > 1 ? 2 : 0}
              stroke="var(--surface)" strokeWidth={2} startAngle={90} endAngle={-270} animationDuration={400}
              onClick={(d) => data.length && open((d as unknown as { f: string }).f)} style={{ cursor: "pointer", outline: "none" }}>
              {(data.length ? data : [{ f: "other" }]).map((d) => (
                <Cell key={d.f} style={{ fill: data.length ? FAMILY_SHADE[d.f as keyof typeof FAMILY_SHADE] : "var(--b-050)" }} />
              ))}
            </Pie>
          </PieChart>
          <div className="pointer-events-none absolute inset-0 grid place-content-center text-center">
            <div className="num text-[28px] font-semibold leading-8 tracking-[-0.02em]">{fmtTok(total)}</div>
            <div className="label">Tokens</div>
          </div>
        </div>
        <ul className="grid min-w-[190px] flex-1 gap-1">
          {FAMILIES.map((f) => (
            <li key={f}>
              <button onClick={() => open(f)} className="flex h-10 w-full items-center gap-3 rounded-lg px-2 text-left hover:bg-surface-2">
                <i className="size-3 shrink-0 rounded-sm" style={{ background: FAMILY_SHADE[f] }} aria-hidden />
                <span className="flex-1">{familyLabel(f)}</span>
                <span className="num">{fmtTok(per[f])}</span>
                <span className="num w-10 text-right text-ink-3"><CountUp to={total ? Math.round((per[f] / total) * 100) : 0} suffix="%" /></span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

/* ---- Token flow: where this week's tokens went. Left to right, one reading direction:
   week → model → kind of token. */
type FlowNode = { name: string; color: string; href?: string; col: number };
export function PipelineFlow() {
  const { usage } = useUsage();
  const router = useRouter();
  const [ref, seen] = useInView<HTMLElement>(0.25);
  const flow = usage!.flow;
  const data = useMemo(() => {
    const KIND: Record<string, string> = { Input: "var(--ink-2)", Output: "var(--ink)", "Cache write": "var(--ink-3)" };
    const nodes: FlowNode[] = [];
    const at = new Map<string, number>();
    const node = (key: string, make: () => FlowNode) => at.get(key) ?? (at.set(key, nodes.push(make()) - 1), nodes.length - 1);
    const links = new Map<string, { source: number; target: number; value: number }>();
    flow.forEach(([, model, kind, n]) => {
      const a = node("week", () => ({ name: "This week", color: "var(--ink)", col: 0, href: "/log" }));
      const b = node(`m:${model}`, () => ({ name: modelLabel(model), color: FAMILY_SHADE[familyOf(model)], col: 1, href: `/log?model=${encodeURIComponent(model)}` }));
      const c = node(`k:${kind}`, () => ({ name: kind, color: KIND[kind] ?? "var(--ink-3)", col: 2 }));
      for (const [s, t] of [[a, b], [b, c]]) {
        const l = links.get(`${s}|${t}`) ?? { source: s, target: t, value: 0 };
        l.value += n;
        links.set(`${s}|${t}`, l);
      }
    });
    return { nodes, links: [...links.values()] };
  }, [flow]);
  const last = Math.max(0, ...data.nodes.map((n) => n.col));

  return (
    <section ref={ref} className="card enter">
      <CardHead title="Where this week’s tokens went" right={<span className="text-[12px] text-ink-3">Click a model to open its log</span>} />
      {data.links.length === 0 ? (
        <Empty title="Nothing used this week" hint="Once Claude Code runs, each model’s share and token kinds show here." />
      ) : (
        <div className="p-4 pr-2">
          <ResponsiveContainer width="100%" height={200}>
            <Sankey data={data} nodeWidth={10} nodePadding={16} linkCurvature={0.5} iterations={64} sort={false} align="left"
              margin={{ top: 8, bottom: 8, left: 130, right: 150 }}
              node={(p: SankeyNodeProps) => <FlowNodeMark {...p} seen={seen} onOpen={(href) => router.push(href)} />}
              link={(p: SankeyLinkProps) => <FlowLink {...p} last={last} seen={seen} />} />
          </ResponsiveContainer>
        </div>
      )}
      {/* In a div: a table ignores sr-only's 1px height and would stretch the page. */}
      <div className="sr-only"><table>
        <caption>Tokens moving between stages this week</caption>
        <thead><tr><th>From</th><th>To</th><th>Tokens</th></tr></thead>
        <tbody>{data.links.map((l) => <tr key={`${l.source}-${l.target}`}><td>{data.nodes[l.source].name}</td><td>{data.nodes[l.target].name}</td><td>{l.value}</td></tr>)}</tbody>
      </table></div>
    </section>
  );
}

/** Nodes fade in on the count-up beat, one column after another. */
function FlowNodeMark({ x, y, width, height, payload, seen, onOpen }: SankeyNodeProps & { seen: boolean; onOpen: (href: string) => void }) {
  const n = payload as unknown as FlowNode & { value: number };
  const delay = COUNT_DELAY + n.col * 280;
  const go = () => n.href && onOpen(n.href);
  return (
    <g role={n.href ? "link" : undefined} tabIndex={n.href ? 0 : undefined} aria-label={`${n.name}: ${fmtTok(n.value)}`} className="flow-node cursor-pointer outline-none"
      onClick={go} onKeyDown={(e) => e.key === "Enter" && go()}
      style={{ opacity: seen ? 1 : 0, transition: `opacity 300ms ease ${delay}ms` }}>
      <rect className="flow-bar" x={x} y={y} width={width} height={Math.max(height, 2)} rx={3} fill={n.color} />
      {/* Plain text, no chip. The first column's label sits left of its bar, the rest to the right. */}
      <text x={n.col === 0 ? x - 8 : x + width + 8} y={y + height / 2}
        textAnchor={n.col === 0 ? "end" : "start"} dominantBaseline="middle" fontSize={13} fill="var(--ink)">
        {n.name} <tspan className="num" fontWeight={600}>{fmtTok(n.value)}</tspan>
      </text>
    </g>
  );
}

const FLOW_MIN_RUN = 64;
/** Each band draws itself left to right, after the column it leaves has landed. */
function FlowLink({ sourceX: sx, sourceY, targetX, targetY, linkWidth, payload, seen }: SankeyLinkProps & { last: number; seen: boolean }) {
  const from = payload.source as unknown as FlowNode;
  const to = payload.target as unknown as FlowNode;
  // A link leaves from its source node's right edge (x + nodeWidth 10) and lands on the target node's x.
  // Start at the bar's centre (nodes draw over links), so the band's edge is tucked under the bar, never beside it.
  const sourceX = Math.min(sx - 10 + 5, targetX - FLOW_MIN_RUN);
  const control = (sourceX + targetX) / 2; // linkCurvature 0.5: both control points at the midpoint
  const delay = COUNT_DELAY + from.col * 280 + 140;
  const color = from.col === 0 ? to.color : from.color; // out of the week node a band wears its model's shade
  return (
    <path d={`M${sourceX},${sourceY} C${control},${sourceY} ${control},${targetY} ${targetX},${targetY}`}
      fill="none" stroke={color} strokeWidth={Math.max(linkWidth, 1)} pathLength={1} strokeDasharray="1"
      className="flow-link"
      style={{ strokeDashoffset: seen ? 0 : 1, transition: `stroke-dashoffset 700ms var(--ease-out) ${delay}ms, opacity 150ms ease` }} />
  );
}

/* Weekly: donut of one day's tokens by model family, shown while its name is hovered. */
export function DayDonut({ title, counts, left, shift }: { title: string; counts: number[]; left: string; shift: string }) {
  const total = counts.reduce((a, b) => a + b, 0);
  const data = FAMILIES.map((f, k) => ({ f, n: counts[k] })).filter((d) => d.n > 0);
  return (
    <div className="pointer-events-none absolute top-2 z-10 flex items-center gap-4 rounded-xl bg-surface p-4 shadow-[var(--shadow-pop)] ring-1 ring-line-strong"
      style={{ left, transform: `translateX(${shift})` }}>
      <div className="relative size-[132px] shrink-0">
        <PieChart width={132} height={132}>
          <Pie data={data.length ? data : [{ f: "other", n: 1 }]} dataKey="n" innerRadius={44} outerRadius={64} paddingAngle={data.length > 1 ? 2 : 0}
            stroke="var(--surface)" strokeWidth={2} startAngle={90} endAngle={-270} animationDuration={250}>
            {(data.length ? data : [{ f: "other" }]).map((d) => (
              <Cell key={d.f} style={{ fill: data.length ? FAMILY_SHADE[d.f as keyof typeof FAMILY_SHADE] : "var(--b-050)" }} />
            ))}
          </Pie>
        </PieChart>
        <div className="absolute inset-0 grid place-content-center text-center">
          <div className="num text-[22px] font-semibold leading-6">{fmtTok(total)}</div>
          <div className="label">Tokens</div>
        </div>
      </div>
      <div className="min-w-40 text-[13px]">
        <div className="mb-2 font-semibold">{title}</div>
        {FAMILIES.map((f, k) => (
          <div key={f} className="flex items-center gap-2 py-0.5">
            <i className="size-2.5 rounded-[3px]" style={{ background: FAMILY_SHADE[f] }} />
            <span className="flex-1 text-ink-2">{familyLabel(f)}</span>
            <span className="num font-semibold">{fmtTok(counts[k])}</span>
            <span className="num w-9 text-right text-ink-3">{total ? Math.round((counts[k] / total) * 100) : 0}%</span>
          </div>
        ))}
      </div>
    </div>
  );
}
