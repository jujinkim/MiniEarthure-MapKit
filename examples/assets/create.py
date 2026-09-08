#!/usr/bin/env python3
"""Reproduce the MIT synthetic static model, texture and recipe-4 source (stdlib)."""
import json
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parent
def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
def create(root=ROOT):
    root.mkdir(parents=True, exist_ok=True)
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 2, 2, 8, 6, 0, 0, 0))
    png += chunk(b'IDAT', zlib.compress(bytes([0,255,200,0,255,20,90,180,255,0,20,90,180,255,255,200,0,255]))) + chunk(b'IEND', b'')
    (root/'checker.png').write_bytes(png)
    # Asymmetric closed tetrahedron. The sloping face leaves empty AABB space.
    vertices = [[-400,0,-300],[500,0,-300],[-400,0,400],[-400,800,-300]]
    faces = [[0,1,2],[0,3,1],[0,2,3],[1,3,2]]
    positions = [[x/100,h/100,-y/100] for x,h,y in vertices]
    data = b''.join(struct.pack('<fff',*p) for p in positions)
    data += b''.join(struct.pack('<HHH',f[0],f[2],f[1]) for f in faces)
    uv_offset=len(data)
    data+=b''.join(struct.pack('<ff',*p) for p in [[0,0],[1,0],[0,1],[1,1]])
    image_offset=len(data);data+=png
    gltf={'asset':{'version':'2.0','generator':'MapKit MIT synthetic example'},'buffers':[{'byteLength':len(data)}],
          'bufferViews':[{'buffer':0,'byteOffset':0,'byteLength':48},{'buffer':0,'byteOffset':48,'byteLength':24},
                         {'buffer':0,'byteOffset':uv_offset,'byteLength':32},{'buffer':0,'byteOffset':image_offset,'byteLength':len(png)}],
          'accessors':[{'bufferView':0,'componentType':5126,'count':4,'type':'VEC3','min':[min(p[i] for p in positions) for i in range(3)],'max':[max(p[i] for p in positions) for i in range(3)]},
                       {'bufferView':1,'componentType':5123,'count':12,'type':'SCALAR'},
                       {'bufferView':2,'componentType':5126,'count':4,'type':'VEC2'}],
          'images':[{'bufferView':3,'mimeType':'image/png'}],'textures':[{'source':0}],
          'materials':[{'pbrMetallicRoughness':{'baseColorTexture':{'index':0},'roughnessFactor':0.7},'doubleSided':True}],
          'meshes':[{'primitives':[{'attributes':{'POSITION':0,'TEXCOORD_0':2},'indices':1,'material':0}]}],
          'nodes':[{'name':'Synthetic tetrahedron','mesh':0}],'scenes':[{'nodes':[0]}],'scene':0}
    j=json.dumps(gltf,separators=(',',':')).encode();j+=b' '*(-len(j)%4);data+=b'\0'*(-len(data)%4)
    (root/'tetra.glb').write_bytes(b'glTF'+struct.pack('<II',2,28+len(j)+len(data))+struct.pack('<I',len(j))+b'JSON'+j+struct.pack('<I',len(data))+b'BIN\0'+data)
    d=json.loads((ROOT.parent/'minimal/document.json').read_text())
    d.update(map_id='synthetic-assets',recipe_version=4,bounds={'min':[0,0],'max':[25600,25600]},cell_size_cm=12800,nodes=[],roads=[],buildings=[],zones=[],repetitions=[])
    attribution={'source':'MapKit synthetic assets/create.py','license':'MIT','notice':'Original procedural fixture; no third-party imagery'}
    material={'albedo_rgba':[180,230,255,255],'metallic_per_mille':100,'roughness_per_mille':700,'double_sided':True,'albedo_texture':'checker'}
    d['assets']=[{'id':'tetra','path':'tetra.glb','attribution':attribution,'collision':[], 'convex_collision':[{'vertices':vertices,'faces':faces}]},
                 {'id':'tinted','path':'tetra.glb','attribution':attribution,'collision':[], 'convex_collision':[{'vertices':vertices,'faces':faces}],'material':material},
                 {'id':'checker','path':'checker.png','attribution':attribution,'collision':[{'center':[0,150,0],'size_cm':[600,300,600]}],'material':{k:v for k,v in material.items() if k!='albedo_texture'}}]
    d['placements']=[{'id':id,'asset_id':asset,'position':p,'quarter_turns':q} for id,asset,p,q in [
        ('tetra-a','tetra',[12400,0,7000],0),('tetra-b','tinted',[7000,0,12000],1),
        ('image-box','checker',[5000,0,5000],0),('tree','builtin:tree',[3000,0,10000],0),
        ('fence','builtin:fence',[8000,0,4000],1),('light','builtin:streetlight',[10000,0,4000],0)]]
    (root/'document.json').write_text(json.dumps(d,indent=2)+'\n')
if __name__=='__main__': create()
