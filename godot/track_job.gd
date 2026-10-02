extends Node
## One immutable worker at a time; cancellation never publishes a late result.
signal completed(request: int, result: Dictionary)
signal progressed(request: int, progress: Dictionary)
var generation := 0
var _thread: Thread
var _token: RefCounted
var _running := 0
var _pending := {}
var _progress_revision := -1

func begin(settings: Dictionary, destination: String, with_preview := false) -> int:
	cancel()
	_pending = {"settings": settings.duplicate(true), "destination": destination, "request": generation, "preview":with_preview}
	return generation

func cancel() -> void:
	generation += 1
	_pending.clear()
	if _token != null: _token.cancel()

func busy() -> bool:
	return _thread != null or not _pending.is_empty()

func _process(_delta: float) -> void:
	if _thread != null:
		_poll_progress()
		if _thread.is_alive(): return
		var result: Dictionary = _thread.wait_to_finish()
		_thread = null
		_token = null
		if _running == generation: completed.emit(_running, result)
		return
	if _pending.is_empty(): return
	var request := _pending
	_pending = {}
	_running = request.request
	_token = ClassDB.instantiate("MapKitWorkToken")
	_progress_revision = -1
	_poll_progress()
	_thread = Thread.new()
	if _thread.start(_generate.bind(request, _token)) != OK:
		_thread = null
		completed.emit(_running, {"ok":false,"error":{"code":"E_THREAD","message":"Cannot start track generation"}})

func _poll_progress() -> void:
	if _token == null or _running != generation or _token.is_cancelled(): return
	var value: Dictionary = JSON.parse_string(_token.progress_json())
	if int(value.revision) == _progress_revision: return
	_progress_revision = int(value.revision)
	progressed.emit(_running, value)

static func _generate(request: Dictionary, token: RefCounted) -> Dictionary:
	token.enter()
	var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
	var result: Dictionary = JSON.parse_string(bridge.generate_track(JSON.stringify(request.settings), request.destination))
	if result.get("ok",false) and request.get("preview",false) and not token.is_cancelled():
		var prepared:Dictionary=preload("./track_authoring_preview.gd").prepare(result.data.document,bridge,{},token)
		if prepared.has("error"): result={"ok":false,"error":{"code":"E_PREVIEW","message":prepared.error}}
		else: result.data.preview=prepared
	token.leave()
	return result

func _exit_tree() -> void:
	cancel()
	if _thread != null: _thread.wait_to_finish()
