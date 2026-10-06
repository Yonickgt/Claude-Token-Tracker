import { useSyncExternalStore, type ReactNode } from "react";

/* Tiny toast store: call toast() from anywhere (state actions, click handlers); <Toaster /> in the shell renders them.
   Tone picks the fuse colour: ok = done, review = handed to a person (orange stays reserved for that), info = neutral. */
export type Toast = {
  id: number;
  title: ReactNode;
  description?: ReactNode;
  icon?: ReactNode;
  tone?: "ok" | "review" | "info";
  actionLabel?: string;
  onAction?: () => void;
  duration?: number;
};

const MAX = 4; // ponytail: oldest drops past 4; a real queue if bursts ever matter
let list: Toast[] = [];
let next = 0;
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((l) => l());

export function toast(t: Omit<Toast, "id">) {
  list = [...list, { ...t, id: ++next }].slice(-MAX);
  emit();
}
export function dismissToast(id: number) {
  list = list.filter((t) => t.id !== id);
  emit();
}

const EMPTY: Toast[] = [];
export const useToasts = () =>
  useSyncExternalStore(
    (l) => { listeners.add(l); return () => { listeners.delete(l); }; },
    () => list,
    () => EMPTY,
  );
