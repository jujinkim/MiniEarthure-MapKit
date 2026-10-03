extends RefCounted
## Geometry preparation is worker-safe; only apply() touches scene nodes/resources.
const GIMMICK := preload("./gimmick_geometry.gd")
static var preparations := 0
static var mesh_builds := 0

static func point(v: Array) -> Vector3:
	return Vector3(float(v[0]),float(v[1]),-float(v[2]))*0.01

static func prepare(document: Dictionary, bridge: RefCounted = null, previous: Dictionary = {}, token: RefCounted = null) -> Dictionary:
	if bridge == null: bridge = ClassDB.instantiate("MapKitBridge")
	if token != null: token.report_progress("preview",0,0,"objects")
	var result: Dictionary = JSON.parse_string(bridge.track_preview(JSON.stringify(document)))
	if not result.ok: return {"error":result.error.message}
	var objects := {}
	var gimmicks := {}
	var assembly: Dictionary = document.get("assembled_track", {})
	var completed := 0
	var total: int = result.data.meshes.size()+result.data.gimmicks.size()+result.data.get("grind_lines",[]).size()
	for id: String in result.data.meshes:
		if token != null and token.is_cancelled(): return {"error":"Track preview cancelled."}
		if token != null: token.report_progress("preview",completed,total,"objects")
		completed += 1 # Published when the next object starts, after this work finishes.
		var vertices := PackedVector3Array()
		for v: Array in result.data.meshes[id]: vertices.append(point(v))
		if vertices.is_empty(): continue
		var owner := -1
		if id.begins_with("assembled-road-") or id.begins_with("assembled-wall-"): owner = int(id.get_slice("-", 2))
		var key := "rc:wall" if id.begins_with("assembled-wall") or id.begins_with("assembled-shell") or id.begins_with("assembled-support") else "rc:floor" if id == "assembled-venue-floor" else "rc:road:"+str(maxi(0,owner)%4)
		objects[id] = {"vertices":vertices, "owner":owner, "color":Color("b9c9d0") if key=="rc:wall" else Color("448fac"), "lit":true, "material_key":key, "seed":int(assembly.get("settings",{}).get("seed",0)), "pose":Transform3D.IDENTITY}
	for raw: Dictionary in result.data.gimmicks:
		if token != null and token.is_cancelled(): return {"error":"Track preview cancelled."}
		if token != null: token.report_progress("preview",completed,total,"objects")
		completed += 1
		var owner := _gimmick_owner(raw.id, assembly)
		var cached: Dictionary = previous.get("gimmicks", {}).get(raw.id, {})
		if cached.get("source") == raw and cached.get("owner") == owner:
			objects.merge(cached.rows)
			gimmicks[raw.id] = cached
			continue
		var rows := {}
		var g := GIMMICK.resolved(raw)
		if g.is_empty(): return {"error":"Could not prepare track attachment geometry."}
		var groups: Array = []
		if g.has("track_mesh"):
			for role: String in ["inner", "shell"]: groups.append(GIMMICK.triangles(g.track_mesh[role]))
		for part: Dictionary in g.parts:
			var points := GIMMICK.points(part)
			var vertices := PackedVector3Array()
			for face: Array in part.faces:
				for i in 3: vertices.append(points[face[i]])
			groups.append(vertices)
		var color := Color8(g.color[0], g.color[1], g.color[2], g.color[3])
		var emission: bool = g.motion.kind in ["boost", "launch", "target_speed", "jump_height", "air_ring"]
		var panel := GIMMICK.panel_style(g)
		for i in groups.size():
			rows[g.id + ":%d" % i] = {"vertices":groups[i], "owner":owner, "color":color, "emission":emission, "lit":true, "pose":GIMMICK.pose(g, 0)}
			if not panel.is_empty(): rows[g.id + ":%d" % i]["panel"] = panel
		if g.motion.kind == "air_ring":
			var top := 0.0
			for part: Dictionary in g.parts:
				for v: Array in part.vertices: top = maxf(top, float(v[1])*0.01)
			var pose := GIMMICK.pose(g, 0)
			pose.origin += pose.basis * Vector3(0, float(g.effect.ring_radius_cm)*0.01+0.15 if g.motion.kind == "air_ring" else top+0.012, 0)
			rows[g.id + ":arrow"] = {"vertices":PackedVector3Array([Vector3(-0.35,0,0.25),Vector3(0,0,-0.55),Vector3(0.35,0,0.25)]), "owner":owner, "color":Color.WHITE, "emission":true, "lit":true, "pose":pose}
		objects.merge(rows)
		gimmicks[raw.id] = {"source":raw, "owner":owner, "rows":rows}
	for line: Dictionary in result.data.get("grind_lines", []):
		objects.merge(preload("./grind_geometry.gd").rows(line))
		completed += 1
		if token != null: token.report_progress("preview",completed,total,"objects")
	if token != null: token.report_progress("preview",completed,total,"objects")
	completed = 0
	if token != null: token.report_progress("preview_meshes",0,objects.size(),"objects")
	for id: String in objects:
		if token != null and token.is_cancelled(): return {"error":"Track preview cancelled."}
		if token != null: token.report_progress("preview_meshes",completed,objects.size(),"objects")
		completed += 1
		var entry: Dictionary = objects[id]
		if entry.has("signature"): continue # Immutable prior worker result.
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = entry.vertices
		if entry.get("lit", false):
			var normals := PackedVector3Array()
			for i in range(0, entry.vertices.size(), 3):
				var n: Vector3 = (entry.vertices[i+2]-entry.vertices[i]).cross(entry.vertices[i+1]-entry.vertices[i]).normalized()
				for j in 3: normals.append(n)
			arrays[Mesh.ARRAY_NORMAL] = normals
		entry.arrays = arrays
		entry.erase("vertices")
		entry.signature = var_to_bytes(entry).hex_encode().sha256_text()
	if token != null: token.report_progress("preview_meshes",completed,objects.size(),"objects")
	return {"objects":objects, "gimmicks":gimmicks}

static func _gimmick_owner(id: String, assembly: Dictionary) -> int:
	if id.begins_with("track-obstacle-"):
		var index := int(id.trim_prefix("track-obstacle-"))
		return int(assembly.obstacles[index].piece_index)
	if id.begins_with("track-"): return int(id.get_slice("-", 1))
	# Boost chains create multiple panels belonging to the same action.
	var source: Dictionary = assembly.authoring if assembly.get("authoring") is Dictionary else assembly.seed_source if assembly.get("seed_source") is Dictionary else {}
	var best_owner := -1
	var best_length := -1
	for action: Dictionary in source.get("actions", []):
		var prefix := "action-" + str(action.id)
		if (id == prefix or id.begins_with(prefix + "-")) and prefix.length() > best_length:
			for i in source.instances.size():
				if source.instances[i].id == action.piece:
					best_owner = i
					best_length = prefix.length()
	return best_owner

static func apply(root: Node3D, prepared: Dictionary, selected := -1) -> void:
	if prepared.has("error"): return
	clear_draft(root)
	if not root.has_meta("material_context"): root.set_meta("material_context",preload("./environment_materials.gd").new())
	var existing: Dictionary = root.get_meta("objects", {})
	for id: String in existing.keys():
		if not prepared.objects.has(id):
			var old: Node = existing[id]
			root.remove_child(old)
			old.queue_free()
			existing.erase(id)
	for id: String in prepared.objects:
		var entry: Dictionary = prepared.objects[id]
		if existing.has(id) and existing[id].get_meta("signature") == entry.signature:
			existing[id].transform = entry.pose
			existing[id].set_meta("owner", entry.owner)
			existing[id].show()
			continue
		if existing.has(id):
			root.remove_child(existing[id])
			existing[id].queue_free()
		var mesh := ArrayMesh.new()
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, entry.arrays)
		mesh_builds += 1
		var view := MeshInstance3D.new()
		view.mesh = mesh
		view.transform = entry.pose
		view.set_meta("signature", entry.signature)
		view.set_meta("owner", entry.owner)
		view.set_meta("color", entry.color)
		view.set_meta("road", id.begins_with("assembled-road-"))
		var material := StandardMaterial3D.new()
		material.albedo_color = entry.color
		material.roughness = 0.72
		if not entry.get("lit", false): material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
		if entry.get("emission", false):
			material.emission_enabled = true
			material.emission = entry.color * 0.35
		material.cull_mode = BaseMaterial3D.CULL_DISABLED
		view.material_override = GIMMICK.panel_material(entry.panel) if entry.has("panel") else material
		if entry.has("material_key"):
			var rc := preload("./rc_venue.gd").material(entry.material_key,entry.seed)
			root.get_meta("material_context").bind_detail(rc,"concrete" if entry.material_key=="rc:wall" else "asphalt")
			view.material_override = rc
			view.set_meta("color",rc.get_shader_parameter("base_color"))
		root.add_child(view)
		existing[id] = view
	root.set_meta("objects", existing)
	select(root, selected)

static func select(root: Node3D, selected: int) -> void:
	if not is_instance_valid(root): return
	for view: MeshInstance3D in root.get_meta("objects", {}).values() + root.get_meta("draft_objects", {}).values():
		var color: Color = Color("f5ce5f") if view.get_meta("road") and view.get_meta("owner") == selected else view.get_meta("color")
		if view.material_override is ShaderMaterial:
			if view.get_meta("road", false): view.material_override.set_shader_parameter("base_color",color)
		else: view.material_override.albedo_color = color

static func create(document: Dictionary, selected := -1) -> Node3D:
	var root := Node3D.new()
	root.name = "TrackAuthoringPreview"
	preparations += 1
	apply(root, prepare(document), selected)
	return root

# Draft display only: paths come from track_instance, never from a second compiler.
# Stable IDs map the validated owners to current indices across additions/deletions.
static func source(document: Dictionary) -> Dictionary:
	var assembly: Dictionary = document.get("assembled_track", {})
	for field in ["authoring", "seed_source"]:
		if assembly.get(field) is Dictionary: return assembly[field]
	return {}

static func frame(sample: Dictionary) -> Transform3D:
	var forward := point(sample.forward).normalized()
	var normal := point(sample.normal).normalized()
	return Transform3D(Basis(forward.cross(normal).normalized(), normal, -forward), point(sample.position_cm))

static func shape(item: Dictionary) -> Dictionary:
	var value := item.duplicate(true)
	for field in ["position_cm", "rotation_mdeg"]: value.erase(field)
	return value

static func clear_draft(root: Node3D) -> void:
	for node: Node3D in root.get_meta("draft_objects", {}).values():
		root.remove_child(node)
		node.queue_free()
	root.set_meta("draft_objects", {})
	if root.has_meta("draft_guides"):
		var guides: Node = root.get_meta("draft_guides")
		root.remove_child(guides)
		guides.queue_free()
		root.remove_meta("draft_guides")
	root.set_meta("draft_pending", false)
	for node: Node3D in root.get_meta("objects", {}).values():
		if node.has_meta("validated_pose"):
			node.transform = node.get_meta("validated_pose")
			node.set_meta("owner", node.get_meta("validated_owner"))
			node.remove_meta("validated_pose")
			node.remove_meta("validated_owner")
		node.show()

static func apply_draft(root: Node3D, draft: Dictionary, pieces: Array, validated: Dictionary, selected := -1) -> void:
	var original: Array = source(validated).get("instances", [])
	var base_pieces: Array = validated.get("assembled_track", {}).get("pieces", [])
	var indices := {}
	for i in draft.get("instances", []).size(): indices[str(draft.instances[i].id)] = i
	var transforms := {}
	for owner in original.size():
		var index := int(indices.get(str(original[owner].id), -1))
		if index < 0 or owner >= base_pieces.size() or pieces[index].path.is_empty(): continue
		if shape(original[owner]) != shape(draft.instances[index]): continue
		transforms[owner] = {"index":index, "pose":frame(pieces[index].path[0]) * frame(base_pieces[owner].path[0]).affine_inverse()}
	var reusable := {}
	var attachments_same := true
	for field in ["actions", "attachments"]:
		attachments_same = attachments_same and draft.get(field, []) == source(validated).get(field, [])
	for object_id: String in root.get_meta("objects", {}):
		var node: Node3D = root.get_meta("objects")[object_id]
		if not node.has_meta("validated_pose"):
			node.set_meta("validated_pose", node.transform)
			node.set_meta("validated_owner", node.get_meta("owner", -1))
		var owner := int(node.get_meta("validated_owner"))
		node.hide() # Junctions, supports and ground wait for the validated revision.
		if object_id.begins_with("grind:") and draft.get("grind_lines", []) == validated.get("grind_lines", []): node.show()
		if not transforms.has(owner): continue
		var index: int = transforms[owner].index
		if not attachments_same and not node.get_meta("road", false): continue
		node.set_meta("owner", index)
		node.transform = transforms[owner].pose * node.get_meta("validated_pose")
		node.show()
		if node.get_meta("road", false): reusable[str(original[owner].id)] = true
	var overlays: Dictionary = root.get_meta("draft_objects", {})
	var wanted := {}
	for i in pieces.size():
		var id := str(draft.instances[i].id)
		if reusable.has(id): continue
		var path: Array = pieces[i].path
		if path.size() < 2: continue
		wanted[id] = true
		var signature := JSON.stringify(path)
		var node: MeshInstance3D = overlays.get(id)
		if node == null:
			node = MeshInstance3D.new()
			var material := StandardMaterial3D.new()
			material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
			material.cull_mode = BaseMaterial3D.CULL_DISABLED
			node.material_override = material
			root.add_child(node)
			overlays[id] = node
		if node.get_meta("path", "") != signature:
			var mesh := ImmediateMesh.new()
			mesh.surface_begin(Mesh.PRIMITIVE_LINES)
			for j in range(1, path.size()):
				for side in [-1.0, 1.0]:
					var a := frame(path[j - 1])
					var b := frame(path[j])
					mesh.surface_add_vertex(a.origin + a.basis.x * side * float(path[j - 1].lateral_cm) * 0.01)
					mesh.surface_add_vertex(b.origin + b.basis.x * side * float(path[j].lateral_cm) * 0.01)
				mesh.surface_add_vertex(point(path[j - 1].position_cm))
				mesh.surface_add_vertex(point(path[j].position_cm))
			mesh.surface_end()
			node.mesh = mesh
			node.set_meta("path", signature)
		node.material_override.albedo_color = Color("f5ce5f") if i == selected else Color("65cce0")
		node.set_meta("owner", i)
		node.set_meta("road", true)
		node.set_meta("color", Color("65cce0"))
	for id: String in overlays.keys():
		if not wanted.has(id):
			root.remove_child(overlays[id])
			overlays[id].queue_free()
			overlays.erase(id)
	root.set_meta("draft_objects", overlays)
	_draft_guides(root, draft, pieces, indices, reusable, attachments_same, validated)
	root.set_meta("draft_pending", true)
	select(root, selected)

static func _draft_guides(root: Node3D, draft: Dictionary, pieces: Array, indices: Dictionary, reusable: Dictionary, attachments_same: bool, validated: Dictionary) -> void:
	var vertices := PackedVector3Array()
	for action: Dictionary in draft.get("actions", []):
		if attachments_same and reusable.has(str(action.piece)): continue
		var index := int(indices.get(str(action.piece), -1))
		if index < 0 or int(action.sample) < 0 or int(action.sample) >= pieces[index].path.size(): continue
		var pose := frame(pieces[index].path[int(action.sample)])
		for axis in [pose.basis.x, pose.basis.z]:
			vertices.append(pose.origin - axis * 0.5 + pose.basis.y * 0.1)
			vertices.append(pose.origin + axis * 0.5 + pose.basis.y * 0.1)
	for attachment: Dictionary in draft.get("attachments", []):
		if attachments_same and reusable.has(str(attachment.piece)): continue
		var index := int(indices.get(str(attachment.piece), -1))
		if index < 0: continue
		var path: Array = pieces[index].path
		var station := 0.0
		for i in range(1, path.size()):
			var a := point(path[i-1].position_cm)
			var b := point(path[i].position_cm)
			var length := a.distance_to(b)
			var target := float(attachment.station_cm) * 0.01
			if target >= station and target <= station + length:
				var center := a.lerp(b, (target-station)/maxf(length,0.00001))
				var up := frame(path[i]).basis.y
				vertices.append(center); vertices.append(center + up)
				break
			station += length
	if draft.get("grind_lines", []) != validated.get("grind_lines", []):
		# Authored control polygon is deliberately a guide until native preparation.
		for line: Dictionary in draft.get("grind_lines", []):
			for i in range(1, line.control_points.size()):
				vertices.append(point(line.control_points[i-1]))
				vertices.append(point(line.control_points[i]))
	var node: MeshInstance3D = root.get_meta("draft_guides") if root.has_meta("draft_guides") else null
	if vertices.is_empty():
		if node != null: node.hide()
		return
	if node == null:
		node = MeshInstance3D.new()
		var material := StandardMaterial3D.new()
		material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
		material.albedo_color = Color("65cce0")
		node.material_override = material
		root.add_child(node)
		root.set_meta("draft_guides", node)
	var mesh := ImmediateMesh.new()
	mesh.surface_begin(Mesh.PRIMITIVE_LINES)
	for vertex in vertices: mesh.surface_add_vertex(vertex)
	mesh.surface_end()
	node.mesh = mesh
	node.show()
