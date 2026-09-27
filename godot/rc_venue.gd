extends RefCounted
## One seeded visual vocabulary for Client, network worlds and Editor previews.
const SHADER := preload("./rc_surface.gdshader")
const NAMES := ["INDOOR RC CUP", "OUTDOOR RC CIRCUIT", "TOY TRACK CHALLENGE"]

static func theme(seed: int) -> int:
	return posmod(seed, 3)

static func palette(seed: int) -> Dictionary:
	return [
		{"floor":Color("243849"),"road":[Color("17657a"),Color("23516d"),Color("276c75"),Color("254763")],"accent":Color("ffb72e"),"barrier":Color("ee4545")},
		{"floor":Color("63933c"),"road":[Color("303a45"),Color("343c47"),Color("323c43"),Color("343a42")],"accent":Color("ffcf3f"),"barrier":Color("e44739")},
		{"floor":Color("bca97e"),"road":[Color("167edd"),Color("e86627"),Color("6b4bc4"),Color("159c86")],"accent":Color("ffe447"),"barrier":Color("f2ca32")}
	][theme(seed)]

static func material(key: String, seed: int) -> ShaderMaterial:
	var colors := palette(seed)
	var result := ShaderMaterial.new()
	result.shader = SHADER
	result.set_shader_parameter("theme", theme(seed))
	result.set_shader_parameter("wall", key == "rc:wall")
	result.set_shader_parameter("floor_surface", key == "rc:floor")
	result.set_shader_parameter("base_color", colors.floor if key == "rc:floor" else colors.road[int(key.get_slice(":",2)) % 4] if key.begins_with("rc:road:") else colors.barrier)
	result.set_shader_parameter("accent", colors.accent)
	return result
