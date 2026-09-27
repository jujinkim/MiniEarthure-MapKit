extends Node
## One immutable worker at a time; cancellation never publishes a late result.
signal completed(request: int, result: Dictionary)
var generation := 0
var _thread: Thread
var _token: RefCounted
var _running := 0
var _pending := {}

func begin(settings: Dictionary, destination: String) -> int:
	cancel()
	_pending = {"settings": settings.duplicate(true), "destination": destination, "request": generation}
	return generation

func cancel() -> void:
	generation += 1
	_pending.clear()
	if _token != null: _token.cancel()

func busy() -> bool:
	return _thread != null or not _pending.is_empty()

func _process(_delta: float) -> void:
	if _thread != null:
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
	_thread = Thread.new()
	if _thread.start(_generate.bind(request, _token)) != OK:
		_thread = null
		completed.emit(_running, {"ok":false,"error":{"code":"E_THREAD","message":"Cannot start track generation"}})

static func _generate(request: Dictionary, token: RefCounted) -> Dictionary:
	token.enter()
	var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
	var result: Dictionary = JSON.parse_string(bridge.generate_track(JSON.stringify(request.settings), request.destination))
	token.leave()
	return result

func _exit_tree() -> void:
	cancel()
	if _thread != null: _thread.wait_to_finish()
