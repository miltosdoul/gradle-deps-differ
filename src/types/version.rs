use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, PartialEq, Clone, Deserialize)]
#[serde(untagged)]
pub enum Version {
  Transitive(String),
  Pinned(String),
  NotApplicable,
}

impl Version {
  pub fn is_applicable(&self) -> bool {
    *self != Version::NotApplicable
  }
}

impl fmt::Display for Version {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Version::Transitive(val) | Version::Pinned(val) => write!(f, "{}", val),
      Version::NotApplicable => write!(f, "N/A"),
    }
  }
}

impl Serialize for Version {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    match self {
      Version::Transitive(val) | Version::Pinned(val) => serializer.serialize_str(val),
      Version::NotApplicable => serializer.serialize_str("N/A"),
    }
  }
}
