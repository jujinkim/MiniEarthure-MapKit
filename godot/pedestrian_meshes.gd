extends RefCounted
## Voxel box figures of the street-life field (street_life_field.gd), drawn by
## pedestrian.gdshader: a near walker of six boxes (legs, torso, arms, head),
## a far walker of three (legs, torso with arms, head) and a pigeon of five
## (body, head, tail, two wings). A walker is one unit tall (the field scales
## it to game scale); a pigeon is in metres. UV carries each vertex's limb and
## colour slot and UV2 its pivot (height, x), as exact floats (vertex COLOR is
## not exact on every renderer); the shader swings the limb about its pivot
## and picks the slot colour from the instance palette. Display only; no
## collision.

const SHADER := preload("./pedestrian.gdshader")
## Limbs the shader moves.
const STILL := 0
const LEFT_LEG := 1
const RIGHT_LEG := 2
const LEFT_ARM := 3
const RIGHT_ARM := 4
const LEFT_WING := 5
const RIGHT_WING := 6
## Colour slots: the instance top and bottom palette entries, then fixed
## tones. The shader draws hair on the top of a SKIN box and shoes at the foot
## of a BOTTOM box (by unit height), and the pigeon's BEAK slot as its dark
## head with a pale beak tip.
const TOP := 0
const BOTTOM := 1
const SKIN := 2
const HAIR := 3
const SHOE := 4
const FEATHER := 5
const WING := 6
const BEAK := 7
## Hip and shoulder pivots of a walker (share of its height).
const HIP := 0.47
const SHOULDER := 0.79

## [min corner, max corner, limb, pivot height, pivot x, slot] per box.
const NEAR := [
	[Vector3(-0.105, 0.0, -0.055), Vector3(-0.01, HIP, 0.055), LEFT_LEG, HIP, 0.0, BOTTOM],
	[Vector3(0.01, 0.0, -0.055), Vector3(0.105, HIP, 0.055), RIGHT_LEG, HIP, 0.0, BOTTOM],
	[Vector3(-0.14, HIP, -0.08), Vector3(0.14, SHOULDER + 0.01, 0.08), STILL, 0.0, 0.0, TOP],
	[Vector3(-0.21, HIP + 0.02, -0.045), Vector3(-0.14, SHOULDER, 0.045), LEFT_ARM, SHOULDER, 0.0, TOP],
	[Vector3(0.14, HIP + 0.02, -0.045), Vector3(0.21, SHOULDER, 0.045), RIGHT_ARM, SHOULDER, 0.0, TOP],
	[Vector3(-0.085, SHOULDER + 0.01, -0.085), Vector3(0.085, 1.0, 0.085), STILL, 0.0, 0.0, SKIN]]
const FAR := [
	[Vector3(-0.105, 0.0, -0.055), Vector3(0.105, HIP, 0.055), STILL, 0.0, 0.0, BOTTOM],
	[Vector3(-0.2, HIP, -0.08), Vector3(0.2, SHOULDER + 0.01, 0.08), STILL, 0.0, 0.0, TOP],
	[Vector3(-0.085, SHOULDER + 0.01, -0.085), Vector3(0.085, 1.0, 0.085), STILL, 0.0, 0.0, SKIN]]
## A pigeon facing -z, 0.12 m long (game scale), standing on its origin.
const PIGEON := [
	[Vector3(-0.028, 0.02, -0.045), Vector3(0.028, 0.065, 0.04), STILL, 0.0, 0.0, FEATHER],
	[Vector3(-0.017, 0.055, -0.072), Vector3(0.017, 0.088, -0.04), STILL, 0.0, 0.0, BEAK],
	[Vector3(-0.02, 0.035, 0.04), Vector3(0.02, 0.05, 0.075), STILL, 0.0, 0.0, WING],
	[Vector3(-0.075, 0.05, -0.03), Vector3(-0.028, 0.058, 0.035), LEFT_WING, 0.055, -0.028, WING],
	[Vector3(0.028, 0.05, -0.03), Vector3(0.075, 0.058, 0.035), RIGHT_WING, 0.055, 0.028, WING]]
static var _meshes: Dictionary = {}
## Bound once when the scripts load, so the shader compiles outside a streaming step.
static var _warm_material: ShaderMaterial = _warm()

static func _warm() -> ShaderMaterial:
	var warm := ShaderMaterial.new()
	warm.shader = SHADER
	return warm

## Shared meshes (near, far, pigeon); built once per process, display only.
static func shared() -> Dictionary:
	if _meshes.is_empty():
		_meshes = {"near": mesh(NEAR, "mk_walker_near"), "far": mesh(FAR, "mk_walker_far"), "pigeon": mesh(PIGEON, "mk_pigeon")}
	return _meshes

## Boxes of `parts` as one indexed surface with per-face normals.
static func mesh(parts: Array, label: String) -> ArrayMesh:
	var vertices := PackedVector3Array()
	var normals := PackedVector3Array()
	var uv := PackedVector2Array()
	var uv2 := PackedVector2Array()
	var indices := PackedInt32Array()
	for part: Array in parts:
		var low: Vector3 = part[0]
		var high: Vector3 = part[1]
		var role := Vector2(float(part[2]), float(part[5]))
		var pivot := Vector2(float(part[3]), float(part[4]))
		for axis in 3:
			for sign in [-1.0, 1.0]:
				var normal := Vector3.ZERO
				normal[axis] = sign
				var u := (axis + 1) % 3
				var v := (axis + 2) % 3
				var first := vertices.size()
				for corner in [Vector2(0, 0), Vector2(1, 0), Vector2(1, 1), Vector2(0, 1)]:
					var p := Vector3.ZERO
					p[axis] = high[axis] if sign > 0.0 else low[axis]
					p[u] = high[u] if corner.x > 0.0 else low[u]
					p[v] = high[v] if corner.y > 0.0 else low[v]
					vertices.append(p)
					normals.append(normal)
					uv.append(role)
					uv2.append(pivot)
				# Clockwise seen from outside (Godot front faces): (u, v) runs
				# counter-clockwise seen from +axis, so +axis faces reverse it.
				if sign > 0.0: indices.append_array([first, first + 2, first + 1, first, first + 3, first + 2])
				else: indices.append_array([first, first + 1, first + 2, first, first + 2, first + 3])
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	arrays[Mesh.ARRAY_NORMAL] = normals
	arrays[Mesh.ARRAY_TEX_UV] = uv
	arrays[Mesh.ARRAY_TEX_UV2] = uv2
	arrays[Mesh.ARRAY_INDEX] = indices
	var result := ArrayMesh.new()
	result.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	result.resource_name = label
	return result

## Boxes per mesh (validators).
static func box_counts() -> Dictionary:
	return {"near": NEAR.size(), "far": FAR.size(), "pigeon": PIGEON.size()}
