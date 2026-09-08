use mapkit_core::*;

fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}

#[test]
fn producer_labels_are_untrusted_but_required() {
    let mut d = document();
    d.provenance.tool_id = "독립 제작 도구".into();
    d.provenance.fingerprint = "arbitrary unknown label / not a signature".into();
    d.attributions.clear(); // Original work need not invent an external source.
    d.validate().unwrap();
    for field in ["tool_id", "version", "build_id", "fingerprint"] {
        for bad in ["", " \t\n", "label\0", "label\n"] {
            let mut value = serde_json::to_value(&d).unwrap();
            value["provenance"][field] = bad.into();
            let changed: MapDocument = serde_json::from_value(value).unwrap();
            assert_eq!(
                changed.validate().unwrap_err().code,
                "E_PROVENANCE",
                "{field}: {bad:?}"
            );
        }
    }
}

#[test]
fn timestamp_calendar_offsets_and_order_are_validated_without_clock() {
    for (first, last) in [
        ("2000-02-29T00:00:00Z", "2000-02-29T00:00:00Z"),
        ("2026-09-07T09:00:00+09:00", "2026-09-07t00:00:00z"),
        ("2026-01-01T00:00:00+01:00", "2025-12-31T23:00:00Z"),
        (
            "2026-01-01T00:00:00.1Z",
            "2026-01-01T00:00:00.100000001+00:00",
        ),
        ("0001-01-01T00:00:00Z", "9999-12-31T23:59:59.999999999Z"),
        ("2026-01-01T00:00:00-01:30", "2026-01-01T01:30:00Z"),
    ] {
        let mut p = document().provenance;
        p.first_created = first.into();
        p.last_edited = last.into();
        p.validate().unwrap();
    }
    for bad in [
        "tomorrow",
        "",
        "2026-02-29T00:00:00Z",
        "1900-02-29T00:00:00Z",
        "2026-04-31T00:00:00Z",
        "2026-00-01T00:00:00Z",
        "2026-13-01T00:00:00Z",
        "2026-01-00T00:00:00Z",
        "2026-01-01T24:00:00Z",
        "2026-01-01T00:60:00Z",
        "2026-01-01T00:00:60Z",
        "2026-01-01T00:00:00-00:00",
        "0000-01-01T00:00:00Z",
        "2026-01-01T00:00:00+24:00",
        "2026-01-01T00:00:00+00:60",
        "2026-01-01T00:00:00",
        "2026-01-01T00:00:00.Z",
        "2026-01-01T00:00:00.1234567890Z",
        "2026-01-01T00:00:00Zsuffix",
        "2026-01-01T00:00:00Z\n",
        "2026-01-01T00:00:00.123456789",
        "2026-01-01T00:00:00+0x:00",
        "2026-01-01T00:00:00é",
        "２０２６-01-01T00:00:00Z",
    ] {
        let mut p = document().provenance;
        p.first_created = bad.into();
        p.last_edited = bad.into();
        assert_eq!(p.validate().unwrap_err().code, "E_PROVENANCE", "{bad:?}");
    }
    let mut p = document().provenance;
    for (first, last) in [
        ("2026-01-01T00:00:00Z", "2025-12-31T23:59:59Z"),
        ("2026-01-01T00:00:00Z", "2026-01-01T00:00:00+00:01"),
        ("2026-01-01T00:00:00.100000001Z", "2026-01-01T00:00:00.1Z"),
    ] {
        p.first_created = first.into();
        p.last_edited = last.into();
        assert_eq!(
            p.validate().unwrap_err().message,
            "last_edited precedes first_created"
        );
    }
}

#[test]
fn source_and_asset_attribution_preserve_notices_and_reject_missing_labels() {
    let mut d = document();
    d.attributions[0].notice = "Original notice\nwith\ttabs\r\nand Unicode 출처".into();
    d.validate().unwrap();
    for field in ["source", "license"] {
        for bad in ["", "  ", "name\u{7f}"] {
            let mut value = serde_json::to_value(&d).unwrap();
            value["attributions"][0][field] = bad.into();
            let mut changed: MapDocument = serde_json::from_value(value).unwrap();
            assert_eq!(changed.validate().unwrap_err().code, "E_ATTRIBUTION");
            changed.assets.push(Asset { convex_collision: vec![], material: None,
                id: "custom".into(),
                path: "custom.png".into(),
                attribution: changed.attributions.remove(0),
                collision: vec![],
            });
            assert_eq!(changed.validate().unwrap_err().code, "E_ATTRIBUTION");
        }
    }
    d.attributions[0].notice = "notice\0".into();
    assert_eq!(d.validate().unwrap_err().code, "E_ATTRIBUTION");
}

#[test]
fn nested_cell_rejects_unknown_fields() {
    assert!(serde_json::from_str::<Cell>(r#"{"x":0,"y":0,"future":1}"#).is_err());
}
