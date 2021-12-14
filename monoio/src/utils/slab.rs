//! Slab
// Forked from https://github.com/tokio-rs/slab/blob/master/src/lib.rs
// Copyright (c) 2021 Tokio Contributors, licensed under the MIT license.

/// Pre-allocated storage for a uniform data type
#[derive(Clone)]
pub struct Slab<T> {
    // Chunk of memory
    entries: Vec<Entry<T>>,

    // Number of Filled elements currently in the slab
    len: usize,

    // Offset of the next available slot in the slab. Set to the slab's
    // capacity when the slab is full.
    next: usize,
}

impl<T> Default for Slab<T> {
    fn default() -> Self {
        Slab::new()
    }
}

#[derive(Clone)]
enum Entry<T> {
    Vacant(usize),
    Occupied(Option<T>),
}

impl<T> Slab<T> {
    /// Construct a new, empty `Slab`.
    pub fn new() -> Slab<T> {
        Slab::with_capacity(0)
    }

    /// Construct a new, empty `Slab` with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Slab<T> {
        Slab {
            entries: Vec::with_capacity(capacity),
            next: 0,
            len: 0,
        }
    }

    /// Return the number of values the slab can store without reallocating.
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Clear the slab of all values.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.len = 0;
        self.next = 0;
    }

    /// Return the number of stored values.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return `true` if there are no values stored in the slab.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return a reference to the value associated with the given key.
    pub fn get(&self, key: usize) -> Option<&T> {
        match self.entries.get(key) {
            Some(&Entry::Occupied(ref val)) => Some(val.as_ref().unwrap()),
            _ => None,
        }
    }

    /// Return a mutable reference to the value associated with the given key.
    pub fn get_mut(&mut self, key: usize) -> Option<&mut T> {
        match self.entries.get_mut(key) {
            Some(&mut Entry::Occupied(ref mut val)) => {
                Some(unsafe { val.as_mut().unwrap_unchecked() })
            }
            _ => None,
        }
    }

    /// Return a reference to the value associated with the given key without
    /// performing bounds checking.
    ///
    /// # Safety
    ///
    /// The key must be within bounds.
    pub unsafe fn get_unchecked(&self, key: usize) -> &T {
        match *self.entries.get_unchecked(key) {
            Entry::Occupied(ref val) => val.as_ref().unwrap_unchecked(),
            _ => std::hint::unreachable_unchecked(),
        }
    }

    /// Return a mutable reference to the value associated with the given key
    /// without performing bounds checking.
    ///
    /// # Safety
    ///
    /// The key must be within bounds.
    pub unsafe fn get_unchecked_mut(&mut self, key: usize) -> &mut T {
        match *self.entries.get_unchecked_mut(key) {
            Entry::Occupied(ref mut val) => val.as_mut().unwrap_unchecked(),
            _ => std::hint::unreachable_unchecked(),
        }
    }

    /// Insert a value in the slab, returning key assigned to the value.
    pub fn insert(&mut self, val: T) -> usize {
        let key = self.next;

        self.len += 1;

        if key == self.entries.len() {
            self.entries.push(Entry::Occupied(Some(val)));
            self.next = key + 1;
        } else {
            self.next = match self.entries.get(key) {
                Some(&Entry::Vacant(next)) => next,
                _ => unsafe { std::hint::unreachable_unchecked() },
            };
            self.entries[key] = Entry::Occupied(Some(val));
        }

        key
    }

    /// Tries to remove the value associated with the given key,
    /// returning the value if the key existed.
    pub fn try_remove(&mut self, key: usize) -> Option<T> {
        if let Some(entry) = self.entries.get_mut(key) {
            // Swap the entry at the provided value
            let prev = std::mem::replace(entry, Entry::Vacant(self.next));

            match prev {
                Entry::Occupied(val) => {
                    self.len -= 1;
                    self.next = key;
                    return val;
                }
                _ => {
                    // Woops, the entry is actually vacant, restore the state
                    *entry = prev;
                }
            }
        }
        None
    }

    /// Remove and return the value associated with the given key.
    pub fn remove(&mut self, key: usize) -> T {
        self.try_remove(key).expect("invalid key")
    }

    /// Return `true` if a value is associated with the given key.
    pub fn contains(&self, key: usize) -> bool {
        matches!(self.entries.get(key), Some(&Entry::Occupied(_)))
    }

    /// Execute f and return.
    ///
    /// # Safety
    /// very unsafe! :)
    pub unsafe fn do_action_unchecked<F, O>(&mut self, key: usize, f: F) -> O
    where
        F: FnOnce(&mut Option<T>) -> O,
    {
        // Find the slot first.
        let slot = self.entries.get_unchecked_mut(key);
        // Get its value.
        let val = match *slot {
            Entry::Occupied(ref mut val) => val,
            _ => std::hint::unreachable_unchecked(),
        };
        // Call user provided lambda.
        let output = f(val);

        // If the lambda set the value to None, we will delete it.
        if val.is_none() {
            *slot = Entry::Vacant(self.next);
            self.next = key;
        }
        output
    }
}

impl<T> std::ops::Index<usize> for Slab<T> {
    type Output = T;

    fn index(&self, key: usize) -> &T {
        match self.entries.get(key) {
            Some(&Entry::Occupied(ref v)) => unsafe { v.as_ref().unwrap_unchecked() },
            _ => panic!("invalid key"),
        }
    }
}

impl<T> std::ops::IndexMut<usize> for Slab<T> {
    fn index_mut(&mut self, key: usize) -> &mut T {
        match self.entries.get_mut(key) {
            Some(&mut Entry::Occupied(ref mut v)) => unsafe { v.as_mut().unwrap_unchecked() },
            _ => panic!("invalid key"),
        }
    }
}

impl<T> std::fmt::Debug for Slab<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("Slab")
            .field("len", &self.len)
            .field("cap", &self.capacity())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_remove_one() {
        let mut slab = Slab::default();
        assert!(slab.is_empty());

        let key = slab.insert(10);

        assert_eq!(slab[key], 10);
        assert_eq!(slab.get(key), Some(&10));
        assert!(!slab.is_empty());
        assert!(slab.contains(key));

        assert_eq!(slab.remove(key), 10);
        assert!(!slab.contains(key));
        assert!(slab.get(key).is_none());
    }

    #[test]
    fn insert_get_many() {
        let mut slab = Slab::with_capacity(10);
        assert_eq!(slab.capacity(), 10);

        for i in 0..10 {
            let key = slab.insert(i + 10);
            assert_eq!(slab[key], i + 10);
        }
    }

    #[test]
    fn insert_get_remove_many() {
        let mut slab = Slab::new();
        let mut keys = vec![];

        for i in 0..10 {
            for j in 0..10 {
                let val = (i * 10) + j;

                let key = slab.insert(val);
                keys.push((key, val));
                assert_eq!(slab[key], val);
            }

            for (key, val) in keys.drain(..) {
                assert_eq!(val, slab.remove(key));
            }
        }
    }

    #[test]
    fn clone_and_clear_slab() {
        let mut slab = Slab::new();
        slab.insert(10);
        let mut slab = slab.clone();
        assert_eq!(slab.len(), 1);
        slab.clear();
        assert_eq!(slab.len(), 0);
        assert!(slab.is_empty());
    }

    #[test]
    fn get_unchecked() {
        let mut slab = Slab::new();
        let id = slab.insert(10);
        unsafe {
            assert_eq!(slab.get_unchecked(id), &10);
            *slab.get_unchecked_mut(id) = 20;
        }
        assert_eq!(*slab.get(id).unwrap(), 20);
    }

    #[test]
    fn try_remove() {
        let mut slab = Slab::new();
        let id = slab.insert(10);
        assert!(slab.try_remove(10000).is_none());
        assert!(slab.try_remove(id).is_some());
    }

    #[test]
    fn get_index() {
        let mut slab = Slab::new();
        let id = slab.insert(10);
        assert_eq!(slab[id], 10);
        slab[id] = 20;
        assert_eq!(slab[id], 20);
    }

    #[test]
    #[should_panic(expected = "invalid key")]
    fn get_index_panic() {
        let slab = Slab::<i32>::new();
        let _ = &slab[10000];
    }

    #[test]
    #[should_panic(expected = "invalid key")]
    fn get_index_mut_panic() {
        let mut slab = Slab::<i32>::new();
        slab[10000] = 20;
    }

    #[test]
    fn test_debug() {
        let mut slab = Slab::new();
        slab.insert(10);
        assert!(format!("{:?}", slab).contains("Slab"));
    }
}
