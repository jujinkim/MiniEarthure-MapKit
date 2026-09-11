"""Recipe-6 presentation identity and per-cell display resource retirement."""
import json

def fixture(root):
    d=json.loads((root/'examples/minimal/document.json').read_text())
    d.update(recipe_version=6,bounds={'min':[0,0],'max':[3200,3200]},cell_size_cm=1600,
             buildings=[],zones=[],placements=[],assets=[],heightmaps=[])
    d['nodes']=[dict(id=name,position=p,level=0) for name,p in [('a',[0,0,800]),('b',[3200,0,800]),('c',[0,-1,1400]),('d',[3200,-1,1400])]]
    marks=dict(lanes=2,center_line=True,edge_lines=True,crosswalk_start=True,crosswalk_end=True)
    d['roads']=[dict(id='street',**{'from':'a','to':'b'},points=[[0,0,800],[3200,0,800]],widths_cm=[400],surfaces=['asphalt'],kind='ground',sidewalk_cm=100,markings=marks),
                dict(id='buried',**{'from':'c','to':'d'},points=[[0,-1,1400],[3200,-1,1400]],widths_cm=[200],surfaces=['asphalt'],kind='elevated',sidewalk_cm=0,markings=marks)]
    d['surface_areas']=[dict(id='paving',polygon=[[0,0],[3200,0],[3200,3200],[0,3200]],surface='concrete')]
    return d

PROBE = '''extends SceneTree
const RENDERER = preload("res://addons/outer_runtime/mapkit/chunk_renderer.gd")
const DATA = preload("res://addons/outer_runtime/mapkit/chunk_data.gd")
class Lease extends RefCounted:
    var refs: Array = []
    var sealed := false
    func track(value: Object) -> void: refs.append(weakref(value))
    func seal() -> void: sealed = true
    func alive() -> bool:
        return refs.any(func(ref: WeakRef): return ref.get_ref() != null)
func _initialize() -> void: run.call_deferred()
func check(value: bool, message: String) -> void:
    if not value:
        push_error(message)
        quit(1)
        assert(value,message)
func shader(node: Node) -> Shader:
    if node is MeshInstance3D and node.material_override is ShaderMaterial: return node.material_override.shader
    for child: Node in node.get_children():
        var found := shader(child)
        if found != null: return found
    return null
func run() -> void:
    var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
    check(JSON.parse_string(bridge.open_package(ProjectSettings.globalize_path("res://urban.memap"))).ok,"urban package")
    var world := Node3D.new()
    root.add_child(world)
    var jobs: Array = []
    var leases: Array = []
    for x in 2:
        var raw: Dictionary = bridge.generate_chunk_packed(x,0)
        var shown: Dictionary = bridge.with_presentation(raw.data)
        check(shown.ok and shown.data.generated_sha256 == raw.data.generated_sha256,"markings preserve collision hash: "+str(shown.get("error",{})))
        var view: Dictionary = DATA.view(shown.data.chunk)
        check(view.presentation.road_materials.size() == DATA.count(view),"exact presentation index alignment")
        check(view.presentation.road_styles.size() == 2,"marked ground and deliberately buried road")
        var text: Dictionary = JSON.parse_string(bridge.generate_chunk(x,0))
        check(text.ok,"JSON generation: "+str(text.get("error",{})))
        var text_shown: Dictionary = bridge.with_presentation(text.data)
        check(text_shown.ok,"JSON presentation: "+str(text_shown.get("error",{})))
        check(text_shown.data.chunk.presentation.road_materials == view.presentation.road_materials,"JSON and packed decoration agree")
        var cost: Dictionary = JSON.parse_string(bridge.estimate_chunk(x,0)).data
        var bytes: int = RENDERER.PLAN.estimate(view,int(cost.presentation_bytes))
        check(bytes>65536,"shader allocation included before rendering")
        var lease := Lease.new()
        var job := RENDERER.begin(shown.data.chunk,world,func(_bytes): return lease,bytes)
        while not RENDERER.advance(job): pass
        check(job.done and job.error.is_empty() and lease.sealed,"bounded material/mesh creation")
        jobs.append(job)
        leases.append(lease)
    var held: Shader = shader(jobs[0].root)
    check(held!=null and held!=shader(jobs[1].root),"cell shaders have independent ownership")
    RENDERER.cancel(jobs[0])
    await process_frame
    check(leases[0].alive(),"external shader retains display reservation")
    held=null
    for _i in range(4): await process_frame
    check(not leases[0].alive() and leases[1].alive(),"old cell retires while adjacent city cell stays visible")
    RENDERER.cancel(jobs[1])
    for _i in range(4): await process_frame
    check(not leases[1].alive(),"last shader and material retire")
    world.queue_free()
    await process_frame
    print("mapkit_urban: PASS")
    quit(0)
'''
