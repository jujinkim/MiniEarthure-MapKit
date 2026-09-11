#!/usr/bin/env python3
"""Original MIT, language-neutral world props. Writes only a NEW library folder.

Profiles belong to the map author. No place/locale is implied by an asset ID.
Source models have explicit solid proxies and no lettering, fonts or textures.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
from world_geometry import Model, GLASS, cm

STONE=(.72,.67,.54,1); WOOD=(.35,.21,.12,1); DARK=(.14,.19,.21,1)
CREAM=(.89,.84,.69,1); RED=(.60,.22,.12,1); GREEN=(.18,.36,.18,1)
SNOW=(.89,.95,.96,1); ICE=(.39,.70,.79,1); SAND=(.78,.60,.34,1)
ATTRIBUTION=dict(source='mapkit-world-library-v1',license='MIT',notice=
    'Original procedural miniature structures, plants, landforms and blank signboard. No surveyed data, extracted models, letters, font or image dependencies.')

def outward(vertices,faces):
    center=[sum(p[a] for p in vertices)/len(vertices) for a in range(3)]
    result=[]
    for face in faces:
        a,b,c=[vertices[i] for i in face]
        u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)]
        n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
        result.append(face if sum(n[i]*(a[i]-center[i]) for i in range(3))>0 else tuple(reversed(face)))
    return result

def append(target,source,x,z):
    import copy
    for color,(vs,fs) in source.groups.items():target.faces([(a+x,h,b+z) for a,h,b in vs],fs,color)
    for proxy in source.collision:
        item=copy.deepcopy(proxy);item['center'][0]+=round(x*100);item['center'][2]+=round(z*100);target.collision.append(item)
    for proxy in source.convex:
        item=copy.deepcopy(proxy)
        for v in item['vertices']:v[0]+=round(x*100);v[2]+=round(z*100)
        target.convex.append(item)


def window(m,x,h,z,w=1,hgt=1):
    m.solid((x,h,z),(w+.12,hgt+.12,.04),DARK)
    m.solid((x,h,z-.03),(w,hgt,.025),GLASS)
    m.solid((x,h,z-.055),(.05,hgt,.02),CREAM)
    m.solid((x,h-hgt/2-.10,z-.08),(w+.25,.09,.18),STONE)

def mass(m,w,d,h,color,base=0,x=0,z=0):
    m.solid((x,base+h/2,z),(w,h,d),color,True)

def gable(m,w,d,base,rise,color):
    # A closed triangular prism. Exact convex roof proxy, not an empty AABB.
    vs=[(-w/2,base,-d/2),(w/2,base,-d/2),(0,base+rise,-d/2),
        (-w/2,base,d/2),(w/2,base,d/2),(0,base+rise,d/2)]
    fs=[(0,2,1),(3,4,5),(0,1,4),(0,4,3),(1,2,5),(1,5,4),(2,0,3),(2,3,5)]
    fs=outward(vs,fs)
    m.faces(vs,fs,color)
    m.convex.append(dict(vertices=cm_vertices(vs),faces=fs))

def cm_vertices(vs): return [cm(v) for v in vs]

def cabin(snow=True):
    m=new();mass(m,7,6,3,RED,.6)
    for x in [-2.7,2.7]:
        for z in [-2.3,2.3]:m.solid((x,.3,z),(.35,.6,.35),DARK,True)
    gable(m,7.5,6.6,3.6,1.25,SNOW if snow else DARK)
    for x in [-2,0,2]:window(m,x,2.1,-3.03,1.15,.9)
    for h in [1,1.4,2.8,3.2]:m.solid((0,h,-3.01),(7,.025,.025),CREAM)
    m.solid((2.4,4.1,1.5),(.5,1.1,.5),DARK,True)
    return m

def tower():
    m=new();mass(m,9,9,3,STONE);mass(m,6.5,6.5,16,GLASS,3)
    for h in range(4,19,2):
        for z in [-3.3,3.3]:m.solid((0,h,z),(6.8,.12,.13),CREAM)
        for x in [-3.3,3.3]:m.solid((x,h,0),(.13,.12,6.8),CREAM)
    for x in [-2,0,2]:
        m.solid((x,11,-3.32),(.065,16,.08),DARK)
        window(m,x,1.4,-4.54,1.6,1.9)
    mass(m,3,3,1,DARK,19)
    m.solid((0,2.8,-5),(9,.18,1),DARK,True)
    return m

def barn():
    m=new();mass(m,8,8,3.3,RED);gable(m,8.8,8.6,3.3,2.5,WOOD)
    m.solid((0,1.3,-4.04),(3.1,2.6,.07),WOOD)
    for x in [-1.5,0,1.5]:m.solid((x,1.3,-4.09),(.1,2.6,.025),CREAM)
    for x in [-3.7,3.7]:m.solid((x,1.65,-4.04),(.18,3.3,.08),CREAM)
    window(m,0,4.0,-4.32,1,.8)
    return m

def courtyard():
    m=new()
    # U-shaped compound; open court and central passage remain traversable.
    for x in [-3.3,3.3]:mass(m,2.4,8,4.3,STONE,x=x)
    mass(m,4.2,2,4.3,STONE,z=3)
    for x in [-3.3,3.3]:
        m.solid((x,4.4,0),(2.5,.2,8.1),CREAM)
        for z in [-3,-1,1]:
            m.solid((x,4.65,z),(.7,.3,.7),CREAM)
        window(m,x,2.0,-4.05,.85,1.4)
    # Lightweight lattice canopy over the open court, solid at roof level only.
    for z in [-2.5,-1.8,-1.1,-.4]:m.solid((0,3.5,z),(4.3,.12,.12),WOOD,True)
    return m

def veranda():
    m=new();mass(m,7,5,4.2,CREAM,.7)
    for x in [-3,3]:
        for z in [-2,2]:m.solid((x,.35,z),(.3,.7,.3),WOOD,True)
    m.solid((0,.7,-3.1),(7.8,.16,1.4),WOOD,True)
    for x in [-3.6,3.6]:m.solid((x,2.4,-3.5),(.16,3.4,.16),WOOD,True)
    gable(m,8.4,8,4.9,1.6,RED)
    for x in [-2,0,2]:
        window(m,x,2.1,-2.54,1.4,1.3)
        for dx in [-.78,.78]:m.solid((x+dx,2.1,-2.6),(.15,1.6,.13),GREEN)
    for h in [3.6,4.4]:m.solid((0,h,-2.55),(7,.08,.06),WOOD)
    return m

def new():
    m=Model();m.convex=[];return m

def landform(color,w=8,h=2,d=6):
    m=new()
    gable(m,w,d,0,h,color)
    return m


def tree(kind):
    m=new()
    h=8 if kind=='canopy' else 5
    m.solid((0,h/2,0),(.45,h,.45),WOOD,True)
    if kind=='palm':
        for n in range(8):
            a=n*math.tau/8
            vertices=[(0,5,0),(math.cos(a-.25)*1.5,5.5,math.sin(a-.25)*1.5),
                (math.cos(a)*3,4.4,math.sin(a)*3),(math.cos(a+.25)*1.5,5.5,math.sin(a+.25)*1.5)]
            m.panel(vertices,(0,1,0),GREEN)
            m.panel(list(reversed(vertices)),(0,-1,0),GREEN)
    else:
        # Faceted multi-layer crowns; roots/twigs have actual solids.
        for x,z,hh,w in [(-1.3,0,h,3.4),(1.4,.8,h+.9,3.5),(0,-1,h+1.8,3.8)]:
            m.solid((x,hh,z),(w,1.5,w),GREEN)
        for x,z in [(-.7,0),(.7,0),(0,.7)]:m.solid((x,.15,z),(1.4,.3,.3),WOOD,True)
    return m

def library():
    models={'polar-cabin':cabin(),'utility-cabin':cabin(False),'podium-tower':tower(),'gable-barn':barn(),
        'courtyard-house':courtyard(),'deep-veranda':veranda(),
        'snow-drift':landform(SNOW,7,1.2,4),'ice-ridge':landform(ICE,5,2.8,3),
        'sand-dune':landform(SAND,11,2.8,7),'rock-outcrop':landform(STONE,5,2.2,4),
        'canopy-tree':tree('canopy'),'palm':tree('palm')}
    for name,color in [('snow',SNOW),('sand',SAND),('earth',(.26,.22,.12,1)),('field',(.47,.51,.21,1))]:
        m=new();m.solid((0,-.025,0),(16,.05,16),color,True);models['ground-'+name]=m
    m=new()
    for x in [-1.8,0,1.8]:m.solid((x,.65,0),(.13,1.3,.13),WOOD,True)
    for h in [.4,1]:m.solid((0,h,0),(3.8,.13,.13),WOOD,True)
    models['farm-fence']=m
    m=new()
    for i in range(7):m.solid((-1.2+i*.4,.24,0),(.12,.48,2.5),(.62,.59,.23,1))
    m.solid((0,.025,0),(2.8,.05,2.6),(.36,.28,.16,1),True)
    models['crop-row']=m
    m=new()
    for x in [-.5,0,.5]:
        for z in [-.5,0,.5]:m.solid((x,.35+abs(x),z),(.4,.7,.35),GREEN)
    m.solid((0,.025,0),(1.5,.05,1.5),(.26,.22,.12,1),True)
    models['undergrowth']=m
    m=new();m.solid((0,.8,0),(.6,1.6,.6),GREEN,True)
    for x in [-.55,.55]:
        m.solid((x,.95,0),(.55,.25,.25),GREEN,True)
        m.solid((x,1.2,0),(.25,.7,.25),GREEN,True)
    models['arid-shrub']=m
    m=new()
    for x in [-2.5,2.5]:m.solid((x,1.6,0),(.16,3.2,.16),WOOD,True)
    m.solid((0,3.15,0),(5.4,.2,3),CREAM,True)
    for x in [-1.5,0,1.5]:m.solid((x,.6,0),(1,.12,.4),WOOD,True)
    models['shade-shelter']=m
    m=new();m.solid((0,.07,0),(.55,.14,3.4),DARK,True)
    for i in range(10):m.solid((0,.145,-1.45+i*.32),(.5,.015,.06),STONE)
    models['drain-channel']=m
    m=new();m.solid((0,1.6,0),(.12,3.2,.12),DARK,True)
    m.solid((0,3.2,-.45),(.18,.12,1),DARK,True)
    m.solid((0,3.12,-.8),(.42,.08,.5),CREAM)
    models['street-lamp']=m
    m=new();m.solid((0,.35,0),(1.8,.18,.5),WOOD,True)
    for x in [-.7,.7]:m.solid((x,.15,0),(.1,.3,.4),DARK,True)
    models['bench']=m
    m=new();m.solid((0,.65,0),(3.2,1.3,.10),DARK,True)
    # Blank surface; map-specific writing is a separate asset, no alphabet glyphs.
    m.solid((0,.65,-.06),(3,1.1,.02),CREAM)
    models['blank-signboard']=m
    # Composed landscaped plots avoid overlapping placement footprints. Their
    # ground, crowns and exact obstacle proxies belong to one reusable asset.
    for name,color,props in [
        ('polar',SNOW,[('snow-drift',-3,0),('ice-ridge',4,2)]),
        ('arid',SAND,[('sand-dune',-1,1),('arid-shrub',6,-4)]),
        ('farm',(.47,.51,.21,1),[('crop-row',x,z) for x in [-5,0,5] for z in [-3,2]]),
        ('garden',(.47,.51,.21,1),[('canopy-tree',0,1)]),
        ('humid',(.26,.22,.12,1),[('canopy-tree',-4,-2),('canopy-tree',4,2),('undergrowth',0,0),('undergrowth',5,-3)]),
        ('tropical',(.37,.40,.19,1),[('palm',-4,1),('palm',4,-1),('undergrowth',0,3)])]:
        m=new();m.solid((0,0,0),(15.96,.04,13.96),color,True)
        for asset,x,z in props: append(m,models[asset],x,z)
        models['plot-'+name]=m
    return models

def create(destination):
    destination=Path(destination);destination.mkdir(parents=True,exist_ok=False)
    (destination/'.gdignore').write_text('')
    records=[]
    for name,m in library().items():
        data=m.export();path='assets/'+name+'.glb'
        (destination/path).parent.mkdir(exist_ok=True)
        (destination/path).write_bytes(data)
        records.append(dict(id='world-'+name,path=path,attribution=ATTRIBUTION,
            collision=m.collision,convex_collision=m.convex))
    catalog=dict(version=1,assets=records,sha256={a['path']:hashlib.sha256((destination/a['path']).read_bytes()).hexdigest() for a in records},
        license='MIT',sign_surface=dict(width_cm=300,height_cm=110,center_cm=[0,65,-8],normal=[0,0,-1]))
    (destination/'library.json').write_text(json.dumps(catalog,indent=2)+'\n')
    return catalog

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('destination',type=Path)
    create(p.parse_args().destination)
