# Research: accurate Claude usage-left across devices (±1%)

## Problem
`tracker.py` estimates usage by summing tokens from local `~/.claude/projects/**/*.jsonl` against hand-calibrated limits (`config.json`). It cannot be ±1% because:
1. **Other devices** and **claude.ai / desktop chats** leave no local log (already admitted in the `DEFAULTS` comment).
2. **Limits aren't published in tokens**; the real meter is server-side and weights models/cache differently than `input+output+cache_creation`.
3. `limit5h`/`limitWeek` drift whenever Anthropic changes limits or the model mix changes.

## Key insight
The real number already exists server-side as a **percentage per account**. Read it, don't rebuild it. It is account-wide, so every device sees the same value; no cross-device log sync is needed for accuracy.

## What Claude Code leaves on disk (verified on this machine, v2.1.292)

| Record | What it holds | Per-session total? | Useful for |
|---|---|---|---|
| `~/.claude/projects/<proj>/<sessionId>.jsonl` | One line per message. Assistant lines carry `message.usage` (`input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `cache_creation.{5m,1h}`, `output_tokens_details.thinking_tokens`), plus `model`, `requestId`, `timestamp`, `sessionId`, `cwd`, `effort` | **No total field.** Sum it yourself, dedupe on `(message.id, requestId)` (what `tracker.py` does) | Exact per-session/per-model/per-project token detail, this device only |
| `~/.claude/stats-cache.json` | `dailyActivity`, `dailyModelTokens`, `modelUsage`, `totalSessions`, `longestSession`, `hourCounts` | Per day/model, not per session; lazily recomputed (last: 2026-09-20, stale) | Cheap history; don't rely on freshness |
| `~/.claude/history.jsonl` | Prompt history | No | Nothing for tokens |
| `~/.claude/session-data/*.tmp`, `sessions/`, `metrics/`, `telemetry/` | Session housekeeping / third-party (ECC) logs | Not token totals | Not worth parsing |
| `~/.claude/.credentials.json` | OAuth token for `/api/oauth/usage` | n/a | Auth for the poller. **Never log or commit.** |
| **statusLine stdin JSON** (live, not on disk unless your script saves it) | `session_id`, `model`, `transcript_path`, `cost.{total_cost_usd,total_duration_ms,...}`, `context_window.{total_input_tokens,total_output_tokens,used_percentage,current_usage}`, **`rate_limits.{five_hour,seven_day}.{used_percentage,resets_at}`** | **Yes: running per-session totals + cost** | Best hook: a tiny script can append one snapshot per refresh to a local log |

Takeaway: Claude Code does **not** persist a per-session total; the jsonl is the ledger and the statusLine JSON is the only place the live per-session total and the real account percentage appear together. Local jsonl is blind to other devices and claude.ai; only the percentage fixes that.

## Sources of truth for "% left" (best → worst)

| # | Source | Gives | Pros | Cons |
|---|--------|-------|------|------|
| 1 | **statusLine `rate_limits`** | exact % + reset, 5h and 7d | Official, free, same meter as web chats | Only fires while Claude Code runs on that device; Pro/Max only; absent before first response |
| 2 | **`GET https://api.anthropic.com/api/oauth/usage`** (`Authorization: Bearer <oauth token>`, `anthropic-beta: oauth-2025-04-20`) | utilization + reset per bucket | Poll anytime, even idle; same data as `/usage` | Undocumented; may change; poll gently |
| 3 | **Response headers** `anthropic-ratelimit-unified-{5h,7d}-utilization` / `-reset` | same % | Free on every subscription request | Needs a proxy to see them; no gain over #1/#2 |
| 4 | Local jsonl token sum (current) | tokens | Per-message detail, cost | Blind to other devices/web; limits are guesses |

## Recommended design
- **Truth layer:** statusLine script (#1) appends `{ts, five_hour_pct, five_hour_reset, week_pct, week_reset, session_id, session_tokens, cost}` to a local `usage-snapshots.jsonl`; optional poller (#2) fills gaps when Claude Code is idle. Local log is fine.
- **Detail layer:** keep the jsonl scan for model/project/cost breakdowns. Descriptive, not the headline.
- **Between snapshots:** interpolate with local tokens × `Δpct/Δtokens` from the last two snapshots; re-fit each time, so limit drift self-corrects (drops the manual `limit5h/limitWeek`).
- **Window bounds:** use `resets_at` (replaces manual `sessionEnd`, `weekDay`, `weekHour`).
- **Cross-device:** each device runs the same snapshotter against the same account. Merge logs only if you want shared history.

## Accuracy budget
- Source is a float %, so quantisation is far below 1%.
- Real error is staleness: at ~1–2 %/min burn, a 60 s-old snapshot is ≈1–2 % off. Poll 15–30 s above 80 %, slower when idle; interpolation shrinks the rest.
- ±1% holds when snapshot age < ~1 min or interpolation is active.

## Risks / to verify before building
1. Confirm `/api/oauth/usage` response field names on this account (incl. per-model weekly buckets).
2. OAuth token expires; read it fresh each poll, never cache or log it.
3. Verify statusLine `rate_limits` moves after a claude.ai web chat (expected yes).
4. Undocumented endpoint: start at ≥30 s interval.
5. API-key (non-subscription) accounts get none of this.

## Next steps
1. Spike: statusLine script that dumps raw stdin JSON to a file; confirm `rate_limits` vs `/usage` within ±1%.
2. Spike: 20-line `/api/oauth/usage` call, compare the same way.
3. Make the snapshot % the headline in `tracker.py`/`server.py`; keep token sums as detail + interpolation.
4. Remove manual limit/`sessionEnd` config once auto-fit works.

## Sources
- [Claude Code issue #45133 (statusline rate_limits)](https://github.com/anthropics/claude-code/issues/45133)
- [Showing 5h/7d limits in the status bar](https://skoula.cz/blog/2026/6/how-to-display-context-and-5h-7d-limits-in-the-claude-code-status-bar/)
- [Claude Code usage-limit status line tip](https://wmedia.es/en/tips/claude-code-usage-limit-status-line)
- [cc-statusline: Claude Code JSON fields](https://www.mintlify.com/chongdashu/cc-statusline/advanced/claude-code-json)
- [ccusage statusline](https://www.mintlify.com/ryoppippi/ccusage/features/statusline)
- [ccusage (PyPI)](https://pypi.org/project/ccusage/)
- [CodexBar issue #1894](https://github.com/steipete/CodexBar/issues/1894)
- [Per-session usage feature request #29721](https://claudeissues.com/issue/29721-feature-request-per-session-usage-contribution-to-5h-and-7d-rate-limit-windows)
- [Rate-limit headers explained](https://www.ssdnodes.com/learn/claude-api-rate-limit-headers-explained)
