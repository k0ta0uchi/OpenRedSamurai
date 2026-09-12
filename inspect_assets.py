"""Dump dimensions of every UI asset image (sorted by path)."""

import os

from PIL import Image

BASE = os.path.dirname(os.path.abspath(__file__))
IMGROOT = os.path.join(BASE, "assets", "images")


def main() -> None:
    rows = []
    for root, _dirs, files in os.walk(IMGROOT):
        for f in sorted(files):
            if not f.lower().endswith((".png", ".bmp", ".jpg")):
                continue
            path = os.path.join(root, f)
            rel = os.path.relpath(path, IMGROOT).replace(os.sep, "/")
            try:
                im = Image.open(path)
                rows.append((rel, im.size, im.mode, os.path.getsize(path)))
            except Exception as err:  # noqa: BLE001 - report and continue
                rows.append((rel, "ERR", str(err), 0))
    for rel, size, mode, size_bytes in sorted(rows):
        print("%-52s %-12s %-5s %6d" % (rel, size, mode, size_bytes))


if __name__ == "__main__":
    main()
