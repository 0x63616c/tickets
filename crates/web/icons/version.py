"""Update/check icon cache keys without image-generation dependencies."""

import argparse
import hashlib
import json
import re
from pathlib import Path

ICONS = Path(__file__).resolve().parent
WEB = ICONS.parent
# Hash only shipped image assets, excluding the manifest to avoid a circular hash.
IMAGES = (
    "apple-touch-icon-precomposed.png",
    "apple-touch-icon.png",
    "favicon-32.png",
    "favicon.ico",
    "favicon.svg",
    "icon-192.png",
    "icon-512.png",
    "icon-maskable-512.png",
)


def token():
    digest = hashlib.sha256()
    for name in sorted(IMAGES):
        digest.update(name.encode() + b"\0" + (ICONS / name).read_bytes())
    return digest.hexdigest()[:8]


def sync(write=False):
    version = token()
    for path in (WEB / "index.html", ICONS / "site.webmanifest"):
        source = path.read_text()
        expected = re.sub(
            r'(?<=")(/(?:' + "|".join(re.escape(n) for n in (*IMAGES, "site.webmanifest"))
            + r'))(?:\?v=[^"\s]*)?(?=")',
            lambda match: f"{match[1]}?v={version}",
            source,
        )
        if write:
            path.write_text(expected)
        elif source != expected:
            raise SystemExit(f"Stale icon URLs in {path}: run python3 {ICONS / 'version.py'} --write")
    assert (ICONS / IMAGES[0]).read_bytes() == (ICONS / "apple-touch-icon.png").read_bytes(), "Precomposed Apple icon must match apple-touch-icon.png"
    html = (WEB / "index.html").read_text()
    for rel, name in (
        ("icon", "favicon.ico"),
        ("icon", "favicon.svg"),
        ("icon", "favicon-32.png"),
        ("apple-touch-icon", "apple-touch-icon.png"),
        ("manifest", "site.webmanifest"),
        ("mask-icon", "favicon.svg"),
    ):
        assert f'rel="{rel}" href="/{name}?v={version}"' in html, f"Missing {rel} link for {name}"
    assert f'rel="mask-icon" href="/favicon.svg?v={version}" color="#000000"' in html
    assert 'rel="copy-file" href="icons/apple-touch-icon-precomposed.png"' in html
    manifest = json.loads((ICONS / "site.webmanifest").read_text())
    for icon in manifest["icons"]:
        assert icon["src"] in {f"/{name}?v={version}" for name in IMAGES}, icon["src"]
    print(f"Icon version: {version}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true", help="update HTML and manifest URLs")
    mode.add_argument("--check", action="store_true", help="fail on stale/missing URLs or alias")
    sync(write=parser.parse_args().write)
