use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::*;
use std::io::{BufRead, BufReader, Error, ErrorKind};
use std::path::Path;
use std::rc::Rc;

// TODO we use a lot of clone for things consumed by the dict
// when we know the dict will outlast whatever is being consumed.
// Learn lifetimes and find a way to turn into references.

#[derive(PartialEq)]
enum FileType {
  UNKNOWN,
  HEADER,
  SOURCE,
  TEMPLATE,
  TEST,
}

// Probably not the best way to work around needing
// mutable references to two values at once.
type UnitDict = HashMap<UnitKey, Rc<RefCell<Box<UnitVal>>>>;

#[derive(Clone, Eq, Hash, PartialEq)]
struct UnitKey {
  name: String,
  root_dir: String,
}

#[derive(Default)]
struct UnitVal {
  headers: Vec<String>,
  srcs: Vec<String>,
  deps: Vec<UnitKey>,
  reverse_deps: Vec<UnitKey>,
}

#[derive(PartialEq)]
enum HeaderLib {
  UNKNOWN,
  FOLLY,
}

fn strip_include(line: &str) -> Option<(UnitKey, HeaderLib)> {
  if !line.starts_with("#include") {
    return None;
  }

  let extract_unit = |start, end| {
    let path: &str = line[start..end]
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
        root_dir: path[0..i].to_string(),
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
      Some(start) => match line[(start + 1)..].find('"') {
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

fn strip_file_name(file_name: &str) -> Result<(String, FileType), Error> {
  // Brittle order.
  let suffixes = [
    ("test.cpp", FileType::TEST),
    (".cpp", FileType::SOURCE),
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

fn add_initial_subtree(
  file_path: &Path,
  nodes: &mut UnitDict,
) -> Result<(), Error> {
  if file_path.is_dir() {
    for child in std::fs::read_dir(file_path)? {
      add_initial_subtree(&child?.path(), nodes)?;
    }
  } else {
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

    let curr_key = UnitKey {
      name: curr_node_name,
      root_dir: parent_string,
    };
    // TODO try putting in a trait and see if borrow checker will allow
    // let curr_node : RefMut<Box<UnitVal>> = nodes.create_extract_mut(&curr_key);
    if !nodes.contains_key(&curr_key) {
      nodes.insert(
        curr_key.clone(),
        Rc::new(RefCell::new(Box::new(Default::default()))),
      );
    }
    let curr_node: Rc<RefCell<Box<UnitVal>>> =
      nodes.get(&curr_key).unwrap().clone();
    match file_type {
      FileType::TEMPLATE | FileType::HEADER => {
        curr_node.borrow_mut().headers.push(file_name.to_string())
      }
      FileType::SOURCE | FileType::TEST => {
        curr_node.borrow_mut().srcs.push(file_name.to_string())
      }
      FileType::UNKNOWN => unreachable!(),
    }

    println!("Path {}", file_path.display());
    let file = BufReader::new(File::open(file_path)?);
    for line in file.lines() {
      let line = line.unwrap();
      match strip_include(&line) {
        None => continue,
        Some((dep_key, hlib)) => {
          if dep_key == curr_key {
            continue;
          }
          println!("{}, {}", dep_key.name, dep_key.root_dir);
          match hlib {
            HeaderLib::FOLLY => {
              if !nodes.contains_key(&dep_key) {
                nodes.insert(
                  dep_key.clone(),
                  Rc::new(RefCell::new(Box::new(Default::default()))),
                );
              }
              let dep_node: Rc<RefCell<Box<UnitVal>>> =
                nodes.get(&dep_key).unwrap().clone();

              dep_node.borrow_mut().reverse_deps.push(curr_key.clone());
              curr_node.borrow_mut().deps.push(dep_key);
            }
            HeaderLib::UNKNOWN => {
              // TODO other header types
              // in the long run want to auto-populate types based on deps
            }
          };
        }
      }
    }
  }

  Ok(())
}

fn merge(
  node1: (UnitKey, UnitVal),
  node2: (UnitKey, UnitVal),
) -> (UnitKey, UnitVal) {
  // TODO merge
  node1
}

fn collapse_cycles(dict: &mut UnitDict) {}

fn write_build_files(dict: &mut UnitDict) {}

fn main() {
  let root = Path::new("/Users/victoria/folly");
  let input_root = root.join("folly");
  let mut dict: UnitDict = HashMap::new();
  match add_initial_subtree(&input_root, &mut dict) {
    Ok(_) => {
      collapse_cycles(&mut dict);
      write_build_files(&mut dict);
    }
    Err(_) => println!("Failed to populate initial minimal compilation units."),
  }
}
