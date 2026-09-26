import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from PIL import Image
from richer_assets import create,textures,unpack,water_mesh

ROOT=Path(__file__).resolve().parents[1]
class RicherAssets(unittest.TestCase):
    def test_thin_water_slivers_never_create_zero_volume_integer_proxies(self):
        for width in [.06,.07,.08,.09,.1,.11,.12,.2,1.0]:
            model=water_mesh([[(0,0),(width,1),(width*.93,.4)]],[0,0],True,lambda x,z:.5)
            for c in model.convex:
                for face in c['faces']:
                    a,b,d=[c['vertices'][i] for i in face];u=[b[i]-a[i] for i in range(3)];v=[d[i]-a[i] for i in range(3)]
                    normal=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
                    self.assertNotEqual(normal,[0,0,0])
                    sides=[sum(normal[i]*(p[i]-a[i]) for i in range(3)) for p in c['vertices']]
                    self.assertEqual(max(sides),0);self.assertLess(min(sides),0)
    def test_common_library_and_tiles_reproduce_without_embedded_bitmaps(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'library';create(p);tiles=Path(tmp)/'tiles';textures(tiles)
            expected=json.loads((ROOT/'assets/richer-library/library.json').read_text())
            self.assertEqual(json.loads((p/'library.json').read_text()),expected)
            for record in expected['assets']:
                data=(p/record['path']).read_bytes();self.assertEqual(hashlib.sha256(data).hexdigest(),expected['sha256'][record['path']])
                d,_=unpack(data);self.assertNotIn('images',d)
                self.assertTrue(record['collision'] or record['convex_collision'])
                for prim in d['meshes'][0]['primitives']:self.assertIn('TEXCOORD_0',prim['attributes'])
            for tile in tiles.glob('*.png'):
                self.assertEqual(tile.read_bytes(),(ROOT/'godot/textures'/tile.name).read_bytes())
                with Image.open(tile) as image:self.assertEqual(image.size,(256,256))

if __name__=='__main__':unittest.main()
