"use client";

import { Suspense, useEffect } from "react";
import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { ArrowLeft, ChevronLeft, ChevronRight } from "lucide-react";
import { useUsage } from "@/lib/app-state";
import { recordOpened } from "@/lib/recent";
import { counted, useLog } from "@/lib/log";
import { defaultOrder, useSessions } from "@/components/SessionTable";
import { RankList, Stat, useInView } from "@/components/overview/widgets";
import { CountUp, Empty, Loading, ModelPill, OfflineNote, Skeleton, StatusPill, UsageScore } from "@/components/ui";
import { fmtTok, hhmm, levelOf, modelLabel, sessionName, type Session } from "@/lib/usage";

function Breakdown({ current, limit }: { current: Session; limit: number }) {
  const router = useRouter();
  const [ref, seen] = useInView<HTMLElement>(0.25);
  const rank = (m: Record<string, number>, hrefKey: "model" | "project") =>
    Object.entries(m).sort((a, b) => b[1] - a[1]).map(([k, n], _i, all) => ({
      key: k, label: hrefKey === "model" ? modelLabel(k) : k, value: n, share: n / all[0][1], text: fmtTok(n),
      go: () => router.push(`/log?${hrefKey}=${encodeURIComponent(k)}&session=${current.id}`),
    }));
  return (
    <section ref={ref} className="card enter">
      <div className="card-head flex-wrap">
        <h2 className="text-[18px] font-semibold leading-6 tracking-[-0.01em]">{sessionName(current)}</h2>
        <StatusPill level={levelOf(current.tokens, limit)} active={current.active} />
      </div>
      <div className="grid gap-px border-b border-line bg-line @2xl:grid-cols-3">
        <Stat label="Tokens counted" value={<CountUp to={current.tokens} />} sub={<>of {fmtTok(limit)} in a 5h session</>} />
        <Stat label="Of the limit" value={<UsageScore used={current.tokens} limit={limit} />} sub={current.active ? "still running" : "closed"} />
        <Stat label="Messages" value={<CountUp to={current.messages} />} sub="requests in this window" />
      </div>
      <div className="grid content-start gap-6 p-5 @3xl:grid-cols-2">
        <RankList title="By model" note="tokens counted" seen={seen} rows={rank(current.models, "model")} />
        <RankList title="By project" note="tokens counted" seen={seen} rows={rank(current.projects, "project")} />
      </div>
    </section>
  );
}

function SessionPage() {
  const { id } = useParams<{ id: string }>();
  const { load, usage, offline, reload } = useUsage();
  const qs = useSessions();
  const router = useRouter();
  const order = defaultOrder(qs.rows);
  const at = order.findIndex((s) => s.id === id);
  const go = (to: number) => order[to] && router.replace(`/session/${order[to].id}${qs.search}`);
  const current = usage?.sessions.find((s) => s.id === id);
  const { page, error } = useLog(`session=${encodeURIComponent(id)}&limit=100`);

  useEffect(() => {
    if (current) recordOpened(id);
  }, [id, current]);

  // J / K walk the filtered list without going back to it.
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey || /INPUT|TEXTAREA|SELECT/.test((e.target as HTMLElement).tagName)) return;
      if (document.querySelector("dialog[open]")) return;
      const k = e.key.toLowerCase();
      if (k === "j") go(at + 1);
      if (k === "k") go(at - 1);
    };
    addEventListener("keydown", on);
    return () => removeEventListener("keydown", on);
  });

  if (load === "loading") return <Loading><Skeleton className="h-[70vh]" /></Loading>;
  const limit = usage?.config.limit5h ?? 1;
  return (
    <div className="grid gap-5">
      {offline && <OfflineNote onRetry={reload} />}
      <div className="flex items-center justify-between gap-3">
        <Link href={`/${qs.search}`} className="btn btn-ghost btn-sm"><ArrowLeft size={14} aria-hidden />Sessions</Link>
        {at >= 0 && (
          <span className="flex items-center gap-1 text-[13px] text-ink-3">
            <span className="num">{at + 1} of {order.length}</span>
            <button className="icon-btn size-9" disabled={at <= 0} onClick={() => go(at - 1)} aria-label="Previous session (K)"><ChevronLeft size={16} /></button>
            <button className="icon-btn size-9" disabled={at >= order.length - 1} onClick={() => go(at + 1)} aria-label="Next session (J)"><ChevronRight size={16} /></button>
          </span>
        )}
      </div>
      {current ? (
        <>
          <Breakdown current={current} limit={limit} />
          <section className="card enter">
            <div className="card-head flex-wrap">
              <h2 className="text-[18px] font-semibold leading-6 tracking-[-0.01em]">Messages</h2>
              <Link href={`/log?session=${id}`} className="text-[13px] text-burgundy hover:underline">Open in the log</Link>
            </div>
            {error ? <Empty title="Couldn’t load the messages" hint="Check that python server.py is running." />
              : !page ? <Skeleton className="m-4 h-48" />
              : (
                <div className="overflow-x-auto">
                  <table className="w-full text-[13px]">
                    <thead className="border-b border-line text-left">
                      <tr><th scope="col" className="label h-10 pl-5">Time</th><th scope="col" className="label">Model</th><th scope="col" className="label">Project</th><th scope="col" className="label pr-5 text-right">Counted</th></tr>
                    </thead>
                    <tbody>
                      {page.rows.map((r) => (
                        <tr key={`${r.ts}-${r.model}-${r.input}-${r.output}`} className="h-11 border-b border-line last:border-0 hover:bg-surface-2">
                          <td className="num whitespace-nowrap pl-5 pr-4">{hhmm(r.ts)}</td>
                          <td className="pr-3"><ModelPill model={r.model} /></td>
                          <td className="num max-w-[240px] truncate pr-4" title={r.project}>{r.project}</td>
                          <td className="num pr-5 text-right font-semibold">{fmtTok(counted(r))}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {page.total > page.rows.length && <p className="border-t border-line px-5 py-3 text-[12px] text-ink-3">Newest {page.rows.length} of {page.total}. Open the log for the rest.</p>}
                </div>
              )}
          </section>
        </>
      ) : (
        <div className="card"><Empty title={`No session ${id}`} hint="The link may be from before the logs were cleaned. Pick a session from the list." /></div>
      )}
    </div>
  );
}

export default function SessionRoute() {
  return <Suspense fallback={<Skeleton className="h-[70vh]" />}><SessionPage /></Suspense>;
}
