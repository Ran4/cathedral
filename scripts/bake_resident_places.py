"""Deterministic frontage patches used by bake_navigation.py.

Standing geometry deliberately leaves the street graph unchanged. Centreline
bands, door approaches, bridge/stair chokes and service areas are exclusions,
not merely walkable-bit tests. Geometry is baked; occupancy belongs to the sim.
"""
import heapq
import json
import math
from collections import defaultdict

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage
from scipy.spatial import cKDTree

SPACING = 1.6
DOOR_CLEARANCE = 2.0
THROUGH_CLEARANCE = 1.3
STANDING_COLLISION_CLEARANCE = .55  # radius .35 + arrival tolerance .15 + margin


def collision_clearance_test(root):
    """Exact footprint distance supplements the raster at diagonal/thin walls."""
    polygons = json.loads((root/'assets/world/collision_footprints.json').read_text())['footprints']
    polygons = [p for p in polygons if len(p)>=3]
    bounds = np.array([[min(p[0] for p in poly),min(p[1] for p in poly),
                        max(p[0] for p in poly),max(p[1] for p in poly)] for poly in polygons])
    radius = STANDING_COLLISION_CLEARANCE

    def clear(point):
        x,z = point
        nearby = np.where((bounds[:,0]-radius<x)&(bounds[:,2]+radius>x)
                          &(bounds[:,1]-radius<z)&(bounds[:,3]+radius>z))[0]
        for i in nearby:
            poly = polygons[i]
            inside = False
            for a,b in zip(poly,poly[1:]+poly[:1]):
                dx,dz = b[0]-a[0],b[1]-a[1]
                length2 = dx*dx+dz*dz
                t = max(0,min(1,((x-a[0])*dx+(z-a[1])*dz)/length2)) if length2 else 0
                if math.hypot(x-a[0]-t*dx,z-a[1]-t*dz)<radius:
                    return False
                if (a[1]>z)!=(b[1]>z) and x<(b[0]-a[0])*(z-a[1])/(b[1]-a[1])+a[0]:
                    inside = not inside
            if inside:
                return False
        return True
    return clear


def segment_exact(surface, a, b):
    g = surface.grid
    ax, az = g.to_px(a)
    bx, bz = g.to_px(b)
    times = [0., 1.]
    for start, end in ((ax, bx), (az, bz)):
        if abs(end - start) < 1e-12:
            continue
        for line in range(math.ceil(min(start, end)), math.floor(max(start, end)) + 1):
            t = (line - start) / (end - start)
            if 0 < t < 1:
                times.append(t)
    times.sort()

    def clear(t):
        x, z = ax + (bx - ax) * t, az + (bz - az) * t
        cols = (round(x)-1, round(x)) if abs(x-round(x)) < 1e-8 else (math.floor(x),)
        rows = (round(z)-1, round(z)) if abs(z-round(z)) < 1e-8 else (math.floor(z),)
        return all(surface.walkable_cell(r, c) for r in rows for c in cols)

    return all(clear(t) for t in times) and all(clear((a+b)/2) for a, b in zip(times, times[1:]))


def path_length(path):
    return sum(math.dist(a, b) for a, b in zip(path, path[1:]))


def local_path(surface, a, b, limit=30):
    if math.dist(a, b) > limit:
        return None
    if segment_exact(surface, a, b):
        return [list(a), list(b)]
    g = surface.grid
    start, goal = g.cell(*a), g.cell(*b)
    came, costs = {}, {start: 0.}
    heap = [(math.dist(a, b), 0., start)]
    closed = set()
    while heap and len(closed) < 8192:
        _, cost, cell = heapq.heappop(heap)
        if cell in closed:
            continue
        closed.add(cell)
        if cell == goal:
            cells = [cell]
            while cell != start:
                cell = came[cell]
                cells.append(cell)
            raw = [list(g.world(*c)) for c in reversed(cells)]
            raw[0], raw[-1] = list(a), list(b)
            out, anchor = [raw[0]], 0
            for i in range(1, len(raw)):
                if not segment_exact(surface, raw[anchor], raw[i]):
                    if i == anchor+1:
                        return None
                    out.append(raw[i-1])
                    anchor = i-1
            out.append(raw[-1])
            return out if path_length(out) <= limit else None
        r, c = cell
        for dr, dc in ((-1,0),(0,-1),(0,1),(1,0),(-1,-1),(-1,1),(1,-1),(1,1)):
            nr, nc = r+dr, c+dc
            if not surface.walkable_cell(nr, nc):
                continue
            if dr and dc and not (surface.walkable_cell(r,nc) and surface.walkable_cell(nr,c)):
                continue
            ng = cost + math.hypot(dr,dc)*0.25
            h = math.dist(g.world(nr,nc),b)
            if ng+h <= limit and ng < costs.get((nr,nc),math.inf):
                came[nr,nc], costs[nr,nc] = (r,c), ng
                heapq.heappush(heap,(ng+h,ng,(nr,nc)))
    return None


def exclusion_mask(plan, surface, doc, root):
    g = surface.grid
    image = Image.new('L',(g.w,g.h),0)
    draw = ImageDraw.Draw(image)

    def circle(p, radius):
        x,z = g.to_px(p)
        r = radius/0.25
        draw.ellipse((x-r,z-r,x+r,z+r),fill=255)

    # Every graph edge, including actual doglegs and door connectors, retains
    # a 1.3 m centre band on either side. This does not assert graph lane widths.
    for a,b,_ in doc['edges']:
        pa,pb = doc['nodes'][a],doc['nodes'][b]
        # One-cell raster allowance makes the Euclidean centreline clearance
        # conservative even for slanting edges and floor-rounded pixel caps.
        radius = THROUGH_CLEARANCE + .25
        draw.line([g.to_px(pa),g.to_px(pb)],fill=255,width=math.ceil(2*radius/.25))
        circle(pa,radius)
        circle(pb,radius)
    for door in doc['doors']:
        circle(doc['nodes'][door['node']],DOOR_CLEARANCE)
        building = next(b for b in plan['buildings'] if b['id']==door['building'])
        polygon = building['polygon']
        a,b = polygon[door['edge']],polygon[(door['edge']+1)%len(polygon)]
        threshold = [(a[0]+b[0])/2,(a[1]+b[1])/2]
        circle(threshold,DOOR_CLEARANCE)
        draw.line([g.to_px(threshold),g.to_px(doc['nodes'][door['node']])],fill=255,width=16)
    by_name = {p['name']:doc['nodes'][p['node']] for p in doc['places']}
    by_name.update({p['name']:doc['nodes'][p['node']] for p in doc['sites']})
    for name,p in sorted(by_name.items()):
        if any(token in name.lower() for token in ('well','cistern','three-curb','needle','bridge','gate')):
            circle(p,6.0)
    food = json.loads((root/'assets/world/food.json').read_text())
    homes = json.loads((root/'assets/world/homes.json').read_text())['homes']
    for counter in food['stalls']+food['counters']:
        p = by_name.get(counter['site'])
        if p is None and counter.get('anchor_actor') in homes:
            home = homes[counter['anchor_actor']]
            door = next((d for d in doc['doors'] if d['building']==home['building']),None)
            if door:
                p = doc['nodes'][door['node']]
        if p is not None:
            off = counter.get('pitch_offset',[0,0])
            # Runtime falls back to the site if its offset is blocked. Protect
            # both so content/occupancy cannot place a resident in either queue.
            circle(p,6.0)
            circle((p[0]+off[0],p[1]+off[1]),6.0)
    for site in plan['sites']:
        if site['id']=='gradine':
            draw.polygon([g.to_px(p) for p in site['polygon']],fill=255)
    for building in plan['buildings']:
        if building.get('use')=='bridge':
            poly = building['polygon']
            draw.polygon([g.to_px(p) for p in poly],fill=255)
            for a,b in zip(poly,poly[1:]+poly[:1]):
                draw.line([g.to_px(a),g.to_px(b)],fill=255,width=16)
    return np.array(image)>0


def bake_resident_places(plan, surface, doc, root):
    g = surface.grid
    forbidden = exclusion_mask(plan,surface,doc,root)
    # Extra margin for resting bodies and arrival tolerance beyond the mover's
    # already-eroded main component. A point on a walkable bit alone is not a spot.
    safe = surface.main & ~forbidden & (ndimage.distance_transform_edt(surface.main)*.25 >= .5)
    collision_clear = collision_clearance_test(root)
    graph_tree = cKDTree(doc['nodes'])
    doors = {d['building']:d for d in doc['doors']}
    positions = defaultdict(list)
    patches = []
    for building in sorted(plan['buildings'],key=lambda b:b['id']):
        if building.get('use')=='bridge' or building['id'] in ('named_lanthorn','named_malt_house'):
            continue
        poly = building['polygon']
        area = sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(poly,poly[1:]+poly[:1]))
        for edge,(a,b) in enumerate(zip(poly,poly[1:]+poly[:1])):
            dx,dz = b[0]-a[0],b[1]-a[1]
            length = math.hypot(dx,dz)
            if length < 4:
                continue
            nx,nz = (dz/length,-dx/length) if area>0 else (-dz/length,dx/length)
            groups = defaultdict(list)
            for slot in range(max(0,int((length-2)/2))):
                t = 1.5+slot*2
                raw = (a[0]+dx*t/length+nx*1.25,a[1]+dz*t/length+nz*1.25)
                r,c = g.cell(*raw)
                if not g.in_bounds(r,c) or not safe[r,c]:
                    continue
                p = g.world(r,c)
                if not collision_clear(p):
                    continue
                bucket = (math.floor(p[0]/SPACING),math.floor(p[1]/SPACING))
                if any(math.dist(p,q)<SPACING for i in range(-1,2) for j in range(-1,2) for q in positions[bucket[0]+i,bucket[1]+j]):
                    continue
                groups[int(t//10)].append((slot,p,bucket))
            for chunk,group in sorted(groups.items()):
                # Five positions span 8 m; a gap/corner starts a separate patch.
                connected = []
                for candidate in group:
                    if not connected or segment_exact(surface,connected[0][1],candidate[1]):
                        connected.append(candidate)
                if len(connected)<2:
                    continue
                anchor = connected[0][1]
                _,indices = graph_tree.query(anchor,k=min(16,len(doc['nodes'])))
                graph_path = None
                for node in sorted(map(int,np.atleast_1d(indices)),key=lambda n:(math.dist(anchor,doc['nodes'][n]),n)):
                    graph_path = local_path(surface,anchor,doc['nodes'][node],20)
                    if graph_path:
                        break
                if graph_path is None:
                    continue
                max_offset = max(math.dist(anchor,p) for _,p,_ in connected)
                home_path = None
                door = doors.get(building['id'])
                if door:
                    home_path = local_path(surface,anchor,doc['nodes'][door['node']],30-max_offset)
                patch_id = f"rp_{building['id']}_e{edge}_c{chunk}"
                spots = []
                for slot,p,bucket in connected:
                    # The current group's own centres were at least two metres
                    # apart before snapping; earlier accepted groups are checked
                    # again because candidates were gathered before committing.
                    if any(math.dist(p,q)<SPACING for i in range(-1,2) for j in range(-1,2) for q in positions[bucket[0]+i,bucket[1]+j]):
                        continue
                    spots.append({'id':f'{patch_id}_s{slot}','xz':list(p),'facing_yaw':round(math.atan2(-nx,-nz),6)})
                if len(spots)<2:
                    continue
                for spot in spots:
                    p = spot['xz']
                    positions[math.floor(p[0]/SPACING),math.floor(p[1]/SPACING)].append(p)
                name = building.get('name')
                description = f"beside {name}" if name else f"beside a frontage in {building.get('district','the city')}"
                patches.append({'id':patch_id,'building':building['id'],'description':description,
                    'capacity':len(spots),'spots':spots,'graph_node':node,'graph_path':graph_path,
                    'home_building':building['id'] if home_path else None,'home_path':home_path or []})
    # Actual authored roof polygons only. Independently spaced destinations
    # use the same standing/door/through-route protections as frontages. Roof
    # capacity is an upper bound; a narrow passage may have no safe stop.
    shelter_spots = []
    shelters = json.loads((root/'assets/world/shelters.json').read_text())['shelters']
    for shelter in sorted(shelters, key=lambda s:s['id']):
        if shelter.get('access','public') != 'public':
            continue
        poly = shelter['polygon_xz']
        def inside(point):
            x,z = point
            return sum((a[1]>z)!=(b[1]>z) and x<(b[0]-a[0])*(z-a[1])/(b[1]-a[1])+a[0]
                       for a,b in zip(poly,poly[1:]+poly[:1])) % 2 == 1
        count = 0
        for row in range(math.floor(min(p[1] for p in poly)/2),math.ceil(max(p[1] for p in poly)/2)):
            for col in range(math.floor(min(p[0] for p in poly)/2),math.ceil(max(p[0] for p in poly)/2)):
                r,c = g.cell(col*2+.5,row*2+.5)
                point = g.world(r,c)
                if count >= shelter.get('capacity',12) or not g.in_bounds(r,c) or not safe[r,c]:
                    continue
                if not inside(point) or not collision_clear(point):
                    continue
                bucket = (math.floor(point[0]/SPACING),math.floor(point[1]/SPACING))
                if any(math.dist(point,q)<SPACING for i in range(-1,2) for j in range(-1,2) for q in positions[bucket[0]+i,bucket[1]+j]):
                    continue
                positions[bucket].append(point)
                shelter_spots.append({'shelter':shelter['id'], 'spot':{
                    'id':f'rs_{shelter["id"]}_{row}_{col}', 'xz':list(point), 'facing_yaw':0.}})
                count += 1
    return {'patches':patches, 'shelter_spots':shelter_spots}
