use mapkit_core::*;
use mapkit_package::*;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Write},
};

fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}
fn asset_doc(path: &str) -> MapDocument {
    let mut d = document();
    d.assets.push(Asset {
        id: "custom".into(),
        path: path.into(),
        attribution: Attribution {
            source: "original synthetic".into(),
            license: "MIT".into(),
            notice: "".into(),
        },
        collision: vec![],
    });
    d
}
fn png() -> Vec<u8> {
    let mut out = vec![];
    {
        let mut e = png::Encoder::new(&mut out, 2, 2);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .unwrap()
            .write_image_data(&[255; 16])
            .unwrap();
    }
    out
}
fn webp() -> Vec<u8> {
    let mut out = vec![];
    image_webp::WebPEncoder::new(&mut out)
        .encode(&[127; 16], 2, 2, image_webp::ColorType::Rgba8)
        .unwrap();
    out
}
fn glb_json() -> Value {
    json!({"asset":{"version":"2.0"},"buffers":[{"byteLength":42}],"bufferViews":[{"buffer":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":6}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}],"nodes":[{"mesh":0}],"scenes":[{"nodes":[0]}],"scene":0})
}
fn bin() -> Vec<u8> {
    let mut b = vec![];
    for v in [0f32, 0., 0., 1., 0., 0., 0., 1., 0.] {
        b.extend(v.to_le_bytes());
    }
    for i in [0u16, 1, 2] {
        b.extend(i.to_le_bytes());
    }
    b
}
fn glb_raw(mut j: Vec<u8>, mut b: Vec<u8>) -> Vec<u8> {
    while j.len() % 4 != 0 {
        j.push(b' ');
    }
    while b.len() % 4 != 0 {
        b.push(0);
    }
    let mut out = b"glTF".to_vec();
    out.extend(2u32.to_le_bytes());
    out.extend(((28 + j.len() + b.len()) as u32).to_le_bytes());
    out.extend((j.len() as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(j);
    out.extend((b.len() as u32).to_le_bytes());
    out.extend(b"BIN\0");
    out.extend(b);
    out
}
fn glb(j: Value, b: Vec<u8>) -> Vec<u8> {
    glb_raw(serde_json::to_vec(&j).unwrap(), b)
}
fn accepts(path: &str, b: Vec<u8>) {
    let p = pack_bytes(asset_doc(path), BTreeMap::from([(path.into(), b)])).unwrap();
    assert!(read_bytes(&p).is_ok());
}
fn rejects(path: &str, b: Vec<u8>, code: &str) {
    let err = pack_bytes(asset_doc(path), BTreeMap::from([(path.into(), b)])).unwrap_err();
    assert_eq!(err.code, code, "{}", err.message);
}

#[test]
fn complete_static_assets_decode_and_roundtrip() {
    accepts("assets/a.png", png());
    accepts("assets/a.webp", webp());
    accepts("assets/a.glb", glb(glb_json(), bin()));
    let mut j = glb_json();
    j["materials"] = json!([{"pbrMetallicRoughness":{"baseColorFactor":[0.5,1.0,0.0,1.0],"roughnessFactor":0.2}}]);
    j["meshes"][0]["primitives"][0]["material"] = json!(0);
    accepts("assets/a.glb", glb(j, bin()));
}
#[test]
fn image_headers_are_not_acceptance() {
    let good = png();
    rejects("assets/a.png", good[..33].to_vec(), "E_ASSET");
    let mut trailing = good.clone();
    trailing.push(0);
    rejects("assets/a.png", trailing, "E_ASSET");
    let mut data = good.clone();
    let p = data.windows(4).position(|w| w == b"IDAT").unwrap();
    let n = u32::from_be_bytes(data[p - 4..p].try_into().unwrap()) as usize;
    data[p + 4] ^= 255;
    let crc = crc32fast::hash(&data[p..p + 4 + n]);
    data[p + 4 + n..p + 8 + n].copy_from_slice(&crc.to_be_bytes());
    rejects("assets/a.png", data, "E_ASSET"); // Valid chunks/CRC; invalid compressed pixels.
    let mut data = webp();
    data.truncate(26);
    let n = (data.len() - 8) as u32;
    data[4..8].copy_from_slice(&n.to_le_bytes());
    let n = (data.len() - 20) as u32;
    data[16..20].copy_from_slice(&n.to_le_bytes());
    rejects("assets/a.webp", data, "E_ASSET");
    let mut data = webp();
    data.push(0);
    rejects("assets/a.webp", data, "E_ASSET");
}
fn png_chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend(kind);
    out.extend(data);
    let crc = crc32fast::hash(&out[4..]);
    out.extend(crc.to_be_bytes());
    out
}
#[test]
fn animated_and_oversized_images_reject_before_decode() {
    let mut a = png();
    a.splice(33..33, png_chunk(b"acTL", &[0, 0, 0, 1, 0, 0, 0, 0]));
    rejects("assets/a.png", a, "E_ASSET");
    let mut a = png();
    a[16..20].copy_from_slice(&8193u32.to_be_bytes());
    let crc = crc32fast::hash(&a[12..29]);
    a[29..33].copy_from_slice(&crc.to_be_bytes());
    rejects("assets/a.png", a, "E_LIMIT");
    let mut a = b"RIFF".to_vec();
    a.extend(22u32.to_le_bytes());
    a.extend(b"WEBPVP8X");
    a.extend(10u32.to_le_bytes());
    a.extend([2, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    rejects("assets/a.webp", a, "E_ASSET");
    let mut a = webp();
    a[21..25].copy_from_slice(&(8192u32 | (1 << 14)).to_le_bytes());
    rejects("assets/a.webp", a, "E_LIMIT");
}
#[test]
fn glb_framing_duplicates_and_executable_resources_reject() {
    for key in ["animations", "skins", "extensions", "extras", "scripts"] {
        let mut j = glb_json();
        j[key] = json!([]);
        rejects("assets/a.glb", glb(j, bin()), "E_ASSET");
    }
    for uri in [
        "https://example.invalid/a",
        "data:application/octet-stream;base64,AA==",
        "../a.bin",
    ] {
        let mut j = glb_json();
        j["buffers"][0]["uri"] = json!(uri);
        rejects("assets/a.glb", glb(j, bin()), "E_ASSET");
    }
    let j = serde_json::to_string(&glb_json()).unwrap().replacen(
        "\"version\":\"2.0\"",
        "\"version\":\"2.0\",\"version\":\"2.0\"",
        1,
    );
    rejects("assets/a.glb", glb_raw(j.into_bytes(), bin()), "E_ASSET");
    let mut b = glb(glb_json(), bin());
    b.extend(b"hidden");
    let len = b.len() as u32;
    b[8..12].copy_from_slice(&len.to_le_bytes());
    rejects("assets/a.glb", b, "E_ASSET");
    rejects("assets/a.pck", b"GDPC".to_vec(), "E_ASSET");
    rejects("assets/a.gd", b"extends Node".to_vec(), "E_ASSET");
}
#[test]
fn glb_ranges_indices_floats_materials_and_cycles_reject() {
    for (pointer, value) in [
        ("/bufferViews/0/byteLength", json!(1000)),
        ("/accessors/0/count", json!(4)),
        ("/accessors/0/byteOffset", json!(1)),
        ("/accessors/0/byteOffset", json!(u64::MAX)),
        ("/accessors/0/count", json!(1_000_001)),
        ("/meshes/0/primitives/0/attributes/POSITION", json!(999)),
        ("/meshes/0/primitives/0/mode", json!(1)),
        ("/buffers/0/byteLength", json!(999)),
    ] {
        let mut j = glb_json(); // Some optional fields need inserting, rather than pointer_mut.
        if pointer.ends_with("byteOffset") {
            j["accessors"][0]["byteOffset"] = value;
        } else if pointer.ends_with("mode") {
            j["meshes"][0]["primitives"][0]["mode"] = value;
        } else {
            *j.pointer_mut(pointer).unwrap() = value;
        }
        let expected = if pointer == "/accessors/0/count" && j["accessors"][0]["count"] == 1_000_001
        {
            "E_LIMIT"
        } else {
            "E_ASSET"
        };
        rejects("assets/a.glb", glb(j, bin()), expected);
    }
    let mut b = bin();
    b[36..38].copy_from_slice(&3u16.to_le_bytes());
    rejects("assets/a.glb", glb(glb_json(), b), "E_ASSET");
    let mut b = bin();
    b[..4].copy_from_slice(&f32::NAN.to_le_bytes());
    rejects("assets/a.glb", glb(glb_json(), b), "E_ASSET");
    let mut j = glb_json();
    j["nodes"] = json!([{"children":[1]},{"children":[0]}]);
    j["scenes"] = json!([]);
    j.as_object_mut().unwrap().remove("scene");
    rejects("assets/a.glb", glb(j, bin()), "E_ASSET");
    let mut j = glb_json();
    j["nodes"] = json!([{"children":[1,1]},{"mesh":0}]);
    rejects("assets/a.glb", glb(j, bin()), "E_ASSET");
    let mut j = glb_json();
    j["materials"] = json!([{"pbrMetallicRoughness":{"metallicFactor":2}}]);
    rejects("assets/a.glb", glb(j, bin()), "E_ASSET");
    let mut j = glb_json();
    j["nodes"][0]["rotation"] = json!([0, 0, 0, 0]);
    rejects("assets/a.glb", glb(j, bin()), "E_ASSET");
}
#[test]
fn embedded_image_is_fully_decoded_and_budgeted() {
    let mut j = glb_json();
    let mut b = bin();
    b.extend([0, 0]);
    let image = png();
    j["bufferViews"]
        .as_array_mut()
        .unwrap()
        .push(json!({"buffer":0,"byteOffset":44,"byteLength":image.len()}));
    j["images"] = json!([{"bufferView":2,"mimeType":"image/png"}]);
    b.extend(&image);
    j["buffers"][0]["byteLength"] = json!(b.len());
    let path = "assets/a.glb";
    let p = pack_bytes(
        asset_doc(path),
        BTreeMap::from([(path.into(), glb(j.clone(), b.clone()))]),
    )
    .unwrap();
    let cost = inspect_read_cost(&p).unwrap();
    assert!(cost.validation_peak_bytes >= 256 * 1024 * 1024);
    assert_eq!(
        read_bytes_with_budget(&p, cost.validation_peak_bytes - 1)
            .err()
            .unwrap()
            .code,
        "E_MEMORY_BUDGET"
    );
    assert!(read_bytes_with_budget(&p, cost.validation_peak_bytes).is_ok());
    b.truncate(44 + 33);
    j["bufferViews"][2]["byteLength"] = json!(33);
    j["buffers"][0]["byteLength"] = json!(b.len());
    rejects(path, glb(j, b), "E_ASSET");
}

fn entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut b = vec![];
            f.read_to_end(&mut b).unwrap();
            (f.name().into(), b)
        })
        .collect()
}
fn zip_entries(entries: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(vec![]));
    for (p, b) in entries {
        z.start_file(p, zip::write::FileOptions::default()).unwrap();
        z.write_all(&b).unwrap();
    }
    z.finish().unwrap().into_inner()
}
#[test]
fn honest_inventory_cannot_bypass_late_asset_failure() {
    let path = "assets/a.png";
    let mut d = asset_doc(path);
    d.normalize();
    let good = pack_bytes(d.clone(), BTreeMap::from([(path.into(), png())])).unwrap();
    let mut e = entries(&good);
    let invalid = png()[..33].to_vec();
    e.iter_mut().find(|(p, _)| p == path).unwrap().1 = invalid.clone();
    let mut m: Value = serde_json::from_slice(&e[0].1).unwrap();
    for f in m["files"].as_array_mut().unwrap() {
        if f["path"] == path {
            f["size"] = json!(invalid.len());
            f["sha256"] = json!(sha256(&invalid));
        }
    }
    let mut doc = serde_json::to_value(d).unwrap();
    doc.as_object_mut().unwrap().remove("provenance");
    doc.as_object_mut().unwrap().remove("attributions");
    m["world_content_hash"] = json!(sha256(
        &canonical(&(doc, BTreeMap::from([(path, sha256(&invalid))]))).unwrap()
    ));
    e[0].1 = canonical(&m).unwrap();
    let damaged = zip_entries(e);
    assert!(inspect_read_cost(&damaged).is_ok());
    assert_eq!(read_bytes(&damaged).err().unwrap().code, "E_ASSET");
    assert!(read_bytes(&good).is_ok());
}

fn u32at(b: &[u8], p: usize) -> usize {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap()) as usize
}
fn set32(b: &mut [u8], p: usize, v: usize) {
    b[p..p + 4].copy_from_slice(&(v as u32).to_le_bytes());
}
#[test]
fn zip_rejects_hidden_bytes_overlaps_trailing_and_count_before_index() {
    let good = pack_bytes(document(), BTreeMap::new()).unwrap();
    let mut trailing = good.clone();
    trailing.extend(b"hidden");
    assert_eq!(inspect_read_cost(&trailing).unwrap_err().code, "E_ZIP");
    let end = good.len() - 22;
    let c = u32at(&good, end + 16);
    let mut b = good.clone();
    b[end + 8..end + 12].copy_from_slice(&[255; 4]);
    assert!(inspect_read_cost(&b).is_err());
    let mut b = good.clone();
    b[end + 8..end + 10].copy_from_slice(&8194u16.to_le_bytes());
    b[end + 10..end + 12].copy_from_slice(&8194u16.to_le_bytes());
    assert_eq!(inspect_read_cost(&b).unwrap_err().code, "E_LIMIT");
    let mut b = good.clone();
    b.splice(c..c, b"hidden".to_vec());
    set32(&mut b, end + 6 + 16, c + 6);
    assert_eq!(inspect_read_cost(&b).unwrap_err().code, "E_ZIP");
    let mut b = good.clone();
    set32(&mut b, c + 42, 1);
    assert!(inspect_read_cost(&b).is_err());
    // Advertise fewer central records: the physical unindexed entry must not disappear.
    let records = entries(&good);
    let mut b = zip_entries(vec![records[0].clone()]);
    let c = u32at(&b, b.len() - 6);
    b.splice(c..c, b"PK\x03\x04hidden".to_vec());
    let end = b.len() - 22;
    set32(&mut b, end + 16, c + 10);
    assert_eq!(inspect_read_cost(&b).unwrap_err().code, "E_ZIP");
}
#[test]
fn zip_deflate_must_end_exactly_within_declared_range() {
    let good = pack_bytes(document(), BTreeMap::new()).unwrap();
    let end = good.len() - 22;
    let c = u32at(&good, end + 16);
    let mut z = zip::ZipArchive::new(Cursor::new(&good)).unwrap();
    let f = z.by_index(1).unwrap();
    let h = f.header_start() as usize;
    let central = f.central_header_start() as usize;
    let size = f.compressed_size() as usize;
    drop(f);
    let mut b = good.clone();
    b.splice(c..c, [0u8]);
    set32(&mut b, h + 18, size + 1);
    set32(&mut b, central + 1 + 20, size + 1);
    set32(&mut b, end + 1 + 16, c + 1);
    assert!(inspect_read_cost(&b).is_ok());
    assert_eq!(read_bytes(&b).err().unwrap().code, "E_ZIP");
}
#[test]
fn arbitrary_truncations_and_bit_mutations_never_panic() {
    let good = pack_bytes(document(), BTreeMap::new()).unwrap();
    for end in 0..good.len() {
        assert!(read_bytes(&good[..end]).is_err());
    }
    for i in (0..good.len()).step_by(7) {
        let mut b = good.clone();
        b[i] ^= 0xff;
        let _ = read_bytes(&b);
    }
    for good in [png(), webp(), glb(glb_json(), bin())] {
        let suffix = if good.starts_with(b"glTF") {
            "glb"
        } else if good.starts_with(b"RIFF") {
            "webp"
        } else {
            "png"
        };
        for end in 0..good.len() {
            assert!(pack_bytes(
                asset_doc(&format!("a.{suffix}")),
                BTreeMap::from([(format!("a.{suffix}"), good[..end].to_vec())])
            )
            .is_err());
        }
    }
}
#[test]
fn heightmap_decode_checks_parameters_and_complete_image() {
    let h = Heightmap {
        cell: Cell { x: 0, y: 0 },
        path: "a.png".into(),
        spacing_cm: 0,
        offset_cm: i64::MAX,
        step_cm: 1,
        source_accuracy_cm: None,
    };
    assert!(decode_heightmap(&h, 51200, &png()).is_err());
    let h = Heightmap {
        spacing_cm: 200,
        offset_cm: 0,
        ..h
    };
    let mut image = vec![];
    {
        let mut e = png::Encoder::new(&mut image, 2, 2);
        e.set_color(png::ColorType::Grayscale);
        e.set_depth(png::BitDepth::Sixteen);
        e.write_header().unwrap().write_image_data(&[0; 8]).unwrap();
    }
    assert!(decode_heightmap(&h, 200, &image).is_ok());
    image.truncate(image.len() - 12);
    assert_eq!(
        decode_heightmap(&h, 200, &image).unwrap_err().code,
        "E_HEIGHTMAP"
    );
}
