mod dependency;
mod version;

pub use dependency::{
  DepParseResult, Dependency, GradleEntry, GradleList, LineParseResult, ParsedDependency, ProcessedDependency,
  ProcessedDependencyObject, TempList, Versions,
};
pub use version::Version;
