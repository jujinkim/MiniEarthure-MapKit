#!/usr/bin/env python3
"""Probe relocatable native bindings without loading a renderer or game repository."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

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
    var estimate: Dictionary = JSON.parse_string(bridge.estimate_chunk(0, 0))
    check(estimate.ok, "estimate without materializing a chunk")
    check(not JSON.parse_string(bridge.estimate_chunk(-1, 0)).ok, "estimate rejects outside cell")
    var first: Dictionary = JSON.parse_string(bridge.generate_chunk(0, 0))
    check(first.ok, "generate initial chunk")
    check(estimate.data.triangles >= first.data.chunk.triangles.size() and estimate.data.objects >= first.data.chunk.objects.size(), "estimated output bounds actual geometry")
    var window: Dictionary = JSON.parse_string(bridge.cell_window(102400, 102400))
    check(window.ok and window.data.cells.size() == 4, "map-edge 3x3 contains existing cells only")
    check(window.data.cell.x == 1 and window.data.cell.y == 1, "maximum edge belongs to last cell")
    check(window.data.world_scale == 0.125, "public scale contract")
    check(window.data.generated_format_version == 6, "public generated contract")
    var outside: Dictionary = JSON.parse_string(bridge.cell_window(-1, 0))
    check(not outside.ok and outside.error.code == "E_CELL", "outside-map request rejected")
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
    var generated: Dictionary = JSON.parse_string(bridge.generate_chunk(1, 1))
    var parent := Node3D.new()
    root.add_child(parent)
    var job := RENDERER.begin(generated.data.chunk, parent)
    var steps := 0
    while not job.done:
        var before := int(job.triangle)
        RENDERER.advance(job)
        if int(job.triangle) > before:
            assert(int(job.triangle) - before <= 128)
        steps += 1
    assert(steps > 2 and job.root.get_child_count() > 0)
    RENDERER.cancel(job)
    assert(RENDERER.advance(job) and parent.get_child_count() == 0)
    var cancelled := RENDERER.begin(generated.data.chunk, parent)
    RENDERER.advance(cancelled)
    RENDERER.cancel(cancelled)
    assert(RENDERER.advance(cancelled) and parent.get_child_count() == 0)
    parent.queue_free()
    await process_frame
    print("mapkit_incremental_renderer: PASS")
    quit(0)
'''


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--godot', required=True)
    args = parser.parse_args()
    library = 'mapkit_godot.dll' if sys.platform == 'win32' else 'libmapkit_godot.so'
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
        shutil.copyfile(ROOT / 'godot/chunk_renderer.gd', addon / 'chunk_renderer.gd')
        subprocess.run(['cargo', 'run', '--quiet', '--locked', '--manifest-path', str(ROOT / 'Cargo.toml'), '-p', 'mapkit-cli', '--', 'pack', str(ROOT / 'examples/minimal'), str(project / 'fixture.memap')], check=True, timeout=60)
        subprocess.run([args.godot, '--headless', '--import', '--frame-delay', '1000', '--path', str(project)], check=True, timeout=60)
        subprocess.run([args.godot, '--headless', '--path', str(project), '--script', 'res://renderer_probe.gd'], check=True, timeout=30)
        subprocess.run([args.godot, '--headless', '--path', str(project), '--script', 'res://probe.gd'], check=True, timeout=30)


if __name__ == '__main__':
    main()
