"""Original MIT production halls, process equipment and service structures."""
import math
from authored_assets import Mesh,beam,gable,window,door,assemble_library,PALETTE,STONE,DARK,GLASS,TRIM,RUST
STEEL=(.38,.47,.49,1); PAINT=(.67,.69,.64,1); SAFETY=(.86,.59,.20,1)
PALETTE.update({STEEL:'metal',PAINT:'metal',SAFETY:'metal'})

def hall(variant=0,distant=False):
    m=Mesh(distant);w,d,h=[(60,36,13),(48,52,18),(72,40,11)][variant]
    m.solid((0,-.3,0),(w+1,1.2,d+1),STONE,True)
    m.solid((0,h/2,0),(w,h,d),PAINT if variant!=1 else STEEL,True)
    for x in (-w/3,0,w/3):gable(m,w/3+.2,d+1,h,3.2,STEEL,x=x)
    for x in (-w*.32,0,w*.32):
        m.solid((x,3,-d/2-.11),(8,6,.18),DARK)
        for dx in (-4.3,4.3):m.solid((x+dx,3,-d/2-.2),(.25,6.4,.3),SAFETY)
        m.solid((x,7.3,-d/2-3),(10,.3,6),STEEL,True)
        for dx in (-4.6,4.6):m.solid((x+dx,3.5,-d/2-5.7),(.22,7,.22),STEEL,True)
    for x in range(-int(w/2)+4,int(w/2)-2,6):
        m.solid((x,h-2.3,-d/2-.06),(4.4,1.8,.12),GLASS)
        m.solid((x,h-2.3,d/2+.06),(4.4,1.8,.12),GLASS)
    door(m,w*.41,d/2+.16)
    for x in (-w*.28,w*.28):m.solid((x,h+2.6,d*.24),(5,2.0,7),DARK,True)
    if not distant:
        for x in range(-int(w/2)+2,int(w/2),4):m.solid((x,h/2,-d/2-.06),(.18,h,.15),STEEL)
        for side in (-1,1):
            for z in range(-int(d/2)+3,int(d/2),5):m.solid((side*(w/2+.06),h*.7,z),(.12,2,3.7),GLASS)
        for i in range(12):m.solid((w/2+1.1,.18+i*.36,d*.22-i*.4),(2.1,.36,.55),STONE,True)
    return m

def tank(variant=0,distant=False):
    m=Mesh(distant);r,h=[(10,17),(6.5,27),(8,12)][variant];sides=12 if distant else 28
    beam(m,(0,-.4,0),(0,h,0),r,PAINT,sides)
    beam(m,(0,h,0),(0,h+1.2,0),r,STEEL,sides,r*.86)
    m.collision=[dict(center=[0,round(h*50),0],size_cm=[round(r*180),round(h*100),round(r*180)])]
    for y in (h*.32,h*.67):beam(m,(0,y-.07,0),(0,y+.07,0),r+.10,STEEL,sides)
    beam(m,(0,h+1.2,0),(0,h+2.7,0),.8,DARK,8)
    for x in (-.65,.65):beam(m,(x,0,-r-.3),(x,h,-r-.3),.07,DARK,6)
    if not distant:
        for i in range(int(h/.4)):beam(m,(-.65,i*.4,-r-.3),(.65,i*.4,-r-.3),.05,DARK,4)
    return m

def boiler(distant=False):
    m=Mesh(distant);m.solid((0,4,0),(16,8,13),STEEL,True)
    for x in (-5,5):
        beam(m,(x,7,0),(x,31,0),2.5,PAINT,10 if distant else 24)
        m.collision.append(dict(center=[round(x*100),1850,0],size_cm=[460,2500,460]))
        beam(m,(x,9,-3),(x,29,-3),.28,RUST,6 if distant else 12)
    for y in (13,22):
        m.solid((0,y,0),(18,.2,8),DARK,True)
        for z in (-3.7,3.7):beam(m,(-8.5,y+1,z),(8.5,y+1,z),.06,SAFETY,6)
    return m

def chimney(distant=False):
    m=Mesh(distant);beam(m,(0,-1,0),(0,62,0),3.0,PAINT,10 if distant else 24,1.8)
    for y in (42,52):beam(m,(0,y,0),(0,y+4,0),2.4-(y-42)*.018,RUST,10 if distant else 24,2.3-(y-42)*.018)
    m.collision=[dict(center=[0,3000,0],size_cm=[490,6200,490])];return m

def rack(distant=False):
    m=Mesh(distant)
    for x in (-11,0,11):
        for z in (-2,2):m.solid((x,2.4,z),(.20,4.8,.20),STEEL,True)
        m.solid((x,4.8,0),(.25,.3,4.8),STEEL)
    for z in (-1.5,0,1.5):beam(m,(-12,5.3,z),(12,5.3,z),.42,STEEL if z else RUST,6 if distant else 12)
    if not distant:
        for x in (-8,0,8):
            for z in (-1.5,0,1.5):beam(m,(x-.08,5.3,z),(x+.08,5.3,z),.52,TRIM,12)
    return m

def process_feed(short=False,distant=False):
    m=Mesh(distant);length=37 if short else 69;post=12 if short else 28
    beam(m,(0,5.3,-length/2),(0,5.3,length/2),.34,RUST,6 if distant else 12)
    for z in (-post,post):m.solid((0,2.55,z),(.24,5.1,.24),STEEL,True)
    beam(m,(0,1.5,0),(0,5.3,0),.25,RUST,6)
    m.solid((0,1,0),(1.6,2,2.4),STEEL,True)
    if not distant:beam(m,(-.9,2.2,0),(.9,2.2,0),.25,SAFETY,10)
    return m

def busbar(distant=False):
    m=Mesh(distant)
    for x in (-10,10):m.solid((x,4.6,0),(.20,9.2,.20),STEEL,True)
    for z in (-.5,0,.5):
        beam(m,(-18.5,6,z),(-10,9.4,z),.08,DARK,4 if distant else 6)
        beam(m,(-10,9.4,z),(10,9.4,z),.08,DARK,4 if distant else 6)
        beam(m,(10,9.4,z),(18.5,6,z),.08,DARK,4 if distant else 6)
    return m

def transformer(distant=False):
    m=Mesh(distant);m.solid((0,.25,0),(8,.5,6),STONE,True);m.solid((0,2.1,0),(6,3.8,4),STEEL,True)
    for x in (-2,0,2):
        beam(m,(x,3.8,0),(x,6,0),.30,TRIM,6 if distant else 12)
        for y in (4.1,4.7,5.3):beam(m,(x,y,0),(x,y+.16,0),.5,TRIM,6 if distant else 12)
    if not distant:
        for x in range(-12,13):m.solid((x*.22,2.1,-2.25),(.08,3,.5),DARK)
    return m

def admin(distant=False):
    m=Mesh(distant);m.solid((0,5,0),(26,10,16),TRIM,True);m.solid((0,10.2,0),(27,.4,17),STEEL,True)
    for floor in range(3):
        for x in (-10,-5,5,10):window(m,x,1.8+floor*3,-8.05,2.8,1.8)
    door(m,0,-8.08,2,2.6);m.solid((0,3.4,-10),(8,.25,4.2),STEEL,True)
    for x in (-3.6,3.6):m.solid((x,1.7,-11.7),(.2,3.4,.2),STEEL,True)
    m.solid((6,10.8,2),(6,1.2,5),DARK,True)
    return m

def compressor(distant=False):
    m=Mesh(distant);m.solid((0,.2,0),(9,.4,5),STONE,True)
    for x in (-2.3,2.3):
        beam(m,(x,1.5,-1.5),(x,1.5,1.5),1.2,STEEL,8 if distant else 16)
        m.solid((x,1.3,2.1),(2,2,1),DARK,True)
    m.collision.append(dict(center=[0,120,0],size_cm=[850,240,430]));return m

def library():
    return assemble_library({**{'production-'+str(i):lambda far,i=i:hall(i,far) for i in range(3)},
        **{'process-tank-'+str(i):lambda far,i=i:tank(i,far) for i in range(3)},
        'boiler-bank':boiler,'factory-stack':chimney,'pipe-rack':rack,'transformer':transformer,
        'factory-admin':admin,'compressor':compressor,'process-feed':lambda far:process_feed(False,far),
        'tank-link':lambda far:process_feed(True,far),'electrical-busbar':busbar},'factory_assets.py')
