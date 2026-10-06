"use client";

import Link from "next/link";
import dynamic from "next/dynamic";
import { USER, useUsage } from "@/lib/app-state";
import { BurnRate, CostEstimate, TrendChart, WeekLeft, WhereTokensGo } from "@/components/overview/widgets";
import { CountUp, Loading, OfflineNote, Skeleton, useNow } from "@/components/ui";
import { cn } from "@/lib/utils";
import { fmtLeft, fmtTok } from "@/lib/usage";

// recharts is ~330 KB: load it after the page shell, behind a same-size card so nothing jumps.
const chartCard = (h: number) => () => <section className="card" style={{ minHeight: h }} aria-busy />;
const PipelineFlow = dynamic(() => import("@/components/overview/charts").then((m) => m.PipelineFlow), { ssr: false, loading: chartCard(280) });
const ModelMix = dynamic(() => import("@/components/overview/charts").then((m) => m.ModelMix), { ssr: false, loading: chartCard(260) });

/* Ordered the way you read it, most actionable first:
   what is left right now → where the tokens went and what it is worth → volume over time → will the pace make it. */
export default function Usage() {
  const { usage, load, offline, reload } = useUsage();
  if (load === "loading") return <Loading><Skeleton className="h-28" /><Skeleton className="h-72" /></Loading>;
  if (!usage) return <OfflineNote onRetry={reload} />;
  return (
    <div className="grid gap-5">
      {offline && <OfflineNote onRetry={reload} />}
      {/* What is left (the headline) on the left, where the tokens went on the right. */}
      <div className="grid gap-5 @4xl:grid-cols-2">
        <Welcome />
        <PipelineFlow />
      </div>
      {/* The two headline numbers stack on the left; the breakdown card spans both rows on the right and shares them
          (subgrid), so the week card ends level with its stat strip and the cost card with its bar lists. */}
      <div className="grid gap-5 @4xl:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]">
        <WeekLeft />
        <CostEstimate />
        <WhereTokensGo />
      </div>
      <TrendChart />
      <div className="grid gap-5 @4xl:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)]">
        <BurnRate />
        <ModelMix />
      </div>
    </div>
  );
}

/** The page's one claim, stated once and biggest: how much of the 5h session is left. The greeting stays, but small. */
function Welcome() {
  const { usage, summary } = useUsage();
  const now = useNow(30_000);
  const s = usage!.session;
  const left = summary!.sessionLeft;
  const tone = left < 20 ? "text-defect" : left < 50 ? "text-review-ink" : "text-ok";
  return (
    <section className="card enter flex flex-col justify-between gap-6 p-6" aria-label="What is left">
      <div className="grid gap-2">
        <p className="text-[14px] text-ink-2">Welcome back, {USER.name}</p>
        <div className={cn("text-[96px] font-semibold leading-[96px] tracking-[-0.04em]", tone)}><CountUp to={left} suffix="%" /></div>
        {/* The sentence is set as a heading so the number reads as a claim, not one stat among the page's others. */}
        {s ? (
          <>
            <p className="text-[26px] font-semibold leading-8 tracking-[-0.01em]">
              <span className="text-ink-2">of <span className="num">{fmtTok(s.limit)}</span></span> tokens left in this 5h session.
            </p>
            <p className="text-[14px] text-ink-2">
              About <span className="num">{fmtTok(Math.max(0, s.limit - s.used))}</span> left. Resets in <span className="num">{fmtLeft(s.end - now)}</span>.
            </p>
          </>
        ) : (
          <>
            <p className="text-[26px] font-semibold leading-8 tracking-[-0.01em]">of your next 5h session is available.</p>
            <p className="text-[14px] text-ink-2">No session is running. The clock starts with your next message.</p>
          </>
        )}
      </div>
      <Link href={s ? "/?scope=active" : "/"} className="btn btn-primary self-start">Open sessions</Link>
    </section>
  );
}
