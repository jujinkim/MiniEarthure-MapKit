"""MIT static mesh authoring helpers; coordinates in actual metres.
Derived from MapEditor driving_school_map/city_assets (MIT). Only authored GLB
serialization; no runtime generation, package writing or physical simulation.
"""
import json, math, struct
from collections import defaultdict

def canonical(value):
    return json.dumps(value,sort_keys=True,separators=(",",":"),allow_nan=False).encode()

FACES = [(0, 1, 2), (0, 2, 3), (4, 6, 5), (4, 7, 6),
         (0, 4, 5), (0, 5, 1), (1, 5, 6), (1, 6, 2),
         (2, 6, 7), (2, 7, 3), (3, 7, 4), (3, 4, 0)]


def cm(point):
    return [round(value * 100) for value in point]


def arc(cx, cy, radius, start, end, steps=12, height=0):
    return [(cx + radius * math.cos(math.radians(start + (end-start)*i/steps)),
             height, cy + radius * math.sin(math.radians(start + (end-start)*i/steps)))
            for i in range(steps + 1)]


def oriented_faces(vertices):
    """Outward integer triangles, including after reflection into glTF axes."""
    center = [sum(p[a] for p in vertices) / len(vertices) for a in range(3)]
    result = []
    for face in FACES:
        a, b, c = [vertices[i] for i in face]
        u, v = [[p[i]-a[i] for i in range(3)] for p in (b, c)]
        normal = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]]
        result.append(list(face if sum(normal[i]*(a[i]-center[i]) for i in range(3)) > 0
                           else (face[0], face[2], face[1])))
    return result


def box(center, size):
    x, h, y = center
    w, t, d = [v/2 for v in size]
    return [(x-w,h-t,y-d),(x+w,h-t,y-d),(x+w,h-t,y+d),(x-w,h-t,y+d),
            (x-w,h+t,y-d),(x+w,h+t,y-d),(x+w,h+t,y+d),(x-w,h+t,y+d)]


def glb(solids, indexed=False, generator=None):
    """Small static flat-shaded meshes, in glTF metres, with embedded materials."""
    binary, views, accessors, primitives, materials = bytearray(), [], [], [], []
    for solid in solids:
        vertices, color = solid[:2]
        vertices = [(x, h, -y) for x, h, y in vertices]
        positions, normals = [], []
        faces = oriented_faces(vertices) if len(solid) == 2 else [(a,c,b) for a,b,c in solid[2]]
        for face in faces:
            a, b, c = [vertices[i] for i in face]
            u, v = [[p[i]-a[i] for i in range(3)] for p in (b,c)]
            n = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]]
            length = math.sqrt(sum(x*x for x in n))
            positions.extend((a,b,c))
            normals.extend([tuple(x/length for x in n)]*3)
        element_indices = []
        if indexed:
            unique = {}
            for position, normal in zip(positions, normals):
                key = (position, normal)
                if key not in unique: unique[key] = len(unique)
                element_indices.append(unique[key])
            positions = [p for p, _ in unique]
            normals = [n for _, n in unique]
        indices = []
        for values in (positions, normals):
            offset = len(binary)
            binary.extend(b"".join(struct.pack("<fff", *v) for v in values))
            views.append(dict(buffer=0, byteOffset=offset, byteLength=len(binary)-offset, target=34962))
            accessors.append(dict(bufferView=len(views)-1, componentType=5126, count=len(values), type="VEC3",
                                  min=[min(p[i] for p in values) for i in range(3)],
                                  max=[max(p[i] for p in values) for i in range(3)]))
            indices.append(len(accessors)-1)
        pbr = color if isinstance(color, dict) else dict(baseColorFactor=color, metallicFactor=0, roughnessFactor=0.85)
        materials.append(dict(pbrMetallicRoughness=pbr))
        primitive = dict(attributes=dict(POSITION=indices[0], NORMAL=indices[1]), material=len(materials)-1, mode=4)
        if indexed:
            offset = len(binary)
            binary.extend(b"".join(struct.pack("<H", i) for i in element_indices))
            views.append(dict(buffer=0, byteOffset=offset, byteLength=len(binary)-offset, target=34963))
            accessors.append(dict(bufferView=len(views)-1, componentType=5123, count=len(element_indices), type="SCALAR"))
            primitive['indices'] = len(accessors)-1
            binary.extend(b"\0" * (-len(binary) % 4))
        primitives.append(primitive)
    doc = dict(asset=dict(version="2.0", generator=generator or "mapkit-world-library-v1"), scene=0, scenes=[dict(nodes=[0])],
               nodes=[dict(mesh=0)], meshes=[dict(primitives=primitives)], materials=materials,
               accessors=accessors, bufferViews=views, buffers=[dict(byteLength=len(binary))])
    data = canonical(doc)
    data += b" " * (-len(data) % 4)
    return (struct.pack("<4sII", b"glTF", 2, 28+len(data)+len(binary)) +
            struct.pack("<I4s", len(data), b"JSON") + data + struct.pack("<I4s", len(binary), b"BIN\0") + binary)


class Model:
    def __init__(self): self.groups=defaultdict(lambda:[[],[]]); self.collision=[]
    def faces(self, vertices, faces, color):
        vs,fs=self.groups[tuple(color)]; offset=len(vs)
        vs.extend(vertices);fs.extend(tuple(i+offset for i in face) for face in faces)
    def solid(self, center, size, color, physical=False):
        vertices=box(center,size)
        self.faces(vertices,oriented_faces(vertices),color)
        if physical: self.collision.append(dict(center=cm(center),size_cm=cm(size)))
    def panel(self, vertices, normal, color):
        a,b,c=vertices[:3];u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)]
        n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
        faces=[(0,1,2),(0,2,3)] if sum(n[i]*normal[i] for i in range(3))>0 else [(0,2,1),(0,3,2)]
        self.faces(vertices,faces,color)

    def export(self):
        return glb([(vs,dict(baseColorFactor=color,metallicFactor=.15 if color==GLASS else 0,
            roughnessFactor=.28 if color==GLASS else .85),fs) for color,(vs,fs) in self.groups.items()],indexed=True)

GLASS = (.16,.34,.39,1)
