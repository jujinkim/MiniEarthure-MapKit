"""Standalone authored asset safety and reproducibility (no game/editor import)."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from world_assets import create

ROOT=Path(__file__).resolve().parents[1]

class WorldAssets(unittest.TestCase):
    def test_exact_library_reproduction_and_language_neutrality(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder=Path(tmp)/'library';catalog=create(folder)
            expected=json.loads((ROOT/'examples/world-library/library.json').read_text())
            self.assertEqual(json.loads(json.dumps(catalog)),expected)
            for record in catalog['assets']:
                data=(folder/record['path']).read_bytes()
                self.assertEqual(hashlib.sha256(data).hexdigest(),expected['sha256'][record['path']])
                length=struct.unpack_from('<I',data,12)[0];doc=json.loads(data[20:20+length])
                self.assertNotIn('images',doc)
                self.assertTrue(record['collision'] or record['convex_collision'])

    def test_every_common_model_is_natively_admissible(self):
        cli=ROOT/'target/release/mapkit'
        self.assertTrue(cli.exists(),'build mapkit-cli release first')
        with tempfile.TemporaryDirectory() as tmp:
            folder=Path(tmp)/'library';catalog=create(folder)
            doc=json.loads((ROOT/'examples/minimal/document.json').read_text())
            doc.update(recipe_version=6,bounds={'min':[0,0],'max':[6400,6400]},cell_size_cm=1600,
                roads=[],nodes=[],buildings=[],heightmaps=[],zones=[],repetitions=[])
            for record in catalog['assets']:
                with self.subTest(asset=record['id']):
                    doc['assets']=[record];doc['placements']=[dict(id='instance',asset_id=record['id'],position=[3200,0,3200],quarter_turns=0)]
                    (folder/'document.json').write_text(json.dumps(doc))
                    result=subprocess.run([str(cli),'pack',str(folder),str(Path(tmp)/(record['id']+'.memap'))],capture_output=True,text=True)
                    self.assertEqual(result.returncode,0,result.stdout+result.stderr)

if __name__=='__main__':unittest.main()
