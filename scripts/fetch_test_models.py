#!/usr/bin/env python3
"""Fetch the models audiocpp-rs's tests and examples use, pinned by sha256.

Files land in models/ (gitignored), in the directory layout audio.cpp's model
specs use, which is where audiocpp/tests/common looks (or $AUDIOCPP_TEST_MODELS).
A download that does not match its pin is deleted and reported, never kept.

  uv run scripts/fetch_test_models.py           every pin
  uv run scripts/fetch_test_models.py --list
  uv run scripts/fetch_test_models.py --only tiny

Standard library only (any Python 3.9+). HF_TOKEN, when set, is sent to
huggingface.co only.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import sys
import tempfile
import urllib.request
from dataclasses import dataclass
from pathlib import Path

MODELS = Path(__file__).resolve().parents[1] / "models"
HF = "https://huggingface.co"


@dataclass(frozen=True)
class Pin:
    path: str  # relative to models/
    url: str
    sha256: str
    size: int
    note: str


PINS = (
    Pin("tiny-voca-GGUF/tiny-voca-orig.gguf",
        f"{HF}/n4ze3m/tiny-voca/resolve/5fe06641d0afecb3c04bc9faca2cf64d9b2b1600/gguf/tiny-voca-orig.gguf",
        "6940a20162d7e6a31141ad42aaa552b4fdb5182edeff9402e5ffaf66adfd72a0", 23_568_000,
        "tiny_voca: 4.5M-parameter English TTS with its own phonemizer (feature model-tiny_voca)"),
    Pin("SenseVoice-Small-GGUF/sensevoice-small-q8-audiocpp-v1.gguf",
        f"{HF}/FunAudioLLM/SenseVoiceSmall-GGUF-audiocpp/resolve/5c3fcfe748a8714216bc135476d5863084fddb72/"
        "sensevoice-small-q8-audiocpp-v1.gguf",
        "4dedf169f625437fb336f2959674f399819729a765e184128c0e25a6e16ff0ec", 254_211_200,
        "SenseVoice Small Q8: multilingual ASR, the round-trip listener (feature model-sense_asr)"),
)


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def fetch(pin: Pin, force: bool) -> bool:
    out = MODELS / pin.path
    if out.exists() and not force:
        if out.stat().st_size == pin.size and sha256_of(out) == pin.sha256:
            print(f"ok      {pin.path}")
            return True
        print(f"warning {pin.path} does not match its pin; downloading again")
    out.parent.mkdir(parents=True, exist_ok=True)
    print(f"fetch   {pin.path} ({pin.size / 1e6:.1f} MB)")
    with tempfile.NamedTemporaryFile(dir=out.parent, suffix=".part", delete=False) as tmp:
        tmp_path = Path(tmp.name)
    try:
        request = urllib.request.Request(pin.url)  # noqa: S310 - pinned https URL
        token = os.environ.get("HF_TOKEN", "")
        if token and pin.url.startswith(HF + "/"):
            request.add_header("Authorization", f"Bearer {token}")
        with urllib.request.urlopen(request) as response, tmp_path.open("wb") as f:  # noqa: S310
            shutil.copyfileobj(response, f, length=1 << 20)
        size, digest = tmp_path.stat().st_size, sha256_of(tmp_path)
        if size != pin.size or digest != pin.sha256:
            print(f"error   {pin.path}: size {size} / sha256 {digest} do not match the pin", file=sys.stderr)
            tmp_path.unlink(missing_ok=True)
            return False
        tmp_path.replace(out)
    except BaseException:
        tmp_path.unlink(missing_ok=True)
        raise
    print(f"ok      {pin.path}")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--only", default="", help="fetch only pins whose path contains this")
    parser.add_argument("--force", action="store_true", help="download again even when verified")
    parser.add_argument("--list", action="store_true", help="list the pins and exit")
    args = parser.parse_args()
    if args.list:
        for pin in PINS:
            state = "present" if (MODELS / pin.path).exists() else "missing"
            print(f"{state:8} {pin.size / 1e6:7.1f} MB  {pin.path}  -- {pin.note}")
        return 0
    selected = [p for p in PINS if args.only in p.path]
    if not selected:
        print(f"error: no pin matches {args.only!r}", file=sys.stderr)
        return 1
    ok = all([fetch(p, args.force) for p in selected])
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
