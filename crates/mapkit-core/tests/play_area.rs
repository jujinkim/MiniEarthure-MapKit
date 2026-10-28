use mapkit_core::*;

fn minimal() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}

#[test]
fn play_area_rings_validate_and_bound_the_drivable_area() {
    let mut d = minimal();
    let [x0, y0] = d.bounds.min;
    let [x1, y1] = d.bounds.max;
    assert!(in_play_area(&d, [x0 + 1, y0 + 1]), "no rings: the whole rectangle");
    let canonical = String::from_utf8(canonical(&d).unwrap()).unwrap();
    assert!(!canonical.contains("play_area"), "an absent play area keeps the document bytes");
    d.play_area = vec![vec![[x0, y0], [x1, y0], [x0, y1]]];
    d.validate().unwrap();
    assert!(in_play_area(&d, [x0 + 10, y0 + 10]));
    assert!(!in_play_area(&d, [x1 - 10, y1 - 10]), "outside the triangle, inside the bounds");
    let mut outside = d.clone();
    outside.play_area[0][1] = [x1 + 1, y0];
    assert_eq!(outside.validate().unwrap_err().code, "E_PLAY_AREA");
    let mut crossed = d.clone();
    crossed.play_area = vec![vec![[x0, y0], [x1, y1], [x1, y0], [x0, y1]]];
    assert_eq!(crossed.validate().unwrap_err().code, "E_PLAY_AREA", "a self-intersecting ring");
    let mut many = d.clone();
    many.play_area = vec![d.play_area[0].clone(); MAX_PLAY_AREA_RINGS + 1];
    assert_eq!(many.validate().unwrap_err().code, "E_PLAY_AREA");
    let text = serde_json::to_string(&d).unwrap();
    let back: MapDocument = serde_json::from_str(&text).unwrap();
    assert_eq!(back.play_area, d.play_area);
}
