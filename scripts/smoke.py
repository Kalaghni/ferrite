#!/usr/bin/env python3
"""Black-box UDP checks; use --spawn to own and clean up a local real server."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import time


def client(host, port):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.connect((host, port))
    sock.settimeout(0.1)
    return sock


def send(sock, seq, name, forward=0, strafe=0, fire=False):
    packet = dict(version=2, seq=seq, name=name, forward=forward, strafe=strafe,
                  yaw=0.0, pitch=0.0, fire=fire, sprint=False)
    sock.send(json.dumps(packet).encode())


def receive(sock, timeout=3, predicate=lambda world: True):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        try:
            world = json.loads(sock.recv(65535))
            if world.get("version") == 2 and predicate(world):
                return world
        except socket.timeout:
            continue
    raise AssertionError("No matching v2 snapshot before deadline")


def self_player(world):
    return next(p for p in world["soldiers"] if p["id"] == world["you"])


def run(host, port, fresh=False):
    a, b = client(host, port), client(host, port)
    try:
        send(a, 1, "smoke_A")
        send(b, 1, "smoke_B")
        sa, sb = receive(a), receive(b)
        assert sa["you"] != sb["you"], "Duplicate player IDs"
        assert self_player(sa)["team"] != self_player(sb)["team"], "Unbalanced teams"
        assert len(sa["zones"]) == 3, "Missing zones"
        start = self_player(sa)["z"]
        for seq in range(2, 15):
            send(a, seq, "smoke_A", forward=1.0)
            send(b, seq, "smoke_B")
            time.sleep(0.05)
        latest = receive(a, predicate=lambda w: w["ack_seq"] >= 14)
        end = self_player(latest)["z"]
        assert end > start + 0.3, f"No movement: {start:.2f} -> {end:.2f}"
        # Check state as well as ack: a stale reverse command must not take effect.
        send(a, 3, "smoke_A", forward=-1.0)
        after = receive(a, predicate=lambda w: w["tick"] >= latest["tick"] + 4)
        assert after["ack_seq"] >= 14, "Stale input rolled back ack"
        assert self_player(after)["z"] > end + 0.3, "Stale command reversed movement"
        send(a, 15, "smoke_A")
        stopped = receive(a, predicate=lambda w: w["ack_seq"] >= 15)
        settled = receive(a, predicate=lambda w: w["tick"] >= stopped["tick"] + 4)
        assert abs(self_player(settled)["z"] - self_player(stopped)["z"]) < 0.001, "Stop ignored"
        if fresh:
            bad = client(host, port)
            try:
                for packet in [b"garbage", b"{}", json.dumps(dict(
                        version=999, seq=1, name="bad", forward=0, strafe=0,
                        yaw=0, pitch=0, fire=False, sprint=False)).encode()]:
                    bad.send(packet)
                try:
                    bad.recv(65535)
                    raise AssertionError("Invalid packet established a session")
                except socket.timeout:
                    pass
                send(a, 16, "smoke_A")
                checked = receive(a, predicate=lambda w: w["ack_seq"] >= 16)
                assert len([p for p in checked["soldiers"] if not p["bot"]]) == 2
            finally:
                bad.close()
        print(f"PASS: two clients, teams, zones, movement, ack, stale rejection, stop"
              f"{', invalid packets' if fresh else ''}; z={start:.2f}->{end:.2f}")
    finally:
        a.close()
        b.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=4000)
    parser.add_argument("--spawn", action="store_true")
    parser.add_argument("--server", type=Path, default=Path("target/debug/ferrite-server" + (".exe" if os.name == "nt" else "")))
    args = parser.parse_args()
    process = None
    try:
        if args.spawn:
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as reserve:
                reserve.bind(("127.0.0.1", 0))
                args.port = reserve.getsockname()[1]
            args.host = "127.0.0.1"
            process = subprocess.Popen([str(args.server.resolve()), f"{args.host}:{args.port}"])
            time.sleep(0.2)
            assert process.poll() is None, "Server exited at startup"
        run(args.host, args.port, fresh=args.spawn)
    finally:
        if process is not None:
            process.terminate()
            process.wait(timeout=5)


if __name__ == "__main__":
    main()
