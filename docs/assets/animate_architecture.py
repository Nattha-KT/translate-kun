"""Add a CSS-only "signal flow" animation to an Archify SVG export.

For every relationship path (data-edge-id), a copy is drawn on top with a short
bright dash that travels from source to target. Edges fire in the order a
translation runs, on one shared cycle. No JavaScript: GitHub renders SVG through
<img>, which runs CSS animations but never scripts. prefers-reduced-motion
turns the animation off and leaves the static diagram.

Usage: python3 animate_svg.py in.svg out.svg
"""

import re
import sys

# The order in which one translation touches each relationship.
ORDER = [
    "you-to-cli",
    "cli-reads-config",
    "cli-to-guard",
    "guard-to-terms",
    "terms-to-budget",
    "budget-writes-usage",
    "budget-to-provider",
    "provider-gets-key",
    "provider-to-google",
    "provider-to-anthropic",
]
STEP = 0.55      # seconds between consecutive edges
TRAVEL = 0.9     # seconds a pulse takes to cross one edge
CYCLE = STEP * len(ORDER) + 1.2


def main(src: str, dst: str) -> None:
    svg = open(src, encoding="utf-8").read()
    body_start = svg.rfind("</style>")
    head, body = svg[:body_start], svg[body_start:]

    found = []

    def add_overlay(match: re.Match) -> str:
        tag = match.group(0)
        edge_id = re.search(r'data-edge-id="([^"]+)"', tag).group(1)
        if edge_id not in ORDER:
            return tag
        d = re.search(r'\sd="([^"]+)"', tag).group(1)
        cls = re.search(r'\sclass="([^"]+)"', tag)
        edge_class = cls.group(1) if cls else ""
        found.append(edge_id)
        idx = ORDER.index(edge_id)
        overlay = (
            f'<path class="{edge_class} kun-pulse" d="{d}" pathLength="100" fill="none" '
            f'style="animation-delay:{idx * STEP:.2f}s" '
            f'pointer-events="none" aria-hidden="true"/>'
        )
        return tag + overlay

    body = re.sub(r"<path\b[^>]*data-edge-id=\"[^\"]+\"[^>]*/?>", add_overlay, body)

    missing = [e for e in ORDER if e not in found]
    if missing:
        sys.exit(f"edges not found in SVG: {missing}")

    on = TRAVEL / CYCLE * 100
    css = f"""
.kun-pulse {{
  stroke-width: 4 !important;
  stroke-linecap: round;
  stroke-dasharray: 22 130 !important;
  stroke-dashoffset: 22;
  opacity: 0;
  marker-end: none !important;
  marker-start: none !important;
  animation: kun-pulse {CYCLE:.2f}s linear infinite;
}}
@keyframes kun-pulse {{
  0% {{ stroke-dashoffset: 22; opacity: 0; }}
  1% {{ opacity: 1; }}
  {on:.2f}% {{ stroke-dashoffset: -100; opacity: 1; }}
  {on + 0.5:.2f}% {{ opacity: 0; }}
  100% {{ stroke-dashoffset: -100; opacity: 0; }}
}}
@media (prefers-reduced-motion: reduce) {{
  .kun-pulse {{ animation: none; display: none; }}
}}
"""
    out = head + css + body
    open(dst, "w", encoding="utf-8").write(out)
    print(f"animated {len(found)} edges, cycle {CYCLE:.2f}s -> {dst}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
