"""Original MIT static amusement landmarks, shops, queues and viaduct supports."""
import math
from authored_assets import Mesh,beam,crown,gable,window,door,assemble_library,PALETTE,STONE,DARK,WOOD,GLASS,TRIM
RED=(.68,.26,.22,1); GOLD=(.87,.66,.29,1); TEAL=(.25,.52,.52,1); BLUE=(.31,.46,.65,1)
PALETTE.update({RED:'metal',GOLD:'metal',TEAL:'metal',BLUE:'metal'})

def solid_leg(m,a,b,r,color):
    vertices=[(x+dx,y,z+dz) for x,y,z in (a,b) for dx,dz in [(-r,-r),(r,-r),(r,r),(-r,r)]]
    m.hull(vertices,[(0,2,1),(0,3,2),(4,5,6),(4,6,7),(0,1,5),(0,5,4),
        (1,2,6),(1,6,5),(2,3,7),(2,7,6),(3,0,4),(3,4,7)],color,True)

def wheel(distant=False):
    m=Mesh(distant);r=27;h=34;n=24 if distant else 48
    for z in (-4,4):
        for x in (-12,12):
            beam(m,(x,-.5,z),(0,h,z),.68,TRIM,6 if distant else 10,.45)
            m.solid((x,.3,z),(4,.6,4),STONE,True)
        points=[(r*math.cos(i*math.tau/n),h+r*math.sin(i*math.tau/n),z*.45) for i in range(n)]
        for i in range(n):beam(m,points[i],points[(i+1)%n],.36,TEAL,5 if distant else 8)
        for i in range(12):
            a=i*math.tau/12;beam(m,(0,h,z*.45),(r*math.cos(a),h+r*math.sin(a),z*.45),.11,GOLD,4 if distant else 6)
    beam(m,(0,h,-5),(0,h,5),.75,DARK,8)
    for i in range(12):
        a=i*math.tau/12;cx=r*math.cos(a);cy=h+r*math.sin(a)-1.6
        beam(m,(cx,cy+1.6,-1.8),(cx,cy+1.6,1.8),.17,TRIM,6)
        m.solid((cx,cy-.8,0),(2.4,.4,2.6),RED if i%2 else GOLD)
        m.solid((cx,cy+1.2,0),(2.7,.22,2.8),TEAL)
        for x in (-1.1,1.1):
            for z in (-1.2,1.2):m.solid((cx+x,cy+.1,z),(.09,1.8,.09),TRIM)
        if not distant:
            for z in (-1.21,1.21):m.solid((cx,cy, z),(2.2,.7,.04),GLASS)
    # Slanted primary legs retain their true solid hulls, without filling the
    # central opening of the wheel with an oversized box proxy.
    for z in (-4,4):
        for side in (-1,1):solid_leg(m,(side*12,0,z),(0,34,z),.5,TRIM)
    return m

def carousel(distant=False):
    m=Mesh(distant);sides=16 if distant else 32
    beam(m,(0,.2,0),(0,.8,0),10.4,TEAL,sides)
    beam(m,(0,6.0,0),(0,9.5,0),11.5,RED,sides,1.0)
    beam(m,(0,.8,0),(0,8,0),.55,GOLD,8)
    m.collision=[dict(center=[0,40,0],size_cm=[1900,80,1900])]
    for i in range(10):
        a=i*math.tau/10;x=7*math.cos(a);z=7*math.sin(a)
        beam(m,(x,.8,z),(x,6.0,z),.08,GOLD,4 if distant else 6)
        crown(m,(x,2.1,z),(1.7,1, .7),70+i,TRIM if i%2 else GOLD)
        crown(m,(x+.6,2.7,z),(.6,1.1,.55),71+i,TRIM if i%2 else GOLD)
        if not distant:
            for dx in (-.5,.5):
                for dz in (-.24,.24):beam(m,(x+dx,2,z+dz),(x+dx,1.2,z+dz),.07,WOOD,5)
    for i in range(10):
        a=i*math.tau/10;beam(m,(0,6,0),(10.5*math.cos(a),6,10.5*math.sin(a)),.12,TRIM,6)
    return m

def coaster(distant=False):
    m=Mesh(distant);count=48 if distant else 96
    def point(t,offset=0):return ((62+offset)*math.cos(t),12+6*math.sin(t)+4*math.sin(2*t),(34+offset)*math.sin(t))
    for side in (-1,1):
        points=[point(i*math.tau/count,side*1.05) for i in range(count)]
        for i in range(count):beam(m,points[i],points[(i+1)%count],.15,RED,4 if distant else 6)
    for i in range(32 if distant else 64):
        t=i*math.tau/(32 if distant else 64);beam(m,point(t,-1.15),point(t,1.15),.12,TRIM,4)
    # Thirty slanted legs plus the station roof fit the current 32-hull limit.
    for i in range(15):
        t=i*math.tau/15;x,y,z=point(t)
        for side in (-1,1):
            sx=x+math.cos(t)*side*2.3;sz=z+math.sin(t)*side*2.3
            solid_leg(m,(sx,-.3,sz),(x,y-.25,z),.27,TEAL)
            m.solid((sx,.1,sz),(1.5,.2,1.5),STONE,True)
    m.solid((62,10.7,0),(9,.4,14),WOOD,True)
    for x in (58,66):
        for z in (-6,6):m.solid((x,7.7,z),(.24,15.4,.24),TEAL,True)
    gable(m,10,15,15.4,2,BLUE,x=62)
    if not distant:
        for i in range(24):m.solid((67,.22+i*.44,-8+i*.45),(2.1,.44,.65),STONE,True)
    else:
        # Keep the station access silhouette as a sloped slab in the far mesh.
        m.hull([(x,y,z) for x in (65.95,68.05) for y,z in [(0,-8.325),(0,2.675),(10.56,2.675)]],
            [(0,1,2),(3,5,4),(0,3,4),(0,4,1),(1,4,5),(1,5,2),(2,5,3),(2,3,0)],STONE)
    return m

def kiosk(variant=0,distant=False):
    m=Mesh(distant);color=[RED,TEAL,BLUE][variant]
    m.solid((0,1.8,0),(7.2,3.6,5.4),color,True);gable(m,8.4,6.8,3.6,2.0,GOLD if variant==0 else TRIM)
    m.solid((0,1.7,-2.77),(4.6,1.8,.12),DARK);m.solid((0,1.0,-3.15),(5.1,.15,.85),WOOD)
    m.solid((0,3.15,-3.6),(7.6,.18,2.3),color)
    for x in (-3.5,3.5):m.solid((x,1.5,-4.5),(.12,3,.12),TRIM,True)
    door(m,1.9,2.79,1.1,2.3)
    if not distant:
        for x in (-2.7,-1.35,0,1.35,2.7):m.solid((x,3.05,-4.72),(.6,.45,.04),TRIM)
    return m

def queue(distant=False):
    m=Mesh(distant)
    for x in (-4,0,4):m.solid((x,.5,0),(.08,1,.08),GOLD,True)
    for y in (.45,.85):beam(m,(-4,y,0),(4,y,0),.035,TEAL,4 if distant else 6)
    m.collision=[dict(center=[0,48,0],size_cm=[815,96,12])];return m

def flowerbed(distant=False):
    m=Mesh(distant);m.solid((0,.2,0),(8,.4,3),STONE,True);m.solid((0,.42,0),(7.5,.06,2.5),WOOD)
    for i in range(5 if distant else 10):
        x=-3.3+i*6.6/(4 if distant else 9)
        crown(m,(x,.73,0),(.65,.55,1.8),i+71,RED if i%2 else GOLD)
    return m

def pier(height,distant=False):
    m=Mesh(distant)
    for x in (-3.6,3.6):m.solid((x,(height-.7)/2,0),(1.8,height-.7,2.7),STONE,True)
    m.solid((0,height-.35,0),(10,.7,3.5),STONE,True)
    for y in range(10,int(height)-8,16):beam(m,(-3.6,y,0),(3.6,y+9,0),.22,TEAL,6)
    return m

def library():
    return assemble_library({'observation-wheel':wheel,'carousel':carousel,'decorative-coaster':coaster,
        **{'park-kiosk-'+str(i):lambda far,i=i:kiosk(i,far) for i in range(3)},'queue-rail':queue,'flowerbed':flowerbed,
        'sky-pier-low':lambda far:pier(59.8,far),'sky-pier-high':lambda far:pier(77.8,far)},'park_assets.py')
