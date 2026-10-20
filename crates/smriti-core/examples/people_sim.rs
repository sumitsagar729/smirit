//! Simulates a user naming groups and setting birth dates, then runs recognition:
//! cargo run --release -p smriti-core --example people_sim -- <library-dir> [<group-id>=<name>] [<name>@<YYYY-MM-DD>] ...

use smriti_core::{Id, Library, PersonRef, PersonUpdate};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let lib = Library::open(args.next().expect("library dir"))?;
    for a in args {
        if let Some((group, name)) = a.split_once('=') {
            let id = lib.name_group(Id(group.parse()?), &PersonRef { person_id: None, name: Some(name.into()) })?;
            println!("named group {group} as {name} (person {id})");
        } else if let Some((name, birth)) = a.split_once('@') {
            let p = lib.people()?.into_iter().find(|p| p.name == name).expect("named person");
            lib.update_person(p.id, &PersonUpdate { name: p.name, birth_date: Some(birth.into()), is_family: true })?;
            println!("{name} born {birth}");
        }
    }
    println!("{:?}", lib.recognize()?);
    for p in lib.people()? {
        println!("{:10} faces {:6} photos {:6}", p.name, p.face_count, p.photo_count);
    }
    println!("groups left (>=3 faces): {}", lib.face_groups(10_000)?.len());
    Ok(())
}
