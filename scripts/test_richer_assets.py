import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from PIL import Image
from richer_assets import create,textures,unpack

ROOT=Path(__file__).resolve().parents[1]
class RicherAssets(unittest.TestCase):
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
