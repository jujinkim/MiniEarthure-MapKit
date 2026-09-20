import unittest
from regional_assets import library

class RegionalAssets(unittest.TestCase):
    def test_deterministic_complete_kit(self):
        a,b=library(),library()
        self.assertGreaterEqual(len(a),40)
        for name,model in a.items():
            self.assertEqual(model.export(),b[name].export(),name)
            self.assertTrue(model.collision or model.convex,name)
            self.assertLess(len(model.export()),200_000,name)
    def test_architecture_variants(self):
        models=library()
        for style in ['tower','shop','stone','farm','polar','stilt','warehouse']:
            self.assertEqual(len({models[f'{style}-{v}'].export() for v in range(4)}),4)

if __name__=='__main__':unittest.main()
