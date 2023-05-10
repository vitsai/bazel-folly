use crate::intrusive_hashmap::{HashMap, HashObj};
use std::collections::HashSet;
use std::hash::Hash;

#[derive(Default, PartialEq, Eq, Hash)]
pub struct UnitKey {
  pub name: String,
  pub root_dir: String,
}

// TODO if we need to compare key against deps, reverse_deps,
// then we can turn into HashSet<HashWrap...> instead.
#[derive(Default)]
pub struct UnitInfo<K: Hash> {
  pub headers: Vec<String>,
  pub srcs: Vec<String>,
  pub deps: HashSet<HashObj<K, UnitInfo<K>>>,
  pub reverse_deps: HashSet<HashObj<K, UnitInfo<K>>>,
}

pub type UnitObj = HashObj<UnitKey, UnitInfo<UnitKey>>;
pub type UnitMap = HashMap<UnitKey, UnitInfo<UnitKey>>;
// TODO
pub type UnitTrie = ();
