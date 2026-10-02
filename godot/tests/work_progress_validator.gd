extends SceneTree
var failures: Array[String] = []
func check(ok: bool, label: String) -> void:
	if not ok: failures.append(label)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	var job := preload("../track_job.gd").new()
	root.add_child(job)
	job.set_process(false)
	var events: Array = []
	job.progressed.connect(func(id: int,p: Dictionary): events.append([id,p]))
	job.generation = 3
	job._running = 3
	job._token = ClassDB.instantiate("MapKitWorkToken")
	job._poll_progress()
	check(events.size()==1 and events[0][1].total==null,"unknown preparation total")
	job._token.report_progress("saving",37,100,"bytes")
	job._poll_progress(); job._poll_progress()
	check(events.size()==2 and events[-1][1].completed==37,"actual count and revision deduplication")
	var ring := preload("../work_progress.gd").new()
	root.add_child(ring)
	ring.set_progress(events[-1][1])
	check(is_equal_approx(ring.fraction(),.37),"stage fraction")
	job._token.report_progress("preview",0,0,"objects")
	job._poll_progress(); ring.set_progress(events[-1][1])
	check(ring.fraction()<0,"new unknown stage clears percent")
	job.cancel(); job._poll_progress()
	check(events.size()==3,"cancelled job cannot publish progress")
	job._token=ClassDB.instantiate("MapKitWorkToken")
	job._poll_progress()
	check(events.size()==3,"old request cannot publish replacement token")
	ring.reduce_motion=true
	var phase: float=ring.phase
	ring._process(1)
	check(ring.phase==phase,"reduced motion retains static progress")
	ring.queue_free();job.queue_free()
	await process_frame
	print(JSON.stringify({"validator":"work_progress","failures":failures}))
	quit(0 if failures.is_empty() else 1)
