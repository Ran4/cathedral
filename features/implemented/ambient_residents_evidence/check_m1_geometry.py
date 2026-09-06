# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy", "pillow", "scipy"]
# ///
"""Independent metric checks plus deterministic rebake, no game/GPU required."""
import hashlib
import json
import math
import sys
from collections import Counter
from pathlib import Path

import numpy as np
from scipy.spatial import cKDTree

EVIDENCE = Path(__file__).resolve().parent
ROOT = next(path for path in EVIDENCE.parents if (path / 'Cargo.toml').is_file())
sys.path.insert(0,str(ROOT/'scripts'))
from bake_navigation import Grid, Surface
from bake_resident_places import bake_resident_places, exclusion_mask, path_length, segment_exact

nav = json.loads((ROOT/'assets/world/navigation.json').read_text())
plan = json.loads((ROOT/'lore/places/ombreval_buildings.json').read_text())
grid = Grid(plan)
bits = np.unpackbits(np.frombuffer((ROOT/'assets/world/navigation.bin').read_bytes(),dtype=np.uint8))
surface = Surface(grid,bits[:grid.w*grid.h].reshape(grid.h,grid.w).astype(bool))
patches = nav['resident_places']['patches']
spots = [s for p in patches for s in p['spots']]
points = np.array([s['xz'] for s in spots])
spacing = float(cKDTree(points).query(points,k=2)[0][:,1].min())
assert spacing >= 1.6
forbidden = exclusion_mask(plan,surface,nav,ROOT)
assert not any(forbidden[grid.cell(*p)] for p in points)

# Exact Euclidean clearance from graph centre segments, independent of the
# exclusion raster and the old half-width labels. Batch to cap working memory.
edges = np.array([[nav['nodes'][a],nav['nodes'][b]] for a,b,_ in nav['edges']])
a,b = edges[:,0],edges[:,1]
ab = b-a
through = math.inf
for offset in range(0,len(points),128):
    delta = points[offset:offset+128,None,:]-a
    t = np.clip((delta*ab).sum(axis=2)/(ab*ab).sum(axis=1),0,1)
    through = min(through,float(np.linalg.norm(delta-t[:,:,None]*ab,axis=2).min()))
assert through >= 1.3, through

doors = np.array([nav['nodes'][d['node']] for d in nav['doors']])
door_min = float(cKDTree(doors).query(points)[0].min())
assert door_min >= 2

# Physical collider polygons, not the navigation bits: reject interiors and
# compute exact distance to every nearby polygon edge (spatial bbox filter).
polys=json.loads((ROOT/'assets/world/collision_footprints.json').read_text())['footprints']
polys += [next(b['polygon'] for b in plan['buildings'] if b['id']=='named_lanthorn')]
polys=[np.array(p) for p in polys if len(p)>=3]
boxes=np.array([[p[:,0].min(),p[:,1].min(),p[:,0].max(),p[:,1].max()] for p in polys])
wall_min=math.inf
for point in points:
    x,z=point
    nearby=np.where((boxes[:,0]-2<x)&(boxes[:,2]+2>x)&(boxes[:,1]-2<z)&(boxes[:,3]+2>z))[0]
    for i in nearby:
        a=polys[i];b=np.roll(a,-1,axis=0);ab=b-a
        t=np.clip(((point-a)*ab).sum(axis=1)/np.maximum((ab*ab).sum(axis=1),1e-12),0,1)
        d=float(np.linalg.norm(point-a-t[:,None]*ab,axis=1).min())
        crossing=(a[:,1]>z)!=(b[:,1]>z)
        inside=False
        for aa,bb in zip(a[crossing],b[crossing]):
            if x<(bb[0]-aa[0])*(z-aa[1])/(bb[1]-aa[1])+aa[0]: inside=not inside
        assert not inside, (point,i)
        wall_min=min(wall_min,d)
assert wall_min >= .55,wall_min

for patch in patches:
    assert len(patch['spots'])==patch['capacity'] and 2<=patch['capacity']<=6
    anchor=patch['spots'][0]['xz']
    for spot in patch['spots']:
        assert segment_exact(surface,anchor,spot['xz'])
    for route in [patch['graph_path'],patch['home_path']]:
        for a,b in zip(route,route[1:]): assert segment_exact(surface,a,b)
    if patch['home_path']:
        assert max(math.dist(anchor,s['xz']) for s in patch['spots'])+path_length(patch['home_path'])<=30

rebake=bake_resident_places(plan,surface,nav,ROOT)
assert rebake==nav['resident_places'],'bake must be deterministic'
# Unmodified street fields are compared to HEAD by this read-only invocation's
# caller, so this proof does not require git or read arbitrary repository state.
by_building={b['id']:b for b in plan['buildings']}
ward_capacity=Counter()
for patch in patches: ward_capacity[by_building[patch['building']].get('district','unknown')]+=patch['capacity']
fixtures=json.loads((EVIDENCE/'probe_fixtures.json').read_text())
views={}
for view in fixtures['views']:
    x0,z0,x1,z1=view['observation_bounds_xz']
    selected=[s['id'] for s in spots if x0<=s['xz'][0]<=x1 and z0<=s['xz'][1]<=z1]
    views[view['id']]={'capacity':len(selected),'example_spots':selected[:4]}
home_capacity=Counter()
for patch in patches:
    if patch['home_building']: home_capacity[patch['home_building']]+=patch['capacity']
report={'patches':len(patches),'capacity':len(spots),'home_connected_capacity':sum(home_capacity.values()),
    'home_connected_patches':sum(bool(p['home_building']) for p in patches),
    'home_connected_doors':len(home_capacity),
    'housed_capacity_by_door_cap':{cap:sum(min(cap,n) for n in home_capacity.values()) for cap in range(1,6)},
    'patch_size_histogram':dict(sorted(Counter(p['capacity'] for p in patches).items())),
    'minimum_spot_spacing_m':spacing,'minimum_door_distance_m':door_min,
    'minimum_through_centreline_distance_m':through,'minimum_physical_wall_distance_m':wall_min,
    'wards':dict(sorted(ward_capacity.items())),'probe_views':views,'deterministic_rebake':True,
    'request_capacity':[{'requested':n,'placeable':min(n,len(spots)),'unplaced':max(0,n-len(spots))} for n in [1000,2000,20000]],
    'navigation_sha256':hashlib.sha256((ROOT/'assets/world/navigation.json').read_bytes()).hexdigest()}
print(json.dumps(report,indent=2))
