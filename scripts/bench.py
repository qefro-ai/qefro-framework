#!/usr/bin/env python3
"""HTTP baseline harness for a running Qefro server. See docs/benchmarks.md."""
from __future__ import annotations

import argparse
import json
import os
import platform
import statistics
import subprocess
import sys
import time
import uuid
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
from typing import Any


def request(
    url: str,
    method: str = "GET",
    token: str | None = None,
    body: dict[str, Any] | None = None,
    timeout: float = 30.0,
) -> tuple[int, Any, float]:
    data = None
    headers = {"accept": "application/json"}
    if token:
        headers["authorization"] = f"Bearer {token}"
    if body is not None:
        data = json.dumps(body).encode()
        headers["content-type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    t0 = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read()
            elapsed = time.perf_counter() - t0
            parsed: Any = None
            if raw:
                try:
                    parsed = json.loads(raw.decode())
                except json.JSONDecodeError:
                    parsed = raw.decode(errors="replace")
            return resp.status, parsed, elapsed
    except urllib.error.HTTPError as e:
        elapsed = time.perf_counter() - t0
        raw = e.read()
        parsed = None
        if raw:
            try:
                parsed = json.loads(raw.decode())
            except json.JSONDecodeError:
                parsed = raw.decode(errors="replace")
        return e.code, parsed, elapsed


def pct(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    k = min(len(ordered) - 1, max(0, int(round((p / 100.0) * (len(ordered) - 1)))))
    return ordered[k]


def summarize(name: str, samples: list[float], errors: int) -> dict[str, Any]:
    n = len(samples)
    total = sum(samples) if samples else 0.0
    return {
        "op": name,
        "n": n,
        "errors": errors,
        "rps": (n / total) if total else 0.0,
        "p50_ms": round(pct(samples, 50) * 1000, 3),
        "p95_ms": round(pct(samples, 95) * 1000, 3),
        "p99_ms": round(pct(samples, 99) * 1000, 3),
        "mean_ms": round((statistics.mean(samples) * 1000) if samples else 0.0, 3),
    }


def cmd_out(args: list[str]) -> str:
    try:
        return subprocess.check_output(args, text=True, stderr=subprocess.DEVNULL).strip()
    except (OSError, subprocess.CalledProcessError):
        return ""


def rss_mb(pid: int | None) -> float | None:
    if not pid:
        return None
    try:
        out = subprocess.check_output(["ps", "-o", "rss=", "-p", str(pid)], text=True)
        return round(int(out.strip()) / 1024.0, 1)
    except (OSError, subprocess.CalledProcessError, ValueError):
        return None


def cpu_pct(pid: int | None) -> float | None:
    if not pid:
        return None
    try:
        out = subprocess.check_output(["ps", "-o", "%cpu=", "-p", str(pid)], text=True)
        return float(out.strip())
    except (OSError, subprocess.CalledProcessError, ValueError):
        return None


def find_listen_pid(port: int) -> int | None:
    try:
        out = subprocess.check_output(["lsof", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN"], text=True)
    except (OSError, subprocess.CalledProcessError):
        return None
    for line in out.splitlines()[1:]:
        parts = line.split()
        if len(parts) >= 2 and parts[1].isdigit():
            return int(parts[1])
    return None


def register(base: str) -> str:
    suffix = f"{int(time.time())}-{os.getpid()}"
    status, body, _ = request(
        f"{base}/api/v1/auth/register",
        "POST",
        body={
            "name": "Bench",
            "email": f"bench-{suffix}@example.com",
            "password": "password123",
            "tenant_name": f"Bench {suffix}",
            "tenant_slug": f"bench-{suffix}"[:40],
        },
    )
    if status != 200 or not isinstance(body, dict) or "access_token" not in body:
        raise SystemExit(f"register failed {status}: {body}")
    return str(body["access_token"])


def pick_entity(ui: dict[str, Any]) -> dict[str, Any]:
    entities = ui.get("entities") or []
    if not isinstance(entities, list) or not entities:
        raise SystemExit("GET /api/v1/meta/ui returned no entities")
    for e in entities:
        perms = e.get("permissions") or {}
        if perms.get("create") is False:
            continue
        fields = e.get("fields") or []
        required = [
            f
            for f in fields
            if f.get("required")
            and f.get("name") not in ("id", "tenant_id", "created_at", "updated_at")
            and f.get("type") not in ("child_table", "relation")
        ]
        if any(f.get("type") == "relation" and f.get("required") for f in fields):
            continue
        if required and all(
            f.get("type") in ("string", "text", "integer", "enum") or f.get("default") is not None
            for f in required
        ):
            return e
    return entities[0]


def sample_payload(entity: dict[str, Any], tag: str) -> dict[str, Any]:
    payload: dict[str, Any] = {}
    for field in entity.get("fields") or []:
        name = field.get("name")
        if not name or name in ("id", "tenant_id", "created_at", "updated_at"):
            continue
        if field.get("computed") or field.get("type") in ("child_table", "relation"):
            continue
        if field.get("required") or name in ("name", "title"):
            ftype = field.get("type")
            widget = (field.get("widget") or "").lower()
            if ftype in ("string", "text"):
                if name == "email" or widget == "email" or (field.get("validation") or {}).get("email"):
                    payload[name] = f"{tag}-{name}@bench.example"
                else:
                    payload[name] = f"{tag}-{name}"[:80]
            elif ftype == "integer":
                payload[name] = 1
            elif ftype == "enum":
                values = field.get("values") or field.get("options") or []
                payload[name] = values[0] if values else "Draft"
            elif ftype in ("boolean",):
                payload[name] = True
    if not payload:
        payload["name"] = tag
    return payload


def timed_loop(fn, n: int) -> tuple[list[float], int]:
    samples: list[float] = []
    errors = 0
    for _ in range(n):
        ok, elapsed = fn()
        samples.append(elapsed)
        if not ok:
            errors += 1
    return samples, errors


def concurrency_get(url: str, token: str, n: int, inflight: int) -> dict[str, Any]:
    samples: list[float] = []
    errors = 0

    def one() -> tuple[bool, float]:
        status, _, elapsed = request(url, token=token)
        return status < 400, elapsed

    t0 = time.perf_counter()
    with ThreadPoolExecutor(max_workers=inflight) as pool:
        futs = [pool.submit(one) for _ in range(n)]
        for fut in as_completed(futs):
            ok, elapsed = fut.result()
            samples.append(elapsed)
            if not ok:
                errors += 1
    wall = time.perf_counter() - t0
    row = summarize(f"list_c{inflight}", samples, errors)
    row["wall_s"] = round(wall, 3)
    row["throughput_rps"] = round(len(samples) / wall, 2) if wall else 0.0
    row["concurrency"] = inflight
    return row


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default=os.environ.get("QEFRO_URL", "http://127.0.0.1:8080"))
    parser.add_argument("--out", default="benches/results")
    parser.add_argument("--ops", type=int, default=50, help="iterations per sequential op")
    parser.add_argument("--max-concurrency", type=int, default=100)
    parser.add_argument("--concurrency-requests", type=int, default=200)
    args = parser.parse_args()
    base = args.url.rstrip("/")

    health_status, health, health_s = request(f"{base}/health")
    if health_status != 200:
        print(f"server not ready at {base}/health ({health_status})", file=sys.stderr)
        return 1

    token = register(base)
    ui_status, ui, _ = request(f"{base}/api/v1/meta/ui", token=token)
    if ui_status != 200 or not isinstance(ui, dict):
        print(f"meta/ui failed {ui_status}: {ui}", file=sys.stderr)
        return 1
    entity = pick_entity(ui)
    slug = entity.get("slug") or entity.get("entity")
    if not slug:
        print("entity has no slug", file=sys.stderr)
        return 1
    payload = sample_payload(entity, f"b{int(time.time()) % 100000}")

    results: list[dict[str, Any]] = []
    created_ids: list[str] = []
    first_error: Any = None

    def create_one() -> tuple[bool, float]:
        nonlocal first_error
        body_payload = sample_payload(entity, f"b{uuid.uuid4().hex[:12]}")
        status, body, elapsed = request(f"{base}/api/v1/{slug}", "POST", token, body_payload)
        ok = status in (200, 201) and isinstance(body, dict) and body.get("id")
        if ok:
            created_ids.append(str(body["id"]))
            payload.clear()
            payload.update(body_payload)
        elif first_error is None:
            first_error = {"status": status, "body": body}
        return bool(ok), elapsed

    samples, errors = timed_loop(create_one, args.ops)
    results.append(summarize("create", samples, errors))
    if not created_ids:
        print("create produced no ids; aborting remaining ops", file=sys.stderr)
        print(json.dumps({"first_error": first_error, "results": results}, indent=2))
        return 1
    rid = created_ids[-1]

    def get_one() -> tuple[bool, float]:
        status, body, elapsed = request(f"{base}/api/v1/{slug}/{rid}", token=token)
        return status == 200 and isinstance(body, dict), elapsed

    samples, errors = timed_loop(get_one, args.ops)
    results.append(summarize("get", samples, errors))

    def list_one() -> tuple[bool, float]:
        status, body, elapsed = request(
            f"{base}/api/v1/{slug}?page=1&page_size=25", token=token
        )
        return status == 200, elapsed

    samples, errors = timed_loop(list_one, args.ops)
    results.append(summarize("list", samples, errors))

    q = urllib.request.quote(str(next(iter(payload.values()))))
    def search_one() -> tuple[bool, float]:
        status, _, elapsed = request(f"{base}/api/v1/{slug}?search={q}", token=token)
        return status == 200, elapsed

    samples, errors = timed_loop(search_one, args.ops)
    results.append(summarize("search", samples, errors))

    patch_body = dict(payload)
    first_str = next((k for k, v in payload.items() if isinstance(v, str)), None)
    if first_str:
        patch_body[first_str] = payload[first_str] + "-u"

        def update_one() -> tuple[bool, float]:
            status, _, elapsed = request(
                f"{base}/api/v1/{slug}/{rid}", "PATCH", token, patch_body
            )
            return status in (200, 204), elapsed

        samples, errors = timed_loop(update_one, min(args.ops, 20))
        results.append(summarize("update", samples, errors))

    got_status, got, _ = request(f"{base}/api/v1/{slug}/{rid}", token=token)
    if got_status == 200 and isinstance(got, dict):
        if got.get("_expanded"):
            results.append({"op": "relation_expand", "present": True, "keys": list(got["_expanded"].keys())})
        else:
            results.append({"op": "relation_expand", "present": False})
        children = [k for k, v in got.items() if isinstance(v, list) and k not in ("_related",)]
        related = got.get("_related")
        results.append(
            {
                "op": "child_or_related",
                "child_list_fields": children[:8],
                "related": bool(related),
            }
        )
        wf = got.get("_workflow") or {}
        transitions = wf.get("allowed") or wf.get("transitions") or []
        if transitions:
            name = transitions[0] if isinstance(transitions[0], str) else transitions[0].get("name")
            if name:
                st, _, elapsed = request(
                    f"{base}/api/v1/{slug}/{rid}/transition",
                    "POST",
                    token,
                    {"transition": name},
                )
                results.append(
                    {
                        "op": "workflow_transition",
                        "transition": name,
                        "status": st,
                        "latency_ms": round(elapsed * 1000, 3),
                    }
                )
        else:
            results.append({"op": "workflow_transition", "present": False})

    levels = [1, 10, 50, 100, 250, 500, 1000]
    levels = [n for n in levels if n <= args.max_concurrency]
    list_url = f"{base}/api/v1/{slug}?page=1&page_size=25"
    for inflight in levels:
        row = concurrency_get(list_url, token, args.concurrency_requests, inflight)
        results.append(row)
        if row["errors"] / max(row["n"], 1) > 0.01:
            results.append(
                {
                    "op": "concurrency_stop",
                    "reason": "error rate > 1%",
                    "at": inflight,
                }
            )
            break

    port = 8080
    try:
        port = int(base.rsplit(":", 1)[-1].split("/")[0])
    except ValueError:
        pass
    pid = find_listen_pid(port)
    env = {
        "recorded_at": datetime.now(timezone.utc).isoformat(),
        "url": base,
        "os": platform.platform(),
        "python": platform.python_version(),
        "rustc": cmd_out(["rustc", "-V"]),
        "qefro_cli": cmd_out(["qefro", "--version"]),
        "qefro_runtime": (health or {}).get("framework") if isinstance(health, dict) else None,
        "postgres": cmd_out(["psql", "--version"]),
        "health": health,
        "health_ms": round(health_s * 1000, 3),
        "entity": entity.get("entity") or entity.get("name"),
        "slug": slug,
        "schema_version": ui.get("schema_version"),
        "server_pid": pid,
        "server_rss_mb": rss_mb(pid),
        "server_cpu_pct": cpu_pct(pid),
        "dataset_created": len(created_ids),
        "frappe": "not executed",
    }
    report = {"env": env, "results": results}
    out_dir = args.out
    os.makedirs(out_dir, exist_ok=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    path = os.path.join(out_dir, f"bench-{stamp}.json")
    with open(path, "w") as f:
        json.dump(report, f, indent=2)
    print(json.dumps(env, indent=2))
    for row in results:
        print(row)
    print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
