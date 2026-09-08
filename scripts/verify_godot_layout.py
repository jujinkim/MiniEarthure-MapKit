#!/usr/bin/env python3
"""Probe relocatable native bindings without loading a renderer or game repository."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from check_input_defense import asset_package
import struct
import zlib
from road_probe import PROBE as ROAD_PROBE
from spatial_terrain_probe import make_fixture as make_terrain_fixture, PROBE as TERRAIN_PROBE

ROOT = Path(__file__).resolve().parents[1]
PROBE = '''extends SceneTree
var failures: Array[String] = []
func check(ok: bool, description: String) -> void:
    if not ok:
        failures.append(description)
        push_error(description)
func _initialize() -> void:
    if not ClassDB.class_exists("MapKitBridge"):
        push_error("MapKitBridge not registered at nested path")
        quit(1)
        return
    var bytes := FileAccess.get_file_as_bytes("res://fixture.memap")
    var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
    var opened: Dictionary = JSON.parse_string(bridge.open_package_bytes(bytes))
    check(opened.ok, "open acquired package bytes")
    if not opened.ok:
        quit(1)
        return
    var peak: int = int(opened.data.validation_peak_bytes)
    check(opened.data.retained_memory_bytes > 0 and peak >= opened.data.retained_memory_bytes, "inspection exposes working-set estimates")
    var rejected: Dictionary = JSON.parse_string(bridge.open_package_bytes_budgeted(bytes, peak - 1))
    check(not rejected.ok and rejected.error.code == "E_MEMORY_BUDGET", "native validation memory gate")
    check(not JSON.parse_string(bridge.generate_chunk(0, 0)).ok, "budget failure clears prior package")
    check(JSON.parse_string(bridge.open_package_bytes_budgeted(bytes, peak)).ok, "native budgeted retry")
    var bad_bytes := FileAccess.get_file_as_bytes("res://invalid-metadata.memap")
    var metadata_rejected: Dictionary = JSON.parse_string(bridge.open_package_bytes(bad_bytes))
    check(not metadata_rejected.ok and metadata_rejected.error.code == "E_PROVENANCE", "independent invalid provenance rejected at native package boundary")
    check(not JSON.parse_string(bridge.generate_chunk(0, 0)).ok, "metadata failure clears previously open package")
    check(JSON.parse_string(bridge.open_package_bytes(bytes)).ok, "valid unknown producer retry after metadata failure")
    var bad_container := FileAccess.get_file_as_bytes("res://invalid-container.memap")
    var container_rejected: Dictionary = JSON.parse_string(bridge.open_package_bytes_budgeted(bad_container, peak))
    check(not container_rejected.ok and container_rejected.error.code == "E_ZIP", "local ZIP header identity must match central inventory before inflation")
    check(not JSON.parse_string(bridge.generate_chunk(0, 0)).ok, "container failure clears previously open package")
    check(JSON.parse_string(bridge.open_package_bytes_budgeted(bytes, peak)).ok, "retry after container rejection preserves the same budget")
    var bad_asset := FileAccess.get_file_as_bytes("res://invalid-asset.memap")
    var asset_rejected: Dictionary = JSON.parse_string(bridge.open_package_bytes(bad_asset))
    check(not asset_rejected.ok and asset_rejected.error.code == "E_ASSET", "honest inventory with truncated PNG body fails full native decode")
    check(not JSON.parse_string(bridge.generate_chunk(0, 0)).ok, "asset failure exposes no previously opened source")
    check(JSON.parse_string(bridge.open_package_bytes_budgeted(bytes, peak)).ok, "native retry after late asset rejection")
    var document: Dictionary = JSON.parse_string(bridge.document_json()).data
    var duplicate_json: String = JSON.stringify(document).insert(1, '"map_id":"duplicate",')
    var duplicate_document: Dictionary = JSON.parse_string(bridge.validate_document(duplicate_json))
    check(not duplicate_document.ok and duplicate_document.error.code == "E_JSON", "native editor document rejects duplicate keys before engine-number normalization")
    document.provenance.last_edited = "tomorrow"
    var invalid_document: Dictionary = JSON.parse_string(bridge.validate_document(JSON.stringify(document)))
    check(not invalid_document.ok and invalid_document.error.code == "E_PROVENANCE", "editor document validation shares metadata contract")
    var overview_cost: Dictionary = JSON.parse_string(bridge.overview_cost())
    check(overview_cost.ok, "overview allocation counts")
    var overview: Dictionary = JSON.parse_string(bridge.overview_json(int(overview_cost.data.json_bytes)))
    check(overview.ok and overview.data.version == 1 and overview.data.roads.size() == 2, "compact overview geometry through native boundary")
    check(not overview.data.has("provenance") and not overview.data.has("heightmaps"), "overview omits editor metadata")
    check(not JSON.parse_string(bridge.overview_json(int(overview_cost.data.json_bytes) - 1)).ok, "overview size gate before serialization")
    var estimate: Dictionary = JSON.parse_string(bridge.estimate_chunk(0, 0))
    check(estimate.ok, "estimate without materializing a chunk")
    check(not JSON.parse_string(bridge.estimate_chunk(-1, 0)).ok, "estimate rejects outside cell")
    var first: Dictionary = JSON.parse_string(bridge.generate_chunk(0, 0))
    check(first.ok, "generate initial chunk")
    check(estimate.data.triangles >= first.data.chunk.triangles.size() and estimate.data.objects >= first.data.chunk.objects.size(), "estimated output bounds actual geometry")
    var packed: Dictionary = bridge.generate_chunk_packed(0, 0)
    check(packed.ok and packed.data.generated_sha256 == first.data.generated_sha256, "packed view preserves generated hash")
    var data = load("res://addons/outer_runtime/mapkit/chunk_data.gd")
    var c: Dictionary = packed.data.chunk
    check(c.packed_version == 1 and not c.has("triangles"), "no per-triangle Dictionary allocation")
    c.merge(c.geometry.view())
    check(c.vertices_cm is PackedInt64Array and c.vertices_cm.size() == first.data.chunk.triangles.size() * 9, "exact packed coordinates")
    for i in first.data.chunk.triangles.size():
        var t: Dictionary = first.data.chunk.triangles[i]
        check(data.object_id(c, i) == t.object_id and data.object_id(first.data.chunk, i) == t.object_id, "public triangle identity adapter matches packed and JSON paths")
        check(["asphalt", "concrete", "dirt", "gravel", "grass"][c.surface_indices[i]] == t.surface, "surface preserved")
        check(c.object_ids[c.object_indices[i]] == t.object_id and bool(c.spawnable[i]) == t.spawnable, "triangle identity and spawnability preserved")
        for v in 3:
            for axis in 3:
                check(c.vertices_cm[i * 9 + v * 3 + axis] == int(t.vertices[v][axis]), "integer centimetres preserved")
    var altered: PackedInt64Array = c.vertices_cm
    var original: int = c.vertices_cm[0]
    altered[0] += 999
    check(c.geometry.view().vertices_cm[0] == original, "native owner isolates mutations in consumer views")
    check(not bridge.generate_chunk_packed(-1, 0).ok, "packed generation rejects invalid cell")
    var query: Dictionary = JSON.parse_string(bridge.query_cells(51199, 100, 51199, 100, 2))
    check(query.ok and query.data.geometry_cells.size() == 1 and query.data.occupancy_cells.size() == 2, "query includes neighbor tree owner counts")
    if query.ok and query.data.geometry_cells.size() == 1 and query.data.occupancy_cells.size() == 2:
        check(int(query.data.geometry_cells[0].x) == 0 and int(query.data.geometry_cells[0].y) == 0 and int(query.data.occupancy_cells[0].x) == 0 and int(query.data.occupancy_cells[0].y) == 0 and int(query.data.occupancy_cells[1].x) == 1 and int(query.data.occupancy_cells[1].y) == 0, "query includes neighbor tree owner coordinates")
    check(not JSON.parse_string(bridge.query_cells(51199, 100, 51199, 100, 1)).ok, "query union enforces cell cap")
    check(not JSON.parse_string(bridge.query_cells(0, 0, 1, 1, -1)).ok, "negative query allowance rejected")
    check(not JSON.parse_string(bridge.query_cells(2, 0, 1, 1, 4)).ok, "reversed query bounds rejected")
    var seam: Dictionary = JSON.parse_string(bridge.query_cells(51200, 51200, 51200, 51200, 4))
    check(seam.ok and seam.data.geometry_cells.size() == 4, "closed corner includes all four cells")
    var occupied: Dictionary = bridge.generate_chunk_occupied_packed(0, 0, 2)
    check(occupied.ok and occupied.data.generated_sha256 == first.data.generated_sha256, "occupied package generation preserves world hash")
    var solids: Dictionary = occupied.data.occupancy.view()
    check(solids.occupancy_version == 1 and solids.shape_kinds == PackedByteArray([1, 1]), "building remains two triangular prisms")
    check(solids.shape_values_cm is PackedInt64Array and solids.shape_values_cm.size() == 16, "fixed eight-integer solid records")
    check(estimate.data.occupied_solids >= solids.shape_kinds.size(), "native occupancy planning bounds sidecar count")
    check(solids.object_ids[solids.object_indices[0]] == "building-1", "occupied object identity")
    var changed_solids: PackedInt64Array = solids.shape_values_cm
    var original_solid: int = changed_solids[0]
    changed_solids[0] += 77
    check(occupied.data.occupancy.view().shape_values_cm[0] == original_solid, "occupied owner isolates consumer mutation")
    check(not bridge.generate_chunk_occupied_packed(0, 0, 1).ok, "solid count limit fails without partial output")
    check(not bridge.generate_chunk_occupied_packed(0, 0, -1).ok and not bridge.generate_chunk_occupied_packed(0, 0, 200001).ok, "invalid allowances rejected")
    check(not bridge.generate_chunk_occupied_packed(-1, 0, 2).ok, "occupied path rejects invalid cell")
    for x in 2:
        for y in 2:
            var archive_info: Dictionary = JSON.parse_string(bridge.chunk_archive_info(x, y))
            check(archive_info.ok, "archive identity and allocation limit")
            var saved: Dictionary = bridge.generate_chunk_archived(x, y, archive_info.data.max_bytes)
            check(saved.ok and saved.data.has("archive"), "native bounded archive generation")
            var loaded: Dictionary = bridge.restore_chunk_archive(x, y, saved.data.archive)
            check(loaded.ok and loaded.data.generated_sha256 == saved.data.generated_sha256, "archive roundtrip canonical hash")
            check(loaded.data.chunk.geometry.view() == saved.data.chunk.geometry.view() and loaded.data.chunk.objects == saved.data.chunk.objects, "archive preserves complete layered geometry and orchard placements")
            check(not bridge.restore_chunk_archive(x + 1, y, saved.data.archive).ok, "archive coordinate gate")
            var declined: Dictionary = bridge.generate_chunk_archived(x, y, 1)
            check(declined.ok and not declined.data.has("archive"), "archive cap preserves generated output")
    var orchard: Dictionary = bridge.generate_chunk_occupied_packed(1, 1, 1000)
    check(orchard.ok, "occupied orchard generation")
    var trunks: Dictionary = orchard.data.occupancy.view()
    check(trunks.shape_kinds.size() > 0, "orchard solid fixture is nonempty")
    for i in trunks.shape_kinds.size():
        check(trunks.shape_kinds[i] == 0 and trunks.shape_values_cm[i * 8 + 6] == 0 and trunks.shape_values_cm[i * 8 + 7] == 0, "box tag and reserved zero fields")
        check(trunks.shape_values_cm[i * 8 + 4] - trunks.shape_values_cm[i * 8 + 1] == 400, "trunk full height preserved")
    var window: Dictionary = JSON.parse_string(bridge.cell_window(102400, 102400))
    check(window.ok and window.data.cells.size() == 4, "map-edge 3x3 contains existing cells only")
    check(window.data.cell.x == 1 and window.data.cell.y == 1, "maximum edge belongs to last cell")
    check(window.data.world_scale == 0.125, "public scale contract")
    check(window.data.recipe_version == 1, "window reports the opened recipe-1 source, not the current recipe")
    check(window.data.generated_format_version == 6, "public generated contract")
    var outside: Dictionary = JSON.parse_string(bridge.cell_window(-1, 0))
    check(not outside.ok and outside.error.code == "E_CELL", "outside-map request rejected")
    check(bridge.map_bounds() == PackedInt64Array([0, 0, 102400, 102400]), "exact bounds without document serialization")
    var contact: Dictionary = JSON.parse_string(bridge.surface_probe(25600, 25600, "bridge-road"))
    check(contact.ok and contact.data.position_cm[1] == 700 and PackedInt32Array(contact.data.normal_q) == PackedInt32Array([0, 1000000, 0]) and contact.data.is_road and not contact.data.blocked_by_building, "bridge geometric probe: " + str(contact))
    var building_contact: Dictionary = JSON.parse_string(bridge.surface_probe(31000, 31000, "terrain"))
    check(building_contact.ok and building_contact.data.blocked_by_building, "terrain beneath building is explicitly obstructed")
    check(not JSON.parse_string(bridge.surface_probe(31000, 31000, "building-1")).ok, "roof probe rejects non-drivable geometry")
    var options: Dictionary = JSON.parse_string(bridge.spawn_options(25600, 25600))
    check(options.ok and options.data.surfaces.size() == 3, "ground, bridge and terrain options at crossing")
    check(options.data.surfaces.all(func(item): return item.surface_id != "building-1"), "roofs never become automatic spawn surfaces")
    bytes.fill(0)
    var source := FileAccess.open("res://fixture.memap", FileAccess.WRITE)
    source.store_buffer(bytes)
    source.close()
    var second: Dictionary = JSON.parse_string(bridge.generate_chunk(0, 0))
    check(second.ok and first.data.generated_sha256 == second.data.generated_sha256, "acquired package remains independent of caller bytes and source file")
    var corrupt: RefCounted = ClassDB.instantiate("MapKitBridge")
    check(not JSON.parse_string(corrupt.open_package_bytes(bytes)).ok, "corrupt acquired bytes rejected")
    print("mapkit_nested_binding: " + ("PASS" if failures.is_empty() else str(failures)))
    quit(0 if failures.is_empty() else 1)
'''


RENDER_PROBE = '''extends SceneTree
const RENDERER = preload("res://addons/outer_runtime/mapkit/chunk_renderer.gd")
func _initialize() -> void:
    run.call_deferred()
func run() -> void:
    var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
    assert(JSON.parse_string(bridge.open_package(ProjectSettings.globalize_path("res://fixture.memap"))).ok)
    var generated: Dictionary = bridge.generate_chunk_packed(1, 1)
    var parent := Node3D.new()
    root.add_child(parent)
    var job := RENDERER.begin(generated.data.chunk, parent)
    var steps := 0
    while not job.done:
        var before := int(job.triangle)
        RENDERER.advance(job)
        if int(job.triangle) > before:
            assert(int(job.triangle) - before <= RENDERER.TRIANGLES_PER_BATCH)
        steps += 1
    assert(steps > 2 and job.root.get_child_count() > 0)
    RENDERER.cancel(job)
    assert(RENDERER.advance(job) and parent.get_child_count() == 0)
    var cancelled := RENDERER.begin(generated.data.chunk, parent)
    RENDERER.advance(cancelled)
    RENDERER.cancel(cancelled)
    assert(RENDERER.advance(cancelled) and parent.get_child_count() == 0)
    var triangles: Array = []
    for i in range(RENDERER.TRIANGLES_PER_BATCH * 2 + 1):
        triangles.append({"surface": "grass", "vertices": [[0, 0, 0], [100, 0, 0], [0, 0, 100]]})
    var shared := RENDERER.begin({"cell": {"x": 0, "y": 0}, "triangles": triangles, "objects": []}, parent)
    while not RENDERER.advance(shared): pass
    assert(shared.root.get_child_count() == 3 and shared.materials.is_empty())
    var material_id: int = shared.root.get_child(0).material_override.get_instance_id()
    var material_ref: WeakRef = weakref(shared.root.get_child(0).material_override)
    for mesh in shared.root.get_children(): assert(mesh.material_override.get_instance_id() == material_id)
    RENDERER.cancel(shared)
    await process_frame
    await process_frame
    assert(material_ref.get_ref() == null)
    parent.queue_free()
    await process_frame
    print("mapkit_incremental_renderer: PASS")
    quit(0)
'''


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--godot', required=True)
    parser.add_argument('--probe', choices=['renderer', 'binding', 'terrain', 'roads'], action='append', help='Run only selected behavioral probes; default runs all')
    args = parser.parse_args()
    library = {'win32': 'mapkit_godot.dll', 'darwin': 'libmapkit_godot.dylib'}.get(sys.platform, 'libmapkit_godot.so')
    built_library = ROOT / 'target/debug' / library
    if not built_library.exists():
        raise SystemExit('Build mapkit-godot for this native platform before the probe.')
    with tempfile.TemporaryDirectory(prefix='mapkit-layout-') as directory:
        project = Path(directory)
        addon = project / 'addons/outer_runtime/mapkit'
        addon.mkdir(parents=True)
        shutil.copyfile(ROOT / 'mapkit.gdextension', addon / 'mapkit.gdextension')
        (addon / 'target/debug').mkdir(parents=True)
        shutil.copyfile(built_library, addon / 'target/debug' / library)
        (project / 'project.godot').write_text('config_version=5\n[application]\nconfig/name="MapKit Nested Probe"\n[rendering]\nrenderer/rendering_method="gl_compatibility"\n')
        (project / 'probe.gd').write_text(PROBE)
        (project / 'renderer_probe.gd').write_text(RENDER_PROBE)
        (project / 'terrain_probe.gd').write_text(TERRAIN_PROBE)
        (project / 'road_probe.gd').write_text(ROAD_PROBE)
        subprocess.run([sys.executable, str(ROOT / 'examples/third_party.py'), str(project / 'roads.memap'), str(ROOT / 'examples/roads/document.json')], check=True, timeout=30)
        make_terrain_fixture(project)
        for name in ('chunk_renderer.gd', 'chunk_data.gd'):
            shutil.copyfile(ROOT / 'godot' / name, addon / name)
        subprocess.run(['cargo', 'run', '--quiet', '--locked', '--manifest-path', str(ROOT / 'Cargo.toml'), '-p', 'mapkit-cli', '--', 'pack', str(ROOT / 'examples/minimal'), str(project / 'fixture.memap')], check=True, timeout=60)
        inconsistent = bytearray((project / 'fixture.memap').read_bytes())
        inconsistent[30] ^= 1  # first physical filename; central manifest is unchanged
        (project / 'invalid-container.memap').write_bytes(inconsistent)
        ihdr = struct.pack('>IIBBBBB', 2, 2, 8, 6, 0, 0, 0)
        invalid_png = b'\x89PNG\r\n\x1a\n' + struct.pack('>I', 13) + b'IHDR' + ihdr + struct.pack('>I', zlib.crc32(b'IHDR' + ihdr))
        (project / 'invalid-asset.memap').write_bytes(asset_package((project / 'fixture.memap').read_bytes(), invalid_png))
        invalid = json.loads((ROOT / 'examples/minimal/document.json').read_text())
        invalid['provenance']['last_edited'] = 'tomorrow'
        (project / 'invalid-document.json').write_text(json.dumps(invalid))
        subprocess.run([sys.executable, str(ROOT / 'examples/third_party.py'), str(project / 'invalid-metadata.memap'), str(project / 'invalid-document.json')], check=True, timeout=30)
        subprocess.run([args.godot, '--headless', '--import', '--frame-delay', '1000', '--path', str(project)], check=True, timeout=60)
        scripts = {'renderer': 'renderer_probe.gd', 'binding': 'probe.gd', 'terrain': 'terrain_probe.gd', 'roads': 'road_probe.gd'}
        fixture_bytes = (project / 'fixture.memap').read_bytes()
        for probe in args.probe or scripts:
            # Binding deliberately corrupts its source to prove snapshot ownership.
            # Each process gets a fresh fixture even when probes run out of order.
            (project / 'fixture.memap').write_bytes(fixture_bytes)
            subprocess.run([args.godot, '--headless', '--path', str(project), '--script', 'res://' + scripts[probe]], check=True, timeout=30)


if __name__ == '__main__':
    main()
