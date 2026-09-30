extends RefCounted
## Shared display-only cap and distance-based bands; never a support collider.
static func point(v: Array, scale := 0.01) -> Vector3:
	return Vector3(v[0], v[1], -v[2]) * scale

static func records(chunk: Dictionary) -> Array:
	return JSON.parse_string(chunk.grind_lines_json) if chunk.has("grind_lines_json") else chunk.get("grind_lines", [])

static func rows(line: Dictionary, clip: Dictionary = {}) -> Dictionary:
	var groups := [PackedVector3Array(), PackedVector3Array()]
	var path: Array = line.get("samples", [])
	var station := 0.0
	for i in range(1, path.size()):
		var a := point(path[i-1].position_cm)+point(path[i-1].normal,0.000001)*float(line.capture_height_cm)*0.01
		var b := point(path[i].position_cm)+point(path[i].normal,0.000001)*float(line.capture_height_cm)*0.01
		var n := point(path[i-1].normal, 0.000001).normalized()
		var r := (b-a).normalized().cross(n).normalized() * 0.06
		var length := a.distance_to(b)
		var interval := Vector2(0.0,1.0)
		if not clip.is_empty():
			for axis in [0,2]:
				var index := 0 if axis==0 else 1
				var low: float = float(clip.min[index])*0.01 if axis==0 else -float(clip.max[index])*0.01
				var high: float = float(clip.max[index])*0.01 if axis==0 else -float(clip.min[index])*0.01
				var d := b[axis]-a[axis]
				if absf(d)<0.000001:
					if a[axis]<low or a[axis]>=high:interval=Vector2(1,0)
				else:
					var t0 := (low-a[axis])/d
					var t1 := (high-a[axis])/d
					interval.x=maxf(interval.x,minf(t0,t1));interval.y=minf(interval.y,maxf(t0,t1))
		var start := interval.x*length
		var stop := interval.y*length
		while start < stop - 0.00001:
			var finish := minf(stop, start + 0.25 - fposmod(station + start, 0.25))
			if finish <= start + 0.00001: finish = minf(stop, start + 0.25)
			var x := a.lerp(b, start / length)
			var y := a.lerp(b, finish / length)
			var vertices: PackedVector3Array = groups[int(floor((station+start+0.00001)/0.25)) % 2]
			for q in [[x-r,y-r,y+r,x+r], [x-r-n*0.035,y-r-n*0.035,y-r,x-r], [x+r,y+r,y+r-n*0.035,x+r-n*0.035]]:
				for j in [0,2,1,0,3,2]: vertices.append(q[j])
			groups[int(floor((station+start+0.00001)/0.25)) % 2] = vertices
			start = finish
		station += length
	var result := {}
	for i in 2:
		result["grind:" + str(line.id) + ":" + str(i)] = {"vertices":groups[i], "owner":-1, "color":Color("24d9d0") if i == 0 else Color("eefcf9"), "emission":true, "lit":true, "pose":Transform3D.IDENTITY}
	return result

static func visual(line: Dictionary, lease: RefCounted = null, clip: Dictionary = {}) -> Node3D:
	var root := Node3D.new()
	root.set_meta("grind_line_id", line.id)
	if lease != null: lease.track(root)
	for row: Dictionary in rows(line,clip).values():
		if row.vertices.is_empty(): continue
		var mesh := ArrayMesh.new()
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = row.vertices
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		var material := StandardMaterial3D.new()
		material.albedo_color = row.color
		material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
		material.cull_mode = BaseMaterial3D.CULL_DISABLED
		var instance := MeshInstance3D.new()
		instance.mesh = mesh
		instance.material_override = material
		root.add_child(instance)
		if lease != null:
			lease.track(instance)
			lease.track(mesh)
			lease.track(material)
	return root
