//! `DeqColumn<T>`: a contiguous column that grows at both ends.
//!
//! Series columns grow at the back (`Append`, real-time ticks) and at the front
//! (`Prepend` when the user scrolls left and the host loads history). A `Vec`
//! makes front insertion O(n) per call; `VecDeque` is not contiguous, so it
//! cannot be handed out as one slice or uploaded with one copy. `DeqColumn` keeps
//! free space on both sides of a single contiguous buffer.
//!

use core::ops::{Deref, DerefMut};

use crate::error::{Error, Result};

/// Smallest buffer allocated when a column first grows.
const MIN_CAPACITY: usize = 16;

/// A contiguous column with free space at both ends.
///
/// Elements are stored in one contiguous buffer as `[front free | data | back free]`.
/// Adding at either end is amortized O(1), and the data is always available as a
/// single slice (through [`Deref`] to `[T]`), so it can be copied or uploaded in
/// one go.
///
/// When the end that needs room is full:
/// - if at least half of the buffer would still be free, the data is re-centred
///   in place (no allocation);
/// - otherwise a new buffer of at least twice the size is allocated and the data
///   is placed in its centre.
///
/// Either way both ends get room again, so mixed front and back growth stays
/// amortized O(1). Allocation failures are returned as [`Error::OutOfMemory`];
/// nothing panics.
///
/// The whole buffer is kept initialized (free slots hold `T::default()`), which is
/// why `T: Copy + Default`. Column element types (`i64`, `f64`, `f32`, `u32`) all
/// satisfy this.
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::DeqColumn;
///
/// let mut col = DeqColumn::from_vec(vec![3.0, 4.0]);
/// col.push_back(5.0)?;            // append a bar
/// col.extend_front(&[1.0, 2.0])?; // prepend history
/// if let Some(last) = col.last_mut() {
///     *last = 5.5;                // real-time update of the last bar
/// }
/// assert_eq!(&col[..], &[1.0, 2.0, 3.0, 4.0, 5.5]);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct DeqColumn<T> {
    /// Fully initialized buffer; its length is the column's capacity.
    buf: Vec<T>,
    /// Index of the first element in `buf`.
    head: usize,
    /// Number of elements.
    len: usize,
}

#[derive(Clone, Copy)]
enum Side {
    Front,
    Back,
}

impl<T> DeqColumn<T> {
    /// Creates an empty column without allocating.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let col = DeqColumn::<f64>::new();
    /// assert!(col.is_empty());
    /// assert_eq!(col.capacity(), 0);
    /// ```
    pub const fn new() -> Self {
        Self {
            buf: Vec::new(),
            head: 0,
            len: 0,
        }
    }

    /// Takes ownership of `v` without copying. The column has no free space at
    /// either end until it grows.
    ///
    /// This is the path for `set_data`: the input column is moved in as is.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let col = DeqColumn::from_vec(vec![1_i64, 2, 3]);
    /// assert_eq!(&col[..], &[1, 2, 3]);
    /// assert_eq!(col.capacity(), 3);
    /// ```
    pub fn from_vec(v: Vec<T>) -> Self {
        let len = v.len();
        Self {
            buf: v,
            head: 0,
            len,
        }
    }

    /// Total number of slots (used and free) in the buffer.
    ///
    /// The heap allocation may be slightly larger: a `Vec` passed to
    /// [`from_vec`](Self::from_vec) keeps its spare capacity until
    /// [`shrink_to_fit`](Self::shrink_to_fit).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::<u32>::new();
    /// col.push_back(1)?;
    /// assert!(col.capacity() >= 1);
    /// assert_eq!(col.capacity(), col.front_capacity() + col.len() + col.back_capacity());
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// Number of free slots before the first element.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::<f32>::new();
    /// col.reserve_front(10)?;
    /// assert!(col.front_capacity() >= 10);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn front_capacity(&self) -> usize {
        self.head
    }

    /// Number of free slots after the last element.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::<f32>::new();
    /// col.reserve_back(10)?;
    /// assert!(col.back_capacity() >= 10);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn back_capacity(&self) -> usize {
        self.buf.len() - self.head - self.len
    }

    /// The elements as one contiguous slice. Same as dereferencing.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let col = DeqColumn::from_vec(vec![1, 2]);
    /// assert_eq!(col.as_slice(), &[1, 2]);
    /// ```
    pub fn as_slice(&self) -> &[T] {
        &self.buf[self.head..self.head + self.len]
    }

    /// The elements as one contiguous mutable slice. Same as mutable dereferencing.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::from_vec(vec![1, 2]);
    /// col.as_mut_slice()[0] = 9;
    /// assert_eq!(&col[..], &[9, 2]);
    /// ```
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.buf[self.head..self.head + self.len]
    }

    /// Removes all elements. The buffer is kept, and the (empty) data position is
    /// moved to its centre so both ends have room. Does not allocate.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::from_vec(vec![1, 2, 3, 4]);
    /// col.clear();
    /// assert!(col.is_empty());
    /// assert_eq!(col.capacity(), 4);
    /// assert_eq!(col.front_capacity(), 2);
    /// ```
    pub fn clear(&mut self) {
        self.head = self.buf.len() / 2;
        self.len = 0;
    }

    /// Removes up to `front` elements at the front and then up to `back` at
    /// the back, keeping the capacity (undoes an `extend_*`).
    pub(crate) fn truncate_ends(&mut self, front: usize, back: usize) {
        let front = front.min(self.len);
        self.head += front;
        self.len -= front;
        self.len -= back.min(self.len);
    }
}

impl<T: Copy + Default> DeqColumn<T> {
    /// Appends one element at the back. Amortized O(1).
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the buffer has to grow and the allocation fails.
    /// The column is unchanged in that case.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::new();
    /// col.push_back(1.0)?;
    /// col.push_back(2.0)?;
    /// assert_eq!(&col[..], &[1.0, 2.0]);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn push_back(&mut self, value: T) -> Result<()> {
        self.reserve_back(1)?;
        self.buf[self.head + self.len] = value;
        self.len += 1;
        Ok(())
    }

    /// Inserts one element at the front. Amortized O(1).
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the buffer has to grow and the allocation fails.
    /// The column is unchanged in that case.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::from_vec(vec![2.0]);
    /// col.push_front(1.0)?;
    /// assert_eq!(&col[..], &[1.0, 2.0]);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn push_front(&mut self, value: T) -> Result<()> {
        self.reserve_front(1)?;
        self.head -= 1;
        self.buf[self.head] = value;
        self.len += 1;
        Ok(())
    }

    /// Appends all elements of `src` at the back, in order. Amortized O(`src.len()`).
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the buffer has to grow and the allocation fails.
    /// The column is unchanged in that case.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::from_vec(vec![1]);
    /// col.extend_back(&[2, 3])?;
    /// assert_eq!(&col[..], &[1, 2, 3]);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn extend_back(&mut self, src: &[T]) -> Result<()> {
        self.reserve_back(src.len())?;
        let start = self.head + self.len;
        self.buf[start..start + src.len()].copy_from_slice(src);
        self.len += src.len();
        Ok(())
    }

    /// Inserts all elements of `src` at the front, keeping their order: afterwards
    /// the column starts with `src`. Amortized O(`src.len()`); existing elements
    /// are not moved unless the front is full.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the buffer has to grow and the allocation fails.
    /// The column is unchanged in that case.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::from_vec(vec![3]);
    /// col.extend_front(&[1, 2])?;
    /// assert_eq!(&col[..], &[1, 2, 3]);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn extend_front(&mut self, src: &[T]) -> Result<()> {
        self.reserve_front(src.len())?;
        self.head -= src.len();
        self.buf[self.head..self.head + src.len()].copy_from_slice(src);
        self.len += src.len();
        Ok(())
    }

    /// Appends `n` copies of `value` at the back. Amortized O(`n`).
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the allocation fails. The column is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::from_vec(vec![1]);
    /// col.grow_back(2, 0)?;
    /// assert_eq!(&col[..], &[1, 0, 0]);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn grow_back(&mut self, n: usize, value: T) -> Result<()> {
        self.reserve_back(n)?;
        let start = self.head + self.len;
        self.buf[start..start + n].fill(value);
        self.len += n;
        Ok(())
    }

    /// Inserts `n` copies of `value` at the front. Amortized O(`n`).
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the allocation fails. The column is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::from_vec(vec![1]);
    /// col.grow_front(2, 0)?;
    /// assert_eq!(&col[..], &[0, 0, 1]);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn grow_front(&mut self, n: usize, value: T) -> Result<()> {
        self.reserve_front(n)?;
        self.head -= n;
        self.buf[self.head..self.head + n].fill(value);
        self.len += n;
        Ok(())
    }

    /// Makes sure at least `additional` free slots exist after the last element.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the allocation fails or the size overflows.
    /// The column is unchanged in that case.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::<i64>::new();
    /// col.reserve_back(100)?;
    /// let cap = col.capacity();
    /// for i in 0..100 {
    ///     col.push_back(i)?;
    /// }
    /// assert_eq!(col.capacity(), cap); // no further growth
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn reserve_back(&mut self, additional: usize) -> Result<()> {
        if self.back_capacity() >= additional {
            return Ok(());
        }
        self.make_room(additional, Side::Back)
    }

    /// Makes sure at least `additional` free slots exist before the first element.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the allocation fails or the size overflows.
    /// The column is unchanged in that case.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::from_vec(vec![0_i64]);
    /// col.reserve_front(100)?;
    /// let cap = col.capacity();
    /// for i in 1..=100 {
    ///     col.push_front(-i)?;
    /// }
    /// assert_eq!(col.capacity(), cap); // no further growth
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn reserve_front(&mut self, additional: usize) -> Result<()> {
        if self.front_capacity() >= additional {
            return Ok(());
        }
        self.make_room(additional, Side::Front)
    }

    /// Releases all free space: afterwards `capacity() == len()` and the heap
    /// allocation is shrunk as far as the allocator allows. Called after
    /// `set_data` so the steady state carries no growth slack.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::DeqColumn;
    ///
    /// let mut col = DeqColumn::new();
    /// col.extend_back(&[1, 2, 3])?;
    /// col.shrink_to_fit();
    /// assert_eq!(col.capacity(), 3);
    /// assert_eq!(&col[..], &[1, 2, 3]);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn shrink_to_fit(&mut self) {
        if self.head != 0 {
            self.buf.copy_within(self.head..self.head + self.len, 0);
            self.head = 0;
        }
        self.buf.truncate(self.len);
        self.buf.shrink_to_fit();
    }

    /// Ensures `additional` free slots on `side`, either by re-centring in place or
    /// by moving to a larger buffer. On error nothing is modified.
    fn make_room(&mut self, additional: usize, side: Side) -> Result<()> {
        let needed = self.len.checked_add(additional).ok_or(Error::OutOfMemory)?;
        let cap = self.buf.len();

        // Re-centring in place costs O(len). Doing it only while at least half of
        // the buffer stays free guarantees it yields >= len / 2 free slots on the
        // requested side, so the cost is amortized O(1) per element.
        let recentre = needed.checked_mul(2).is_some_and(|twice| cap >= twice);
        if recentre {
            let head = centred_head(cap, needed, additional, side);
            self.buf.copy_within(self.head..self.head + self.len, head);
            self.head = head;
            return Ok(());
        }

        let new_cap = cap
            .saturating_mul(2)
            .max(needed.checked_mul(2).ok_or(Error::OutOfMemory)?)
            .max(MIN_CAPACITY);
        let mut buf = Vec::new();
        buf.try_reserve_exact(new_cap)
            .map_err(|_| Error::OutOfMemory)?;
        // Within the reserved capacity: does not reallocate.
        buf.resize(new_cap, T::default());
        let head = centred_head(new_cap, needed, additional, side);
        buf[head..head + self.len].copy_from_slice(self.as_slice());
        self.buf = buf;
        self.head = head;
        Ok(())
    }
}

/// Where the data starts after re-centring in a buffer of `cap` slots that must
/// hold `needed = len + additional` elements, with the `additional` slots on
/// `side`. The remaining free space is split evenly between both ends.
fn centred_head(cap: usize, needed: usize, additional: usize, side: Side) -> usize {
    let left = (cap - needed) / 2;
    match side {
        Side::Front => left + additional,
        Side::Back => left,
    }
}

impl<T> Default for DeqColumn<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Deref for DeqColumn<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> DerefMut for DeqColumn<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

/// Compares the elements only; free space and capacity are ignored.
impl<T: PartialEq> PartialEq for DeqColumn<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Eq> Eq for DeqColumn<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::VecDeque;

    fn check<T: Copy + PartialEq + core::fmt::Debug>(col: &DeqColumn<T>, model: &VecDeque<T>) {
        assert!(col.head + col.len <= col.buf.len());
        let expected: Vec<T> = model.iter().copied().collect();
        assert_eq!(col.as_slice(), &expected[..]);
    }

    #[test]
    fn empty_column_does_not_allocate() {
        let col = DeqColumn::<f64>::new();
        assert_eq!(col.buf.capacity(), 0);
        assert_eq!(col.len(), 0);
        assert_eq!(col.first(), None);
    }

    #[test]
    fn from_vec_is_zero_copy() {
        let v = vec![1.0_f64, 2.0, 3.0];
        let ptr = v.as_ptr();
        let col = DeqColumn::from_vec(v);
        assert_eq!(col.as_ptr(), ptr);
    }

    #[test]
    fn growth_centres_data() {
        let mut col = DeqColumn::from_vec(vec![0_u32; 100]);
        col.push_back(1).unwrap();
        // New buffer, data in the middle: both ends got room.
        assert!(col.capacity() >= 200);
        assert!(col.front_capacity() >= 40);
        assert!(col.back_capacity() >= 40);
    }

    #[test]
    fn appends_are_amortized() {
        // Pure back growth: the number of reallocations is logarithmic.
        let mut col = DeqColumn::<i64>::new();
        let mut reallocs = 0;
        let mut last_cap = col.capacity();
        for i in 0..1_000_000 {
            col.push_back(i).unwrap();
            if col.capacity() != last_cap {
                reallocs += 1;
                last_cap = col.capacity();
            }
        }
        assert!(reallocs <= 40, "{reallocs} reallocations");
        // Capacity stays within a constant factor of the length.
        assert!(col.capacity() <= 4 * col.len());
    }

    #[test]
    fn prepends_are_amortized() {
        // Elements moved by re-centring/reallocation stay O(n) in total.
        let mut col = DeqColumn::<i64>::new();
        let mut moved = 0usize;
        let mut last_head = col.head;
        for i in 0..1_000_000 {
            let before = col.len();
            col.push_front(-i).unwrap();
            if col.head + 1 != last_head {
                moved += before;
            }
            last_head = col.head;
        }
        assert!(moved <= 4 * col.len(), "moved {moved}");
    }

    #[test]
    fn recentres_in_place_when_mostly_free() {
        let mut col = DeqColumn::<u32>::new();
        col.reserve_back(1000).unwrap();
        let cap = col.capacity();
        col.extend_back(&[7; 10]).unwrap();
        // Front is small after centring for a back reservation; asking for more
        // front room than exists must re-centre, not reallocate.
        let want = col.front_capacity() + 1;
        col.reserve_front(want).unwrap();
        assert_eq!(col.capacity(), cap);
        assert!(col.front_capacity() >= want);
        assert_eq!(&col[..], &[7; 10]);
    }

    #[test]
    fn out_of_memory_is_an_error_and_leaves_column_unchanged() {
        let mut col = DeqColumn::from_vec(vec![1_u64, 2, 3]);
        for additional in [usize::MAX, usize::MAX / 2, usize::MAX / 16] {
            assert_eq!(col.reserve_back(additional), Err(Error::OutOfMemory));
            assert_eq!(col.reserve_front(additional), Err(Error::OutOfMemory));
        }
        assert_eq!(&col[..], &[1, 2, 3]);
        assert_eq!(col.capacity(), 3);
    }

    #[test]
    fn clear_keeps_buffer() {
        let mut col = DeqColumn::from_vec(vec![1_u8; 64]);
        col.clear();
        assert!(col.is_empty());
        assert_eq!(col.capacity(), 64);
        col.extend_front(&[1; 32]).unwrap();
        col.extend_back(&[2; 32]).unwrap();
        assert_eq!(col.capacity(), 64);
    }

    #[test]
    fn shrink_to_fit_moves_data_to_start() {
        let mut col = DeqColumn::<i32>::new();
        col.extend_back(&[1, 2, 3]).unwrap();
        col.extend_front(&[-1, 0]).unwrap();
        col.shrink_to_fit();
        assert_eq!(col.capacity(), 5);
        assert_eq!(col.front_capacity(), 0);
        assert_eq!(&col[..], &[-1, 0, 1, 2, 3]);
        assert!(col.buf.capacity() >= 5);
    }

    #[derive(Clone, Debug)]
    enum Op {
        PushBack(i32),
        PushFront(i32),
        ExtendBack(Vec<i32>),
        ExtendFront(Vec<i32>),
        ReserveBack(usize),
        ReserveFront(usize),
        SetLast(i32),
        Clear,
        Shrink,
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            4 => any::<i32>().prop_map(Op::PushBack),
            4 => any::<i32>().prop_map(Op::PushFront),
            2 => prop::collection::vec(any::<i32>(), 0..50).prop_map(Op::ExtendBack),
            2 => prop::collection::vec(any::<i32>(), 0..50).prop_map(Op::ExtendFront),
            1 => (0usize..200).prop_map(Op::ReserveBack),
            1 => (0usize..200).prop_map(Op::ReserveFront),
            2 => any::<i32>().prop_map(Op::SetLast),
            1 => Just(Op::Clear),
            1 => Just(Op::Shrink),
        ]
    }

    proptest! {
        /// Model test against `VecDeque`: any sequence of operations yields the
        /// same elements in the same order.
        #[test]
        fn behaves_like_vecdeque(init in prop::collection::vec(any::<i32>(), 0..20),
                                 ops in prop::collection::vec(op(), 0..200)) {
            let mut col = DeqColumn::from_vec(init.clone());
            let mut model: VecDeque<i32> = init.into_iter().collect();
            for op in ops {
                match op {
                    Op::PushBack(v) => { col.push_back(v).unwrap(); model.push_back(v); }
                    Op::PushFront(v) => { col.push_front(v).unwrap(); model.push_front(v); }
                    Op::ExtendBack(s) => { col.extend_back(&s).unwrap(); model.extend(s); }
                    Op::ExtendFront(s) => {
                        col.extend_front(&s).unwrap();
                        for v in s.into_iter().rev() { model.push_front(v); }
                    }
                    Op::ReserveBack(n) => {
                        col.reserve_back(n).unwrap();
                        prop_assert!(col.back_capacity() >= n);
                    }
                    Op::ReserveFront(n) => {
                        col.reserve_front(n).unwrap();
                        prop_assert!(col.front_capacity() >= n);
                    }
                    Op::SetLast(v) => {
                        if let Some(last) = col.last_mut() { *last = v; }
                        if let Some(last) = model.back_mut() { *last = v; }
                    }
                    Op::Clear => { col.clear(); model.clear(); }
                    Op::Shrink => {
                        col.shrink_to_fit();
                        prop_assert_eq!(col.capacity(), col.len());
                    }
                }
                check(&col, &model);
            }
        }
    }
}
