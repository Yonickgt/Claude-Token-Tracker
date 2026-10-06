const KEY = "claude-tracker.recent";
export type Recent = { id: string; at: number };

export function readRecent(): Recent[] {
  try {
    const v = JSON.parse(sessionStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}

/** Newest first, one entry per session, keep the last 3. */
export function recordOpened(id: string) {
  try {
    const next = [{ id, at: Date.now() }, ...readRecent().filter((r) => r.id !== id)].slice(0, 3);
    sessionStorage.setItem(KEY, JSON.stringify(next));
  } catch {}
}
