#!/usr/bin/env python3
"""Original MIT miniature kit. Reusable geometry only; no regional layout."""
import argparse
import hashlib
import json
import math
from pathlib import Path
from world_geometry import box, oriented_faces, cm
from world_assets import outward, courtyard, new, mass, gable, window, tree, landform, STONE, WOOD, DARK, CREAM, RED, GREEN, SNOW, SAND

ATTRIBUTION = dict(source="mapkit-regional-miniatures", license="MIT", notice="Original authored modular geometry. No external datasets, fonts or extracted game assets.")


def building(style, variant):
    m = new()
    w, d = 7 + variant % 3, 6 + variant % 2 * 2
    floors = (5 + variant * 2) if style == "tower" else 1 + variant % 3
    h = floors * 2.6
    colors = {"tower":(.33,.43,.47,1), "shop":(.76,.63,.49,1), "stone":STONE,
              "farm":(.78,.72,.60,1), "polar":[(.65,.21,.14,1),(.79,.60,.22,1),(.24,.40,.53,1),(.35,.48,.35,1)][variant%4],
              "stilt":(.66,.54,.35,1), "warehouse":(.42,.46,.48,1)}
    base = 1.8 if style == "stilt" else .4
    if style == "stilt":
        for x in [-w/2+.5,w/2-.5]:
            for z in [-d/2+.5,d/2-.5]:m.solid((x,base/2,z),(.4,base,.4),WOOD,True)
    else:mass(m,w+.4,d+.4,3.4,STONE,-3)
    mass(m,w,d,h,colors[style],base)
    if style in ["farm","polar","stilt","warehouse"]:
        gable(m,w+.8,d+.9,base+h,1.4,SNOW if style=="polar" else RED if style in ["farm","stilt"] else DARK)
    else:
        m.solid((0,base+h+.15,0),(w+.2,.3,d+.2),CREAM,True)
        m.solid((1,base+h+.7,1),(2,1.1,1.8),DARK,True)
        if variant%2:m.solid((-2,base+h+.55,-1),(1.4,.8,1.4),STONE,True)
    for floor in range(floors):
        for x in [-w*.3,0,w*.3]:window(m,x,base+1.5+floor*2.6,-d/2-.05,1.2,1.3)
        for side in [-1,1]:
            for z in [-d*.28,d*.28]:m.solid((side*(w/2+.035),base+1.5+floor*2.6,z),(.07,1.25,1.2),DARK)
    m.solid((0,base+1,-d/2-.09),(1.2,2,.08),WOOD)
    if style in ["shop","stone","stilt"]:
        m.solid((0,base+2.3,-d/2-.7),(w,.16,1.5),RED if variant%2 else GREEN,True)
        m.solid((0,.2,-d/2-.55),(2,.4,1.1),STONE,True)
    if style=="stone":
        mass(m,2,3,1.8,STONE,x=w/2+.5,z=1)
    return m


def butte(variant=0):
    """Layered, tapered sandstone pillars with exact closed convex proxies."""
    m=new();w=6+variant;d=4+variant;h=13+variant*3
    ring=[(w*x,d*z) for x,z in [(1,0),(.75,.75),(0,1),(-.75,.75),(-1,0),(-.75,-.75),(0,-1),(.75,-.75)]]
    for bottom,top,scale0,scale1,color in [(0,h*.45,1,.88,(.49,.24,.15,1)),(h*.45,h*.75,.88,.80,(.66,.35,.20,1)),(h*.75,h,.80,.64,(.72,.44,.27,1))]:
        vertices=[(x*scale0,bottom,z*scale0) for x,z in ring]+[(x*scale1,top,z*scale1) for x,z in ring]
        faces=[]
        for j in range(1,7):faces.extend([(0,j+1,j),(8,8+j,8+j+1)])
        for j in range(8):
            k=(j+1)%8;faces.extend([(j,k,8+k),(j,8+k,8+j)])
        faces=outward(vertices,faces);m.faces(vertices,faces,color);m.convex.append(dict(vertices=[cm(v) for v in vertices],faces=faces))
    return m


def landmark(name):
    m=new()
    if name=="windmill":
        mass(m,4,4,8,CREAM);gable(m,5,5,8,2,RED)
        for x in [-1,1]:window(m,x,3,-2.04,.6,1)
        # Broad timber sails read at miniature overview and street scale.
        m.solid((0,7,-2.6),(.3,8,.18),WOOD,True);m.solid((0,7,-2.6),(8,.3,.18),WOOD,True)
        for x,y,w,h in [(-2,7,3,.9),(2,7,3,.9),(0,5,.9,3),(0,9,.9,3)]:m.solid((x,y,-2.68),(w,h,.09),CREAM)
    elif name=="watermill":
        m=building("farm",0)
        for n in range(12):
            a=n*math.tau/12;m.solid((4.6,1.5+1.3*math.sin(a),1.3*math.cos(a)),(.65,.3,.3),WOOD,True)
        m.solid((4.6,1.5,0),(.5,2.2,.18),WOOD,True)
    elif name=="waterfall":
        mass(m,9,5,6,STONE)
        m.solid((0,3,-2.6),(2.6,6,.12),(.28,.66,.71,1),True)
        m.solid((0,.12,-4),(5,.24,3),(.34,.72,.74,1),True)
    return m


def library():
    result = {f"{style}-{v}":building(style,v) for style in ["tower","shop","stone","farm","polar","stilt","warehouse"] for v in range(4)}
    result.update({"canopy":tree("canopy"),"palm":tree("palm"),"rock":landform(STONE,10,6,8),"sandstone":butte(0),"dune":landform(SAND,14,3,9),"snow":landform(SNOW,12,2,8)})
    for name,color,size in [("water",(.12,.42,.48,1),(15.96,.06,15.96)),("field",(.47,.51,.21,1),(14,.06,12)),("rice",(.42,.56,.23,1),(14,.06,12)),("quay",STONE,(12,1.2,5)),("wall",STONE,(8,2,.6)),("pier",STONE,(1.4,8,1.4))]:
        m=new();m.solid((0,size[1]/2,0),size,color,True);result[name]=m
    m=new()
    mass(m,5,5,12,STONE)
    gable(m,6,6,12,2,RED)
    for x in [-1,1]:window(m,x,10,-2.54,.7,1)
    result["bell-tower"]=m
    m=new();mass(m,9,9,3,CREAM);mass(m,4,4,8,CREAM,3);m.solid((0,11.5,0),(6,1,6),DARK,True);result["observatory"]=m
    m=new();mass(m,2,2,3,DARK)
    m.solid((0,13,0),(1,23,1),RED,True);m.solid((6,24,0),(14,.7,.7),RED,True)
    m.solid((12,18,0),(.12,12,.12),DARK);result["crane"]=m
    m=new();mass(m,10,6,3,CREAM);m.solid((0,4,0),(12,.4,8),RED,True);result["service-canopy"]=m
    result["courtyard"]=courtyard()
    for v in range(1,4):result["sandstone-"+str(v)]=butte(v)
    for name in ["windmill","watermill","waterfall"]:result[name]=landmark(name)
    for step in range(1,41):
        m=new();height=step*.5
        m.solid((0,height/2,0),(.7,height,.7),STONE,True)
        result["support-"+str(step)]=m
    for angle in range(0,180,15):
        m=new();r=math.radians(angle)
        vertices=[(x*math.cos(r)-z*math.sin(r),y,x*math.sin(r)+z*math.cos(r)) for x,y,z in box((0,.45,0),(2.2,.65,.16))]
        faces=oriented_faces(vertices);m.faces(vertices,faces,CREAM)
        m.convex.append(dict(vertices=[cm(v) for v in vertices],faces=faces))
        result["rail-"+str(angle)]=m
    m=new()
    for x in [-1.25,1.25]:m.solid((x,.9,0),(.14,1.8,.14),DARK,True)
    result["sign-post"]=m
    return result


def create(destination):
    destination=Path(destination);destination.mkdir(parents=True,exist_ok=False)
    (destination/".gdignore").write_text("")
    records=[]; hashes={}
    for name,model in library().items():
        path="assets/"+name+".glb";data=model.export()
        (destination/path).parent.mkdir(exist_ok=True);(destination/path).write_bytes(data)
        records.append(dict(id="regional-"+name,path=path,attribution=ATTRIBUTION,collision=model.collision,convex_collision=model.convex))
        hashes[path]=hashlib.sha256(data).hexdigest()
    catalog=dict(version=1,license="MIT",assets=records,sha256=hashes)
    (destination/"library.json").write_text(json.dumps(catalog,indent=2)+"\n")
    return catalog

if __name__=="__main__":
    p=argparse.ArgumentParser(description=__doc__);p.add_argument("destination",type=Path);create(p.parse_args().destination)
