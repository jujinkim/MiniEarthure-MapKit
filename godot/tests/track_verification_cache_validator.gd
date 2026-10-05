extends SceneTree
## Immutable-package reuse must never suppress candidate checks or survive reopen.
var failed:=false
func check(value:bool,label:String) -> void:
	if not value:failed=true;push_error(label)
func _initialize() -> void:run.call_deferred()
func run() -> void:
	var bridge:RefCounted=ClassDB.instantiate("MapKitBridge")
	var path:=OS.get_environment("MINIEARTHURE_TEST_MEMAP")
	check(JSON.parse_string(bridge.open_package(path)).ok,"fixture opens")
	var course:Dictionary=JSON.parse_string(bridge.courses_json()).data.courses[0]
	var text:=JSON.stringify(course)
	var started:=Time.get_ticks_usec()
	check(JSON.parse_string(bridge.verify_track(text)).ok,"stored package/course verification")
	var first:=Time.get_ticks_usec()-started
	started=Time.get_ticks_usec()
	check(JSON.parse_string(bridge.verify_track(text)).ok,"same immutable package verification")
	var reused:=Time.get_ticks_usec()-started
	print("TRACK_VERIFY first_usec=",first," reused_usec=",reused)
	var changed:=course.duplicate(true);changed.definition.checkpoints[0].position_cm[0]+=10
	check(not JSON.parse_string(bridge.verify_track(JSON.stringify(changed))).ok,"changed candidate still rejected after warm verification")
	var modified:=OS.get_environment("TRACK_MODIFIED_MEMAP")
	var bytes:=FileAccess.get_file_as_bytes(modified)
	for mode in range(5):
		check(JSON.parse_string(bridge.open_package(path)).ok,"valid reopen")
		check(JSON.parse_string(bridge.verify_track(text)).ok,"valid document verified afresh")
		var opened:Dictionary
		match mode:
			0:opened=JSON.parse_string(bridge.open_package(modified))
			1:opened=JSON.parse_string(bridge.open_package_budgeted(modified,1073741824))
			2:opened=JSON.parse_string(bridge.open_package_bytes(bytes))
			3:opened=JSON.parse_string(bridge.open_package_bytes_budgeted(bytes,1073741824))
			4:opened=JSON.parse_string(bridge.open_project(modified.get_basename()))
		check(opened.ok,"modified but structurally valid package opens")
		var rejected:Dictionary=JSON.parse_string(bridge.verify_track(text))
		check(not rejected.ok and rejected.error.code in ["E_TRACK_COURSE","E_COURSE_HASH"],"reopen invalidates successful document verification "+str(mode))
	check(not JSON.parse_string(bridge.open_package_bytes(PackedByteArray())).ok,"failed open")
	check(not JSON.parse_string(bridge.verify_track(text)).ok,"failed open cannot reuse verified document")
	print("track_verification_cache_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
