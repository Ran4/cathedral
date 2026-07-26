# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy", "pillow", "scipy"]
# ///
"""Bake the Ombreval navigation artifact from the authoritative cadastral plan.

Produces two checked-in files that the sim loads as `&str` / `&[u8]` (exactly
like `areas.json`), never touching the filesystem itself:

    assets/world/navigation.json   the street graph, places, sites and doors
    assets/world/navigation.bin    the walkable bitset (main component only)

The bake is deterministic: no RNG, sorted iteration, quarter-metre grid. Running
it twice on the same plan yields byte-identical output (`bake_is_reproducible`).

Design: features/implemented/movement/02_navigation.md. The load-bearing rules:

  * "roads win" is *not* a blind carve. A road cell under a ground-level solid
    stays blocked (you cannot walk through a house); the passages stay open
    because their covering structures — the three bridges and the malt-house —
    are *overhead* (their collider starts at y>=3.8), so their footprints are
    never subtracted. This keeps the walkable surface in agreement with
    `CollisionWorld` at walk height, which `no_walkable_cell_is_solid` proves.
  * A road centreline is a schematic hint, not gospel: several cut straight
    through a solid building (St Maren's, storage bays). Every graph edge is
    therefore validated against the walkable bitset and, where the straight line
    is blocked, re-routed around the obstacle with a windowed grid A*. So every
    edge lies on walkable ground (`graph_matches_bitset`) and the roads remain
    traversable end to end (`roads_are_walkable_end_to_end`).
  * The door on a building is the render's `stable_hash`-chosen edge, but chosen
    only among edges you can actually stand at — midpoint + 0.8 m outward on
    walkable ground — tie-broken by the same hash. The chosen edge index is
    baked so `add_facade_openings` renders the *same* door the sim walks to.
"""

import hashlib
import heapq
import json
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage
from scipy.spatial import cKDTree

ROOT = Path(__file__).resolve().parent.parent
PLAN_PATH = ROOT / "lore/places/ombreval_buildings.json"
COLLISION_PATH = ROOT / "assets/world/collision_footprints.json"
OUT_JSON = ROOT / "assets/world/navigation.json"
OUT_BIN = ROOT / "assets/world/navigation.bin"

CELL = 0.25            # metres per grid cell
AGENT_RADIUS = 0.35    # matches PLAYER_HALF_SIZE.x in controller.rs
WELD_M = 0.75          # coincident graph vertices within this distance are one
DOOR_STAND_M = 0.8     # how far outside the threshold a person waits at a door
DOOR_MIN_EDGE_M = 3.2  # add_facade_openings skips edges shorter than this
FORECOURT_XZ = (0.0, 95.0)  # PLAYER_SPAWN — a guaranteed-walkable reference

# Buildings add_facade_openings (city/mod.rs) renders NO door for, so the bake
# must not emit one either — the sim's door has to be the door the player sees.
#   * named_lanthorn: its cathedral interior is built by scene.rs (CathedralPlugin),
#     not build_city, so its colliders are absent from the collision export and its
#     front doors are baked there too, not by add_facade_openings. Its footprint is
#     subtracted explicitly (build_walkable) so the nave is not treated as open
#     ground; routing inside the cathedral is a later interior carve-in, not M1.
#   * named_malt_house and the three `use == "bridge"` buildings: overhead scenery.
#     "roads win" leaves the ground *under* them walkable (their collider starts
#     above head height), so choose_door_edge would otherwise bake a door standing
#     on open ground beneath a structure with no threshold the player can see. The
#     malt-house is matched by id (its `use` is "trade", not "bridge"); the bridges
#     by `use`, mirroring add_facade_openings' `use_name == "bridge"` check exactly.
SKIP_DOOR_IDS = {"named_lanthorn", "named_malt_house"}
# Places whose anchor sits deep inside a large footprint (the Lanthorn nave is
# ~44 m from its nearest wall) still resolve to the nearest walkable apron.
PLACE_SNAP_M = 60.0


def log(msg):
    print(msg, file=sys.stderr)


# --------------------------------------------------------------------------- #
# Geometry helpers
# --------------------------------------------------------------------------- #
def signed_area(poly):
    """Matches plan::signed_area — same winding sign the render's door uses."""
    return 0.5 * sum(a[0] * b[1] - b[0] * a[1]
                     for a, b in zip(poly, poly[1:] + poly[:1]))


def stable_hash(text):
    """FNV-1a, byte-identical to stable_hash in src/city/mod.rs:1949."""
    h = 2166136261
    for byte in text.encode():
        h = ((h ^ byte) * 16777619) & 0xFFFFFFFF
    return h


# --------------------------------------------------------------------------- #
# The walkable surface
# --------------------------------------------------------------------------- #
class Grid:
    def __init__(self, plan):
        xs = [p[0] for p in plan["wall_polygon_xz"]]
        zs = [p[1] for p in plan["wall_polygon_xz"]]
        margin = 8.0
        self.x0 = min(xs) - margin
        self.z0 = min(zs) - margin
        self.w = int((max(xs) + margin - self.x0) / CELL) + 1
        self.h = int((max(zs) + margin - self.z0) / CELL) + 1

    def to_px(self, p):
        return ((p[0] - self.x0) / CELL, (p[1] - self.z0) / CELL)

    def cell(self, x, z):
        """World -> (row, col). Row indexes z, col indexes x."""
        return (int((z - self.z0) / CELL), int((x - self.x0) / CELL))

    def world(self, row, col):
        """(row, col) -> the world XZ of the cell centre."""
        return (round(self.x0 + (col + 0.5) * CELL, 4),
                round(self.z0 + (row + 0.5) * CELL, 4))

    def in_bounds(self, row, col):
        return 0 <= row < self.h and 0 <= col < self.w


def build_walkable(plan, grid):
    """inside_wall minus every collider that stops the player at walking height,
    eroded by the agent radius, reduced to the single main component.

    The obstacle set is the game's own `CollisionWorld`, exported footprint by
    footprint (`assets/world/collision_footprints.json`). Taking the real
    colliders rather than re-deriving them from the plan is what keeps the
    walkable surface an exact complement of what stops the player, wall thickness,
    towers, gatehouses, bridge piers and all — and it makes "roads win" automatic:
    an overhead structure's collider starts above head height, so it is absent
    from the export and the covered way beneath it stays open."""
    if not COLLISION_PATH.exists():
        raise SystemExit(
            f"{COLLISION_PATH.relative_to(ROOT)} is missing; regenerate it with\n"
            "  cargo test export_collision_footprints -- --ignored --nocapture"
        )
    footprints = json.loads(COLLISION_PATH.read_text())["footprints"]

    img = Image.new("L", (grid.w, grid.h), 0)
    draw = ImageDraw.Draw(img)
    draw.polygon([grid.to_px(p) for p in plan["wall_polygon_xz"]], fill=255)
    for poly in footprints:
        if len(poly) >= 3:
            draw.polygon([grid.to_px(p) for p in poly], fill=0)
    # The cathedral is built by CathedralPlugin, not build_city, so its interior
    # colliders are not in the export; subtract its footprint here.
    lanthorn = next(b for b in plan["buildings"] if b["id"] == "named_lanthorn")
    draw.polygon([grid.to_px(p) for p in lanthorn["polygon"]], fill=0)

    free = np.array(img) > 0
    log(f"free (inside wall minus {len(footprints)} collider footprints): "
        f"{free.sum() * CELL * CELL / 1e4:.1f} ha")

    # Distance-transform erosion keeps a cell iff the nearest obstacle is at
    # least the agent radius away — gentler and more accurate than a disc.
    dist = ndimage.distance_transform_edt(free) * CELL
    walk = dist >= AGENT_RADIUS

    labels, n = ndimage.label(walk)
    sizes = ndimage.sum(walk, labels, range(1, n + 1))
    main_label = int(np.argmax(sizes)) + 1
    main = labels == main_label
    log(f"walkable {walk.sum() * CELL * CELL / 1e4:.1f} ha; "
        f"main component {main.sum() * CELL * CELL / 1e4:.1f} ha "
        f"({100 * main.sum() / walk.sum():.1f}%), {n} components")
    return main


# --------------------------------------------------------------------------- #
# Bitset queries and windowed grid A*
# --------------------------------------------------------------------------- #
class Surface:
    def __init__(self, grid, main):
        self.grid = grid
        self.main = main  # (h, w) bool

    def walkable_cell(self, row, col):
        return self.grid.in_bounds(row, col) and bool(self.main[row, col])

    def walkable_point(self, x, z):
        row, col = self.grid.cell(x, z)
        return self.walkable_cell(row, col)

    def snap(self, x, z, max_r_m=30.0):
        """Nearest walkable cell centre to a world point, ring search outward."""
        row, col = self.grid.cell(x, z)
        if self.walkable_cell(row, col):
            return (row, col)
        best = None
        best_d2 = None
        for rad in range(1, int(max_r_m / CELL) + 1):
            lo_r, hi_r = row - rad, row + rad
            lo_c, hi_c = col - rad, col + rad
            for rr in range(lo_r, hi_r + 1):
                for cc in range(lo_c, hi_c + 1):
                    if abs(rr - row) != rad and abs(cc - col) != rad:
                        continue  # ring perimeter only
                    if self.walkable_cell(rr, cc):
                        d2 = (rr - row) ** 2 + (cc - col) ** 2
                        if best_d2 is None or d2 < best_d2:
                            best_d2, best = d2, (rr, cc)
            if best is not None:
                return best
        return None

    def segment_walkable(self, a_cell, b_cell):
        """Every cell the straight line between the two cell centres passes
        through is walkable, and the line never squeezes diagonally past a
        blocked corner. Sampled at a quarter-cell in *world* space with the same
        floor(point) convention the runtime uses. Rejecting the corner graze is
        what keeps every edge's midpoint on walkable ground under one rule
        (`graph_matches_bitset`) — a lattice-corner midpoint means the line
        clipped a blocked cell's corner, and an agent with radius could not."""
        ax, az = self.grid.world(*a_cell)
        bx, bz = self.grid.world(*b_cell)
        steps = int(math.ceil(math.hypot(bx - ax, bz - az) / (CELL / 4)))
        if steps == 0:
            return self.walkable_point(ax, az)
        prev = None
        for i in range(steps + 1):
            t = i / steps
            row, col = self.grid.cell(ax + (bx - ax) * t, az + (bz - az) * t)
            if not self.walkable_cell(row, col):
                return False
            if prev is not None:
                pr, pc = prev
                if row != pr and col != pc:  # diagonal step: check both shoulders
                    if not (self.walkable_cell(pr, col) and self.walkable_cell(row, pc)):
                        return False
            prev = (row, col)
        return True

    def astar(self, a_cell, b_cell, margin_cells=64):
        """Grid A* confined to the segment's bbox + margin. Returns a cell path
        or None. The bitset is one component, so widening the window always
        eventually succeeds; the margin just keeps the common case cheap."""
        (r0, c0), (r1, c1) = a_cell, b_cell
        for margin in (margin_cells, margin_cells * 4, max(self.grid.h, self.grid.w)):
            lo_r = max(0, min(r0, r1) - margin)
            hi_r = min(self.grid.h - 1, max(r0, r1) + margin)
            lo_c = max(0, min(c0, c1) - margin)
            hi_c = min(self.grid.w - 1, max(c0, c1) + margin)
            path = self._astar_window(a_cell, b_cell, lo_r, hi_r, lo_c, hi_c)
            if path is not None:
                return path
        return None

    def _astar_window(self, a_cell, b_cell, lo_r, hi_r, lo_c, hi_c):
        main = self.main
        SQRT2 = math.sqrt(2)

        def h(r, c):
            return math.hypot(r - b_cell[0], c - b_cell[1])

        open_heap = [(h(*a_cell), 0.0, a_cell)]
        came = {}
        g = {a_cell: 0.0}
        neigh = ((-1, 0, 1.0), (1, 0, 1.0), (0, -1, 1.0), (0, 1, 1.0),
                 (-1, -1, SQRT2), (-1, 1, SQRT2), (1, -1, SQRT2), (1, 1, SQRT2))
        while open_heap:
            _, gc, cur = heapq.heappop(open_heap)
            if cur == b_cell:
                path = [cur]
                while cur in came:
                    cur = came[cur]
                    path.append(cur)
                return path[::-1]
            if gc > g.get(cur, float("inf")):
                continue
            cr, cc = cur
            for dr, dc, cost in neigh:
                nr, nc = cr + dr, cc + dc
                if not (lo_r <= nr <= hi_r and lo_c <= nc <= hi_c):
                    continue
                if not main[nr, nc]:
                    continue
                if dr != 0 and dc != 0 and not (main[cr, nc] and main[nr, cc]):
                    continue  # no corner-cutting through a diagonal pinch
                ng = gc + cost
                if ng < g.get((nr, nc), float("inf")):
                    g[(nr, nc)] = ng
                    came[(nr, nc)] = cur
                    heapq.heappush(open_heap, (ng + h(nr, nc), ng, (nr, nc)))
        return None

    def string_pull(self, cells):
        """Reduce an A* cell path to the fewest waypoints whose straight links
        are all walkable (greedy line-of-sight)."""
        if len(cells) <= 2:
            return list(cells)
        out = [cells[0]]
        anchor = 0
        i = 1
        while i < len(cells):
            if not self.segment_walkable(cells[anchor], cells[i]):
                out.append(cells[i - 1])
                anchor = i - 1
            i += 1
        out.append(cells[-1])
        # drop consecutive duplicates
        pruned = [out[0]]
        for c in out[1:]:
            if c != pruned[-1]:
                pruned.append(c)
        return pruned


# --------------------------------------------------------------------------- #
# The street graph
# --------------------------------------------------------------------------- #
class Graph:
    def __init__(self, surface):
        self.surface = surface
        self.nodes = []                 # list[(x, z)]
        self.node_at = {}               # (row, col) -> node index
        self.adj = {}                   # node -> dict[node] = half_width
        self._kdt = None

    def add_node_cell(self, cell):
        if cell in self.node_at:
            return self.node_at[cell]
        idx = len(self.nodes)
        self.nodes.append(self.surface.grid.world(*cell))
        self.node_at[cell] = idx
        self.adj[idx] = {}
        return idx

    def add_node_point(self, x, z):
        cell = self.surface.snap(x, z)
        if cell is None:
            return None
        return self.add_node_cell(cell)

    def link(self, a, b, half_width):
        if a == b:
            return
        prev = self.adj[a].get(b)
        hw = half_width if prev is None else max(prev, half_width)
        self.adj[a][b] = hw
        self.adj[b][a] = hw

    def connect_cells(self, a_cell, b_cell, half_width):
        """Add an edge between two cells whose straight link may be blocked; the
        grid A* + string-pull inserts intermediate nodes so every emitted edge
        lies on walkable ground."""
        a = self.add_node_cell(a_cell)
        if self.surface.segment_walkable(a_cell, b_cell):
            b = self.add_node_cell(b_cell)
            self.link(a, b, half_width)
            return b
        path = self.surface.astar(a_cell, b_cell)
        if path is None:
            return None
        waypoints = self.surface.string_pull(path)
        prev = a
        prev_cell = a_cell
        for cell in waypoints[1:]:
            nxt = self.add_node_cell(cell)
            # width narrows to a walkable default on a detour we invented
            self.link(prev, nxt, min(half_width, 0.6))
            prev, prev_cell = nxt, cell
        return prev

    def components(self):
        seen = {}
        comp = 0
        for start in range(len(self.nodes)):
            if start in seen:
                continue
            stack = [start]
            seen[start] = comp
            while stack:
                u = stack.pop()
                for v in self.adj[u]:
                    if v not in seen:
                        seen[v] = comp
                        stack.append(v)
            comp += 1
        return seen, comp

    def kdtree(self):
        if self._kdt is None or self._kdt.n != len(self.nodes):
            self._kdt = cKDTree(np.array(self.nodes))
        return self._kdt

    def nearest_reachable_node(self, x, z, k=12):
        """Nearest existing node whose straight link from (x,z) is walkable."""
        cell = self.surface.snap(x, z)
        if cell is None:
            return None
        tree = self.kdtree()
        k = min(k, len(self.nodes))
        _, idxs = tree.query([x, z], k=k)
        for idx in np.atleast_1d(idxs):
            n_cell = self.surface.grid.cell(*self.nodes[int(idx)])
            if self.surface.segment_walkable(cell, n_cell):
                return int(idx)
        return None

    def attach_leaf(self, x, z, half_width=0.6, max_r_m=30.0):
        """Add a leaf node at (x,z) joined to the graph, routing around any
        obstacle between the leaf and its street. Returns the leaf node index."""
        cell = self.surface.snap(x, z, max_r_m)
        if cell is None:
            return None
        leaf = self.add_node_cell(cell)
        target = self.nearest_reachable_node(*self.nodes[leaf])
        if target is not None and target != leaf:
            self.link(leaf, target, half_width)
            return leaf
        # No clear straight driveway: A* to the nearest node by distance.
        tree = self.kdtree()
        _, idxs = tree.query(self.nodes[leaf], k=min(24, len(self.nodes)))
        for idx in np.atleast_1d(idxs):
            idx = int(idx)
            if idx == leaf:
                continue
            end = self.connect_cells(cell, self.surface.grid.cell(*self.nodes[idx]),
                                     half_width)
            if end is not None:
                return leaf
        return leaf


def segment_intersections(a0, a1, b0, b1):
    """Interior crossing point of segments a0-a1 and b0-b1, or None."""
    r = (a1[0] - a0[0], a1[1] - a0[1])
    s = (b1[0] - b0[0], b1[1] - b0[1])
    denom = r[0] * s[1] - r[1] * s[0]
    if abs(denom) < 1e-9:
        return None
    qp = (b0[0] - a0[0], b0[1] - a0[1])
    t = (qp[0] * s[1] - qp[1] * s[0]) / denom
    u = (qp[0] * r[1] - qp[1] * r[0]) / denom
    eps = 1e-6
    if eps < t < 1 - eps and eps < u < 1 - eps:
        return (a0[0] + t * r[0], a0[1] + t * r[1])
    return None


def build_graph(plan, surface):
    """Split the 49 road polylines at their geometric crossings, weld coincident
    vertices, snap everything to walkable ground, and emit a validated graph."""
    # 1. Flatten roads into segments carrying their corridor half-width.
    segments = []  # (p, q, half_width)
    for r in plan["roads"]:
        pts = r["points"]
        hw = r["width_m"] / 2.0
        for p, q in zip(pts, pts[1:]):
            segments.append((tuple(p), tuple(q), hw))

    # 2. Cut every segment at crossings with every other segment.
    split_pts = [[] for _ in segments]
    for i in range(len(segments)):
        ai, bi, _ = segments[i]
        for j in range(i + 1, len(segments)):
            aj, bj, _ = segments[j]
            x = segment_intersections(ai, bi, aj, bj)
            if x is not None:
                split_pts[i].append(x)
                split_pts[j].append(x)

    graph = Graph(surface)
    # 3. Emit each sub-segment as a validated (possibly re-routed) edge.
    for i, (a, b, hw) in enumerate(segments):
        chain = [a] + sorted(split_pts[i],
                             key=lambda p: (p[0] - a[0]) ** 2 + (p[1] - a[1]) ** 2) + [b]
        prev_cell = None
        for point in chain:
            cell = surface.snap(*point)
            if cell is None:
                prev_cell = None
                continue
            if prev_cell is not None and cell != prev_cell:
                graph.connect_cells(prev_cell, cell, hw)
            else:
                graph.add_node_cell(cell)
            prev_cell = cell

    # 4. Weld nodes closer than WELD_M by connecting them (union without
    #    collapsing — keeps the graph planar and reproducible).
    if graph.nodes:
        tree = cKDTree(np.array(graph.nodes))
        for a, b in sorted(tree.query_pairs(WELD_M)):
            if surface.segment_walkable(surface.grid.cell(*graph.nodes[a]),
                                        surface.grid.cell(*graph.nodes[b])):
                graph.link(a, b, 0.6)

    # 5. Repair connectivity: bridge any stray component into the main one along
    #    walkable ground. The bitset is a single region, so this always closes.
    _bridge_components(graph, surface)
    return graph


def _bridge_components(graph, surface):
    seen, ncomp = graph.components()
    if ncomp <= 1:
        log(f"street graph: {len(graph.nodes)} nodes, already connected")
        return
    # main = the component with the most nodes
    counts = {}
    for c in seen.values():
        counts[c] = counts.get(c, 0) + 1
    main_comp = max(counts, key=counts.get)
    # Snapshot main-component nodes into a KD-tree; connect_cells will grow the
    # node list, so build the tree once over the originals.
    main_nodes = sorted(n for n, c in seen.items() if c == main_comp)
    main_tree = cKDTree(np.array([graph.nodes[n] for n in main_nodes]))
    members_by_comp = {}
    for n, c in seen.items():
        if c != main_comp:
            members_by_comp.setdefault(c, []).append(n)
    for comp in sorted(members_by_comp):
        members = sorted(members_by_comp[comp])
        best = None
        for m in members:
            d, j = main_tree.query(graph.nodes[m])
            if best is None or d < best[0]:
                best = (d, m, main_nodes[int(j)])
        if best is not None:
            _, m, idx = best
            graph.connect_cells(surface.grid.cell(*graph.nodes[m]),
                                surface.grid.cell(*graph.nodes[idx]), 0.6)
    seen, ncomp = graph.components()
    log(f"street graph: {len(graph.nodes)} nodes after bridging, "
        f"{ncomp} component(s)")


# --------------------------------------------------------------------------- #
# Doors — the render's edge, chosen for reachability
# --------------------------------------------------------------------------- #
def choose_door_edge(building, surface):
    """Return (edge_index, stand_point) for the building's door, or None.

    Reproduces add_facade_openings' geometry: only edges >= DOOR_MIN_EDGE_M can
    carry a door, the outward normal follows the polygon winding, and among the
    edges you can actually stand at the choice is stable_hash % len(candidates).
    """
    poly = building["polygon"]
    n = len(poly)
    orientation = 1.0 if signed_area(poly) >= 0 else -1.0
    candidates = []
    for i in range(n):
        a = poly[i]
        b = poly[(i + 1) % n]
        ex, ez = b[0] - a[0], b[1] - a[1]
        length = math.hypot(ex, ez)
        if length < DOOR_MIN_EDGE_M:
            continue
        nx, nz = ez / length, -ex / length
        if orientation < 0:
            nx, nz = -nx, -nz
        mx, mz = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2
        stand = (mx + DOOR_STAND_M * nx, mz + DOOR_STAND_M * nz)
        if surface.walkable_point(*stand):
            candidates.append((i, stand))
    if not candidates:
        return None
    pick = stable_hash(building["id"]) % len(candidates)
    return candidates[pick]


# --------------------------------------------------------------------------- #
# Bake
# --------------------------------------------------------------------------- #
def bake():
    plan = json.loads(PLAN_PATH.read_text())
    grid = Grid(plan)
    log(f"grid {grid.w} x {grid.h} = {grid.w * grid.h / 1e6:.1f}M cells @ {CELL} m")

    main = build_walkable(plan, grid)
    surface = Surface(grid, main)

    graph = build_graph(plan, surface)

    # Places — the 69 named_place_index anchors, snapped to walkable ground.
    places = []
    unreachable = []
    for p in sorted(plan["named_place_index"], key=lambda p: p["number"]):
        node = graph.attach_leaf(*p["anchor"], max_r_m=PLACE_SNAP_M)
        if node is None:
            unreachable.append(p["name"])
            continue
        places.append({"name": p["name"], "node": node, "kind": p["kind"]})
    if unreachable:
        log(f"WARNING: {len(unreachable)} places did not snap: {unreachable}")

    # Sites — polygons; attach their centroid (free movement inside is M2+).
    sites = []
    for s in sorted(plan["sites"], key=lambda s: s["id"]):
        cx = sum(p[0] for p in s["polygon"]) / len(s["polygon"])
        cz = sum(p[1] for p in s["polygon"]) / len(s["polygon"])
        node = graph.attach_leaf(cx, cz, max_r_m=PLACE_SNAP_M)
        if node is not None:
            sites.append({"id": s["id"], "name": s["name"], "node": node})

    # Doors — one per building that has a reachable, render-eligible edge.
    doors = []
    door_far = 0
    for b in sorted(plan["buildings"], key=lambda b: b["id"]):
        if b["id"] in SKIP_DOOR_IDS or b.get("use") == "bridge":
            continue  # renderer draws no door here (Lanthorn, malt-house, bridges)
        chosen = choose_door_edge(b, surface)
        if chosen is None:
            door_far += 1
            continue
        edge_index, stand = chosen
        node = graph.attach_leaf(*stand)
        if node is None:
            door_far += 1
            continue
        doors.append({"building": b["id"], "edge": edge_index, "node": node})
    log(f"doors: {len(doors)} reachable, {door_far} with no stand-able edge")

    forecourt = graph.attach_leaf(*FORECOURT_XZ)

    # self-check: every emitted edge must lie on walkable ground
    if "--check" in sys.argv:
        seg_bad = 0
        mid_bad = 0
        for a in graph.adj:
            for b in graph.adj[a]:
                if a >= b:
                    continue
                ac = grid.cell(*graph.nodes[a])
                bc = grid.cell(*graph.nodes[b])
                if not surface.segment_walkable(ac, bc):
                    seg_bad += 1
                mx = (graph.nodes[a][0] + graph.nodes[b][0]) / 2
                mz = (graph.nodes[a][1] + graph.nodes[b][1]) / 2
                if not surface.walkable_point(mx, mz):
                    mid_bad += 1
        log(f"SELF-CHECK: {seg_bad} edges fail segment_walkable, "
            f"{mid_bad} fail midpoint")

    # Emit the bitset (main component, row-major, MSB-first within each byte).
    packed = np.packbits(main.reshape(-1))
    raw = packed.tobytes()
    OUT_BIN.write_bytes(raw)
    digest = hashlib.sha256(raw).hexdigest()
    # A non-cryptographic FNV-1a the sim suite can recompute with no dependency,
    # to prove the committed bitset still matches its manifest.
    fnv = 14695981039346656037
    for byte in raw:
        fnv = ((fnv ^ byte) * 1099511628211) & 0xFFFFFFFFFFFFFFFF

    edges = []
    for a in sorted(graph.adj):
        for b, hw in sorted(graph.adj[a].items()):
            if a < b:
                edges.append([a, b, round(hw, 3)])

    doc = {
        "schema_version": 1,
        "generated_by": "scripts/bake_navigation.py",
        "source_seed": plan["seed"],
        "grid": {
            "x0": round(grid.x0, 4),
            "z0": round(grid.z0, 4),
            "cell_m": CELL,
            "w": grid.w,
            "h": grid.h,
            "agent_radius_m": AGENT_RADIUS,
            "bitset_file": "navigation.bin",
            "bitset_bits": grid.w * grid.h,
            "bitset_sha256": digest,
            "bitset_fnv1a": fnv,
        },
        "nodes": [[round(x, 4), round(z, 4)] for x, z in graph.nodes],
        "edges": edges,
        "places": places,
        "sites": sites,
        "doors": doors,
        "reference": {"forecourt": forecourt},
    }
    OUT_JSON.write_text(json.dumps(doc, separators=(",", ":"), sort_keys=False) + "\n")
    log(f"wrote {OUT_JSON.relative_to(ROOT)} "
        f"({len(graph.nodes)} nodes, {len(edges)} edges, {len(places)} places, "
        f"{len(sites)} sites, {len(doors)} doors)")
    log(f"wrote {OUT_BIN.relative_to(ROOT)} ({len(packed)} bytes, sha {digest[:12]})")


if __name__ == "__main__":
    bake()
