extends SceneTree
## Packed point lookup must agree with the authoritative window at every edge.
var failed := false
func check(ok: bool, message: String) -> void:
	if not ok: failed=true; push_error(message)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	var native: RefCounted = ClassDB.instantiate("MapKitBridge")
	var regional: RefCounted = ClassDB.instantiate("MapKitRegionReader")
	for bridge: RefCounted in [native,regional]: check(bridge.cell_at(0,0).is_empty(),"unopened lookup is empty")
	var source := ProjectSettings.globalize_path(get_script().resource_path.get_base_dir().path_join("../../examples/minimal"))
	check(JSON.parse_string(native.open_project(source)).ok,"synthetic project opened")
	var path := ProjectSettings.globalize_path("user://cell-query.mkregions")
	check(JSON.parse_string(regional.export_project(source,path,1)).ok,"synthetic regional export")
	check(JSON.parse_string(regional.open_index(path,64*1024*1024,"")).ok,"regional index opened")
	for x in [-1,0,51199,51200,102399,102400]:
		for y in [-1,0,51199,51200,102399,102400]:
			for bridge: RefCounted in [native,regional]:
				var window: Dictionary = JSON.parse_string(bridge.cell_window(x,y))
				var point: PackedInt32Array = bridge.cell_at(x,y)
				check(point.size()==2 if window.ok else point.is_empty(),"point/window admission agrees")
				if window.ok:
					check(point==PackedInt32Array([window.data.cell.x,window.data.cell.y]),"packed point agrees at cell/map edges")
	print("cell_query_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
