"""Original MIT port buildings, working structures and street furniture, metres.

Shared structural members keep openings and silhouette in the authored far mesh;
window mullions, corrugations, equipment and joinery belong to the near model.
"""
import math
from authored_assets import (Mesh,beam,crown,gable,window,door,lettering,LETTERS,
    assemble_library,cm,STONE,CREAM,TRIM,BRICK,ROOF,SLATE,WOOD,DARK,GLASS,BLUE,RUST,PALETTE)

TEAL=(.15,.42,.43,1); YELLOW=(.73,.51,.17,1); RED=(.57,.22,.16,1)
CYAN=(.24,.88,.90,1); PINK=(.91,.32,.58,1); WARM=(.92,.77,.40,1)
PALETTE.update({TEAL:'metal',YELLOW:'metal',RED:'metal',CYAN:'glass',PINK:'glass',WARM:'glass'})
LETTERS.update({'H':['10001','10001','10001','11111','10001','10001','10001'],
 ' ':['00000']*7,'I':['11111','00100','00100','00100','00100','00100','11111'],
 'L':['10000','10000','10000','10000','10000','10000','11111'],
 'N':['10001','11001','11001','10101','10011','10011','10001'],
 'P':['11110','10001','10001','11110','10000','10000','10000'],
 'S':['01111','10000','10000','01110','00001','00001','11110'],
 'U':['10001','10001','10001','10001','10001','10001','01110'],
 'D':['11110','10001','10001','10001','10001','10001','11110']})

def block(variant=0,distant=False):
    m=Mesh(distant);w=[13.2,16.0,12.0,18.0,14.2,15.4][variant];d=[13,14,12,14,16,13][variant]
    floors=[4,5,3,4,6,3][variant];h=3.8+(floors-1)*3.0
    wall=[BRICK,CREAM,BLUE,(.57,.53,.46,1),(.40,.37,.35,1),(.70,.64,.49,1)][variant]
    neon=[CYAN,PINK,WARM,CYAN,PINK,WARM][variant]
    m.solid((0,-.16,0),(w+.3,.55,d+.3),STONE,True)
    m.solid((0,h/2,0),(w,h,d),wall,True)
    m.solid((0,h+.12,0),(w+.7,.24,d+.7),SLATE,True)
    # Street-facing continuous storefronts; upper floors remain proper dwellings.
    z=-d/2-.08
    for x in (-w*.30,w*.30):window(m,x,1.75,z,w*.31,2.55)
    door(m,0,z,1.5,2.8,DARK)
    m.solid((0,3.35,z-.1),(w-.5,.65,.22),DARK)
    m.solid((0,3.34,z-.23),(w-.85,.12,.03),neon)
    lettering(m,['PORT CAFE','NIGHT MARKET','CORNER BAR','HARBOR HOTEL','NEON ARCADE','RECORDS'][variant],3.88,z-.23,w*.7)
    for floor in range(1,floors):
        y=4.7+(floor-1)*3
        for x in (-w*.31,0,w*.31):window(m,x,y,z,1.45,1.6)
        for side in (-1,1):
            for tz in (-d*.3,0,d*.3):
                m.solid((side*(w/2+.035),y,tz),(.08,1.75,1.5),TRIM)
                m.solid((side*(w/2+.09),y,tz),(.035,1.55,1.3),GLASS)
        for x in (-w*.30,0,w*.30):
            m.solid((x,y,d/2+.05),(1.55,1.65,.1),TRIM)
            m.solid((x,y,d/2+.11),(1.35,1.45,.035),GLASS)
        if variant in (1,4):
            m.solid((0,y-.86,z-.6),(w*.72,.16,1.2),STONE,True)
            for x in (-w*.35,0,w*.35):m.solid((x,y-.37,z-1.1),(.08,.85,.08),DARK)
            beam(m,(-w*.35,y+.04,z-1.1),(w*.35,y+.04,z-1.1),.045,DARK,4)
        if not distant:
            m.solid((0,y-1.1,z),(w+.2,.12,.15),STONE)
            m.solid((w*.31,y-.9,z-.25),(1.1,.45,.55),CREAM)
    # Real parapets and a varied roof silhouette, visible from slopes and bridges.
    for side in (-1,1):
        m.solid((side*w/2,h+.58,0),(.2,.95,d),wall)
        m.solid((0,h+.58,side*d/2),(w,.95,.2),wall)
    m.solid((-w*.25,h+.8,2),(3.2,1.6,3.5),DARK,True)
    m.solid((w*.24,h+.5,1),(2.0,1.0,2.4),CREAM,True)
    if variant in (0,3,5):
        for x in (-w*.25,w*.25):m.solid((x,h+1.6,d*.24),(.7,3.2,.8),BRICK,True)
    if variant==4:
        for x in (-1.5,1.5):m.solid((x,h+1.2,0),(.18,2.4,.18),DARK)
        beam(m,(0,h+1.2,0),(0,h+4.7,0),1.8,TEAL,8 if distant else 16)
    # Projecting sign, service entrance and rear service stair.
    m.solid((-w*.43,5.3,z-.75),(.9,2.8,1.25),DARK)
    m.solid((-w*.43-.46,5.3,z-.75),(.035,2.5,1.1),neon)
    m.solid((-w*.43+.46,5.3,z-.75),(.035,2.5,1.1),neon)
    m.solid((0,1.15,d/2+.08),(1.2,2.3,.13),WOOD)
    m.solid((0,.12,d/2+.5),(2,.24,1),STONE,True)
    if not distant:
        beam(m,(w/2+.17,.15,d*.38),(w/2+.17,h,d*.38),.075,DARK)
    for i in ((0,7) if distant else range(8)):
        m.solid((w*.30,1.5+i*.4,d/2+.75+i*.27),(1.2,.10,.4),DARK)
    for side in (-1,1):
        beam(m,(w*.30+side*.5,1.5,d/2+.75),(w*.30+side*.5,4.3,d/2+2.64),.05,DARK,4)
    return m

def warehouse(variant=0,distant=False):
    m=Mesh(distant);w=[32,26,38][variant];d=[24,32,26][variant];h=[8,7,10][variant]
    wall=[(.48,.51,.49,1),(.52,.36,.28,1),(.30,.39,.41,1)][variant]
    m.solid((0,-.15,0),(w+.5,.55,d+.5),STONE,True)
    m.solid((0,h/2,0),(w,h,d),wall,True)
    gable(m,w+.6,d+.9,h,2.4,SLATE)
    # Raised loading dock with three bays, doors and protection bollards.
    m.solid((0,.55,-d/2-1.5),(w-.6,1.1,3),STONE,True)
    for x in (-w*.31,0,w*.31):
        m.solid((x,3.2,-d/2-.09),(5,4.2,.18),DARK)
        m.solid((x,3.2,-d/2-.2),(4.55,3.8,.055),TEAL)
        if not distant:
            for y in range(8):m.solid((x,1.5+y*.43,-d/2-.24),(4.5,.045,.04),TRIM)
        for edge in (-1,1):beam(m,(x+edge*2.65,0,-d/2-3.2),(x+edge*2.65,1.4,-d/2-3.2),.12,YELLOW,6)
    m.solid((0,6.2,-d/2-2),(w+1,.2,4.4),SLATE,True)
    for side in (-1,1):
        for z in (-d*.35,0,d*.35):
            m.solid((side*(w/2+.04),h-1.2,z),(.1,1.3,3.1),GLASS)
        beam(m,(side*(w/2+.2),0,d*.35),(side*(w/2+.2),h,d*.35),.13,DARK)
    for z in (-d*.28,d*.28):m.solid((0,h+2.4,z),(2.3,.9,3.3),CREAM,True)
    m.solid((w*.3,1.2,d/2+.06),(1.4,2.4,.14),WOOD)
    return m

def container(variant=0,distant=False,tiers=1):
    m=Mesh(distant);color=[RED,TEAL,YELLOW,(.33,.41,.54,1)][variant%4]
    for tier in range(tiers):
        cy=1.3+tier*2.6
        m.solid((0,cy,0),(12.2,2.6,2.36),color,True)
        for x in (-6,6):
            for z in (-1.24,1.24):m.solid((x,cy,z),(.18,2.6,.18),DARK)
        for z in (-1.29,1.29):
            # Formed ribs stand clear of the panel: no nearly coplanar faces.
            # Broad painted ribs survive middle-distance pixel sampling.
            for y in (cy-1.22,cy+1.22):m.solid((0,y,z),(12.2,.10,.08),color)
            # Shared metal relief supplies the fine side corrugation. Long
            # overlapping rib boxes alias at the normal middle display range.
        if not distant:
            for z in (-.6,.6):beam(m,(6.22,cy-1.1,z),(6.22,cy+1.1,z),.035,DARK,4)
    return m

def crane(distant=False):
    m=Mesh(distant);color=YELLOW
    # Open gantry legs and a cantilevered trussed jib; no bounding-box fill.
    for x in (-7.5,7.5):
        for z in (-5.5,5.5):
            m.solid((x,.75,z),(2.4,1.5,3.1),DARK,True)
            m.solid((x,8,z),(.95,15,.95),color,True)
        m.solid((x,15.5,0),(1.2,1.4,14),color,True)
    m.solid((0,16,0),(16.8,1.3,12.3),color,True)
    m.solid((0,19.7,2),(4.4,6,4.8),TEAL,True)
    m.solid((0,20.3,-1.5),(3.8,2.9,2.2),GLASS,True)
    for x in (-2.6,2.6):
        for y in (23,26):beam(m,(x,y,6),(x,y,-24),.23,color,6 if distant else 10)
        for i in range(6):
            z=6-i*5
            beam(m,(x,23,z),(x,26,z-5),.14,color,5)
            beam(m,(x,26,z),(x,23,z-5),.14,color,5)
        beam(m,(x,25,-21),(x,31,3),.07,DARK,4)
    for z in (6,-4,-14,-24):beam(m,(-2.6,26,z),(2.6,26,z),.14,color,5)
    m.solid((0,30.2,3),(1.1,10.2,1.1),color,True)
    m.solid((0,23.5,8),(6,3.5,4),DARK,True)
    for x in (-1.2,1.2):beam(m,(x,23,-19),(x,8,-19),.045,DARK,4)
    m.solid((0,8,-19),(4.4,.35,1.2),YELLOW)
    if not distant:
        for i in range(38):m.solid((7.95,i*.4+.2,5.5),(.55,.06,.45),TRIM)
        for x in (-8,8):beam(m,(x,16.8,-6),(x,16.8,6),.045,TRIM,4)
    return m

def truck(distant=False):
    m=Mesh(distant)
    m.solid((0,1.0,0),(2.3,.5,8.6),DARK,True)
    m.solid((0,2.2,1.1),(2.45,2.4,6.4),CREAM,True)
    m.solid((0,1.9,-3.1),(2.35,2.25,2.3),TEAL,True)
    m.solid((0,2.55,-4.27),(2.05,.8,.06),GLASS)
    m.solid((0,1.1,-4.3),(2.4,.28,.15),DARK)
    for z in (-3.2,1.9,3.0):
        for side in (-1,1):beam(m,(side*.9,.52,z),(side*1.3,.52,z),.51,DARK,8 if distant else 12)
    for x in (-.8,.8):m.solid((x,1.4,-4.31),(.38,.22,.08),WARM)
    return m

def dock_prop(kind,distant=False):
    m=Mesh(distant)
    if kind=='bollard':
        m.solid((0,.12,0),(.85,.24,.7),STONE,True)
        beam(m,(0,.1,0),(0,.85,0),.2,DARK,6 if distant else 12)
        beam(m,(-.42,.75,0),(.42,.75,0),.16,DARK,6)
        m.collision=[dict(center=[0,40,0],size_cm=[85,80,70])]
    elif kind=='quay-wall':
        m.solid((0,-2.2,0),(15,4.4,1.5),STONE,True)
        m.solid((0,.12,0),(15.2,.24,2),TRIM,True)
        for x in (-4,4):m.solid((x,-.8,-1.05),(1.2,2,.5),DARK)
    elif kind=='port-fence':
        for x in (-3,3):m.solid((x,1.4,0),(.10,2.8,.10),DARK,True)
        for y in (.2,1.4,2.65):m.solid((0,y,0),(6,.07,.07),DARK)
        if not distant:
            for x in range(-12,13):beam(m,(x*.24,0,0),(x*.24,2.8,0),.012,TRIM,4)
        m.collision=[dict(center=[0,140,0],size_cm=[610,280,12])]
    elif kind=='port-lamp':
        beam(m,(0,-.1,0),(0,10,0),.15,DARK,6)
        m.solid((0,10,0),(3.5,.2,.4),DARK)
        for x in (-1.3,0,1.3):
            m.solid((x,9.8,0),(.9,.3,.65),DARK)
            m.solid((x,9.64,0),(.7,.03,.5),WARM)
        m.collision=[dict(center=[0,500,0],size_cm=[32,1000,32])]
    elif kind=='pipeline':
        for x in (-7,0,7):
            m.solid((x,1.5,0),(.25,3,.25),STONE,True)
            m.solid((x,3,0),(.4,.3,2.3),STONE,True)
        for z in (-.6,.6):beam(m,(-8,3.4,z),(8,3.4,z),.3,TEAL,6 if distant else 12)
        if not distant:
            for x in (-6,-2,2,6):
                for z in (-.6,.6):beam(m,(x-.05,3.4,z),(x+.05,3.4,z),.37,TRIM,10)
    elif kind=='harbor-pier':
        for x in (-4,4):m.solid((x,6,0),(1.3,12,3),STONE,True)
        m.solid((0,11.8,0),(10.5,.4,3.8),STONE,True)
    elif kind=='guardrail':
        for x in (-2,0,2):m.solid((x,.53,0),(.1,1.1,.1),DARK,True)
        m.solid((0,.75,0),(4.3,.32,.1),TRIM,True)
    else:raise ValueError(kind)
    return m

def library():
    factories={**{'urban-'+str(i):lambda far,i=i:block(i,far) for i in range(6)},
        **{'warehouse-'+str(i):lambda far,i=i:warehouse(i,far) for i in range(3)},
        **{'container-'+str(i):lambda far,i=i:container(i,far) for i in range(4)},
        **{'container-stack-'+str(i):lambda far,i=i:container(i,far,3) for i in range(4)},
        'dock-crane':crane,'parked-truck':truck}
    factories.update({name:lambda far,name=name:dock_prop(name,far) for name in
        ('bollard','quay-wall','port-fence','port-lamp','pipeline','harbor-pier','guardrail')})
    return assemble_library(factories,'harbor_assets.py')
