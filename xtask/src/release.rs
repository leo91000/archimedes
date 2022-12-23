use clap::ValueEnum;
use derive_more::Display;

#[derive(ValueEnum, Clone, Display)]
pub enum ReleaseType {
    Fix,
    Minor,
    Major,
    Auto,
}

pub fn release_command(release_type: ReleaseType) {
    println!("Release {release_type}");
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
