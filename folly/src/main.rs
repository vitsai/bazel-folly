use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Error, ErrorKind};
use std::path::Path;

use crate::intrusive_hashmap::MutateExtract;
use crate::types::*;
use crate::util::*;

pub use crate::util::FileType;

mod intrusive_hashmap;
mod types;
mod util;

trait CompileTrie {
  fn write_build_files(&self) -> Result<(), Error>;
}

trait CompileGraph<T: CompileTrie> {
  fn add_initial_subtree(&mut self, file_path: &Path) -> Result<(), Error>;
  fn collapse_cycles(&mut self) -> Result<(), Error>;
  fn generate_compilation_trie(&mut self) -> Result<T, Error>;
}

trait _UnitMap {
  fn add_dependency_edges(
    &mut self,
    file_path: &Path,
    curr_node: UnitObj,
  ) -> Result<(), Error>;
  fn add_node(&mut self, file_path: &Path) -> Result<(), Error>;
}

impl CompileTrie for UnitTrie {
  fn write_build_files(&self) -> Result<(), Error> {
    Ok(())
  }
}

impl _UnitMap for UnitMap {
  fn add_dependency_edges(
    &mut self,
    file_path: &Path,
    curr_node: UnitObj,
  ) -> Result<(), Error> {
    let file = BufReader::new(File::open(file_path)?);
    for line in file.lines() {
      let line = line.unwrap();
      match strip_include(&line) {
        None => continue,
        Some((dep_key, hlib)) => {
          if dep_key == curr_node.key {
            continue;
          }
          println!("{}, {}", dep_key.name, dep_key.root_dir);
          match hlib {
            HeaderLib::FOLLY => {
              let dep_node: UnitObj = self.extract_with_create(dep_key);

              dep_node
                .val
                .borrow_mut()
                .reverse_deps
                .insert(curr_node.clone());
              curr_node.val.borrow_mut().deps.insert(dep_node.clone());
            }
            HeaderLib::UNKNOWN => {
              // TODO other header types
              // in the long run want to auto-populate types based on deps
            }
          };
        }
      }
    }
    Ok(())
  }

  fn add_node(&mut self, file_path: &Path) -> Result<(), Error> {
    let file_name: &str = match file_path.file_name() {
      Some(osstr) => Ok(osstr.to_str().unwrap()),
      None => Err(Error::new(
        ErrorKind::NotFound,
        format!("Could not determine file name {}", file_path.display()),
      )),
    }?;
    let (curr_node_name, file_type): (String, FileType) =
      strip_file_name(file_name)?;

    if file_type == FileType::UNKNOWN {
      println!("Ignoring file: {}", curr_node_name);
      return Ok(());
    }

    let parent_string = match Path::parent(file_path) {
      Some(path) => match path.to_str() {
        Some(path_str) => Ok(path_str.to_string()),
        None => Err(std::io::Error::new(
          ErrorKind::NotFound,
          format!("Failure converting {} to string", path.display()),
        )),
      },
      None => Err(std::io::Error::new(
        ErrorKind::NotFound,
        "Parent dir not found.",
      )),
    }?;

    // Populate initial information.
    let curr_key = UnitKey {
      name: curr_node_name,
      root_dir: parent_string,
    };
    let curr_node: UnitObj = self.extract_with_create(curr_key);
    match file_type {
      FileType::TEMPLATE | FileType::HEADER => curr_node
        .val
        .borrow_mut()
        .headers
        .push(file_name.to_string()),
      FileType::SOURCE | FileType::TEST => {
        curr_node.val.borrow_mut().srcs.push(file_name.to_string())
      }
      FileType::UNKNOWN => unreachable!(),
    }

    println!("Path {}", file_path.display());
    self.add_dependency_edges(file_path, curr_node)
  }
}

impl CompileGraph<UnitTrie> for UnitMap {
  fn add_initial_subtree(&mut self, file_path: &Path) -> Result<(), Error> {
    if file_path.is_dir() {
      for child in std::fs::read_dir(file_path)? {
        self.add_initial_subtree(&child?.path())?;
      }
    } else {
      self.add_node(file_path)?;
    }
    Ok(())
  }

  fn collapse_cycles(&mut self) -> Result<(), Error> {
    Ok(())
  }

  // TODO trie-building
  fn generate_compilation_trie(&mut self) -> Result<UnitTrie, Error> {
    Ok(())
  }
}

fn main() {
  let root = Path::new("/Users/victoria/folly");
  let input_root = root.join("folly");
  let mut dict: UnitMap = HashSet::new();
  match dict.add_initial_subtree(&input_root) {
    Ok(_) => match dict.collapse_cycles() {
      Ok(_) => match dict.generate_compilation_trie() {
        Ok(trie) => match trie.write_build_files() {
          Ok(_) => println!("Successfully generated Starlark build files."),
          Err(_) => {
            println!("Failed to generate build files for compilation units.")
          }
        },
        Err(_) => {
          println!("Failed to generate trie of compilation units.")
        }
      },
      Err(_) => println!("Failed to collapse cycles in dependency graph."),
    },
    Err(_) => println!("Failed to populate initial minimal compilation units."),
  }
}
