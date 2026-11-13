#!/usr/bin/env python3
"""Run the RTT/loss matrix using actual graphical clients and correlated hits."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
from summarize_metrics import summarize


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--xvfb', default='Xvfb')
    parser.add_argument('--xdotool', default='xdotool')
    parser.add_argument('--duration', type=float, default=35)
    parser.add_argument('--display-base', type=int, default=120)
    parser.add_argument('--output', type=Path, default=Path('verification/client-matrix'))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    rows = []
    for rtt in [0, 50, 100, 200]:
        for loss in [0, 2, 5]:
            destination = args.output/f'rtt-{rtt}-loss-{loss}'
            destination.mkdir(exist_ok=True)
            with (destination/'driver.log').open('w') as log:
                command = [sys.executable, 'scripts/graphical_soak.py', '--xvfb', args.xvfb,
                    '--xdotool', args.xdotool, '--duration', str(args.duration),
                    '--display-number', str(args.display_base+len(rows)), '--output', str(destination),
                    '--rtt-ms', str(rtt), '--loss-percent', str(loss), '--seed', str(20261112+len(rows)),
                    '--require-combat']
                run = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
            result = summarize(destination)
            result['driver_exit_code'] = run.returncode
            rows.append(result)
            (args.output/'summary.json').write_text(json.dumps(rows, indent=2)+'\n')
            print(json.dumps(result), flush=True)
    if any(r['driver_exit_code'] != 0 for r in rows):
        raise SystemExit('Some graphical scenarios failed; inspect their driver logs')


if __name__ == '__main__':
    main()
