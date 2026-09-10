use mapkit_core::*;
use mapkit_package::*;
use std::path::Path;
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let help="usage: mapkit inspect|validate|validate-cells PACKAGE | pack PROJECT OUTPUT.memap | unpack PACKAGE NEW_DIRECTORY | generate-chunk PACKAGE X Y OUTPUT.json | schema document|manifest OUTPUT.json";
    let arg = |i: usize| {
        args.get(i)
            .map(String::as_str)
            .ok_or_else(|| error("E_USAGE", help))
    };
    match arg(0)? {
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
