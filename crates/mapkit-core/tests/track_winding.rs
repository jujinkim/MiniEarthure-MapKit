use mapkit_core::{assembled_preview, assembled_track::{self as track, authoring::*}, Vertex};

fn cross([a, b, c]: [Vertex; 3]) -> [i128; 3] {
    let u = std::array::from_fn::<_, 3, _>(|i| i128::from(b[i] - a[i]));
    let v = std::array::from_fn::<_, 3, _>(|i| i128::from(c[i] - a[i]));
    [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]]
}

#[test]
fn published_track_faces_match_package_winding_and_shared_join() {
    let mut source = Source::empty();
    source.instances.push(instance("a", "straight", 400));
    let mut second = instance("b", "straight", 400);
    let end = piece(&source.instances[0]).unwrap().path.last().unwrap().position_cm;
    second.position_cm = end;
    source.instances.push(second);
    source.connections.push(Connection { from: "a".into(), to: "b".into() });
    let document = track::document_from_assembly(compile(&source).unwrap()).unwrap();
    let geometry = assembled_preview(&document).unwrap();
    for face in &geometry.triangles {
        if face.object_id.starts_with("assembled-road-") || face.object_id=="assembled-venue-floor" {
            // In package coordinates a top points inward; the Godot reflection
            // and index conversion make its clockwise front face point upward.
            assert!(cross(face.vertices)[1] < 0, "{} top must face traffic", face.object_id);
        }
        if face.object_id.starts_with("assembled-shell-") && face.vertices.iter().all(|v|v[1]==-10) {
            assert!(cross(face.vertices)[1] > 0, "underside must face below");
        }
    }
    let edge = |id: &str| -> std::collections::BTreeSet<Vertex> {
        geometry.triangles.iter().filter(|t|t.object_id==id).flat_map(|t|t.vertices)
            .filter(|v|v[2]==end[2]).collect()
    };
    let a = edge("assembled-road-0");
    assert_eq!(a.len(), 2);
    assert_eq!(a, edge("assembled-road-1"), "shared exact seam vertices");
}

#[test]
fn published_wall_shells_keep_outward_godot_faces() {
    for preset in ["straight", "gentle90", "slope_up", "curve_up", "finish_plaza"] {
        let mut source = Source::empty();
        source.instances.push(instance("road", preset, 400));
        let document = track::document_from_assembly(compile(&source).unwrap()).unwrap();
        let geometry = assembled_preview(&document).unwrap();
        let walls: Vec<_> = geometry.triangles.iter().filter(|t|t.object_id.starts_with("assembled-wall-")).collect();
        assert!(!walls.is_empty(), "{preset}");
        let volume: i128 = walls.iter().map(|t| {
            let n=cross(t.vertices);
            (0..3).map(|i|n[i]*i128::from(t.vertices[0][i])).sum::<i128>()
        }).sum();
        assert!(volume<0, "{preset}: package shell winding encloses negative signed volume");
    }
}

#[test]
fn published_rotated_curve_joins_have_one_shared_cross_section() {
    for preset in ["gentle90","curve_up","slope_up","spiral90_left_up"] {
        for rotation in [[0,37000,0],[27000,17000,11000]] {
            let mut source=Source::empty();
            let mut a=instance("a",preset,600);
            a.position_cm=[137,211,389];a.rotation_mdeg=rotation;
            let b=snap(&instance("b","straight",600),&a).unwrap();
            let end=piece(&a).unwrap().path.pop().unwrap();
            source.instances=vec![a,b];
            source.connections.push(Connection{from:"a".into(),to:"b".into()});
            let document=track::document_from_assembly(compile(&source).unwrap()).unwrap();
            let geometry=assembled_preview(&document).unwrap();
            let section=|id:&str| -> std::collections::BTreeSet<Vertex> {
                geometry.triangles.iter().filter(|f|f.object_id==id).flat_map(|f|f.vertices)
                    .filter(|v|(0..3).map(|j|(v[j]-end.position_cm[j])*end.forward[j]).sum::<i64>().abs()<1_000_000).collect()
            };
            let left=section("assembled-road-0");
            assert_eq!(left.len(),2,"{preset} {rotation:?}");
            assert_eq!(left,section("assembled-road-1"),"no gap/overlap in published faces {preset} {rotation:?}");
        }
    }
}
