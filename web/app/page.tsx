"use client";

import { Suspense } from "react";
import { RefreshCw } from "lucide-react";
import { HeaderActions } from "@/components/shell/Shell";
import { SessionStatusRow, SessionTable } from "@/components/SessionTable";
import { Loading, OfflineNote, Skeleton, useNow } from "@/components/ui";
import { RecentlyOpened } from "@/components/RecentlyOpened";
import { useUsage } from "@/lib/app-state";
import { fmtLeft, fmtTok } from "@/lib/usage";

export default function SessionsPage() {
  return <Suspense fallback={<Skeleton className="h-[420px]" />}><Sessions /></Suspense>;
}

function Sessions() {
  const { load, offline, reload, usage, summary, refresh } = useUsage();
  const now = useNow(30_000);

  if (load === "loading") {
    return (
      <Loading>
        <Skeleton className="h-20" />
        <Skeleton className="h-[420px]" />
      </Loading>
    );
  }
  const s = usage?.session;
  const w = usage?.week;
  return (
    <div className="grid gap-5">
      <header className="flex flex-wrap items-start justify-between gap-4">
        {usage && summary && w && (
          <p className="max-w-3xl text-[14px] text-ink-2" aria-live="polite">
            {s
              ? <>5h session: <b className="num font-semibold text-ink">{summary.sessionLeft}% left</b>, about <span className="num">{fmtTok(Math.max(0, s.limit - s.used))}</span> tokens, resets in <span className="num">{fmtLeft(s.end - now)}</span>. </>
              : <>No session running; the next message starts a new 5h window. </>}
            Week: <b className="num font-semibold text-ink">{summary.weekLeft}% left</b>, resets in <span className="num">{fmtLeft(w.end - now)}</span>.
          </p>
        )}
        <HeaderActions>
          <button className="btn btn-primary" onClick={() => void refresh()}>
            <RefreshCw size={16} aria-hidden /> Re-read logs
          </button>
        </HeaderActions>
      </header>

      {offline && <OfflineNote onRetry={reload} />}
      {usage && (
        <>
          <RecentlyOpened />
          <SessionStatusRow />
          <SessionTable />
        </>
      )}
    </div>
  );
}
