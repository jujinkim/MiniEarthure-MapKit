use godot::prelude::*;
use mapkit_core::{canonical, Cell, GenerationInput, SpawnRequest};
use mapkit_package::{pack_bytes, read, read_bytes, read_project, write_new, Package};
use std::path::Path;
struct MapKitExtension;
#[gdextension]
unsafe impl ExtensionLibrary for MapKitExtension {}
#[derive(GodotClass)]
#[class(base=RefCounted)]
struct MapKitBridge {
    base: Base<RefCounted>,
    package: Option<Package>,
}
#[godot_api]
impl IRefCounted for MapKitBridge {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            base,
            package: None,
        }
    }
}
fn response(result: mapkit_core::Result<serde_json::Value>) -> GString {
    let value = match result {
        Ok(data) => serde_json::json!({"ok":true,"data":data}),
        Err(e) => serde_json::json!({"ok":false,"error":e}),
    };
    GString::from(serde_json::to_string(&value).unwrap().as_str())
}
fn engine_document(text: &str) -> mapkit_core::Result<mapkit_core::MapDocument> {
    let mut value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| mapkit_core::error("E_JSON", e.to_string()))?;
    fn integers(v: &mut serde_json::Value) -> mapkit_core::Result<()> {
        match v {
            serde_json::Value::Number(n) if n.is_f64() => {
                let f = n.as_f64().unwrap();
                if f.fract() != 0.0 || f.abs() > 9_007_199_254_740_991.0 {
                    return Err(mapkit_core::error(
                        "E_JSON",
                        "engine map numbers must be exactly representable integers",
                    ));
                }
                *v = serde_json::Value::from(f as i64);
            }
            serde_json::Value::Array(a) => {
                for v in a {
                    integers(v)?;
                }
            }
            serde_json::Value::Object(o) => {
                for v in o.values_mut() {
                    integers(v)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    integers(&mut value)?;
    serde_json::from_value(value).map_err(|e| mapkit_core::error("E_JSON", e.to_string()))
}
#[godot_api]
impl MapKitBridge {
    #[func]
    fn open_package(&mut self, path: GString) -> GString {
        self.package = None;
        response(read(Path::new(&path.to_string())).map(|p| {
            let info = serde_json::to_value(&p.inspection).unwrap();
            self.package = Some(p);
            info
        }))
    }
    #[func]
    fn open_project(&mut self, path: GString) -> GString {
        self.package = None;
        response(
            read_project(Path::new(&path.to_string()))
                .and_then(|(d, f)| pack_bytes(d, f))
                .and_then(|b| read_bytes(&b))
                .map(|p| {
                    let info = serde_json::to_value(&p.inspection).unwrap();
                    self.package = Some(p);
                    info
                }),
        )
    }
    #[func]
    fn document_json(&self) -> GString {
        response(
            self.package
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))
                .map(|p| serde_json::to_value(&p.document).unwrap()),
        )
    }
    #[func]
    fn generate_chunk(&self, x: i32, y: i32) -> GString {
        response(
            self.package
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))
                .and_then(|p| p.generate(Cell { x, y }, 500_000))
                .map(|chunk| {
                    let hash = chunk.hash().unwrap();
                    serde_json::json!({"chunk":chunk,"generated_sha256":hash})
                }),
        )
    }
    #[func]
    fn preview_document(&self, document: GString, x: i32, y: i32) -> GString {
        response(engine_document(&document.to_string()).map_err(|e|mapkit_core::error("E_JSON",e.to_string())).and_then(|d|mapkit_core::generate(GenerationInput{document:&d,cell:Cell{x,y},heightgrid:None,max_triangles:500_000})).map(|chunk|serde_json::json!({"generated_sha256":chunk.hash().unwrap(),"chunk":chunk})))
    }
    #[func]
    fn validate_document(&self, document: GString) -> GString {
        response(
            engine_document(&document.to_string())
                .map_err(|e| mapkit_core::error("E_JSON", e.to_string()))
                .and_then(|mut d| {
                    d.normalize();
                    d.validate()?;
                    Ok(serde_json::json!({"canonical": String::from_utf8(canonical(&d)?).unwrap(), "document":d}))
                }),
        )
    }
    #[func]
    fn export_project(&self, path: GString, destination: GString) -> GString {
        response(
            read_project(Path::new(&path.to_string()))
                .and_then(|(d, f)| pack_bytes(d, f))
                .and_then(|bytes| {
                    let p = read_bytes(&bytes)?;
                    write_new(Path::new(&destination.to_string()), &bytes)?;
                    Ok(serde_json::to_value(p.inspection).unwrap())
                }),
        )
    }
    #[func]
    fn spawn(&self, x_cm: i64, y_cm: i64, surface: GString) -> GString {
        response(self.package.as_ref().ok_or_else(||mapkit_core::error("E_STATE","open package first")).and_then(|p| {
            let cell=p.document.cell_at([x_cm,y_cm]).ok_or_else(||mapkit_core::error("E_SPAWN","outside map"))?;
            let position=p.generate(cell,500_000)?.spawn(&SpawnRequest{position_cm:[x_cm,y_cm],surface_id:surface.to_string()})?;
            Ok(serde_json::json!({"position_cm":position,"window":p.document.window([x_cm,y_cm])}))
        }))
    }
    #[func]
    fn canonical_document(&self) -> GString {
        self.package
            .as_ref()
            .and_then(|p| canonical(&p.document).ok())
            .map(|b| GString::from(String::from_utf8(b).unwrap().as_str()))
            .unwrap_or_default()
    }
}
