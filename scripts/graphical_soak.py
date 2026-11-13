#!/usr/bin/env python3
"""Linux graphical endurance check using real clients, Xvfb, and XTest input.

Requires Xvfb, xdotool, ImageMagick's import, and a built client/server.
This is scripted gameplay; human evaluation of feel remains necessary.
"""
import argparse
import json
import math
import os
from pathlib import Path
import socket
import subprocess
import time
from udp_proxy import UdpProxy


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--xvfb', default='Xvfb')
    parser.add_argument('--xdotool', default='xdotool')
    parser.add_argument('--duration', type=float, default=600)
    parser.add_argument('--display-number', type=int, default=109)
    parser.add_argument('--output', type=Path, default=Path('verification'))
    parser.add_argument('--require-combat', action='store_true')
    parser.add_argument('--rtt-ms', type=int, default=0)
    parser.add_argument('--loss-percent', type=float, default=0)
    parser.add_argument('--seed', type=int, default=20261112)
    args = parser.parse_args()
    output = args.output
    output.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, DISPLAY=f'127.0.0.1:{args.display_number}', LIBGL_ALWAYS_SOFTWARE='1')
    processes, logs, world = [], [], None
    proxies = []
    observer = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    observer.setblocking(False)
    sequence, last_send, last_snapshot = 0, 0.0, 0.0
    events, respawns, previous_health = {}, {}, {}
    max_gap, snapshots = 0.0, 0
    started = time.monotonic()

    def launch(name, command, measure=False):
        log = open(output / (name + '.log'), 'w')
        logs.append(log)
        child_env = dict(env)
        if measure:
            child_env['FERRITE_METRICS_PATH'] = str(output/(name+'-metrics.jsonl'))
        p = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, env=child_env)
        processes.append(p)
        return p

    def xdo(*command):
        return subprocess.check_output([args.xdotool, *map(str, command)], env=env, text=True).strip()

    def pump(seconds):
        nonlocal world, sequence, last_send, last_snapshot, max_gap, snapshots
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            assert all(p.poll() is None for p in processes), 'Graphical process exited early'
            assert all(proxy.error is None for proxy in proxies), 'UDP impairment proxy failed'
            now = time.monotonic()
            if now - last_send >= 0.05:
                sequence += 1
                observer.send(json.dumps(dict(version=2, seq=sequence, name='SoakObserver',
                    forward=0, strafe=0, yaw=0, pitch=0, fire=False, sprint=False)).encode())
                last_send = now
            while True:
                try:
                    incoming = json.loads(observer.recv(65535))
                except BlockingIOError:
                    break
                if world is not None and incoming['tick'] <= world['tick']:
                    continue
                world = incoming
                snapshots += 1
                if last_snapshot:
                    max_gap = max(max_gap, now-last_snapshot)
                last_snapshot = now
                for event in world['events']:
                    events[event['id']] = event
                for soldier in world['soldiers']:
                    sid = str(soldier['id'])
                    if previous_health.get(sid, 100) == 0 and soldier['health'] > 0:
                        respawns[sid] = respawns.get(sid, 0) + 1
                    previous_health[sid] = soldier['health']
            assert not last_snapshot or now-last_snapshot < 3, 'Snapshot stream stalled'
            time.sleep(0.005)

    def state(name):
        return next(p for p in world['soldiers'] if p['name'] == name)

    def focus(window):
        xdo('key', 'Escape')
        pump(0.15)
        xdo('windowraise', window)
        xdo('windowfocus', window)
        xdo('mousemove', '--window', window, 640, 360)
        xdo('click', 1)
        pump(0.2)

    def walk(window, key, seconds):
        focus(window)
        xdo('keydown', key)
        pump(seconds)
        xdo('keyup', key)
        pump(0.2)

    try:
        launch('xvfb', [args.xvfb, f':{args.display_number}', '-screen', '0', '1600x900x24',
            '-ac', '-listen', 'tcp', '-nolisten', 'unix', '-nolisten', 'local', '-xkbdir', '/usr/share/X11/xkb'])
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as reserve:
            reserve.bind(('127.0.0.1', 0))
            port = reserve.getsockname()[1]
        launch('soak-server', ['target/debug/ferrite-server', f'127.0.0.1:{port}'])
        time.sleep(0.5)
        proxies = [UdpProxy(('127.0.0.1', port), args.rtt_ms, args.loss_percent, args.seed+i) for i in range(2)]
        a_port, b_port = [p.address[1] for p in proxies]
        launch('client-A', ['target/debug/ferrite-client', f'127.0.0.1:{a_port}', 'GraphicsA'], measure=True)
        time.sleep(0.5)
        launch('client-B', ['target/debug/ferrite-client', f'127.0.0.1:{b_port}', 'GraphicsB'], measure=True)
        # Do not let the telemetry socket consume player/team slot #2.
        deadline = time.monotonic() + 10
        while (output/'soak-server.log').read_text().count('JOIN ') < 2:
            assert time.monotonic() < deadline, 'Graphical clients failed to join'
            assert all(p.poll() is None for p in processes), 'Graphical client failed at startup'
            time.sleep(0.05)
        observer.connect(('127.0.0.1', port))
        pump(2)
        windows = xdo('search', '--name', 'FERRITE').splitlines()
        assert len(windows) == 2, f'Expected two game windows: {windows}'
        a, b = windows
        xdo('windowmove', a, 0, 0)
        xdo('windowmove', b, 260, 100)
        initial = {name: dict(state(name)) for name in ['GraphicsA', 'GraphicsB']}
        # Navigate both real clients to an open lane, then aim using server telemetry.
        walk(a, 'd', 4.6)
        walk(b, 'a', 4.1)
        walk(a, 'w', 4.0)
        walk(b, 'w', 4.0)
        moved = {name: math.hypot(state(name)['x']-initial[name]['x'], state(name)['z']-initial[name]['z'])
                 for name in initial}
        print(json.dumps({'initial': initial, 'after_movement': [state(n) for n in initial]}), flush=True)
        assert all(distance > 1 for distance in moved.values()), f'Keyboard movement failed: {moved}'
        focus(a)
        pump(0.3 + args.rtt_ms/1000)
        old_yaw = state('GraphicsA')['yaw']
        xdo('mousemove_relative', '--', 10, 0)
        pump(0.4 + args.rtt_ms/1000)
        measured_delta = (old_yaw-state('GraphicsA')['yaw']+math.pi) % math.tau - math.pi
        mouse_gain = measured_delta / 10
        assert 0.0001 < mouse_gain < 0.05, f'Unexpected XTest mouse gain: {mouse_gain}'
        for _ in range(15):
            shooter, target = state('GraphicsA'), state('GraphicsB')
            desired = math.atan2(target['x']-shooter['x'], target['z']-shooter['z'])
            delta = (desired-shooter['yaw']+math.pi) % math.tau - math.pi
            if abs(delta) < mouse_gain*0.6:
                break
            pixels = round(max(-20, min(20, -delta/mouse_gain)))
            xdo('mousemove_relative', '--', pixels, 0)
            pump(0.3 + args.rtt_ms/1000)
        print(json.dumps({'phase':'aim', 'shooter':state('GraphicsA'), 'target':state('GraphicsB')}), flush=True)
        distance = math.hypot(state('GraphicsB')['x']-state('GraphicsA')['x'], state('GraphicsB')['z']-state('GraphicsA')['z'])
        xdo('mousemove_relative', '--', 0, round(-math.atan2(-0.75, distance)/mouse_gain))
        pump(0.3 + args.rtt_ms/1000)
        xdo('mousedown', 1)
        pump(0.35)
        subprocess.run(['import', '-window', a, str(output/'graphical-combat.png')], env=env, check=True)
        pump(1.25)
        xdo('mouseup', 1)
        xdo('key', 'Escape')
        pump(5)
        print(json.dumps({'phase': 'scripted movement/combat', 'moved_m': moved,
            'players': [state(n) for n in initial], 'combat_events': len(events), 'respawns': respawns}), flush=True)
        # Keep both render loops and their real UDP connections running for ten minutes.
        next_report = 30.0
        while time.monotonic()-started < args.duration:
            pump(0.2)
            elapsed = time.monotonic()-started
            if elapsed >= next_report:
                progress = dict(elapsed_seconds=round(elapsed, 2), snapshots=snapshots,
                    tick=world['tick'], max_snapshot_gap_seconds=round(max_gap, 3),
                    combat_events=len(events), respawns=respawns, moved_m=moved)
                (output/'graphical-progress.json').write_text(json.dumps(progress, indent=2)+'\n')
                print(json.dumps(progress), flush=True)
                next_report += 30
        subprocess.run(['import', '-window', 'root', str(output/'graphical-end.png')], env=env, check=True)
        if args.require_combat:
            human_ids = {initial[n]['id'] for n in initial}
            assert any(e['shooter'] in human_ids and e['victim'] in human_ids and e['killed']
                       for e in events.values()), 'No confirmed player-versus-player kill'
            assert any(respawns.get(str(sid), 0) > 0 for sid in human_ids), 'No human respawn'
        result = dict(status='PASS', elapsed_seconds=round(time.monotonic()-started, 2),
            snapshots=snapshots, tick=world['tick'], max_snapshot_gap_seconds=round(max_gap, 3),
            combat_events=list(events.values()), respawns=respawns, moved_m=moved,
            final_players=[state(n) for n in initial], mode='Xvfb software GL; scripted XTest gameplay')
        result['network'] = dict(rtt_ms=args.rtt_ms, loss_percent=args.loss_percent, seed=args.seed,
                                proxy_counts=[p.counts for p in proxies])
        (output/'graphical-result.json').write_text(json.dumps(result, indent=2)+'\n')
        print(json.dumps({**result, 'combat_events':len(events)}), flush=True)
    finally:
        observer.close()
        for p in reversed(processes):
            if p.poll() is None:
                p.terminate()
                p.wait(timeout=5)
        for log in logs:
            log.close()
        for proxy in proxies:
            proxy.close()


if __name__ == '__main__':
    main()
