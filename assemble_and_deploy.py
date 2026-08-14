#!/usr/bin/env python3
"""Assembles Assembler/assembly.masm and copies the resulting binary into
MICA_inter_emu/, so the interpreter's next run picks up whatever you just
assembled instead of a stale binary from a previous test.

Usage:
    python3 Tools/assemble_and_deploy.py          # assemble + copy only
    python3 Tools/assemble_and_deploy.py --run     # also cargo build --release + run

Paths are resolved relative to this script's location, so it can be run
from anywhere (repo root, Tools/, etc.) without needing a specific cwd.
"""
import json
import os
import shutil
import subprocess
import sys

dir_path = os.path.dirname(os.path.realpath(__file__))
assembler_dir = os.path.join(dir_path, "Assembler")
inter_emu_dir = os.path.join(dir_path, "MICA_inter_emu")


def main() -> None:
    run_after = "--run" in sys.argv

    print(f"Assembling {os.path.join(assembler_dir, 'assembly.masm')} ...")
    result = subprocess.run([sys.executable, "assembler.py"], cwd=assembler_dir)
    if result.returncode != 0:
        print("Assembler failed, aborting.", file=sys.stderr)
        sys.exit(result.returncode)

    # Figure out which output file the assembler just produced - the name
    # depends on the configured bit width (e.g. output32.bin).
    with open(os.path.join(assembler_dir, "config.json")) as f:
        assembler_config = json.load(f)
    bitwidth = assembler_config["bit_width"]
    output_name = f"output{bitwidth}.bin"

    src = os.path.join(assembler_dir, output_name)
    dst = os.path.join(inter_emu_dir, output_name)

    if not os.path.isfile(src):
        print(f"Expected assembled output at {src} but it wasn't found.", file=sys.stderr)
        sys.exit(1)

    shutil.copyfile(src, dst)
    print(f"Copied {output_name} -> {dst}")

    if run_after:
        print("Building MICA_inter_emu (release) ...")
        build = subprocess.run(["cargo", "build", "--release"], cwd=inter_emu_dir)
        if build.returncode != 0:
            print("cargo build failed, aborting.", file=sys.stderr)
            sys.exit(build.returncode)

        binary = os.path.join(inter_emu_dir, "target", "release", "MICA_inter_emu")
        print("Running MICA_inter_emu ...")
        subprocess.run([binary], cwd=inter_emu_dir)


if __name__ == "__main__":
    main()
