//! Copy course definitions to a revised map identity, without completion evidence.
use std::{env, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 4 {
        return Err("usage: reseal_courses NEW.memap NEW_DIRECTORY OLD.mecourse ...".into());
    }
    let package = mapkit_package::read(Path::new(&args[1]))?;
    let destination = Path::new(&args[2]);
    std::fs::create_dir_all(destination)?;
    for path in &args[3..] {
        let path = Path::new(path);
        let old: mapkit_core::course::Course = mapkit_core::course::decode(&std::fs::read(path)?)?;
        let mut definition = old.definition;
        definition.map_id = package.document.map_id.clone();
        definition.world_content_hash = package.inspection.world_content_hash.clone();
        let course =
            mapkit_core::course::Course::from_definition(definition, &package.document.bounds)?;
        mapkit_package::write_new(
            &destination.join(path.file_name().unwrap()),
            &mapkit_core::canonical(&course)?,
        )?;
        println!("{} {} unverified", path.display(), course.course_id);
    }
    Ok(())
}
