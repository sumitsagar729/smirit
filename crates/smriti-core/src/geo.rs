//! Offline geocoding from GeoNames: GPS -> "Ooty, Tamil Nadu, India", and
//! place names spotted in folder names ("ooty - nov 2021").
//!
//! Data files (in the app's `geo` resources, or `SMRITI_GEO`):
//! `cities15000.txt`, `admin1CodesASCII.txt`, `countryInfo.txt`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub const CITIES_FILE: &str = "cities15000.txt";
pub const ADMIN1_FILE: &str = "admin1CodesASCII.txt";
pub const COUNTRIES_FILE: &str = "countryInfo.txt";

/// Beyond this, a photo isn't "in" any town we know.
const MAX_KM: f64 = 60.0;
/// A much bigger town this much further away still wins ("Business Bay" -> "Dubai").
const BIG_TOWN_SLACK_KM: f64 = 15.0;
const BIG_TOWN_FACTOR: u32 = 3;
/// Suburbs take the name of a million-plus city this close in the same
/// country ("Lal Bahadur Nagar" -> "Hyderabad", "Deira" -> "Dubai").
const METRO_KM: f64 = 25.0;
const METRO_POPULATION: u32 = 1_000_000;
/// Folder-name matches: only towns at least this big, to avoid surprises.
const FOLDER_MIN_POPULATION: u32 = 20_000;

/// Names people actually use for some towns.
const FRIENDLY: &[(&str, &str)] = &[
    ("Udhagamandalam", "Ooty"),
    ("Madikeri", "Coorg"),
    ("Chikmagalur", "Chikmagaluru"),
    ("Mysore", "Mysuru"),
    ("Bangalore", "Bengaluru"),
    ("Panjim", "Goa"),
    ("Calangute", "Goa"),
    ("Panaji", "Goa"),
    ("Mapuca", "Goa"),
    ("Margao", "Goa"),
    ("Candolim", "Goa"),
];
/// Folder spellings -> GeoNames names.
const FOLDER_ALIASES: &[(&str, &str)] = &[
    ("chikmangluru", "Chikmagalur"),
    ("chikmagaluru", "Chikmagalur"),
    ("chikmagalur", "Chikmagalur"),
    ("coorg", "Madikeri"),
    ("mysuru", "Mysore"),
    ("bengaluru", "Bangalore"),
    ("bangalore", "Bangalore"),
    ("pondicherry", "Puducherry"),
];

#[derive(Debug, Clone)]
pub struct City {
    pub name: String,
    /// The name without accents, for matching typed text.
    pub ascii: String,
    pub lat: f64,
    pub lon: f64,
    pub population: u32,
    pub country_code: String,
    pub admin1_code: String,
}

/// A resolved place, ready to store.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub name: String,
    pub admin1: Option<String>,
    pub country: Option<String>,
    pub country_code: String,
    pub lat: f64,
    pub lon: f64,
}

pub struct Geocoder {
    cities: Vec<City>,
    /// 1-degree cells -> city indexes.
    grid: HashMap<(i32, i32), Vec<u32>>,
    /// Lower-case name -> city indexes (for folder names).
    names: HashMap<String, Vec<u32>>,
    admin1: HashMap<String, String>,
    countries: HashMap<String, String>,
}

/// The geodata folder: `SMRITI_GEO`, else the first of `candidates` holding it.
pub fn find_data_dir(candidates: &[PathBuf]) -> Option<PathBuf> {
    std::env::var_os("SMRITI_GEO")
        .map(PathBuf::from)
        .into_iter()
        .chain(candidates.iter().cloned())
        .find(|d| d.join(CITIES_FILE).is_file())
}

pub fn km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1, la2, lo2) = (a.0.to_radians(), a.1.to_radians(), b.0.to_radians(), b.1.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2) + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    6371.0 * 2.0 * h.sqrt().min(1.0).asin()
}

fn cell(lat: f64, lon: f64) -> (i32, i32) {
    (lat.floor() as i32, lon.floor() as i32)
}

impl Geocoder {
    pub fn load(dir: &Path) -> std::io::Result<Self> {
        let lines = |f: &str| -> std::io::Result<Vec<String>> {
            BufReader::new(std::fs::File::open(dir.join(f))?).lines().collect()
        };
        let mut cities = Vec::new();
        for line in lines(CITIES_FILE)? {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() < 15 {
                continue;
            }
            let (Ok(lat), Ok(lon)) = (c[4].parse(), c[5].parse()) else { continue };
            cities.push(City {
                name: c[1].to_string(),
                ascii: c[2].to_string(),
                lat,
                lon,
                population: c[14].parse().unwrap_or(0),
                country_code: c[8].to_string(),
                admin1_code: c[10].to_string(),
            });
        }
        let mut admin1 = HashMap::new();
        for line in lines(ADMIN1_FILE).unwrap_or_default() {
            let mut c = line.split('\t');
            if let (Some(code), Some(name)) = (c.next(), c.next()) {
                admin1.insert(code.to_string(), name.to_string());
            }
        }
        let mut countries = HashMap::new();
        for line in lines(COUNTRIES_FILE).unwrap_or_default() {
            if line.starts_with('#') {
                continue;
            }
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() > 4 {
                countries.insert(c[0].to_string(), c[4].to_string());
            }
        }
        Ok(Self::build(cities, admin1, countries))
    }

    fn build(cities: Vec<City>, admin1: HashMap<String, String>, countries: HashMap<String, String>) -> Self {
        let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        let mut names: HashMap<String, Vec<u32>> = HashMap::new();
        for (i, c) in cities.iter().enumerate() {
            grid.entry(cell(c.lat, c.lon)).or_default().push(i as u32);
            if c.population >= FOLDER_MIN_POPULATION {
                names.entry(c.name.to_lowercase()).or_default().push(i as u32);
                if c.ascii != c.name {
                    names.entry(c.ascii.to_lowercase()).or_default().push(i as u32);
                }
            }
        }
        Self { cities, grid, names, admin1, countries }
    }

    pub fn len(&self) -> usize {
        self.cities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cities.is_empty()
    }

    /// The town a photo was taken in, preferring a well-known bigger town
    /// that is nearly as close.
    pub fn reverse(&self, lat: f64, lon: f64) -> Option<Place> {
        let (cy, cx) = cell(lat, lon);
        let mut near: Vec<(f64, u32)> = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                for &i in self.grid.get(&(cy + dy, cx + dx)).into_iter().flatten() {
                    let c = &self.cities[i as usize];
                    near.push((km((lat, lon), (c.lat, c.lon)), i));
                }
            }
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let &(d0, mut best) = near.first()?;
        if d0 > MAX_KM {
            return None;
        }
        for &(d, i) in near.iter().take(15) {
            if d < d0 + BIG_TOWN_SLACK_KM
                && self.cities[i as usize].population > self.cities[best as usize].population * BIG_TOWN_FACTOR
            {
                best = i;
            }
        }
        let b = &self.cities[best as usize];
        if let Some(&(_, metro)) = near
            .iter()
            .filter(|(d, i)| {
                let c = &self.cities[*i as usize];
                *d <= METRO_KM && c.population >= METRO_POPULATION && c.country_code == b.country_code
            })
            .max_by_key(|(_, i)| self.cities[*i as usize].population)
        {
            best = metro;
        }
        Some(self.place(best as usize))
    }

    /// A town named in free text such as a folder name, within `country_code`
    /// (the family's home country) to avoid accidental matches abroad.
    pub fn find_in_text(&self, text: &str, country_code: &str) -> Option<Place> {
        let words: Vec<String> =
            text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect();
        // Longest phrases first ("new delhi" before "delhi").
        for n in (1..=3).rev() {
            for win in words.windows(n) {
                let phrase = win.join(" ");
                if phrase.len() < 4 || phrase.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let key = FOLDER_ALIASES
                    .iter()
                    .find(|(a, _)| *a == phrase)
                    .map_or(phrase.clone(), |(_, name)| name.to_lowercase());
                let best = self
                    .names
                    .get(&key)
                    .into_iter()
                    .flatten()
                    .filter(|&&i| self.cities[i as usize].country_code == country_code)
                    .max_by_key(|&&i| self.cities[i as usize].population);
                if let Some(&i) = best {
                    return Some(self.place(i as usize));
                }
            }
        }
        None
    }

    fn place(&self, i: usize) -> Place {
        let c = &self.cities[i];
        let admin1 = self.admin1.get(&format!("{}.{}", c.country_code, c.admin1_code)).cloned();
        let name = FRIENDLY
            .iter()
            .find(|(raw, _)| *raw == c.name || *raw == c.ascii)
            .map_or_else(|| c.name.clone(), |(_, f)| f.to_string());
        Place {
            name,
            admin1,
            country: self.countries.get(&c.country_code).cloned(),
            country_code: c.country_code.clone(),
            lat: c.lat,
            lon: c.lon,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A tiny world: enough towns to test the rules without the real data.
    pub(crate) fn sample() -> Geocoder {
        let city = |name: &str, lat, lon, population, cc: &str, a1: &str| City {
            name: name.into(),
            ascii: name.into(),
            lat,
            lon,
            population,
            country_code: cc.into(),
            admin1_code: a1.into(),
        };
        let cities = vec![
            city("Bengaluru", 12.97194, 77.59369, 8_443_675, "IN", "19"),
            city("Ooty", 11.4134, 76.69521, 233_426, "IN", "25"),
            city("Business Bay", 25.18544, 55.26969, 191_000, "AE", "03"),
            city("Dubai", 25.07725, 55.30927, 3_478_300, "AE", "03"),
            city("Kolar", 13.13768, 78.12999, 138_553, "IN", "19"),
            city("Lal Bahadur Nagar", 17.34769, 78.55757, 609_000, "IN", "40"),
            city("Hyderabad", 17.38405, 78.45636, 6_809_970, "IN", "40"),
            city("Vista", 33.20004, -117.24254, 101_659, "US", "CA"),
        ];
        let admin1 = [("IN.19", "Karnataka"), ("IN.25", "Tamil Nadu"), ("AE.03", "Dubai"), ("US.CA", "California")];
        let countries = [("IN", "India"), ("AE", "United Arab Emirates"), ("US", "United States")];
        Geocoder::build(
            cities,
            admin1.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
            countries.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        )
    }

    #[test]
    fn reverse_geocodes_with_friendly_names() {
        let g = sample();
        let p = g.reverse(12.95, 77.6).unwrap();
        assert_eq!(
            (p.name.as_str(), p.admin1.as_deref(), p.country.as_deref()),
            ("Bengaluru", Some("Karnataka"), Some("India"))
        );
        assert_eq!(g.reverse(11.40, 76.70).unwrap().name, "Ooty");
        // Business Bay is closer, but everyone calls it Dubai.
        assert_eq!(g.reverse(25.186, 55.27).unwrap().name, "Dubai");
        // Suburbs of big cities take the city's name.
        assert_eq!(g.reverse(17.34, 78.56).unwrap().name, "Hyderabad");
        // Middle of the ocean.
        assert!(g.reverse(-30.0, 80.0).is_none());
    }

    #[test]
    fn finds_towns_in_folder_names_at_home_only() {
        let g = sample();
        assert_eq!(g.find_in_text("hoskote kolar - 19 aug 2021", "IN").unwrap().name, "Kolar");
        assert_eq!(g.find_in_text("ooty - nov 2021", "IN").unwrap().admin1.as_deref(), Some("Tamil Nadu"));
        assert!(g.find_in_text("for vista album", "IN").is_none()); // Vista, California is not a family trip
        assert!(g.find_in_text("diwali - Nov 2021", "IN").is_none());
    }

    #[test]
    fn distance_is_sane() {
        let d = km((12.97, 77.59), (11.41, 76.70));
        assert!((190.0..215.0).contains(&d), "{d}");
    }
}
