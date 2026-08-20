#!/usr/bin/env python3
"""Morrow pixel-art logo generator.

The grid below is the design source: 32 rows x 32 chars, one char per 16px
cell. Palette: 2=night 3=dusk 4=dawn H=horizon M=moon S=sun .=outside the
rounded square. Run: python3 scripts/gen-logo.py  (writes assets/logo.svg)
"""
import sys
from pathlib import Path

GRID = [
    ".......222222222222222222.......",
    ".....2222222222222222222222.....",
    "...22222222222222222222222222...",
    "..2222222222222222222222222222..",
    ".222222222222222222222222222222.",
    "22222222222222222222222222222222",
    "222222......MMMMMM22222222222222",
    "22222.......MMMMM222222222222222",
    "22222222....MMMMM2222222M2222222",
    "222222......MMMMM222222222222222",
    "22222.....MMMMM2222222222222M222",
    "22222.....MMMMM22222222222222222",
    "22222.....MMMMM222222M2222222222",
    "222222......MMMMM222222222222222",
    "333333......MMMMM333333333333333",
    "33333.....MMMMM33333333333333333",
    "33333.....MMMMM33333333333333333",
    "3333333.....MMMMM333333333333333",
    "333333333.....MMMMM3333333333333",
    "33333333333.....MMMMM33333333333",
    "3333333333333.....MMMMM333333333",
    "4444444444444444444444444SSSSS44",
    "44444444444444444444444SSSSSSSSS",
    "4444444444444444444444SSSSSSSSSS",
    "444444444444444444444SSSSSSSSSSS",
    "44444444444444444444SSSSSSSSSSSS",
    "44444444444444444444SSSSSSSSSSSS",
    ".HHHHHHHHHHHHHHHHHHHHHHHHHHHHHH.",
    "..HHHHHHHHHHHHHHHHHHHHHHHHHHHH..",
    "...HHHHHHHHHHHHHHHHHHHHHHHHHH...",
    ".....HHHHHHHHHHHHHHHHHHHHHH.....",
    ".......HHHHHHHHHHHHHHHHHH.......",
]

PALETTE = {
    "2": "#16163a",  # night sky
    "3": "#3b2f6e",  # dusk
    "4": "#ff6b35",  # dawn
    "H": "#120f24",  # horizon
    "M": "#e6e6f2",  # moon / stars
    "S": "#ffd27d",  # sun
    ".": None,
}

CELL = 16  # 32 x 32 cells in a 512 x 512 canvas


def main() -> None:
    assert len(GRID) == 32, f"{len(GRID)} rows, want 32"
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">',
           "  <!-- Morrow pixel mark. Edit the GRID in scripts/gen-logo.py and re-run. -->"]
    for y, row in enumerate(GRID):
        assert len(row) == 32, f"row {y}: {len(row)} cols, want 32"
        bad = set(row) - set(PALETTE)
        assert not bad, f"row {y}: unknown chars {sorted(bad)}"
        x = 0
        while x < 32:
            ch = row[x]
            if ch == ".":
                x += 1
                continue
            run = 1
            while x + run < 32 and row[x + run] == ch:
                run += 1
            out.append(f'  <rect x="{x*CELL}" y="{y*CELL}" width="{run*CELL}" height="{CELL}" fill="{PALETTE[ch]}"/>')
            x += run
    out.append("</svg>")
    path = Path(__file__).resolve().parent.parent / "assets" / "logo.svg"
    path.write_text("\n".join(out) + "\n")
    print(f"wrote {path} ({len(out)} lines)")


if __name__ == "__main__":
    sys.exit(main())
