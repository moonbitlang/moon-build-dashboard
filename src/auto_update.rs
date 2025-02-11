use std::collections::HashSet;

use crate::mooncakesio::get_all_mooncakes;

#[test]
fn gen_latest_list_with_version() {
    let repos = std::fs::read_to_string("repos.txt").unwrap();
    let parts: Vec<&str> = repos
        .splitn(2, "# generated list to test mooncakes on mooncakes.io")
        .collect();

    let exclude = std::fs::read_to_string("exclude.txt").unwrap();
    let exclude: HashSet<String> = exclude.lines().map(|s| s.to_string()).collect();

    let mut mooncakesio = String::new();
    let db = get_all_mooncakes().unwrap();
    for (name, versions) in db.db {
        if exclude.contains(&name.replace("\\", "/")) {
            continue;
        }
        let latest_version = versions.last().unwrap();
        mooncakesio.push_str(&format!("{} {}\n", name, latest_version));
    }

    let updated = format!(
        "{}# generated list to test mooncakes on mooncakes.io\n{}",
        parts[0], mooncakesio
    );
    std::fs::write("repos.txt", updated).unwrap();
}
