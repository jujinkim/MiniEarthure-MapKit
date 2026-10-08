"""Original MIT metre-scale architectural and landscape mesh authoring.

Paired models share their local origin, silhouette and material palette. Fine
joinery is omitted in the authored distance mesh; openings remain real geometry.
The common renderer supplies its shared <=512px surface-detail material tiles.
"""
import hashlib
import json
import math
import random
import struct
from world_geometry import Model, cm, canonical

STONE=(.55,.52,.44,1); CREAM=(.82,.78,.65,1); TRIM=(.86,.84,.73,1)
BRICK=(.50,.26,.17,1); ROOF=(.31,.17,.13,1); SLATE=(.21,.26,.27,1)
WOOD=(.34,.24,.15,1); DARK=(.13,.17,.17,1); GLASS=(.17,.29,.31,1)
GREEN=(.23,.36,.25,1); BLUE=(.32,.46,.49,1); RUST=(.55,.28,.17,1)
PALETTE={STONE:'stone',CREAM:'plaster',TRIM:'plaster',BRICK:'brick',ROOF:'brick',
         SLATE:'stone',WOOD:'wood',DARK:'metal',GLASS:'glass',GREEN:'wood',BLUE:'plaster',RUST:'metal'}

def outward(vertices, faces):
    center=[sum(v[a] for v in vertices)/len(vertices) for a in range(3)]
    result=[]
    for face in faces:
        a,b,c=[vertices[i] for i in face]
        u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)]
        n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
        result.append(face if sum(n[i]*(a[i]-center[i]) for i in range(3))>0 else tuple(reversed(face)))
    return result

class Mesh(Model):
    def __init__(self, distant=False):
        super().__init__();self.distant=distant;self.convex=[]
    def hull(self, vertices, faces, color, physical=False):
        faces=outward(vertices,faces);self.faces(vertices,faces,color)
        if physical:self.convex.append(dict(vertices=[cm(v) for v in vertices],faces=faces))
    def detail(self, center, size, color):
        if not self.distant:self.solid(center,size,color)
    def export(self):
        data=super().export();n=struct.unpack_from('<I',data,12)[0]
        doc=json.loads(data[20:20+n]);binary=data[28+n:]
        for color,material in zip(self.groups,doc['materials']):
            material['name']='mk_'+PALETTE.get(color,'leaf' if color[1]>color[0]*1.12 else 'wood')
            if color==GLASS:material['pbrMetallicRoughness'].update(metallicFactor=.25,roughnessFactor=.23)
        raw=canonical(doc);raw+=b' '*(-len(raw)%4)
        return (struct.pack('<4sII',b'glTF',2,28+len(raw)+len(binary))+
                struct.pack('<I4s',len(raw),b'JSON')+raw+struct.pack('<I4s',len(binary),b'BIN\0')+binary)

def beam(m,a,b,r,color,sides=8,r2=None):
    """Tapered circular member, stable around arbitrary axes."""
    axis=[b[i]-a[i] for i in range(3)];length=math.sqrt(sum(v*v for v in axis));axis=[v/length for v in axis]
    ref=[0,1,0] if abs(axis[1])<.9 else [1,0,0]
    cross=lambda u,v:[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
    u=cross(axis,ref);n=math.sqrt(sum(v*v for v in u));u=[v/n for v in u];v=cross(axis,u)
    points=[]
    for center,radius in [(a,r),(b,r if r2 is None else r2)]:
        for i in range(sides):
            angle=math.tau*i/sides
            points.append(tuple(center[j]+radius*(u[j]*math.cos(angle)+v[j]*math.sin(angle)) for j in range(3)))
    faces=[]
    for i in range(sides):
        j=(i+1)%sides;faces.extend([(i,j,j+sides),(i,j+sides,i+sides)])
    for i in range(1,sides-1):faces.extend([(0,i,i+1),(sides,sides+i+1,sides+i)])
    m.hull(points,faces,color)

def crown(m,center,size,seed,color):
    r=random.Random(seed);sides=6 if m.distant else 9;rings=3 if m.distant else 5
    points=[(center[0],center[1]-size[1]/2,center[2])]
    for row in range(1,rings):
        angle=-math.pi/2+math.pi*row/rings
        for col in range(sides):
            a=math.tau*col/sides;rad=1+r.uniform(-.16,.13)
            points.append((center[0]+size[0]/2*math.cos(angle)*math.cos(a)*rad,
                center[1]+size[1]/2*math.sin(angle)+r.uniform(-.12,.12),
                center[2]+size[2]/2*math.cos(angle)*math.sin(a)*rad))
    points.append((center[0]+.13,center[1]+size[1]/2,center[2]-.10));faces=[]
    for i in range(sides):
        faces.append((0,1+(i+1)%sides,1+i))
        for row in range(rings-2):
            a=1+row*sides+i;b=1+row*sides+(i+1)%sides
            faces.extend([(a,b,b+sides),(a,b+sides,a+sides)])
        faces.append((len(points)-1,1+(rings-2)*sides+i,1+(rings-2)*sides+(i+1)%sides))
    m.hull(points,faces,color)

def gable(m,w,d,h,rise,color=ROOF,x=0,z=0):
    vs=[(x-w/2,h,z-d/2),(x+w/2,h,z-d/2),(x,h+rise,z-d/2),
        (x-w/2,h,z+d/2),(x+w/2,h,z+d/2),(x,h+rise,z+d/2)]
    fs=[(0,1,2),(3,5,4),(0,3,4),(0,4,1),(1,4,5),(1,5,2),(2,5,3),(2,3,0)]
    m.hull(vs,fs,color,True)
    if not m.distant:
        # Raised ridge, fascia, gutter and seams follow the pitched roof.
        beam(m,(x,h+rise+.04,z-d/2-.04),(x,h+rise+.04,z+d/2+.04),.075,color)
        for side in (-1,1):
            m.solid((x+side*w/2,h-.05,z),(.13,.16,d+.12),TRIM)
            for end in (-1,1):beam(m,(x+side*w/2,h,z+end*d/2),(x,h+rise,z+end*d/2),.075,TRIM)
        for row in range(1,7):
            a=row/7
            for side in (-1,1):
                beam(m,(x+side*w/2*a,h+rise*(1-a)+.025,z-d/2),
                     (x+side*w/2*a,h+rise*(1-a)+.025,z+d/2),.018,color,4)

def window(m,x,y,z,w=1.15,h=1.35,shutters=False):
    m.solid((x,y,z),(w+.16,h+.16,.10),TRIM)
    m.solid((x,y,z-.07),(w,h,.045),GLASS)
    if not m.distant:
        for dx in [-w/2,0,w/2]:m.solid((x+dx,y,z-.10),(.055,h,.055),TRIM)
        m.solid((x,y-.12,z-.10),(w,.055,.055),TRIM)
        m.solid((x,y-h/2-.09,z-.11),(w+.27,.12,.24),STONE)
        if shutters:
            for side in (-1,1):
                m.solid((x+side*(w*.75+.11),y,z-.04),(w*.42,h+.10,.11),GREEN)
                for dy in (-.4,-.2,0,.2,.4):m.solid((x+side*(w*.75+.11),y+dy,z-.11),(w*.40,.035,.035),DARK)

def door(m,x,z,w=1.05,h=2.2,color=WOOD):
    m.solid((x,h/2+.12,z),(w+.22,h+.20,.15),TRIM)
    m.solid((x,h/2+.08,z-.10),(w,h,.06),color)
    m.detail((x,h*.75,z-.15),(w*.62,h*.25,.035),GLASS)
    m.detail((x+w*.35,1.15,z-.16),(.04,.17,.035),DARK)

def house(variant=0,distant=False):
    m=Mesh(distant)
    w,d,h=[(8.4,7.2,3.3),(8.8,8.2,5.9),(12.0,6.8,3.5),(6.8,8,6.0),(10.2,8.5,5.5),(8.0,9.2,4.1)][variant]
    wall=[CREAM,BLUE,TRIM,BRICK,CREAM,(.64,.62,.48,1)][variant]
    m.solid((0,-.18,0),(w+.30,.65,d+.30),STONE,True)
    m.solid((0,h/2,0),(w,h,d),wall,True)
    gable(m,w+.8,d+.85,h,2.0 if variant!=3 else 2.8,ROOF if variant%2==0 else SLATE)
    # Side wing makes two residences read differently even from a moving car.
    if variant in (2,4):
        m.solid((w*.35,1.5,-d*.42),(w*.44,3.,d*.7),wall,True)
        gable(m,w*.44+.5,d*.7+.5,3.,1.4,ROOF,w*.35,-d*.42)
    if variant in (0,1,3):
        m.solid((-w*.27,h+1.05,d*.24),(.66,2.6,.74),BRICK,True)
        m.solid((-w*.27,h+2.4,d*.24),(.86,.18,.92),STONE)
    front=-d/2-.04
    for floor in range(1 if h<5 else 2):
        for x in (-w*.29,w*.29):window(m,x,1.85+floor*2.7,front,shutters=variant!=3)
    door(m,0,front)
    for x in (-w*.48,w*.48):m.detail((x,h/2,front+.015),(.12,h,.12),TRIM)
    # Rear openings and a kitchen lean-to belong to the building, not a blank box.
    for x in (-w*.28,w*.28):
        m.solid((x,1.9,d/2+.055),(1.25,1.25,.10),TRIM)
        m.solid((x,1.9,d/2+.12),(1.10,1.10,.04),GLASS)
    if not distant:
        for side in (-1,1):
            for y in [1.9]+([4.6] if h>5 else []):
                for z in (-d*.23,d*.23):
                    m.solid((side*(w/2+.055),y,z),(.1,1.3,1.1),TRIM)
                    m.solid((side*(w/2+.11),y,z),(.035,1.12,.95),GLASS)
            beam(m,(side*(w/2+.12),.1,front+.12),(side*(w/2+.12),h,front+.12),.05,DARK)
    m.solid((0,.09,front-.65),(2.5,.18,1.3),STONE,True)
    if variant in (0,4,5):
        m.solid((0,2.8,front-.9),(3.3,.16,2),SLATE,True)
        for x in (-1.45,1.45):m.solid((x,1.4,front-1.65),(.14,2.8,.14),WOOD,True)
    return m

LETTERS={'A':['01110','10001','10001','11111','10001','10001','10001'],
 'B':['11110','10001','10001','11110','10001','10001','11110'],
 'C':['01111','10000','10000','10000','10000','10000','01111'],
 'E':['11111','10000','10000','11110','10000','10000','11111'],
 'F':['11111','10000','10000','11110','10000','10000','10000'],
 'G':['01111','10000','10000','10111','10001','10001','01111'],
 'K':['10001','10010','10100','11000','10100','10010','10001'],
 'M':['10001','11011','10101','10101','10001','10001','10001'],
 'O':['01110','10001','10001','10001','10001','10001','01110'],
 'R':['11110','10001','10001','11110','10100','10010','10001'],
 'T':['11111','00100','00100','00100','00100','00100','00100'],
 'Y':['10001','10001','01010','00100','00100','00100','00100']}
def lettering(m,text,y,z,width):
    if m.distant:return
    step=width/(len(text)*6);left=-width/2
    for i,char in enumerate(text):
        for row,bits in enumerate(LETTERS[char]):
            for col,bit in enumerate(bits):
                if bit=='1':m.solid((left+(i*6+col+.5)*step,y+(3-row)*step,z),(step*.85,step*.85,.022),TRIM)

def shop(variant=0,distant=False):
    m=Mesh(distant);w=[9.4,11.8,8.6,12.8][variant];d=8.5;h=[6.0,5.6,6.8,4.2][variant]
    wall=[BRICK,CREAM,BLUE,(.65,.62,.52,1)][variant];accent=[GREEN,RUST,BLUE,DARK][variant]
    m.solid((0,-.15,0),(w+.3,.6,d+.3),STONE,True)
    m.solid((0,h/2,0),(w,h,d),wall,True)
    if variant!=3:gable(m,w+.5,d+.6,h,2.1,ROOF if variant<2 else SLATE)
    else:
        m.solid((0,h+.12,0),(w+.6,.24,d+.6),SLATE,True)
        m.solid((w*.27,h+.65,d*.2),(1.5,1.1,2),DARK,True)
    z=-d/2-.06
    for x in (-w*.28,w*.28):window(m,x,1.55,z,w*.29,2.25)
    door(m,0,z,1.35,2.5,accent)
    m.solid((0,3.3,z-.1),(w-.5,.75,.22),accent)
    lettering(m,['BAKERY','MARKET','CAFE','GARAGE'][variant],3.3,z-.225,w*.64)
    if variant!=3:
        for x in (-w*.28,0,w*.28):window(m,x,4.85,z,1.15,1.35,variant==2)
    # The striped canvas has a physical sloping canopy and a thin scalloped edge.
    count=3 if distant else 12
    for i in range(count):
        x=-w/2+(i+.5)*w/count
        color=accent if i%2==0 else TRIM
        m.panel([(x-w/count/2,2.9,z),(x+w/count/2,2.9,z),
                 (x+w/count/2,2.65,z-1.5),(x-w/count/2,2.65,z-1.5)],(0,1,-.2),color)
        m.solid((x,2.54,z-1.5),(w/count,.22,.05),color)
    for side in (-1,1):
        for y in ([1.8,4.75] if h>5 else [1.8]):
            for z_side in (-1.6,1.6):
                m.solid((side*(w/2+.04),y,z_side),(.09,1.45,1.3),TRIM)
                m.solid((side*(w/2+.10),y,z_side),(.035,1.24,1.10),GLASS)
    m.solid((-w*.27,1.12,d/2+.04),(1.1,2.24,.12),WOOD)
    m.solid((w*.27,.3,d/2+1),(2.1,.6,2),STONE,True)
    return m

def market_hall(distant=False):
    m=Mesh(distant);w=17.;d=10.;h=6.7
    # Three open arcade bays stay open in both representations.
    m.solid((0,4.9,0),(w,3.6,d),CREAM,True)
    for x in (-7.6,-2.6,2.6,7.6):m.solid((x,1.6,-4.2),(.6,3.2,.65),STONE,True)
    m.solid((0,1.6,2.5),(w,3.2,5),CREAM,True)
    gable(m,w+.7,d+.8,h,2.9,SLATE)
    for x in (-6,-3,0,3,6):window(m,x,5.25,-5.04,1.45,1.7)
    m.solid((0,10.6,0),(2.8,3.9,2.8),CREAM,True)
    gable(m,3.5,3.5,12.55,2,SLATE)
    # Clock disk and hands are a readable civic landmark.
    beam(m,(0,11.1,-1.45),(0,11.1,-1.56),.94,TRIM,12 if distant else 24)
    if not distant:
        beam(m,(0,11.1,-1.58),(0,11.77,-1.58),.045,DARK,4)
        beam(m,(0,11.1,-1.59),(.43,10.9,-1.59),.05,DARK,4)
        for i in range(12):
            a=math.tau*i/12
            m.solid((math.sin(a)*.78,11.1+math.cos(a)*.78,-1.60),(.06,.08,.025),DARK)
    return m

def barn(distant=False):
    m=Mesh(distant);m.solid((0,-.18,0),(13,.7,17),STONE,True)
    m.solid((0,2.65,0),(12.5,5.3,16.5),RUST,True);gable(m,13.4,17.4,5.3,3.1,SLATE)
    m.solid((0,2.2,-8.3),(5,4.4,.10),WOOD)
    for x in (-2.5,0,2.5):m.solid((x,2.2,-8.4),(.16,4.4,.12),TRIM)
    for x in (-5.8,5.8):m.solid((x,2.65,-8.35),(.16,5.3,.13),TRIM)
    if not distant:
        for i in range(-20,21):m.detail((i*.3,2.65,-8.30),(.035,5.3,.03),WOOD)
        for side in (-1,1):
            beam(m,(side*.12,.15,-8.5),(side*2.35,4.25,-8.5),.065,TRIM,4)
        for z in (-5,0,5):
            for side in (-1,1):m.solid((side*6.3,3.8,z),(.10,1.2,1.4),GLASS)
    window(m,0,6.35,-8.62,1.2,1.1)
    return m

def shed(distant=False):
    m=Mesh(distant)
    m.solid((0,2,2.6),(9,4,.3),WOOD,True)
    for x in (-4.3,0,4.3):
        for z in (-2.5,2.5):m.solid((x,2,z),(.22,4,.22),WOOD,True)
    gable(m,9.6,6.2,4,1.15,SLATE)
    m.detail((0,.08,0),(8.8,.16,5.5),STONE)
    return m

def tree(variant=0,distant=False):
    m=Mesh(distant);r=random.Random(802+variant)
    height=[12.5,15.2,10.6,8.0,4.8][variant];radius=[4.6,4,4.4,3.5,1.8][variant]
    bark=(.48,.49,.41,1) if variant==1 else WOOD
    beam(m,(0,-.28,0),(.25,height*.72,.1),.26 if variant!=4 else .10,bark,6 if distant else 9,r2=.11)
    m.collision=[dict(center=cm((0,height*.26,0)),size_cm=cm((.55 if variant!=4 else .22,height*.57,.55 if variant!=4 else .22)))]
    count=8
    for i in range(count):
        angle=math.tau*i/count+r.uniform(-.23,.23)
        ring=radius*r.uniform(.28,.70)
        y=height*(.68+r.uniform(-.16,.15));x=math.cos(angle)*ring;z=math.sin(angle)*ring
        if not distant:beam(m,(.1,height*.36,0),(x,y,z),.10,bark,6,r2=.035)
        color=[(.23,.35,.15,1),(.30,.42,.19,1),(.19,.31,.17,1),(.35,.44,.22,1)][(i+variant)%4]
        crown(m,(x,y,z),(radius*.98,height*.38,radius*.98),i+variant*51,color)
    crown(m,(.3,height*.85,-.25),(radius*1.25,height*.31,radius*1.12),910+variant,(.29,.40,.17,1))
    return m

def shrub(distant=False,hedge=False):
    m=Mesh(distant)
    for i in range(2 if distant else 5):
        x=(i/(1 if distant else 4)-.5)*(3 if hedge else 1.5)
        crown(m,(x,.68,0),(1.9,1.5,1.45),40+i,(.22+i*.008,.32+i*.014,.14,1))
    if hedge:m.collision=[dict(center=[0,50,0],size_cm=[440,100,110])]
    return m

def rock(variant=0,distant=False):
    m=Mesh(distant);r=random.Random(variant+330)
    for i in range(1+variant%2):
        center=(i*1.6,.65,0);size=(3+variant*.5,2.3,2.6)
        crown(m,center,size,variant*10+i,STONE)
        m.collision.append(dict(center=cm((center[0],.55,0)),size_cm=cm((size[0]*.7,1.6,size[2]*.72))))
    return m

def fence(distant=False):
    m=Mesh(distant)
    for x in (-2,2):m.solid((x,.64,0),(.13,1.5,.13),WOOD,True)
    for y in (.5,1.05):m.solid((0,y,0),(4.1,.11,.10),TRIM,True)
    return m

def prop(kind,distant=False):
    m=Mesh(distant)
    if kind=='bench':
        for x in (-.72,.72):m.solid((x,.24,0),(.1,.48,.5),DARK,True)
        for i in range(3):m.solid((0,.47,-.18+i*.16),(1.85,.075,.13),WOOD,True)
        for i in range(2):m.solid((0,.72+i*.15,.25),(1.85,.11,.065),WOOD,True)
    elif kind=='lamp':
        beam(m,(0,0,0),(0,4.7,0),.08,DARK)
        beam(m,(0,4.5,0),(.75,4.7,0),.065,DARK)
        m.solid((.65,4.68,0),(.7,.16,.36),DARK)
        m.solid((.65,4.59,0),(.54,.025,.28),TRIM)
        m.collision=[dict(center=[0,230,0],size_cm=[18,460,18])]
    elif kind=='hay':
        beam(m,(-.7,.66,0),(.7,.66,0),.68,(.60,.50,.24,1),8 if distant else 14)
        if not distant:
            for x in (-.43,.43):beam(m,(x-.025,.66,0),(x+.025,.66,0),.692,WOOD,14)
        m.collision=[dict(center=[0,60,0],size_cm=[140,120,130])]
    elif kind=='crate':
        m.solid((0,.4,0),(.9,.8,.7),WOOD,True)
        for y in (.14,.4,.67):m.detail((0,y,-.365),(.96,.17,.08),CREAM)
        for x in (-.39,.39):m.detail((x,.4,-.42),(.075,.84,.08),WOOD)
    elif kind=='table':
        for x in (-.55,.55):m.solid((x,.38,0),(.12,.76,.55),WOOD,True)
        m.solid((0,.79,0),(1.5,.10,.85),WOOD,True)
    elif kind=='planter':
        m.solid((0,.32,0),(1.7,.64,.7),STONE,True)
        for x in (-.55,0,.55):
            crown(m,(x,.86,0),(.75,.8,.7),round(x*10)+20,(.24,.35,.16,1))
            if not distant:
                for i in range(3):crown(m,(x+.14*math.cos(i*2),1.1,.2*math.sin(i*2)),(.13,.12,.13),i,(.71,.38,.30,1))
    elif kind=='bin':
        beam(m,(0,.1,0),(0,.85,0),.28,GREEN,8);m.solid((0,.83,0),(.61,.06,.61),DARK)
        m.collision=[dict(center=[0,45,0],size_cm=[60,90,60])]
    elif kind=='log':
        beam(m,(-2,.32,0),(2,.50,.18),.30,WOOD,7,r2=.22)
        m.collision=[dict(center=[0,35,0],size_cm=[400,60,60])]
    elif kind=='reeds':
        for i in range(3 if distant else 12):
            x=math.sin(i*2.4)*.6;z=math.cos(i*1.4)*.5;h=1+math.sin(i)*.25
            beam(m,(x,0,z),(x+.12,h,z+.13),.018,GREEN,4)
            beam(m,(x+.12,h*.8,z+.13),(x+.12,h+.15,z+.13),.05,WOOD,4)
    elif kind=='grass':
        for i in range(3 if distant else 16):
            x=math.sin(i*2.4)*1.9;z=math.cos(i*1.4)*1.8;h=.25+math.sin(i)*.13
            m.panel([(x-.04,0,z),(x+.04,0,z),(x+.1,h,z+.04),(x+.05,h*.6,z+.03)],(0,0,-1),(.36,.43,.22,1))
    else:raise ValueError(kind)
    return m

def vehicle(tractor=False,distant=False):
    m=Mesh(distant);color=GREEN if tractor else BLUE
    if tractor:
        m.solid((0,.88,0),(1.5,.55,2.8),GREEN,True)
        m.solid((0,1.27,-.65),(1.15,.65,1.3),GREEN)
        m.solid((0,1.6,.53),(1.3,1.4,1.25),GLASS)
        m.solid((0,2.36,.53),(1.55,.12,1.55),CREAM)
        beam(m,(.49,1.55,-.8),(.49,2.45,-.8),.06,DARK)
    else:
        m.solid((0,.64,0),(1.75,.64,4.15),color,True)
        m.solid((0,1.08,.05),(1.55,.55,2.25),GLASS)
        m.solid((0,1.42,.25),(1.54,.12,1.8),color)
        m.detail((0,.65,-2.10),(1.7,.14,.10),DARK)
        for x in (-.6,.6):m.detail((x,.83,-2.105),(.38,.17,.08),TRIM)
    for z in (-1.05,1.05):
        radius=.63 if tractor and z>0 else .38
        for side in (-1,1):
            beam(m,(side*.75,radius,z),(side*1.05,radius,z),radius,DARK,8 if distant else 12)
            if not distant:beam(m,(side*1.045,radius,z),(side*1.06,radius,z),radius*.45,CREAM,8)
    return m

def crop(kind=0,distant=False):
    """A small 8x8m cell-local patch, never one whole-field visibility anchor."""
    m=Mesh(distant)
    for row in range(6):
        z=-3.25+row*1.3
        if distant:
            m.solid((0,.25 if kind else .50,z),(7.7,.48 if kind else .85,.45),(.39,.45,.19,1) if kind else (.60,.54,.28,1))
        else:
            for col in range(10):
                x=-3.5+col*.78
                if kind:
                    crown(m,(x,.23,z),(.65,.5,.62),row*11+col,(.34,.43,.19,1))
                else:
                    beam(m,(x,0,z),(x+.06,.96,z),.023,(.55,.53,.27,1),4)
                    beam(m,(x+.06,.74,z),(x+.08,1.07,z),.067,(.65,.58,.29,1),5)
    return m

def bridge_support(distant=False,height=6.8):
    """Bank/pier module; the authored base height meets a separate road deck."""
    m=Mesh(distant)
    m.solid((0,height/2,0),(8.8,height,3.6),STONE,True)
    m.solid((0,height-.1,0),(9.1,.2,4.1),STONE,True)
    return m

def library():
    factories={**{'home-'+str(i):lambda far,i=i:house(i,far) for i in range(6)},
        **{'shop-'+str(i):lambda far,i=i:shop(i,far) for i in range(4)},
        **{'tree-'+str(i):lambda far,i=i:tree(i,far) for i in range(5)},
        **{'rock-'+str(i):lambda far,i=i:rock(i,far) for i in range(3)},
        'market-hall':market_hall,'barn':barn,'shed':shed,'shrub':shrub,'hedge':lambda far:shrub(far,True),
        'fence':fence,'bridge-support':bridge_support,'bridge-support-high':lambda far:bridge_support(far,17.8),
        'tractor':lambda far:vehicle(True,far),'parked-car':lambda far:vehicle(False,far),
        'wheat':lambda far:crop(0,far),'cabbage':lambda far:crop(1,far)}
    factories.update({name:lambda far,name=name:prop(name,far) for name in ('bench','lamp','hay','crate','table','planter','bin','log','reeds','grass')})
    result={}
    for name,factory in factories.items():
        near=factory(False);far=factory(True);payloads={}
        paths=[]
        for model in (near,far):
            data=model.export();path='assets/'+hashlib.sha256(data).hexdigest()+'.glb';payloads[path]=data;paths.append(path)
        vertices=[v for vs,_ in near.groups.values() for v in vs]
        bounds=[[min(v[a] for v in vertices) for a in range(3)],[max(v[a] for v in vertices) for a in range(3)]]
        if not near.collision and not near.convex:
            # Soft vegetation has a buried root-bed footprint. Its one metre
            # burial keeps the proxy below this recipe's maximum local relief;
            # the declared footprint still participates in placement validation.
            near.collision=[dict(center=cm(((bounds[0][0]+bounds[1][0])/2,-1.1,(bounds[0][2]+bounds[1][2])/2)),
                size_cm=cm((bounds[1][0]-bounds[0][0],.2,bounds[1][2]-bounds[0][2])))]
        record=dict(id='authored-'+name,path=paths[0],distant_path=paths[1],collision=near.collision,
            convex_collision=near.convex,attribution=dict(source='mapkit-authored-assets-v1',license='MIT',
                notice='Original metre-scale geometry; authored_assets.py. Shared MapKit material tiles. No extracted assets.'))
        result[name]=(record,payloads,bounds)
    return result
