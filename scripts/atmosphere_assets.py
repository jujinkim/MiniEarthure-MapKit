#!/usr/bin/env python3
"""MIT miniature material authoring. Preserve input; write a new project directory.

Adds embedded, deterministic 128px tile images and projected UVs to static GLBs.
Geometry, normal/index buffers and collision declarations are preserved. Lighting
roles are baked explicitly into recipe-8 metadata for these authored sample assets.
"""
import argparse
import copy
import json
import math
from pathlib import Path
import shutil
import struct
import zlib

CONCEPTS = ['polar','metropolis','countryside','middle-eastern','desert','jungle','southeast-asian']
MATERIALS = ['asphalt','concrete','brick','plaster','wood','metal','earth','gravel','sand','snow','bark','leaves']

def canonical(value):
    return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()

def png(kind, size=128):
    def chunk(tag, data):
        return struct.pack('>I',len(data))+tag+data+struct.pack('>I',zlib.crc32(tag+data)&0xffffffff)
    rows=bytearray()
    for y in range(size):
        rows.append(0)
        for x in range(size):
            noise=((x*73856093)^(y*19349663)^71231)&0xffffffff
            noise=(noise*1664525+1013904223)&0xffffffff
            value=229+(noise%17)-8
            if kind=='brick' and (y%24<2 or (x+(24 if (y//24)%2 else 0))%48<2): value=190
            elif kind in ['wood','bark']:
                value-=int(12*(1+math.sin(x*.35+math.sin(y*math.tau/size)*1.5)))
                if x%32<1:value-=18
            elif kind=='concrete' and (x%64==0 or y%64==0):value-=12
            elif kind in ['asphalt','gravel']:value-=noise%24
            elif kind=='sand':value-=int(9*(1+math.sin((y+math.sin(x*math.tau/size)*3)*math.tau/16)))
            elif kind=='metal':value=237-noise%7
            elif kind=='snow':value=242-noise%9
            rows.extend([max(0,min(255,value))]*3)
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',size,size,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(bytes(rows),9))+chunk(b'IEND',b'')

def unpack_glb(data):
    magic,version,length=struct.unpack_from('<4sII',data)
    if magic!=b'glTF' or version!=2 or length!=len(data):raise ValueError('Expected static GLB2')
    json_size,tag=struct.unpack_from('<I4s',data,12)
    if tag!=b'JSON':raise ValueError('Missing GLB JSON')
    doc=json.loads(data[20:20+json_size]);offset=20+json_size
    binary=bytearray(data[offset+8:])
    return doc,binary

def pack_glb(doc,binary):
    binary.extend(b'\0'*(-len(binary)%4));doc['buffers'][0]['byteLength']=len(binary)
    data=canonical(doc);data+=b' '*(-len(data)%4)
    return struct.pack('<4sII',b'glTF',2,28+len(data)+len(binary))+struct.pack('<I4s',len(data),b'JSON')+data+struct.pack('<I4s',len(binary),b'BIN\0')+binary

def style(data, asset_id):
    doc,binary=unpack_glb(data)
    # Existing map-language images retain their exact bytes and UVs.
    existing_textures=bool(doc.get('images'))
    windows=[];bulbs=[]
    lamp='lamp' in asset_id or 'streetlight' in asset_id
    building=any(key in asset_id for key in ['building','tower','cabin','barn','house','veranda','shop','home','office','hotel','market-hall','arcade']) or asset_id in ['q01-cafe','q01-book','q01-diner','q01-stationer','q01-pharmacy']
    texture_cache={}
    for index,material in enumerate(doc.get('materials',[])):
        pbr=material.setdefault('pbrMetallicRoughness',{})
        r,g,b,a=pbr.get('baseColorFactor',[1,1,1,1])
        if building and g>r*1.18 and b>r*1.18 and r<.55 and b>g*.8 and b>.25:
            windows.append(index);material['name']='window-'+str(index)
            pbr['roughnessFactor']=.36
            continue
        if lamp and r>.7 and g>.65:
            bulbs.append(index);material['name']='lamp-'+str(index)
            continue
        if existing_textures:continue
        kind='plaster'
        if 'snow' in asset_id or (r>.83 and b>r):kind='snow'
        elif 'sand' in asset_id:kind='sand'
        elif 'tree' in asset_id or 'palm' in asset_id:kind='leaves' if g>r else 'bark'
        elif 'fence' in asset_id or 'barn' in asset_id:kind='wood'
        elif r>g*1.5 and r>b*1.6:kind='brick'
        elif max(r,g,b)-min(r,g,b)<.08:kind='concrete' if r>.35 else 'metal'
        elif r>g>b and r<.65:kind='wood'
        if kind not in texture_cache:
            image=png(kind);binary.extend(b'\0'*(-len(binary)%4));offset=len(binary);binary.extend(image)
            doc['bufferViews'].append(dict(buffer=0,byteOffset=offset,byteLength=len(image)))
            images=doc.setdefault('images',[]);images.append(dict(bufferView=len(doc['bufferViews'])-1,mimeType='image/png',name=kind))
            samplers=doc.setdefault('samplers',[dict(magFilter=9729,minFilter=9987,wrapS=10497,wrapT=10497)])
            textures=doc.setdefault('textures',[]);textures.append(dict(source=len(images)-1,sampler=0))
            texture_cache[kind]=len(textures)-1
        material['name']=kind+'-'+str(index)
        pbr['baseColorTexture']=dict(index=texture_cache[kind])
        pbr['roughnessFactor']=.5 if kind=='metal' else .88
    if existing_textures:return data,windows,bulbs
    def vectors(accessor_id):
        accessor=doc['accessors'][accessor_id];view=doc['bufferViews'][accessor['bufferView']]
        if accessor['componentType']!=5126 or accessor['type']!='VEC3':raise ValueError('Expected float VEC3 authoring buffer')
        offset=view.get('byteOffset',0)+accessor.get('byteOffset',0);stride=view.get('byteStride',12)
        return [struct.unpack_from('<fff',binary,offset+i*stride) for i in range(accessor['count'])]
    for mesh in doc['meshes']:
        for primitive in mesh['primitives']:
            attributes=primitive['attributes']
            positions=vectors(attributes['POSITION']);normals=vectors(attributes['NORMAL'])
            uv=[]
            for position,normal in zip(positions,normals):
                axis=max(range(3),key=lambda i:abs(normal[i]))
                axes=[i for i in range(3) if i!=axis]
                uv.append((position[axes[0]]/2.0,position[axes[1]]/2.0))
            binary.extend(b'\0'*(-len(binary)%4));offset=len(binary)
            binary.extend(b''.join(struct.pack('<ff',*point) for point in uv))
            doc['bufferViews'].append(dict(buffer=0,byteOffset=offset,byteLength=len(uv)*8,target=34962))
            doc['accessors'].append(dict(bufferView=len(doc['bufferViews'])-1,componentType=5126,count=len(uv),type='VEC2'))
            attributes['TEXCOORD_0']=len(doc['accessors'])-1
    doc['asset']['generator']='mapkit-atmosphere-materials-v1'
    return pack_glb(doc,binary),windows,bulbs

def upgrade(source, destination, concept='metropolis'):
    source=Path(source);destination=Path(destination)
    destination.mkdir(parents=True,exist_ok=False)
    doc=json.loads((source/'document.json').read_text())
    # Copy only declarative source/payload files, never imports, caches or secrets.
    paths={a['path'] for a in doc['assets']}|{h['path'] for h in doc['heightmaps']}
    for path in paths:
        if Path(path).is_absolute() or '..' in Path(path).parts:raise ValueError('Unsafe asset path')
        target=destination/path;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source/path,target)
    lights=[]
    for asset in doc['assets']:
        if not asset['path'].endswith('.glb'):continue
        target=destination/asset['path'];data,windows,bulbs=style(target.read_bytes(),asset['id']);target.write_bytes(data)
        if windows or bulbs:
            lights.append(dict(asset_id=asset['id'],window_materials=windows,bulb_materials=bulbs,
                position_cm=[0,312 if 'world-' in asset['id'] else 334,-80],range_cm=1200,color=[255,218,165]))
    latitude={'polar':78000,'metropolis':37500,'countryside':49000,'middle-eastern':34000,'desert':29000,'jungle':1000,'southeast-asian':14000}[concept]
    architecture,climate,settlement={'polar':('timber','polar','sparse'),'metropolis':('modern','temperate','urban'),'countryside':('rural','temperate','village'),'middle-eastern':('adobe','arid','urban'),'desert':('adobe','arid','wilderness'),'jungle':('tropical','tropical','wilderness'),'southeast-asian':('tropical','tropical','village')}[concept]
    doc['environment']=dict(architecture=architecture,climate=climate,settlement=settlement,version=1,concept=concept,latitude_mdeg=latitude,longitude_mdeg=0,utc_offset_minutes=0,
        sunrise_minutes=360,sunset_minutes=1080,regions=[],lights=lights)
    doc['recipe_version']=8;doc['revision']+=1
    doc['attributions'].append(dict(source='mapkit-atmosphere-materials-v1',license='MIT',notice='Original deterministic miniature material images and projected UV authoring; original geometry and source attribution retained.'))
    (destination/'document.json').write_bytes(canonical(doc))
    for name in ['driving.json','world.json']:
        if (source/name).exists():shutil.copyfile(source/name,destination/name)
    return doc

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source',type=Path);parser.add_argument('destination',type=Path)
    parser.add_argument('--concept',choices=CONCEPTS,default='metropolis')
    args=parser.parse_args();upgrade(args.source,args.destination,args.concept)
