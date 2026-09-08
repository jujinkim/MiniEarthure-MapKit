//! Typed native ownership for full occupied volumes; separate from v6 chunk data.
use godot::prelude::*;
use mapkit_core::{GeneratedOccupancy, SolidShape};
use std::collections::BTreeMap;

#[derive(GodotClass)]
#[class(base=RefCounted, no_init)]
pub struct MapKitPackedOccupancy {
    base: Base<RefCounted>,
    kinds: PackedByteArray,
    values: PackedInt64Array,
    indices: PackedInt32Array,
    ids: PackedStringArray,
    slope_tops: PackedInt64Array,
    convex_vertices: PackedInt64Array,
    convex_faces: PackedByteArray,
}
#[godot_api]
impl MapKitPackedOccupancy {
    #[func]
    fn view(&self) -> VarDictionary {
        vdict! {
            "occupancy_version" => 1i64,
            "shape_kinds" => &self.kinds,
            "shape_values_cm" => &self.values,
            "object_indices" => &self.indices,
            "object_ids" => &self.ids,
            "slope_tops_cm" => &self.slope_tops,
            "convex_vertices_cm" => &self.convex_vertices,
            "convex_faces" => &self.convex_faces,
        }
    }
}

pub fn response(result: mapkit_core::Result<GeneratedOccupancy>) -> VarDictionary {
    super::packed::respond(result.and_then(|result| {
        let mut kinds = Vec::with_capacity(result.solids.len());
        let mut values = Vec::with_capacity(result.solids.len() * 8);
        let mut slope_tops=vec![];
        let mut convex_vertices=vec![];
        let mut convex_faces=vec![];
        let mut indices = Vec::with_capacity(result.solids.len());
        let mut ids = PackedStringArray::new();
        let mut index = BTreeMap::<&str, i32>::new();
        for solid in &result.solids {
            let id = *index.entry(solid.object_id.as_str()).or_insert_with(|| {
                let next = ids.len() as i32;
                ids.push(&GString::from(solid.object_id.as_str()));
                next
            });
            indices.push(id);
            match &solid.shape {
                SolidShape::Convex(c) => {
                    kinds.push(3);
                    values.extend_from_slice(&[convex_vertices.len() as i64,c.vertices.len() as i64,convex_faces.len() as i64,c.faces.len() as i64,0,0,0,0]);
                    convex_vertices.extend(c.vertices.iter().flatten().copied());convex_faces.extend(c.faces.iter().flatten().copied());
                }
                SolidShape::Box { min, max } => {
                    kinds.push(0);
                    values.extend_from_slice(min);
                    values.extend_from_slice(max);
                    values.extend_from_slice(&[0, 0]);
                }
                SolidShape::TriangularPrism {
                    footprint,
                    bottom_cm,
                    top_cm,
                } => {
                    kinds.push(1);
                    values.extend(footprint.iter().flatten().copied());
                    values.extend_from_slice(&[*bottom_cm, *top_cm]);
                }
                SolidShape::SlopedPrism {footprint,bottom_cm,top_cm} => {
                    kinds.push(2);values.extend(footprint.iter().flatten().copied());
                    values.extend_from_slice(&[*bottom_cm,slope_tops.len() as i64]);
                    slope_tops.extend_from_slice(top_cm);
                }
            }
        }
        let occupancy = Gd::from_init_fn(|base| MapKitPackedOccupancy {
            base,
            convex_vertices: PackedInt64Array::from(convex_vertices.as_slice()),
            convex_faces: PackedByteArray::from(convex_faces.as_slice()),
            kinds: PackedByteArray::from(kinds.as_slice()),
            values: PackedInt64Array::from(values.as_slice()),
            indices: PackedInt32Array::from(indices.as_slice()),
            ids,
            slope_tops: PackedInt64Array::from(slope_tops.as_slice()),
        });
        // Solid arrays and normal triangle packing coexist at peak; callers must
        // reserve both representations before invoking this generation method.
        let mut packed = super::packed::pack(result.chunk)?;
        packed.set("occupancy", &occupancy);
        Ok(packed)
    }))
}
