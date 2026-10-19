#!/usr/bin/env python3
"""Extended-brightness probe for the Lecoo GK301 vendor interface.

Reconstructs the Windows tool's startup apply path (task sub_4206f0 sends the
stored 20-record table entries unclamped; ledeffect.xml defaults carry
brightness=13) and sweeps the brightness byte on effect 0 (Steady) while a
concurrent reader logs any vendor input reports from the same hidraw node.

Uses the same command.lock as the controller, so the panel never interleaves.
No random command scan: only the startup-form record packets.
"""
import argparse
import fcntl
import json
import os
import select
import sys
import threading
import time

IOC_WRITE = 1
IOC_READ = 2


def ioc(dir_, ty, nr, size):
    return (dir_ << 30) | (size << 16) | (ty << 8) | nr


def set_feature_ioctl(size):
    return ioc(IOC_WRITE | IOC_READ, ord('H'), 0x06, size)


def get_feature_ioctl(size):
    return ioc(IOC_WRITE | IOC_READ, ord('H'), 0x07, size)


def hx(b):
    return " ".join(f"{v:02X}" for v in b)


class Probe:
    def __init__(self, device, events):
        self.fd = os.open(device, os.O_RDWR | os.O_NONBLOCK)
        self.events = events
        self.running = True

    def log(self, kind, **kw):
        ev = {"t": round(time.monotonic(), 3), "kind": kind, **kw}
        self.events.append(ev)
        print(json.dumps(ev), flush=True)

    def reader_loop(self):
        deadline = time.monotonic() + 3600
        seen = set()
        while self.running and time.monotonic() < deadline:
            r, _, _ = select.select([self.fd], [], [], 0.05)
            if not r:
                continue
            try:
                data = os.read(self.fd, 64)
            except BlockingIOError:
                continue
            report_id = data[0] if data else -1
            ev = (round(time.monotonic(), 3), hx(data))
            if ev not in seen:
                seen.add(ev)
                self.log("input_report", report_id=report_id, raw=hx(data))

    def transfer(self, send, buf):
        """HIDIOCSFEATURE/HIDIOCGFEATURE with retry on EAGAIN."""
        req = set_feature_ioctl(len(buf)) if send else get_feature_ioctl(len(buf))
        while True:
            try:
                rc = fcntl.ioctl(self.fd, req, buf, True)
                return rc, bytes(buf)
            except BlockingIOError:
                time.sleep(0.01)

    def set_feature(self, payload):
        buf = bytearray(bytes([0x07]) + bytes(payload))
        assert len(buf) == 8
        self.log("set_feature", raw=hx(buf))
        self.transfer(True, buf)
        self.log("set_feature_done", raw=hx(buf))

    def get_feature(self):
        buf = bytearray(b"\x07" + b"\x00" * 7)
        _rc, resp = self.transfer(False, buf)
        self.log("get_feature", raw=hx(resp))
        return resp

    def firmware_query(self):
        self.set_feature([0xF1, 0, 0, 0, 0, 0, 0])
        time.sleep(0.12)
        self.get_feature()


def parse_state(path):
    if not path or not os.path.exists(path):
        return None
    text = open(path).read()
    vals = {}
    for key in ("effect", "brightness", "speed", "select"):
        i = text.find('"{}"'.format(key))
        if i >= 0:
            j = text.find(":", i) + 1
            digits = ""
            while j < len(text) and text[j].isdigit():
                digits += text[j]
                j += 1
            vals[key] = int(digits)
    return vals


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--device", default=os.environ.get("LECOO_DEVICE", "/dev/hidraw5"))
    ap.add_argument("--out", required=True)
    ap.add_argument("--wait-first", type=float, default=0.0,
                    help="Delay before the first probe so a capture is running.")
    ap.add_argument("--state", default=None, help="state.json to restore at the end")
    args = ap.parse_args()

    lock_path = os.path.join(
        os.environ.get("XDG_STATE_HOME", os.path.expanduser("~/.local/state")),
        "lecoo-gk301", "command.lock")
    os.makedirs(os.path.dirname(lock_path), exist_ok=True)
    lock_fd = open(lock_path, "w")
    fcntl.flock(lock_fd, fcntl.LOCK_EX)

    events = []
    p = Probe(args.device, events)
    reader = threading.Thread(target=p.reader_loop, daemon=True)
    reader.start()

    restored = parse_state(args.state)
    time.sleep(args.wait_first)

    p.log("start", device=args.device)
    # 1) firmware acknowledgment (known-good)
    p.firmware_query()
    time.sleep(1.0)
    # 2) baseline: normal clamped writer, Steady at 5 (known good)
    p.set_feature([0xFF, 0xFF, 0x00, 0x05, 0x02, 0x00, 0x00])
    time.sleep(1.2)
    # 3) startup-form sweep: stored-record path (sub_4206f0 reconstruction)
    #    record [brightness, speed, direction, selection] with brightness>5.
    for b in (8, 10, 13):
        p.set_feature([0xFF, 0xFF, 0x00, b, 0x00, 0x00, 0x00])
        time.sleep(1.2)
    # 4) restore the user's own saved settings through the normal writer
    if restored:
        p.set_feature([0xFF, 0xFF,
                       restored.get("effect", 0) & 0xFF,
                       min(restored.get("brightness", 5), 5),
                       min(restored.get("speed", 2), 2) & 0xFF,
                       min(restored.get("select", 0), 2) & 0xFF,
                       0x00])
        p.log("restored", state=restored)
    time.sleep(0.5)
    p.running = False
    reader.join(timeout=2.0)
    os.close(p.fd)
    with open(args.out, "w") as f:
        json.dump({"device": args.device, "events": events}, f, indent=2)
    print("saved {} events -> {}".format(len(events), args.out))


if __name__ == "__main__":
    main()
