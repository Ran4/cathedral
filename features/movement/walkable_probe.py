# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy", "pillow", "scipy"]
# ///
"""Probe: can a walkable surface be baked from the Ombreval cadastral plan?

Rasterises free space at CELL m, erodes by the agent radius, flood-fills, and
reports what fraction of the city (and of the authored spawn points / named
places) actually ends up reachable.  Writes a PNG so the result can be looked at.
"""

import json, sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

ROOT = Path("/home/ran/src/rust/cathedralbevy")
PLAN = json.loads((ROOT / "lore/places/ombreval_buildings.json").read_text())

CELL = 0.25          # metres per cell
AGENT_RADIUS = 0.35  # same as PLAYER_HALF_SIZE.x

xs = [p[0] for p in PLAN["wall_polygon_xz"]]
zs = [p[1] for p in PLAN["wall_polygon_xz"]]
MARGIN = 8.0
x0, x1 = min(xs) - MARGIN, max(xs) + MARGIN
z0, z1 = min(zs) - MARGIN, max(zs) + MARGIN
W = int((x1 - x0) / CELL) + 1
H = int((z1 - z0) / CELL) + 1
print(f"world  x:[{x0:.0f},{x1:.0f}] z:[{z0:.0f},{z1:.0f}]  "
      f"= {x1-x0:.0f} x {z1-z0:.0f} m")
print(f"grid   {W} x {H} = {W*H/1e6:.1f}M cells @ {CELL} m  "
      f"({W*H/8/1e6:.2f} MB as a bitset)")


def to_px(pt):
    return ((pt[0] - x0) / CELL, (pt[1] - z0) / CELL)


def obb_corners(f):
    """Fixture OBB -> 4 corners, matching plan.rs's test rotation convention."""
    import math
    a = math.radians(f["angle_deg"])
    hx, hz = f["size"][0] / 2, f["size"][1] / 2
    cx, cz = f["position"]
    out = []
    for sx, sz in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
        lx, lz = sx * hx, sz * hz
        out.append((cx + lx * math.cos(a) - lz * math.sin(a),
                    cz + lx * math.sin(a) + lz * math.cos(a)))
    return out


# ---- pass 1: inside the wall -------------------------------------------------
img = Image.new("L", (W, H), 0)
d = ImageDraw.Draw(img)
d.polygon([to_px(p) for p in PLAN["wall_polygon_xz"]], fill=255)
inside_wall = np.array(img) > 0
print(f"\ninside wall            {inside_wall.sum()*CELL*CELL/1e4:8.1f} ha")

# ---- pass 2: subtract buildings + fixtures -----------------------------------
for b in PLAN["buildings"]:
    d.polygon([to_px(p) for p in b["polygon"]], fill=0)
n_fix_solid = 0
for f in PLAN["fixtures"]:
    d.polygon([to_px(p) for p in obb_corners(f)], fill=0)
    n_fix_solid += 1
free_naive = np.array(img) > 0
print(f"minus {len(PLAN['buildings'])} buildings "
      f"+ {n_fix_solid} fixtures  {free_naive.sum()*CELL*CELL/1e4:8.1f} ha")

# ---- pass 3: roads win (a building over a passage must not block it) ---------
roads_img = Image.new("L", (W, H), 0)
rd = ImageDraw.Draw(roads_img)
for r in PLAN["roads"]:
    pts = [to_px(p) for p in r["points"]]
    w_px = max(1, int(r["width_m"] / CELL))
    rd.line(pts, fill=255, width=w_px, joint="curve")
    # round the joints so corners are not clipped
    for p in pts:
        rr = w_px / 2
        rd.ellipse([p[0]-rr, p[1]-rr, p[0]+rr, p[1]+rr], fill=255)
roads = (np.array(roads_img) > 0) & inside_wall

blocked_road = (roads & ~free_naive).sum()
print(f"road cells blocked by a building/fixture: {blocked_road} "
      f"({blocked_road*CELL*CELL:.0f} m2)  <- 'roads win' matters" )

free = free_naive | roads
print(f"free (roads win)      {free.sum()*CELL*CELL/1e4:8.1f} ha  "
      f"({100*free.sum()/inside_wall.sum():.1f}% of intramural)")

# ---- pass 4: erode by agent radius ------------------------------------------
r_cells = int(np.ceil(AGENT_RADIUS / CELL))
yy, xx = np.mgrid[-r_cells:r_cells+1, -r_cells:r_cells+1]
disc = (xx**2 + yy**2) <= r_cells**2
walk = ndimage.binary_erosion(free, structure=disc)
print(f"eroded by r={AGENT_RADIUS} m   {walk.sum()*CELL*CELL/1e4:8.1f} ha")

# ---- pass 5: connectivity ----------------------------------------------------
lab, n = ndimage.label(walk)
sizes = ndimage.sum(walk, lab, range(1, n + 1))
big = int(np.argmax(sizes)) + 1
main = lab == big
print(f"\ncomponents             {n}")
print(f"largest component      {main.sum()*CELL*CELL/1e4:8.1f} ha "
      f"({100*main.sum()/walk.sum():.1f}% of walkable)")
top = sorted(sizes, reverse=True)[:6]
print("top component sizes    " + ", ".join(f"{s*CELL*CELL:.0f} m2" for s in top))


def cell_of(x, z):
    return int((z - z0) / CELL), int((x - x0) / CELL)


def nearest_walkable(x, z, max_r=12.0):
    """Snap to the nearest cell in the main component (what a real bake does)."""
    r, c = cell_of(x, z)
    for rad in range(0, int(max_r / CELL)):
        lo_r, hi_r = max(0, r - rad), min(H, r + rad + 1)
        lo_c, hi_c = max(0, c - rad), min(W, c + rad + 1)
        win = main[lo_r:hi_r, lo_c:hi_c]
        if win.any():
            return rad * CELL
    return None


# ---- pass 6: are the authored points reachable? ------------------------------
print()
bad = []
for p in PLAN["named_place_index"]:
    dist = nearest_walkable(*p["anchor"])
    if dist is None or dist > 6.0:
        bad.append((p["name"], dist))
print(f"named places (69): {69-len(bad)} within 6 m of the main component")
for name, dist in bad:
    print(f"    UNREACHABLE  {name}  (nearest {dist} m)" if dist is None
          else f"    far         {name}  ({dist:.1f} m)")

# spawn points of the 470 distributed NPCs
seed_dirs = sorted((ROOT / "lore/characters").rglob("*.json"))
spawns, far = 0, []
for f in seed_dirs:
    try:
        c = json.loads(f.read_text())
    except Exception:
        continue
    pos = c.get("position_m") or c.get("spawn", {}).get("position_m")
    if not pos:
        continue
    x, z = (pos["x"], pos["z"]) if isinstance(pos, dict) else (pos[0], pos[2])
    spawns += 1
    dist = nearest_walkable(x, z)
    if dist is None or dist > 3.0:
        far.append((f.stem, dist))
print(f"\nauthored spawns found: {spawns}; "
      f"{spawns-len(far)} within 3 m of the main component")
for n_, dist in far[:12]:
    print(f"    far  {n_}  ({dist})")

# building doors: midpoint of each polygon edge, pushed 1 m outward
print()
reach = 0
tot = 0
for b in PLAN["buildings"]:
    poly = b["polygon"]
    ok = False
    cx = sum(p[0] for p in poly) / len(poly)
    cz = sum(p[1] for p in poly) / len(poly)
    for i in range(len(poly)):
        a, bb = poly[i], poly[(i + 1) % len(poly)]
        mx, mz = (a[0] + bb[0]) / 2, (a[1] + bb[1]) / 2
        ox, oz = mx - cx, mz - cz
        L = (ox * ox + oz * oz) ** 0.5 or 1.0
        px, pz = mx + 1.2 * ox / L, mz + 1.2 * oz / L
        dist = nearest_walkable(px, pz, max_r=3.0)
        if dist is not None and dist <= 1.0:
            ok = True
            break
    tot += 1
    reach += ok
print(f"buildings with >=1 reachable door candidate: {reach}/{tot} "
      f"({100*reach/tot:.1f}%)")

# ---- render ------------------------------------------------------------------
out = np.zeros((H, W, 3), np.uint8)
out[inside_wall] = (26, 24, 30)
out[free] = (70, 62, 58)
out[walk] = (120, 108, 96)
out[main] = (232, 214, 178)
for p in PLAN["named_place_index"]:
    r, c = cell_of(*p["anchor"])
    out[max(0, r-4):r+4, max(0, c-4):c+4] = (220, 70, 60)
Image.fromarray(np.flipud(out)).save(
    "/tmp/claude-1000/-home-ran-src-rust-cathedralbevy/"
    "b167c8fc-524e-4146-98b8-df57bb421175/scratchpad/navmesh_probe.png")
print("\nwrote navmesh_probe.png")
