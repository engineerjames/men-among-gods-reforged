/// A simple circular buffer implementation.
pub struct CircularBuffer<T> {
    buffer: Vec<Option<T>>,
    head: usize,
    capacity: usize,
}

impl<T> CircularBuffer<T> {
    /// Creates a new CircularBuffer with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        let mut buffer: Vec<Option<T>> = Vec::with_capacity(capacity);
        buffer.resize_with(capacity, || None);
        CircularBuffer {
            buffer,
            head: 0,
            capacity,
        }
    }

    /// Adds an item to the circular buffer, overwriting the oldest item if full.
    pub fn push(&mut self, item: T) {
        self.buffer[self.head] = Some(item);
        self.head = (self.head + 1) % self.capacity;
    }

    /// Retrieves an item by its index, where 0 is the most recently added item.
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.capacity {
            return None;
        }
        let idx = (self.head + self.capacity - index - 1) % self.capacity;
        self.buffer[idx].as_ref()
    }

    /// Clears the buffer, setting all elements to None.
    pub fn clear(&mut self) {
        for slot in self.buffer.iter_mut() {
            *slot = None;
        }
    }

    /// Returns the number of elements currently stored in the buffer.
    pub fn len(&self) -> usize {
        self.buffer.iter().filter(|item| item.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::CircularBuffer;

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn new_panics_on_zero_capacity() {
        let _ = CircularBuffer::<u8>::new(0);
    }

    #[test]
    fn get_returns_none_when_empty() {
        let buf = CircularBuffer::<i32>::new(3);
        assert_eq!(buf.get(0), None);
        assert_eq!(buf.get(1), None);
        assert_eq!(buf.get(2), None);
    }

    #[test]
    fn get_out_of_range_returns_none() {
        let mut buf = CircularBuffer::new(3);
        buf.push(10);
        assert_eq!(buf.get(3), None);
        assert_eq!(buf.get(999), None);
    }

    #[test]
    fn push_then_get_most_recent() {
        let mut buf = CircularBuffer::new(3);
        buf.push(42);
        assert_eq!(buf.get(0), Some(&42));
        assert_eq!(buf.get(1), None);
        assert_eq!(buf.get(2), None);
    }

    #[test]
    fn preserves_reverse_insertion_order() {
        let mut buf = CircularBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.get(0), Some(&3));
        assert_eq!(buf.get(1), Some(&2));
        assert_eq!(buf.get(2), Some(&1));
    }

    #[test]
    fn overwrites_oldest_when_full() {
        let mut buf = CircularBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        buf.push(4);
        assert_eq!(buf.get(0), Some(&4));
        assert_eq!(buf.get(1), Some(&3));
        assert_eq!(buf.get(2), Some(&2));
    }

    #[test]
    fn capacity_one_behaves_as_latest_value() {
        let mut buf = CircularBuffer::new(1);
        buf.push(7);
        assert_eq!(buf.get(0), Some(&7));
        buf.push(8);
        assert_eq!(buf.get(0), Some(&8));
        assert_eq!(buf.get(1), None);
    }

    #[test]
    fn clear_empties_all_slots() {
        let mut buf = CircularBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        buf.clear();
        assert_eq!(buf.get(0), None);
        assert_eq!(buf.get(1), None);
        assert_eq!(buf.get(2), None);
    }

    #[test]
    fn push_after_clear_still_works() {
        let mut buf = CircularBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.clear();
        buf.push(9);
        assert_eq!(buf.get(0), Some(&9));
        assert_eq!(buf.get(1), None);
        assert_eq!(buf.get(2), None);
    }
}
