"use client";

import { useMemo, useState } from "react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { Menu } from "@base-ui/react/menu";
import Link from "next/link";
import { ArrowDown, ArrowUp, ArrowUpDown, ChevronLeft, ChevronRight, Copy, ExternalLink, MoreHorizontal, Search } from "lucide-react";
import SwipeRow from "@/components/SwipeRow";
import JellyRadio from "@/components/JellyRadio";
import { toast } from "@/lib/toast";
import { useUsage } from "@/lib/app-state";
import { FAMILIES, ddmmyy, familyLabel, familyOf, fmtTok, levelOf, searchText, sessionName, topKey, type Level, type Session } from "@/lib/usage";
import { Dropdown, Empty, ModelPill, StatusPill, UsageScore } from "@/components/ui";

const PAGE = 10;
const LEVEL_RANK: Record<Level, number> = { over: 0, near: 1, ok: 2 };
type SortKey = "date" | "tokens" | "messages" | "status";

/** Filters live in the URL so a chart click, a deep link and a reload all agree. */
export function useSessions() {
  const { usage } = useUsage();
  const params = useSearchParams();
  const router = useRouter();
  const path = usePathname();
  const q = params.get("q") ?? "";
  const family = params.get("family") ?? "";
  const status = params.get("status") ?? "";
  const scope = params.get("scope") ?? ""; // "active": the summary chips, so each count equals its list
  const limit = usage?.config.limit5h ?? 1;
  const set = (patch: Record<string, string>) => {
    const next = new URLSearchParams(params.toString());
    Object.entries(patch).forEach(([k, v]) => (v ? next.set(k, v) : next.delete(k)));
    next.delete("page");
    router.replace(`${path}${next.size ? `?${next}` : ""}`, { scroll: false });
  };
  const sessions = usage?.sessions;
  const rows = useMemo(() => {
    const n = q.trim().toLowerCase();
    return (sessions ?? []).filter(
      (s) =>
        (!n || searchText(s).includes(n)) &&
        (!family || Object.keys(s.models).some((m) => familyOf(m) === family)) &&
        (!status || levelOf(s.tokens, limit) === status) &&
        (scope !== "active" || s.active),
    );
  }, [sessions, limit, q, family, status, scope]);
  const active = !!(q || family || status || scope);
  // Carried into /session/[id] so J/K there walks the same filtered list.
  const search = params.size ? `?${params}` : "";
  return { rows, limit, q, family, status, scope, set, active, search, clear: () => set({ q: "", family: "", status: "", scope: "" }) };
}

export function Filters({ q: qs }: { q: ReturnType<typeof useSessions> }) {
  const { q, family, status, set } = qs;
  return (
    <div className="flex flex-wrap items-center gap-2">
      <label className="relative min-w-[200px] flex-1">
        <span className="sr-only">Search sessions</span>
        <Search size={15} className="pointer-events-none absolute left-3 top-3 text-ink-3" aria-hidden />
        <input className="field w-full pl-9" placeholder="Search by day, project or model" value={q} autoComplete="off" name="session-search"
          onChange={(e) => set({ q: e.target.value })} />
      </label>
      <Dropdown label="Model family" value={family} onChange={(v) => set({ family: v })}
        options={[{ value: "", label: "All models" }, ...FAMILIES.map((f) => ({ value: f, label: familyLabel(f) }))]} />
      <Dropdown label="Status" value={status} onChange={(v) => set({ status: v, scope: "" })}
        options={[{ value: "", label: "All statuses" }, { value: "near", label: "Near limit" }, { value: "over", label: "Over limit" }, { value: "ok", label: "Under limit" }]} />
    </div>
  );
}

export const defaultOrder = (rows: Session[]) => [...rows].sort((a, b) => b.start - a.start);

/** One row of counts, each a filter. The only place these numbers appear. */
export function SessionStatusRow() {
  const { summary: s } = useUsage();
  const qs = useSessions();
  // Near + over + under = every session; Active is the one running now (it is also in one of those three).
  const chips = [
    { label: "All sessions", n: s!.total, on: !qs.status && !qs.scope, patch: { status: "", scope: "" }, cls: "" },
    { label: "Active", n: s!.active, on: qs.scope === "active", patch: { status: "", scope: "active" }, cls: "" },
    // Chip colours go in as --jr-* vars (the jelly skin reads them); amber stays reserved for "needs a person".
    { label: "Near limit", n: s!.near, on: qs.status === "near", patch: { status: "near", scope: "" }, cls: "[--jr-chip:var(--review-tint)] [--jr-text:var(--review-ink)] [--jr-ring:inset_0_0_0_1px_var(--review)]" },
    { label: "Over limit", n: s!.over, on: qs.status === "over", patch: { status: "over", scope: "" }, cls: "[--jr-chip:var(--defect-tint)] [--jr-text:var(--defect)] [--jr-ring:inset_0_0_0_1px_var(--defect)]" },
    { label: "Under limit", n: s!.ok, on: qs.status === "ok", patch: { status: "ok", scope: "" }, cls: "" },
  ];
  return (
    <JellyRadio
      ariaLabel="Session summary"
      className="-mx-[var(--jr-pad-x)] flex-wrap [--jr-ring:inset_0_0_0_1px_var(--line-strong)]"
      items={chips.map((c) => ({
        value: c.label,
        className: c.cls,
        label: <span className="flex items-center gap-1.5"><span className="num text-[15px] font-semibold">{c.n}</span>{c.label}</span>,
      }))}
      value={chips.find((c) => c.on)?.label ?? ""}
      onChange={(v) => qs.set(chips.find((c) => c.label === v)!.patch)}
      chipColor="var(--surface)"
      activeColor="var(--ink)"
      textColor="var(--ink-2)"
      activeTextColor="var(--paper)"
      swell={0.12}
    />
  );
}

export function SessionTable() {
  const qs = useSessions();
  const router = useRouter();
  const [sort, setSort] = useState<{ key: SortKey; dir: 1 | -1 }>({ key: "date", dir: -1 });
  const [page, setPage] = useState(1);
  const sorted = useMemo(() => {
    const v = (s: Session) => (sort.key === "date" ? s.start : sort.key === "tokens" ? s.tokens : sort.key === "messages" ? s.messages : LEVEL_RANK[levelOf(s.tokens, qs.limit)]);
    return [...qs.rows].sort((a, b) => (v(a) - v(b)) * sort.dir || b.start - a.start);
  }, [qs.rows, qs.limit, sort]);
  const pages = Math.max(1, Math.ceil(sorted.length / PAGE));
  const p = Math.min(page, pages);
  const shown = sorted.slice((p - 1) * PAGE, p * PAGE);
  const th = (key: SortKey, text: string, cls = "") => (
    <th scope="col" aria-sort={sort.key === key ? (sort.dir === 1 ? "ascending" : "descending") : "none"} className={cls}>
      <button className="group flex h-10 items-center gap-1 label" onClick={() => setSort((s) => ({ key, dir: s.key === key ? (s.dir === 1 ? -1 : 1) : key === "status" ? 1 : -1 }))}>
        {text}
        {sort.key === key ? (sort.dir === 1 ? <ArrowUp size={12} /> : <ArrowDown size={12} />) : <ArrowUpDown size={12} className="opacity-40 group-hover:opacity-100" />}
      </button>
    </th>
  );
  const open = (id: string) => router.push(`/session/${id}${qs.search}`);

  return (
    <section id="sessions" className="card min-w-0 scroll-mt-24">
      <div className="card-head flex-wrap">
        <h2 className="text-[18px] font-semibold leading-6 tracking-[-0.01em]">5h sessions</h2>
        <span className="text-[13px] text-ink-3" aria-live="polite">{qs.rows.length} {qs.rows.length === 1 ? "session" : "sessions"}</span>
      </div>
      <div className="border-b border-line p-4"><Filters q={qs} /></div>
      {shown.length === 0 ? (
        <Empty
          title={qs.active ? "No sessions match these filters" : "No sessions yet"}
          hint={qs.active ? "Try a different search, or clear the filters." : "A session starts with your first Claude Code message and lasts five hours."}
          action={qs.active ? <button className="btn btn-sm" onClick={qs.clear}>Clear filters</button> : undefined}
        />
      ) : (
        <>
        {/* Below 900px (the shell's narrow breakpoint) the list is swipe cards: tap opens, swipe left for Open / Copy id.
            Desktop keeps the sortable table. CSS picks one, so only one is ever in the tab order. */}
        <ol className="grid gap-2 p-3 min-[900px]:hidden" aria-label="5h sessions">
          {shown.map((s) => (
            <li key={s.id} className="min-w-0">{/* grid items default to min-width:auto, which defeats truncate */}
              <SwipeRow
                label={sessionName(s)}
                fullSwipe={false}
                height="auto"
                actionWidth={76}
                rowColor="var(--surface-2)"
                textColor="var(--ink)"
                actionColor="#0f5c6b"
                drawerColor="#3f4b51"
                actions={[
                  { id: "open", label: "Open", icon: <ExternalLink size={18} />, onSelect: () => open(s.id) },
                  { id: "copy", label: "Copy id", icon: <Copy size={18} />, onSelect: () => copyId(s.id) },
                ]}
              >
                <Link href={`/session/${s.id}${qs.search}`} className="flex min-w-0 flex-1 items-center gap-3 py-3 outline-none focus-visible:underline">
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-semibold">{sessionName(s)}</span>
                    <span className="num block truncate text-[12px] text-ink-3">{fmtTok(s.tokens)} · {s.messages} msgs · {topKey(s.projects)}</span>
                  </span>
                  <StatusPill level={levelOf(s.tokens, qs.limit)} active={s.active} />
                </Link>
              </SwipeRow>
            </li>
          ))}
        </ol>
        <div className="hidden overflow-x-auto min-[900px]:block">
          <table className="w-full text-[13px]">
            <thead className="border-b border-line text-left">
              <tr>
                {th("date", "Session", "pl-5")}
                <th scope="col" className="label">Models</th>
                <th scope="col" className="label hidden lg:table-cell">Busiest project</th>
                {th("messages", "Messages", "hidden lg:table-cell")}
                {th("tokens", "Tokens")}
                <th scope="col" className="label">Of limit</th>
                {th("status", "Status")}
                <th scope="col" className="w-12"><span className="sr-only">Actions</span></th>
              </tr>
            </thead>
            <tbody>
              {shown.map((s) => {
                const models = Object.entries(s.models).sort((a, b) => b[1] - a[1]);
                return (
                  <tr
                    key={s.id}
                    tabIndex={0}
                    onClick={() => open(s.id)}
                    onKeyDown={(e) => {
                      if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); open(s.id); }
                    }}
                    className="h-11 cursor-pointer border-b border-line outline-none last:border-0 hover:bg-surface-2 focus-visible:bg-surface-2 focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-burgundy"
                  >
                    <td className="whitespace-nowrap pl-5 pr-4"><span className="num">{sessionName(s)}</span> <span className="num text-[12px] text-ink-3">{ddmmyy(s.start)}</span></td>
                    <td className="pr-3">
                      <span className="flex flex-wrap items-center gap-1">
                        {models.slice(0, 2).map(([m]) => <ModelPill key={m} model={m} />)}
                        {models.length > 2 && <span className="num text-[12px] text-ink-3">+{models.length - 2}</span>}
                      </span>
                    </td>
                    <td className="num hidden max-w-[180px] truncate pr-4 lg:table-cell" title={topKey(s.projects) ?? ""}>{topKey(s.projects) ?? "—"}</td>
                    <td className="num hidden pr-6 lg:table-cell">{s.messages.toLocaleString()}</td>
                    <td className="num pr-4">{fmtTok(s.tokens)}</td>
                    <td className="pr-3"><UsageScore used={s.tokens} limit={qs.limit} /></td>
                    <td className="pr-3"><StatusPill level={levelOf(s.tokens, qs.limit)} active={s.active} /></td>
                    <td onClick={(e) => e.stopPropagation()}><RowMenu id={s.id} onOpen={() => open(s.id)} /></td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        </>
      )}
      <div className="flex items-center justify-between border-t border-line px-5 py-3 text-[13px] text-ink-3">
        <span className="num">{sorted.length ? `${(p - 1) * PAGE + 1}–${Math.min(p * PAGE, sorted.length)} of ${sorted.length}` : "0 of 0"}</span>
        <span className="flex gap-1">
          <button className="icon-btn size-9" disabled={p <= 1} onClick={() => setPage(p - 1)} aria-label="Previous page"><ChevronLeft size={16} /></button>
          <button className="icon-btn size-9" disabled={p >= pages} onClick={() => setPage(p + 1)} aria-label="Next page"><ChevronRight size={16} /></button>
        </span>
      </div>
    </section>
  );
}

/** Copy a session id and confirm it; only toasts once the clipboard write has actually succeeded. */
const copyId = (id: string) =>
  navigator.clipboard?.writeText(id).then(() => toast({ title: "Session id copied", description: id, icon: <Copy />, tone: "info", duration: 2000 }));

function RowMenu({ id, onOpen }: { id: string; onOpen: () => void }) {
  return (
    <Menu.Root>
      <Menu.Trigger className="icon-btn" aria-label={`Actions for session ${id}`}><MoreHorizontal size={16} /></Menu.Trigger>
      <Menu.Portal>
        <Menu.Positioner sideOffset={4} align="end" className="z-50">
          <Menu.Popup className="pop w-44 rounded-[var(--radius-control)] border border-line bg-surface p-1 shadow-[var(--shadow-pop)] outline-none">
            <Menu.Item onClick={onOpen} className="cursor-pointer rounded-lg px-3 py-2 outline-none data-[highlighted]:bg-surface-2">Open session</Menu.Item>
            <Menu.Item onClick={() => copyId(id)} className="cursor-pointer rounded-lg px-3 py-2 outline-none data-[highlighted]:bg-surface-2">Copy session id</Menu.Item>
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}
