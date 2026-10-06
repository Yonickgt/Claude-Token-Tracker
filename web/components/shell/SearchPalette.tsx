"use client";

import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { Command } from "cmdk";
import { Boxes, FolderClock, Layers, LayoutDashboard, Timer } from "lucide-react";
import { useUsage } from "@/lib/app-state";
import { readRecent } from "@/lib/recent";
import { FAMILIES, familyLabel, fmtTok, modelLabel, searchText, sessionName, topKey, type Session } from "@/lib/usage";
import { Modal } from "@/components/ui";

const PAGES = [
  { href: "/", label: "Sessions", Icon: Layers },
  { href: "/usage", label: "Usage", Icon: LayoutDashboard },
  { href: "/log", label: "Log", Icon: FolderClock },
];
const item = "flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2 data-[selected=true]:bg-surface-2";

/** One palette over pages, sessions, models and projects. cmdk does keyboard nav + a11y; the filter is ours
    so 500 sessions render 8 rows, not 500. */
export function SearchPalette({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { usage } = useUsage();
  const router = useRouter();
  const [q, setQ] = useState("");
  const [opened, setOpened] = useState<{ id: string; at: number }[]>([]);
  useEffect(() => { if (!open) setQ(""); else setOpened(readRecent()); }, [open]);
  const sessions = useMemo(() => usage?.sessions ?? [], [usage]);
  const limit = usage?.config.limit5h ?? 1;
  const recent = useMemo(() => opened.flatMap((r) => sessions.find((s) => s.id === r.id) ?? []), [sessions, opened]);
  const needle = q.trim().toLowerCase();
  const hits = useMemo(
    () => (needle ? sessions.filter((s) => searchText(s).includes(needle)).slice(0, 8) : sessions.filter((s) => s.tokens >= limit * 0.8).slice(0, 5)),
    [sessions, needle, limit],
  );
  const projects = useMemo(() => [...new Set(sessions.flatMap((s) => Object.keys(s.projects)))].filter((p) => needle && p.toLowerCase().includes(needle)).slice(0, 5), [sessions, needle]);
  const models = useMemo(() => [...new Set(sessions.flatMap((s) => Object.keys(s.models)))].filter((m) => needle && `${m} ${modelLabel(m)}`.toLowerCase().includes(needle)).slice(0, 5), [sessions, needle]);
  const families = FAMILIES.filter((f) => needle && familyLabel(f).toLowerCase().includes(needle));
  const pages = PAGES.filter((p) => !needle || p.label.toLowerCase().includes(needle));
  const go = (href: string) => { onClose(); router.push(href); };
  const row = (s: Session) => (
    <Command.Item key={s.id} value={`s:${s.id}`} onSelect={() => go(`/session/${s.id}`)} className={item}>
      <Timer size={16} aria-hidden className="shrink-0" />
      <span className="min-w-0 flex-1 truncate">{sessionName(s)}</span>
      <span className="num shrink-0 text-[12px] text-ink-3">{topKey(s.projects)}</span>
      <span className="num shrink-0 text-[12px] text-ink-3">{fmtTok(s.tokens)}</span>
    </Command.Item>
  );

  return (
    <Modal open={open} onClose={onClose} title="Search">
      <Command shouldFilter={false} label="Search" loop>
        <Command.Input value={q} onValueChange={setQ} placeholder="Search sessions, models, projects…" className="field mb-3 w-full" autoFocus />
        <Command.List className="max-h-80 overflow-y-auto">
          <Command.Empty className="px-3 py-6 text-center text-ink-2">Nothing matches “{q}”. Try a project name, a model, or a day like “Tue”.</Command.Empty>
          {pages.length > 0 && (
            <Command.Group heading={<span className="label px-3">Go to</span>}>
              {pages.map(({ href, label, Icon }) => (
                <Command.Item key={href} value={href} onSelect={() => go(href)} className={item}><Icon size={16} aria-hidden />{label}</Command.Item>
              ))}
            </Command.Group>
          )}
          {!needle && recent.length > 0 && <Command.Group heading={<span className="label px-3">Recent</span>}>{recent.map(row)}</Command.Group>}
          {hits.length > 0 && (
            <Command.Group heading={<span className="label px-3">{needle ? "Sessions" : "Near or over the limit"}</span>}>{hits.map(row)}</Command.Group>
          )}
          {(models.length > 0 || families.length > 0) && (
            <Command.Group heading={<span className="label px-3">Models</span>}>
              {models.map((m) => (
                <Command.Item key={m} value={`m:${m}`} onSelect={() => go(`/log?model=${encodeURIComponent(m)}`)} className={item}>
                  <Boxes size={16} aria-hidden className="shrink-0" />{modelLabel(m)}<span className="num text-[12px] text-ink-3">{m}</span>
                </Command.Item>
              ))}
              {families.map((f) => (
                <Command.Item key={f} value={`f:${f}`} onSelect={() => go(`/log?family=${f}`)} className={item}>
                  <Boxes size={16} aria-hidden className="shrink-0" />All {familyLabel(f)} models
                </Command.Item>
              ))}
            </Command.Group>
          )}
          {projects.length > 0 && (
            <Command.Group heading={<span className="label px-3">Projects</span>}>
              {projects.map((p) => (
                <Command.Item key={p} value={`p:${p}`} onSelect={() => go(`/log?project=${encodeURIComponent(p)}`)} className={item}>
                  <FolderClock size={16} aria-hidden className="shrink-0" /><span className="num truncate">{p}</span>
                </Command.Item>
              ))}
            </Command.Group>
          )}
        </Command.List>
      </Command>
    </Modal>
  );
}
