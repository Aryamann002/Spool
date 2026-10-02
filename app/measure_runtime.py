#!/usr/bin/env python3
"""Run frame-paced production-renderer diagnostics. No input automation.
Build first: cargo build --locked
Run: python3 measure_runtime.py --repeats 2
Raw logs and machine-readable per-run summaries default to target/phase13.
"""
import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import time


def parse_report(text):
    result = {}
    for line in text.splitlines():
        if not line.startswith("spool_diagnostics "):
            continue
        fields = dict(token.split("=", 1) for token in shlex.split(line)[1:])
        if fields["label"].endswith(":finished"):
            if fields["kind"] != "drag_finish_history":
                continue
        result[fields["kind"]] = fields
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repeats", type=int, default=2)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--cases", nargs="+", help="optional subset, e.g. drag:10000")
    parser.add_argument("--run-start", type=int, default=1)
    parser.add_argument("--output", default="target/phase13")
    args = parser.parse_args()
    if args.repeats < 1 or args.timeout <= 0:
        parser.error("repeats and timeout must be positive")
    root = Path(__file__).resolve().parent
    output = root / args.output
    output.mkdir(parents=True, exist_ok=True)
    cases = [f"{kind}:{count}" for kind in ("pan", "drag", "selection", "text") for count in (100, 1000, 10000)]
    if args.cases:
        if any(case not in cases for case in args.cases):
            parser.error("unsupported case")
        cases = args.cases
    summary_path = output / "summary.json"
    summaries = json.loads(summary_path.read_text()) if summary_path.exists() else []
    for repeat in range(args.run_start, args.run_start + args.repeats):
        for case in cases:
            log = output / f"{case.replace(':', '-')}-run{repeat}.log"
            env = dict(os.environ, SPOOL_DIAGNOSTICS="1", SPOOL_WORKLOAD=case)
            with log.open("w") as stream:
                process = subprocess.Popen([str(root / "target/debug/Spool")], cwd=root, env=env,
                                           stdout=subprocess.DEVNULL, stderr=stream)
                deadline = time.monotonic() + args.timeout
                try:
                    while "spool_workload_complete" not in log.read_text():
                        if process.poll() is not None:
                            raise RuntimeError(f"{case} exited early: see {log}")
                        if time.monotonic() >= deadline:
                            raise TimeoutError(f"{case} exceeded {args.timeout}s: see {log}")
                        time.sleep(0.2)
                finally:
                    if process.poll() is None:
                        process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
            result = {"workload": case, "run": repeat, "metrics": parse_report(log.read_text())}
            metrics = result["metrics"]
            if metrics.get("workload_updates", {}).get("count") != "60":
                raise RuntimeError(f"interrupted workload: see {log}")
            summaries = [row for row in summaries if (row["workload"], row["run"]) != (case, repeat)]
            summaries.append(result)
            # Preserve completed runs even if a later workload fails.
            (output / "summary.json").write_text(json.dumps(summaries, indent=2) + "\n")
            metrics = result["metrics"]
            print(json.dumps({"workload": case, "run": repeat,
                              "canvas": metrics["canvas_render"]["count"],
                              "shell": metrics["shell_render"]["count"],
                              "constructed": metrics["objects_constructed"]["count"],
                              "culled": metrics.get("objects_culled", {}).get("count", "0"),
                              "considered": metrics.get("objects_considered", {}).get("count", "0"),
                              "offscreen": metrics["geometry_offscreen"]["count"],
                              "visibility_scan": metrics.get("visibility_scan"),
                              "canvas_elements": metrics["canvas_elements"],
                              "shell_build": metrics["shell_render_build"],
                              "layers_build": metrics.get("layers_tree_build"),
                              "layers_projections": metrics.get("layers_projection_rebuild", {}).get("count", "0"),
                              "layers_rows": metrics.get("layers_row_construction", {}).get("count", "0"),
                              "row_updates": metrics.get("row_presentation_updates", {}).get("count", "0"),
                              "update": metrics["workload_update"]}), flush=True)


if __name__ == "__main__":
    main()
