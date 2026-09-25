#!/usr/bin/env python3
"""Capture what a real browser sends to the agent port — M39/T39.0.

`crates/lumen-static-agent/tests/transport.rs` replays these bytes against
the agent transport. A request typed from memory tests its author's idea
of a browser (docs/plan-agent-transport.md §0 rule 4), so the captured
fixtures are written by this script and nothing else.

Two listeners, both on loopback:

- PAGE_PORT serves one HTML page. It is a *different origin* from the
  recorder (another port), which is the cross-origin shape a web page the
  developer happens to have open has.
- REC_PORT is the recorder: it reads each connection's request line,
  headers and `Content-Length` body, writes the raw bytes to
  `<out>/<browser>-<vector>.bin`, and answers `204` (or `400` to the
  WebSocket upgrade) so the browser moves on.

The page fires four vectors, each at its own path so the recorder can
name the file: a `no-cors` `fetch` POST, `navigator.sendBeacon`, a
`new WebSocket`, and — last, because it navigates — a
`<form enctype="text/plain">` POST shaped so its `name=value` body is a
JSON line. Every body carries the same two JSON-RPC lines the gates
look for: a click on counter-window's `#inc` and one on tasks-demo's
`#spawn-one`.

Usage: capture_browser_requests.py OUT_DIR BROWSER_NAME -- BROWSER_ARGV...
The URL is appended to BROWSER_ARGV. Ports come from LUMEN_CAPTURE_PORTS
("rec,page", default 7810,7811).
"""
import os
import socket
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

REC_PORT, PAGE_PORT = (int(p) for p in os.environ.get("LUMEN_CAPTURE_PORTS", "7810,7811").split(","))

# The two JSON-RPC lines every body carries. Kept short: a form field's
# name cannot contain `=`, and the trick below needs the value to close
# the JSON the name opened.
CLICK_INC = '{"jsonrpc":"2.0","id":1,"method":"input.click","params":{"selector":"#inc"}}'
CLICK_SPAWN = '{"jsonrpc":"2.0","id":2,"method":"input.click","params":{"selector":"#spawn-one"}}'
BODY = CLICK_INC + "\n" + CLICK_SPAWN + "\n"

PAGE = """<!doctype html><meta charset="utf-8"><title>capture</title>
<body>
<form id="f" method="POST" enctype="text/plain" action="http://127.0.0.1:{rec}/form">
<input type="hidden" name='{inc_head}' value='{inc_tail}'>
<input type="hidden" name='{spawn_head}' value='{spawn_tail}'>
</form>
<script>
const rec = "127.0.0.1:{rec}";
const body = {body_js};
const pause = ms => new Promise(r => setTimeout(r, ms));
(async () => {{
  try {{ await fetch("http://" + rec + "/fetch", {{method: "POST", mode: "no-cors", body}}); }} catch (e) {{}}
  await pause(300);
  navigator.sendBeacon("http://" + rec + "/beacon", body);
  await pause(600);
  try {{ const ws = new WebSocket("ws://" + rec + "/ws"); ws.onerror = () => {{}}; }} catch (e) {{}}
  await pause(600);
  document.getElementById("f").submit();
}})();
</script>
"""


def form_split(line):
    """A JSON line as a text/plain form field: the body is `name=value\\r\\n`,
    so the name carries the JSON up to a string value and the value
    closes it — the `=` lands inside a string the JSON ignores."""
    head = line[:-1] + ',"pad":"'
    tail = '"}'
    return head, tail


def page_html():
    ih, it = form_split(CLICK_INC)
    sh, st = form_split(CLICK_SPAWN)
    import json
    return PAGE.format(
        rec=REC_PORT,
        inc_head=ih, inc_tail=it, spawn_head=sh, spawn_tail=st,
        body_js=json.dumps(BODY),
    )


class Page(BaseHTTPRequestHandler):
    def do_GET(self):
        data = page_html().encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *a):
        pass


def record(out, browser, seen, done):
    srv = socket.socket()
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", REC_PORT))
    srv.listen(8)
    srv.settimeout(0.5)
    while not done.is_set():
        try:
            conn, _ = srv.accept()
        except socket.timeout:
            continue
        conn.settimeout(2.0)
        buf = b""
        try:
            while b"\r\n\r\n" not in buf:
                chunk = conn.recv(65536)
                if not chunk:
                    break
                buf += chunk
            head, _, rest = buf.partition(b"\r\n\r\n")
            length = 0
            for h in head.split(b"\r\n")[1:]:
                k, _, v = h.partition(b":")
                if k.strip().lower() == b"content-length":
                    length = int(v.strip())
            while len(rest) < length:
                chunk = conn.recv(65536)
                if not chunk:
                    break
                rest += chunk
                buf += chunk
        except socket.timeout:
            pass
        if not buf:
            conn.close()
            continue
        path = buf.split(b" ", 2)[1].decode(errors="replace").strip("/") or "root"
        vector = {"fetch": "fetch-no-cors", "beacon": "send-beacon", "ws": "websocket-upgrade",
                  "form": "form-text-plain"}.get(path, path.replace("/", "_"))
        name = f"{browser}-{vector}.bin"
        with open(os.path.join(out, name), "wb") as f:
            f.write(buf)
        seen.add(vector)
        print(f"captured {name}: {len(buf)} B", flush=True)
        status = b"400 Bad Request" if vector == "websocket-upgrade" else b"204 No Content"
        try:
            conn.sendall(b"HTTP/1.1 " + status + b"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        except OSError:
            pass
        conn.close()
    srv.close()


def main():
    out, browser = sys.argv[1], sys.argv[2]
    argv = sys.argv[sys.argv.index("--") + 1:]
    os.makedirs(out, exist_ok=True)
    seen, done = set(), threading.Event()
    t = threading.Thread(target=record, args=(out, browser, seen, done), daemon=True)
    t.start()
    page = HTTPServer(("127.0.0.1", PAGE_PORT), Page)
    threading.Thread(target=page.serve_forever, daemon=True).start()
    proc = subprocess.Popen(argv + [f"http://127.0.0.1:{PAGE_PORT}/"],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    want = {"fetch-no-cors", "send-beacon", "websocket-upgrade", "form-text-plain"}
    deadline = time.time() + 60
    while time.time() < deadline and not want <= seen:
        time.sleep(0.2)
    proc.terminate()
    try:
        proc.wait(5)
    except subprocess.TimeoutExpired:
        proc.kill()
    done.set()
    page.shutdown()
    missing = want - seen
    if missing:
        print(f"missing: {sorted(missing)}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
