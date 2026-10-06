"""Backend for the dashboard: python server.py  ->  http://127.0.0.1:8787 (Next proxies /api here)."""
import json, time
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

import tracker as t

TTL = 5  # seconds; ponytail: re-reads every log per call, fine until logs reach the GBs (then index by mtime)
_cache = {"at": 0.0, "rows": []}

def rows():
    if time.time() - _cache["at"] > TTL:
        _cache.update(at=time.time(), rows=t.load())
    return _cache["rows"]

ms = lambda d: int(d.timestamp() * 1000)

def usage():
    c, now, rs = t.cfg(), datetime.now(timezone.utc), rows()
    bl = t.blocks(rs, t.pin(c))
    ws = t.week_start(now, c)
    wk = [r for r in rs if r.ts >= ws]

    def block(b):
        return {"id": str(ms(b[0])), "start": ms(b[0]), "end": ms(b[1]), "active": now < b[1], "messages": len(b[2]),
                "tokens": sum(r.tok for r in b[2]), "models": t.by(b[2], lambda r: r.model), "projects": t.by(b[2], lambda r: r.project)}

    hourly = {}
    for r in rs:
        k = (ms(r.ts.replace(minute=0, second=0, microsecond=0)), r.model)
        hourly[k] = hourly.get(k, 0) + r.tok
    flow = {}
    for r in wk:
        for kind, n in (("Input", r.inp), ("Output", r.out), ("Cache write", r.cw)):
            if n:
                k = (r.project, r.model, kind)
                flow[k] = flow.get(k, 0) + n
    cur = block(bl[-1]) if bl and now < bl[-1][1] else None
    return {
        "now": ms(now), "config": c,
        "session": cur and {**cur, "used": cur["tokens"], "limit": c["limit5h"]},
        "week": {"start": ms(ws), "end": ms(ws + timedelta(days=7)), "used": sum(r.tok for r in wk), "limit": c["limitWeek"],
                 "models": t.by(wk, lambda r: r.model), "projects": t.by(wk, lambda r: r.project),
                 "input": sum(r.inp for r in wk), "output": sum(r.out for r in wk), "cacheWrite": sum(r.cw for r in wk),
                 "cacheRead": sum(r.cache_read for r in wk), "cost": round(sum(map(t.cost, wk)), 2)},
        "sessions": [block(b) for b in reversed(bl)][:500],
        "hourly": [[h, m, n] for (h, m), n in sorted(hourly.items())],
        "flow": [list(k) + [v] for k, v in flow.items()],
        "totals": {"all": sum(r.tok for r in rs), "cacheRead": sum(r.cache_read for r in rs), "messages": len(rs),
                   "cost": round(sum(map(t.cost, rs)), 2)},
    }

def log(q):
    one = lambda k: q.get(k, [""])[0]
    rs = rows()
    if one("session"):
        b = next((b for b in t.blocks(rs, t.pin(t.cfg())) if str(ms(b[0])) == one("session")), None)
        rs = b[2] if b else []
    rs = [r for r in rs
          if (not one("model") or r.model == one("model")) and (not one("family") or one("family") in r.model)
          and (not one("project") or r.project == one("project"))
          and (not one("q") or one("q").lower() in f"{r.project} {r.model}".lower())]
    if one("order") != "asc":
        rs = rs[::-1]
    off, lim = int(one("offset") or 0), min(int(one("limit") or 200), 5000)
    return {"total": len(rs), "rows": [
        {"ts": ms(r.ts), "model": r.model, "project": r.project, "input": r.inp, "output": r.out, "cacheWrite": r.cw, "cacheRead": r.cache_read}
        for r in rs[off:off + lim]]}

def save_config(body):
    # trust boundary: only known keys, only in-range ints
    ranges = {"limit5h": (1, 10**12), "limitWeek": (1, 10**12), "weekDay": (0, 6), "weekHour": (0, 23), "sessionEnd": (0, 10**14)}
    new = {k: int(body[k]) for k in ranges if k in body}
    bad = [k for k, v in new.items() if not ranges[k][0] <= v <= ranges[k][1]]
    if bad:
        raise ValueError(f"out of range: {', '.join(bad)}")
    c = {**t.cfg(), **new}
    json.dump(c, open(t.CFG_PATH, "w"), indent=2)
    return c

class H(BaseHTTPRequestHandler):
    def send(self, code, body):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        u = urlparse(self.path)
        if u.path == "/api/usage":
            return self.send(200, usage())
        if u.path == "/api/log":
            return self.send(200, log(parse_qs(u.query)))
        self.send(404, {"error": "not found"})

    def do_POST(self):
        try:
            body = json.loads(self.rfile.read(int(self.headers.get("Content-Length") or 0)) or b"{}")
            if self.path == "/api/refresh":
                _cache["at"] = 0
                return self.send(200, {"ok": True})
            if self.path == "/api/config":
                return self.send(200, save_config(body))
            self.send(404, {"error": "not found"})
        except (ValueError, TypeError) as e:
            self.send(400, {"error": str(e)})

    def log_message(self, *a):
        pass

if __name__ == "__main__":
    print("Claude token tracker API on http://127.0.0.1:8787")
    ThreadingHTTPServer(("127.0.0.1", 8787), H).serve_forever()
