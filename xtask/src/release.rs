use std::{
    collections::HashMap,
    env, fs,
    path::PathBuf,
    process::{Command, Stdio},
    str::FromStr,
};

use clap::ValueEnum;
use derive_more::Display;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use structstruck::strike;

use crate::utils::StringUtils;

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
    let commits = parse_commits(env::current_dir().unwrap(), None).unwrap();
    dbg!(commits);

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

strike! {
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
                DependencyDetail(struct { path: Option<String>, #[allow(dead_code)] version: String }),
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
        let sort_a = (&a.name, a.nest_level);
        let sort_b = (&b.name, b.nest_level);
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

strike! {
    #[strikethrough[derive(Debug)]]
    struct Commit {
        message: String,
        description: String,
        short_hash: String,
        change_type: enum CommitType {
            Fix,
            Feat,
            Chore,
            Other(String),
        },
        breaking_change: bool,
        scope: Option<String>,
        references: Vec<struct CommitReference {
            ref_type: enum CommitReferenceType {
                PullRequest,
                Issue,
                Hash,
            },
            value: String,
        }>,
        authors: Vec<struct CommitAuthor {
            name: String,
            email: String,
        }>
    }
}

impl FromStr for CommitType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let r = match s {
            "fix" => Self::Fix,
            "feat" => Self::Feat,
            "chore" => Self::Chore,
            _ => Self::Other(s.to_string()),
        };

        Ok(r)
    }
}

// https://www.conventionalcommits.org/en/v1.0.0/
// https://regex101.com/r/FSfNvA/1
const CONVENTIONAL_COMMIT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new("(?P<type>[a-z]+)(\\((?P<scope>.+)\\))?(?P<breaking>!)?: (?P<description>.+)")
        .unwrap()
});
const CO_AUTHORED_BY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("Co-authored-by:\\s*(?P<name>.+)(<(?P<email>.+)>)").unwrap());
const PR_RE: Lazy<Regex> = Lazy::new(|| Regex::new("\\([a-z ]*(#[0-9]+)\\s*\\)").unwrap());
const ISSUE_RE: Lazy<Regex> = Lazy::new(|| Regex::new("(#[0-9]+)").unwrap());

fn parse_commits(dir: PathBuf, from: Option<String>) -> anyhow::Result<Vec<Commit>> {
    let mut cmd = Command::new("git");
    cmd.arg("--no-pager").arg("log");

    let to = "HEAD";
    if let Some(from) = from {
        cmd.arg(format!("{from}...{to}"));
    } else {
        cmd.arg(format!("{to}"));
    }

    let output = cmd
        .arg(r#"--pretty="----%n%s|%h|%an|%ae%n%b""#)
        .arg("--name-status")
        .arg("--")
        .arg(dir.to_str().unwrap())
        .stdout(Stdio::piped())
        .output()?;

    let stdout: String = String::from_utf8(output.stdout)?;

    let commits = stdout
        .split("----\n")
        .skip(1)
        .filter_map(|raw_commit| {
            let (commit_data, body) = raw_commit.split_once('\n').expect("Bad git >-<");
            let (message, short_hash, author_name, author_email) =
                commit_data.split_4("|").expect("Bad git fmt >-<");

            let cc_cap = (*CONVENTIONAL_COMMIT_RE).captures_iter(message).next()?;
            let change_type: CommitType = cc_cap.name("type")?.as_str().parse().ok()?;
            let scope = cc_cap.name("scope").map(|m| m.as_str().to_string());
            let breaking_change = cc_cap.name("breaking").is_some();
            let description = cc_cap.name("description")?.as_str();

            let mut references: Vec<CommitReference> = (*PR_RE)
                .captures_iter(description)
                .filter_map(|cap| {
                    Some(CommitReference {
                        ref_type: CommitReferenceType::PullRequest,
                        value: cap.get(1)?.as_str().into(),
                    })
                })
                .chain((*ISSUE_RE).captures_iter(description).filter_map(|cap| {
                    Some(CommitReference {
                        ref_type: CommitReferenceType::PullRequest,
                        value: cap.get(1)?.as_str().into(),
                    })
                }))
                .collect();

            references.push(CommitReference {
                ref_type: CommitReferenceType::Hash,
                value: short_hash.to_string(),
            });
            references.sort_by_key(|c| c.value.clone());
            references.dedup_by_key(|c| c.value.clone());

            let mut authors: Vec<CommitAuthor> = (*CO_AUTHORED_BY_RE)
                .captures_iter(body)
                .filter_map(|c| {
                    Some(CommitAuthor {
                        name: c.name("name")?.as_str().into(),
                        email: c.name("email")?.as_str().into(),
                    })
                })
                .collect();

            authors.insert(
                0,
                CommitAuthor {
                    name: author_name.to_string(),
                    email: author_email.to_string(),
                },
            );

            let description = (*PR_RE).replace_all(description, "");

            Some(Commit {
                message: message.into(),
                description: description.into(),
                short_hash: short_hash.into(),
                change_type,
                breaking_change,
                scope,
                references,
                authors,
            })
        })
        .collect::<Vec<_>>();

    Ok(commits)
}
