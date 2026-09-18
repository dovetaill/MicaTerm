#!/usr/bin/env python3
"""Durable version preparation followed by Windows publication and a local tag."""
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def release(root):
    if os.environ.get("PROFILE", "release") != "release":
        raise ValueError("use --build-only for non-release profiles")
    if git(root, "status", "--porcelain"):
        raise ValueError("commit/stash changes first, or use --build-only for development")
    if git(root, "rev-parse", "--is-shallow-repository") == "true":
        raise ValueError("fetch full history and tags before releasing")
    git(root, "symbolic-ref", "--quiet", "HEAD")
    manifest, lock = root / "Cargo.toml", root / "Cargo.lock"
    package = tomllib.loads(manifest.read_text())["package"]
    version = package["version"]
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("automatic releases require a stable major.minor.patch version")
    own = [p for p in tomllib.loads(lock.read_text())["package"]
           if p["name"] == package["name"] and "source" not in p]
    if len(own) != 1 or own[0]["version"] != version:
        raise ValueError("Cargo.toml and Cargo.lock versions do not match")
    head = git(root, "rev-parse", "HEAD")
    tags = git(root, "tag", "--list").splitlines()
    tag = "v" + version
    if tag in tags and git(root, "rev-list", "-n", "1", tag) != head:
        major, minor, patch = map(int, version.split("."))
        next_version = f"{major}.{minor}.{patch + 1}"
        if "v" + next_version in tags:
            raise ValueError("next version already exists; update branch before releasing")
        texts = []
        # TOML parsing identifies the package; preserve unrelated text and dependencies.
        for path, section in [(manifest, "[package]"), (lock, 'name = "' + package["name"] + '"')]:
            text = path.read_text()
            start = text.index(section)
            match = re.search(r'^version = "' + re.escape(version) + r'"$', text[start:], re.M)
            if match is None:
                raise ValueError(f"cannot locate version in {path}")
            a, b = start + match.start(), start + match.end()
            texts.append((path, text[:a] + f'version = "{next_version}"' + text[b:]))
        for path, text in texts:
            path.write_text(text)
        git(root, "add", "Cargo.toml", "Cargo.lock")
        git(root, "commit", "-m", f"chore(release): prepare v{next_version}")
        version, tag = next_version, "v" + next_version
        head = git(root, "rev-parse", "HEAD")
    elif tag not in tags:
        # Untagged manifest versions are pending releases; retry without bumping.
        if git(root, "log", "-1", "--format=%s") != f"chore(release): prepare {tag}":
            git(root, "commit", "--allow-empty", "-m", f"chore(release): prepare {tag}")
            head = git(root, "rev-parse", "HEAD")
    env = os.environ.copy()
    env["MICA_TERM_RELEASE_DATE"] = git(root, "show", "-s", "--format=%cI", head)
    env["MICA_TERM_RELEASE_VERSION"] = version
    print(f"==> Release {tag} at {head[:12]}", flush=True)
    subprocess.run(["bash", str(root / "build-win-x64.sh"), "--build-only"],
                   cwd=root, env=env, check=True)
    if git(root, "rev-parse", "HEAD") != head or git(root, "status", "--porcelain"):
        raise ValueError("checkout changed during build; refusing to tag")
    if tag not in tags:
        git(root, "tag", "-a", tag, head, "-m", f"MicaTerm {tag}")
    print(f"==> Release complete: {tag} (not pushed)")


if __name__ == "__main__":
    try:
        release(Path(__file__).resolve().parents[1])
    except (ValueError, subprocess.CalledProcessError) as error:
        sys.exit(f"error: {error}")
