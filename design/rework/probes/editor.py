#!/usr/bin/env python3
"""Probe authored selector navigation in the isolated Step 1 workspace."""

import argparse
import json
from pathlib import Path
import queue
import subprocess
import threading
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("workspace", type=Path)
    parser.add_argument("--rust-analyzer", required=True)
    parser.add_argument("--source", default="crates/vihaco/tests/rework_contracts.rs")
    parser.add_argument("--probe", action="append", help="LABEL=NEEDLE to inspect a path's final segment")
    args = parser.parse_args()
    workspace = args.workspace.resolve()
    path = workspace / args.source
    source = path.read_text()
    uri = path.as_uri()
    messages = queue.Queue()
    process = subprocess.Popen(
        [args.rust_analyzer], cwd=workspace, stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL
    )

    def receive():
        while True:
            headers = {}
            while True:
                line = process.stdout.readline()
                if not line:
                    messages.put({"error": "server closed stdout"})
                    return
                if line == b"\r\n":
                    break
                name, value = line.decode().split(":", 1)
                headers[name.lower()] = value.strip()
            messages.put(json.loads(process.stdout.read(int(headers["content-length"]))))

    threading.Thread(target=receive, daemon=True).start()
    serial = 0

    def send(method, params, request=False):
        nonlocal serial
        message = {"jsonrpc": "2.0", "method": method, "params": params}
        if request:
            serial += 1
            message["id"] = serial
        data = json.dumps(message).encode()
        process.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
        process.stdin.flush()
        return serial

    def next_message(timeout):
        message = messages.get(timeout=timeout)
        if "method" in message and "id" in message:
            # This probe uses initializationOptions and needs no client services.
            response = {"jsonrpc": "2.0", "id": message["id"], "result": None}
            data = json.dumps(response).encode()
            process.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
            process.stdin.flush()
        return message

    def request(method, params):
        identity = send(method, params, request=True)
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            message = next_message(max(0.1, deadline - time.monotonic()))
            if message.get("id") == identity and "method" not in message:
                if "error" in message:
                    raise RuntimeError(message["error"])
                return message.get("result")
        raise TimeoutError(method)

    try:
        request("initialize", {
            "processId": None, "rootUri": workspace.as_uri(),
            "capabilities": {"experimental": {"serverStatusNotification": True}},
            "initializationOptions": {
                "cargo": {"allTargets": True}, "procMacro": {"enable": True},
                "checkOnSave": False
            }
        })
        send("initialized", {})
        send("textDocument/didOpen", {"textDocument": {
            "uri": uri, "languageId": "rust", "version": 1, "text": source
        }})
        deadline = time.monotonic() + 180
        while True:
            message = next_message(max(0.1, deadline - time.monotonic()))
            if message.get("method") == "experimental/serverStatus" and message["params"].get("quiescent"):
                break
            if time.monotonic() >= deadline:
                raise TimeoutError("workspace initialization")

        results = {"rust_analyzer": subprocess.check_output(
            [args.rust_analyzer, "--version"], text=True
        ).strip()}
        probes = [
            ("reference_authored", "Add(field::cpu::Add)"),
            ("staged_merge", "Halt(field::cpu::Halt)"),
            ("mount_helper", "Stop(field::cpu::Halt)"),
            ("canonical_direct", "Option<canonical::cpu::Halt>"),
            ("provenance", "Option<__vihaco_inventory_two_aliases::__types::surface::provider_1::cpu::Halt>")
        ]
        if args.probe:
            probes = [probe.split("=", 1) for probe in args.probe]
        for label, needle in probes:
            offset = source.index(needle) + needle.rindex("::") + 2
            prefix = source[:offset]
            position = {"line": prefix.count("\n"), "character": len(prefix.rsplit("\n", 1)[-1])}
            params = {"textDocument": {"uri": uri}, "position": position}
            results[label] = {
                "hover": request("textDocument/hover", params),
                "definition": request("textDocument/definition", params),
                "completion": request("textDocument/completion", params)
            }
        # Print the relevant operations, excluding the large completion payload.
        for label, result in results.items():
            if label == "rust_analyzer":
                continue
            completion = result["completion"]
            items = completion.get("items", []) if isinstance(completion, dict) else completion or []
            result["completion_labels"] = [item["label"] for item in items]
            del result["completion"]
        print(json.dumps(results, indent=2), flush=True)
        request("shutdown", None)
        send("exit", None)
    finally:
        process.terminate()
        process.wait(timeout=10)


if __name__ == "__main__":
    main()
