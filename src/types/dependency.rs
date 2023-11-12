use crate::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
  pub name: String,
  pub namespace: String,
  pub gradle_entries: Vec<GradleEntry>,
}

#[derive(Debug)]
pub struct ParsedDependency {
  pub name: String,
  pub namespace: String,
  pub transitive: Version,
  pub pinned: Version,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedDependencyObject {
  pub dependency: ProcessedDependency,
  pub changed: bool,
  pub gradle_versions: Vec<GradleList>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedDependency {
  pub name: String,
  pub namespace: String,
  pub gradle_entries_before: Vec<GradleEntry>,
  pub gradle_entries_after: Vec<GradleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradleEntry {
  pub gradle_config_name: String,
  pub versions: Versions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Versions {
  pub transitive: Vec<Version>,
  pub pinned: Version,
}

impl Versions {
  pub fn transitive_contains(&self, val: &Version) -> bool {
    return self.transitive.contains(val);
  }
}

#[derive(Debug)]
pub enum LineParseResult {
  Parsed,
  Skip,
  End,
}

#[derive(Debug)]
pub enum DepParseResult {
  Dep(ParsedDependency),
  NoDependencies,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempList {
  pub gradle_config_name: String,
  pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradleList {
  pub gradle_config_name: String,
  pub version_before: String,
  pub version_after: String,
}
