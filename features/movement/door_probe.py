# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy", "pillow", "scipy"]
# ///
"""Are the REAL doors reachable?

The probe tested "any edge midpoint". The game picks exactly one edge, via
FNV-1a. Different claim. This tests the actual rule, reproducing
`stable_hash` and `add_facade_openings` from src/city/mod.rs:920-923,1949-1953.
"""
import json, math
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

ROOT = Path("/home/ran/src/rust/cathedralbevy")
PLAN = json.loads((ROOT / "lore/places/ombreval_buildings.json").read_text())
CELL, AGENT_R = 0.25, 0.35


def stable_hash(text: str) -> int:            # src/city/mod.rs:1949-1953
    h = 2166136261
    for b in text.encode():
        h = ((h ^ b) * 16777619) & 0xFFFFFFFF
    return h


xs = [p[0] for p in PLAN["wall_polygon_xz"]]
zs = [p[1] for p in PLAN["wall_polygon_xz"]]
x0, x1 = min(xs) - 8, max(xs) + 8
z0, z1 = min(zs) - 8, max(zs) + 8
W, H = int((x1 - x0) / CELL) + 1, int((z1 - z0) / CELL) + 1
px = lambda p: ((p[0] - x0) / CELL, (p[1] - z0) / CELL)

img = Image.new("L", (W, H), 0)
d = ImageDraw.Draw(img)
d.polygon([px(p) for p in PLAN["wall_polygon_xz"]], fill=255)
for b in PLAN["buildings"]:
    d.polygon([px(p) for p in b["polygon"]], fill=0)
for f in PLAN["fixtures"]:
    a = math.radians(f["angle_deg"])
    hx, hz = f["size"][0] / 2, f["size"][1] / 2
    cx, cz = f["position"]
    d.polygon([px((cx + sx*hx*math.cos(a) - sz*hz*math.sin(a),
                   cz + sx*hx*math.sin(a) + sz*hz*math.cos(a)))
               for sx, sz in ((-1,-1), (1,-1), (1,1), (-1,1))], fill=0)
free = np.array(img) > 0

roads = Image.new("L", (W, H), 0)
rd = ImageDraw.Draw(roads)
for r in PLAN["roads"]:
    pts = [px(p) for p in r["points"]]
    w = max(1, int(r["width_m"] / CELL))
    rd.line(pts, fill=255, width=w, joint="curve")
    for p in pts:
        rd.ellipse([p[0]-w/2, p[1]-w/2, p[0]+w/2, p[1]+w/2], fill=255)
free |= np.array(roads) > 0

rc = int(np.ceil(AGENT_R / CELL))
yy, xx = np.mgrid[-rc:rc+1, -rc:rc+1]
walk = ndimage.binary_erosion(free, structure=(xx**2 + yy**2) <= rc**2)
lab, n = ndimage.label(walk)
main = lab == (int(np.argmax(ndimage.sum(walk, lab, range(1, n+1)))) + 1)


def reachable(x, z, max_r):
    r, c = int((z - z0) / CELL), int((x - x0) / CELL)
    for rad in range(int(max_r / CELL) + 1):
        lo_r, hi_r = max(0, r-rad), min(H, r+rad+1)
        lo_c, hi_c = max(0, c-rad), min(W, c+rad+1)
        if main[lo_r:hi_r, lo_c:hi_c].any():
            return rad * CELL
    return None


ok = far = blocked = 0
worst = []
for b in PLAN["buildings"]:
    poly = b["polygon"]
    n_e = len(poly)
    edge = stable_hash(b["id"]) % n_e            # THE ACTUAL RULE
    a, bb = poly[edge], poly[(edge + 1) % n_e]
    mx, mz = (a[0] + bb[0]) / 2, (a[1] + bb[1]) / 2
    # outward normal: away from the centroid
    cx = sum(p[0] for p in poly) / n_e
    cz = sum(p[1] for p in poly) / n_e
    ox, oz = mx - cx, mz - cz
    L = math.hypot(ox, oz) or 1.0
    # stand 0.8 m outside the threshold — where a person waits at a door
    dx, dz = mx + 0.8*ox/L, mz + 0.8*oz/L
    dist = reachable(dx, dz, 6.0)
    if dist is None:
        blocked += 1
        worst.append((b["id"], b["use"], None))
    elif dist <= 1.0:
        ok += 1
    else:
        far += 1
        worst.append((b["id"], b["use"], dist))

tot = len(PLAN["buildings"])
print(f"THE ACTUAL stable_hash-chosen doors, {tot} buildings:")
print(f"  reachable (<=1.0 m to walkable)   {ok:5d}  ({100*ok/tot:.1f}%)")
print(f"  far       (1-6 m)                 {far:5d}  ({100*far/tot:.1f}%)")
print(f"  blocked   (no walkable within 6m) {blocked:5d}  ({100*blocked/tot:.1f}%)")
print()
print("worst offenders:")
for i, (bid, use, dd) in enumerate(sorted(worst, key=lambda t: (t[2] is not None, t[2] or 0), reverse=True)[:10]):
    print(f"  {bid:20s} {use:16s} {'BLOCKED' if dd is None else f'{dd:.1f} m'}")
