//! Retained ownership of a typed authoring source, independent of its validity.
//!
//! This is an allocation-free walk of the current schema, not an RSS reading or
//! an admission to decode/validate the source. The caller reserves parsing first.
//! Inline fields are charged by their enclosing struct/vector allocation; only
//! owned heap buffers are added recursively. Spare capacity remains charged.
use mapkit_core::*;
use std::mem::size_of;

// Logical allocator/header/rounding allowance for each nonempty allocation.
// Keep it separate from capacity so even very short strings have a fixed cost.
const ALLOCATION_OVERHEAD: u64 = 64;

#[derive(Default)]
struct Retained(u64);

impl Retained {
    fn add(&mut self, bytes: u64) -> Result<()> {
        self.0 = self.0.checked_add(bytes).ok_or_else(overflow)?;
        Ok(())
    }

    fn allocation(&mut self, capacity: usize, element_bytes: usize) -> Result<()> {
        if capacity == 0 || element_bytes == 0 {
            return Ok(());
        }
        let bytes = u64::try_from(capacity)
            .ok()
            .and_then(|n| n.checked_mul(u64::try_from(element_bytes).ok()?))
            .and_then(|n| n.checked_add(ALLOCATION_OVERHEAD))
            .ok_or_else(overflow)?;
        self.add(bytes)
    }

    fn string(&mut self, value: &String) -> Result<()> {
        self.allocation(value.capacity(), size_of::<u8>())
    }

    fn vector<T>(&mut self, values: &Vec<T>) -> Result<()> {
        self.allocation(values.capacity(), size_of::<T>())
    }

    fn polygons(&mut self, values: &Vec<Vec<Point>>) -> Result<()> {
        self.vector(values)?;
        for polygon in values {
            self.vector(polygon)?;
        }
        Ok(())
    }

    fn attribution(&mut self, value: &Attribution) -> Result<()> {
        let Attribution {
            source,
            license,
            notice,
        } = value;
        self.string(source)?;
        self.string(license)?;
        self.string(notice)
    }

    fn provenance(&mut self, value: &Provenance) -> Result<()> {
        let Provenance {
            tool_id,
            version,
            build_id,
            fingerprint,
            first_created,
            last_edited,
        } = value;
        self.string(tool_id)?;
        self.string(version)?;
        self.string(build_id)?;
        self.string(fingerprint)?;
        self.string(first_created)?;
        self.string(last_edited)
    }
}

fn overflow() -> Error {
    error(
        "E_MEMORY_BUDGET",
        "typed source retained allocation size overflow",
    )
}

/// Charge the root struct, every vector's full capacity and every live nested
/// string/vector buffer. No validation, cloning, normalization or geometry runs.
/// Malformed coordinates, indices and counts are safe to inspect here.
pub(crate) fn document_retained_bytes(document: &MapDocument) -> Result<u64> {
    // MapDocument's indexed_topology field is private to mapkit-core, requiring
    // `..` here. The schema-field test below guards additions to its public data.
    // All accessible nested structures use exhaustive field patterns so newly
    // added fields require an explicit ownership decision at compile time.
    let MapDocument {
        map_id,
        revision: _,
        bounds: Bounds { min: _, max: _ },
        cell_size_cm: _,
        seed: _,
        recipe_version: _,
        theme,
        terrain_base_cm: _,
        heightmaps,
        nodes,
        roads,
        surface_areas,
        buildings,
        zones,
        assets,
        placements,
        repetitions,
        attributions,
        provenance,
        ..
    } = document;
    let mut retained = Retained(u64::try_from(size_of::<MapDocument>()).map_err(|_| overflow())?);
    retained.string(map_id)?;
    retained.string(theme)?;
    retained.vector(heightmaps)?;
    for Heightmap {
        cell: Cell { x: _, y: _ },
        path,
        spacing_cm: _,
        offset_cm: _,
        step_cm: _,
        source_accuracy_cm: _,
    } in heightmaps
    {
        retained.string(path)?;
    }
    retained.vector(nodes)?;
    for RoadNode {
        id,
        position: _,
        level: _,
    } in nodes
    {
        retained.string(id)?;
    }
    retained.vector(roads)?;
    for Road {
        id,
        from,
        to,
        points,
        widths_cm,
        surfaces,
        kind: _,
        clearance_cm: _,
        sidewalk_cm: _,
        markings,
    } in roads
    {
        retained.string(id)?;
        retained.string(from)?;
        retained.string(to)?;
        retained.vector(points)?;
        retained.vector(widths_cm)?;
        retained.vector(surfaces)?;
        if let Some(RoadMarkings {
            lanes: _,
            center_line: _,
            edge_lines: _,
            crosswalk_start: _,
            crosswalk_end: _,
        }) = markings
        {}
    }
    retained.vector(surface_areas)?;
    for SurfaceArea {
        id,
        polygon,
        surface: _,
    } in surface_areas
    {
        retained.string(id)?;
        retained.vector(polygon)?;
    }
    retained.vector(buildings)?;
    for Building {
        id,
        footprint,
        holes,
        base_cm: _,
        height_cm: _,
        usage,
        material,
        roof,
        entrances,
    } in buildings
    {
        retained.string(id)?;
        retained.vector(footprint)?;
        retained.polygons(holes)?;
        retained.string(usage)?;
        retained.string(material)?;
        retained.string(roof)?;
        retained.polygons(entrances)?;
    }
    retained.vector(zones)?;
    for Zone {
        id,
        polygon,
        kind: _,
        spacing_cm: _,
        density_per_mille: _,
        exclusions,
        tree,
    } in zones
    {
        retained.string(id)?;
        retained.vector(polygon)?;
        retained.polygons(exclusions)?;
        if let Some(tree) = tree { retained.string(&tree.asset_id)?; }
    }
    retained.vector(assets)?;
    for Asset {
        id,
        path,
        attribution,
        collision,
        convex_collision,
        material,
    } in assets
    {
        retained.string(id)?;
        retained.string(path)?;
        retained.attribution(attribution)?;
        retained.vector(collision)?;
        for CollisionBox {
            center: _,
            size_cm: _,
        } in collision
        {}
        retained.vector(convex_collision)?;
        for CollisionConvex { vertices, faces } in convex_collision {
            retained.vector(vertices)?;
            retained.vector(faces)?;
        }
        if let Some(AssetMaterial {
            albedo_rgba: _,
            metallic_per_mille: _,
            roughness_per_mille: _,
            double_sided: _,
            albedo_texture,
        }) = material
        {
            if let Some(texture) = albedo_texture {
                retained.string(texture)?;
            }
        }
    }
    retained.vector(placements)?;
    for Placement {
        id,
        asset_id,
        position: _,
        quarter_turns: _,
    } in placements
    {
        retained.string(id)?;
        retained.string(asset_id)?;
    }
    retained.vector(repetitions)?;
    for Repetition {
        id,
        asset_id,
        points,
        spacing_cm: _,
    } in repetitions
    {
        retained.string(id)?;
        retained.string(asset_id)?;
        retained.vector(points)?;
    }
    retained.vector(attributions)?;
    for attribution in attributions {
        retained.attribution(attribution)?;
    }
    retained.provenance(provenance)?;
    Ok(retained.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn empty() -> MapDocument {
        serde_json::from_str(
            r#"{
            "map_id":"","revision":0,"bounds":{"min":[0,0],"max":[0,0]},
            "cell_size_cm":0,"seed":0,"recipe_version":0,"theme":"","terrain_base_cm":0,
            "heightmaps":[],"nodes":[],"roads":[],"surface_areas":[],"buildings":[],
            "zones":[],"assets":[],"placements":[],"repetitions":[],"attributions":[],
            "provenance":{"tool_id":"","version":"","build_id":"","fingerprint":"",
                "first_created":"","last_edited":""}
        }"#,
        )
        .unwrap()
    }

    fn string(capacity: usize) -> String {
        let mut value = String::with_capacity(capacity);
        value.push('x');
        value
    }

    fn vector<T>(capacity: usize, value: T) -> Vec<T> {
        let mut values = Vec::with_capacity(capacity);
        values.push(value);
        values
    }

    fn allocation(capacity: usize, element_bytes: usize) -> u64 {
        (capacity * element_bytes) as u64 + ALLOCATION_OVERHEAD
    }

    #[test]
    fn root_layout_and_empty_reserved_buffers_are_charged() {
        let mut d = empty();
        assert_eq!(
            document_retained_bytes(&d).unwrap(),
            size_of::<MapDocument>() as u64
        );
        d.map_id = String::with_capacity(97);
        d.nodes = Vec::with_capacity(23);
        let expected = size_of::<MapDocument>() as u64
            + allocation(d.map_id.capacity(), 1)
            + allocation(d.nodes.capacity(), size_of::<RoadNode>());
        assert_eq!(document_retained_bytes(&d).unwrap(), expected);
        assert!(d.map_id.is_empty() && d.nodes.is_empty());
    }

    #[test]
    fn nested_capacities_and_all_metadata_are_counted_without_validation() {
        let mut d = empty();
        d.map_id = string(101);
        d.theme = string(103);
        d.provenance = Provenance {
            tool_id: string(107),
            version: string(109),
            build_id: string(113),
            fingerprint: string(127),
            first_created: string(131),
            last_edited: string(137),
        };
        d.attributions = vector(
            2,
            Attribution {
                source: string(139),
                license: string(149),
                notice: string(65_537),
            },
        );
        d.buildings = vector(
            3,
            Building {
                id: string(151),
                footprint: vector(11, [i64::MAX, i64::MIN]),
                holes: vector(5, vector(13, [i64::MAX, i64::MIN])),
                base_cm: i64::MIN,
                height_cm: u32::MAX,
                usage: string(157),
                material: string(163),
                roof: string(167),
                entrances: vector(7, vector(17, [i64::MAX, i64::MIN])),
            },
        );
        d.zones = vector(
            19,
            Zone {
                tree: Some(mapkit_core::ZoneTree { asset_id: string(177), radius_cm: 58, clearance_cm: 5 }),
                id: string(173),
                polygon: vector(23, [0, 0]),
                kind: ZoneKind::Forest,
                spacing_cm: 0,
                density_per_mille: u16::MAX,
                exclusions: vector(29, vector(31, [0, 0])),
            },
        );
        d.assets = vector(
            37,
            Asset {
                id: string(179),
                path: string(181),
                attribution: Attribution {
                    source: string(191),
                    license: string(193),
                    notice: string(197),
                },
                collision: vector(
                    41,
                    CollisionBox {
                        center: [0; 3],
                        size_cm: [0; 3],
                    },
                ),
                convex_collision: vector(
                    43,
                    CollisionConvex {
                        vertices: vector(47, [i64::MAX; 3]),
                        faces: vector(53, [u8::MAX; 3]),
                    },
                ),
                material: Some(AssetMaterial {
                    albedo_rgba: [0; 4],
                    metallic_per_mille: 0,
                    roughness_per_mille: 0,
                    double_sided: false,
                    albedo_texture: Some(string(199)),
                }),
            },
        );
        let before = serde_json::to_vec(&d).unwrap();
        let mut expected = size_of::<MapDocument>() as u64;
        let strings = [
            &d.map_id,
            &d.theme,
            &d.provenance.tool_id,
            &d.provenance.version,
            &d.provenance.build_id,
            &d.provenance.fingerprint,
            &d.provenance.first_created,
            &d.provenance.last_edited,
            &d.attributions[0].source,
            &d.attributions[0].license,
            &d.attributions[0].notice,
            &d.buildings[0].id,
            &d.buildings[0].usage,
            &d.buildings[0].material,
            &d.buildings[0].roof,
            &d.zones[0].id,
            &d.zones[0].tree.as_ref().unwrap().asset_id,
            &d.assets[0].id,
            &d.assets[0].path,
            &d.assets[0].attribution.source,
            &d.assets[0].attribution.license,
            &d.assets[0].attribution.notice,
            d.assets[0]
                .material
                .as_ref()
                .unwrap()
                .albedo_texture
                .as_ref()
                .unwrap(),
        ];
        for value in strings {
            expected += allocation(value.capacity(), 1);
        }
        for (capacity, element_bytes) in [
            (d.attributions.capacity(), size_of::<Attribution>()),
            (d.buildings.capacity(), size_of::<Building>()),
            (d.buildings[0].footprint.capacity(), size_of::<Point>()),
            (d.buildings[0].holes.capacity(), size_of::<Vec<Point>>()),
            (d.buildings[0].holes[0].capacity(), size_of::<Point>()),
            (d.buildings[0].entrances.capacity(), size_of::<Vec<Point>>()),
            (d.buildings[0].entrances[0].capacity(), size_of::<Point>()),
            (d.zones.capacity(), size_of::<Zone>()),
            (d.zones[0].polygon.capacity(), size_of::<Point>()),
            (d.zones[0].exclusions.capacity(), size_of::<Vec<Point>>()),
            (d.zones[0].exclusions[0].capacity(), size_of::<Point>()),
            (d.assets.capacity(), size_of::<Asset>()),
            (d.assets[0].collision.capacity(), size_of::<CollisionBox>()),
            (
                d.assets[0].convex_collision.capacity(),
                size_of::<CollisionConvex>(),
            ),
            (
                d.assets[0].convex_collision[0].vertices.capacity(),
                size_of::<Vertex>(),
            ),
            (
                d.assets[0].convex_collision[0].faces.capacity(),
                size_of::<[u8; 3]>(),
            ),
        ] {
            expected += allocation(capacity, element_bytes);
        }
        assert_eq!(document_retained_bytes(&d).unwrap(), expected);
        assert_eq!(serde_json::to_vec(&d).unwrap(), before);
        assert!(
            d.validate().is_err(),
            "the ownership walker must not assume valid geometry/schema values"
        );
    }

    #[test]
    fn transport_geometry_and_optional_inline_fields_do_not_hide_allocations() {
        let mut d = empty();
        d.heightmaps = vector(
            2,
            Heightmap {
                cell: Cell { x: 0, y: 0 },
                path: string(101),
                spacing_cm: 0,
                offset_cm: 0,
                step_cm: 0,
                source_accuracy_cm: Some(0),
            },
        );
        d.nodes = vector(
            3,
            RoadNode {
                id: string(103),
                position: [0; 3],
                level: 0,
            },
        );
        d.roads = vector(
            5,
            Road {
                id: string(107),
                from: string(109),
                to: string(113),
                points: vector(7, [0; 3]),
                widths_cm: vector(11, 0),
                surfaces: vector(13, Surface::Grass),
                kind: RoadKind::Tunnel,
                clearance_cm: Some(0),
                sidewalk_cm: Some(0),
                markings: Some(RoadMarkings {
                    lanes: 0,
                    center_line: false,
                    edge_lines: false,
                    crosswalk_start: false,
                    crosswalk_end: false,
                }),
            },
        );
        d.surface_areas = vector(
            17,
            SurfaceArea {
                id: string(127),
                polygon: vector(19, [0; 2]),
                surface: Surface::Dirt,
            },
        );
        d.placements = vector(
            23,
            Placement {
                id: string(131),
                asset_id: string(137),
                position: [0; 3],
                quarter_turns: 255,
            },
        );
        d.repetitions = vector(
            29,
            Repetition {
                id: string(139),
                asset_id: string(149),
                points: vector(31, [0; 3]),
                spacing_cm: 0,
            },
        );
        let mut expected = size_of::<MapDocument>() as u64;
        for value in [
            &d.heightmaps[0].path,
            &d.nodes[0].id,
            &d.roads[0].id,
            &d.roads[0].from,
            &d.roads[0].to,
            &d.surface_areas[0].id,
            &d.placements[0].id,
            &d.placements[0].asset_id,
            &d.repetitions[0].id,
            &d.repetitions[0].asset_id,
        ] {
            expected += allocation(value.capacity(), 1);
        }
        for (capacity, element_bytes) in [
            (d.heightmaps.capacity(), size_of::<Heightmap>()),
            (d.nodes.capacity(), size_of::<RoadNode>()),
            (d.roads.capacity(), size_of::<Road>()),
            (d.roads[0].points.capacity(), size_of::<Vertex>()),
            (d.roads[0].widths_cm.capacity(), size_of::<u32>()),
            (d.roads[0].surfaces.capacity(), size_of::<Surface>()),
            (d.surface_areas.capacity(), size_of::<SurfaceArea>()),
            (d.surface_areas[0].polygon.capacity(), size_of::<Point>()),
            (d.placements.capacity(), size_of::<Placement>()),
            (d.repetitions.capacity(), size_of::<Repetition>()),
            (d.repetitions[0].points.capacity(), size_of::<Vertex>()),
        ] {
            expected += allocation(capacity, element_bytes);
        }
        assert_eq!(document_retained_bytes(&d).unwrap(), expected);
        d.roads[0].markings = None;
        d.roads[0].clearance_cm = None;
        d.heightmaps[0].source_accuracy_cm = None;
        assert_eq!(
            document_retained_bytes(&d).unwrap(),
            expected,
            "inline options do not change reserved layout"
        );
    }

    #[test]
    fn large_unvalidated_nested_buffers_keep_their_full_spare_capacity() {
        let mut d = empty();
        let mut holes = Vec::with_capacity(4096);
        for _ in 0..2049 {
            holes.push(vector(128, [i64::MAX, i64::MIN]));
        }
        let nested_bytes: u64 = holes
            .iter()
            .map(|points| allocation(points.capacity(), size_of::<Point>()))
            .sum();
        let holes_bytes = allocation(holes.capacity(), size_of::<Vec<Point>>());
        d.buildings = vector(
            5,
            Building {
                id: String::new(),
                footprint: Vec::new(),
                holes,
                base_cm: i64::MIN,
                height_cm: 0,
                usage: String::new(),
                material: String::new(),
                roof: String::new(),
                entrances: Vec::new(),
            },
        );
        let expected = size_of::<MapDocument>() as u64
            + allocation(d.buildings.capacity(), size_of::<Building>())
            + holes_bytes
            + nested_bytes;
        assert_eq!(document_retained_bytes(&d).unwrap(), expected);
        assert!(expected > 4 * 1024 * 1024);
        assert_eq!(d.buildings[0].holes.len(), 2049);
        assert_eq!(d.buildings[0].holes[0].len(), 1);
    }

    #[test]
    fn checked_accounting_rejects_multiplication_and_sum_overflow() {
        let mut retained = Retained::default();
        // On 32-bit targets no two usize values can overflow a u64 product.
        if usize::BITS == 64 {
            assert_eq!(
                retained.allocation(usize::MAX, 2).unwrap_err().code,
                "E_MEMORY_BUDGET"
            );
        }
        assert_eq!(retained.0, 0);
        retained.0 = u64::MAX - ALLOCATION_OVERHEAD;
        assert_eq!(
            retained.allocation(1, 1).unwrap_err().code,
            "E_MEMORY_BUDGET"
        );
        assert_eq!(retained.0, u64::MAX - ALLOCATION_OVERHEAD);
        retained.0 = u64::MAX;
        assert_eq!(retained.add(1).unwrap_err().code, "E_MEMORY_BUDGET");
        assert_eq!(retained.0, u64::MAX);
    }

    #[test]
    fn public_document_schema_requires_an_explicit_ownership_decision() {
        let schema = schemars::schema_for!(MapDocument);
        let actual: BTreeSet<_> = schema
            .schema
            .object
            .unwrap()
            .properties
            .into_keys()
            .collect();
        let expected: BTreeSet<_> = [
            "map_id",
            "revision",
            "bounds",
            "cell_size_cm",
            "seed",
            "recipe_version",
            "theme",
            "terrain_base_cm",
            "heightmaps",
            "nodes",
            "roads",
            "surface_areas",
            "buildings",
            "zones",
            "assets",
            "placements",
            "repetitions",
            "attributions",
            "provenance",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        assert_eq!(
            actual, expected,
            "update the retained ownership walker for every new document field"
        );
    }
}
