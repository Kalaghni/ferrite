#!/usr/bin/env python3
"""Deterministic artificial RTT/loss matrix against real UDP server processes.

Two socket clients predict one-axis movement in the clear spawn lanes, then
reconcile using the actual snapshot acknowledgement and pending commands.
Corrections are position changes at reconciliation, not rendered camera error.
This measures movement only; it does not validate lag-compensated combat.
"""
import argparse
import heapq
import json
import math
import os
from pathlib import Path
import random
import socket
import statistics
import subprocess
import time


def percentile(values, fraction):
    values = sorted(values)
    return values[min(len(values)-1, math.ceil(fraction*len(values))-1)] if values else 0


def scenario(server, duration, rtt_ms, loss_percent, seed):
    rng = random.Random(seed)
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as reserve:
        reserve.bind(('127.0.0.1', 0))
        port = reserve.getsockname()[1]
    process = subprocess.Popen([str(server.resolve()), f'127.0.0.1:{port}'], stdout=subprocess.DEVNULL)
    sockets, states, queue = [], [], []
    corrections, serial, out_count, in_count, out_drop, in_drop = [], 0, 0, 0, 0, 0
    delay = rtt_ms / 2000  # symmetric: half the requested RTT on each leg
    try:
        time.sleep(0.15)
        assert process.poll() is None, 'Server exited at startup'
        for index in range(2):
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.connect(('127.0.0.1', port))
            sock.setblocking(False)
            sockets.append(sock)
            states.append(dict(z=None, pending=[], seq=0, ack=0, tick=-1,
                snapshots=0, max_pending=0, zs=[], death=False))
        started = time.monotonic()
        next_command = started
        while time.monotonic()-started < duration:
            now = time.monotonic()
            assert process.poll() is None, 'Server crashed during scenario'
            while now >= next_command:
                elapsed = next_command-started
                forward = 1.0 if int(elapsed) % 2 == 0 else -1.0
                for i, state in enumerate(states):
                    state['seq'] += 1
                    packet = dict(version=2, seq=state['seq'], name=f'Lab_{i}', forward=forward,
                        strafe=0, yaw=0, pitch=0, fire=False, sprint=False)
                    if state['z'] is not None:
                        state['z'] += forward*0.4
                        state['pending'].append((state['seq'], forward))
                        state['pending'] = state['pending'][-64:]
                        state['max_pending'] = max(state['max_pending'], len(state['pending']))
                    out_count += 1
                    if rng.random()*100 < loss_percent:
                        out_drop += 1
                    else:
                        serial += 1
                        heapq.heappush(queue, (next_command+delay, serial, 'out', i, json.dumps(packet).encode()))
                next_command += 0.05
            for i, sock in enumerate(sockets):
                while True:
                    try:
                        packet = sock.recv(65535)
                    except BlockingIOError:
                        break
                    in_count += 1
                    if rng.random()*100 < loss_percent:
                        in_drop += 1
                    else:
                        serial += 1
                        heapq.heappush(queue, (now+delay, serial, 'in', i, packet))
            while queue and queue[0][0] <= now:
                _, _, direction, i, payload = heapq.heappop(queue)
                if direction == 'out':
                    sockets[i].send(payload)
                    continue
                world = json.loads(payload)
                state = states[i]
                if world['tick'] <= state['tick']:
                    continue
                assert world['version'] == 2
                assert world['ack_seq'] >= state['ack'], 'Ack moved backwards'
                me = next(p for p in world['soldiers'] if p['id'] == world['you'])
                assert math.isfinite(me['x']) and math.isfinite(me['z'])
                state['death'] |= me['health'] <= 0
                state['pending'] = [p for p in state['pending'] if p[0] > world['ack_seq']]
                corrected = me['z'] + sum(forward*0.4 for _, forward in state['pending'])
                if state['z'] is not None:
                    corrections.append(abs(corrected-state['z']))
                state['z'] = corrected
                state['zs'].append(me['z'])
                state['ack'], state['tick'] = world['ack_seq'], world['tick']
                state['snapshots'] += 1
            time.sleep(0.001)
        for state in states:
            assert state['snapshots'] >= duration*4, f"Snapshot stream stalled: {state}"
            assert state['ack'] >= state['seq']-15, f"Input acknowledgements stalled: {state}"
            assert max(state['zs'])-min(state['zs']) > 2, 'Player did not move'
            assert not state['death'], 'Spawn-lane fixture interrupted by combat'
        return dict(rtt_ms=rtt_ms, loss_percent=loss_percent, seed=seed, duration_seconds=duration,
            status='PASS', snapshots=sum(s['snapshots'] for s in states),
            correction_samples=len(corrections), correction_mean_m=round(statistics.mean(corrections), 5),
            correction_p95_m=round(percentile(corrections, .95), 5), correction_max_m=round(max(corrections), 5),
            max_pending=max(s['max_pending'] for s in states), sent=out_count, outbound_dropped=out_drop,
            received=in_count, inbound_dropped=in_drop,
            last_ack=[s['ack'] for s in states], last_tick=[s['tick'] for s in states])
    finally:
        for sock in sockets:
            sock.close()
        process.terminate()
        process.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', type=Path, default=Path('target/debug/ferrite-server'+('.exe' if os.name == 'nt' else '')))
    parser.add_argument('--duration', type=float, default=6)
    parser.add_argument('--rtt-ms', type=int, nargs='+', default=[0, 50, 100, 200])
    parser.add_argument('--loss-percent', type=float, nargs='+', default=[0, 2, 5])
    parser.add_argument('--seed', type=int, default=20261112)
    parser.add_argument('--output', type=Path, default=Path('verification/network-lab.json'))
    args = parser.parse_args()
    assert args.duration >= 2 and all(0 <= loss < 100 for loss in args.loss_percent)
    assert all(rtt >= 0 for rtt in args.rtt_ms)
    rows = []
    for rtt in args.rtt_ms:
        for loss in args.loss_percent:
            result = scenario(args.server, args.duration, rtt, loss, args.seed+len(rows))
            rows.append(result)
            print(json.dumps(result), flush=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(dict(method=__doc__, scenarios=rows), indent=2)+'\n')


if __name__ == '__main__':
    main()
