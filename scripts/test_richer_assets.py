import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from PIL import Image
from richer_assets import FACADE_TILES,LEAF_ATLAS,create,facade_textures,leaf_atlas,textures,unpack,water_mesh

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
    def test_racing_tiles_are_separate_bounded_and_reproducible(self):
        with tempfile.TemporaryDirectory() as tmp:
            tiles=Path(tmp);textures(tiles,racing=True)
            for tile in tiles.rglob('*.png'):
                self.assertEqual(tile.read_bytes(),(ROOT/'godot/textures/racing'/tile.relative_to(tiles)).read_bytes())
                with Image.open(tile) as image:
                    light=list(image.getchannel('R').getdata())
                    self.assertAlmostEqual(sum(light)/len(light)/255,.65,delta=.003)
                    self.assertGreater(max(light)-min(light),8)
                    self.assertLess(max(light)-min(light),30)
                    # Small relief, no grain normal spikes at any texture tier.
                    for channel in ('B','A'):
                        self.assertLess(max(image.getchannel(channel).getdata())-min(image.getchannel(channel).getdata()),15)

    def test_facade_relief_tiles_reproduce_at_every_tier(self):
        with tempfile.TemporaryDirectory() as tmp:
            tiles=Path(tmp);facade_textures(tiles)
            produced=sorted(str(t.relative_to(tiles)) for t in tiles.rglob('*.png'))
            self.assertEqual(produced,sorted(f'{d}{k}.png' for d in ['','128/'] for k in FACADE_TILES))
            for tile in tiles.rglob('*.png'):
                self.assertEqual(tile.read_bytes(),(ROOT/'godot/textures/facade'/tile.relative_to(tiles)).read_bytes())
                size=256 if tile.parent==tiles else int(tile.parent.name)
                with Image.open(tile) as image:
                    self.assertEqual((image.size,image.mode),((size,size),'RGBA'))
                    light=list(image.getchannel('R').getdata());rough=list(image.getchannel('G').getdata())
                    self.assertAlmostEqual(sum(light)/len(light)/255,.65,delta=.004)
                    self.assertAlmostEqual(sum(rough)/len(rough)/255,.5,delta=.004)
                    # Real relief, stronger than the flat racing tiles, never saturated.
                    self.assertGreater(max(light)-min(light),30)
                    if size==256:
                        for channel in ('B','A'):
                            values=list(image.getchannel(channel).getdata())
                            self.assertGreater(max(values)-min(values),40,tile)
                            self.assertAlmostEqual(sum(values)/len(values)/255,.5,delta=.01)

    def test_leaf_atlas_reproduces_as_data_channels(self):
        with tempfile.TemporaryDirectory() as tmp:
            leaf_atlas(Path(tmp))
            produced=Path(tmp)/LEAF_ATLAS
            self.assertEqual(produced.read_bytes(),(ROOT/'godot/textures'/LEAF_ATLAS).read_bytes())
            with Image.open(produced) as image:
                self.assertEqual((image.size,image.mode),((256,256),'RGBA'))
                alpha=list(image.getchannel('A').getdata())
                light=[l for l,a in zip(image.getchannel('R').getdata(),alpha) if a==255]
                # Clusters cover about a third of each cell; opaque luminance is centred on 0.65.
                self.assertGreater(sum(a>127 for a in alpha)/len(alpha),.25)
                self.assertLess(sum(a>127 for a in alpha)/len(alpha),.5)
                self.assertAlmostEqual(sum(light)/len(light)/255,.65,delta=.03)
                self.assertEqual(set(image.getchannel('B').getdata()),{0})
                # Transparent texels carry the mean leaf value (no dark mip fringes).
                self.assertEqual({l for l,a in zip(image.getchannel('R').getdata(),alpha) if a==0},{round(.65*255)})

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
            for tile in tiles.rglob('*.png'):
                self.assertEqual(tile.read_bytes(),(ROOT/'godot/textures'/tile.relative_to(tiles)).read_bytes())
                size=256 if tile.parent==tiles else int(tile.parent.name)
                with Image.open(tile) as image:
                    self.assertEqual(image.size,(size,size))
                    luminance=list(image.getchannel('R').getdata())
                    self.assertAlmostEqual(sum(luminance)/len(luminance)/255,.65,delta=.003)
                    self.assertGreater(max(luminance)-min(luminance),25)

if __name__=='__main__':unittest.main()
