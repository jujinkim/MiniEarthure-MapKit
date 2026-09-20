#!/usr/bin/env python3
"""Original MIT district-scale architecture and landscape kit; no map layout."""
import argparse
import hashlib
import json
import math
from pathlib import Path
from world_assets import new, mass, gable, window, append, tree, STONE, WOOD, DARK, CREAM, RED, GREEN, GLASS
from regional_assets import library as miniature_library

ATTRIBUTION = dict(source="mapkit-district-kit",license="MIT",notice="Original district architecture and landscape geometry; no extracted assets or datasets.")


def facade(m,w,d,floors,base=0,color=CREAM,spacing=3.2):
    """Windows on both street fronts and side elevations, with floor bands."""
    for floor in range(floors):
        y=base+1.8+floor*3.2
        for x in range(round(-w/2+2),round(w/2-1),max(2,round(spacing))):
            for side in [-1,1]:
                z=side*(d/2+.04)
                m.solid((x,y,z),(1.45,1.75,.08),GLASS)
                m.solid((x,y-1,z),(1.7,.12,.20),color)
        for z in range(round(-d/2+2),round(d/2-1),max(2,round(spacing))):
            for side in [-1,1]:m.solid((side*(w/2+.04),y,z),(.08,1.75,1.45),GLASS)
        m.solid((0,base+(floor+1)*3.2-.15,0),(w+.10,.20,d+.10),color)


def streetwall(v):
    m=new();units=4+v%2;unit=9;w=units*unit;d=16+v%2*2
    # One continuous parcel contains distinct shop/house bays, not isolated cubes.
    for i in range(units):
        x=(i-(units-1)/2)*unit;floors=3+(i+v)%3
        color=[(.67,.57,.44,1),(.76,.73,.64,1),(.48,.55,.55,1),(.63,.45,.37,1)][(i+v)%4]
        block=new();mass(block,unit,d,4,STONE,-4)
        mass(block,unit,d,floors*3.2,color)
        facade(block,unit,d,floors,color=color)
        mass(block,unit+.15,d+.15,.35,CREAM,floors*3.2)
        block.solid((0,1.5,-d/2-.08),(6,2.8,.12),GLASS)
        block.solid((0,3.05,-d/2-.6),(8,.22,1.3),RED if i%2 else GREEN,True)
        block.solid((0,3.45,-d/2-.16),(7,.5,.14),color)
        block.solid((2,floors*3.2+.8,2),(2,1.0,2),DARK,True)
        # Varied shallow rooftop additions and party-wall rhythm.
        if i%2:mass(block,4,5,2.2,color,floors*3.2,x=-1,z=1)
        append(m,block,x,0)
    return m


def tower(v):
    m=new();w=22+2*(v%3);d=24;floors=20+v*5;h=floors*3.2
    mass(m,60,54,4,STONE,-4);mass(m,60,54,8, (.38,.43,.44,1))
    facade(m,60,54,2,color=(.38,.43,.44,1),spacing=4)
    mass(m,w,d,h,(.26+.025*v,.37+.02*v,.41+.02*v,1),8)
    # Glass bands and vertical mullions define a visibly urban high-rise.
    for f in range(floors):
        y=8+f*3.2+1.5
        for side in [-1,1]:
            m.solid((0,y,side*(d/2+.05)),(w-1.2,2.15,.10),GLASS)
            m.solid((side*(w/2+.05),y,0),(.10,2.15,d-1.2),GLASS)
    for x in range(-int(w/2)+1,int(w/2),4):
        for side in [-1,1]:m.solid((x,8+h/2,side*(d/2+.15)),(.20,h,.20),CREAM)
    for z in range(-10,12,4):
        for side in [-1,1]:m.solid((side*(w/2+.15),8+h/2,z),(.20,h,.20),CREAM)
    mass(m,w-5,d-6,3,DARK,8+h)
    m.solid((0,4,-27.1),(24,5.5,.12),GLASS)
    m.solid((0,6,-28),(16,.4,2.4),CREAM,True)
    return m


def apartment(v):
    m=new();w=28;d=18;floors=6+v*2;color=[(.64,.62,.57,1),(.48,.57,.59,1),(.68,.54,.43,1),(.67,.69,.64,1)][v]
    mass(m,w,d,12,STONE,-12);mass(m,w,d,floors*3.2,color)
    facade(m,w,d,floors,color=color)
    for f in range(1,floors):
        for x in [-8,0,8]:
            y=f*3.2
            m.solid((x,y,-10),(5.5,.20,2.0),CREAM,True)
            m.solid((x,y+.65,-10.9),(5.5,1.1,.14),color)
    mass(m,w+1,d+1,.4,CREAM,floors*3.2)
    mass(m,5,5,3,color,floors*3.2,x=7,z=3)
    m.solid((0,1.5,-9.08),(2.4,3,.10),DARK)
    return m


def warehouse(v):
    m=new();w=38+v*4;d=28;h=9
    mass(m,w,d,3,STONE,-3);mass(m,w,d,h,(.45,.49,.48,1));gable(m,w+1,d+1,h,3,DARK)
    for x in [-12,0,12]:
        m.solid((x,3.2,-d/2-.07),(8,6,.14),(.27,.31,.31,1))
        for y in range(1,6):m.solid((x,y,-d/2-.16),(8,.10,.05),CREAM)
        m.solid((x,6.4,-d/2-1),(9,.25,2.2),DARK,True)
    return m


def workshop(v):
    m=new();mass(m,24,20,3,STONE,-3);mass(m,24,20,5,CREAM);gable(m,25,21,5,1.5,RED)
    for x in [-7,0,7]:m.solid((x,2.3,-10.06),(5.6,4.3,.12),DARK)
    m.solid((0,4.7,-12),(24,.24,4.5),RED,True)
    for x in [-11,11]:m.solid((x,2.2,-13.5),(.3,4.4,.3),STONE,True)
    return m


def library():
    result=miniature_library()
    # A forest canopy spans the spaces between trunks. Three crown heights and
    # leaf colours form layers; physical trunks/roots retain accurate clearance.
    for v in range(3):
        model=tree('canopy');scale=2.5+v*.35;vertical=1.7+v*.25
        groups={}
        for color,(vertices,faces) in model.groups.items():
            tint=tuple(c*(.85+v*.06) for c in color[:3])+(color[3],) if color==GREEN else color
            groups[tint]=[[(x*scale,y*vertical,z*scale) for x,y,z in vertices],faces]
        model.groups=groups
        for c in model.collision:
            c['center']=[round(c['center'][0]*scale),round(c['center'][1]*vertical),round(c['center'][2]*scale)]
            c['size_cm']=[round(c['size_cm'][0]*scale),round(c['size_cm'][1]*vertical),round(c['size_cm'][2]*scale)]
        result['forest-'+str(v)]=model
    for angle in range(0,180,15):
        model=result['rail-'+str(angle)]
        for d in [-.65,.65]:model.solid((d*math.cos(math.radians(angle)),.08,d*math.sin(math.radians(angle))),(.14,.5,.14),STONE,True)
    for v in range(4):
        model=new();w=52+v*4;d=60+(v%2)*16
        ground=[(.39,.43,.16,1),(.53,.49,.22,1),(.36,.44,.22,1),(.52,.41,.22,1)][v]
        model.solid((0,.015,0),(w,.03,d),ground,True)
        for z in range(-d//2+1,d//2,2):model.solid((0,.08,z),(w-.7,.09,.75),tuple(min(1,c*1.12) for c in ground[:3])+(1,))
        result['crop-'+str(v)]=model
    for v in range(4):
        model=new();color=[(.63,.57,.44,1),(.72,.65,.50,1),(.61,.55,.43,1),(.74,.68,.57,1)][v]
        # Stone perimeter wings enclose a shaded inner court; stepped rooftops
        # vary independently from the neighbouring parcel.
        for x,z,w,d,floors in [(-8,0,6,20,2+v%2),(7,0,8,20,3),(0,7,10,6,2+(v+1)%2)]:
            wing=new();mass(wing,w,d,4,STONE,-4);mass(wing,w,d,floors*3.2,color);facade(wing,w,d,floors,color=color)
            mass(wing,w+.25,d+.25,.3,CREAM,floors*3.2);append(model,wing,x,z)
        model.solid((0,3,-7),(10,.2,4),WOOD,True)
        result['courtyard-block-'+str(v)]=model
    for size in [4,8]:
        model=new();model.solid((0,.03,0),(size-.04,.06,size-.04),(.12,.42,.48,1),True);result['water-'+str(size)]=model
    for style,build in [('streetwall',streetwall),('civic-tower',tower),('apartment',apartment),('dock-shed',warehouse),('workshop',workshop)]:
        for v in range(4):result[f'{style}-{v}']=build(v)
    for name,color,size in [('paved-plinth',STONE,(40,.12,32)),('seawall',STONE,(15.9,4,3)),('container',(.48,.22,.15,1),(12.2,2.6,2.45)),('bollard',DARK,(.3,.8,.3))]:
        m=new();m.solid((0,size[1]/2,0),size,color,True);result[name]=m
    m=new();m.solid((0,3.8,0),(.15,7.6,.15),DARK,True);m.solid((0,7.6,-1.2),(.18,.18,2.6),DARK);m.solid((0,7.5,-2.3),(.6,.16,.8),CREAM);result['street-lamp']=m
    m=new();m.solid((0,.8,0),(2,.18,.65),WOOD,True)
    for x in [-.7,.7]:m.solid((x,.4,0),(.12,.8,.5),DARK,True)
    m.solid((0,1.15,.3),(2,.55,.12),WOOD,True);result['bench']=m
    return result


def create(destination):
    destination=Path(destination);destination.mkdir(parents=True,exist_ok=False);(destination/'.gdignore').write_text('')
    assets=[];hashes={}
    for name,model in library().items():
        path='assets/'+name+'.glb';data=model.export();target=destination/path;target.parent.mkdir(exist_ok=True);target.write_bytes(data)
        assets.append(dict(id='district-'+name,path=path,attribution=ATTRIBUTION,collision=model.collision,convex_collision=model.convex))
        hashes[path]=hashlib.sha256(data).hexdigest()
    (destination/'library.json').write_text(json.dumps(dict(version=1,assets=assets,sha256=hashes,license='MIT'),indent=2)+'\n')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('destination',type=Path);create(p.parse_args().destination)
