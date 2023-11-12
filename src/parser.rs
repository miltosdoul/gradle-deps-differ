use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use version_compare::{compare_to, Cmp};

use crate::types::*;

const ARROW: &str = "->";
const ROOT_PROJECT_IDENTIFIER: &str = "Root project";
const VALID_DEP_LINE_START_CHARS: [char; 4] = [
  '\\', /* \--- com.h2database:h2 (n) */
  '+',  /* +--- org.openapitools:openapi-generator-gradle-plugin:7.0.1 */
  '|',  /* |    +--- org.apache.commons:commons-text:1.10.0 */
  'N',  /* No dependencies */
];

pub struct DependencyParser {
  pub dep_maps: [Box<FxHashMap<String, Dependency>>; 2],
  pub active_map: usize,
  skip: usize,
  in_task: bool,
  found_root: bool,
  curr_gradle_task: String,
}

impl DependencyParser {
  pub fn new() -> Self {
    Self {
      dep_maps: [Box::new(FxHashMap::default()), Box::new(FxHashMap::default())],
      active_map: 0,
      skip: 0,
      in_task: false,
      found_root: false,
      curr_gradle_task: String::new(),
    }
  }

  pub fn parse_file(&mut self, file: BufReader<fs::File>) {
    for line in file.lines().map(|l| l.unwrap()) {
      match self.parse_line(line) {
        LineParseResult::Parsed | LineParseResult::Skip => (),
        LineParseResult::End => break,
      }
    }

    self.active_map += 1;
  }

  fn parse_line(&mut self, line: String) -> LineParseResult {
    /* first check if skipping */
    if self.skip > 0 {
      self.skip -= 1;
      /* always skipping to next gradle task */
      if self.skip == 0 {
        self.in_task = true;
      }
      return LineParseResult::Skip;
    }

    /* skip non-root lines and empty lines */
    if (!self.found_root && !line.starts_with(ROOT_PROJECT_IDENTIFIER)) || line.is_empty() {
      return LineParseResult::Skip;
    }

    if line.starts_with(ROOT_PROJECT_IDENTIFIER) {
      self.found_root = true;
      self.skip = 2;
      return LineParseResult::Skip;
    }

    /* declares end of parsing when encountering lines like
    (c) - dependency constraint */
    if line.starts_with('(') {
      return LineParseResult::End;
    }

    if self.in_task {
      if VALID_DEP_LINE_START_CHARS.contains(&line.trim().chars().next().unwrap()) {
        let dep_res = self.parse_dep_line(line);

        match dep_res {
          DepParseResult::Dep(dep_opt) => {
            self.add_or_update_dep(dep_opt);
            return LineParseResult::Parsed;
          }
          DepParseResult::NoDependencies => {
            self.skip = 1;
            return LineParseResult::Skip;
          }
        }
      } else {
        /* If in task block but line doesn't start with any of "\\, +, |, N",
        line is the gradle task name. */
        self.curr_gradle_task = match line.find(' ') {
          Some(idx) => line[..idx].to_string(),
          None => line,
        };
      }

      return LineParseResult::Skip;
    }
    return LineParseResult::Skip;
  }

  fn parse_dep_line(&self, line: String) -> DepParseResult {
    if line == "No dependencies" {
      return DepParseResult::NoDependencies;
    }

    let (arrow_idx, has_arrow) = match line.find(ARROW) {
      Some(idx) => (idx, true),
      None => (0, false),
    };

    let (specifier_idx, has_specifier) = match line.find('(') {
      Some(idx) => (idx, true),
      None => (0, false),
    };

    /* get dependency name start and end indexes */
    let name_start: usize = line
      .find(':')
      .expect("No colon character in line - Invalid dependency line");

    let offset = name_start + 1;

    let (name_end, is_single_colon) = match line[(offset)..].find(':') {
      Some(idx) => (idx + offset, false),
      None => (line[(offset)..].find(' ').unwrap() + offset, true),
    };

    let ver_transitive: Version;
    let ver_pinned: Version;

    /* parse versions */
    match is_single_colon {
      true => {
        match has_arrow {
          true => {
            if has_specifier {
              ver_pinned = Version::Pinned(line[(arrow_idx + 3)..(specifier_idx - 1)].to_string());
            } else {
              ver_pinned = Version::Pinned(line[(arrow_idx + 3)..].to_string());
            }
          }
          false => {
            ver_pinned = Version::NotApplicable;
          }
        }

        ver_transitive = Version::NotApplicable;
      }

      false => match has_arrow {
        true => {
          ver_transitive = Version::Transitive(line[(name_end + 1)..(arrow_idx - 1)].to_string());

          if has_specifier {
            ver_pinned = Version::Pinned(line[(arrow_idx + 3)..(specifier_idx - 1)].to_string());
          } else {
            ver_pinned = Version::Pinned(line[(arrow_idx + 3)..].to_string());
          }
        }
        false => {
          ver_pinned = Version::NotApplicable;

          if has_specifier {
            ver_transitive = Version::Transitive(line[(name_end + 1)..(specifier_idx - 1)].to_string());
          } else {
            ver_transitive = Version::Transitive(line[(name_end + 1)..].to_string());
          }
        }
      },
    }

    /* parse namespace (find first alphabetic char) */
    let namespace_start = line.chars().position(|c| c.is_alphabetic()).unwrap();

    let namespace = line[namespace_start..name_start].to_string();

    return DepParseResult::Dep(ParsedDependency {
      name: line[(name_start + 1)..name_end].to_string(),
      namespace: namespace,
      transitive: ver_transitive,
      pinned: ver_pinned,
    });
  }

  fn add_or_update_dep(&mut self, dependency: ParsedDependency) {
    if self.dep_maps[self.active_map].contains_key(&dependency.name) {
      self.update_dep(dependency);
    } else {
      self.add_dep(dependency);
    }
  }

  fn add_dep(&mut self, dependency: ParsedDependency) {
    let dep_entry = Dependency {
      name: dependency.name.clone(),
      namespace: dependency.namespace.clone(),
      gradle_entries: vec![GradleEntry {
        gradle_config_name: self.curr_gradle_task.clone(),
        versions: Versions {
          transitive: vec![dependency.transitive],
          pinned: dependency.pinned,
        },
      }],
    };

    self.dep_maps[self.active_map].insert(dependency.name.clone(), dep_entry);
  }

  fn update_dep(&mut self, dependency: ParsedDependency) {
    let mut existing = self.dep_maps[self.active_map]
      .get(&dependency.name)
      .unwrap()
      .clone();

    let gradle_config_idx = existing
      .gradle_entries
      .iter()
      .position(|e| e.gradle_config_name == self.curr_gradle_task);

    match gradle_config_idx {
      Some(idx) => {
        self.update_existing(&mut existing.gradle_entries[idx], &dependency);
      }

      None => {
        let ver_entry = GradleEntry {
          gradle_config_name: self.curr_gradle_task.clone(),
          versions: Versions {
            transitive: vec![dependency.transitive],
            pinned: dependency.pinned,
          },
        };

        existing.gradle_entries.push(ver_entry);
      }
    }

    self.dep_maps[self.active_map].insert(dependency.name, existing);
  }

  fn update_existing(&self, existing: &mut GradleEntry, new: &ParsedDependency) {
    /* update pinned only if old is "N/A" or if new is greater */
    if new.pinned.is_applicable() {
      if !existing.versions.pinned.is_applicable() {
        existing.versions.pinned = new.pinned.clone();
      } else {
        let is_greater = compare_to(new.pinned.to_string(), existing.versions.pinned.to_string(), Cmp::Gt).unwrap();

        if is_greater {
          existing.versions.pinned = new.pinned.clone();
        }
      }
    }

    /* if newly parsed transitive value isn't "N/A" and
    is not already in the array, add it */
    if new.transitive.is_applicable() {
      if !existing.versions.transitive_contains(&new.transitive) {
        existing
          .versions
          .transitive
          .push(new.transitive.clone());
      }
    }
  }

  /// Produces a list of 'ProcessedDependencyObject' structs
  /// that contains all the dependencies,
  /// and for the dependencies that exist in both maps,
  /// has the version before and the version after as fields.
  pub fn compare_versions(&self) -> Vec<ProcessedDependencyObject> {
    let mut processed: Vec<ProcessedDependencyObject> = Vec::new();
    let mut common: FxHashSet<String> = FxHashSet::default();

    self.dep_maps[0].iter().for_each(|(k, v)| {
      let value_after = self.dep_maps[1].get(k);

      if value_after.is_some() {
        /* Insert common elements in hashmap
        to be able to find unique of other hash map later */
        common.insert(k.clone());
      }

      let gradle_lists = create_gradle_lists(Option::Some(v), value_after);

      let changed = gradle_lists
        .iter()
        .any(|el| el.version_before != el.version_after);

      let entries_after = if value_after.is_some() {
        value_after.unwrap().gradle_entries.clone()
      } else {
        Vec::new()
      };

      processed.push(ProcessedDependencyObject {
        dependency: ProcessedDependency {
          name: v.name.clone(),
          namespace: v.namespace.clone(),
          gradle_entries_before: v.gradle_entries.clone(),
          gradle_entries_after: entries_after,
        },
        gradle_versions: gradle_lists,
        changed: changed,
      });
    });

    /* Add dependencies that are unique to second map */
    self.dep_maps[1]
      .iter()
      .filter(|(k, _)| !common.contains(&k as &String))
      .for_each(|(_, v)| {
        let gradle_lists = create_gradle_lists(Option::None, Option::Some(v));

        let changed = gradle_lists
          .iter()
          .any(|el| el.version_before != el.version_after);

        processed.push(ProcessedDependencyObject {
          dependency: ProcessedDependency {
            name: v.name.clone(),
            namespace: v.namespace.clone(),
            gradle_entries_before: Vec::new(),
            gradle_entries_after: v.gradle_entries.clone(),
          },
          gradle_versions: gradle_lists,
          changed: changed,
        });
      });

    return processed;
  }
}

/// Get the greatest version in an array of versions ([] or Vec),
/// as Gradle will pick the greatest version of a dependency to download.
fn get_greatest(arr: &[Version]) -> Option<String> {
  // first, check if it only contains N/A to return early.
  if !arr.iter().any(|ver| ver.is_applicable()) {
    return None;
  }

  arr
    .iter()
    .filter(|ver| ver.is_applicable())
    .map(|ver| ver.to_string())
    .reduce(|a, b| {
      let ver_a = version_compare::Version::from(&a).unwrap();
      let ver_b = version_compare::Version::from(&b).unwrap();

      match ver_a.compare(ver_b) {
        Cmp::Gt | Cmp::Eq => a,
        Cmp::Lt => b,
        _ => unreachable!(),
      }
    })
}

/// Creates Gradle task list with `version_before` and `version_after` for each
/// Gradle task of each dependency.
/// If both dependencies are provided, does join of gradle tasks and versions. e.g.:
/// * x tasks: `['compileClasspath', 'compileJava']`
/// * y tasks: `['annotationClasspath', 'compileJava']`
/// * join   : `['compileClasspath', 'compileJava', 'annotationClasspath']`. \
/// For the unique tasks, value of other is `"N/A"`. \
/// If only one is provided, makes a vector with `before` or `after` for the missing one
/// having the value `"N/A"`.
/// e.g.:
/// ```json
/// [
///  {
///   "gradle_config_name": "compileClasspath",
///   "version_before": "1.18.30",
///   "version_after": "1.18.30"
///  }
/// ],
/// ...
/// ```
fn create_gradle_lists(before: Option<&Dependency>, after: Option<&Dependency>) -> Vec<GradleList> {
  let mut res: Vec<GradleList> = Vec::new();

  if before.is_some() && after.is_some() {
    /* keep already encountered Gradle tasks here */
    let mut done: FxHashSet<String> = FxHashSet::default();

    let ver_before = get_versions(before.unwrap());
    let ver_after = get_versions(after.unwrap());

    /* cover tasks before */
    ver_before.iter().for_each(|t_b| {
      let mut found = false;

      for t_a in ver_after.iter() {
        if t_b.gradle_config_name == t_a.gradle_config_name {
          found = true;
          done.insert(t_b.gradle_config_name.clone());

          res.push(GradleList {
            gradle_config_name: t_b.gradle_config_name.clone(),
            version_before: t_b.version.clone(),
            version_after: t_a.version.clone(),
          });

          break;
        }
      }

      /* if not found, means it doesn't exist in other dep */
      if !found {
        res.push(GradleList {
          gradle_config_name: t_b.gradle_config_name.clone(),
          version_before: t_b.version.clone(),
          version_after: "N/A".to_string(),
        })
      }
    });

    /* cover tasks that are unique to after */
    ver_after
      .iter()
      .filter(|t| !done.contains(&t.gradle_config_name))
      .for_each(|t| {
        res.push(GradleList {
          gradle_config_name: t.gradle_config_name.clone(),
          version_before: "N/A".to_string(),
          version_after: t.version.clone(),
        })
      });
  } else if before.is_some() {
    let ver_before = get_versions(before.unwrap());

    ver_before.iter().for_each(|t| {
      res.push(GradleList {
        gradle_config_name: t.gradle_config_name.clone(),
        version_before: t.version.clone(),
        version_after: "N/A".to_string(),
      })
    });
  } else if after.is_some() {
    let ver_after = get_versions(after.unwrap());

    ver_after.iter().for_each(|t| {
      res.push(GradleList {
        gradle_config_name: t.gradle_config_name.clone(),
        version_before: "N/A".to_string(),
        version_after: t.version.clone(),
      })
    });
  }

  return res;
}

/// For each Gradle task of the dependency, checks transitive and pinned before,
/// and transitive and pinned after. \
/// * If it has no pinned, means transitive is valid. \
/// * If it has no transitive, means pinned is valid. \
/// * Else (if both transitive and pinned exist), pinned is going to be the
/// active version in the Gradle task.
fn get_versions(element: &Dependency) -> Vec<TempList> {
  let mut versions_for_each_config: Vec<TempList> = Vec::new();

  for entry in element.gradle_entries.iter() {
    let ver = match entry.versions.pinned.is_applicable() {
      /* If pinned isn't "N/A", then version == pinned */
      true => entry.versions.pinned.clone().to_string(),

      /* If pinned is "N/A", then version == max(transitive) */
      false => match get_greatest(&entry.versions.transitive) {
        Some(v) => v.to_string(),
        None => "N/A".to_string(),
      },
    };

    versions_for_each_config.push(TempList {
      gradle_config_name: entry.gradle_config_name.clone(),
      version: ver,
    });
  }

  return versions_for_each_config;
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_parses_dep_line_without_versions() {
    let parser = DependencyParser::new();

    let parsed = parser.parse_dep_line(String::from("io.github.openfeign:feign-core (n)"));

    if let DepParseResult::Dep(dep) = parsed {
      assert_eq!(dep.name, "feign-core");
      assert_eq!(dep.namespace, "io.github.openfeign");
      assert!(matches!(dep.transitive, Version::NotApplicable));
      assert!(matches!(dep.pinned, Version::NotApplicable));
    } else {
      panic!("Unable to destructure DepParseResult object");
    }
  }

  #[test]
  fn it_parses_dep_line_with_transitive_version() {
    let parser = DependencyParser::new();

    let parsed = parser.parse_dep_line(String::from("io.github.openfeign:feign-core:4.0.4"));

    if let DepParseResult::Dep(dep) = parsed {
      assert_eq!(dep.name, "feign-core");
      assert_eq!(dep.namespace, "io.github.openfeign");
      assert!(matches!(dep.transitive, Version::Transitive(_)));
      assert!(matches!(dep.pinned, Version::NotApplicable));

      if let Version::Transitive(ver) = dep.transitive {
        assert_eq!(ver, "4.0.4");
      } else {
        panic!("Transitive version parsed incorrectly");
      }
    } else {
      panic!("Unable to destructure DepParseResult object");
    }
  }

  #[test]
  fn it_parses_dep_line_with_pinned_version() {
    let parser = DependencyParser::new();
    let dep_line = "io.github.openfeign:feign-core -> 4.0.4".to_string();

    let parsed = parser.parse_dep_line(dep_line);

    if let DepParseResult::Dep(dep) = parsed {
      assert_eq!(dep.name, "feign-core");
      assert_eq!(dep.namespace, "io.github.openfeign");
      assert!(matches!(dep.transitive, Version::NotApplicable));
      assert!(matches!(dep.pinned, Version::Pinned(_)));

      if let Version::Pinned(ver) = dep.pinned {
        assert_eq!(ver, "4.0.4");
      } else {
        panic!("Pinned version parsed incorrectly");
      }
    } else {
      panic!("Unable to destructure DepParseResult object");
    }
  }

  #[test]
  fn it_parses_dep_line_with_transitive_and_pinned_version() {
    let parser = DependencyParser::new();

    let parsed = parser.parse_dep_line(String::from("io.github.openfeign:feign-core:4.0.3 -> 4.0.4"));

    if let DepParseResult::Dep(dep) = parsed {
      assert_eq!(dep.name, "feign-core");
      assert_eq!(dep.namespace, "io.github.openfeign");
      assert!(matches!(dep.transitive, Version::Transitive(_)));
      assert!(matches!(dep.pinned, Version::Pinned(_)));

      /* assert transitive */
      if let Version::Transitive(ver) = dep.transitive {
        assert_eq!(ver, "4.0.3");
      } else {
        panic!("Transitive version parsed incorrectly");
      }

      /* assert pinned */
      if let Version::Pinned(ver) = dep.pinned {
        assert_eq!(ver, "4.0.4");
      } else {
        panic!("Pinned version parsed incorrectly");
      }
    } else {
      panic!("Unable to destructure DepParseResult object");
    }
  }

  #[test]
  #[should_panic]
  fn it_panics_on_dep_line_with_no_colon() {
    let parser = DependencyParser::new();
    let _ = parser.parse_dep_line(String::from("io.github.openfeign -> 4.0.4"));
  }

  #[test]
  fn it_returns_largest_transitive_version() {
    let transitive_vers = ["1.2.3", "1.2.4", "1.3.0"].map(|v| Version::Transitive(v.to_string()));

    let largest = get_greatest(&transitive_vers).unwrap();
    assert_eq!(largest, "1.3.0");
  }

  #[test]
  fn it_returns_join_of_gradle_tasks_before_and_after() {
    let dep_before = Dependency {
      name: "dep".to_string(),
      namespace: "dep_ns".to_string(),
      gradle_entries: [
        GradleEntry {
          gradle_config_name: "compileClasspath".to_string(),
          versions: Versions {
            transitive: [Version::Transitive("1.2.3".to_string())].to_vec(),
            pinned: Version::Pinned("1.2.4".to_string()),
          },
        },
        GradleEntry {
          gradle_config_name: "runtimeClasspath".to_string(),
          versions: Versions {
            transitive: [Version::Transitive("1.2.3".to_string())].to_vec(),
            pinned: Version::Pinned("1.2.4".to_string()),
          },
        },
      ]
      .to_vec(),
    };

    let dep_after = Dependency {
      name: "dep".to_string(),
      namespace: "dep_ns".to_string(),
      gradle_entries: [
        GradleEntry {
          gradle_config_name: "compileClasspath".to_string(),
          versions: Versions {
            transitive: [
              Version::Transitive("1.2.2".to_string()),
              Version::Transitive("1.2.7".to_string()),
            ]
            .to_vec(),
            pinned: Version::NotApplicable,
          },
        },
        GradleEntry {
          gradle_config_name: "productionRuntimeClasspath".to_string(),
          versions: Versions {
            transitive: [Version::Transitive("1.2.6".to_string())].to_vec(),
            pinned: Version::Pinned("1.2.7".to_string()),
          },
        },
      ]
      .to_vec(),
    };

    let list = create_gradle_lists(Option::Some(&dep_before), Option::Some(&dep_after));

    println!("{}", serde_json::to_string_pretty(&list).unwrap());

    assert_eq!(list.len(), 3);

    let gradle_tasks_set: Vec<String> = list
      .iter()
      .map(|o| o.gradle_config_name.clone())
      .collect();

    assert!(gradle_tasks_set.contains(&"productionRuntimeClasspath".to_string()));
    assert!(gradle_tasks_set.contains(&"runtimeClasspath".to_string()));
    assert!(gradle_tasks_set.contains(&"compileClasspath".to_string()));

    // TODO: Assert versions more correctly as well.
  }

  #[test]
  fn it_returns_empty_after_when_only_providing_before() {
    let dep_before = Dependency {
      name: "dep".to_string(),
      namespace: "dep_ns".to_string(),
      gradle_entries: [
        GradleEntry {
          gradle_config_name: "compileClasspath".to_string(),
          versions: Versions {
            transitive: [Version::Transitive("1.2.3".to_string())].to_vec(),
            pinned: Version::Pinned("1.2.4".to_string()),
          },
        },
        GradleEntry {
          gradle_config_name: "runtimeClasspath".to_string(),
          versions: Versions {
            transitive: [Version::Transitive("1.2.3".to_string())].to_vec(),
            pinned: Version::Pinned("1.2.4".to_string()),
          },
        },
      ]
      .to_vec(),
    };

    let list = create_gradle_lists(Option::Some(&dep_before), Option::None);

    println!("{}", serde_json::to_string_pretty(&list).unwrap());

    assert_eq!(list.len(), 2);

    let gradle_tasks_set: Vec<String> = list
      .iter()
      .map(|o| o.gradle_config_name.clone())
      .collect();

    assert!(gradle_tasks_set.contains(&"runtimeClasspath".to_string()));
    assert!(gradle_tasks_set.contains(&"compileClasspath".to_string()));

    list
      .iter()
      .for_each(|t| assert!(t.version_after == "N/A"))
  }

  #[test]
  fn it_returns_empty_before_when_only_providing_after() {
    let dep_after = Dependency {
      name: "dep".to_string(),
      namespace: "dep_ns".to_string(),
      gradle_entries: [
        GradleEntry {
          gradle_config_name: "compileClasspath".to_string(),
          versions: Versions {
            transitive: [
              Version::Transitive("1.2.2".to_string()),
              Version::Transitive("1.2.7".to_string()),
            ]
            .to_vec(),
            pinned: Version::NotApplicable,
          },
        },
        GradleEntry {
          gradle_config_name: "productionRuntimeClasspath".to_string(),
          versions: Versions {
            transitive: [Version::Transitive("1.2.6".to_string())].to_vec(),
            pinned: Version::Pinned("1.2.7".to_string()),
          },
        },
      ]
      .to_vec(),
    };

    let list = create_gradle_lists(Option::None, Option::Some(&dep_after));

    println!("{}", serde_json::to_string_pretty(&list).unwrap());

    assert_eq!(list.len(), 2);

    let gradle_tasks_set: Vec<String> = list
      .iter()
      .map(|o| o.gradle_config_name.clone())
      .collect();

    assert!(gradle_tasks_set.contains(&"productionRuntimeClasspath".to_string()));
    assert!(gradle_tasks_set.contains(&"compileClasspath".to_string()));

    list
      .iter()
      .for_each(|t| assert!(t.version_before == "N/A"))
  }

  #[test]
  // TODO: Convert this to an integration test
  fn it_updates_existing_pinned_version_if_is_greater() {
    let mut parser = DependencyParser::new();
    parser.found_root = true;
    parser.in_task = true;
    parser.parse_line("compileClasspath".to_string());

    parser.parse_line(String::from("| io.github.openfeign:feign-core:4.0.3 -> 4.0.4"));
    parser.parse_line(String::from("| io.github.openfeign:feign-core:4.0.3 -> 4.0.5"));

    let dep = parser.dep_maps[0]
      .get("feign-core")
      .expect("Dependency should exist");

    let gradle_entry: &GradleEntry = &dep
      .gradle_entries
      .iter()
      .cloned()
      .filter(|e| e.gradle_config_name == "compileClasspath")
      .collect::<Vec<GradleEntry>>()[0];

    assert_eq!(gradle_entry.versions.pinned.to_string(), "4.0.5");
  }
}
