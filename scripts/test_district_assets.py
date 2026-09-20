import hashlib
import json
import unittest
from pathlib import Path
from district_assets import library
ROOT=Path(__file__).resolve().parents[1]

class DistrictAssets(unittest.TestCase):
    def test_reproducible_assets_and_city_scale(self):
        record=json.loads((ROOT/'examples/district-library/library.json').read_text())
        models=library()
        for name,model in models.items():
            payload=model.export();path='assets/'+name+'.glb'
            self.assertEqual(hashlib.sha256(payload).hexdigest(),record['sha256'][path],name)
            self.assertEqual(payload,(ROOT/'examples/district-library'/path).read_bytes())
        heights=[]
        for v in range(4):
            vertices=[p for values in models['civic-tower-'+str(v)].groups.values() for p in values[0]]
            heights.append(max(p[1] for p in vertices))
            self.assertGreaterEqual(max(p[0] for p in vertices)-min(p[0] for p in vertices),60)
        self.assertEqual(len(set(heights)),4)
        self.assertGreater(min(heights),70);self.assertGreater(max(heights),120)
        self.assertGreater(len(models['streetwall-0'].groups),5)
        self.assertEqual(len({models['forest-'+str(v)].export() for v in range(3)}),3)

if __name__=='__main__':unittest.main()
