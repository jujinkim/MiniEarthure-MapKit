#!/usr/bin/env python3
"""Original MIT miniature kit. Metres, modest relief, shared 256px material tiles.

No map layout, fonts, external datasets, or duplicated embedded bitmaps.
"""
import argparse
import hashlib
import json
import math
import random
import struct
from pathlib import Path
from PIL import Image
from world_assets import new, mass, gable, street_tree, outward, append
from world_geometry import glb

COLORS = {
    'brick': (.64,.40,.29,1), 'plaster': (.76,.73,.62,1),
    'wood': (.40,.29,.19,1), 'stone': (.51,.53,.48,1),
    'metal': (.24,.30,.31,1), 'glass': (.17,.33,.37,1),
    'leaf': (.29,.44,.25,1), 'sign': (.35,.52,.52,1),
}
ATTRIBUTION = dict(source='mapkit-richer-miniatures',license='MIT',notice='Original fictional miniature geometry and procedural material tiles. No external images or datasets.')

def unpack(data):
    n=struct.unpack_from('<I',data,12)[0]
    return json.loads(data[20:20+n]),bytearray(data[28+n:])

def pack(doc,binary):
    doc['buffers'][0]['byteLength']=len(binary)
    j=json.dumps(doc,sort_keys=True,separators=(',',':')).encode();j+=b' '*(-len(j)%4)
    return struct.pack('<4sII',b'glTF',2,28+len(j)+len(binary))+struct.pack('<I4s',len(j),b'JSON')+j+struct.pack('<I4s',len(binary),b'BIN\0')+binary

def export(model,roles=None):
    """Planar metre UVs include face normals, so walls and roofs never stretch."""
    doc,binary=unpack(model.export())
    for primitive in doc['meshes'][0]['primitives']:
        mi=primitive['material'];mat=doc['materials'][mi]
        color=tuple(mat['pbrMetallicRoughness']['baseColorFactor'])
        role=(roles or getattr(model,'material_roles',{})).get(color,next((k for k,v in COLORS.items() if v==color),'plaster'))
        mat['name']='mk_'+role
        if role=='glass':mat['pbrMetallicRoughness'].update(roughnessFactor=.25,metallicFactor=.15)
        if role=='metal':mat['pbrMetallicRoughness'].update(roughnessFactor=.48,metallicFactor=.7)
        p=doc['accessors'][primitive['attributes']['POSITION']];n=doc['accessors'][primitive['attributes']['NORMAL']]
        po=doc['bufferViews'][p['bufferView']]['byteOffset'];no=doc['bufferViews'][n['bufferView']]['byteOffset']
        uv=[]
        for i in range(p['count']):
            x,y,z=struct.unpack_from('<fff',binary,po+i*12);nx,ny,nz=struct.unpack_from('<fff',binary,no+i*12)
            uv.append((x,z) if abs(ny)>.6 else ((z,y) if abs(nx)>abs(nz) else (x,y)))
        offset=len(binary);binary.extend(b''.join(struct.pack('<ff',*v) for v in uv))
        doc['bufferViews'].append(dict(buffer=0,byteOffset=offset,byteLength=len(uv)*8,target=34962))
        doc['accessors'].append(dict(bufferView=len(doc['bufferViews'])-1,componentType=5126,count=len(uv),type='VEC2'))
        primitive['attributes']['TEXCOORD_0']=len(doc['accessors'])-1
    return pack(doc,binary)

def building(style,v):
    m=new();c=COLORS
    w=[3.2,4.1,3.7,4.6][v];d=[3.0,3.5,4.1,3.4][v]
    floors=1+(v%3);h=floors*1.45
    if style=='tower':w,d,h=6+v*.5,5+v*.3,12+v*3
    if style=='shed':w,d,h=6+v,4.5+v*.3,2.8+v*.2
    base=.65 if style=='stilt' else .10
    wall=c['wood'] if style in ['nord','stilt','farm'] else c['brick'] if style=='shop' else c['plaster']
    role='wood' if style in ['nord','stilt','farm'] else 'brick' if style=='shop' else 'plaster'
    tint=[(1.0,.98,.92),(.79,.91,.99),(.99,.89,.79),(.89,.96,.82)][v]
    wall=tuple(wall[i]*tint[i] for i in range(3))+(1,);m.material_roles={wall:role}
    if style=='nord':
        wall=[(.62,.28,.21,1),(.31,.45,.54,1),(.70,.64,.45,1),(.39,.51,.43,1)][v];m.material_roles[wall]='wood'
    # Foundation extends into graded parcel; door remains at street/yard level.
    mass(m,w,d,.45,c['stone'],-.35)
    if style=='stilt':
        m.collision.clear();m.groups.clear()
        for x in [-w/2+.25,w/2-.25]:
            for z in [-d/2+.25,d/2-.25]:mass(m,.22,.22,1.0,c['wood'],-.35,x,z)
    if style=='courtyard':
        mass(m,1.0,d,h,wall,base,-w/2+.5)
        mass(m,1.0,d,h,wall,base,w/2-.5)
        mass(m,w-2,1.0,h,wall,base,0,d/2-.5)
    else:mass(m,w,d,h,wall,base)
    mass(m,w+.18,d+.18,.15,c['stone'],base)
    # Recessed dark panes, projecting frames/sills, compact metre-scale bays.
    for side in [-1,1]:
        z=side*(d/2+.018)
        for row in range(max(1,round(h/1.45))):
            for x in ([-w/2+.5,w/2-.5] if style=='courtyard' else [-w*.29,w*.29]):
                y=base+.87+row*1.45
                m.solid((x,y,z),(.67,.74,.035),c['glass'])
                for dx in [-.365,.365]:m.solid((x+dx,y,z+side*.035),(.055,.85,.09),c['wood'])
                m.solid((x,y-.42,z+side*.08),(.84,.07,.22),c['stone'],True)
    front=-d/2
    door=d/2-1.02 if style=='courtyard' else front-.022
    m.solid((0,base+.55,door),(.65,1.10,.045),c['wood'])
    for x in [-.37,.37]:m.solid((x,base+.59,door-.025),(.06,1.18,.09),c['stone'],True)
    m.solid((0,base+.10,front-.20),(.92,.20,.40),c['stone'],True)
    if style in ['shop','shed','stilt']:
        m.solid((0,base+1.35,front-.50),(w+.25,.12,1.08),c['metal'],True)
        # Solid board/frame, abstract coloured pictogram; no readable lettering.
        m.solid((w*.27,base+1.87,front-.16),(1.12,.44,.18),c['wood'],True)
        m.solid((w*.27,base+1.87,front-.26),(.98,.31,.025),c['sign'])
        for k in range(3):m.solid((w*.27-.28+k*.27,base+1.86,front-.28),(.12,.12+(k%2)*.08,.02),c['plaster'])
    if style in ['courtyard','tower']:
        if style=='courtyard':
            for x in [-w/2+.5,w/2-.5]:mass(m,1.15,d+.25,.18,c['stone'],base+h,x)
            mass(m,w-2,1.15,.18,c['stone'],base+h,0,d/2-.5)
        else:mass(m,w+.25,d+.25,.18,c['stone'],base+h)
        for x in [-w/2,w/2]:mass(m,.15,d,.32,wall,base+h,x)
        mass(m,.8 if style=='courtyard' else 1,1,.45,c['metal'],base+h,w/2-.5 if style=='courtyard' else .6,.5)
    else:
        gable(m,w+.40,d+.45,base+h,.65+v*.10,c['metal'] if style in ['nord','shed'] else c['brick'])
        mass(m,w+.44,.18,.10,c['stone'],base+h,0,-d/2-.17)
    return m

def rock(v):
    m=new();rng=random.Random(90+v)
    vs=[(0,1.0+v*.24,0)]+[(math.cos(i*math.tau/7)*(1+.2*rng.random()),-.08,math.sin(i*math.tau/7)*(.65+.1*v)) for i in range(7)]
    faces=outward(vs,[(0,1+i,1+(i+1)%7) for i in range(7)]+[(1,7,6),(1,6,5),(1,5,4),(1,4,3),(1,3,2)])
    m.faces(vs,faces,COLORS['stone']);m.convex=[dict(vertices=[[round(c*100) for c in p] for p in vs],faces=faces)]
    return m

def water_mesh(triangles,origin,flowing,depth,elevation=lambda x,z:0.0):
    """Cell-sized continuous water geometry with disjoint, thin static proxies.

    Native manual placement footprints exclude touching neighbours. Keep the
    physical proxy 3cm inside the visible surface without opening visual seams.
    No simulation, buoyancy or new material/physics contract is introduced.
    """
    from shapely.geometry import Polygon
    from world_geometry import cm
    m=new();cx,cz=origin;color=(.29,.51,.49,1)
    for polygon in triangles:
        if Polygon(polygon).area<.002:continue
        center=[sum(p[k] for p in polygon)/3 for k in range(2)]
        vertices=[(center[0]-cx,elevation(*center),center[1]-cz)]+[(x-cx,elevation(x,z),z-cz) for x,z in polygon]
        # GEOS triangles are clockwise or counterclockwise depending on ring.
        a,b,c=polygon
        ccw=(b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])>0
        faces=[(0,(i+1)%3+1,i+1) if ccw else (0,i+1,(i+1)%3+1) for i in range(3)]
        m.faces(vertices,faces,color)
        inset=Polygon(polygon).buffer(-.03,join_style=2)
        if inset.is_empty:continue
        inner=list(inset.exterior.coords)[:-1]
        if len(inner)!=3:continue
        vs=[cm((x-cx,elevation(x,z),z-cz)) for x,z in inner];vs += [[p[0],p[1]-2,p[2]] for p in vs]
        a,b,c=vs[:3]
        if (b[0]-a[0])*(c[2]-a[2])-(b[2]-a[2])*(c[0]-a[0])==0:continue
        fs=outward(vs,[(0,1,2),(3,5,4),(0,3,4),(0,4,1),(1,4,5),(1,5,2),(2,5,3),(2,3,0)])
        if len(set(tuple(p) for p in vs))==6:m.convex.append(dict(vertices=vs,faces=fs))
    def tint(p):
        d=depth(cx+p[0],cz-p[2]);return (1.08-.40*d,1.10-.28*d,1.0-.1*d)
    m.vertex_tint=tint;m.material_roles={color:'water_flow' if flowing else 'water_still'}
    return m

def library():
    result={f'{style}-{v}':building(style,v) for style in ['shop','house','farm','nord','courtyard','stilt','shed','tower'] for v in range(4)}
    for kind in ['canopy','palm']:
        for v in range(3):
            m=street_tree(kind);scale=.32+v*.055
            m.groups={COLORS['leaf'] if color[1]>color[0] else COLORS['wood']:[[(x*scale,y*scale,z*scale) for x,y,z in vs],fs] for color,(vs,fs) in m.groups.items()}
            for c in m.collision:c['center']=[round(a*scale) for a in c['center']];c['size_cm']=[max(1,round(a*scale)) for a in c['size_cm']]
            result[kind+'-'+str(v)]=m
    for v in range(3):result['rock-'+str(v)]=rock(v)
    for v in range(3):
        m=new()
        for x,z in [(-1.2,-1.2),(1.4,-.7),(.3,1.4)]:append(m,result['canopy-'+str(v)],x,z)
        result['grove-'+str(v)]=m
    for v in range(4):
        m=new();w,d=10+v*2,7+v
        m.solid((0,-.04,0),(w,.08,d),(.39,.43,.24,1),True)
        for z in range(-d//2+1,d//2):
            m.solid((0,.025,z),(w-.3,.05,.28),(.47,.51,.28,1))
        m.material_roles={(.39,.43,.24,1):'earth',(.47,.51,.28,1):'grass'}
        result['crop-'+str(v)]=m
    m=new();mass(m,2.2,1.05,1.2,COLORS['metal'])
    for x in [-1.06,1.06]:mass(m,.08,1.12,.09,COLORS['stone'],.02,x)
    for x in [-.92,-.69,-.46,-.23,0,.23,.46,.69,.92]:
        for z in [-.538,.538]:mass(m,.035,.028,1.08,COLORS['metal'],.06,x,z)
    for x in [-.045,.045]:mass(m,.025,.035,.94,COLORS['stone'],.13,x,-.56)
    result['container']=m
    m=new();mass(m,1.3,.4,.12,COLORS['wood'],.32)
    for x in [-.5,.5]:mass(m,.1,.35,.32,COLORS['metal'],0,x)
    result['bench']=m
    m=new();mass(m,.07,.07,2.8,COLORS['metal']);mass(m,.45,.24,.1,COLORS['glass'],2.7,0,-.16);mass(m,.50,.28,.06,COLORS['metal'],2.80,0,-.16);mass(m,.14,.14,.16,COLORS['stone']);result['lamp']=m
    m=new()
    for x in [-1.6,1.6]:mass(m,.35,.4,6.5,COLORS['metal'],0,x)
    mass(m,8,.35,.35,COLORS['metal'],6.3,1.7)
    mass(m,.08,.08,3,COLORS['metal'],3.3,5.2)
    mass(m,1.2,1.0,.8,COLORS['stone'],5.6,-1.4)
    result['dock-crane']=m
    return result

def textures(destination):
    """Packed shaded luminance/AO, roughness and baked normal XY at three sizes."""
    destination.mkdir(parents=True,exist_ok=True)
    kinds=['brick','plaster','wood','stone','asphalt','concrete','earth','grass','metal','leaf']
    for index,kind in enumerate(kinds):
        size=512;rng=random.Random(624+index);heights=[];base=[]
        for y in range(size):
            for x in range(size):
                u=x*.5;v=y*.5;noise=rng.random()-.5
                broad=math.sin(u*math.tau/256)*math.cos(v*math.tau/256)
                seam=0
                if kind=='brick':seam=float(v%32<2 or (u+(32 if (v//32)%2 else 0))%64<2)
                if kind=='concrete':seam=float(u%128<1 or v%128<1)
                grain=math.sin(u*.28+2*math.sin(v*math.tau/256)) if kind=='wood' else 0
                strata=math.sin(v*.13+math.sin(u*math.tau/128)) if kind=='stone' else 0
                streak=math.sin(u*.37+math.sin(v*.04))*.5+.5
                wear=max(0,broad*.7+streak*.3-.48)
                if kind=='metal':grain=math.sin(v*.5)*.35
                if kind=='leaf':grain=math.sin(u*.2+v*.3)*.45
                heights.append(.5+noise*.12+broad*.06+grain*.075+strata*.04-seam*.24)
                light=.88+noise*.07+broad*.055+grain*.035+strata*.025-seam*.15-wear*.13
                rough=(.52 if kind=='metal' else .82)+noise*.09+seam*.08+wear*.15
                base.append((light,rough))
        pixels=[]
        for y in range(size):
            for x in range(size):
                dx=(heights[y*size+(x+1)%size]-heights[y*size+(x-1)%size])*2.0
                dy=(heights[((y+1)%size)*size+x]-heights[((y-1)%size)*size+x])*2.0
                light,rough=base[y*size+x]
                pixels.append(tuple(round(max(0,min(1,v))*255) for v in [light,rough,.5+dx*.5,.5+dy*.5]))
        image=Image.new('RGBA',(size,size));image.putdata(pixels)
        for resolution in [128,256,512]:
            target=destination if resolution==256 else destination/str(resolution)
            target.mkdir(exist_ok=True)
            image.resize((resolution,resolution),Image.Resampling.LANCZOS).save(target/(kind+'.png'),compress_level=9)

def create(destination):
    destination.mkdir(parents=True,exist_ok=False);(destination/'.gdignore').write_text('')
    (destination/'assets').mkdir();assets=[];hashes={}
    for name,m in library().items():
        data=export(m);path='assets/'+name+'.glb';(destination/path).write_bytes(data)
        assets.append(dict(id='richer-'+name,path=path,attribution=ATTRIBUTION,collision=m.collision,convex_collision=m.convex))
        hashes[path]=hashlib.sha256(data).hexdigest()
    (destination/'library.json').write_text(json.dumps(dict(version=1,assets=assets,sha256=hashes,license='MIT'),indent=2)+'\n')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('destination',type=Path);p.add_argument('--textures',action='store_true');a=p.parse_args()
    textures(a.destination) if a.textures else create(a.destination)
