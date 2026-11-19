#!/usr/bin/env python3
"""Tiny Chrome-DevTools-Protocol helper for Steam's own web UI (stdlib only).

Steam's UI is a Chromium page. With a file named `.cef-enable-debugging` in the Steam
folder, Steam exposes it on http://127.0.0.1:8080 (the same feature Steam Deck tools use).
That lets us SEE the UI (screenshots) even when the Wine window is drawn black.

Usage (run with `python3 -I steam_cdp.py ...`):
  list                          show the pages Steam exposes
  shot <title-part> <out.png>   screenshot of the page whose title contains <title-part>
  click <title-part> <x> <y>    left click at page coordinates   (ONLY after the user agreed)
  eval <title-part> <js>        run JavaScript in the page and print the result
Only ever click on the user's behalf after they said so. Accepting anything legal (EULA,
agreements) needs the user's explicit "yes" for that specific agreement.
"""
import base64, json, os, socket, struct, sys, urllib.request

HOST, PORT = "127.0.0.1", int(os.environ.get("STEAM_CDP_PORT", "8080"))


def pages():
    with urllib.request.urlopen(f"http://{HOST}:{PORT}/json", timeout=5) as r:
        return json.load(r)


def find(part):
    part = part.lower()
    for p in pages():
        if part in (p.get("title", "") + " " + p.get("url", "")).lower():
            return p
    raise SystemExit(f"no page matching {part!r}; run `list`")


class WS:
    def __init__(self, url):
        assert url.startswith("ws://")
        hostport, path = url[5:].split("/", 1)
        host, port = hostport.split(":")
        self.s = socket.create_connection((host, int(port)), timeout=15)
        key = base64.b64encode(os.urandom(16)).decode()
        self.s.sendall((f"GET /{path} HTTP/1.1\r\nHost: {hostport}\r\nUpgrade: websocket\r\n"
                        f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n"
                        f"Sec-WebSocket-Version: 13\r\n\r\n").encode())
        buf = b""
        while b"\r\n\r\n" not in buf:
            buf += self.s.recv(1)
        if b" 101 " not in buf.split(b"\r\n")[0]:
            raise SystemExit("websocket handshake failed: " + buf.decode(errors="replace")[:200])
        self.n = 0

    def send(self, obj):
        data = json.dumps(obj).encode()
        mask = os.urandom(4)
        hdr = bytes([0x81])
        n = len(data)
        if n < 126:
            hdr += bytes([0x80 | n])
        elif n < 65536:
            hdr += bytes([0x80 | 126]) + struct.pack(">H", n)
        else:
            hdr += bytes([0x80 | 127]) + struct.pack(">Q", n)
        self.s.sendall(hdr + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))

    def _read(self, n):
        out = b""
        while len(out) < n:
            chunk = self.s.recv(n - len(out))
            if not chunk:
                raise SystemExit("connection closed")
            out += chunk
        return out

    def recv(self):
        msg = b""
        while True:
            b1, b2 = self._read(2)
            ln = b2 & 0x7F
            if ln == 126:
                ln = struct.unpack(">H", self._read(2))[0]
            elif ln == 127:
                ln = struct.unpack(">Q", self._read(8))[0]
            payload = self._read(ln)
            op = b1 & 0x0F
            if op == 8:
                raise SystemExit("closed by Steam")
            if op in (1, 0):
                msg += payload
                if b1 & 0x80:
                    return json.loads(msg)

    def call(self, method, **params):
        self.n += 1
        self.send({"id": self.n, "method": method, "params": params})
        while True:
            m = self.recv()
            if m.get("id") == self.n:
                if "error" in m:
                    raise SystemExit(f"{method} failed: {m['error']}")
                return m.get("result", {})


def main():
    a = sys.argv[1:]
    if not a:
        raise SystemExit(__doc__)
    if a[0] == "list":
        for p in pages():
            print(f"{p.get('id','')[:8]}  {p.get('title','')!r:40}  {p.get('url','')[:90]}")
        return
    ws = WS(find(a[1])["webSocketDebuggerUrl"])
    if a[0] == "shot":
        r = ws.call("Page.captureScreenshot", format="png")
        open(a[2], "wb").write(base64.b64decode(r["data"]))
        print("saved", a[2])
    elif a[0] == "click":
        x, y = float(a[2]), float(a[3])
        for t in ("mousePressed", "mouseReleased"):
            ws.call("Input.dispatchMouseEvent", type=t, x=x, y=y, button="left", clickCount=1)
        print("clicked", x, y)
    elif a[0] == "eval":
        r = ws.call("Runtime.evaluate", expression=a[2], returnByValue=True)
        print(json.dumps(r.get("result", {}).get("value"), ensure_ascii=False)[:2000])
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
