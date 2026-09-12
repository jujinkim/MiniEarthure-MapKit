//! Indexed full-audit hash with the legacy `content_hash` byte contract.
//!
//! Borrowed views serialize object keys in canonical order. Only one top-level
//! collection's index array is allocated at a time; nested arrays, strings and
//! payloads are written directly to SHA-256. Keep the legacy implementation as
//! the independent oracle when changing document fields or normalization.
use crate::*;
use serde::ser::{SerializeMap, SerializeSeq};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, cmp::Ordering};

/// Conservative auxiliary allowance, excluding the borrowed document/payloads.
/// `sort_unstable_by` is in-place; its original-index tie break preserves the
/// stable ordering of `MapDocument::normalize`. No document or JSON tree exists
/// in this path. The fixed allowance covers the SHA/serializer state, temporary
/// payload digest and final 64-byte result, rather than scaling with JSON bytes.
pub(crate) fn scratch_bound(document: &MapDocument) -> u64 {
    let max_records = [
        document.heightmaps.len(),
        document.nodes.len(),
        document.roads.len(),
        document.surface_areas.len(),
        document.buildings.len(),
        document.zones.len(),
        document.assets.len(),
        document.placements.len(),
        document.repetitions.len(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    (max_records as u64)
        .saturating_mul(std::mem::size_of::<usize>() as u64)
        .saturating_add(4096)
}

pub(crate) fn content_hash(
    document: &MapDocument,
    files: &BTreeMap<String, Vec<u8>>,
    mut check: impl FnMut() -> Result<()>,
) -> Result<String> {
    struct HashWriter(Sha256);
    impl Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    check()?;
    let probe = Probe {
        check: RefCell::new(&mut check),
        failure: RefCell::new(None),
    };
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(
        &mut writer,
        &(Document(document, &probe), PayloadHashes(files, &probe)),
    )
    .map_err(|e| probe.failure.borrow_mut().take().unwrap_or_else(|| io(e)))?;
    (probe.check.borrow_mut())()?;
    Ok(format!("{:x}", writer.0.finalize()))
}

struct Probe<'a> {
    check: RefCell<&'a mut dyn FnMut() -> Result<()>>,
    failure: RefCell<Option<Error>>,
}
impl Probe<'_> {
    fn check<E: serde::ser::Error>(&self) -> std::result::Result<(), E> {
        (self.check.borrow_mut())().map_err(|e| {
            let converted = E::custom(&e);
            *self.failure.borrow_mut() = Some(e);
            converted
        })
    }
}
struct PayloadHashes<'a>(&'a BTreeMap<String, Vec<u8>>, &'a Probe<'a>);
impl Serialize for PayloadHashes<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        for (path, bytes) in self.0 {
            if path != "document.json" {
                self.1.check()?;
                let mut digest = Sha256::new();
                for chunk in bytes.chunks(64 * 1024) {
                    self.1.check()?;
                    digest.update(chunk);
                }
                map.serialize_entry(path, &format!("{:x}", digest.finalize()))?;
            }
        }
        map.end()
    }
}

struct Canonical<'a, T>(&'a T);
struct Document<'a>(&'a MapDocument, &'a Probe<'a>);
struct Ordered<'a, T>(&'a [T]);
struct Normalized<'a, T>(&'a [T], &'a Probe<'a>);
trait NormalizeOrder {
    fn compare(&self, other: &Self) -> Ordering;
}
macro_rules! by_id {
    ($($ty:ty),+ $(,)?) => {$(
        impl NormalizeOrder for $ty {
            fn compare(&self, other: &Self) -> Ordering { self.id.cmp(&other.id) }
        }
    )+};
}
by_id!(
    RoadNode,
    Road,
    SurfaceArea,
    Building,
    Zone,
    Asset,
    Placement,
    Repetition
);
impl NormalizeOrder for Heightmap {
    fn compare(&self, other: &Self) -> Ordering {
        self.cell.cmp(&other.cell)
    }
}
impl<T> Serialize for Ordered<'_, T>
where
    for<'a> Canonical<'a, T>: Serialize,
{
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for value in self.0 {
            seq.serialize_element(&Canonical(value))?;
        }
        seq.end()
    }
}
impl<T: NormalizeOrder> Serialize for Normalized<'_, T>
where
    for<'a> Canonical<'a, T>: Serialize,
{
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        self.1.check()?;
        let mut order = Vec::new();
        order
            .try_reserve_exact(self.0.len())
            .map_err(serde::ser::Error::custom)?;
        order.extend(0..self.0.len());
        order.sort_unstable_by(|&a, &b| self.0[a].compare(&self.0[b]).then(a.cmp(&b)));
        let mut seq = serializer.serialize_seq(Some(order.len()))?;
        for (ordinal, index) in order.into_iter().enumerate() {
            if ordinal.is_multiple_of(64) {
                self.1.check()?;
            }
            seq.serialize_element(&Canonical(&self.0[index]))?;
        }
        seq.end()
    }
}

// Each field list must remain lexicographic. Explicit omitted fields mirror
// serde's skip_serializing_if rules in mapkit-core, including older recipes.
macro_rules! fields {
    ($map:ident, $value:ident, $($field:ident),+ $(,)?) => {$(
        $map.serialize_entry(stringify!($field), &$value.$field)?;
    )+};
}
macro_rules! record {
    ($ty:ty, $value:ident, $map:ident, $body:block) => {
        impl Serialize for Canonical<'_, $ty> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
                let $value = self.0;
                let mut $map = serializer.serialize_map(None)?;
                $body
                $map.end()
            }
        }
    };
}
impl Serialize for Document<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let d = self.0;
        let mut m = serializer.serialize_map(None)?;
        m.serialize_entry("assets", &Normalized(&d.assets, self.1))?;
        m.serialize_entry("bounds", &Canonical(&d.bounds))?;
        m.serialize_entry("buildings", &Normalized(&d.buildings, self.1))?;
        fields!(m, d, cell_size_cm);
        m.serialize_entry("heightmaps", &Normalized(&d.heightmaps, self.1))?;
        fields!(m, d, map_id);
        m.serialize_entry("nodes", &Normalized(&d.nodes, self.1))?;
        m.serialize_entry("placements", &Normalized(&d.placements, self.1))?;
        fields!(m, d, recipe_version);
        if !d.repetitions.is_empty() {
            m.serialize_entry("repetitions", &Normalized(&d.repetitions, self.1))?;
        }
        fields!(m, d, revision);
        m.serialize_entry("roads", &Normalized(&d.roads, self.1))?;
        fields!(m, d, seed);
        if !d.surface_areas.is_empty() {
            m.serialize_entry("surface_areas", &Normalized(&d.surface_areas, self.1))?;
        }
        fields!(m, d, terrain_base_cm, theme);
        m.serialize_entry("zones", &Normalized(&d.zones, self.1))?;
        m.end()
    }
}
record!(Bounds, d, m, {
    fields!(m, d, max, min);
});
record!(Cell, d, m, {
    fields!(m, d, x, y);
});
record!(RoadNode, d, m, {
    fields!(m, d, id, level, position);
});
record!(Road, d, m, {
    fields!(m, d, clearance_cm, from, id, kind);
    if let Some(markings) = &d.markings {
        m.serialize_entry("markings", &Canonical(markings))?;
    }
    fields!(m, d, points, sidewalk_cm, surfaces, to, widths_cm);
});
record!(RoadMarkings, d, m, {
    fields!(
        m,
        d,
        center_line,
        crosswalk_end,
        crosswalk_start,
        edge_lines,
        lanes
    );
});
record!(SurfaceArea, d, m, {
    fields!(m, d, id, polygon, surface);
});
record!(Building, d, m, {
    fields!(m, d, base_cm);
    if !d.entrances.is_empty() {
        fields!(m, d, entrances);
    }
    fields!(m, d, footprint, height_cm);
    if !d.holes.is_empty() {
        fields!(m, d, holes);
    }
    fields!(m, d, id, material, roof, usage);
});
record!(Zone, d, m, {
    fields!(
        m,
        d,
        density_per_mille,
        exclusions,
        id,
        kind,
        polygon,
        spacing_cm
    );
});
record!(Heightmap, d, m, {
    m.serialize_entry("cell", &Canonical(&d.cell))?;
    fields!(
        m,
        d,
        offset_cm,
        path,
        source_accuracy_cm,
        spacing_cm,
        step_cm
    );
});
record!(Asset, d, m, {
    m.serialize_entry("attribution", &Canonical(&d.attribution))?;
    m.serialize_entry("collision", &Ordered(&d.collision))?;
    if !d.convex_collision.is_empty() {
        m.serialize_entry("convex_collision", &Ordered(&d.convex_collision))?;
    }
    fields!(m, d, id);
    if let Some(material) = &d.material {
        m.serialize_entry("material", &Canonical(material))?;
    }
    fields!(m, d, path);
});
record!(Attribution, d, m, {
    fields!(m, d, license, notice, source);
});
record!(CollisionBox, d, m, {
    fields!(m, d, center, size_cm);
});
record!(CollisionConvex, d, m, {
    fields!(m, d, faces, vertices);
});
record!(AssetMaterial, d, m, {
    fields!(m, d, albedo_rgba);
    if d.albedo_texture.is_some() {
        fields!(m, d, albedo_texture);
    }
    fields!(m, d, double_sided, metallic_per_mille, roughness_per_mille);
});
record!(Placement, d, m, {
    fields!(m, d, asset_id, id, position, quarter_turns);
});
record!(Repetition, d, m, {
    fields!(m, d, asset_id, id, points, spacing_cm);
});

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal() -> MapDocument {
        serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
    }

    fn payloads() -> BTreeMap<String, Vec<u8>> {
        BTreeMap::from([
            ("document.json".into(), b"excluded document bytes".to_vec()),
            ("a.bin".into(), vec![0, 255, 1]),
            ("unused.bin".into(), vec![42; 128 * 1024 + 1]),
            ("z-empty.bin".into(), vec![]),
        ])
    }

    fn compare(document: &MapDocument, files: &BTreeMap<String, Vec<u8>>) {
        let before = canonical(document).unwrap();
        assert_eq!(
            content_hash(document, files, || Ok(())).unwrap(),
            crate::content_hash(document, files).unwrap()
        );
        // Compare bytes as well as the final digest. This catches field or
        // omission drift as the public schema evolves, independently of SHA.
        let mut normalized = document.clone();
        normalized.normalize();
        let mut value = serde_json::to_value(normalized).unwrap();
        value.as_object_mut().unwrap().remove("provenance");
        value.as_object_mut().unwrap().remove("attributions");
        let mut check = || Ok(());
        let probe = Probe {
            check: RefCell::new(&mut check),
            failure: RefCell::new(None),
        };
        assert_eq!(
            serde_json::to_vec(&Document(document, &probe)).unwrap(),
            canonical(&value).unwrap()
        );
        assert_eq!(
            canonical(document).unwrap(),
            before,
            "borrowed input unchanged"
        );
    }

    fn reverse_collections(d: &mut MapDocument) {
        d.heightmaps.reverse();
        d.nodes.reverse();
        d.roads.reverse();
        d.surface_areas.reverse();
        d.buildings.reverse();
        d.zones.reverse();
        d.assets.reverse();
        d.placements.reverse();
        d.repetitions.reverse();
        d.attributions.reverse();
    }

    #[test]
    fn borrowed_stream_matches_legacy_recipes_one_through_six_and_optional_fields() {
        let files = payloads();
        for recipe in 1..=6 {
            let mut d = minimal();
            d.recipe_version = recipe;
            compare(&d, &files);
            reverse_collections(&mut d);
            compare(&d, &files);
        }
        for source in [
            include_str!("../../../examples/roads/document.json"),
            include_str!("../../../examples/placement/document.json"),
            include_str!("../../../examples/assets/document.json"),
            include_str!("../../../examples/courtyard/document.json"),
        ] {
            let mut d: MapDocument = serde_json::from_str(source).unwrap();
            compare(&d, &files);
            reverse_collections(&mut d);
            compare(&d, &files);
        }
    }

    #[test]
    fn borrowed_stream_preserves_nested_order_and_all_content_metadata() {
        let mut d = minimal();
        d.recipe_version = 6;
        d.map_id = "한글 \"quoted\" \\ newline\n\u{1}".into();
        d.seed = 9_007_199_254_740_991;
        d.terrain_base_cm = -123456;
        d.roads[0].markings = Some(RoadMarkings {
            lanes: 3,
            center_line: true,
            edge_lines: false,
            crosswalk_start: true,
            crosswalk_end: false,
        });
        d.roads[0].sidewalk_cm = Some(0);
        d.roads[0].clearance_cm = Some(340);
        d.buildings[0].holes = vec![vec![[3, 5], [7, 1], [0, 2]], vec![[9, 3], [6, 1], [2, 8]]];
        d.buildings[0].entrances = vec![vec![[9, 2], [8, 7], [1, 5]]];
        d.zones[0].exclusions = d.buildings[0].holes.clone();
        d.surface_areas = vec![SurfaceArea {
            id: "ground".into(),
            polygon: vec![[4, 1], [0, 2], [3, 5]],
            surface: Surface::Grass,
        }];
        d.repetitions = vec![Repetition {
            id: "repeat".into(),
            asset_id: "builtin:fence".into(),
            points: vec![[3, 2, 1], [0, 8, 2]],
            spacing_cm: 500,
        }];
        d.heightmaps = vec![
            Heightmap {
                cell: Cell { x: 2, y: -1 },
                path: "z.png".into(),
                spacing_cm: 200,
                offset_cm: -120,
                step_cm: 1,
                source_accuracy_cm: None,
            },
            Heightmap {
                cell: Cell { x: -1, y: 3 },
                path: "a.png".into(),
                spacing_cm: 400,
                offset_cm: 230,
                step_cm: 100,
                source_accuracy_cm: Some(345),
            },
        ];
        let assets: MapDocument =
            serde_json::from_str(include_str!("../../../examples/assets/document.json")).unwrap();
        d.assets = assets.assets;
        d.placements = assets.placements;
        let extra_box = CollisionBox {
            center: [-10, 5, 3],
            size_cm: [20, 40, 60],
        };
        d.assets[0].collision.push(extra_box.clone());
        d.assets[0].collision.push(CollisionBox {
            center: [6, 1, 2],
            ..extra_box
        });
        let extra_convex = d.assets[0].convex_collision[0].clone();
        d.assets[0].convex_collision.push(extra_convex);
        let files = payloads();
        compare(&d, &files);
        reverse_collections(&mut d);
        for a in &mut d.assets {
            a.collision.reverse();
            a.convex_collision.reverse();
            for c in &mut a.convex_collision {
                c.faces.reverse();
                c.vertices.reverse();
            }
        }
        d.buildings[0].holes.reverse();
        d.buildings[0].holes[0].reverse();
        d.zones[0].exclusions.reverse();
        compare(&d, &files);

        let initial = content_hash(&d, &files, || Ok(())).unwrap();
        d.provenance.tool_id = "producer ignored".into();
        d.attributions.clear();
        let mut changed_files = files.clone();
        changed_files.insert("document.json".into(), vec![255; 900]);
        assert_eq!(
            initial,
            content_hash(&d, &changed_files, || Ok(())).unwrap()
        );
        d.assets[0]
            .attribution
            .notice
            .push_str("asset notice retained");
        assert_ne!(
            initial,
            content_hash(&d, &changed_files, || Ok(())).unwrap()
        );
        compare(&d, &changed_files);
        let before_unused = content_hash(&d, &changed_files, || Ok(())).unwrap();
        changed_files.get_mut("unused.bin").unwrap()[0] ^= 1;
        assert_ne!(
            before_unused,
            content_hash(&d, &changed_files, || Ok(())).unwrap()
        );
        compare(&d, &changed_files);
    }

    #[test]
    fn borrowed_stream_preserves_stable_ties_and_bounds_only_one_order_array() {
        let mut d = minimal();
        let node = d.nodes[0].clone();
        d.nodes = (0..129)
            .map(|i| RoadNode {
                position: [i, -i, i + 1],
                ..node.clone()
            })
            .collect();
        d.roads = vec![d.roads[0].clone(); 100];
        let files = payloads();
        compare(&d, &files);
        d.nodes.reverse();
        compare(&d, &files);
        assert_eq!(
            scratch_bound(&d),
            129 * std::mem::size_of::<usize>() as u64 + 4096
        );
        let prior = scratch_bound(&d);
        d.roads[0].points = vec![[0, 1, 2]; 100_000];
        d.map_id = "s".repeat(100_000);
        d.attributions = vec![d.attributions[0].clone(); 1000];
        assert_eq!(
            scratch_bound(&d),
            prior,
            "nested geometry and excluded metadata are borrowed"
        );
        compare(&d, &files);
    }

    #[test]
    fn borrowed_stream_cancellation_covers_collections_payloads_and_completion() {
        let mut d = minimal();
        d.nodes = vec![d.nodes[0].clone(); 129];
        let files = payloads();
        let mut calls = 0;
        content_hash(&d, &files, || {
            calls += 1;
            Ok(())
        })
        .unwrap();
        assert!(calls >= 20);
        for cutoff in 1..=calls {
            let mut current = 0;
            let failure = content_hash(&d, &files, || {
                current += 1;
                if current == cutoff {
                    Err(error("E_CANCELLED", "audit hash cancelled"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(failure.code, "E_CANCELLED", "checkpoint {cutoff}");
            assert_eq!(current, cutoff);
        }
    }
}
