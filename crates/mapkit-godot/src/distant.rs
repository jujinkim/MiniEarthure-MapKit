use godot::prelude::*;

/// Immutable COW view with an observable owner, separate from collision geometry.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init)]
struct MapKitFarGeometry {
    base: Base<RefCounted>,
    vertices: PackedVector3Array,
    normals: PackedVector3Array,
    colors: PackedColorArray,
}
#[godot_api]
impl MapKitFarGeometry {
    #[func]
    fn view(&self) -> VarDictionary {
        vdict! { "vertices" => &self.vertices, "normals" => &self.normals, "colors" => &self.colors }
    }
}
pub(super) fn pack(mesh: mapkit_package::distant::DistantMesh) -> VarDictionary {
    let points: Vec<_> = mesh
        .vertices
        .iter()
        .map(|p| Vector3::new(p[0], p[1], p[2]))
        .collect();
    let normals: Vec<_> = points
        .chunks_exact(3)
        .flat_map(|v| {
            let n = (v[2] - v[0]).cross(v[1] - v[0]);
            [if n.length_squared() > 0. {
                n.normalized()
            } else {
                Vector3::UP
            }; 3]
        })
        .collect();
    let colors: Vec<_> = mesh
        .colors
        .iter()
        .map(|c| {
            Color::from_rgba(
                c[0] as f32 / 255.,
                c[1] as f32 / 255.,
                c[2] as f32 / 255.,
                1.,
            )
        })
        .collect();
    let triangles = (points.len() / 3) as i64;
    let owner = Gd::from_init_fn(|base| MapKitFarGeometry {
        base,
        vertices: PackedVector3Array::from(points.as_slice()),
        normals: PackedVector3Array::from(normals.as_slice()),
        colors: PackedColorArray::from(colors.as_slice()),
    });
    vdict! { "geometry" => &owner, "triangles" => triangles,
    "retained_bytes" => 4096i64 + triangles * 128,
    "display_bytes" => 65536i64 + triangles * 256 }
}
