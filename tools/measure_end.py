#!/usr/bin/env python3
"""Measures how far the ddsom-game engine still is from its end shape (docs/fin.md, section 1).

    tools/measure_end.py [--rev REV] [--json] [--graph] [--compare OLD.json NEW.json]

No option: table per zone (the batch perimeters of docs/fin.md, section 3) of the ROM-shaped marks,
then the crate sizes and the module graph summary. Comments (`//`) are dropped before counting and
`tools/` is left out, as in the plan. Counts are regular expressions: approximate to a few percent.

Columns:
  files, lines   Rust files and their lines (all lines, as `wc -l`)
  rom            ROM reads through an accessor: data8/16/32, rom8/16/32, rom_i16, prog8/16/32
  fields         object fields at a literal offset: `x.u8(0x5a)`, `o.set_u16(b, 0x1c, v)`...
  a5             work RAM at a literal offset: `A5 + 0x..`, `a5_u8(0x..)`, `ram8(0x..)`...
  raw            raw bus accesses: rd8/rd16/rd32/wr8/wr16/wr32
  sound          sounds played by the address of their 68000 envelope: `sound::play(b, 0xede08, o)`, or a
                 `u32` constant (`sound::play(x.b, SFX_LAND, x.o)`, `x.sound(rom::INTRO_ACTION, p)`)
  romconst       `u32` constants declared in 0x400-0x3fffff (ROM tables and original routine addresses)
  over1500       files of more than 1500 lines
  largest        largest file of the zone

--rev REV       measure a git revision instead of the working tree (`git archive` into a temporary dir)
--json          machine-readable output (zones, crates, graph), to compare two revisions
--graph         list the module graph: edges that close the cycles, strongly connected components
--compare A B   table of the differences between two --json outputs
"""
import json
import re
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Zones of docs/fin.md, section 1 (first match wins; paths relative to ddsom-game/src).
ZONES = [
    ("effects", r"families/effects[^/]*(/.*)?\.rs$"),
    ("set", r"families/(set[^/]*\.rs|set[^/]*/.*\.rs|scenery\.rs)$"),
    ("eshl_sobj_items", r"families/(eshl[^/]*|sobj|items|items_b|boxes|shell)(/.*)?\.rs$"),
    ("enemies", r"families/(enemy[^/]*(/.*)?|reactions(\.rs|/.*))$"),
    ("bosses", r"families/boss[^/]*(/.*)?$"),
    ("players", r"^(player/|families/player/|combat/|combat\.rs$)"),
    ("screens", r"^(modes/|hud/|service/|scene/|credits[^/]*\.rs$|text\.rs$|popups\.rs$)"),
    ("families_common", r"^families/(cx|mod|small|creators)\.rs$"),
    ("engine", r""),
]

ROM_ACCESSOR = r"(?<!fn )\b(?:data|rom|prog)(?:8|16|32)\(|\brom_i16\("
FIELD_METHODS = r"(?:u8|u16|u32|i8|i16|i32|set8|set16|set32|set_u8|set_u16|set_u32|add8|add16|add32|set_bit|bit|linked)"
LITERAL = r"(?:0x[0-9a-fA-F_]+|\d+)\b"
PATTERNS = {
    "rom": ROM_ACCESSOR,
    # the offset is the first argument, or the second after the bus or context (`o.u16(b, 0x1c)`)
    "fields": rf"\.{FIELD_METHODS}\((?:[a-z_][\w.]*, *)?{LITERAL}",
    "a5": r"A5 \+ 0x|A5\.wrapping_add\(0x|\b(?:set_)?(?:a5_?u(?:8|16|32)|ram(?:8|16|32))\(0x",
    "raw": r"\b(?:rd|wr)(?:8|16|32)\(",
}
# Sound call sites: `sound::play*(bus, sound, ...)` and the bosses' `x.sound(sound, ...)`.
SOUND_CALL = re.compile(r"(?<!fn )(?:\bsound::play\w*\([^,()]+|\.sound\(), *([\w:]+)")
COLUMNS = ["files", "lines"] + list(PATTERNS) + ["sound", "romconst", "over1500"]
U32_CONST = re.compile(r"\bconst (\w+): u32 = (0x[0-9a-fA-F_]+|\d+)")


def code_lines(text: str):
    """Lines of code without their `//` comments (strings are rare enough to be ignored)."""
    for line in text.splitlines():
        i = line.find("//")
        yield line if i < 0 else line[:i]


def zone_of(rel: str) -> str:
    for name, pattern in ZONES:
        if re.search(pattern, rel):
            return name
    raise AssertionError(rel)


def measure_file(text: str) -> dict:
    """Counts of one file. A sound argument by name counts if the file declares that name as a `u32`."""
    u32_names = {n for line in code_lines(text) for n, _ in U32_CONST.findall(line)}
    m = defaultdict(int)
    m["files"] = 1
    m["lines"] = len(text.splitlines())
    m["over1500"] = m["lines"] > 1500
    for line in code_lines(text):
        for k, p in PATTERNS.items():
            m[k] += len(re.findall(p, line))
        for arg in SOUND_CALL.findall(line):
            m["sound"] += arg.startswith("0x") or arg.split("::")[-1] in u32_names
        for _, v in U32_CONST.findall(line):
            m["romconst"] += 0x400 <= int(v.replace("_", ""), 0) < 0x400000
    return m


def game_files(root: Path):
    src = root / "port/ddsom-game/src"
    for p in sorted(src.rglob("*.rs")):
        rel = p.relative_to(src).as_posix()
        if not rel.startswith("tools/"):
            yield rel, p.read_text()


def zones(root: Path) -> dict:
    files = list(game_files(root))
    out = {}
    for rel, text in files:
        z = out.setdefault(zone_of(rel), {k: 0 for k in COLUMNS} | {"largest": ["", 0]})
        m = measure_file(text)
        for k in COLUMNS:
            z[k] += m[k]
        if m["lines"] > z["largest"][1]:
            z["largest"] = [rel, m["lines"]]
    return out


def crates(root: Path) -> dict:
    """Lines of src/ and of examples+tests per crate (ddsom-game src without tools/, tools apart)."""
    out = {}
    for toml in sorted((root / "port").glob("*/Cargo.toml")):
        c = toml.parent
        src = sum(len(p.read_text().splitlines()) for p in c.glob("src/**/*.rs"))
        extra = sum(len(p.read_text().splitlines()) for d in ("examples", "tests") for p in c.glob(f"{d}/**/*.rs"))
        out[c.name] = {"src": src, "examples_tests": extra}
    tools = root / "port/ddsom-game/src/tools"
    if tools.exists():
        out["ddsom-game"]["tools"] = sum(len(p.read_text().splitlines()) for p in tools.rglob("*.rs"))
    return out


def module_graph(root: Path):
    """Edges `crate::x` between the top-level modules of ddsom-game (outside tools/)."""
    src = root / "port/ddsom-game/src"
    modules = {p.stem for p in src.glob("*.rs")} | {p.name for p in src.iterdir() if p.is_dir()}
    modules -= {"lib", "tools"}
    edges = defaultdict(set)
    for rel, text in game_files(root):
        top = rel.split("/")[0].removesuffix(".rs")
        if top == "lib":
            continue
        body = "\n".join(code_lines(text))
        for m in re.findall(r"\bcrate::(\w+)", body):
            if m in modules and m != top:
                edges[top].add(m)
        # `use crate::{a, b::c}` groups
        for group in re.findall(r"\bcrate::\{([^}]*)\}", body):
            for m in re.findall(r"(?:^|,)\s*(\w+)", group):
                if m in modules and m != top:
                    edges[top].add(m)
    return sorted(modules), {k: sorted(v) for k, v in edges.items()}


def strongly_connected(nodes, edges):
    """Tarjan's algorithm; components sorted by decreasing size."""
    index, low, on, stack, out = {}, {}, set(), [], []

    def visit(v):
        index[v] = low[v] = len(index)
        stack.append(v)
        on.add(v)
        for w in edges.get(v, ()):
            if w not in index:
                visit(w)
                low[v] = min(low[v], low[w])
            elif w in on:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            comp = []
            while True:
                w = stack.pop()
                on.discard(w)
                comp.append(w)
                if w == v:
                    break
            out.append(sorted(comp))

    sys.setrecursionlimit(10000)
    for v in nodes:
        if v not in index:
            visit(v)
    return sorted(out, key=lambda c: (-len(c), c))


def measure(root: Path) -> dict:
    nodes, edges = module_graph(root)
    scc = strongly_connected(nodes, edges)
    return {
        "zones": zones(root),
        "crates": crates(root),
        "graph": {
            "modules": len(nodes),
            "edges": sum(len(v) for v in edges.values()),
            "largest_scc": len(scc[0]) if scc else 0,
            "components": [c for c in scc if len(c) > 1],
            "outside": [c[0] for c in scc if len(c) == 1],
            "adjacency": edges,
        },
    }


def checkout(rev: str) -> Path:
    tmp = Path(tempfile.mkdtemp(prefix="measure_end_"))
    archive = subprocess.run(["git", "-C", str(ROOT), "archive", rev, "port"], check=True, capture_output=True).stdout
    subprocess.run(["tar", "-x", "-C", str(tmp)], input=archive, check=True)
    return tmp


def print_zones(z: dict):
    head = ["zone"] + COLUMNS + ["largest"]
    print("| " + " | ".join(head) + " |")
    print("|---|" + "---:|" * len(COLUMNS) + "---|")
    total = defaultdict(int)
    for name, _ in ZONES:
        if name not in z:
            continue
        v = z[name]
        for k in COLUMNS:
            total[k] += v[k]
        print(f"| {name} | " + " | ".join(str(v[k]) for k in COLUMNS) + f" | {v['largest'][0]} ({v['largest'][1]}) |")
    print("| **total** | " + " | ".join(f"**{total[k]}**" for k in COLUMNS) + " | |")


def print_crates(c: dict):
    print("\n| crate | src | examples, tests |")
    print("|---|---:|---:|")
    for name, v in sorted(c.items(), key=lambda kv: -kv[1]["src"]):
        tools = f" (tools/ {v['tools']})" if "tools" in v else ""
        print(f"| {name} | {v['src']}{tools} | {v['examples_tests']} |")


def print_graph(g: dict, full: bool):
    print(f"\nModules: {g['modules']}, edges: {g['edges']}, largest strongly connected component: {g['largest_scc']}")
    print(f"Outside any cycle: {', '.join(g['outside'])}")
    if full:
        for c in g["components"]:
            print(f"Component ({len(c)}): {', '.join(c)}")
            members = set(c)
            for a in c:
                inner = [b for b in g["adjacency"].get(a, []) if b in members]
                print(f"  {a} -> {', '.join(inner)}")


def compare(a: dict, b: dict):
    print("| zone | " + " | ".join(COLUMNS) + " |")
    print("|---|" + "---:|" * len(COLUMNS))
    for name, _ in ZONES:
        va, vb = a["zones"].get(name), b["zones"].get(name)
        if not va or not vb:
            continue
        cells = [f"{va[k]} → {vb[k]}" if va[k] != vb[k] else str(va[k]) for k in COLUMNS]
        print(f"| {name} | " + " | ".join(cells) + " |")
    print(f"\nLargest strongly connected component: {a['graph']['largest_scc']} → {b['graph']['largest_scc']}")


def main(args):
    if "--compare" in args:
        i = args.index("--compare")
        compare(*(json.loads(Path(f).read_text()) for f in args[i + 1 : i + 3]))
        return
    root = checkout(args[args.index("--rev") + 1]) if "--rev" in args else ROOT
    m = measure(root)
    if "--json" in args:
        print(json.dumps(m, indent=1))
        return
    print_zones(m["zones"])
    print_crates(m["crates"])
    print_graph(m["graph"], "--graph" in args)


if __name__ == "__main__":
    main(sys.argv[1:])
