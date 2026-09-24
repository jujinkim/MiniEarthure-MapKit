"""Public geometry regressions; synthetic crossfall and existing per-model budgets."""
import json
import math
import struct
import unittest
from pathlib import Path
from driving_structures import authoring_templates, definition, fit_to_surface, ramp
from world_assets import tree, street_tree

class StreetGeometry(unittest.TestCase):
    def test_ramp_entry_follows_both_surface_edges(self):
        for yaw in [0, 37, 90, 193]:
            for slope, crossfall in [(0,0),(.2,.07),(-.13,.09)]:
                surface=lambda x,z: round(slope*z+crossfall*x)
                g=definition('ramp','ramp',[1000,surface(1000,2000),2000],yaw)
                fit_to_surface(g,surface)
                c,s=math.cos(math.radians(yaw)),math.sin(math.radians(yaw))
                entry=[v for v in g['parts'][0]['vertices'] if v[2]==-225]
                for x in [-100,100]:
                    top=max(v[1] for v in entry if v[0]==x)
                    self.assertEqual(top+g['position'][1],surface(1000+c*x-s*225,2000-s*x-c*225))
                for p in g['parts']:
                    center=[sum(v[a] for v in p['vertices'])/len(p['vertices']) for a in range(3)]
                    for f in p['faces']:
                        a,b,d=[p['vertices'][i] for i in f];u=[b[i]-a[i] for i in range(3)];v=[d[i]-a[i] for i in range(3)]
                        n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
                        self.assertGreater(sum(n[i]*(a[i]-center[i]) for i in range(3)),0)

    def test_current_authoring_templates(self):
        saved=json.loads((Path(__file__).resolve().parents[1]/'godot/driving_templates.json').read_text())
        self.assertEqual(saved,authoring_templates())
        self.assertEqual(len(saved),11)
        for kind in ['ramp','jump','humps']:
            for part in saved[kind]['parts']:
                z_values=[v[2] for v in part['vertices']]
                ends=[max(v[1] for v in part['vertices'] if v[2]==z) for z in [min(z_values),max(z_values)]]
                self.assertEqual(min(ends),0)
                self.assertEqual(min(v[1] for v in part['vertices']),-5)

    def test_tree_cost_and_ground_attachment(self):
        for kind in ['canopy','palm']:
            old,new=tree(kind),street_tree(kind)
            self.assertLessEqual(sum(len(fs) for vs,fs in new.groups.values()),sum(len(fs) for vs,fs in old.groups.values()))
            self.assertLessEqual(len(new.groups),len(old.groups))
            self.assertEqual(min(p[1] for vs,fs in new.groups.values() for p in vs),0)
            data=new.export();length=struct.unpack_from('<I',data,12)[0];doc=json.loads(data[20:20+length])
            self.assertEqual(len(doc['materials']),2)
            self.assertTrue(all('COLOR_0' in p['attributes'] for p in doc['meshes'][0]['primitives']))
            self.assertEqual(data,street_tree(kind).export())

if __name__=='__main__': unittest.main()
