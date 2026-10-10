use mapkit_core::{poi::PointOfInterest, *};
use mapkit_package::{indexed::*, *};
use std::{collections::BTreeMap, io::Cursor};

fn document() -> MapDocument {
    let mut d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.pois.push(PointOfInterest {
        id: "library-1".into(),
        name: "작은도서관".into(),
        category: "도서관".into(),
        position: [1000, 1000],
        source: Attribution {
            source: "Synthetic CSV row 2".into(),
            license: "CC0-1.0".into(),
            notice: "Coordinates supplied; elevation unspecified".into(),
        },
    });
    d
}

#[test]
fn facilities_survive_both_packages_and_bounded_overview() {
    let d = document();
    let bytes = pack_bytes(d.clone(), BTreeMap::new()).unwrap();
    let loaded = read_bytes(&bytes).unwrap();
    assert_eq!(loaded.document.pois, d.pois);
    assert_eq!(pack_bytes(loaded.document, loaded.files).unwrap(), bytes);
    let view = overview(&d).unwrap();
    let cost = view.cost().unwrap();
    assert!(view.to_json(cost.json_bytes - 1).is_err());
    assert!(view
        .to_json(cost.json_bytes)
        .unwrap()
        .contains("작은도서관"));
    let regions = pack_source(d.clone(), BTreeMap::new(), 1).unwrap();
    let mut reader = IndexedReader::open(Cursor::new(regions), 1024 * 1024 * 1024, None).unwrap();
    assert_eq!(reader.index().world.pois, d.pois);
    reader
        .audit_summary(1024 * 1024 * 1024, &ReadEpoch::default().begin())
        .unwrap();
}

#[test]
fn facilities_reject_invalid_text_coordinates_and_ids() {
    let d = document();
    let mut plain = d.clone();
    plain.pois.clear();
    // Informational points do not allocate extra execution cells.
    assert_eq!(d.cell_count().unwrap(), plain.cell_count().unwrap());
    for mutation in 0..5 {
        let mut bad = d.clone();
        match mutation {
            0 => bad.pois[0].name = " ".into(),
            1 => bad.pois[0].category = "bad\nvalue".into(),
            2 => bad.pois[0].position = [i64::MAX, 0],
            3 => bad.pois[0].id = "terrain".into(),
            _ => bad.pois.push(bad.pois[0].clone()),
        }
        assert!(bad.validate().is_err());
    }
    assert_eq!(d.recipe_version, 1);
}
