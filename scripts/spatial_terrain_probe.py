"""Original synthetic terrain, made by the independent stdlib producer."""
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib


def make_fixture(project):
    root = Path(__file__).resolve().parents[1]
    source = project / 'terrain-source'
    (source / 'terrain').mkdir(parents=True)
    document = json.loads((root / 'examples/minimal/document.json').read_text())
    for field in ('nodes', 'roads', 'buildings', 'zones', 'assets', 'placements', 'heightmaps'):
        document[field] = []
    document.update(bounds={'min': [-837, -851], 'max': [436, 242]}, cell_size_cm=800)
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    for y in range(2):
        for x in range(2):
            step = (1, 2, 3, 6)[y * 2 + x]
            path = f'terrain/{x}-{y}.png'
            document['heightmaps'].append(dict(cell=dict(x=x, y=y), path=path, spacing_cm=200,
                                               offset_cm=-4000, step_cm=step, source_accuracy_cm=3000))
            rows = b''.join(b'\0' + b''.join(struct.pack('>H', (3000 + (x*4+i)*6 + (y*4+j)*12)//step)
                                           for i in range(5)) for j in range(5))
            png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 5, 5, 16, 0, 0, 0, 0))
            (source / path).write_bytes(png + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))
    (source / 'document.json').write_text(json.dumps(document))
    subprocess.run([sys.executable, str(root / 'examples/third_party.py'), str(project / 'terrain.memap'),
                    str(source / 'document.json')], check=True, timeout=30)


PROBE = '''extends SceneTree
const DATA = preload("res://addons/outer_runtime/mapkit/chunk_data.gd")
const RENDERER = preload("res://addons/outer_runtime/mapkit/chunk_renderer.gd")
var failures: Array[String] = []
func check(ok: bool, message: String) -> void:
    if not ok:
        failures.append(message)
        push_error(message)
func _initialize() -> void: run.call_deferred()
func run() -> void:
    var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
    var opened: Dictionary = JSON.parse_string(bridge.open_package(ProjectSettings.globalize_path("res://terrain.memap")))
    check(opened.ok, "independent 16-bit PNG package loads")
    if not opened.ok:
        quit(1)
        return
    var world := Node3D.new()
    root.add_child(world)
    world.position = Vector3(11, 3, -7)
    var probes: Array[Vector3] = []
    for y in 2:
        for x in 2:
            var packed: Dictionary = bridge.generate_chunk_packed(x, y)
            var raw: Dictionary = JSON.parse_string(bridge.generate_chunk(x, y))
            check(packed.ok and raw.ok and packed.data.generated_sha256 == raw.data.generated_sha256, "terrain packed/JSON hash")
            var c: Dictionary = DATA.view(packed.data.chunk)
            var faces := PackedVector3Array()
            for t in DATA.count(c):
                for v in [0, 2, 1]:
                    var vertex := DATA.scene_vertex(c, t, v)
                    check(vertex == DATA.scene_vertex(raw.data.chunk, t, v), "JSON/packed conversion equality")
                    check(vertex == RENDERER.scene_position(raw.data.chunk.triangles[t].vertices[v]), "object and terrain use the same centimetre scale and y flip")
                    faces.append(vertex)
                var start := faces.size() - 3
                probes.append((faces[start] + faces[start+1] + faces[start+2]) / 3.0)
            var body := StaticBody3D.new()
            var collider := CollisionShape3D.new()
            var shape := ConcavePolygonShape3D.new()
            shape.backface_collision = true
            shape.set_faces(faces)
            collider.shape = shape
            body.add_child(collider)
            world.add_child(body)
            var render := RENDERER.attach(packed.data.chunk, world)
            var mesh: MeshInstance3D = render.get_child(0)
            var arrays := mesh.mesh.surface_get_arrays(0)
            var rendered: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
            var indices: PackedInt32Array = arrays[Mesh.ARRAY_INDEX] if arrays[Mesh.ARRAY_INDEX] != null else PackedInt32Array()
            var expanded := PackedVector3Array()
            if indices.is_empty(): expanded = rendered
            else:
                for index in indices: expanded.append(rendered[index])
            check(expanded == faces, "common renderer mesh exactly matches collision faces")
    check(DATA.scene_vertex({"triangles": [{"vertices": [[-800, -400, 1600]]}]}, 0, 0) == Vector3(-8, -4, -16), "negative x/height and north axis exact metre scale")
    await physics_frame
    await physics_frame
    # Every triangle centroid, including partial boundary fragments, must be
    # hittable at the same height after translating the complete world.
    var space := world.get_world_3d().direct_space_state
    for local in probes:
        var target := world.to_global(local)
        var query := PhysicsRayQueryParameters3D.create(target + Vector3.UP, target - Vector3.UP)
        var hit := space.intersect_ray(query)
        check(not hit.is_empty(), "terrain collider covers rendered triangle")
        if not hit.is_empty(): check(hit.position.distance_to(target) < 0.00001, "physical/restored height within 0.01 mm scene units")
    # Exact shared edge and clipped map corner are still valid native queries.
    for point in [Vector2i(-37, -50), Vector2i(436, 242), Vector2i(-837, -851)]:
        var sample: Dictionary = JSON.parse_string(bridge.surface_probe(point.x, point.y, "terrain"))
        check(sample.ok and sample.data.position_cm[0] == point.x and sample.data.position_cm[2] == point.y, "native partial/boundary surface probe")
    check(not JSON.parse_string(bridge.cell_window(437, 242)).ok, "outside partial map rejected")
    world.queue_free()
    await process_frame
    await process_frame
    print("mapkit_spatial_terrain: " + ("PASS (independent PNG, packed/JSON/render/collision, negative origin, partial edges)" if failures.is_empty() else str(failures)))
    quit(0 if failures.is_empty() else 1)
'''
