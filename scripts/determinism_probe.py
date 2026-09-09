"""Golden native binding/cache audit. Fixtures and expectations are public MIT."""
PROBE = '''extends SceneTree
const DATA = preload("res://addons/outer_runtime/mapkit/chunk_data.gd")
var failures: Array[String] = []
func check(ok: bool, message: String) -> void:
    if not ok:
        failures.append(message)
        push_error(message)
func _initialize() -> void: run.call_deferred()
func run() -> void:
    var vectors: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://determinism-vectors.json"))
    var packages := {"recipe1":"fixture", "recipe2":"roads", "recipe3":"placement", "recipe4":"assets"}
    var count := 0
    for fixture: Dictionary in vectors.fixtures:
        if not packages.has(fixture.name): continue
        var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
        var opened: Dictionary = JSON.parse_string(bridge.open_package(ProjectSettings.globalize_path("res://" + packages[fixture.name] + ".memap")))
        check(opened.ok, fixture.name + " package opens")
        if not opened.ok: continue
        var cells: Array = fixture.cells.duplicate()
        cells.reverse()
        for expected: Dictionary in cells:
            var x := int(expected.cell.x)
            var y := int(expected.cell.y)
            var raw: Dictionary = JSON.parse_string(bridge.generate_chunk(x,y))
            var packed: Dictionary = bridge.generate_chunk_occupied_packed(x,y,200000)
            check(raw.ok and packed.ok, "cold/occupied generation")
            if not raw.ok or not packed.ok: continue
            check(raw.data.generated_sha256 == expected.generated_sha256 and packed.data.generated_sha256 == expected.generated_sha256, fixture.name + str(expected.cell) + " core/JSON/packed golden")
            var original: Dictionary = DATA.view(packed.data.chunk)
            var info: Dictionary = JSON.parse_string(bridge.chunk_archive_info(x,y))
            var saved: Dictionary = bridge.generate_chunk_archived(x,y,info.data.max_bytes)
            check(saved.ok and saved.data.has("archive"), "bounded archive exists")
            if not saved.ok or not saved.data.has("archive"): continue
            var restored: Dictionary = bridge.restore_chunk_archive(x,y,saved.data.archive)
            check(restored.ok and restored.data.generated_sha256 == expected.generated_sha256, "warm archive golden")
            check(restored.data.chunk.geometry.view() == packed.data.chunk.geometry.view() and restored.data.chunk.objects == packed.data.chunk.objects, "all warm/cold geometry and instances equal")
            var decorated: Dictionary = bridge.with_presentation(restored.data)
            check(decorated.ok and decorated.data.generated_sha256 == expected.generated_sha256, "warm presentation preserves golden")
            check(decorated.data.chunk.geometry.view() == packed.data.chunk.geometry.view(), "decoration preserves physical view")
            for probe: Dictionary in expected.surface_probes:
                var point: Array = probe.position_cm
                # Query the selected source surface through the package owner after cache work.
                var selected: Dictionary = JSON.parse_string(bridge.surface_probe(int(point[0]),int(point[2]),probe.surface_id))
                check(selected.ok and selected.data.position_cm == point, "spawn query remains equal after cache/decoration")
            var changed: Dictionary = restored.data.chunk.geometry.view()
            changed.vertices_cm[0] += 999
            check(packed.data.chunk.geometry.view().vertices_cm[0] == original.vertices_cm[0], "warm view cannot mutate cold physical owner")
            var regenerated: Dictionary = bridge.generate_chunk_packed(x,y)
            check(regenerated.ok and regenerated.data.generated_sha256 == expected.generated_sha256, "queries and view edits do not seed generation")
            count += 1
    check(count == 16, "four recipes and four cells actually checked")
    print("mapkit_determinism: " + ("PASS (16 core/JSON/packed/cache goldens, reverse cells, spawn, display isolation)" if failures.is_empty() else str(failures)))
    quit(0 if failures.is_empty() else 1)
'''
