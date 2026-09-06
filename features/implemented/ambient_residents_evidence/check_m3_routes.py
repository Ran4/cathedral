# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy", "pillow", "scipy"]
# ///
"""Reproduce append-only refinement and validate all external node references."""
import copy
import json
import math
import subprocess
import sys
from pathlib import Path
import numpy as np

EVIDENCE=Path(__file__).resolve().parent
ROOT=next(p for p in EVIDENCE.parents if (p/'Cargo.toml').is_file())
sys.path.insert(0,str(ROOT/'scripts'))
from bake_navigation import Grid,Surface
from bake_route_clearance import refine_routes
from bake_resident_places import segment_exact,collision_clearance_test,exclusion_mask

doc=json.loads((ROOT/'assets/world/navigation.json').read_text())
baseline=json.loads((EVIDENCE/'baseline_meta.json').read_text())['head']
original=json.loads(subprocess.check_output(['git','show',f'{baseline}:assets/world/navigation.json'],cwd=ROOT))
plan=json.loads((ROOT/'lore/places/ombreval_buildings.json').read_text())
grid=Grid(plan)
bits=np.unpackbits(np.frombuffer((ROOT/'assets/world/navigation.bin').read_bytes(),dtype=np.uint8))
surface=Surface(grid,bits[:grid.w*grid.h].reshape((grid.h,grid.w)).astype(bool))
assert doc['nodes'][:len(original['nodes'])]==original['nodes']
assert doc['endpoint_nodes']==len(original['nodes'])
for field in ['grid','places','sites','doors','reference']:
    assert doc[field]==original[field],field
rebuilt=copy.deepcopy(original)
measurements=refine_routes(surface,rebuilt)
assert rebuilt['nodes']==doc['nodes']
assert rebuilt['edges']==doc['edges']
again=copy.deepcopy(doc)
refine_routes(surface,again)
assert again['nodes']==doc['nodes']
assert again['edges']==doc['edges']
for filename in ['places.json','shelters.json']:
    data=json.loads((ROOT/'assets/world'/filename).read_text())
    def refs(value):
        if isinstance(value,dict):
            for k,v in value.items():
                if k in ('node','route_node') and isinstance(v,int):
                    assert v<len(original['nodes']), (filename,k,v)
                    assert doc['nodes'][v]==original['nodes'][v]
                else:refs(v)
        elif isinstance(value,list):
            for v in value:refs(v)
    refs(data)
clear=collision_clearance_test(ROOT)
excluded=exclusion_mask(plan,surface,doc,ROOT)
shelters={s['id']:s for s in json.loads((ROOT/'assets/world/shelters.json').read_text())['shelters']}
spots=doc['resident_places']['shelter_spots']
for s in spots:
    assert s['shelter'] in shelters
    p=s['spot']['xz']
    assert clear(p) and not excluded[grid.cell(*p)]
    assert segment_exact(surface,p,p)
measurements.update({'preserved_original_nodes':True,'external_references_preserved':True,
                     'rebake_identical':True,'refinement_idempotent':True,
                     'safe_shelter_spots':len(spots)})
print(json.dumps(measurements,indent=2))
