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
