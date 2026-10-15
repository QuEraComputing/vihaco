#!/usr/bin/env python3
"""Run Step 1 fixtures against the preserved draft, without changing this checkout."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

REFERENCE = "2b87ad69e0a312c62c2157b6317b48b386215cdd"
PROBES = Path(__file__).resolve().parent
REPOSITORY = PROBES.parents[2]


def check_attribute_grammar(work):
    """Distinguish compiler acceptance from the component helper's policy."""
    directory = work / "attribute-grammar"
    directory.mkdir()
    shim = directory / "consume.rs"
    shim.write_text(
        "extern crate proc_macro;\nuse proc_macro::TokenStream;\n"
        "#[proc_macro_attribute]\npub fn consume(_: TokenStream, _: TokenStream) "
        "-> TokenStream { TokenStream::new() }\n"
    )
    flags = ["rustc", "--edition=2024", "--crate-type=proc-macro", str(shim)]
    library = subprocess.check_output(flags + ["--print=file-names"], text=True).strip()
    subprocess.run(flags + ["--out-dir", str(directory)], check=True)
    for name, attribute in [
        ("braces", "#[instructions { cpu }]"),
        ("equals", "#[instructions = { cpu }]")
    ]:
        source = directory / f"{name}.rs"
        source.write_text(f"#[consume::consume]\n{attribute}\nstruct Device;\nfn main() {{}}\n")
        subprocess.run(
            ["rustc", "--edition=2024", str(source), "--extern",
             f"consume={directory / library}", "--out-dir", str(directory)], check=True
        )
        print(f"Compiler accepted {attribute} when the outer macro consumes it", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, help="fresh directory for the archived draft")
    parser.add_argument("--bless", action="store_true", help="review and update diagnostic snapshots")
    args = parser.parse_args()
    work = (args.work_dir or Path(tempfile.mkdtemp(prefix="vihaco-rework-step1-"))).resolve()
    work.mkdir(parents=True, exist_ok=True)
    if any(work.iterdir()):
        parser.error("--work-dir must be empty (the runner only changes an isolated archive)")
    print(f"Reference: {REFERENCE}\nArchive: {work}", flush=True)
    archive = subprocess.Popen(
        ["git", "archive", REFERENCE], cwd=REPOSITORY, stdout=subprocess.PIPE
    )
    try:
        subprocess.run(["tar", "-x", "-C", str(work)], stdin=archive.stdout, check=True)
    finally:
        archive.stdout.close()
    if archive.wait():
        raise RuntimeError("could not archive the reference revision")
    check_attribute_grammar(work)
    tests = work / "crates/vihaco/tests"
    for path in PROBES.glob("*.rs"):
        shutil.copy2(path, tests / path.name)
    fixtures = tests / "compile_fail/rework"
    shutil.copytree(PROBES / "compile_fail", fixtures)
    env = os.environ.copy()
    # Snapshot rewriting is always explicit, even if the shell has TRYBUILD set.
    env.pop("TRYBUILD", None)
    if args.bless:
        env["TRYBUILD"] = "overwrite"
    result = subprocess.run(
        ["cargo", "test", "--offline", "-p", "vihaco", "--test", "rework_contracts",
         "--test", "rework_compile_fail"], cwd=work, env=env
    )
    if args.bless:
        for snapshot in fixtures.glob("*.stderr"):
            shutil.copy2(snapshot, PROBES / "compile_fail" / snapshot.name)
    print(f"Retained probe workspace: {work}", flush=True)
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
