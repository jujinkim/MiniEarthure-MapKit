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
        }
    }
}

pub fn response(result: mapkit_core::Result<GeneratedOccupancy>) -> VarDictionary {
    super::packed::respond(result.and_then(|result| {
        let mut kinds = Vec::with_capacity(result.solids.len());
        let mut values = Vec::with_capacity(result.solids.len() * 8);
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
            }
        }
        let occupancy = Gd::from_init_fn(|base| MapKitPackedOccupancy {
            base,
            kinds: PackedByteArray::from(kinds.as_slice()),
            values: PackedInt64Array::from(values.as_slice()),
            indices: PackedInt32Array::from(indices.as_slice()),
            ids,
        });
        // Solid arrays and normal triangle packing coexist at peak; callers must
        // reserve both representations before invoking this generation method.
        let mut packed = super::packed::pack(result.chunk)?;
        packed.set("occupancy", &occupancy);
        Ok(packed)
    }))
}
