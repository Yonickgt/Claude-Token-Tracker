"use client";

import { Suspense, useMemo } from "react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { ArrowDownUp, ChevronLeft, ChevronRight, Copy, Download, Search, X } from "lucide-react";
import { HeaderActions } from "@/components/shell/Shell";
import { Dropdown, Empty, ModelPill, OfflineNote, Skeleton } from "@/components/ui";
import SwipeRow from "@/components/SwipeRow";
import { toast } from "@/lib/toast";
import { useUsage } from "@/lib/app-state";
import { counted, useLog } from "@/lib/log";
import { FAMILIES, familyLabel, fmtTok, hhmm, ddmmyy, modelLabel, type LogRow } from "@/lib/usage";

const PAGE = 50;

export default function LogRoute() {
  return <Suspense fallback={<Skeleton className="h-[420px]" />}><Log /></Suspense>;
}

function Log() {
  const { usage } = useUsage();
  const params = useSearchParams();
  const router = useRouter();
  const path = usePathname();
  const get = (k: string) => params.get(k) ?? "";
  const [q, model, family, project, session, order] = ["q", "model", "family", "project", "session", "order"].map(get);
  const page = Math.max(1, Number(get("page")) || 1);
  const set = (patch: Record<string, string>) => {
    const next = new URLSearchParams(params.toString());
    Object.entries(patch).forEach(([k, v]) => (v ? next.set(k, v) : next.delete(k)));
    if (!("page" in patch)) next.delete("page");
    router.replace(`${path}${next.size ? `?${next}` : ""}`, { scroll: false });
  };
  const filter = useMemo(() => {
    const f = new URLSearchParams();
    Object.entries({ q, model, family, project, session, order }).forEach(([k, v]) => v && f.set(k, v));
    return f;
  }, [q, model, family, project, session, order]);
  const { page: data, error, loading } = useLog(`${filter}${filter.size ? "&" : ""}limit=${PAGE}&offset=${(page - 1) * PAGE}`);
  const models = useMemo(() => [...new Set((usage?.sessions ?? []).flatMap((s) => Object.keys(s.models)))].sort(), [usage]);
  const pages = Math.max(1, Math.ceil((data?.total ?? 0) / PAGE));

  const save = async (kind: "csv" | "json") => {
    const r = await fetch(`/api/log?${filter}${filter.size ? "&" : ""}limit=5000`).then((x) => x.json() as Promise<{ rows: LogRow[] }>);
    const cell = (v: unknown) => `"${String(v ?? "").replace(/"/g, '""')}"`;
    const body = kind === "json" ? JSON.stringify(r.rows, null, 2)
      : ["time,model,project,input,output,cache_write,cache_read", ...r.rows.map((x) => [new Date(x.ts).toISOString(), x.model, x.project, x.input, x.output, x.cacheWrite, x.cacheRead].map(cell).join(","))].join("\r\n");
    const url = URL.createObjectURL(new Blob([body], { type: kind === "json" ? "application/json" : "text/csv" }));
    Object.assign(document.createElement("a"), { href: url, download: `token-log-${new Date().toISOString().slice(0, 10)}.${kind}` }).click();
    URL.revokeObjectURL(url);
    toast({ title: "Log exported", description: `${r.rows.length} rows saved as ${kind.toUpperCase()}${data && data.total > r.rows.length ? ` (newest ${r.rows.length} of ${data.total})` : ""}`, icon: <Download />, tone: "info" });
  };
  const chip = (label: string, key: string) => <button key={key} className="btn btn-sm" onClick={() => set({ [key]: "" })}>{label} <X size={13} aria-hidden /></button>;

  return (
    <div className="grid gap-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-ink-2"><span className="num font-semibold text-ink">{(data?.total ?? 0).toLocaleString()}</span> messages. Every request Claude Code logged, with the tokens that count against your limits.</p>
        <HeaderActions>
          <button className="btn" onClick={() => void save("csv")} disabled={!data?.total}><Download size={15} aria-hidden />Export CSV</button>
          <button className="btn" onClick={() => void save("json")} disabled={!data?.total}><Download size={15} aria-hidden />Export JSON</button>
        </HeaderActions>
      </div>
      {error && <OfflineNote onRetry={() => router.refresh()} />}
      <section className="card">
        <div className="flex flex-wrap items-center gap-2 border-b border-line p-4">
          <label className="relative min-w-[220px] flex-1"><span className="sr-only">Search the log</span>
            <Search size={15} className="pointer-events-none absolute left-3 top-3 text-ink-3" aria-hidden />
            <input className="field w-full pl-9" placeholder="Search project or model" value={q} onChange={(e) => set({ q: e.target.value })} />
          </label>
          <Dropdown label="Model" value={model} onChange={(v) => set({ model: v, family: "" })}
            options={[{ value: "", label: "All models" }, ...models.map((m) => ({ value: m, label: modelLabel(m) }))]} />
          <button className="btn" onClick={() => set({ order: order === "asc" ? "" : "asc" })}><ArrowDownUp size={15} aria-hidden />{order === "asc" ? "Oldest first" : "Newest first"}</button>
          {family && chip(`Family: ${familyLabel(family as (typeof FAMILIES)[number])}`, "family")}
          {project && chip(`Project: ${project}`, "project")}
          {session && chip("One session", "session")}
        </div>
        {loading ? (
          <Skeleton className="m-4 h-72" />
        ) : !data?.rows.length ? (
          <Empty title="No messages match" hint="Clear a filter to see more." />
        ) : (
          <>
            {/* Below 900px: swipe cards (swipe left to copy the row). Desktop: the table. */}
            <ol className="grid gap-2 p-3 min-[900px]:hidden" aria-label="Messages">
              {data.rows.map((r) => (
                <li key={`${r.ts}-${r.model}-${r.input}-${r.output}`} className="min-w-0">
                  <SwipeRow label={`${modelLabel(r.model)} ${hhmm(r.ts)}`} fullSwipe={false} height="auto" actionWidth={76} rowColor="var(--surface-2)" textColor="var(--ink)" actionColor="#0f5c6b" drawerColor="#3f4b51"
                    actions={[{ id: "copy", label: "Copy", icon: <Copy size={18} />, onSelect: () => void navigator.clipboard?.writeText(JSON.stringify(r)).then(() => toast({ title: "Row copied", icon: <Copy />, tone: "info", duration: 2000 })) }]}>
                    <div className="flex min-w-0 flex-1 items-center gap-3 py-3">
                      <span className="min-w-0 flex-1">
                        <span className="block truncate font-semibold">{r.project}</span>
                        <span className="num block truncate text-[12px] text-ink-3">{ddmmyy(r.ts)} {hhmm(r.ts)} · {modelLabel(r.model)}</span>
                      </span>
                      <span className="num font-semibold">{fmtTok(counted(r))}</span>
                    </div>
                  </SwipeRow>
                </li>
              ))}
            </ol>
            <div className="hidden overflow-x-auto min-[900px]:block">
              <table className="w-full text-[13px]">
                <thead className="border-b border-line text-left">
                  <tr>
                    <th scope="col" className="label h-10 pl-5">Time</th>
                    <th scope="col" className="label">Model</th>
                    <th scope="col" className="label">Project</th>
                    <th scope="col" className="label text-right">Input</th>
                    <th scope="col" className="label text-right">Output</th>
                    <th scope="col" className="label text-right">Cache write</th>
                    <th scope="col" className="label text-right">Cache read</th>
                    <th scope="col" className="label pr-5 text-right">Counted</th>
                  </tr>
                </thead>
                <tbody>
                  {data.rows.map((r) => (
                    <tr key={`${r.ts}-${r.model}-${r.input}-${r.output}`} className="h-11 border-b border-line last:border-0 hover:bg-surface-2">
                      <td className="num whitespace-nowrap pl-5 pr-4">{ddmmyy(r.ts)} {hhmm(r.ts)}</td>
                      <td className="pr-3"><ModelPill model={r.model} /></td>
                      <td className="num max-w-[200px] truncate pr-4" title={r.project}>{r.project}</td>
                      <td className="num pr-4 text-right">{fmtTok(r.input)}</td>
                      <td className="num pr-4 text-right">{fmtTok(r.output)}</td>
                      <td className="num pr-4 text-right">{fmtTok(r.cacheWrite)}</td>
                      <td className="num pr-4 text-right text-ink-3">{fmtTok(r.cacheRead)}</td>
                      <td className="num pr-5 text-right font-semibold">{fmtTok(counted(r))}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </>
        )}
        <div className="flex items-center justify-between border-t border-line px-5 py-3 text-[13px] text-ink-3">
          <span className="num">{data?.total ? `${(page - 1) * PAGE + 1}–${Math.min(page * PAGE, data.total)} of ${data.total.toLocaleString()}` : "0 of 0"}</span>
          <span className="flex gap-1">
            <button className="icon-btn size-9" disabled={page <= 1} onClick={() => set({ page: String(page - 1) })} aria-label="Previous page"><ChevronLeft size={16} /></button>
            <button className="icon-btn size-9" disabled={page >= pages} onClick={() => set({ page: String(page + 1) })} aria-label="Next page"><ChevronRight size={16} /></button>
          </span>
        </div>
      </section>
    </div>
  );
}
