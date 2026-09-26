#!/usr/bin/env python3
"""Original MIT landmarks for arcade worlds. Geometry only; no game dependency."""
import argparse
import hashlib
import json
import math
from pathlib import Path
from richer_assets import export, COLORS
from world_assets import new, mass, gable, outward

ATTRIBUTION = dict(source='mapkit-arcade-landmarks', license='MIT', notice='Original fictional geometry; no external datasets or images.')

def cylinder(m, radius, height, color, base=0, x=0, z=0, top=None, physical=True):
    top = radius if top is None else top
    vertices = [(x+r*math.cos(i*math.tau/12),base+y,z+r*math.sin(i*math.tau/12)) for y,r in [(0,radius),(height,max(.01,top))] for i in range(12)]
    faces=[]
    for i in range(12):
        j=(i+1)%12;faces.extend([(i,j,j+12),(i,j+12,i+12)])
    for i in range(1,11):faces.extend([(0,i+1,i),(12,12+i,13+i)])
    faces=outward(vertices,faces);m.faces(vertices,faces,color)
    if physical:
        if top != radius:
            # Integer, planar square frustum for the tapered proxy. Independent
            # rounding of twelve-sided taper quads can make them non-convex.
            proxy=[(x+dx*r,base+y,z+dz*r) for y,r in [(0,radius*.71),(height,max(.02,top*.71))] for dx,dz in [(-1,-1),(1,-1),(1,1),(-1,1)]]
            pfaces=[(0,2,1),(0,3,2),(4,5,6),(4,6,7),(0,1,5),(0,5,4),(1,2,6),(1,6,5),(2,3,7),(2,7,6),(3,0,4),(3,4,7)]
            m.convex.append(dict(vertices=[[round(v*100) for v in p] for p in proxy],faces=outward(proxy,pfaces)))
        else:m.convex.append(dict(vertices=[[round(v*100) for v in p] for p in vertices],faces=faces))

def library():
    c=COLORS;result={}
    m=new();mass(m,.44,.44,.06,c['metal']);cylinder(m,.17,.5,(1,.36,.06,1),.06,top=.03);result['cone']=m
    m=new();mass(m,.26,.26,3.3,c['wood'])
    for r,y in [(1.4,.7),(1.1,1.6),(.8,2.4)]:cylinder(m,r,1.8,(.16,.34,.27,1),y,top=0,physical=False)
    result['pine']=m
    m=new();mass(m,9,5,2.8,(.82,.72,.47,1));gable(m,9.4,5.4,2.8,1.3,(.38,.24,.19,1))
    for x in [-3,-1.5,1.5,3]:mass(m,.8,.08,.85,c['glass'],1.05,x,-2.54)
    mass(m,1.2,.10,1.7,c['wood'],0,0,-2.56);mass(m,1.1,.2,.7,(.95,.9,.7,1),3.25,0,-2.5);result['school']=m
    m=new();cylinder(m,2.2,6,c['metal']);cylinder(m,2.3,.15,(.74,.47,.16,1),1);cylinder(m,2.3,.15,(.74,.47,.16,1),4.7);result['tank']=m
    m=new();mass(m,.5,.5,5.4,c['metal'],0,-3);mass(m,.5,.5,5.4,c['metal'],0,3);mass(m,7,.8,.8,c['metal'],5);result['pipe-gantry']=m
    m=new();mass(m,1,1,8,c['metal'],0,-4);mass(m,1,1,8,c['metal'],0,4)
    for i in range(24):
        angle=i*math.tau/24;x,y=7*math.cos(angle),8+7*math.sin(angle)
        mass(m,.8,.45,.8,(.93,.65,.21,1),y-.4,x)
        # Thin radial spokes retain a recognizable wheel silhouette.
        for j in range(1,8):mass(m,.09,.16,.09,c['metal'],8+(y-8)*j/8,x*j/8)
        if i%3==0:mass(m,1.1,.8,.85,[(.78,.2,.22,1),(.2,.63,.72,1),(.95,.65,.12,1)][i//3%3],y-1.1,x)
    result['ferris-wheel']=m
    m=new();cylinder(m,4,.35,(.25,.55,.62,1));cylinder(m,.35,3.8,c['metal'],.35);cylinder(m,4.5,1.3,(.9,.48,.25,1),3.4,top=.15)
    for i in range(8):
        a=i*math.tau/8;x,z=3*math.cos(a),3*math.sin(a);mass(m,.08,.08,3.3,c['metal'],.35,x,z);mass(m,.75,.45,.55,(.96,.76,.35,1),.8,x,z)
    result['carousel']=m
    return result

def create(destination):
    destination.mkdir(parents=True,exist_ok=False);(destination/'.gdignore').write_text('');(destination/'assets').mkdir()
    assets=[];hashes={}
    for name,model in library().items():
        data=export(model);path='assets/'+name+'.glb';(destination/path).write_bytes(data)
        assets.append(dict(id='arcade-'+name,path=path,attribution=ATTRIBUTION,collision=model.collision,convex_collision=model.convex))
        hashes[path]=hashlib.sha256(data).hexdigest()
    (destination/'library.json').write_text(json.dumps(dict(version=1,assets=assets,sha256=hashes,license='MIT'),indent=2)+'\n')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('destination',type=Path);create(p.parse_args().destination)
