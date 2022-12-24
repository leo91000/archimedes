use std::{collections::HashMap, env, fs, path::PathBuf};

use clap::ValueEnum;
use derive_more::Display;
use serde::Deserialize;

#[derive(ValueEnum, Clone, Display)]
pub enum ReleaseType {
    Fix,
    Minor,
    Major,
    Auto,
}

pub fn release_command(release_type: ReleaseType) {
    println!("Release {release_type}");
    let packages = parse_packages();
    dbg!(packages);

    // Input 1 : Update type (fix, minor, major, auto)
    // ====================================
    // 1. Dependency graph of workspaces
    // ====================================
    // 2. From deeper workspace to higher one :
    // - Get all commit in workspace from latest tag {lib_name}@{version}
    // - If no commit found, continue;
    // - If auto mode, figure out if fix, minor or major with conventional commits
    // - Bump version in Cargo.toml
    // - Generate CHANGELOG.md from changes
    // - Add to list of commit tag {lib_name}@{version}
    // - Add package to list of packages to publish (FIFO)
    // ====================================
    // 3. Commit all changes and tag with all bumped tags
    // ====================================
    // 4. cd into all packages to release (FIFO) & cargo publish
}

structstruck::strike! {
    #[strikethrough[derive(Deserialize, Debug)]]
    pub struct CargoToml {
        package: struct {
            name: String,
            version: String,
        },
        dependencies: HashMap<
            String,
            #[serde(untagged)]
            enum {
                DependencyDetail(struct { path: Option<String>, version: String }),
                Version(String),
            }
        >
    }
}

#[derive(Deserialize, Debug)]
struct PackageDetail {
    name: String,
    version: String,
    path: PathBuf,
    nest_level: u8,
}

fn parse_packages() -> Vec<PackageDetail> {
    let dir = env::current_dir().unwrap();
    let mut packages = parse_packages_recur(dir, 0).unwrap();

    // We dedup package by name and we keep highest nest level
    packages.sort_unstable_by(|a, b| {
        let sort_a = (a.name.clone(), a.nest_level);
        let sort_b = (b.name.clone(), b.nest_level);
        sort_b.cmp(&sort_a)
    });
    packages.dedup_by_key(|p| p.name.clone());
    // We sort by nest level
    packages.sort_unstable_by_key(|p| p.nest_level);

    packages
}

fn parse_packages_recur(path: PathBuf, nest_level: u8) -> anyhow::Result<Vec<PackageDetail>> {
    if nest_level >= 10 {
        panic!("Nest level of dependencies reached 10. There is probably a cyclic depdency.");
    }

    let file = fs::read_to_string(path.join("Cargo.toml"))?;
    let cargo_toml: CargoToml = toml::from_str(&file)?;

    let mut packages = vec![PackageDetail {
        name: cargo_toml.package.name,
        version: cargo_toml.package.version,
        path: path.clone(),
        nest_level,
    }];

    for (_k, dependency) in cargo_toml.dependencies {
        match dependency {
            Dependencies::DependencyDetail(DependencyDetail {
                path: Some(dependency_path),
                version: _,
            }) => {
                let dependency_path = path.join(dependency_path);
                let mut dependency_package = parse_packages_recur(dependency_path, nest_level + 1)?;
                packages.append(&mut dependency_package);
            }
            _ => {}
        }
    }

    Ok(packages)
}
