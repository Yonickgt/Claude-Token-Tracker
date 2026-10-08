# Claude Token Tracker

Shows how much of your Claude **5-hour session** and **weekly** limit you have used, plus per-model, per-project and cost breakdowns. Three front ends share one engine:

| Part | What it is | Start it |
|---|---|---|
| `tracker.py` | Core engine + terminal view | `python tracker.py` (or `--watch` to refresh every 30 s) |
| `server.py` + `web/` | JSON API on `127.0.0.1:8787` + Next.js dashboard on `localhost:3000` | double-click `start.bat` |
| `widget/` | Native Windows desktop widget (Rust, same maths as `tracker.py`) | `cd widget && cargo run --release` |

Requirements: Python 3.10+, Node (dashboard), Rust (widget, optional). You must be logged in to Claude Code (the live meter reads `~/.claude/.credentials.json`).

## How to use

1. Run `start.bat`. It starts the API and builds/starts the dashboard at <http://localhost:3000>.
2. Pages: **Overview** (session + week meters, charts), **Usage**, **Log** (every message, filterable), **Session/[id]** (one 5-hour block).
3. Calibration is automatic. Within about 30 seconds of the first run the tracker has a live reading and sets limits and reset times itself. `config.json` is only the fallback used when you are offline or logged out (edit it, or use the dashboard's Calibrate dialog, which POSTs to `/api/config`).

## Where each value comes from

There are two independent sources. Know which one a number comes from.

### Source A: local logs (exact, this PC only)

`~/.claude/projects/**/*.jsonl`, one JSON object per line. Each assistant line has `message.usage`. `load()` in `tracker.py` reads them.

| Value | How it is extracted |
|---|---|
| Input / output tokens | `usage.input_tokens`, `usage.output_tokens` |
| Cache write | `usage.cache_creation_input_tokens` |
| Cache read | `usage.cache_read_input_tokens` (shown separately, **not** counted toward limits) |
| **Counted tokens (`tok`)** | `input + output + cache_write`. This is what the limit maths uses. |
| Model | `message.model`. Rows whose model starts with `<` (synthetic) or with all-zero usage are dropped. |
| Timestamp | `timestamp` (UTC ISO 8601) |
| Project | basename of the first `cwd` seen in that conversation file (falls back to the last chunk of the log folder name) |
| Duplicates | Claude Code logs one streamed message several times. Rows are keyed on `(message.id, requestId)` and counted once. **The largest reading is kept**, because `output_tokens` grows as the stream progresses. |
| Cost (USD) | tokens x list price per model (`PRICES`, first substring match wins: input, output, cache write, cache read per million). An API-price estimate, **not** your subscription bill. Unknown models cost 0. |
| Messages | number of deduplicated rows |

### Source B: Anthropic account meter (the real percentage, all devices)

`GET https://api.anthropic.com/api/oauth/usage` with your OAuth token, polled at most every 30 s (`fetch()`). It returns `five_hour` and `seven_day`, each with `utilization` (percent) and `resets_at`. This covers claude.ai chats and other devices, which leave no local log. It is an **undocumented** endpoint and could change; on any failure the tracker silently keeps the last reading.

| Value | How it is derived |
|---|---|
| **% used (5h and week)** | `utilization` straight from the API. Zero if `resets_at` is already in the past. |
| Reset times | `resets_at`. The weekly reset also sets the week's weekday and hour, and the 5h reset pins the session window (replaces `weekDay`, `weekHour`, `sessionEnd`). |
| **Token limit** | Each reading is saved to `usage-snapshots.jsonl` (widget: `%APPDATA%\ClaudeTokenWidget\readings.jsonl`) together with this PC's counted tokens in that window. Limit = `tokens x 100 / pct`; the **maximum** over the last 300 readings with pct >= 10 wins. |
| **"Used" tokens shown** | `pct / 100 x limit` when a live percentage exists, otherwise this PC's own counted tokens. |
| 5h session window | Blocks of 5 h starting at the first message, floored to the hour; the live block is replaced by the exact `[reset-5h, reset]` window from the API. |
| Weekly window | Latest local `weekDay` at `weekHour` (Mon=0), 7 days long. Week totals, models, projects, cost and input/output/cache splits all sum rows inside it. |
| Burn rate / ETA (widget) | used tokens / hours since session start (min 15 min), ETA = remaining / burn rate. |
| Cache hit (widget) | `cache_read / (cache_read + cache_write + input)` over the week. |

## Accuracy

- The percentages and reset times come straight from your Anthropic account and match `/usage`.
- Token counts, models, projects and cost are exact for this PC only. Other devices and claude.ai chats show up in the percentage but not in the token breakdowns.
- The token limit is derived from the percentage, so it is an estimate and reads low if other devices used part of your quota.
- The percentage is a whole number, so expect up to about 1% rounding.
- The usage endpoint is undocumented. If you are offline or logged out, the tracker falls back to local counts and the limits in `config.json`; open Claude Code to refresh your login.
- Prices in `PRICES` (`tracker.py`, `widget/src/tokens.rs`) are list prices, an API-cost estimate and not your subscription bill.

## Installing and updating the widget

| Way | How |
|---|---|
| Installer | Download `ClaudeTokenWidget-<version>-Setup.exe` from the [latest release](https://github.com/Yonickgt/Claude-Token-Tracker/releases/latest) and run it. No admin needed. |
| PowerShell (needs Git + Rust) | `irm https://raw.githubusercontent.com/Yonickgt/Claude-Token-Tracker/main/install.ps1 \| iex` (run it again to update) |

**Update button:** the widget checks the latest GitHub Release on start and every 6 h. A green **Update** button appears in the header only when a newer version exists; clicking it downloads the installer, runs it silently and relaunches the widget.
