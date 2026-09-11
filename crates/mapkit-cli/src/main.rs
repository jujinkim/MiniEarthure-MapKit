use mapkit_core::*;
use mapkit_package::*;
use std::path::Path;
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let help="usage: mapkit inspect|validate|validate-cells PACKAGE | pack PROJECT OUTPUT.memap | unpack PACKAGE NEW_DIRECTORY | generate-chunk PACKAGE X Y OUTPUT.json | schema document|manifest OUTPUT.json | pack-regions PROJECT NEW.mkregions SIDE_CELLS | inspect-regions|audit-regions INDEXED MEMORY_BYTES | unpack-regions INDEXED NEW_DIRECTORY MEMORY_BYTES | generate-region-chunk INDEXED X Y NEW.json SOURCE_MEMORY_BYTES";
    let arg = |i: usize| {
        args.get(i)
            .map(String::as_str)
            .ok_or_else(|| error("E_USAGE", help))
    };
    match arg(0)? {
        "unpack-regions" if args.len() == 4 => {
            let budget = arg(3)?.parse::<u64>().map_err(|_| error("E_USAGE", "memory budget must be bytes"))?;
            let mut reader = indexed::IndexedReader::open_path(Path::new(arg(1)?), budget, None)?;
            reader.unpack_source(Path::new(arg(2)?), budget, &indexed::ReadEpoch::default().begin())?;
        }
        "pack-regions" if args.len() == 4 => {
            let side = arg(3)?.parse::<u32>().map_err(|_| error("E_USAGE", "storage side must be an integer cell count"))?;
            let (document, files) = indexed::read_source_project(Path::new(arg(1)?))?;
            let bytes = indexed::pack_source(document, files, side)?;
            let reader = indexed::IndexedReader::open(std::io::Cursor::new(&bytes), u64::MAX, None)?;
            let output = serde_json::json!({"index_sha256": reader.identity(), "package_bytes": bytes.len(),
                "regions": reader.index().regions.len(), "execution_cells": reader.index().world.cell_count()?,
                "world_content_hash": reader.index().world_content_hash});
            write_new(Path::new(arg(2)?), &bytes)?;
            println!("{output}");
        }
        "inspect-regions" | "audit-regions" if args.len() == 3 => {
            let budget = arg(2)?.parse::<u64>().map_err(|_| error("E_USAGE", "memory budget must be bytes"))?;
            let mut reader = indexed::IndexedReader::open_path(Path::new(arg(1)?), budget, None)?;
            if arg(0)? == "audit-regions" { reader.audit(budget, &indexed::ReadEpoch::default().begin())?; }
            println!("{}", serde_json::json!({"index_sha256": reader.identity(),
                "regions": reader.index().regions.len(), "execution_cells": reader.index().world.cell_count()?,
                "index_retained_bytes": reader.index_retained_bytes(),
                "verification": if arg(0)? == "audit-regions" { "complete-source" } else { "index-only" }}));
        }
        "generate-region-chunk" if args.len() == 6 => {
            let budget = arg(5)?.parse::<u64>().map_err(|_| error("E_USAGE", "source memory budget must be bytes"))?;
            let mut reader = indexed::IndexedReader::open_path(Path::new(arg(1)?), budget, None)?;
            let parse = |i| arg(i)?.parse::<i32>().map_err(|_| error("E_USAGE", "cell coordinate must be an integer"));
            let cell = Cell { x: parse(2)?, y: parse(3)? };
            let id = reader.region_for_cell(cell)?;
            let region = reader.load_region(id, budget, &indexed::ReadEpoch::default().begin())?;
            let generated = region.package.generate(cell, 500_000)?;
            write_new(Path::new(arg(4)?), &canonical(&generated)?)?;
            println!("{}", serde_json::json!({"generated_sha256": generated.hash()?,
                "region_identity": region.identity, "source_bytes_read": region.bytes_read,
                "source_cost": region.package.inspection}));
        }
        "validate-cells" if args.len() == 2 => {
            let p = read(Path::new(arg(1)?))?;
            let mut hashes = std::collections::BTreeMap::new();
            for cell in p.document.cells() {
                let chunk = p.generate(cell, 500_000)?;
                hashes.insert(format!("{}/{}", cell.x, cell.y), chunk.hash()?);
            }
            println!("{}", serde_json::to_string(&hashes).unwrap());
        }
        "inspect" | "validate" if args.len() == 2 => {
            let p = read(Path::new(arg(1)?))?;
            let mut output =
                serde_json::to_value(&p.inspection).map_err(|e| error("E_JSON", e.to_string()))?;
            if arg(0)? == "inspect" {
                output["manifest"] = serde_json::to_value(&p.manifest)
                    .map_err(|e| error("E_JSON", e.to_string()))?;
            }
            println!("{}", String::from_utf8(canonical(&output)?).unwrap());
        }
        "pack" if args.len() == 3 => {
            let (document, files) = read_project(Path::new(arg(1)?))?;
            let bytes = pack_bytes(document, files)?;
            let verified = read_bytes(&bytes)?;
            write_new(Path::new(arg(2)?), &bytes)?;
            println!(
                "{}",
                String::from_utf8(canonical(&verified.inspection)?).unwrap()
            );
        }
        "unpack" if args.len() == 3 => unpack(&read(Path::new(arg(1)?))?, Path::new(arg(2)?))?,
        "generate-chunk" if args.len() == 5 => {
            let p = read(Path::new(arg(1)?))?;
            let parse = |i| {
                arg(i)?
                    .parse::<i32>()
                    .map_err(|_| error("E_USAGE", "cell coordinates must be integers"))
            };
            let generated = p.generate(
                Cell {
                    x: parse(2)?,
                    y: parse(3)?,
                },
                500_000,
            )?;
            write_new(Path::new(arg(4)?), &canonical(&generated)?)?;
            println!("{}", generated.hash()?);
        }
        "schema" if args.len() == 3 => {
            let schema = match arg(1)? {
                "document" => schemars::schema_for!(MapDocument),
                "manifest" => schemars::schema_for!(PackageManifest),
                _ => return Err(error("E_USAGE", help)),
            };
            write_new(
                Path::new(arg(2)?),
                &serde_json::to_vec_pretty(&schema).map_err(|e| error("E_JSON", e.to_string()))?,
            )?;
        }
        _ => return Err(error("E_USAGE", help)),
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{}", serde_json::to_string(&e).unwrap());
        std::process::exit(1);
    }
}
