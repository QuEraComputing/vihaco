#!/usr/bin/env python3
"""Compile static Step 1 fixtures; source stays under design, artifacts in /tmp."""
import argparse
import json
import os
from pathlib import Path
import subprocess

CASES = {
    "missing-coverage": (False, "Execute", True),
    "new-uncovered-leaf": (False, "Omitted", True),
    "generic-coverage": (False, "Copy", True),
    "undeclared-despite-execute": (False, "Supports", True),
    "repeated-dialect-alias": (False, "conflicting implementations", True),
    "duplicate-selected-alias": (False, "SelectedPair", True),
    "two-set-associations": (False, "HasInstructionSet", True),
    "unknown-target": (False, "__vihaco_field_unknown", True),
    "overlap-outside-selection": (False, "overlapping canonical instruction selector", True),
    "force-invalid-generic-const": (False, "overlapping canonical instruction selector", True),
    "same-instruction-different-target": (True, "", True),
    "unused-invalid-const": (True, "", True),
    "invalid-generic-report": (True, "", True),
    "out-of-line": (False, "requires an inline module", True),
    "empty-dialect": (False, "needs a marked instruction", True),
    "named-instruction": (False, "nongeneric unit or tuple struct", True),
    "generic-instruction": (False, "nongeneric unit or tuple struct", True),
    "marked-enum": (False, "requires a unit or tuple struct", True),
    "repeated-marker": (False, "exactly one bare instruction marker", True),
    "conditional-leaf": (False, "conditional instruction membership", True),
    "surface-name-collision": (False, "surface name collides", True),
    "raw-leaf": (False, "raw instruction names", True),
    "raw-parser-reference": (False, "invalid pattern", True),
    "raw-parser-explicit": (False, "proc-macro derive panicked", True),
    "missing-parser-root": (False, "could not find", False),
    "associated-machine-payload": (True, "", True),
    "user-dialect-shape": (True, "", True),
    "misplaced-marker": (False, "cannot find attribute `instruction`", True),
    "nested-marker": (False, "cannot find attribute `instruction`", True),
    "raw-field": (True, "", True),
    "field-index-spellings": (True, "", True),
    "target-source-alias": (False, "__vihaco_field_p", True),
    "mixed-leaf-visibility": (False, "unrestricted pub visibility", True),
    "private-instruction": (False, "unrestricted pub visibility", True),
    "crate-instruction": (False, "unrestricted pub visibility", True),
    "super-instruction": (False, "unrestricted pub visibility", True),
    "scoped-instruction": (False, "unrestricted pub visibility", True),

    "rooted-parser-model": (True, "", False),
}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, default=Path("/tmp/vihaco-rework2-step1-target"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    target = args.target_dir.resolve()
    logs = target / "evidence"
    logs.mkdir(parents=True, exist_ok=True)
    artifacts = {}
    for command in ["test", "build"]:
        cmd = ["cargo", command, "--offline", "--manifest-path", str(root / "Cargo.toml"),
               "--target-dir", str(target)]
        if command == "build":
            cmd.append("--message-format=json")
        result = subprocess.run(cmd, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        (logs / f"cargo-{command}.log").write_text(result.stdout)
        print(f"cargo {command}: {result.returncode}", flush=True)
        if result.returncode:
            raise SystemExit(result.stdout)
        if command == "build":
            for line in result.stdout.splitlines():
                try:
                    message = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if message.get("reason") == "compiler-artifact":
                    artifacts[message["target"]["name"]] = [Path(p) for p in message["filenames"]]
    dependencies = target / "debug/deps"
    def artifact(name, macro=False):
        suffixes = (".dylib", ".so", ".dll") if macro else (".rlib",)
        found = [p for p in artifacts[name] if p.suffix in suffixes]
        if len(found) != 1:
            raise RuntimeError(f"expected one artifact for {name}: {found}; use a fresh target directory")
        return found[0]
    externs = {"vm": artifact("proof_support"), "external": artifact("proof_producer"),
               "chain": artifact("proof_middle"), "consumer": artifact("proof_consumer"),
               "declarations": artifact("proof_macros", True),
               "chumsky": artifact("chumsky"), "vihaco_parser": artifact("vihaco_parser")}
    env = dict(os.environ, CARGO_MANIFEST_DIR=str(root / "consumer"))
    report = {}
    failures = []
    for name, (success, diagnostic, parser_deps) in CASES.items():
        for mode in ["metadata", "link"]:
            expected = success
            needle = diagnostic
            if name == "invalid-generic-report" and mode == "link":
                expected = False
                needle = "overlapping canonical instruction selector"
            cmd = ["rustc", "--edition=2024", "--crate-name", "probe_case", str(root / "cases" / f"{name}.rs"),
                   "--emit=" + mode, "--out-dir", str(logs), "-L", f"dependency={dependencies}"]
            for alias, path in externs.items():
                if parser_deps or alias not in ("vihaco_parser", "chumsky"):
                    cmd += ["--extern", f"{alias}={path}"]
            result = subprocess.run(cmd, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            log = logs / f"{name}-{mode}.log"
            log.write_text(result.stdout)
            matched = ((result.returncode == 0) == expected and (not needle or needle in result.stdout))
            report[f"{name}/{mode}"] = {"exit": result.returncode, "expected_success": expected,
                                       "matched": matched, "diagnostic": needle, "log": str(log)}
            if not matched:
                failures.append(f"{name}/{mode}: {result.stdout}")
        print(f"{name}: {'PASS' if all(report[f'{name}/{m}']['matched'] for m in ['metadata', 'link']) else 'FAIL'}", flush=True)
    for name, command, success in [
        ("unused", "check", True), ("unused", "build", True),
        ("report", "check", True), ("report", "build", False),
        ("forced", "check", False), ("forced", "build", False),
    ]:
        cmd = ["cargo", command, "--offline", "--manifest-path", str(root / "validation/Cargo.toml"),
               "--bin", name, "--target-dir", str(target / "validation")]
        result = subprocess.run(cmd, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        log = logs / f"cargo-{name}-{command}.log"
        log.write_text(result.stdout)
        matched = (result.returncode == 0) == success
        if not success:
            matched &= "overlapping canonical instruction selector" in result.stdout
        report[f"cargo/{name}/{command}"] = {"exit": result.returncode,
            "expected_success": success, "matched": matched, "log": str(log)}
        print(f"cargo {command} {name}: {'PASS' if matched else 'FAIL'}", flush=True)
        if not matched:
            failures.append(f"cargo/{name}/{command}: {result.stdout}")
    configurations = [
        ("baseline", "", True, ""),
        ("declarations", "", False, "could not find `dialect`"),
        ("declarations", "runtime-only", False, "could not find `dialect`"),
        ("declarations", "macros-only", False, "InstructionSpec"),
        ("declarations", "typed-declarations", True, ""),
        ("declarations", "indirect", True, ""),
    ]
    for binary, features, success, needle in configurations:
        for command in ["check", "build"]:
            cmd = ["cargo", command, "--offline", "--manifest-path", str(root / "features/Cargo.toml"),
                   "--bin", binary, "--no-default-features", "--target-dir", str(target / "features")]
            if features:
                cmd += ["--features", features]
            result = subprocess.run(cmd, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            name = f"feature-{binary}-{features or 'off'}-{command}"
            log = logs / f"{name}.log"
            log.write_text(result.stdout)
            matched = (result.returncode == 0) == success and (not needle or needle in result.stdout)
            report[name] = {"exit": result.returncode, "expected_success": success,
                            "matched": matched, "diagnostic": needle, "log": str(log)}
            print(f"{name}: {'PASS' if matched else 'FAIL'}", flush=True)
            if not matched:
                failures.append(f"{name}: {result.stdout}")
    (logs / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    if failures:
        raise SystemExit("\n".join(failures))
    print(f"{len(report)} compiler outcomes matched; logs in {logs}", flush=True)

if __name__ == "__main__":
    main()
