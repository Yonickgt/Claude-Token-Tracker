"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { Timer } from "lucide-react";
import { useUsage } from "@/lib/app-state";
import { readRecent, type Recent } from "@/lib/recent";
import { ddmmyy, fmtTok, levelOf, sessionName } from "@/lib/usage";
import { StatusPill } from "@/components/ui";
import BorderGlow from "@/components/BorderGlow";

/** The last three sessions this person opened, newest first. Hidden until there is one. */
export function RecentlyOpened() {
  const { usage } = useUsage();
  const [recent, setRecent] = useState<Recent[]>([]);
  useEffect(() => setRecent(readRecent()), []);
  const items = recent.flatMap((r) => {
    const s = usage?.sessions.find((x) => x.id === r.id);
    return s ? [{ s, at: r.at }] : [];
  });
  if (!items.length) return null;
  return (
    <section aria-label="Recently opened" className="grid gap-2">
      <h2 className="label">Recently opened</h2>
      <ul className="grid gap-3 min-[700px]:grid-cols-3">
        {items.map(({ s, at }) => (
          <li key={s.id} className="min-w-0">
            <BorderGlow className="recent-glow" backgroundColor="var(--recent-bg)" glowColor="190 70 70" colors={["#86b7c0", "#3f8794", "#e08a00"]}
              borderRadius={16} glowRadius={28} edgeSensitivity={30} coneSpread={25} fillOpacity={0.35}>
              <Link href={`/session/${s.id}`} className="flex min-w-0 flex-col gap-2 p-4 outline-none focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-burgundy">
                <span className="truncate font-semibold text-[var(--recent-ink)]">{sessionName(s)}</span>
                <span className="flex items-center gap-2 text-[12px] text-[var(--recent-ink-3)]">
                  <Timer size={14} aria-hidden className="shrink-0" />
                  <span className="num truncate">{fmtTok(s.tokens)} · Opened {ddmmyy(at)}</span>
                  <span className="ml-auto shrink-0"><StatusPill level={levelOf(s.tokens, usage!.config.limit5h)} active={s.active} /></span>
                </span>
              </Link>
            </BorderGlow>
          </li>
        ))}
      </ul>
    </section>
  );
}
