extends SceneTree
const POI := preload("../poi_renderer.gd")
var failed := false
func check(ok: bool, message: String) -> void:
	if not ok: failed=true; push_error(message)
func _initialize() -> void:
	var font := ThemeDB.fallback_font
	var project := func(p: Array): return Vector2(p[0],p[1])
	var values := [{"position":[20,30],"name":"도서관"},{"position":[20,30],"name":"Duplicate"},{"position":[-1,0],"name":"Outside"},{"position":[80,130],"name":"Library"}]
	var entries := POI.layout(values,project,Rect2(0,0,400,200),font)
	check(entries.size()==2,"coincident pins and outside coordinates are culled")
	check(entries[0].label=="도서관","map-owned Korean label preserved")
	check(not entries[0].box.intersects(entries[1].box),"visible labels do not overlap")
	check(POI.layout([],project,Rect2(0,0,400,200),font).is_empty(),"empty maps have no pins")
	print("poi_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
