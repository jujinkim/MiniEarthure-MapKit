"""Original MIT alpine firs, exposed rock, snow and mountain structures, metres."""
import math
from authored_assets import Mesh,beam,crown,gable,window,door,assemble_library,PALETTE,STONE,DARK,WOOD,GLASS,TRIM
SNOW=(.87,.90,.89,1); ICE=(.49,.67,.71,1); GRANITE=(.39,.43,.46,1); NEEDLE=(.15,.25,.23,1)
PALETTE.update({SNOW:'stone',ICE:'glass',GRANITE:'stone',NEEDLE:'leaf'})

def fir(variant=0,distant=False):
    m=Mesh(distant);h=[14,21,25][variant];sides=8 if distant else 12
    beam(m,(0,-.4,0),(.12,h,0),.32,WOOD,6 if distant else 10,.065)
    for row in range(6):
        y=h*.21+row*h*.115;radius=(4.3-row*.52)*(1+variant*.09)
        vs=[(math.cos(a)*radius,y+math.sin(a*3+row)*.22,math.sin(a)*radius) for a in [i*math.tau/sides for i in range(sides)]]
        vs.append((.12,y+3.9,0));m.hull(vs,[(i,(i+1)%sides,sides) for i in range(sides)],NEEDLE)
        if variant>0 or row%2==0:
            r=radius*.84;v=[(math.cos(a)*r,y+.65+math.sin(a*3+row)*.15,math.sin(a)*r) for a in [i*math.tau/sides for i in range(sides)]]
            v.append((.12,y+4.03,0));m.hull(v,[(i,(i+1)%sides,sides) for i in range(sides)],SNOW)
        if not distant:
            for i in range(4):
                a=i*math.pi/2+row*.6;beam(m,(0,y-.3,0),(math.cos(a)*radius*.9,y+.2,math.sin(a)*radius*.9),.06,WOOD,4,.02)
    m.collision=[dict(center=[0,round(h*40),0],size_cm=[54,round(h*82),54])]
    return m

def crag(variant=0,distant=False):
    m=Mesh(distant);w,h,d=[(25,20,17),(17,29,15),(12,9,10)][variant]
    crown(m,(0,h*.38,0),(w,h,d),521+variant,GRANITE)
    crown(m,(-w*.06,h*.73,-d*.06),(w*.68,h*.30,d*.70),518+variant,SNOW)
    m.collision=[dict(center=[0,round(h*30),0],size_cm=[round(w*68),round(h*68),round(d*66)])]
    return m

def lodge(variant=0,distant=False):
    m=Mesh(distant);w,d,h=[(17,11,6.2),(10,8,3.8)][variant]
    m.solid((0,-.25,0),(w+.5,.9,d+.5),STONE,True);m.solid((0,h/2,0),(w,h,d),WOOD,True)
    gable(m,w+1.8,d+2,h,4.2 if variant==0 else 3.2,SNOW)
    for x in (-w*.3,w*.3):
        window(m,x,2,-d/2-.06,2.3,1.6,True)
        if not variant:window(m,x,4.7,-d/2-.06,2.3,1.5)
    door(m,0,-d/2-.1,1.4,2.5)
    m.solid((0,.2,-d/2-1.8),(w,.4,3.5),WOOD,True)
    for x in (-w*.44,w*.44):m.solid((x,1.8,-d/2-2.4),(.24,3.6,.24),WOOD,True)
    if not variant:
        m.solid((0,3.4,-d/2-1.3),(w,.2,2.8),WOOD,True)
        for x in (-w*.46,w*.46):m.solid((x,3.95,-d/2-2.6),(.16,1.1,.16),WOOD)
        beam(m,(-w*.46,4.5,-d/2-2.6),(w*.46,4.5,-d/2-2.6),.08,WOOD,6)
    m.solid((w*.27,h+2,d*.2),(.8,4,1),STONE,True)
    for side in (-1,1):
        for z in (-d*.24,d*.24):m.solid((side*(w/2+.05),2,z),(.12,1.5,1.4),GLASS)
    if not distant:
        for y in range(int(h/.3)):m.solid((0,y*.3,-d/2-.03),(w,.06,.08),DARK)
    return m

def bank(distant=False):
    m=Mesh(distant);crown(m,(0,.65,0),(10,2.5,3),143,SNOW)
    m.collision=[dict(center=[0,30,0],size_cm=[900,60,240])];return m

def rail(distant=False):
    m=Mesh(distant)
    for x in (-8,0,8):m.solid((x,.35,0),(.18,1.8,.18),DARK,True)
    for y in (.65,1.0):m.solid((0,y,0),(18,.16,.13),STONE,True)
    return m

def gallery(distant=False):
    m=Mesh(distant)
    for x in (-6.6,6.6):
        for z in (-15,-5,5,15):m.solid((x,3,z),(.65,7,.65),STONE,True)
    m.solid((0,6.5,0),(15,.5,34),STONE,True)
    # Open travel bore, with snow held above it by a solid canopy.
    m.solid((0,6.85,0),(14.8,.2,33.8),SNOW)
    if not distant:
        for z in range(-15,16,5):m.solid((0,6.1,z),(14,.25,.28),DARK)
    return m

def frozen_lake(distant=False):
    m=Mesh(distant);sides=12 if distant else 24
    bottom=[(44*math.cos(i*math.tau/sides),-1.2,29*math.sin(i*math.tau/sides)) for i in range(sides)]
    top=[(x,0,z) for x,_,z in bottom];vs=bottom+top;faces=[]
    for i in range(sides):j=(i+1)%sides;faces.extend([(i,j,j+sides),(i,j+sides,i+sides)])
    for i in range(1,sides-1):faces.extend([(0,i+1,i),(sides,sides+i,sides+i+1)])
    m.hull(vs,faces,ICE,distant)
    if not distant:m.convex=frozen_lake(True).convex
    if not distant:
        for a,b in [((-32,.025,-7),(12,.025,9)),((12,.025,9),(30,.025,-7)),((-6,.026,-23),(1,.026,18))]:beam(m,a,b,.055,TRIM,4)
    return m

def library():
    return assemble_library({**{'snow-fir-'+str(i):lambda far,i=i:fir(i,far) for i in range(3)},
        **{'alpine-crag-'+str(i):lambda far,i=i:crag(i,far) for i in range(3)},
        **{'alpine-lodge-'+str(i):lambda far,i=i:lodge(i,far) for i in range(2)},
        'snowbank':bank,'alpine-rail':rail,'snow-gallery':gallery,'frozen-lake':frozen_lake},'snow_assets.py')
