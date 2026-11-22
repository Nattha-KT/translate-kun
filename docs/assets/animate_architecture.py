"""Add a CSS-only "Live" signal flow to an Archify SVG export.

Archify's Live mode is JavaScript, and GitHub renders a README SVG through
<img>, which runs CSS animations but never scripts. This script imitates Live
with CSS only:

- a glowing comet head with a soft tail travels along every relationship,
  in the order one translation runs, on one shared cycle;
- the target box glows when the comet arrives.

prefers-reduced-motion turns all of it off and leaves the static diagram.

Usage: python3 docs/assets/animate_architecture.py in.svg out.svg
(in.svg: Archify viewer -> Export -> "SVG · Dark", signal-flow style)
"""

import re
import sys

# Relationship id -> target node id, in the order one translation runs.
ORDER = [
    ("you-to-cli", "cli"),
    ("cli-reads-config", "config"),
    ("cli-to-guard", "guard"),
    ("guard-to-terms", "terms"),
    ("terms-to-budget", "budget"),
    ("budget-writes-usage", "usage"),
    ("budget-to-provider", "provider"),
    ("provider-gets-key", "creds"),
    ("provider-to-google", "google"),
    ("provider-to-anthropic", "anthropic"),
]
STEP = 0.55     # seconds between consecutive relationships
TRAVEL = 0.9    # seconds a comet takes to cross one relationship
GLOW = 0.9      # seconds a target box glows after the comet arrives
CYCLE = STEP * len(ORDER) + TRAVEL + GLOW

FILTERS = (
    "<defs>"
    '<filter id="kun-glow" x="-50%" y="-50%" width="200%" height="200%">'
    '<feGaussianBlur stdDeviation="2.2" result="b"/>'
    '<feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>'
    '<filter id="kun-glow-strong" x="-30%" y="-60%" width="160%" height="220%">'
    '<feGaussianBlur stdDeviation="4" result="b"/>'
    '<feMerge><feMergeNode in="b"/><feMergeNode in="b"/>'
    '<feMergeNode in="SourceGraphic"/></feMerge></filter>'
    "</defs>"
)


def pct(seconds: float) -> float:
    return seconds / CYCLE * 100


def css() -> str:
    on = pct(TRAVEL)
    return f"""
.kun-tail, .kun-head {{
  stroke-linecap: round;
  marker-start: none !important;
  marker-end: none !important;
  opacity: 0;
  filter: url(#kun-glow);
  animation: kun-comet {CYCLE:.2f}s linear infinite;
}}
.kun-tail {{
  stroke-width: 5 !important;
  stroke-dasharray: 26 140 !important;
  stroke-dashoffset: 26;
}}
.kun-head {{
  stroke: #e6fffb;
  stroke-width: 3 !important;
  stroke-dasharray: 5 140 !important;
  stroke-dashoffset: 5;
}}
.kun-node-glow {{
  fill: none !important;
  stroke-width: 2.5 !important;
  opacity: 0;
  filter: url(#kun-glow-strong);
  animation: kun-node {CYCLE:.2f}s ease-out infinite;
}}
@keyframes kun-comet {{
  0% {{ stroke-dashoffset: 26; opacity: 0; }}
  1% {{ opacity: 1; }}
  {on:.2f}% {{ stroke-dashoffset: -100; opacity: 1; }}
  {on + 0.6:.2f}% {{ opacity: 0; }}
  100% {{ stroke-dashoffset: -100; opacity: 0; }}
}}
@keyframes kun-node {{
  0% {{ opacity: 0; }}
  {pct(0.15):.2f}% {{ opacity: 1; }}
  {pct(GLOW):.2f}% {{ opacity: 0; }}
  100% {{ opacity: 0; }}
}}
@media (prefers-reduced-motion: reduce) {{
  .kun-tail, .kun-head, .kun-node-glow {{ animation: none; display: none; }}
}}
"""


def main(src: str, dst: str) -> None:
    svg = open(src, encoding="utf-8").read()
    split = svg.rfind("</style>")
    head, body = svg[:split], svg[split:]
    edge_index = {edge: i for i, (edge, _) in enumerate(ORDER)}
    node_index = {node: i for i, (_, node) in enumerate(ORDER)}
    found_edges, found_nodes = [], []

    def add_comet(match: re.Match) -> str:
        tag = match.group(0)
        edge_id = re.search(r'data-edge-id="([^"]+)"', tag).group(1)
        if edge_id not in edge_index:
            return tag
        d = re.search(r'\sd="([^"]+)"', tag).group(1)
        cls = re.search(r'\sclass="([^"]+)"', tag)
        edge_class = cls.group(1) if cls else ""
        delay = f"animation-delay:{edge_index[edge_id] * STEP:.2f}s"
        found_edges.append(edge_id)
        common = f'd="{d}" pathLength="100" fill="none" pointer-events="none" aria-hidden="true"'
        return (
            tag
            + f'<path class="{edge_class} kun-tail" {common} style="{delay}"/>'
            + f'<path class="kun-head" {common} style="{delay}"/>'
        )

    def add_node_glow(match: re.Match) -> str:
        before, node_id, rect = match.group(1), match.group(2), match.group(3)
        if node_id not in node_index:
            return match.group(0)
        found_nodes.append(node_id)
        delay = node_index[node_id] * STEP + TRAVEL
        glow = re.sub(r'\sclass="([^"]+)"', r' class="\1 kun-node-glow"', rect)
        glow = re.sub(r'\s(data-animate|style)="[^"]*"', "", glow)
        glow = glow.replace(
            "<rect ", f'<rect style="animation-delay:{delay:.2f}s" aria-hidden="true" ', 1
        )
        return before + rect + glow

    body = re.sub(r'<path\b[^>]*data-edge-id="[^"]+"[^>]*/?>', add_comet, body)
    # Each node's main box is the <rect data-animate="node"> inside <g id="node-...">.
    body = re.sub(
        r'(?s)(<g id="node-([^"]+)"[^>]*>(?:(?!<g id="node-).)*?)(<rect\b[^>]*data-animate="node"[^>]*/>)',
        add_node_glow,
        body,
    )

    missing = [e for e, _ in ORDER if e not in found_edges]
    missing += [n for _, n in ORDER if n not in found_nodes]
    if missing:
        sys.exit(f"not found in SVG: {missing}")

    body = body.replace("</style>", "</style>" + FILTERS, 1)
    open(dst, "w", encoding="utf-8").write(head + css() + body)
    print(
        f"animated {len(found_edges)} relationships and {len(found_nodes)} boxes; "
        f"cycle {CYCLE:.2f}s -> {dst}"
    )


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
