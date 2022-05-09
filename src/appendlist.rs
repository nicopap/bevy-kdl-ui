// TODO: migrate off pointers. For example it's possible to use a simple
// ID that is the index of the declaration and query the binding set
// from some sort of context
use std::cell::UnsafeCell;

/// A Vec that can only grow in size up to provided
/// capacity.
///
/// This let us create slices for intermediate values of
/// vec that are valid even after a `push` operation.
pub(crate) struct AppendList<T>(UnsafeCell<Vec<T>>);
impl<T> AppendList<T> {
    pub(crate) fn with_capacity(cap: usize) -> Self {
        Self(UnsafeCell::new(Vec::with_capacity(cap)))
    }
    pub(crate) fn push(&self, to_push: T) -> Option<()> {
        // SAFE: Is it safe though???
        let free_space = unsafe {
            let vec = &*self.0.get();
            vec.capacity() > vec.len()
        };
        if free_space {
            // SAFE: Is it safe though???
            let mutable = unsafe { &mut *self.0.get() };
            mutable.push(to_push);
            Some(())
        } else {
            None
        }
    }
    // FIXME TODO: this is unsound, as a mutable pointer can exist at
    // this point in time
    pub(crate) fn as_slice<'a>(&'a self) -> &'a [T] {
        // SAFE: The lifetime is coherced to that of
        // the AppendList. The AppendList never mutates
        // already added elements.
        unsafe {
            let vec = &*self.0.get();
            vec.as_slice()
        }
    }
}
