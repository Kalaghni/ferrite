#!/usr/bin/env python3
"""Summarize real client JSONL telemetry and the server-observed combat events."""
import argparse
import json
import math
from pathlib import Path
import statistics


def pct(values, fraction):
    ordered = sorted(values)
    return ordered[max(0, min(len(ordered)-1, math.ceil(fraction*len(ordered))-1))] if ordered else None


def summarize(directory):
    directory = Path(directory)
    all_rec, moving_rec, hits, first_hits, queue_ms = [], [], [], [], []
    exposure, moving_exposure, clients, seen = 0.0, 0.0, 0, set()
    for path in sorted(directory.glob('client-*-metrics.jsonl')):
        lines = path.read_text().splitlines()
        rows = [json.loads(line) for line in lines if line.strip()]
        rec = [r for r in rows if r['kind'] == 'reconcile']
        if not rec:
            continue
        clients += 1
        exposure += rec[-1]['t']-rec[0]['t']
        for previous, row in zip(rec, rec[1:]):
            if row['moving']:
                moving_exposure += row['t']-previous['t']
        all_rec.extend(rec)
        moving_rec.extend(r for r in rec if r['moving'] and not r['discontinuity'])
        for row in rows:
            if row['kind'] == 'hit_feedback':
                seen.add(row['event_id'])
                if row['command_to_hud_ms'] is not None:
                    hits.append(row['command_to_hud_ms'])
                if row['trigger_to_first_hud_ms'] is not None:
                    first_hits.append(row['trigger_to_first_hud_ms'])
            elif row['kind'] == 'fire_command':
                queue_ms.append(row['input_queue_ms'])
    result_path = directory/'graphical-result.json'
    game = json.loads(result_path.read_text()) if result_path.exists() else {}
    human_ids = {p['id'] for p in game.get('final_players', [])}
    server_hits = {e['id'] for e in game.get('combat_events', []) if e['shooter'] in human_ids}
    errors = [r['prediction_error_m'] for r in moving_rec]
    return dict(directory=str(directory), clients=clients,
        network=game.get('network'), graphical_status=game.get('status'),
        reconciliations=len(all_rec), moving_samples=len(moving_rec),
        discontinuities=sum(r['discontinuity'] for r in all_rec),
        client_seconds=round(exposure, 3), moving_client_seconds=round(moving_exposure, 3),
        replay_hz=round(len(all_rec)/exposure, 3) if exposure else None,
        moving_corrections=sum(r['corrected'] for r in moving_rec),
        moving_correction_hz=round(sum(r['corrected'] for r in moving_rec)/moving_exposure, 3) if moving_exposure else None,
        moving_prediction_mean_m=round(statistics.mean(errors), 5) if errors else None,
        moving_prediction_p95_m=round(pct(errors, .95), 5) if errors else None,
        moving_prediction_max_m=round(max(errors), 5) if errors else None,
        confirmed_hit_samples=len(hits), hit_command_to_hud_p50_ms=statistics.median(hits) if hits else None,
        hit_command_to_hud_p95_ms=pct(hits, .95), hit_command_to_hud_max_ms=max(hits) if hits else None,
        trigger_to_first_hud_ms=first_hits,
        input_queue_p95_ms=pct(queue_ms, .95),
        server_observed_human_hits=len(server_hits), feedback_seen=len(server_hits & seen),
        feedback_missing=len(server_hits-seen), max_pending=max((r['pending'] for r in all_rec), default=0))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directories', nargs='+', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    rows = [summarize(path) for path in args.directories]
    output = json.dumps(rows, indent=2)+'\n'
    if args.output:
        args.output.write_text(output)
    print(output, end='')


if __name__ == '__main__':
    main()
