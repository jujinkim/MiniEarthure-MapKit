extends RefCounted
## Static GLB sign surface: unit UVs, embedded PNG, no fonts/URLs at consumption.
## This authors bytes only. Callers MUST pass the result through native asset
## validation before adopting/displaying it; PNG contents are not trusted here.
static func encode(png: PackedByteArray, width_cm: int, height_cm: int) -> PackedByteArray:
	if png.is_empty() or png.size()>16*1024*1024 or width_cm<10 or width_cm>2000 or height_cm<10 or height_cm>1000: return PackedByteArray()
	var w := width_cm/200.0
	var h := height_cm/100.0
	# Source-facing -Y maps to glTF +Z. Rotation stays a placement choice.
	var positions := PackedFloat32Array([-w,0,.01,w,0,.01,w,h,.01,-w,h,.01]).to_byte_array()
	var normals := PackedFloat32Array([0,0,1,0,0,1,0,0,1,0,0,1]).to_byte_array()
	var uv := PackedFloat32Array([0,1,1,1,1,0,0,0]).to_byte_array()
	var indices := PackedInt32Array([0,1,2,0,2,3]).to_byte_array()
	var binary := positions+normals+uv+indices+png
	var image_offset := positions.size()+normals.size()+uv.size()+indices.size()
	var doc := {"asset":{"version":"2.0","generator":"mapkit-sign-surface-v1"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],
		"meshes":[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2},"indices":3,"material":0,"mode":4}]}],
		"buffers":[{"byteLength":binary.size()}],"bufferViews":[
			{"buffer":0,"byteOffset":0,"byteLength":positions.size(),"target":34962},
			{"buffer":0,"byteOffset":positions.size(),"byteLength":normals.size(),"target":34962},
			{"buffer":0,"byteOffset":positions.size()+normals.size(),"byteLength":uv.size(),"target":34962},
			{"buffer":0,"byteOffset":positions.size()+normals.size()+uv.size(),"byteLength":indices.size(),"target":34963},
			{"buffer":0,"byteOffset":image_offset,"byteLength":png.size()}],
		"accessors":[{"bufferView":0,"componentType":5126,"count":4,"type":"VEC3","min":[-w,0,.01],"max":[w,h,.01]},
			{"bufferView":1,"componentType":5126,"count":4,"type":"VEC3"},
			{"bufferView":2,"componentType":5126,"count":4,"type":"VEC2"},
			{"bufferView":3,"componentType":5125,"count":6,"type":"SCALAR"}],
		"images":[{"bufferView":4,"mimeType":"image/png"}],"samplers":[{"magFilter":9729,"minFilter":9987,"wrapS":33071,"wrapT":33071}],
		"textures":[{"sampler":0,"source":0}],"materials":[{"doubleSided":true,"pbrMetallicRoughness":{"baseColorTexture":{"index":0},"metallicFactor":0,"roughnessFactor":.9}}]}
	var json := JSON.stringify(doc).to_utf8_buffer()
	while json.size()%4: json.append(32)
	while binary.size()%4: binary.append(0)
	var result := PackedByteArray()
	result.resize(12);result.encode_u32(0,0x46546c67);result.encode_u32(4,2);result.encode_u32(8,28+json.size()+binary.size())
	var header := PackedByteArray();header.resize(8)
	header.encode_u32(0,json.size());header.encode_u32(4,0x4e4f534a)
	result.append_array(header);result.append_array(json)
	header.encode_u32(0,binary.size());header.encode_u32(4,0x004e4942)
	result.append_array(header);result.append_array(binary)
	return result
