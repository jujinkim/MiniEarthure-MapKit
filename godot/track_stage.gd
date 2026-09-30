extends Node3D
## Bounded display-only RC venue. No collision, occupancy or spawn metadata.
const VENUE := preload("./rc_venue.gd")

static func point(v: Array) -> Vector3:
	return Vector3(v[0],v[1],-v[2])*0.01

static func paint(color: Color) -> StandardMaterial3D:
	var material := StandardMaterial3D.new()
	material.albedo_color = color
	material.roughness = 0.75
	return material

static func box(parent: Node3D, at: Vector3, size: Vector3, material: Material) -> MeshInstance3D:
	var node := MeshInstance3D.new()
	var shape := BoxMesh.new()
	shape.size = size
	node.mesh = shape
	node.material_override = material
	node.position = at
	parent.add_child(node)
	return node

static func label(parent: Node3D, title: String, at: Vector3, size: float, color: Color) -> Label3D:
	var node := Label3D.new()
	node.text = title
	node.font_size = 96
	node.pixel_size = size/96.0
	node.modulate = color
	node.outline_size = 5
	node.position = at
	parent.add_child(node)
	return node

static func create(_bounds: PackedInt64Array, assembly: Dictionary) -> Node3D:
	var root := Node3D.new()
	root.name = "ToyTrackStage"
	var seed := int(assembly.settings.seed)
	var theme := VENUE.theme(seed)
	root.set_meta("rc_venue_theme",theme)
	var colors := VENUE.palette(seed)
	var white := paint(Color("f7f6eb"))
	var black := paint(Color("17222d"))
	var accent := paint(colors.accent)
	var red := paint(colors.barrier)
	var lo := Vector3(INF,INF,INF)
	var hi := Vector3(-INF,-INF,-INF)
	for piece: Dictionary in assembly.pieces:
		for sample: Dictionary in piece.path:
			var p := point(sample.position_cm)
			lo = lo.min(p)
			hi = hi.max(p)
	var floor_y := float(assembly.floor.min_cm[1])*0.01
	var floor_a := point(assembly.floor.min_cm)
	var floor_b := point(assembly.floor.max_cm)
	var center := (floor_a+floor_b)*0.5
	var extent := (floor_b-floor_a).abs()
	box(root,Vector3(center.x,floor_y-0.25,center.z),Vector3(extent.x,0.5,extent.z),VENUE.material("rc:floor",seed))
	for side in [-1,1]:
		box(root,Vector3(center.x,floor_y+0.65,center.z+side*(extent.z/2-1)),Vector3(extent.x-2,1.3,0.25),black)
		box(root,Vector3(center.x+side*(extent.x/2-1),floor_y+0.65,center.z),Vector3(0.25,1.3,extent.z-2),black)
	if theme==0:
		box(root,Vector3(center.x,floor_y+4.5,lo.z-15),Vector3(extent.x,9,0.4),paint(Color("435768")))
		for side in [-1,1]:
			box(root,Vector3(center.x+side*(extent.x/2),floor_y+4.5,center.z),Vector3(0.4,9,extent.z),paint(Color("435768")))
			for i in 5: box(root,Vector3(center.x+side*(extent.x/2-0.2),floor_y+4.5,lo.z-12+i*extent.z/5),Vector3(0.4,9,0.5),black)
	for i in 6:
		var x := center.x-extent.x*0.35+extent.x*0.7*float(i)/5.0
		box(root,Vector3(x,floor_y+2.0,lo.z-11),Vector3(7,2,0.2),accent if i%2==0 else red)
		label(root,["RC CUP", "RACE • BUILD • REPEAT", "MINIEARTHURE"][i%3],Vector3(x,floor_y+2.05,lo.z-10.85),0.52,Color("17222d"))
	var first: Dictionary = assembly.pieces[2]
	var start := point(first.path[first.path.size()/2].position_cm)
	var width := float(first.connection_width_cm)*0.01
	var gate := Node3D.new()
	root.add_child(gate)
	gate.position = start
	gate.rotation.y = -float(first.quarter_turns)*PI/2.0
	for side in [-1,1]:
		box(gate,Vector3(side*(width/2+0.45),1.3,0),Vector3(0.3,2.6,0.4),accent)
	box(gate,Vector3(0,2.6,0),Vector3(width+1.2,0.6,0.4),black)
	label(gate,"MINIEARTHURE  RC",Vector3(0,2.65,0.23),0.42,Color.WHITE)
	for row in 2:
		for col in 16:
			box(gate,Vector3(-width/2+width*(float(col)+0.5)/16.0,0.012,-0.25+float(row)*0.5),Vector3(width/16.0,0.015,0.5),white if (row+col)%2==0 else black)
	for row in 4:
		for side in [-1,1]:
			box(gate,Vector3(side*width*0.24,0.013,2.5+row*2.3),Vector3(0.95,0.015,0.08),white)
	for i in 4:
		var pit := Vector3(lo.x-7,floor_y,center.z-7.5+i*5)
		box(root,pit+Vector3(0,0.75,0),Vector3(3,0.12,2),white)
		for side in [-1,1]: box(root,pit+Vector3(side*1.2,0.35,0),Vector3(0.12,0.7,1.6),black)
		box(root,pit+Vector3(0,2.15,0),Vector3(3.8,0.18,3.2),red if i%2==0 else accent)
		for side in [-1,1]: box(root,pit+Vector3(side*1.65,1.1,-1.3),Vector3(0.08,2.2,0.08),black)
		label(root,"PIT %02d" % (i+1),pit+Vector3(0,1.75,1.55),0.4,Color.WHITE)
	for i in 6:
		var p := Vector3(hi.x+7,floor_y,center.z-12.5+i*5)
		if theme==0:
			for tier in 3: box(root,p+Vector3(tier*0.8,0.3+tier*0.3,0),Vector3(0.85,0.6+tier*0.6,4),red if i%2==0 else accent)
		elif theme==1:
			box(root,p+Vector3(0,0.12,0),Vector3(4,0.24,4),paint(Color("8abc56")))
			box(root,p+Vector3(0,1.3,0),Vector3(0.12,2.6,0.12),black)
			box(root,p+Vector3(0.55,2.25,0),Vector3(1.1,0.7,0.08),accent if i%2==0 else red)
		else:
			box(root,p+Vector3(0,0.9,0),Vector3(2.6,1.8,2.6),paint(colors.road[i%4]))
			label(root,str(i+1),p+Vector3(0,1,1.32),1.2,Color.WHITE)
	label(root,VENUE.NAMES[theme],Vector3(center.x,floor_y+4,lo.z-12),1.2,colors.accent)
	root.set_meta("rc_display_only",true)
	return root
