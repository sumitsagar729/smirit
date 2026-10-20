//! Computes places and trips for a library and prints them:
//! cargo run --release -p smriti-core --example places_run -- <library-dir> <geo-data-dir>

use smriti_core::Library;
use smriti_core::geo::Geocoder;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let lib = Library::open(&args[1])?;
    let t = std::time::Instant::now();
    let geo = Geocoder::load(std::path::Path::new(&args[2]))?;
    println!("geocoder: {} towns in {} ms", geo.len(), t.elapsed().as_millis());
    println!("{:?}", lib.update_places(&geo)?);
    println!("home: {:?}", lib.home()?);
    for p in lib.places()?.iter().take(15) {
        println!("  {:24} {:20} {:6}", p.name, p.admin1.clone().unwrap_or_default(), p.count);
    }
    for t in lib.trips()? {
        let stops: Vec<&str> = t.stops.iter().map(|s| s.name.as_str()).collect();
        println!(
            "{} → {}  {:30} {:5} items  {:5.0} km  [{}]",
            t.start,
            t.end,
            t.title,
            t.count,
            t.distance_km.unwrap_or(0.0),
            stops.join(" → ")
        );
    }
    Ok(())
}
