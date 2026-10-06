"use client";

import { useEffect, useState } from "react";
import type { LogRow } from "./usage";

export type LogPage = { total: number; rows: LogRow[] };

/** Rows from /api/log for a query string. `error` is set when the backend is unreachable, so a page can say so. */
export function useLog(qs: string) {
  const [page, setPage] = useState<LogPage | null>(null);
  const [error, setError] = useState(false);
  useEffect(() => {
    let live = true;
    setError(false);
    fetch(`/api/log?${qs}`, { cache: "no-store", signal: AbortSignal.timeout(20_000) })
      .then((r) => (r.ok ? (r.json() as Promise<LogPage>) : Promise.reject(new Error(String(r.status)))))
      .then((p) => live && setPage(p))
      .catch(() => live && setError(true));
    return () => { live = false; };
  }, [qs]);
  return { page, error, loading: !page && !error };
}

export const counted = (r: LogRow) => r.input + r.output + r.cacheWrite;
