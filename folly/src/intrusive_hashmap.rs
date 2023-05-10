use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

pub trait MutateExtract<K, V> {
  fn extract_with_create(&mut self, key: K) -> V;
}

impl<K: Eq + Hash + PartialEq, V: Default> MutateExtract<K, HashObj<K, V>>
  for HashMap<K, V>
{
  fn extract_with_create(&mut self, key: K) -> HashObj<K, V> {
    if self.contains(&key) {
      self.get(&key).unwrap().0.clone()
    } else {
      let val = Rc::new(IntrusiveRefCell::from(key));
      self.insert(HashWrap(val.clone()));
      val
    }
  }
}

// Potentially not the best way to work around needing
// mutable references to two values at once.
pub type HashObj<K, V> = Rc<IntrusiveRefCell<K, V>>;
pub type HashMap<K, V> = HashSet<HashWrap<K, V>>;

// IntrusiveRefCell encapsulates a key and a RefCell<value>.
// It is meant to be the (key, value) pair in an intrusive hashmap, so it implements PartialEq, Eq,
// and Hash by leveragint the trait implementations of the key.
//
// Note that the value on a borrowed IntrusiveRefCell is still mutable; this allows concurrent modification of multiple distinct entries in the intrusive hashmap.
#[derive(Default)]
pub struct IntrusiveRefCell<K, V> {
  pub key: K,
  pub val: RefCell<V>,
}

impl<K: PartialEq, V> PartialEq for IntrusiveRefCell<K, V> {
  fn eq(&self, other: &Self) -> bool {
    self.key == other.key
  }
}

impl<K: Eq, V> Eq for IntrusiveRefCell<K, V> {}

impl<K: Hash, V> Hash for IntrusiveRefCell<K, V> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.key.hash(state);
  }
}

impl<K, V: Default> From<K> for IntrusiveRefCell<K, V> {
  fn from(item: K) -> Self {
    IntrusiveRefCell {
      key: item,
      val: Default::default(),
    }
  }
}

// Sad! Borrow trait not transitive. We wouldn't need this if:
// Rc<T>: Borrow<T> && T: Borrow<T'> => Rc<T>: Borrow<T'>
// This is also the reason HashObj uses the newtype pattern
// and not the alias. Very unfortunate.
pub struct HashWrap<K, V>(HashObj<K, V>);
impl<K, V> Borrow<K> for HashWrap<K, V> {
  fn borrow(&self) -> &K {
    &self.0.key
  }
}

impl<K: Hash, V> Hash for HashWrap<K, V> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.hash(state);
  }
}

impl<K: PartialEq, V> PartialEq for HashWrap<K, V> {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl<K: Eq, V> Eq for HashWrap<K, V> {}

impl<K, V> From<HashObj<K, V>> for HashWrap<K, V> {
  fn from(item: HashObj<K, V>) -> Self {
    HashWrap(item)
  }
}
