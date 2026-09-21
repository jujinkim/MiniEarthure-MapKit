use mapkit_core::*;
fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/placement/document.json")).unwrap()
}
fn generated(d: &MapDocument, cell: Cell) -> GeneratedOccupancy {
    generate_with_occupancy(
        GenerationInput {
            document: d,
            cell,
            heightgrid: None,
            max_triangles: 500_000,
        },
        20_000,
    )
    .unwrap()
}
#[test]
fn roofs_solid_parts_styles_concavity_and_cell_seams() {
    let d = document();
    let a = generated(&d, Cell { x: 0, y: 0 });
    let b = generated(&d, Cell { x: 1, y: 0 });
    let parts: Vec<_> = a
        .chunk
        .building_prisms
        .iter()
        .chain(&b.chunk.building_prisms)
        .filter(|p| p.object_id == "gable")
        .collect();
    assert!(!parts.is_empty());
    assert!(parts
        .iter()
        .all(|p| p.valid() && p.material == "wood" && p.usage == "public"));
    assert_eq!(parts.iter().flat_map(|p| p.top_cm).max(), Some(1350));
    assert!(a
        .chunk
        .building_prisms
        .iter()
        .filter(|p| p.object_id == "gable")
        .all(|p| p.footprint.iter().all(|v| v[0] <= 12800)));
    assert!(b
        .chunk
        .building_prisms
        .iter()
        .filter(|p| p.object_id == "gable")
        .all(|p| p.footprint.iter().all(|v| v[0] >= 12800)));
    assert!(!a
        .chunk
        .building_prisms
        .iter()
        .any(|p| p.object_id == "concave" && point_in_polygon([5000, 4000], &p.footprint)));
    assert!(a
        .chunk
        .building_prisms
        .iter()
        .any(|p| p.object_id == "concave" && point_in_polygon([2500, 4000], &p.footprint)));
    assert!(a
        .chunk
        .spawn(&SpawnRequest {
            position_cm: [2500, 2000],
            surface_id: "concave".into()
        })
        .is_err());
    assert!(a.solids.iter().any(|p|matches!(&p.shape,SolidShape::SlopedPrism{top_cm,..} if top_cm.iter().min()!=top_cm.iter().max())));
    for c in [&a.chunk, &b.chunk] {
        let cost = estimate_generation(&d, c.cell, 500_000).unwrap();
        assert!(cost.building_prisms >= c.building_prisms.len() as u64);
        assert!(cost.triangles >= c.triangles.len() as u64);
        assert_eq!(sha256(&canonical(c).unwrap()), c.hash().unwrap());
        let key = archive_key(&"a".repeat(64), c.cell);
        let bytes = encode_archive(c, &key, archive_limit(&cost)).unwrap();
        assert_eq!(decode_archive(&bytes, &key, c.cell, &cost).unwrap(), *c);
        let mut bad = bytes.clone();
        let n = bad.len();
        bad[n - 1] ^= 128;
        assert!(decode_archive(&bad, &key, c.cell, &cost).is_err());
        let mut small = cost.clone();
        small.building_prisms = 0;
        assert!(decode_archive(&bytes, &key, c.cell, &small).is_err());
    }
}
#[test]
fn sidewalks_obey_theme_width_disable_and_full_entrance_clearance() {
    let mut d = document();
    let cell = Cell { x: 0, y: 0 };
    let c = generated(&d, cell).chunk;
    let sidewalk: Vec<_> = c
        .triangles
        .iter()
        .filter(|t| t.object_id == "street:sidewalk" && t.spawnable)
        .collect();
    assert!(!sidewalk.is_empty());
    assert!(sidewalk
        .iter()
        .all(|t| t.vertices.iter().all(|p| p[1] == 12)));
    assert!(c
        .spawn(&SpawnRequest {
            position_cm: [3900, 6400],
            surface_id: "street:sidewalk".into()
        })
        .is_err());
    assert!(c
        .spawn(&SpawnRequest {
            position_cm: [8000, 6400],
            surface_id: "street:sidewalk".into()
        })
        .is_ok());
    d.roads[0].sidewalk_cm = Some(0);
    assert!(!generated(&d, cell)
        .chunk
        .triangles
        .iter()
        .any(|t| t.object_id == "street:sidewalk"));
    d.theme = "rural".into();
    d.roads[0].sidewalk_cm = None;
    assert!(!generated(&d, cell)
        .chunk
        .triangles
        .iter()
        .any(|t| t.object_id == "street:sidewalk"));
    d.roads[0].sidewalk_cm = Some(100);
    let narrow = generated(&d, cell).chunk;
    assert!(narrow
        .triangles
        .iter()
        .any(|t| t.object_id == "street:sidewalk"));
    assert!(narrow
        .spawn(&SpawnRequest {
            position_cm: [8000, 6450],
            surface_id: "street:sidewalk".into()
        })
        .is_err());
}
#[test]
fn full_vegetation_footprint_spacing_boundary_ownership_and_order() {
    let mut d = document();
    let mut objects = vec![];
    let mut hashes = vec![];
    for y in 0..2 {
        for x in 0..2 {
            let c = generated(&d, Cell { x, y }).chunk;
            hashes.push(c.hash().unwrap());
            for o in c.objects {
                assert_eq!(
                    d.cell_at([o.position[0], o.position[2]]),
                    Some(Cell { x, y })
                );
                objects.push(o);
            }
        }
    }
    let ids: std::collections::BTreeSet<_> = objects.iter().map(|o| &o.id).collect();
    assert_eq!(ids.len(), objects.len());
    let trees: Vec<_> = objects
        .iter()
        .filter(|o| o.id.starts_with("forest:") || o.id.starts_with("orchard:"))
        .collect();
    assert!(trees.len() > 35);
    for (i, t) in trees.iter().enumerate() {
        let z = d
            .zones
            .iter()
            .find(|z| t.id.starts_with(&format!("{}:", z.id)))
            .unwrap();
        for dx in [-200, 200] {
            for dy in [-200, 200] {
                let p = [t.position[0] + dx, t.position[2] + dy];
                assert!(point_in_polygon(p, &z.polygon));
                assert!(z.exclusions.iter().all(|e| !point_in_polygon(p, e)));
            }
        }
        for other in &trees[..i] {
            let dx = t.position[0] - other.position[0];
            let dy = t.position[2] - other.position[2];
            assert!(dx.abs() > 400 || dy.abs() > 400);
            if t.id.starts_with("forest:") && other.id.starts_with("forest:") {
                assert!(dx * dx + dy * dy >= 900 * 900);
            }
        }
    }
    d.zones.reverse();
    d.buildings.reverse();
    d.repetitions.reverse();
    d.placements.reverse();
    for (i, (y, x)) in (0..2).flat_map(|y| (0..2).map(move |x| (y, x))).enumerate() {
        assert_eq!(
            generated(&d, Cell { x, y }).chunk.hash().unwrap(),
            hashes[i]
        );
    }
}
#[test]
fn seam_tree_has_one_complete_trunk_and_repetitions_have_unique_stable_ids() {
    let mut d = document();
    d.zones.truncate(1);
    d.zones[0].polygon = vec![
        [11000, 16000],
        [14000, 16000],
        [14000, 20000],
        [11000, 20000],
    ];
    d.zones[0].spacing_cm = 800;
    d.zones[0].exclusions.clear();
    let left = generated(&d, Cell { x: 0, y: 1 }).chunk;
    let right = generated(&d, Cell { x: 1, y: 1 }).chunk;
    let id = "orchard:16:22";
    assert!(!left.objects.iter().any(|o| o.id == id));
    assert!(right.objects.iter().any(|o| o.id == id));
    let vertices: Vec<_> = right
        .triangles
        .iter()
        .filter(|t| t.object_id == id)
        .flat_map(|t| t.vertices)
        .collect();
    assert_eq!(vertices.iter().map(|p| p[0]).min(), Some(12780));
    assert_eq!(vertices.iter().map(|p| p[0]).max(), Some(12820));
    assert!(left
        .objects
        .iter()
        .any(|o| o.id.starts_with("fence-line:repeat:")));
    assert!(generated(&d, Cell { x: 0, y: 0 })
        .chunk
        .objects
        .iter()
        .any(|o| o.id.starts_with("lights:repeat:")));
}
#[test]
fn invalid_authored_overlaps_names_roofs_and_work_limits_fail_closed() {
    let mut d = document();
    d.placements[0].position = [1900, 0, 2000];
    assert_eq!(d.validate().unwrap_err().code, "E_GEOMETRY"); // canopy edge, centre outside
    let mut d = document();
    d.buildings[0].roof = "magic".into();
    assert!(d.validate().is_err());
    let mut d = document();
    d.buildings[0].roof = "gable".into();
    assert!(d.validate().is_err());
    let mut d = document();
    d.placements[0].id = "street:sidewalk".into();
    assert_eq!(d.validate().unwrap_err().code, "E_ID");
    let mut d = document();
    d.repetitions[0].points[1][2] += 100;
    assert!(d.validate().is_err());
    let d = document();
    assert_eq!(
        generate(GenerationInput {
            document: &d,
            cell: Cell { x: 0, y: 0 },
            heightgrid: None,
            max_triangles: 1
        })
        .unwrap_err()
        .code,
        "E_BUDGET"
    );
}

#[test]
fn automatic_sidewalk_fits_setback_and_follows_sampled_terrain() {
    let mut d=document();d.buildings.retain(|b|b.id=="shop");d.buildings[0].entrances.clear();
    d.buildings[0].footprint=vec![[3000,6420],[6000,6420],[6000,8000],[3000,8000]];
    let c=generated(&d,Cell{x:0,y:0}).chunk;
    assert!(c.spawn(&SpawnRequest{position_cm:[4000,6380],surface_id:"street:sidewalk".into()}).is_ok());
    assert!(c.spawn(&SpawnRequest{position_cm:[4000,6430],surface_id:"street:sidewalk".into()}).is_err());
    d.roads[0].sidewalk_cm=Some(180);
    assert!(generated(&d,Cell{x:0,y:0}).chunk.spawn(&SpawnRequest{position_cm:[4000,6430],surface_id:"street:sidewalk".into()}).is_err());
    d.buildings.clear();d.zones.clear();d.placements.clear();d.repetitions.clear();
    d.heightmaps.push(Heightmap{cell:Cell{x:0,y:0},path:"slope.png".into(),spacing_cm:3200,offset_cm:0,step_cm:1,source_accuracy_cm:None});
    let grid=HeightGrid{side:5,heights_cm:(0..5).flat_map(|y|(0..5).map(move|x|x*320+y*160)).collect()};
    let c=generate(GenerationInput{document:&d,cell:Cell{x:0,y:0},heightgrid:Some(&grid),max_triangles:500_000}).unwrap();
    let p=c.spawn(&SpawnRequest{position_cm:[8000,6400],surface_id:"street:sidewalk".into()}).unwrap();
    assert_eq!(p[1],1132); // x/10+y/20+12; no flat sidewalk or relative-rounding drift
}

#[test]
fn elevated_pier_requires_vertical_separation_and_never_clears_ground() {
    let mut d = document();
    d.buildings.clear(); d.zones.clear(); d.placements.clear(); d.repetitions.clear();
    d.assets = serde_json::from_value(serde_json::json!([{
        "id":"pier", "path":"pier.glb", "attribution":{"source":"synthetic","license":"MIT","notice":"unit fixture"},
        "collision":[{"center":[0,200,0],"size_cm":[100,400,100]}]
    }])).unwrap();
    d.placements = serde_json::from_value(serde_json::json!([{"id":"column","asset_id":"pier","position":[10000,0,6000],"quarter_turns":0}])).unwrap();
    d.roads[0].kind = RoadKind::Elevated;
    for p in &mut d.roads[0].points { p[1] = 600; }
    for n in &mut d.nodes { n.position[1] = 600; n.level = 1; }
    d.validate().unwrap();
    let c=generated(&d,Cell{x:0,y:0});
    assert!(c.solids.iter().any(|s|s.object_id=="column"));
    d.placements[0].position[1] = 200;
    assert_eq!(d.validate().unwrap_err().code,"E_GEOMETRY");
    d.placements[0].position[1] = 0;
    d.roads[0].kind = RoadKind::Ground;
    assert_eq!(d.validate().unwrap_err().code,"E_GEOMETRY");
    d.roads[0].kind = RoadKind::Bridge;
    d.roads[0].points[1][1] = 300;
    d.nodes[1].position[1] = 300;
    d.placements[0].position[0] = 23000;
    assert_eq!(d.validate().unwrap_err().code,"E_GEOMETRY");
}

#[test]
fn diagonal_proxy_hull_rejects_contact_without_empty_aabb_corners() {
    let mut d=document();
    d.buildings.clear();d.zones.clear();d.placements.clear();d.repetitions.clear();
    d.assets=serde_json::from_value(serde_json::json!([
        {"id":"brace","path":"brace.glb","attribution":{"source":"synthetic","license":"MIT","notice":"fixture"},
         "collision":[{"center":[-300,50,-300],"size_cm":[100,100,100]},{"center":[300,50,300],"size_cm":[100,100,100]}]},
        {"id":"post","path":"post.glb","attribution":{"source":"synthetic","license":"MIT","notice":"fixture"},
         "collision":[{"center":[0,50,0],"size_cm":[100,100,100]}]}
    ])).unwrap();
    d.placements=serde_json::from_value(serde_json::json!([
        {"id":"brace-one","asset_id":"brace","position":[10000,0,9000],"quarter_turns":0},
        {"id":"post-one","asset_id":"post","position":[9700,0,9300],"quarter_turns":0}
    ])).unwrap();
    d.validate().unwrap();
    d.placements[1].position=[9700,0,8700];
    assert_eq!(d.validate().unwrap_err().code,"E_GEOMETRY");
}

#[test]
fn prepared_source_index_preserves_order_hash_and_boundary_occupancy() {
    let d=document(); let prepared=PreparedMap::new(d.clone()).unwrap();
    for cell in d.cells() {
        let old=generated(&d,cell);
        let new=prepared.generate_with_occupancy(cell,None,500_000,Some(20_000)).unwrap();
        assert_eq!(old.chunk.hash().unwrap(),new.chunk.hash().unwrap());
        assert_eq!(old.solids,new.solids);
    }
}
