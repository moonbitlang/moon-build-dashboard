use std::{collections::HashSet, path::Path};

use crate::{
    mooncakesio::get_all_mooncakes,
    util::{get_repos_config, Mooncake, ReposConfig},
};

#[test]
fn update_mooncakes_list() {
    let mut repos: ReposConfig = get_repos_config(Path::new("repos.yml"));

    let exclude = std::fs::read_to_string("exclude.txt").unwrap();
    let exclude: HashSet<String> = exclude.lines().map(|s| s.to_string()).collect();

    let db = get_all_mooncakes().unwrap();

    // 创建一个集合，存储 ReposConfig 中已有的 mooncake 名称
    let mut existing_mooncakes = HashSet::new();

    let mut updated_mooncakes = Vec::new();

    // 更新 mooncakes 中的版本信息
    for mooncake in &mut repos.mooncakes {
        let name = &mooncake.name;
        existing_mooncakes.insert(name.clone());

        // 跳过被排除的 mooncake
        if exclude.contains(&name.replace("\\", "/")) {
            continue;
        }

        // 检查是否在数据库中有对应的 mooncake
        if let Some(versions) = db.db.get(name) {
            if let Some(latest_version) = versions.last() {
                // 更新版本信息
                mooncake.version = latest_version.clone();
                updated_mooncakes.push(mooncake.clone());
            }
        }
    }

    // 添加数据库中有但 ReposConfig 中没有的 mooncake
    for (name, versions) in &db.db {
        // 跳过已经存在于 ReposConfig 中的 mooncake
        if existing_mooncakes.contains(name) {
            continue;
        }

        // 跳过被排除的 mooncake
        if exclude.contains(&name.replace("\\", "/")) {
            continue;
        }

        if let Some(latest_version) = versions.last() {
            // 创建新的 Mooncake 对象
            let new_mooncake = Mooncake {
                name: name.clone(),
                version: latest_version.clone(),
                running_os: None,
                running_backend: None,
            };

            // 添加到 ReposConfig 中
            updated_mooncakes.push(new_mooncake);
        }
    }

    repos.mooncakes = updated_mooncakes;
    // 将更新后的配置写回文件
    let updated_content = serde_yaml::to_string(&repos).unwrap();
    std::fs::write("repos.yml", updated_content).unwrap();
}
