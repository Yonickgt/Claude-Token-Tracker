"use client";

import { useState } from "react";
import { Dropdown, Modal } from "@/components/ui";
import { useUsage } from "@/lib/app-state";
import { fmtTok, type Config } from "@/lib/usage";

const DAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"].map((label, i) => ({ value: String(i), label }));
const HOURS = Array.from({ length: 24 }, (_, h) => ({ value: String(h), label: `${String(h).padStart(2, "0")}:00` }));

/** Anthropic publishes no token limits, so the percentages are only as good as these numbers.
    Either type the limit, or say what Claude's own /usage shows right now and the limit is worked out from tokens used so far. */
export function Calibrate({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Modal open={open} onClose={onClose} title="Calibrate limits">
      <Form onClose={onClose} />
    </Modal>
  );
}

function Form({ onClose }: { onClose: () => void }) {
  const { usage, saveConfig } = useUsage();
  const c = usage!.config;
  const [f, setF] = useState({ limit5h: String(c.limit5h), limitWeek: String(c.limitWeek), weekDay: String(c.weekDay), weekHour: String(c.weekHour), pct5h: "", pctWeek: "", resetAt: "" });
  const [error, setError] = useState("");
  const set = (k: keyof typeof f) => (v: string) => setF((s) => ({ ...s, [k]: v }));
  const used5h = usage!.session?.used ?? 0;
  const usedWeek = usage!.week.used;

  /** Next occurrence of a local HH:MM, as epoch ms. */
  const nextAt = (hhmm: string) => {
    const [h, m] = hhmm.split(":").map(Number);
    const d = new Date();
    d.setHours(h, m, 0, 0);
    if (d.getTime() <= Date.now()) d.setDate(d.getDate() + 1);
    return d.getTime();
  };

  const submit = async () => {
    setError("");
    try {
      // Step 1: the anchors. Percentages depend on which window the tokens fall in, so they come from the re-read numbers.
      const anchors: Partial<Config> = { weekDay: Number(f.weekDay), weekHour: Number(f.weekHour), ...(f.resetAt ? { sessionEnd: nextAt(f.resetAt) } : {}) };
      const fresh = await saveConfig(anchors, true);
      const fromPct = (pct: string, used: number, typed: string) => (Number(pct) > 0 && used > 0 ? Math.round((used / Number(pct)) * 100) : Number(typed));
      const limits = { limit5h: fromPct(f.pct5h, fresh.session?.used ?? 0, f.limit5h), limitWeek: fromPct(f.pctWeek, fresh.week.used, f.limitWeek) };
      if (![limits.limit5h, limits.limitWeek].every((n) => Number.isFinite(n) && n > 0)) return setError("Limits must be positive numbers.");
      await saveConfig(limits);
      onClose();
    } catch (e) { setError(e instanceof Error ? e.message : "Couldn’t save."); }
  };

  const row = (label: string, key: "limit5h" | "limitWeek", pctKey: "pct5h" | "pctWeek", used: number) => (
    <fieldset className="grid gap-2 rounded-[var(--radius-control)] border border-line p-4">
      <legend className="label px-1">{label}</legend>
      <label className="grid gap-1 text-[13px]">Limit (tokens)
        <input className="field num" inputMode="numeric" value={f[key]} onChange={(e) => set(key)(e.target.value.replace(/[^\d]/g, ""))} />
      </label>
      <label className="grid gap-1 text-[13px]">
        <span>or: Claude’s /usage shows <span className="text-ink-3">% used right now</span> <span className="num text-ink-3">({fmtTok(used)} used so far)</span></span>
        <input className="field num w-32" inputMode="decimal" placeholder="e.g. 38" value={f[pctKey]} onChange={(e) => set(pctKey)(e.target.value.replace(/[^\d.]/g, ""))} />
      </label>
    </fieldset>
  );

  return (
    <div className="grid gap-4">
      <p className="text-ink-2">Anthropic doesn’t publish plan limits in tokens, so set them from what <span className="num">/usage</span> shows. Counted tokens are input + output + cache writes.</p>
      {row("5h session", "limit5h", "pct5h", used5h)}
      {row("Week", "limitWeek", "pctWeek", usedWeek)}
      <label className="grid gap-1 text-[13px]">
        <span>5h session resets at <span className="text-ink-3">(local time, from /usage; leave empty to use the logs)</span></span>
        <input className="field num w-32" type="time" value={f.resetAt} onChange={(e) => set("resetAt")(e.target.value)} />
      </label>
      <div className="flex flex-wrap items-end gap-3">
        <div className="grid gap-1 text-[13px]"><span>Weekly reset day</span><Dropdown label="Weekly reset day" value={f.weekDay} onChange={set("weekDay")} options={DAYS} /></div>
        <div className="grid gap-1 text-[13px]"><span>Hour (local)</span><Dropdown label="Weekly reset hour" value={f.weekHour} onChange={set("weekHour")} options={HOURS} /></div>
      </div>
      {error && <p role="alert" className="rounded-[var(--radius-control)] bg-defect-tint px-3 py-2 text-[13px] text-defect">{error}</p>}
      <div className="flex justify-end gap-2">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn btn-primary" onClick={submit}>Save limits</button>
      </div>
    </div>
  );
}
