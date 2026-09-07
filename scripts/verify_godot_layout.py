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
    var first: Dictionary = JSON.parse_string(bridge.generate_chunk(0, 0))
    check(first.ok, "generate initial chunk")
    var window: Dictionary = JSON.parse_string(bridge.cell_window(102400, 102400))
    check(window.ok and window.data.cells.size() == 4, "map-edge 3x3 contains existing cells only")
    check(window.data.cell.x == 1 and window.data.cell.y == 1, "maximum edge belongs to last cell")
    check(window.data.world_scale == 0.125, "public scale contract")
    check(window.data.generated_format_version == 6, "public generated contract")
    var outside: Dictionary = JSON.parse_string(bridge.cell_window(-1, 0))
    check(not outside.ok and outside.error.code == "E_CELL", "outside-map request rejected")
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
        subprocess.run(['cargo', 'run', '--quiet', '--locked', '--manifest-path', str(ROOT / 'Cargo.toml'), '-p', 'mapkit-cli', '--', 'pack', str(ROOT / 'examples/minimal'), str(project / 'fixture.memap')], check=True, timeout=60)
        subprocess.run([args.godot, '--headless', '--import', '--frame-delay', '1000', '--path', str(project)], check=True, timeout=60)
        subprocess.run([args.godot, '--headless', '--path', str(project), '--script', 'res://probe.gd'], check=True, timeout=30)


if __name__ == '__main__':
    main()
