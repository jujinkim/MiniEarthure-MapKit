use mapkit_core::*;
use std::collections::{BTreeMap,BTreeSet};

fn compare(prepared: &PreparedMap, cell: Cell, grid: Option<&HeightGrid>) -> DistantSource {
    let ordinary = prepared.generate(cell,grid,500_000).unwrap();
    let source = prepared.generate_distant_source(cell,grid,500_000).unwrap();
    let mut omitted: BTreeSet<_> = prepared.placements.iter().filter(|p|
        prepared.asset(&p.asset_id).is_some_and(|a|a.path.ends_with(".glb")))
        .map(|p|p.id.as_str()).collect();
    omitted.extend(ordinary.objects.iter().filter(|o|o.asset_id=="builtin:tree" ||
        prepared.asset(&o.asset_id).is_some_and(|a|a.path.ends_with(".glb"))).map(|o|o.id.as_str()));
    let expected: Vec<_> = ordinary.triangles.iter().filter(|t|!omitted.contains(t.object_id.as_str())).cloned().map(|mut t| {
        t.contact_class=0; t.snow_retention_percent=100; t
    }).collect();
    assert_eq!(source.triangles,expected,"cell {cell:?}");
    assert_eq!(source.objects,ordinary.objects);
    assert_eq!(source.water_bodies,ordinary.water_bodies);
    assert_eq!(ordinary,prepared.generate(cell,grid,500_000).unwrap());
    assert_eq!(source.triangles,prepared.generate_distant_source(cell,grid,500_000).unwrap().triangles);
    source
}

#[test]
fn source_keeps_roofs_courtyards_roads_placement_ownership_and_clipping() {
    for text in [include_str!("../../../examples/placement/document.json"),
        include_str!("../../../examples/courtyard/document.json"),
        include_str!("../../../examples/roads/document.json")] {
        let d: MapDocument=serde_json::from_str(text).unwrap();
        let p=PreparedMap::new(d).unwrap();
        for cell in p.cells() { compare(&p,cell,None); }
    }
    let mut d:MapDocument=serde_json::from_str(include_str!("../../../examples/placement/document.json")).unwrap();
    d.placements[0].position=[12800,0,14000];
    let p=PreparedMap::new(d).unwrap();
    let left=compare(&p,Cell{x:0,y:1},None);
    let right=compare(&p,Cell{x:1,y:1},None);
    assert!(left.triangles.iter().any(|t|t.object_id=="manual-tree"),"neighbour-owned builtin proxy is retained");
    assert!(!right.triangles.iter().any(|t|t.object_id=="manual-tree"));
    assert!(right.objects.iter().any(|o|o.id=="manual-tree"));
}

#[test]
fn vegetation_keeps_water_islands_gimmick_exclusions_and_custom_tree_ownership() {
    let mut d:MapDocument=serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear();d.roads.clear();d.buildings.clear();d.zones.clear();
    d.bounds.max=[12800,12800];d.cell_size_cm=6400;
    d.zones.push(Zone {id:"orchard".into(),polygon:vec![[100,100],[12700,100],[12700,12700],[100,12700]],
        kind:ZoneKind::Orchard,spacing_cm:800,density_per_mille:1000,exclusions:vec![],tree:None});
    d.water_bodies.push(water::WaterBody{id:"lake".into(),polygon:vec![[1000,1000],[4000,1000],[4000,4000],[1000,4000]],
        islands:vec![vec![[2000,2000],[3000,2000],[3000,3000],[2000,3000]]],surface_cm:100,bottom_cm:-100,flow_cm_s:[30,-20]});
    let mut templates:BTreeMap<String,gimmick::Gimmick>=serde_json::from_str(include_str!("../../../examples/driving-library/library.json")).unwrap();
    d.gimmicks.push(templates.remove("cylinder").unwrap());
    let without= { let mut d=d.clone();d.water_bodies.clear();d.gimmicks.clear();PreparedMap::new(d).unwrap() };
    let candidates:Vec<_>=without.cells().into_iter().flat_map(|c|without.generate(c,None,500_000).unwrap().objects).collect();
    assert!(candidates.iter().any(|o|d.water_bodies[0].contains(o.position)));
    assert!(candidates.iter().any(|o|d.gimmicks[0].excludes_spawn(o.position)));
    for custom in [false,true] {
        if custom {
            d.assets=serde_json::from_value(serde_json::json!([{"id":"tree","path":"tree.glb",
                "attribution":{"source":"synthetic","license":"MIT","notice":"test"},
                "collision":[{"center":[0,100,0],"size_cm":[40,200,40]}]}])).unwrap();
            d.zones[0].tree=Some(ZoneTree{asset_id:"tree".into(),radius_cm:200,clearance_cm:100});
        }
        let p=PreparedMap::new(d.clone()).unwrap();
        let mut count=0;
        for cell in p.cells() {
            let source=compare(&p,cell,None);count+=source.objects.len();
            assert!(source.objects.iter().all(|o|!d.water_bodies[0].contains(o.position) && !d.gimmicks[0].excludes_spawn(o.position)));
        }
        assert!(count>0 && count<candidates.len());
    }
}

#[test]
fn height_input_limits_cancellation_and_replacement_are_preserved() {
    let mut d:MapDocument=serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear();d.roads.clear();d.buildings.clear();d.zones.clear();
    d.heightmaps.push(Heightmap{cell:Cell{x:0,y:0},path:"slope.png".into(),spacing_cm:12800,offset_cm:0,step_cm:1,source_accuracy_cm:None});
    let p=PreparedMap::new(d).unwrap();let cell=Cell{x:0,y:0};
    let grid=HeightGrid{side:5,heights_cm:(0..25).collect()};
    compare(&p,cell,Some(&grid));
    assert_eq!(p.generate_distant_source(cell,None,500_000).unwrap_err().code,"E_HEIGHTMAP");
    assert_eq!(p.generate_distant_source(cell,Some(&grid),1).unwrap_err().code,"E_BUDGET");
    assert_eq!(p.generate_distant_source(Cell{x:99,y:99},None,500_000).unwrap_err().code,"E_CELL");
    let token=cancellation::CancellationToken::default();
    let result=token.run(|| { token.cancel();p.generate_distant_source(cell,Some(&grid),500_000) });
    assert_eq!(result.unwrap_err().code,"E_CANCELLED");
    cancellation::CancellationToken::default().run(||p.generate_distant_source(cell,Some(&grid),500_000)).unwrap();
}
