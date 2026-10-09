"""Authored silhouettes retain real openings and metre-scale bounds at lower cost."""
import hashlib
import json
import struct
import unittest
import numpy as np
from authored_assets import library
from harbor_assets import library as harbor_library
from forest_assets import library as forest_library
from canyon_assets import library as canyon_library
from snow_assets import library as snow_library

def triangles(data):
    size=struct.unpack_from('<I',data,12)[0]
    doc=json.loads(data[20:20+size]);blob=data[28+size:];result=[]
    for primitive in doc['meshes'][0]['primitives']:
        accessor=doc['accessors'][primitive['attributes']['POSITION']]
        view=doc['bufferViews'][accessor['bufferView']]
        points=np.frombuffer(blob,dtype='<f4',count=accessor['count']*3,
            offset=view.get('byteOffset',0)+accessor.get('byteOffset',0)).reshape(-1,3)
        if 'indices' in primitive:
            indices=doc['accessors'][primitive['indices']];view=doc['bufferViews'][indices['bufferView']]
            points=points[np.frombuffer(blob,dtype='<u2',count=indices['count'],offset=view.get('byteOffset',0)+indices.get('byteOffset',0))]
        result.extend(points.reshape(-1,3,3))
    return np.array(result)

def ray_hits(faces,x,y):
    # Imported GLB front is positive Z. Test each front-facing ray intersection.
    origin=np.array([x,y,30.]);direction=np.array([0.,0.,-1.]);hits=[]
    for a,b,c in faces:
        e=b-a;f=c-a;h=np.cross(direction,f);det=np.dot(e,h)
        if abs(det)<1e-7:continue
        s=origin-a;u=np.dot(s,h)/det;q=np.cross(s,e);v=np.dot(direction,q)/det;t=np.dot(f,q)/det
        if 0<=u<=1 and v>=0 and u+v<=1 and t>=0:hits.append(origin[2]-t)
    return hits

class AuthoredAssets(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.assets=library()
    def test_deterministic_hashes_bounds_and_triangle_reduction(self):
        self.assertEqual(self.assets,library())
        reductions={}
        for name,(record,payloads,_) in self.assets.items():
            pair=[]
            for field in ('path','distant_path'):
                path=record[field];data=payloads[path]
                self.assertEqual(path,'assets/'+hashlib.sha256(data).hexdigest()+'.glb')
                pair.append(triangles(data))
            near,far=pair;self.assertLessEqual(len(far),len(near),name)
            # Architectural envelopes use exact origins; irregular crowns may
            # contract by up to 15% when intermediate ring vertices disappear.
            a=near.reshape(-1,3);b=far.reshape(-1,3);extent=np.ptp(a,axis=0)
            if name.startswith(('home','shop','tree','rock')) or name in ('market-hall','barn','shed'):
                self.assertTrue(np.all(np.abs(a.min(0)-b.min(0))<=np.maximum(.25,extent*.15)),name)
                self.assertTrue(np.all(np.abs(a.max(0)-b.max(0))<=np.maximum(.25,extent*.15)),name)
                self.assertLess(len(far),len(near),name)
            reductions[name]=[len(near),len(far)]
        print('Authored triangle counts (near/far):',json.dumps(reductions,sort_keys=True))
    def test_open_arcades_and_equipment_bays_survive_both_lods(self):
        for name,x,front,wall in [('market-hall',0,4.2,0),('shed',2,2.5,-2.4)]:
            record,payloads,_=self.assets[name]
            for field in ('path','distant_path'):
                hits=ray_hits(triangles(payloads[record[field]]),x,1.5)
                self.assertTrue(hits,(name,field))
                self.assertLess(max(hits),front-1,(name,field,'opening filled'))
                self.assertGreaterEqual(max(hits),wall-.2,(name,field,'back wall lost'))

class HarborAssets(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.assets=harbor_library()

    def test_paired_envelopes_hashes_and_reduction(self):
        self.assertEqual(self.assets,harbor_library())
        for name,(record,payloads,_) in self.assets.items():
            pair=[]
            for field in ('path','distant_path'):
                data=payloads[record[field]]
                self.assertEqual(record[field],'assets/'+hashlib.sha256(data).hexdigest()+'.glb')
                pair.append(triangles(data))
            near,far=pair
            self.assertLessEqual(len(far),len(near),name)
            a=near.reshape(-1,3);b=far.reshape(-1,3)
            self.assertTrue(np.all(np.abs(a.min(0)-b.min(0))<=.30),name)
            self.assertTrue(np.all(np.abs(a.max(0)-b.max(0))<=.30),name)
            if name.startswith(('urban-','warehouse-','container-')) or name=='dock-crane':
                self.assertLess(len(far),len(near),name)

    def test_crane_and_bridge_openings_survive(self):
        for name in ('dock-crane','harbor-pier'):
            record,payloads,_=self.assets[name]
            for field in ('path','distant_path'):
                faces=triangles(payloads[record[field]])
                self.assertFalse(ray_hits(faces,0,3),(name,field,'central opening filled'))
                self.assertTrue(ray_hits(faces,7.5 if name=='dock-crane' else 4,3),(name,field,'leg lost'))

class ForestAssets(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.assets=forest_library()

    def test_growth_forms_hashes_and_far_silhouettes(self):
        self.assertEqual(self.assets,forest_library())
        for name,(record,payloads,_) in self.assets.items():
            pair=[]
            for field in ('path','distant_path'):
                data=payloads[record[field]]
                self.assertEqual(record[field],'assets/'+hashlib.sha256(data).hexdigest()+'.glb')
                pair.append(triangles(data))
            near,far=pair;a=near.reshape(-1,3);b=far.reshape(-1,3)
            self.assertLessEqual(len(far),len(near),name)
            tolerance=np.maximum(.3,np.ptp(a,axis=0)*.18)
            self.assertTrue(np.all(np.abs(a.min(0)-b.min(0))<=tolerance),name)
            self.assertTrue(np.all(np.abs(a.max(0)-b.max(0))<=tolerance),name)
            if name.startswith(('cedar','sapling')) or name in ('beech','ranger-lodge'):
                self.assertLess(len(far),len(near),name)

    def test_bridge_and_shelter_remain_open(self):
        for name in ('forest-pier','picnic-shelter'):
            record,payloads,_=self.assets[name]
            for field in ('path','distant_path'):
                faces=triangles(payloads[record[field]])
                self.assertFalse(ray_hits(faces,0,2),(name,field))
                self.assertTrue(ray_hits(faces,3,2),(name,field))

class CanyonAssets(unittest.TestCase):
    def test_paired_strata_equipment_and_hashes(self):
        assets=canyon_library();self.assertEqual(assets,canyon_library())
        for name,(record,payloads,_) in assets.items():
            pair=[]
            for field in ('path','distant_path'):
                data=payloads[record[field]];self.assertEqual(record[field],'assets/'+hashlib.sha256(data).hexdigest()+'.glb')
                pair.append(triangles(data))
            near,far=pair;a=near.reshape(-1,3);b=far.reshape(-1,3)
            self.assertLessEqual(len(far),len(near),name)
            tolerance=np.maximum(.3,np.ptp(a,axis=0)*.18)
            self.assertTrue(np.all(np.abs(a.min(0)-b.min(0))<=tolerance),name)
            self.assertTrue(np.all(np.abs(a.max(0)-b.max(0))<=tolerance),name)
            if name.startswith('sandstone-'):self.assertLess(len(far),len(near),name)
    def test_open_quarry_bay_and_bridge_legs(self):
        for name,x in [('quarry-crusher',3),('canyon-pier',3.2)]:
            record,payloads,_=canyon_library()[name]
            for field in ('path','distant_path'):
                faces=triangles(payloads[record[field]])
                self.assertFalse(ray_hits(faces,0,2),(name,field))
                self.assertTrue(ray_hits(faces,x,2),(name,field))

class SnowAssets(unittest.TestCase):
    def test_alpine_pairs_and_frozen_surface(self):
        assets=snow_library();self.assertEqual(assets,snow_library())
        for name,(record,payloads,_) in assets.items():
            pair=[]
            for field in ('path','distant_path'):
                data=payloads[record[field]];self.assertEqual(record[field],'assets/'+hashlib.sha256(data).hexdigest()+'.glb');pair.append(triangles(data))
            near,far=pair;a=near.reshape(-1,3);b=far.reshape(-1,3)
            self.assertLessEqual(len(far),len(near),name)
            tolerance=np.maximum(.3,np.ptp(a,axis=0)*.18)
            self.assertTrue(np.all(np.abs(a.min(0)-b.min(0))<=tolerance),name)
            self.assertTrue(np.all(np.abs(a.max(0)-b.max(0))<=tolerance),name)
            if name.startswith(('snow-fir','alpine-lodge')):self.assertLess(len(far),len(near),name)
        record,payloads,_=assets['frozen-lake'];self.assertTrue(record['convex_collision'])
        self.assertEqual(max(v[1] for v in record['convex_collision'][0]['vertices']),0)
    def test_gallery_travel_opening_survives(self):
        record,payloads,_=snow_library()['snow-gallery']
        for field in ('path','distant_path'):
            faces=triangles(payloads[record[field]])
            self.assertFalse(ray_hits(faces,0,2),field);self.assertTrue(ray_hits(faces,6.6,2),field)

if __name__=='__main__':unittest.main()
