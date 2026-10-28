from pathlib import Path
import tempfile
import unittest
from PIL import Image
from sign_atlas import build

ROOT = Path(__file__).resolve().parents[1]


class SignAtlas(unittest.TestCase):
    def test_committed_atlas_reproduces_from_fixed_seeds(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "sign_atlas.png"
            build(path)
            self.assertEqual(path.read_bytes(), (ROOT / "godot/textures/sign_atlas.png").read_bytes())
            with Image.open(path) as image:
                self.assertEqual((image.size, image.mode), ((1024, 640), "RGB"), "16x32 signs of 64x20 px")


if __name__ == "__main__":
    unittest.main()
