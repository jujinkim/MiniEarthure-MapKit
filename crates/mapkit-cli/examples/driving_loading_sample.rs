//! Fixed short component samples; this does not assert end-to-end driving acceptance.
use std::{path::Path, time::Instant};
fn main() {
 let a: Vec<_> = std::env::args().collect();
 let opened=Instant::now();
 let p=mapkit_package::read(Path::new(&a[1])).unwrap();
 let open_ms=opened.elapsed().as_secs_f64()*1000.;
 let x:i64=a[2].parse().unwrap(); let y:i64=a[3].parse().unwrap();
 for c in p.document.window([x,y]) {
  let now=Instant::now(); let cost=p.document.estimate(c,500_000).unwrap();
  let cost_ms=now.elapsed().as_secs_f64()*1000.;
  let now=Instant::now(); let chunk=p.generate(c,500_000).unwrap();
  let generate_ms=now.elapsed().as_secs_f64()*1000.;
  println!("{}",serde_json::json!({"cell":c,"open_ms":open_ms,"cost_ms":cost_ms,"generate_ms":generate_ms,"cost":cost,"hash":chunk.hash().unwrap(),"triangles":chunk.triangles.len()}));
 }
}
