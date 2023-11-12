mod parser;
mod types;
use clap::Parser;
use handlebars::Handlebars;
use std::fs;
use std::io::ErrorKind;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use types::ProcessedDependencyObject;

use crate::types::Version;
use parser::DependencyParser;

#[derive(Parser)]
#[command(author, version, about = "Gradle Deps Differ - Diffs two Gradle dependencies files", long_about = None)]
struct Cli {
  /// Path to file listing Gradle dependencies before
  #[arg(short = 'b', long, value_name = "path-to-deps-before-file")]
  file_before: Option<PathBuf>,

  /// Path to file listing Gradle dependencies after
  #[arg(short = 'a', long, value_name = "path-to-deps-after-file")]
  file_after: Option<PathBuf>,

  /// Output JSON
  #[arg(short, long, action)]
  json: bool,
}

fn main() -> std::io::Result<()> {
  let template = include_bytes!("../report/report_template.hbs");
  let cli = Cli::parse();

  let file_before = match cli.file_before {
    Some(f) => f,
    None => panic!("File 1 not provided"),
  };

  let file_after = match cli.file_after {
    Some(f) => f,
    None => panic!("File 2 not provided"),
  };

  let mut parser = Box::new(DependencyParser::new());

  if validate_input_file(&file_before).is_err() {
    panic!("Provided gradle dependencies before file is invalid");
  }

  if validate_input_file(&file_after).is_err() {
    panic!("Provided gradle dependencies after file is invalid");
  }

  match read_file(&file_before) {
    Ok(file) => {
      parser.parse_file(file);
    }
    Err(e) => panic!("Error encountered while trying to open file: {}", e),
  };

  match read_file(file_after) {
    Ok(file) => {
      parser.parse_file(file);
    }
    Err(e) => panic!("Error encountered while trying to open file: {}", e),
  };

  if cli.json {
    println!("{}", serde_json::to_string_pretty(&parser.compare_versions()).unwrap());
  } else {
    let mut handlebars = Handlebars::new();
    generate_report(&parser, &mut handlebars, &std::str::from_utf8(template).unwrap());
  }

  Ok(())
}

fn validate_input_file<P>(filepath: P) -> std::io::Result<()>
where
  P: AsRef<Path>,
{
  let reader_res = read_file(filepath);
  let reader: BufReader<fs::File>;
  if reader_res.is_err() {
    return Err(reader_res.err().unwrap());
  } else {
    reader = reader_res.unwrap();
  }

  for line in reader
    .lines()
    .filter(|l| !l.as_ref().unwrap().is_empty())
    .take(10)
    .map(|l| l.unwrap())
  {
    if line.contains("> Task :dependencies") || line.contains("Root project") {
      return Ok(());
    }
  }

  Err(std::io::Error::new(ErrorKind::Other, "File validation failed"))
}

fn read_file<P>(filename: P) -> std::io::Result<BufReader<fs::File>>
where
  P: AsRef<Path>,
{
  return match fs::File::open(filename) {
    Ok(f) => Ok(BufReader::new(f)),
    Err(e) => {
      println!("ERROR: {}", e);
      Err(e)
    }
  };
}

fn generate_report(parser: &DependencyParser, handlebars: &mut Handlebars, template: &str) {
  let _ = match handlebars.register_template_string("report_template", template) {
    Ok(_) => (),
    Err(e) => panic!("{}", e),
  };

  let values: Vec<ProcessedDependencyObject> = parser.compare_versions();

  let mut file = fs::File::create("gradle-dependencies-diff-report.html").unwrap();

  let _ = file.write_all(
    handlebars
      .render("report_template", &values)
      .unwrap()
      .as_bytes(),
  );
}
