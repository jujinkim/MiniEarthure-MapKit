extends SceneTree
## Every default-world backdrop builds a bounded, display-only, seeded stage
## outside the venue floor; an absent or rc_arena backdrop keeps the RC venue.
const STAGE := preload("../track_stage.gd")
const BACKDROP := preload("../backdrop_stage.gd")
var failed := false
func check(ok: bool, message: String) -> void:
	if not ok: failed=true;push_error(message)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	var native: RefCounted = ClassDB.instantiate("MapKitBridge")
	var settings: Dictionary = JSON.parse_string(native.track_catalogue()).data.defaults
	settings.seed=4;settings.length="short";settings.categories=["driving"]
	var file := ProjectSettings.globalize_path("user://backdrop.memap")
	var generated: Dictionary = JSON.parse_string(native.generate_track(JSON.stringify(settings),file))
	check(generated.ok,"generated fixture")
	if not generated.ok: quit(1);return
	var assembly: Dictionary = generated.data.document.assembled_track
	var floor_lo := Vector2(assembly.floor.min_cm[0],-assembly.floor.max_cm[2])*0.01
	var floor_hi := Vector2(assembly.floor.max_cm[0],-assembly.floor.min_cm[2])*0.01
	for backdrop: String in ["", "rc_arena"]:
		var arena := STAGE.create(PackedInt64Array(),assembly,backdrop)
		check(arena.has_meta("rc_venue_theme") and not arena.has_meta("backdrop"),"RC venue kept for '%s'" % backdrop)
		arena.free()
	for backdrop: String in BACKDROP.THEMES:
		var stage := STAGE.create(PackedInt64Array(),assembly,backdrop)
		root.add_child(stage)
		check(stage.name == "ToyTrackStage" and stage.get_meta("backdrop") == backdrop,"themed stage "+backdrop)
		var models := 0
		var pending: Array[Node] = [stage]
		while not pending.is_empty():
			var node: Node = pending.pop_back()
			check(not node is CollisionObject3D,"display-only "+backdrop)
			pending.append_array(node.get_children())
		for child: Node in stage.get_children():
			if child is Node3D and child.scene_file_path.ends_with(".glb"):
				models += 1
				var at := Vector2(child.position.x,child.position.z)
				check(at.x < floor_lo.x or at.x > floor_hi.x or at.y < floor_lo.y or at.y > floor_hi.y,"%s model outside the venue floor at %s" % [backdrop,at])
		check(models > 8 and models <= BACKDROP.MAX_SCATTER + BACKDROP.MAX_BUILDINGS + 3,"%s places a bounded model set (%d)" % [backdrop,models])
		for id: String in BACKDROP.THEMES[backdrop].landmarks + BACKDROP.THEMES[backdrop].buildings + BACKDROP.THEMES[backdrop].scatter:
			check(BACKDROP.model(backdrop,id) != null,"%s model %s loads" % [backdrop,id])
		var again := STAGE.create(PackedInt64Array(),assembly,backdrop)
		check(again.get_child_count() == stage.get_child_count() and (again.get_child(5) as Node3D).transform == (stage.get_child(5) as Node3D).transform,"seeded placement "+backdrop)
		again.free()
		stage.free()
	if not failed: print("backdrop_stage_validator: PASS")
	quit(1 if failed else 0)
