"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { Gauge, SlidersHorizontal } from "lucide-react";
import { LEVEL_WORD, levelOf, summarise, type Config, type Summary, type Usage } from "./usage";
import { toast } from "./toast";

/** Local single-user app: no login, so the "account" is just whoever runs the tracker. */
export const USER = { name: "Nicholas", role: "Local" };

type Load = "loading" | "ready";
type State = {
  usage: Usage | null;
  summary: Summary | null;
  load: Load;
  offline: boolean;
  syncedAt: number | null;
  reload: () => void;
  /** Ask the backend to re-read the logs, then refetch. */
  refresh: () => Promise<void>;
  /** Saves, re-reads, and resolves with the fresh numbers so a caller can chain a dependent save. */
  saveConfig: (patch: Partial<Config>, quiet?: boolean) => Promise<Usage>;
};
const Ctx = createContext<State | null>(null);
export const useUsage = () => useContext(Ctx)!;

const POLL_MS = 15_000;
const post = (url: string, body?: unknown) =>
  fetch(url, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body ?? {}) });

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [usage, setUsage] = useState<Usage | null>(null);
  const [offline, setOffline] = useState(false);
  const [syncedAt, setSyncedAt] = useState<number | null>(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let live = true;
    const pull = () =>
      fetch("/api/usage", { cache: "no-store", signal: AbortSignal.timeout(20_000) })
        .then((r) => (r.ok ? (r.json() as Promise<Usage>) : Promise.reject(new Error(`Backend answered ${r.status}`))))
        .then((u) => { if (live) { setUsage(u); setOffline(false); setSyncedAt(Date.now()); } })
        .catch((e) => { if (live) { setOffline(true); console.error("Usage request failed:", e); } });
    const pullIfVisible = () => document.visibilityState === "visible" && pull();
    pull();
    const i = setInterval(pullIfVisible, POLL_MS);
    // A tab that was hidden has stale numbers: refetch the moment it is looked at again.
    document.addEventListener("visibilitychange", pullIfVisible);
    addEventListener("focus", pullIfVisible);
    return () => { live = false; clearInterval(i); document.removeEventListener("visibilitychange", pullIfVisible); removeEventListener("focus", pullIfVisible); };
  }, [tick]);

  const summary = useMemo(() => (usage ? summarise(usage) : null), [usage]);

  // One toast per window per crossing: a session or week that goes near / over its limit tells you once.
  const told = useRef(new Set<string>());
  useEffect(() => {
    if (!usage || !summary) return;
    const check = (key: string, name: string, used: number, limit: number) => {
      const lv = levelOf(used, limit);
      if (lv === "ok" || told.current.has(`${key}:${lv}`)) return;
      told.current.add(`${key}:${lv}`);
      toast({ title: `${name}: ${LEVEL_WORD[lv].toLowerCase()}`, description: `${Math.round((used / limit) * 100)}% of the limit used`, icon: <Gauge />, tone: "review" });
    };
    if (usage.session) check(`s${usage.session.id}`, "5h session", usage.session.used, usage.session.limit);
    check(`w${usage.week.start}`, "Week", usage.week.used, usage.week.limit);
  }, [usage, summary]);

  const reload = useCallback(() => setTick((t) => t + 1), []);
  const refresh = useCallback(async () => { await post("/api/refresh").catch(() => {}); reload(); }, [reload]);
  const saveConfig = useCallback(async (patch: Partial<Config>, quiet = false) => {
    const r = await post("/api/config", patch);
    if (!r.ok) throw new Error((await r.json().catch(() => null))?.error ?? `Backend answered ${r.status}`);
    await post("/api/refresh").catch(() => {});
    const fresh = (await fetch("/api/usage", { cache: "no-store" }).then((x) => x.json())) as Usage;
    setUsage(fresh);
    told.current.clear();
    if (!quiet) toast({ title: "Limits saved", description: "Percentages now use your numbers", icon: <SlidersHorizontal />, tone: "info" });
    return fresh;
  }, []);

  const value = useMemo<State>(
    () => ({ usage, summary, load: usage || offline ? "ready" : "loading", offline, syncedAt, reload, refresh, saveConfig }),
    [usage, summary, offline, syncedAt, reload, refresh, saveConfig],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
