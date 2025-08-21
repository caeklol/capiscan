use std::mem::MaybeUninit;

// fun! ive never written unsafe code before. i could have used option yes but this is so much more
// enjoyable no? T isn't even gonna be that memory intensive but im assuming its 1GB
pub struct CircularBuffer<T> {
    buffer: Box<[MaybeUninit<T>]>,
    head: usize,
    size: usize
}

impl<T> CircularBuffer<T> {
    // [1, 2, 3]
    pub fn new(size: usize) -> CircularBuffer<T> {
        let v = Box::new_uninit_slice(size);

        CircularBuffer {
            buffer: v,
            head: 0,
            size
        }
    }

    /// pushes a new element to the end of the circular buffer. elements shift left when there is
    /// not enough space.
    /// ```
    /// use mc_scanner::circ::CircularBuffer;
    ///
    /// fn equal<T: std::cmp::PartialEq>(box_slice: Box<[&T]>, vec: Vec<T>) -> bool {
    ///     assert_eq!(box_slice.len(), vec.len());
    ///     box_slice.iter().zip(vec.iter()).all(|(b, v)| *b == v)
    /// }
    ///
    /// fn main() {
    ///     let mut buffer: CircularBuffer<usize> = CircularBuffer::new(2); // heap allocated type
    ///
    ///     assert!(equal(buffer.values(), vec![]));
    ///
    ///     buffer.push(1);
    ///     assert!(equal(buffer.values(), vec![1]));
    ///
    ///     buffer.push(2);
    ///     assert!(equal(buffer.values(), vec![1, 2]));
    ///
    ///     buffer.push(3);
    ///     assert!(equal(buffer.values(), vec![2, 3]));
    /// }
    /// ```
    pub fn push(&mut self, e: T) {
        if (self.head + 1) > self.size {
            // shift
            for idx in 1..self.size {
                unsafe {
                    std::ptr::copy_nonoverlapping(self.buffer[idx].as_mut_ptr(), self.buffer[idx-1].as_mut_ptr(), 1);
                }
            }
            self.buffer[self.size - 1].write(e);
        } else {
            // direct
            self.buffer[self.head].write(e);
            self.head += 1;
        }
    }

    pub fn values(&mut self) -> Box<[&T]> {
        let refs: Vec<&T> = (0..self.head)
            .map(|idx| {
                unsafe {
                    self.buffer[idx].assume_init_ref()
                }
            })
            .collect();

        refs.into_boxed_slice()
    }

    pub fn len(&self) -> usize {
        self.head
    }
}
