use std::io::Error;

use crate::types::UnitKey;

#[derive(PartialEq)]
pub enum FileType {
  UNKNOWN,
  HEADER,
  SOURCE,
  TEMPLATE,
  TEST,
}

#[derive(PartialEq)]
pub enum HeaderLib {
  UNKNOWN,
  FOLLY,
}

#[derive(PartialEq)]
pub enum CharType {
  DELIM,
  UPPER,
  LOWER,
  REGULAR,
}

pub fn get_char_type(c: char) -> CharType {
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

pub fn camel_to_snake(string: &str) -> String {
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

// TODO if we fail in combining cc and h in one unit, try again with cc and h
// all in their own units.
pub fn strip_file_name(file_name: &str) -> Result<(String, FileType), Error> {
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

pub fn strip_include(line: &str) -> Option<(UnitKey, HeaderLib)> {
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
