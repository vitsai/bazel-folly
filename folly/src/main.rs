use std::collections::HashSet;
use std::fs::*;
use std::hash::Hash;
use std::io::{BufRead, BufReader, Error, ErrorKind};
use std::path::Path;

use crate::intrusive_hashmap::{HashMap, HashObj, MutateExtract};

mod intrusive_hashmap;

#[derive(PartialEq)]
enum FileType {
  UNKNOWN,
  HEADER,
  SOURCE,
  TEMPLATE,
  TEST,
}

#[derive(PartialEq)]
enum HeaderLib {
  UNKNOWN,
  FOLLY,
}

#[derive(PartialEq)]
enum CharType {
  DELIM,
  UPPER,
  LOWER,
  REGULAR,
}

fn get_char_type(c: char) -> CharType {
  if c == '_' {
    return CharType::DELIM;
  } else if (c >= 'a') & (c <= 'z') {
    return CharType::LOWER;
  } else if (c >= 'A') & (c <= 'Z') {
    return CharType::UPPER;
  } else {
    return CharType::REGULAR;
  }
}

#[derive(Default, PartialEq, Eq, Hash)]
struct UnitKey {
  name: String,
  root_dir: String,
}

// TODO if we need to compare key against deps, reverse_deps,
// then we can turn into HashSet<HashWrap...> instead.
#[derive(Default)]
struct UnitInfo<K: Hash> {
  headers: Vec<String>,
  srcs: Vec<String>,
  deps: HashSet<HashObj<K, UnitInfo<K>>>,
  reverse_deps: HashSet<HashObj<K, UnitInfo<K>>>,
}

type UnitObj = HashObj<UnitKey, UnitInfo<UnitKey>>;
type UnitMap = HashMap<UnitKey, UnitInfo<UnitKey>>;
// TODO
type UnitTrie = ();

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

// TODO if we fail in combining cc and h in one unit, try again with cc and h
// all in their own units.
fn strip_file_name(file_name: &str) -> Result<(String, FileType), Error> {
  // Brittle order.
  let suffixes = [
    ("test.cpp", FileType::TEST),
    ("test.cc", FileType::TEST),
    (".cpp", FileType::SOURCE),
    (".cc", FileType::SOURCE),
    ("-inl.h", FileType::TEMPLATE),
    (".h", FileType::HEADER),
  ];

  for (suffix, file_type) in suffixes {
    if file_name.ends_with(suffix) {
      return Ok((
        camel_to_snake(file_name.trim_end_matches(suffix)),
        file_type,
      ));
    }
  }

  Ok((file_name.to_string(), FileType::UNKNOWN))
}

fn camel_to_snake(string: &str) -> String {
  let mut prev_char = CharType::DELIM;
  let mut word_start = 0;
  let mut snake_string: String = String::new();
  for (i, c) in string.chars().enumerate() {
    let curr_char = get_char_type(c);

    if curr_char == CharType::DELIM {
      snake_string += &string[word_start..i].to_lowercase();
      snake_string.push(c);
      // Technically unnecessary, but just to be safe.
      word_start = i;
    } else if prev_char == CharType::DELIM {
      word_start = i;
    } else if ((prev_char == CharType::LOWER)
      | (prev_char == CharType::REGULAR))
      & (curr_char == CharType::UPPER)
    {
      snake_string += &string[word_start..i].to_lowercase();
      snake_string.push('_');
      word_start = i;
    } else if (prev_char == CharType::UPPER)
      & (curr_char == CharType::LOWER)
      & (word_start < (i - 1))
    {
      // To accomodate all-caps words, we admit no 1-character words.
      snake_string += &string[word_start..(i - 1)].to_lowercase();
      snake_string.push('_');
      word_start = i - 1;
    }
    prev_char = curr_char;
  }
  snake_string += &string[word_start..].to_lowercase();
  snake_string
}

fn strip_include(line: &str) -> Option<(UnitKey, HeaderLib)> {
  if !line.starts_with("#include") {
    return None;
  }

  let extract_unit = |start, end| {
    let path: &str = &line[start..end]
      .trim_end_matches("-inl.h")
      .trim_end_matches(".h");
    let root: &str = match path.find('/') {
      None => path,
      Some(i) => &path[0..i],
    };

    let key: UnitKey = match path.rfind('/') {
      None => UnitKey {
        name: camel_to_snake(path),
        root_dir: String::new(),
      },
      Some(i) => UnitKey {
        name: camel_to_snake(&path[(i + 1)..]),
        root_dir: (&path[0..i]).to_string(),
      },
    };

    match root {
      "folly" => Some((key, HeaderLib::FOLLY)),
      _ => Some((key, HeaderLib::UNKNOWN)),
    }
  };

  match line.find('<') {
    Some(start) => match line.find('>') {
      Some(end) => extract_unit(start + 1, end),
      None => {
        assert!(false, "Bad include! {}", line);
        None
      }
    },
    None => match line.find('"') {
      Some(start) => match (&line[(start + 1)..]).find('"') {
        Some(end) => extract_unit(start + 1, end),
        None => {
          assert!(false, "Bad include! {}", line);
          None
        }
      },
      None => {
        println!("Unexpected include: {}", line);
        None
      }
    },
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
