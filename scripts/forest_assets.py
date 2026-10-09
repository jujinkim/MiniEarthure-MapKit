"""Original MIT temperate woodland models and quiet campsite structures, metres."""
import math
from authored_assets import (Mesh,beam,crown,gable,window,door,assemble_library,
    STONE,TRIM,WOOD,DARK,GLASS,ROOF,GREEN,PALETTE)

NEEDLES=(.15,.27,.20,1); MOSS=(.30,.39,.24,1); BARK=(.25,.20,.16,1)
PALETTE.update({NEEDLES:'leaf',MOSS:'leaf',BARK:'wood'})

def cedar(variant=0,distant=False):
    m=Mesh(distant);height=[21,17,25][variant];radius=[.48,.38,.57][variant]
    beam(m,(0,-.3,0),(.3,height-.7,-.25),radius,BARK,6 if distant else 10,.10)
    m.collision=[dict(center=[0,round(height*42),0],size_cm=[round(radius*175),round(height*85),round(radius*175)])]
    for i in range(6):
        y=height*.30+i*height*.108;spread=5.9-i*.74+(variant==2)*.8
        # Offset layered whorls keep the tree asymmetric and allow daylight
        # between branches; the far tree retains every principal crown tier.
        for j in range(3 if distant else 4):
            a=j*math.tau/(3 if distant else 4)+i*.7+variant
            x=math.cos(a)*spread*.28;z=math.sin(a)*spread*.28
            crown(m,(x,y,z),(spread,3.5,spread*.83),41+i*13+j,NEEDLES if i%2 else GREEN)
            if not distant:beam(m,(.1,y-.7,0),(x*2.4,y-.2,z*2.4),.10,BARK,5,.035)
    for j in range(5):
        a=j*math.tau/5
        beam(m,(0,.2,0),(math.cos(a)*1.6,-.2,math.sin(a)*1.6),.20,BARK,5,.10)
    return m

def beech(distant=False):
    m=Mesh(distant);beam(m,(0,-.3,0),(.6,10,.3),.57,BARK,6 if distant else 10,.22)
    m.collision=[dict(center=[0,390,0],size_cm=[90,840,90])]
    for i,(x,y,z,s) in enumerate([(-3,10,-1,8),(3,11,1,8),(1,14,-2,9),(-2,14,2,8),(0,17,0,6)]):
        beam(m,(.3,5+i*.8,0),(x,y,z),.22,BARK,5,.08)
        crown(m,(x,y,z),(s,s*.65,s*.83),810+i,GREEN if i%2 else MOSS)
    return m

def sapling(variant=0,distant=False):
    m=Mesh(distant);height=4.7 if variant==0 else 6.0
    beam(m,(0,-.12,0),(.15,height,0),.095,BARK,5,.025)
    m.collision=[dict(center=[0,160,0],size_cm=[16,340,16])]
    for i in range(3):
        crown(m,((-.3 if i%2 else .3),2+i*(height-2)/3,0),(2.4-i*.2,2.2,2.2),515+i+variant*7,NEEDLES if variant else MOSS)
    return m

def fern_patch(distant=False):
    m=Mesh(distant)
    # One small spatial patch, with separate fronds rather than solid green mats.
    for plant,(x,z) in enumerate([(-1.4,-.7),(.9,-.9),(-.2,.7),(1.3,1.0)]):
        for leaf in range(5 if distant else 8):
            a=leaf*math.tau/(5 if distant else 8)+plant
            ex=x+math.cos(a)*.83;ez=z+math.sin(a)*.83
            side=(-math.sin(a)*.20,math.cos(a)*.20)
            m.hull([(x,.08,z),(ex+side[0],.32,ez+side[1]),(ex,.65,ez),
                    (ex-side[0],.32,ez-side[1])],[(0,1,2),(0,2,3)],MOSS)
    m.collision=[dict(center=[0,2,0],size_cm=[8,4,8])]
    return m

def fallen(distant=False):
    m=Mesh(distant);beam(m,(-4,.43,0),(4,.68,.65),.48,BARK,7 if distant else 12,.34)
    for i in range(3):beam(m,(-2+i*2,.55,.18),( -2+i*2,1.6,.18+(-1 if i%2 else 1)*1.0),.12,BARK,5,.04)
    # Exposed pale end-grain, moss and a root ball break the cylindrical outline.
    beam(m,(-4.03,.43,0),(-4.06,.43,0),.42,WOOD,7)
    crown(m,(-2,.9,0),(3.2,.45,1.0),79,MOSS)
    m.collision=[dict(center=[0,48,32],size_cm=[810,90,145])]
    return m

def lodge(distant=False):
    m=Mesh(distant);w=14;d=10;h=4.6
    m.solid((0,-.2,0),(14.5,.8,10.5),STONE,True)
    m.solid((0,h/2,0),(w,h,d),WOOD,True);gable(m,15.7,12,h,3.4,ROOF)
    for x in (-4.5,4.5):window(m,x,2,-5.05,2.8,2,True)
    door(m,0,-5.05,1.5,2.7)
    m.solid((0,.3,-6.5),(14, .6,3.2),WOOD,True)
    for x in (-6.5,6.5):m.solid((x,2.1,-7.3),(.22,4.2,.22),WOOD,True)
    for side in (-1,1):
        for z in (-2,2):m.solid((side*7.05,2,z),(.12,1.8,1.6),GLASS)
    m.solid((4,6.8,2),(1,5,1.2),STONE,True)
    if not distant:
        for i in range(16):
            m.solid((0,.25+i*.27,-5.03),(14,.055,.07),DARK)
            m.solid((0,.25+i*.27,5.03),(14,.055,.07),DARK)
    return m

def tent(distant=False):
    m=Mesh(distant)
    m.hull([(-2,0,-2.4),(2,0,-2.4),(0,2.4,-2.4),(-2,0,2.4),(2,0,2.4),(0,2.4,2.4)],
        [(0,1,2),(3,5,4),(0,3,4),(0,4,1),(1,4,5),(1,5,2),(2,5,3),(2,3,0)],(.48,.45,.24,1))
    m.hull([(-.7,.04,-2.43),(.7,.04,-2.43),(0,1.9,-2.43)],[(0,1,2)],DARK)
    for x in (-2.5,2.5):
        for z in (-2.6,2.6):beam(m,(x,.02,z),(math.copysign(1.5,x),1.0,math.copysign(2.1,z)),.015,WOOD,4)
    m.collision=[dict(center=[0,100,0],size_cm=[380,200,470])]
    return m

def shelter(distant=False):
    m=Mesh(distant)
    for x in (-3,3):
        for z in (-2.3,2.3):m.solid((x,1.7,z),(.28,3.4,.28),WOOD,True)
    gable(m,7.2,6,3.4,1.6,ROOF)
    m.solid((0,.78,0),(4,.14,1.4),WOOD,True)
    for z in (-1.2,1.2):m.solid((0,.44,z),(4,.12,.48),WOOD,True)
    return m

def forest_pier(distant=False):
    m=Mesh(distant)
    for x in (-3,3):m.solid((x,12.2,0),(1.25,24.4,2.0),STONE,True)
    m.solid((0,24.6,0),(8,.4,2.6),STONE,True)
    return m

def library():
    factories={**{'cedar-'+str(i):lambda far,i=i:cedar(i,far) for i in range(3)},
        'beech':beech,**{'sapling-'+str(i):lambda far,i=i:sapling(i,far) for i in range(2)},
        'fern-patch':fern_patch,'fallen-cedar':fallen,'ranger-lodge':lodge,
        'camp-tent':tent,'picnic-shelter':shelter,'forest-pier':forest_pier}
    return assemble_library(factories,'forest_assets.py')
