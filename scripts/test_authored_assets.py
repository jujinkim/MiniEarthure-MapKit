"""Authored silhouettes retain real openings and metre-scale bounds at lower cost."""
import hashlib
import json
import struct
import unittest
import numpy as np
from authored_assets import library

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

if __name__=='__main__':unittest.main()
