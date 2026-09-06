"""Measured street clearance, with append-only graph repairs/subdivision.

The navigation bitset is already eroded for the body. Distance to its blocked
cells bounds additional lane displacement; neither old nominal road width nor
the width of another incident edge is permission to move sideways.
"""
import math
from collections import Counter

import numpy as np
from scipy import ndimage

from bake_resident_places import segment_exact, local_path


def refine_routes(surface, doc):
    nodes = doc['nodes']
    original_count = len(nodes)
    doc.setdefault('endpoint_nodes', original_count)
    old_edges = doc['edges']
    distance = ndimage.distance_transform_edt(surface.main) * .25
    edges, repaired = [], []

    def width(a, b):
        count = max(1, math.ceil(math.dist(a, b) / .25))
        clear = math.inf
        for i in range(count + 1):
            t = i / count
            p = [a[k] + t * (b[k] - a[k]) for k in (0, 1)]
            row, col = surface.grid.cell(*p)
            centre = surface.grid.world(row, col)
            # Nearest blocked-cell centre minus its half diagonal and the
            # sample's offset from its own cell centre; then half a sample gap.
            bound = distance[row, col] - math.sqrt(2) * .125 - math.dist(p, centre) - .125
            clear = min(clear, bound)
        # Width includes the body radius. Values below the former .6 minimum
        # are meaningful at real corners/doors; no artificial minimum uplift.
        return math.floor((.35 + max(0., min(3.65, clear))) * 1000) / 1000

    for a, b, old_width in old_edges:
        pa, pb = nodes[a], nodes[b]
        path = [pa, pb]
        if not segment_exact(surface, pa, pb):
            path = local_path(surface, pa, pb, max(30., math.dist(pa, pb) + 10.))
            if path is None:
                raise ValueError(f'No local exact repair for edge {a}, {b}')
            repaired.append([a, b])
        previous = a
        for part, (start, end) in enumerate(zip(path, path[1:])):
            steps = max(1, math.ceil((math.dist(start, end) - 1e-7) / 4.))
            for step in range(1, steps + 1):
                final = part == len(path) - 2 and step == steps
                point = [round(start[k] + (end[k] - start[k]) * step / steps, 8) for k in (0, 1)]
                if final:
                    nxt = b
                else:
                    nxt = len(nodes)
                    nodes.append(point)
                if not segment_exact(surface, nodes[previous], nodes[nxt]):
                    raise ValueError(f'Unsafe refinement {previous}, {nxt}')
                edges.append([previous, nxt, width(nodes[previous], nodes[nxt])])
                previous = nxt
    doc['edges'] = edges
    return {'original_nodes': original_count, 'nodes': len(nodes),
            'original_edges': len(old_edges), 'edges': len(edges),
            'repaired_edges': repaired,
            'width_histogram': dict(sorted(Counter(e[2] for e in edges).items())),
            'exact_failures': 0, 'maximum_segment_m': max(math.dist(nodes[a], nodes[b]) for a,b,_ in edges)}
