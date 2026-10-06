"""Claude token tracker core + terminal view: python tracker.py [--watch]"""
import glob, json, os, sys, time
from collections import defaultdict
from datetime import datetime, timedelta, timezone
from typing import NamedTuple

HERE = os.path.dirname(os.path.abspath(__file__))
CFG_PATH = os.path.join(HERE, "config.json")
# Calibration knobs: Anthropic doesn't publish plan limits in tokens. Set them from your own observed 100%
# (tokens = input + output + cache_creation, see tok()). weekDay: Mon=0, weekHour: local hour of your weekly reset.
# sessionEnd: epoch ms of the reset time /usage shows for the current session (0 = derive from the logs).
# Needed because claude.ai chats share the window but leave no local log, so the true window can start earlier than the first local message.
DEFAULTS = {"limit5h": 5_000_000, "limitWeek": 50_000_000, "weekDay": 4, "weekHour": 9, "sessionEnd": 0}

# USD per million tokens: (input, output, cache write 5m, cache read). First substring match wins.
# ponytail: list prices as cached 2026-09-25 (claude-api skill); API-equivalent estimate only, not your plan's bill.
PRICES = [
    ("fable", (10, 50, 12.5, 0.25)), ("mythos", (10, 50, 12.5, 0.25)),
    ("opus-5-5", (4, 20, 5, 0.20)), ("opus", (5, 25, 6.25, 0.50)),
    ("sonnet-5", (2, 10, 2.5, 0.20)), ("sonnet", (3, 15, 3.75, 0.30)),
    ("haiku", (1, 5, 1.25, 0.10)),
]

class Row(NamedTuple):
    ts: datetime
    model: str
    tok: int          # input + output + cache_creation: what counts against limits
    cache_read: int
    project: str
    inp: int
    out: int
    cw: int

def cfg():
    try:
        return {**DEFAULTS, **json.load(open(CFG_PATH))}
    except (OSError, ValueError):
        return dict(DEFAULTS)

def cost(r):
    p = next((v for k, v in PRICES if k in r.model), None)
    return 0.0 if p is None else (r.inp * p[0] + r.out * p[1] + r.cw * p[2] + r.cache_read * p[3]) / 1e6

def load():
    seen, rows = set(), []
    for f in glob.glob(os.path.expanduser("~/.claude/projects/**/*.jsonl"), recursive=True):
        project = os.path.basename(os.path.dirname(f)).split("-")[-1] or "?"
        root = None  # the first cwd in a conversation is its project root; later ones drift into subfolders
        for line in open(f, encoding="utf-8", errors="ignore"):
            try:
                d = json.loads(line)
                if root is None and d.get("cwd"):
                    root = os.path.basename(d["cwd"].replace("\\", "/").rstrip("/")) or None
                m, u = d["message"], d["message"]["usage"]
                key = (m.get("id"), d.get("requestId"))
                if key[0] and key in seen:
                    continue
                seen.add(key)
                inp, out, cw = u.get("input_tokens", 0), u.get("output_tokens", 0), u.get("cache_creation_input_tokens", 0)
                cr = u.get("cache_read_input_tokens", 0)
                model = m.get("model", "?")
                if model.startswith("<") or not (inp + out + cw + cr):
                    continue
                ts = datetime.fromisoformat(d["timestamp"].replace("Z", "+00:00"))
                rows.append(Row(ts, model, inp + out + cw, cr, root or project, inp, out, cw))
            except (KeyError, ValueError, TypeError, AttributeError):
                pass
    return sorted(rows)

def pin(c):
    e = c.get("sessionEnd") or 0
    return (datetime.fromtimestamp(e / 1000 - 5 * 3600, timezone.utc), datetime.fromtimestamp(e / 1000, timezone.utc)) if e else None

def blocks(rows, pinned=None):
    """5h session blocks as [start, end, rows]: a block starts at its first message (floored to the hour),
    except the pinned window, whose real bounds come from /usage."""
    out = []
    for r in rows:
        if pinned and pinned[0] <= r.ts < pinned[1]:
            if out and out[-1][0] == pinned[0]:
                out[-1][2].append(r)
            else:
                out.append([pinned[0], pinned[1], [r]])
        elif out and r.ts < out[-1][1]:
            out[-1][2].append(r)
        else:
            s = r.ts.replace(minute=0, second=0, microsecond=0)
            out.append([s, s + timedelta(hours=5), [r]])
    return out

def week_start(now, c):
    local = now.astimezone().replace(minute=0, second=0, microsecond=0, hour=c["weekHour"])
    local -= timedelta(days=(local.weekday() - c["weekDay"]) % 7)
    if local > now.astimezone():
        local -= timedelta(days=7)
    return local.astimezone(timezone.utc)

def by(rows, key):
    out = defaultdict(int)
    for r in rows:
        out[key(r)] += r.tok
    return dict(out)

# ---- terminal view -------------------------------------------------------------------------
def bar(p, w=30):
    n = round(max(0, min(1, p)) * w)
    return "█" * n + "░" * (w - n)

def fmt(n):
    return f"{n/1e6:.2f}M" if n >= 1e6 else f"{n/1e3:.1f}K"

def left(td):
    s = max(0, int(td.total_seconds()))
    return f"{s//3600}h {s%3600//60:02d}m"

def panel(title, rows, limit, end, now):
    used = sum(r.tok for r in rows)
    print(f"\n{title}  resets in {left(end - now)}")
    print(f"  {bar(used/limit)} {used/limit:.0%} used, {max(0, 1-used/limit):.0%} left")
    print(f"  used {fmt(used)} / {fmt(limit)}   expected left {fmt(max(0, limit - used))}")
    for m, t in sorted(by(rows, lambda r: r.model).items(), key=lambda x: -x[1]):
        print(f"    {m:<32}{fmt(t):>9}  {t/used:.0%}")

def render():
    c, now, rows = cfg(), datetime.now(timezone.utc), load()
    print(f"Claude usage  {now.astimezone():%a %H:%M}")
    b = blocks(rows, pin(c))
    if b and now < b[-1][1]:
        panel("5h SESSION", b[-1][2], c["limit5h"], b[-1][1], now)
    else:
        print("\n5h SESSION  no active session (starts on next message)")
    ws = week_start(now, c)
    panel("WEEK", [r for r in rows if r.ts >= ws], c["limitWeek"], ws + timedelta(days=7), now)
    print(f"\nall-time {fmt(sum(r.tok for r in rows))} tokens, cache reads {fmt(sum(r.cache_read for r in rows))} (not counted)")

if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")  # the bar glyphs fail on cp1252 pipes
    while True:
        if "--watch" in sys.argv:
            os.system("cls" if os.name == "nt" else "clear")
        render()
        if "--watch" not in sys.argv:
            break
        time.sleep(30)
