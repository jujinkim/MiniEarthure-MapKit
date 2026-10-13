extends "../diagnostics/fixed_world_capture.gd"
var failed := false
class BrokenBridge extends RefCounted:
	func cell_window(_x: int, _y: int) -> String: return '{"ok":true,"data":{"visual_margin_cm":0}}'
	func generate_chunk_packed(_x: int, _y: int) -> Dictionary: return {"ok":false,"error":"synthetic preparation failure"}

func check(value: bool, message: String) -> void:
	if not value: failed=true;push_error(message)

func run() -> void:
	check_preparation_costs()
	var zeros := METRICS.summary([0.0,0.0],true)
	check(zeros.status=="unavailable" and zeros.p95_ms==null,"zero GPU readings must be unavailable")
	check(METRICS.summary([0.0,2.0],true).status=="unavailable","partially missing GPU samples must not claim headroom")
	check(METRICS.summary([NAN],true).status=="unavailable","invalid readings must be unavailable")
	check(METRICS.verdict(METRICS.summary([])).fixed_scene=="unavailable","empty run cannot pass")
	check(METRICS.verdict(METRICS.summary([16.7])).fixed_scene=="pass","inclusive frame boundary")
	var hitch: Array=[]
	for index in 180: hitch.append(10.0 if index<178 else 34.0)
	var verdict := METRICS.verdict(METRICS.summary(hitch))
	check(verdict.p95_pass and not verdict.p99_pass and verdict.driving=="not_measured","p99 failures and driving status are independent")
	var saved := {}
	for key in ["SOURCE","OUTPUT","QUALITY","WIDTH","REPEATS","CAPTURE","THEME","OCCLUSION"]:
		var name: String="DEFAULT_WORLD_"+key
		saved[name]=OS.get_environment(name) if OS.has_environment(name) else null
		OS.unset_environment(name)
	OS.set_environment("DEFAULT_WORLD_SOURCE","/synthetic-source");OS.set_environment("DEFAULT_WORLD_OUTPUT","/synthetic-output")
	check(not options().has("error"),"default options accepted")
	for pair in [["QUALITY","medium"],["WIDTH","0"],["REPEATS","0"],["CAPTURE","yes"],["THEME","../escape"],["OCCLUSION","2"]]:
		OS.set_environment("DEFAULT_WORLD_"+pair[0],pair[1])
		check(options().has("error"),"reject malformed option "+pair[0])
		OS.unset_environment("DEFAULT_WORLD_"+pair[0])
	var config := options()
	config.wanted=PackedStringArray(["typo"])
	check(selected_views({"review_views":[{"name":"first"}]},config).is_empty(),"unknown view cannot silently pass")
	for key: String in saved:
		if saved[key]==null: OS.unset_environment(key)
		else: OS.set_environment(key,saved[key])
	var view := {"name":"failure","position_m":[16,5,16],"target_m":[16,0,0],"view_distance_m":384}
	var meta := {"size":[32,32]}
	var doc := {"environment":SCENE.PROFILE.defaults()}
	doc.environment.start_minutes=840
	for timed_out in [false,true]:
		var scene := SCENE.new()
		var ticks := [0,2]
		if timed_out: scene.clock=func(): return ticks.pop_front() if ticks.size()>1 else ticks[0]
		var cache := SCENE.CACHE.new()
		var result: Dictionary=await scene.prepare(self,BrokenBridge.new(),meta,doc,view,cache,lighting,1 if timed_out else 1000)
		check(not result.get("ok",false) and str(result.error).contains("timeout" if timed_out else "synthetic"),"prepare failure/timeout remains explicit")
		scene.release()
		for frame in 8: await process_frame
		check(scene.live_resources()==0 and cache.bytes()==0,"partial scene resources returned after failure/timeout")
	print("render_measurement_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)

func check_preparation_costs() -> void:
	var first := SCENE.new()
	var cache := SCENE.CACHE.new()
	first.cache=cache
	var context := {"cell":[2,3],"asset_id":"facade"}
	first.record_cost("near/mesh_upload",9000,context)
	context.cell[0]=99
	first.record_cost("near/mesh_upload",1000,context)
	var cost: Dictionary=first.preparation_stages["near/mesh_upload"]
	check(cost.samples==2 and cost.total_usec==10000 and cost.peak_usec==9000 and cost.peak_context.cell==[2,3],"phase aggregate preserves the slowest call and detached context")
	cache.preparation_usec={"lod":100000}
	cache.preparation_peak_usec={"lod":100000}
	first.record_cache({},context)
	var revisit := SCENE.new();revisit.cache=cache
	var before: Dictionary=cache.preparation_usec.duplicate()
	revisit.record_cache(before,context)
	check(revisit.preparation_stages.is_empty(),"reused templates cannot inherit lifetime LOD costs or peaks")
	cache.preparation_usec.lod+=2000
	revisit.record_cache(before,context)
	cost=revisit.preparation_stages["cache/lod"]
	check(cost.total_usec==2000 and cost.peak_usec==2000 and cost.samples==1,"new revisit work measures only its own operation")
	check(cache.preparation_peak_usec.lod==100000 and cache.preparation_usec.lod==102000,"observation preserves shared lifetime diagnostics")
	first.cache=null;revisit.cache=null;cache.shutdown()
