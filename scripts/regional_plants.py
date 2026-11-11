#!/usr/bin/env python3
"""Original MIT regional plant templates, shared verbatim by native/far and Godot.
Coordinates are scene metres, clockwise triangle winding. Run from any directory.
"""
import json, math
from pathlib import Path

def template(kind, variant):
    faces=[]
    def tri(a,b,c,leaf=True):
        faces.append({"vertices":[[round(v,6) for v in p] for p in (a,b,c)],"leaf":leaf})
    def ring(radius,y,n=8):
        return [(radius*math.cos(i*math.tau/n),y,radius*math.sin(i*math.tau/n)) for i in range(n)]
    def band(a,b,leaf=True):
        for i in range(len(a)):
            j=(i+1)%len(a)
            if len(b)==1: tri(a[i],a[j],b[0],leaf)
            else:
                tri(a[i],b[j],b[i],leaf)
                tri(a[i],a[j],b[j],leaf)
    tree=kind in ('conifer','palm','orchard')
    height=(1.0,1.07,0.95)[variant]
    if tree:
        band(ring(.19,-.1),ring(.15,1.8),False)
        band(ring(.15,1.8),ring(.07,(3.3 if kind=='palm' else 2.8)*height),False)
    if kind=='conifer':
        for y,r,h in ((1.35,.86,1.65),(2.05,.68,1.5),(2.8,.43,1.2)):
            band(ring(r,y*height),[(0,(y+h)*height,0)])
    elif kind=='palm':
        centre=(0,3.6*height,0)
        for i in range(10):
            a=i*math.tau/10+variant*.23
            def point(r,y,side=0):
                return (r*math.cos(a)-side*math.sin(a),y*height,r*math.sin(a)+side*math.cos(a))
            mid=point(.48,3.85);tip=point(.88,3.2)
            left=point(.5,3.7,.18);right=point(.5,3.7,-.18)
            # Folded fronds: two ridges, two lower faces. Opaque silhouette on every tier.
            tri(centre,mid,left);tri(centre,right,mid);tri(mid,tip,left);tri(mid,right,tip)
            tri(centre,left,right);tri(left,tip,right)
    else:
        r=.86 if tree else .30 if kind=='scrub' else .25
        base=1.7 if tree else -.03
        h=1.35 if tree else .58 if kind=='scrub' else .36
        band(ring(r*.45,base),ring(r,base+h*.45))
        band(ring(r,base+h*.45),ring(r*.55,base+h*.85))
        band(ring(r*.55,base+h*.85),[(0,base+h,0)])
    return faces

def generate():
    colors={'conifer':[55,91,67],'palm':[92,126,63],'orchard':[101,132,66],'scrub':[113,129,79],'polar':[126,136,108]}
    data={k:{'color':c,'variants':[template(k,v) for v in range(3)]} for k,c in colors.items()}
    target=Path(__file__).resolve().parents[1]/'godot/regional_plants.json'
    target.write_text(json.dumps(data,separators=(',',':'))+'\n')
if __name__=='__main__': generate()
