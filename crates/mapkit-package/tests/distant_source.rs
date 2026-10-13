use mapkit_core::*;
use mapkit_package::*;
use std::{collections::BTreeMap, hint::black_box, time::Instant};
#[path = "support/distant.rs"]
mod support;

fn scenarios() -> Vec<(&'static str, MapDocument, BTreeMap<String, Vec<u8>>)> {
    let (mut dense, _) = support::fixture();
    dense.assets.clear(); dense.placements.clear();
    dense.bounds.max = [25600,25600]; dense.cell_size_cm = 12800;
    for y in 0..16 { for x in 0..16 {
        let x = 300+x*1550; let y = 300+y*1550;
        dense.buildings.push(Building { id:format!("building-{x}-{y}"),
            footprint:vec![[x,y],[x+1200,y],[x+1200,y+1200],[x,y+1200]], holes:vec![],
            base_cm:0,height_cm:1500,usage:"public".into(),
            material:if x%2==0 {"wood"} else {"brick"}.into(),
            roof:if y%2==0 {"gable"} else {"flat"}.into(),entrances:vec![] });
    }}
    let (mut repeated, files) = support::fixture();
    repeated.bounds.max=[25600,25600]; repeated.cell_size_cm=12800;
    repeated.placements.clear();
    for y in 0..12 { for x in 0..16 {
        repeated.placements.push(Placement {id:format!("portal-{x}-{y}"), asset_id:"portal".into(),
            position:[500+x*1500,0,500+y*1500],quarter_turns:(x%4) as u8,yaw_offset_mdeg:30000});
    }}
    repeated.placements.push(Placement {id:"seam".into(),asset_id:"portal".into(),position:[12800,0,22000],quarter_turns:0,yaw_offset_mdeg:30000});
    repeated.repetitions = serde_json::from_value(serde_json::json!([
        {"id":"fences", "asset_id":"builtin:fence", "points":[[2000,0,24000],[24000,0,24000]],"spacing_cm":200}
    ])).unwrap();
    let mut open = dense.clone(); open.buildings.clear();
    let mut result=vec![("dense",dense,BTreeMap::new()),("repeated",repeated,files),("open",open,BTreeMap::new())];
    for name in ["minimal","roads","courtyard","placement","assets"] {
        let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name);
        let (d,f)=read_project(&path).unwrap(); result.push((name,d,f));
    }
    result
}

fn display_hash(mesh: &distant::DistantMesh) -> String {
    let mut bytes=Vec::new();
    for p in &mesh.vertices {for v in p {bytes.extend(v.to_le_bytes());}}
    for c in &mesh.colors {bytes.extend(c);}
    for p in &mesh.light_data {for v in p {bytes.extend(v.to_le_bytes());}}
    bytes.extend(&mesh.decoration); sha256(&bytes)
}

#[test]
fn frozen_ordinary_and_display_outputs() {
    let mut snapshots=BTreeMap::new();
    for (name,d,files) in scenarios() {
        let p=read_bytes(&pack_bytes(d,files).unwrap()).unwrap();
        for cell in p.document.cells() {
            let chunk=p.generate(cell,500_000).unwrap();
            let reference=p.distant_from_generated(&chunk).unwrap();
            let actual=p.generate_distant(cell).unwrap();
            assert_eq!(reference.vertices,actual.vertices,"{name} {cell:?}");
            assert_eq!(reference.colors,actual.colors);
            assert_eq!(reference.light_data,actual.light_data);
            assert_eq!(reference.decoration,actual.decoration);
            snapshots.insert(format!("{name}/{}/{}",cell.x,cell.y), (chunk.hash().unwrap(),display_hash(&actual)));
        }
    }
    let expected = serde_json::from_str::<BTreeMap<String,(String,String)>>(include_str!("support/distant-baseline.json")).unwrap();
    assert_eq!(snapshots,expected,"unchanged ordinary hashes and frozen packed distant display bytes");
}

#[test]
#[ignore = "short local timing comparison; no performance threshold"]
fn short_distant_measurement() {
    for (name,d,files) in scenarios().into_iter().take(3) {
        let bytes=pack_bytes(d,files).unwrap();
        for trial in 0..3 {
            let start=Instant::now();let p=read_bytes(&bytes).unwrap();let load=start.elapsed();
            let cells:Vec<_>=p.document.cells();
            let start=Instant::now();for &c in &cells {black_box(p.document.estimate(c,500_000).unwrap());}let estimate=start.elapsed();
            for pass in 0..2 {
                let mut generation=std::time::Duration::ZERO;let mut conversion=generation;let mut dedicated=generation;
                let mut counts=[0usize;4];
                let mut source_counts=[0usize;2]; let mut source_time=generation;
                for &c in &cells {
                    let start=Instant::now();let chunk=p.generate(c,500_000).unwrap();generation+=start.elapsed();
                    counts[0]+=chunk.triangles.len();counts[1]+=chunk.objects.len();counts[2]+=chunk.building_prisms.len();counts[3]+=chunk.asset_convexes.len();
                    let start=Instant::now();black_box(p.distant_from_generated(&chunk).unwrap());conversion+=start.elapsed();
                    let start=Instant::now();black_box(p.generate_distant(c).unwrap());dedicated+=start.elapsed();
                    let start=Instant::now();let source=p.document.generate_distant_source(c,None,500_000).unwrap();source_time+=start.elapsed();
                    source_counts[0]+=source.triangles.len();source_counts[1]+=source.objects.len();
                }
                println!("{name} trial={trial} pass={pass} load_us={} estimate_us={} generate_us={} convert_us={} far_us={} intermediate={counts:?} source={source_counts:?} source_us={}", load.as_micros(),estimate.as_micros(),generation.as_micros(),conversion.as_micros(),dedicated.as_micros(),source_time.as_micros());
            }
        }
    }
}
