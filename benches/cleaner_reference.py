#!/usr/bin/env python3
import json
import os
import pathlib
import sys


def classify(path: pathlib.Path):
    parent = path.parent
    if path.name == "target" and (parent / "Cargo.toml").is_file():
        if any((path / marker).exists() for marker in (".rustc_info.json", "CACHEDIR.TAG", "debug", "release")):
            return "rust-target", parent
    if path.name == "node_modules" and (parent / "package.json").is_file():
        return "node-modules", parent
    if path.name == "cache" and parent.name == ".next" and (parent.parent / "package.json").is_file():
        return "next-cache", parent.parent
    if path.name == ".turbo" and (parent / "package.json").is_file():
        return "turbo-cache", parent
    if path.name == ".vite" and parent.name == "node_modules" and (parent.parent / "package.json").is_file():
        return "vite-cache", parent.parent
    return None


def measure(root: pathlib.Path, device: int, uid: int):
    total_bytes = 0
    total_files = 0
    stack = [root]
    while stack:
        path = stack.pop()
        try:
            stat = path.lstat()
        except OSError:
            continue
        if path.is_symlink() or stat.st_dev != device or stat.st_uid != uid:
            continue
        if path.is_file():
            total_bytes += stat.st_size
            total_files += 1
        elif path.is_dir():
            try:
                stack.extend(path.iterdir())
            except OSError:
                pass
    return total_bytes, total_files


def scan(root: pathlib.Path):
    root = root.resolve(strict=True)
    root_stat = root.stat()
    stack = [root]
    candidates = []
    while stack:
        directory = stack.pop()
        try:
            entries = list(directory.iterdir())
        except OSError:
            continue
        for path in entries:
            try:
                stat = path.lstat()
            except OSError:
                continue
            if path.is_symlink() or not path.is_dir() or stat.st_dev != root_stat.st_dev or stat.st_uid != root_stat.st_uid:
                continue
            if path.name == ".git":
                continue
            found = classify(path)
            if found:
                byte_count, file_count = measure(path, root_stat.st_dev, root_stat.st_uid)
                candidates.append({"kind": found[0], "bytes": byte_count, "files": file_count})
            else:
                stack.append(path)
    candidates.sort(key=lambda item: (item["kind"], item["bytes"], item["files"]))
    return {
        "schemaVersion": 1,
        "candidateCount": len(candidates),
        "totalBytes": sum(item["bytes"] for item in candidates),
        "totalFiles": sum(item["files"] for item in candidates),
    }


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: cleaner_reference.py ROOT")
    print(json.dumps(scan(pathlib.Path(sys.argv[1])), sort_keys=True))
