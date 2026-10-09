"""Original MIT eroded sandstone, desert plants and quarry machinery, metres."""
import math
from authored_assets import Mesh,beam,crown,door,window,assemble_library,PALETTE,STONE,DARK,GLASS,WOOD,RUST,TRIM
SAND=(.67,.38,.23,1); OCHRE=(.76,.52,.31,1); BAND=(.47,.25,.17,1); PALE=(.82,.64,.43,1)
PALETTE.update({c:'stone' for c in (SAND,OCHRE,BAND,PALE)})

def strata(variant=0,distant=False):
    m=Mesh(distant);w,d,h=[(22,16,34),(18,14,46),(27,18,27),(12,10,19)][variant]
    sides=8 if distant else 16;levels=[(-.7,1.04),(h*.18,1.0),(h*.24,1.08),(h*.45,.81),(h*.52,.94),(h*.76,.69),(h,.58)]
    rings=[]
    for row,(y,radius) in enumerate(levels):
        rings.append([(math.cos(a)*w*.5*radius*(1+.07*math.sin(a*3+variant)),y+.35*math.sin(a*2+row),
            math.sin(a)*d*.5*radius*(1+.09*math.cos(a*3+variant))) for a in [i*math.tau/sides for i in range(sides)]])
    for row in range(len(rings)-1):
        vertices=rings[row]+rings[row+1];faces=[]
        for i in range(sides):j=(i+1)%sides;faces.extend([(i,j,j+sides),(i,j+sides,i+sides)])
        m.hull(vertices,faces,[SAND,BAND,OCHRE,PALE,SAND,OCHRE][row])
    for ring in (rings[0],rings[-1]):m.hull(ring,[(0,i,i+1) for i in range(1,sides-1)],OCHRE)
    m.collision=[dict(center=[0,round(h*45),0],size_cm=[round(w*74),round(h*90),round(d*72)])]
    return m

def rubble(distant=False):
    m=Mesh(distant)
    for i,(x,z,s) in enumerate([(-2,0,2.4),(0,1,3.2),(2,-1,2.0),(1,-2,1.1),(-1,-2,.8)]):
        crown(m,(x,s*.32,z),(s,s*.75,s*.86),91+i,SAND if i%2 else PALE)
    m.collision=[dict(center=[0,35,0],size_cm=[430,70,390])]
    return m

def stockpile(distant=False):
    m=Mesh(distant);crown(m,(0,1.8,0),(22,7,16),153,OCHRE)
    m.collision=[dict(center=[0,150,0],size_cm=[1700,300,1200])]
    return m

def cactus(distant=False):
    m=Mesh(distant);green=(.30,.39,.26,1)
    beam(m,(0,-.1,0),(0,4.8,0),.32,green,6 if distant else 10,.25)
    for x,h in [(-1.0,3.7),(1.1,3.1)]:
        beam(m,(0,2,0),(x,2,0),.23,green,6);beam(m,(x,2,0),(x,h,0),.23,green,6,.17)
    m.collision=[dict(center=[0,210,0],size_cm=[50,440,50])]
    return m

def scrub(distant=False):
    m=Mesh(distant)
    for i in range(5):
        a=i*math.tau/5;beam(m,(0,-.05,0),(math.cos(a)*.65,.85,math.sin(a)*.65),.035,WOOD,4,.015)
        crown(m,(math.cos(a)*.45,.63,math.sin(a)*.45),(1.0,.55,.8),23+i,(.46,.43,.28,1))
    m.collision=[dict(center=[0,10,0],size_cm=[15,20,15])]
    return m

def crusher(distant=False):
    m=Mesh(distant)
    for x in (-3,3):
        for z in (-2,2):m.solid((x,3,z),(.4,6,.4),DARK,True)
    m.solid((0,6,0),(8,.5,6),DARK,True)
    # Broad hopper above an open discharge bay, retained in the distance mesh.
    vs=[(x,y,z) for y,w,d in [(6.2,2,1.5),(10,4,3)] for x,z in [(-w,-d),(w,-d),(w,d),(-w,d)]]
    m.hull(vs,[(0,2,1),(0,3,2),(0,1,5),(0,5,4),(1,2,6),(1,6,5),(2,3,7),(2,7,6),(3,0,4),(3,4,7),(4,5,6),(4,6,7)],RUST,True)
    for side in (-1,1):beam(m,(side*3,0,-2),(side*3,6,2),.12,DARK,6)
    m.detail((4,4,0),(1,2,2),DARK)
    return m

def conveyor(distant=False):
    m=Mesh(distant)
    for x in (-1.2,1.2):
        beam(m,(x,1,-8),(x,6,8),.14,DARK,6)
        for z in (-6,0,6):m.solid((x,(1+(z+8)*5/16)/2,z),(.18,1+(z+8)*5/16,.18),DARK,True)
    for i in range(6 if distant else 15):
        z=-8+16*i/(5 if distant else 14);y=1+(z+8)*5/16
        beam(m,(-1.2,y,z),(1.2,y,z),.11,DARK,6)
    vs=[(-1,1,-8),(1,1,-8),(1,6,8),(-1,6,8)]
    m.hull(vs,[(0,1,2),(0,2,3)],BAND)
    m.collision.append(dict(center=[0,280,0],size_cm=[230,530,1600]))
    return m

def loader(distant=False):
    m=Mesh(distant);yellow=(.74,.53,.20,1)
    m.solid((0,1.2,0),(2.8,1.6,5.7),yellow,True);m.solid((0,2.7,.6),(2.4,2.1,2.3),GLASS,True)
    m.solid((0,3.9,.6),(2.6,.2,2.5),yellow)
    for x in (-1.5,1.5):
        for z in (-1.8,1.8):beam(m,(x-.35,.9,z),(x+.35,.9,z),.91,DARK,8 if distant else 12)
        beam(m,(x,1,-1.5),(x,.5,-4),.16,yellow,6)
    m.solid((0,.5,-4),(3.4,.65,1.4),DARK,True)
    return m

def office(distant=False):
    m=Mesh(distant);m.solid((0,1.8,0),(11,3.6,6),TRIM,True);m.solid((0,3.7,0),(11.8,.22,6.7),RUST,True)
    door(m,0,-3.08)
    for x in (-3.5,3.5):window(m,x,2,-3.08,2.2,1.4)
    for x in (-4,4):m.detail((x,2,3.05),(1.3,1.0,.7),DARK)
    return m

def pier(distant=False):
    m=Mesh(distant)
    for x in (-3.2,3.2):m.solid((x,22.1,0),(1.8,44.2,2.6),STONE,True)
    m.solid((0,44.5,0),(9,.6,3.4),STONE,True)
    for y in (12,28):beam(m,(-3.2,y,0),(3.2,y+8,0),.25,STONE,6)
    return m

def library():
    return assemble_library({**{'sandstone-'+str(i):lambda far,i=i:strata(i,far) for i in range(4)},
        'canyon-rubble':rubble,'canyon-stockpile':stockpile,'cactus':cactus,'desert-scrub':scrub,'quarry-crusher':crusher,
        'quarry-conveyor':conveyor,'quarry-loader':loader,'quarry-office':office,'canyon-pier':pier},'canyon_assets.py')
