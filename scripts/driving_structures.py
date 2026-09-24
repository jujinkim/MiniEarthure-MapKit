"""MIT reusable convex driving geometry, in centimetres; no map or game data."""
import math

FACES = [(0,1,2),(0,2,3),(4,7,6),(4,6,5),(0,4,5),(0,5,1),(1,5,6),(1,6,2),(2,6,7),(2,7,3),(3,7,4),(3,4,0)]

def prism(vertices):
    vertices=[[round(x) for x in v] for v in vertices]
    center=[sum(v[a] for v in vertices)/len(vertices) for a in range(3)]
    faces=[]
    for face in FACES:
        a,b,c=[vertices[i] for i in face]
        u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)]
        n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
        faces.append(list(reversed(face)) if sum(n[i]*(center[i]-a[i]) for i in range(3))>0 else list(face))
    return dict(vertices=vertices,faces=faces)

def box(width,height,length,x=0,y=0,z=0):
    return prism([[x+xx*width/2,y+yy*height/2,z+zz*length/2] for xx,yy,zz in [(-1,-1,-1),(1,-1,-1),(1,1,-1),(-1,1,-1),(-1,-1,1),(1,-1,1),(1,1,1),(-1,1,1)]])

def ramp(width=200,length=450,height=45,reverse=False):
    v=box(width,20,length)['vertices']
    for p in v:
        # Bury the underside; the driving face starts exactly at the surface.
        p[1]=(-5 if p[1]<0 else round(height*((-p[2] if reverse else p[2])+length/2)/length))
    return prism(v)

def fit_to_surface(g, sample_height):
    """Bake static structure contact into its shared visual/collision vertices.

    Sample both entry corners (including crossfall), not just centreline pitch.
    The convex hull tolerates quantized, non-planar terrain without open facets.
    All coordinates and the callable's heights are map centimetres.
    """
    if g['motion']['kind'] != 'static':
        raise ValueError('surface fitting requires a static structure')
    yaw=math.radians(g['rotation_mdeg'][1]/1000)
    c,s=math.cos(yaw),math.sin(yaw)
    g['rotation_mdeg'][0]=g['rotation_mdeg'][2]=0
    for part in g['parts']:
        vertices=[]
        for x,y,z in part['vertices']:
            sx=x*g['scale_per_mille'][0]/1000
            sz=z*g['scale_per_mille'][2]/1000
            wx=g['position'][0]+c*sx+s*sz
            wz=g['position'][2]-s*sx+c*sz
            vertices.append([x,round(y+(sample_height(wx,wz)-g['position'][1])*1000/g['scale_per_mille'][1]),z])
        part.update(convex_hull(vertices))
    radius=math.ceil(max(sum(abs(v[a]*g['scale_per_mille'][a]/1000) for a in range(3)) for p in g['parts'] for v in p['vertices']))
    g['safety_min_cm']=[v-radius for v in g['position']]
    g['safety_max_cm']=[v+radius for v in g['position']]
    return g

def tube(radius=100,length=600,half=False):
    result=[]
    for i in range(6 if half else 12):
        a=math.pi+i*math.pi/6 if half else i*math.pi/6
        b=a+math.pi/6
        quad=[(radius*math.cos(a),radius+radius*math.sin(a)),((radius+16)*math.cos(a),radius+(radius+16)*math.sin(a)),((radius+16)*math.cos(b),radius+(radius+16)*math.sin(b)),(radius*math.cos(b),radius+radius*math.sin(b))]
        result.append(prism([[x,y,z] for z in [-length/2,length/2] for x,y in quad]))
    return result

def parts(kind):
    if kind in ['pipe','log']:return tube()
    if kind=='halfpipe':return tube(190,700,True)
    if kind=='jump':return [ramp(),dict(vertices=[[v[0],v[1],v[2]+700] for v in ramp(reverse=True)['vertices']],faces=ramp(reverse=True)['faces'])]
    if kind=='humps':return [dict(vertices=[[v[0],v[1],v[2]+shift] for v in ramp(220,180,18,reverse)['vertices']],faces=ramp(220,180,18,reverse)['faces']) for shift,reverse in [(-180,False),(0,True),(180,False),(360,True)]]
    if kind=='rotate':return [box(280,18,28),box(18,180,28)]
    if kind=='barrier':return [box(25,45,180,y=24)]
    if kind=='platform':return [box(240,25,450,y=0)]
    if kind in ['boost','launch']:return [box(180,8,260,y=4)]
    return [ramp(200,450,45)]

def definition(ident,kind,position,yaw=0,color=(210,130,50,255)):
    motion=dict(kind={'barrier':'translate','platform':'translate'}.get(kind,kind if kind in ['rotate','boost','launch'] else 'static'),delta_cm=[0,0,0],axis=2,period_ms=5000,phase_ms=0,impulse_cmps=[0,0,0],cooldown_ms=1500)
    if kind=='barrier':motion['delta_cm']=[200,0,0]
    if kind=='platform':motion['delta_cm']=[0,100,0];motion['period_ms']=7000
    if kind=='boost':motion['impulse_cmps']=[round(350*math.sin(math.radians(yaw))),0,round(350*math.cos(math.radians(yaw)))]
    if kind=='launch':motion['impulse_cmps']=[round(450*math.sin(math.radians(yaw))),450,round(450*math.cos(math.radians(yaw)))]
    if kind=='rotate':position=[position[0],position[1]+110,position[2]]
    g=dict(id=ident,position=list(map(round,position)),rotation_mdeg=[0,round(yaw*1000),0],scale_per_mille=[1000]*3,parts=parts(kind),surface='concrete',color=list(color),motion=motion)
    radius=max(sum(abs(x) for x in v) for p in g['parts'] for v in p['vertices'])
    margin=2000 if kind in ['launch','boost'] else 0
    g['safety_min_cm']=[round(position[a]-radius+min(0,motion['delta_cm'][a])-margin) for a in range(3)]
    g['safety_max_cm']=[round(position[a]+radius+max(0,motion['delta_cm'][a])+margin) for a in range(3)]
    return g

def convex_hull(vertices):
    """Small integer hull for re-quantized public proxies; coplanar facets merge."""
    import itertools
    from functools import reduce
    points=sorted(set(tuple(v) for v in vertices));planes={}
    for a,b,c in itertools.combinations(points,3):
        u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)]
        n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
        if not any(n):continue
        d=sum(n[i]*a[i] for i in range(3));sides=[sum(n[i]*p[i] for i in range(3))-d for p in points]
        if min(sides)<0<max(sides):continue
        if max(sides)>0:n=[-x for x in n];d=-d
        divisor=reduce(math.gcd,[abs(x) for x in n]);n=tuple(x//divisor for x in n);d//=divisor
        planes[n,d]=[p for p in points if sum(n[i]*p[i] for i in range(3))==d]
    facets=[]
    for (normal,_),pts in sorted(planes.items()):
        drop=max(range(3),key=lambda i:abs(normal[i]));axes=[i for i in range(3) if i!=drop]
        pts=sorted(pts,key=lambda p:(p[axes[0]],p[axes[1]]))
        def cross(a,b,c):return (b[axes[0]]-a[axes[0]])*(c[axes[1]]-a[axes[1]])-(b[axes[1]]-a[axes[1]])*(c[axes[0]]-a[axes[0]])
        chain=[]
        for order in [pts,list(reversed(pts))]:
            edge=[]
            for p in order:
                while len(edge)>1 and cross(edge[-2],edge[-1],p)<=0:edge.pop()
                edge.append(p)
            chain.extend(edge[:-1])
        a,b,c=chain[:3];u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)]
        n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
        if sum(n[i]*normal[i] for i in range(3))<0:chain.reverse()
        facets.extend((chain[0],chain[i],chain[i+1]) for i in range(1,len(chain)-1))
    vertices=sorted(set(p for f in facets for p in f))
    return dict(vertices=[list(p) for p in vertices],faces=[[vertices.index(p) for p in f] for f in facets])


def authoring_templates():
    """Current templates for newly authored objects; historical artifacts stay intact."""
    kinds = ('ramp', 'jump', 'humps', 'pipe', 'log', 'halfpipe',
             'rotate', 'barrier', 'platform', 'boost', 'launch')
    return {kind: definition(kind, kind, [3200, 0, 3200]) for kind in kinds}


if __name__ == '__main__':
    import json
    from pathlib import Path
    destination = Path(__file__).resolve().parents[1] / 'godot/driving_templates.json'
    destination.write_text(json.dumps(authoring_templates(), indent=2) + '\n')
