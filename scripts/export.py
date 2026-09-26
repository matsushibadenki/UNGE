#!/usr/bin/env python3
"""Export only reusable source files. No dependencies outside Python's standard library."""
from pathlib import Path
import argparse
import os
import re
import zipfile

ROOT = Path(__file__).resolve().parent.parent
ROOT_FILES = {"Cargo.toml", "Cargo.lock", "README.md", "AGENTS.md", "LICENSE-MIT", "LICENSE-APACHE", ".gitignore"}
DIRECTORIES = {"crates", "bindings", "docs", "examples", "scripts", ".github"}
EXCLUDED = {"target", "node_modules", "gen", ".git", "__pycache__", "dist"}


def export(destination: Path) -> tuple[Path, int]:
    version = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.MULTILINE).group(1)
    files = [ROOT / name for name in ROOT_FILES]
    for directory in sorted(DIRECTORIES):
        for parent, dirs, names in os.walk(ROOT / directory):
            dirs[:] = sorted(d for d in dirs if d not in EXCLUDED and not (Path(parent) / d).is_symlink())
            files.extend(Path(parent) / name for name in sorted(names) if name != ".DS_Store" and not (Path(parent) / name).is_symlink())
    destination.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for source in sorted(files):
            archive.write(source, Path(f"unge-{version}") / source.relative_to(ROOT))
    return destination, len(files)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "dist" / "unge-0.1.0.zip")
    args = parser.parse_args()
    path, count = export(args.output.resolve())
    print(f"{path} ({count} files)")
