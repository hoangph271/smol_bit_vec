use std::iter::FusedIterator;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SmolBitVecBits {
    Inline(usize),
    Heap(Box<[usize]>),
}

#[derive(Clone)]
pub struct SmolBitVec {
    len: usize,
    bits: SmolBitVecBits,
}

impl PartialEq for SmolBitVec {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }

        match (&self.bits, &other.bits) {
            (SmolBitVecBits::Inline(a_bits), SmolBitVecBits::Inline(b_bits)) => a_bits == b_bits,
            (SmolBitVecBits::Heap(a_bits_chunks), SmolBitVecBits::Heap(b_bits_chunks)) => {
                let chunks_size_in_used = (self.len() + BITS_PER_WORD - 1) / BITS_PER_WORD;
                let a_used_bits = &a_bits_chunks[..chunks_size_in_used];
                let b_used_bits = &b_bits_chunks[..chunks_size_in_used];

                a_used_bits == b_used_bits
            }
            (SmolBitVecBits::Inline(a), SmolBitVecBits::Heap(b)) => {
                let a_bits = *a;
                let b_bits = b[0];
                a_bits == b_bits
            }
            (SmolBitVecBits::Heap(a), SmolBitVecBits::Inline(b)) => {
                let a_bits = a[0];
                let b_bits = *b;
                a_bits == b_bits
            }
        }
    }
}

impl Eq for SmolBitVec {}

const BITS_PER_WORD: usize = usize::BITS as usize;

fn is_inlineable_len(len: usize) -> bool {
    len <= BITS_PER_WORD
}

impl SmolBitVec {
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, value: bool) {
        let len = self.len();
        let next_len = len + 1;

        match &mut self.bits {
            SmolBitVecBits::Inline(bits) => {
                if is_inlineable_len(next_len) {
                    if value {
                        let mask = 1usize << len;

                        *bits |= mask;
                    }
                } else {
                    self.bits = SmolBitVecBits::Heap(Box::new([*bits, value as usize]));
                }
            }
            SmolBitVecBits::Heap(items) => {
                let reserved_len = items.len() * BITS_PER_WORD;
                let needs_new_item = next_len > reserved_len;

                if needs_new_item {
                    *items = items
                        .iter()
                        .copied()
                        .chain([if value { 1usize } else { 0usize }])
                        .collect::<Vec<usize>>()
                        .into_boxed_slice();
                } else {
                    if value {
                        let item_offset = len % BITS_PER_WORD;
                        let item_index = len / BITS_PER_WORD;
                        let mask = 1usize << item_offset;

                        if let Some(last) = items.get_mut(item_index) {
                            *last |= mask;
                        }
                    }
                }
            }
        }

        self.len = next_len;
    }

    pub fn get(&self, index: usize) -> Option<bool> {
        if index >= self.len() {
            return None;
        }

        match &self.bits {
            SmolBitVecBits::Inline(bits) => {
                let mask = 1usize << index;

                Some(bits & mask != 0)
            }
            SmolBitVecBits::Heap(items) => {
                let item_index = index / BITS_PER_WORD;
                let item_offset = index % BITS_PER_WORD;

                // We can also use bit shift operations to get item_index
                // and bit AND operations to get item_offset
                // but the compiler is smart enough to optimize the above into the below
                // so for readability, we use the above

                // // ? trailing_zeros() gives 6 for 64-bit, so right shift by 6 gives item_index
                // let item_index = index >> BITS_PER_WORD.trailing_zeros();
                // // ? index & (BITS_PER_WORD - 1) gives item_offset (bit position within item)
                // let item_offset = index & (BITS_PER_WORD - 1);

                let item_container = items[item_index];

                let mask = 1usize << item_offset;

                Some(item_container & mask != 0)
            }
        }
    }

    pub fn set(&mut self, index: usize, value: bool) -> Option<bool> {
        if index >= self.len() {
            return None;
        }

        match &mut self.bits {
            SmolBitVecBits::Inline(bits) => {
                let mask = 1usize << index;
                let old_value = *bits & mask != 0;

                if value {
                    // Toggle the bit ON if it was OFF, leave it on otherwise
                    *bits |= mask
                } else {
                    // Invert the mask, making every bits ON except at index
                    // &= will turn the bit at index in bits to OFF
                    *bits &= !mask
                }

                Some(old_value)
            }
            SmolBitVecBits::Heap(items) => {
                let item_index = index / BITS_PER_WORD;
                let item_offset = index % BITS_PER_WORD;

                let item_container = items[item_index];

                // Every bit in mask is shifted left by item_offset, so it's 1 at the bit we want to toggle
                let mask = 1usize << item_offset;

                let old_value = item_container & mask != 0;

                if value {
                    // Toggle the bit ON if it was OFF, leave it on otherwise
                    items[item_index] |= mask
                } else {
                    // Toggle the bit OFF if it was ON, leave it off otherwise
                    items[item_index] &= !mask
                }

                Some(old_value)
            }
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn pop(&mut self) -> Option<bool> {
        if self.is_empty() {
            return None;
        }

        let last_index = self.len() - 1;
        let next_len = self.len - 1;

        let value = match &mut self.bits {
            SmolBitVecBits::Inline(bits) => {
                let mask = 1usize << last_index;
                let value = *bits & (mask) != 0;

                *bits &= !mask;

                value
            }
            SmolBitVecBits::Heap(items) => {
                let item_index = last_index / BITS_PER_WORD;
                let item_offset = last_index % BITS_PER_WORD;

                let value = items[item_index] & (1usize << item_offset) != 0;

                if is_inlineable_len(next_len) {
                    self.bits = SmolBitVecBits::Inline(items[0]);
                } else {
                    let bits_chunk_size = items.len();

                    if item_offset == 0 && item_index == bits_chunk_size - 1 {
                        *items = Box::from(&items[0..items.len() - 1]);
                    } else {
                        if let Some(block) = items.get_mut(item_index) {
                            let mask = 1usize << item_offset;
                            *block &= !mask;
                        }
                    }
                }

                value
            }
        };

        self.len -= 1;

        Some(value)
    }

    pub fn clear(&mut self) {
        self.len = 0;
        self.bits = SmolBitVecBits::Inline(0);
    }

    pub fn last(&self) -> Option<bool> {
        if self.len == 0 {
            return None;
        }

        let index = self.len - 1;

        match &self.bits {
            SmolBitVecBits::Inline(inline) => {
                let value = (inline >> (index % BITS_PER_WORD)) & 1 != 0;
                Some(value)
            }
            SmolBitVecBits::Heap(heap) => {
                let value = (heap[index / BITS_PER_WORD] >> (index % BITS_PER_WORD)) & 1 != 0;
                Some(value)
            }
        }
    }

    pub fn first(&self) -> Option<bool> {
        if self.len == 0 {
            return None;
        }

        let bits_block = match &self.bits {
            SmolBitVecBits::Inline(inline) => inline,
            SmolBitVecBits::Heap(bits) => &bits[0],
        };

        Some(bits_block & 1 != 0)
    }

    pub fn reserve(&mut self, additional: usize) {
        let next_len = self.len + additional;

        match &mut self.bits {
            SmolBitVecBits::Inline(inline_bits) => {
                if is_inlineable_len(next_len) {
                    return;
                }

                let next_bits_array_len = (next_len + BITS_PER_WORD - 1) / BITS_PER_WORD;
                let mut bits_vec = Vec::with_capacity(next_bits_array_len);

                bits_vec.push(*inline_bits);
                bits_vec.resize(next_bits_array_len, 0);

                self.bits = SmolBitVecBits::Heap(bits_vec.into_boxed_slice());
            }
            SmolBitVecBits::Heap(items) => {
                let next_bits_array_len = (next_len + BITS_PER_WORD - 1) / BITS_PER_WORD;

                if items.len() >= next_bits_array_len {
                    return;
                }

                let mut bits_vec = Vec::from(items.clone());
                bits_vec.resize(next_bits_array_len, 0);
                *items = bits_vec.into_boxed_slice();
            }
        }
    }
}

impl Default for SmolBitVec {
    fn default() -> Self {
        Self {
            len: 0,
            bits: SmolBitVecBits::Inline(0),
        }
    }
}

impl std::fmt::Debug for SmolBitVec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

impl FromIterator<bool> for SmolBitVec {
    fn from_iter<T: IntoIterator<Item = bool>>(iter: T) -> Self {
        let mut smol_bit_vec = Self::new();

        // TODO: This is inefficient
        // We should remove the loop entirely
        // Only here to fulfill the trait requirement
        for value in iter.into_iter() {
            smol_bit_vec.push(value);
        }

        smol_bit_vec
    }
}

pub struct SmolBitVecIter<'a> {
    vec: &'a SmolBitVec,
    index: usize,
}

impl<'a> Iterator for SmolBitVecIter<'a> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.vec.len() {
            return None;
        }

        let value = self.vec.get(self.index);
        self.index += 1;

        value
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.vec.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for SmolBitVecIter<'a> {
    fn len(&self) -> usize {
        self.vec.len() - self.index
    }
}

impl<'a> FusedIterator for SmolBitVecIter<'a> {}

impl<'a> IntoIterator for &'a SmolBitVec {
    type Item = bool;
    type IntoIter = SmolBitVecIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            vec: self,
            index: 0,
        }
    }
}

impl Extend<bool> for SmolBitVec {
    fn extend<T: IntoIterator<Item = bool>>(&mut self, iter: T) {
        // TODO: Optimize by checking size_hint() and performing bulk transitions.
        // If we know we are adding many bits, transition to Heap once and
        // fill entire usize blocks at a time.
        for item in iter {
            self.push(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO (7): Property-Based Testing.
    // Use `proptest` or `fuzzing` to verify Inline <-> Heap transitions
    // and structural integrity under random operation sequences.

    #[test]
    fn test_new_and_empty() {
        let bv = SmolBitVec::new();
        assert!(bv.is_empty());
        assert_eq!(bv.len(), 0);
    }

    #[test]
    fn test_push_pop_inline() {
        let mut bv = SmolBitVec::new();
        let limit = BITS_PER_WORD;

        for i in 0..limit {
            bv.push(i % 2 == 0);
            assert_eq!(bv.len(), i + 1);
        }

        for i in (0..limit).rev() {
            assert_eq!(bv.pop(), Some(i % 2 == 0));
            assert_eq!(bv.len(), i);
        }
        assert_eq!(bv.pop(), None);
    }

    #[test]
    fn test_spillover_boundary() {
        let mut bv = SmolBitVec::new();
        let limit = BITS_PER_WORD;

        // Fill exactly up to limit (inline)
        for _ in 0..limit {
            bv.push(true);
        }
        assert_eq!(bv.len(), limit);

        // This should trigger spillover
        bv.push(false);
        assert_eq!(bv.len(), limit + 1);
        assert_eq!(bv.get(limit), Some(false));
        assert_eq!(bv.get(limit - 1), Some(true));

        // Pop back to inline
        assert_eq!(bv.pop(), Some(false));
        assert_eq!(bv.len(), limit);
    }

    #[test]
    fn test_get_set_spilled() {
        let mut bv = SmolBitVec::new();
        let large_size = (usize::BITS * 3) as usize;

        for _i in 0..large_size {
            bv.push(false);
        }

        // Set bits across multiple blocks
        bv.set(0, true);
        bv.set(BITS_PER_WORD, true);
        bv.set(large_size - 1, true);

        assert_eq!(bv.get(0), Some(true));
        assert_eq!(bv.get(1), Some(false));
        assert_eq!(bv.get(BITS_PER_WORD), Some(true));
        assert_eq!(bv.get(BITS_PER_WORD + 1), Some(false));
        assert_eq!(bv.get(large_size - 1), Some(true));
        assert_eq!(bv.get(large_size), None);
    }

    #[test]
    fn test_from_iterator_and_extend() {
        let bits = vec![true, false, true, true, false, false, true];
        let mut bv: SmolBitVec = bits.iter().copied().collect();

        assert_eq!(bv.len(), bits.len());
        for (i, &b) in bits.iter().enumerate() {
            assert_eq!(bv.get(i), Some(b));
        }

        let extra = vec![false, true];
        bv.extend(extra.iter().copied());
        assert_eq!(bv.len(), bits.len() + extra.len());
        assert_eq!(bv.get(bits.len()), Some(false));
    }

    #[test]
    fn test_into_iterator() {
        let bits = vec![true, false, true];
        let bv: SmolBitVec = bits.iter().copied().collect();

        // Test &SmolBitVec
        let mut count = 0;
        for (i, b) in (&bv).into_iter().enumerate() {
            assert_eq!(b, bits[i]);
            count += 1;
        }
        assert_eq!(count, 3);

        // Test SmolBitVec
        let collected: Vec<bool> = bv.into_iter().collect();
        assert_eq!(collected, bits);
    }

    #[test]
    fn test_debug_format() {
        let mut bv = SmolBitVec::new();
        bv.push(true);
        bv.push(false);
        let debug_str = format!("{:?}", bv);
        // Ensure it doesn't just show raw bits but looks like a list or similar
        assert!(debug_str.contains("true"));
        assert!(debug_str.contains("false"));
    }

    #[test]
    fn test_default() {
        let bv = SmolBitVec::default();
        assert!(bv.is_empty());
    }

    #[test]
    fn test_inline_specific_behavior() {
        let mut bv = SmolBitVec::new();
        // Test empty state
        assert_eq!(bv.get(0), None);
        assert_eq!(bv.pop(), None);
        // Verify out-of-bounds set fails safely
        assert_eq!(bv.set(0, true), None);

        // Test single bit
        bv.push(true);
        assert_eq!(bv.len(), 1);
        assert_eq!(bv.get(0), Some(true));
        assert_eq!(bv.get(1), None);

        // Test set and get within inline capacity
        assert_eq!(bv.set(0, false), Some(true));
        assert_eq!(bv.get(0), Some(false));

        // Test pop
        assert_eq!(bv.pop(), Some(false));
        assert_eq!(bv.len(), 0);
    }

    #[test]
    fn test_pop_corruption_guard() {
        let mut bv = SmolBitVec::new();

        // Push [true, false, true]
        bv.push(true); // Index 0
        bv.push(false); // Index 1
        bv.push(true); // Index 2

        // Pop the last item (Index 2). It should be true.
        assert_eq!(bv.pop(), Some(true));
        assert_eq!(bv.len(), 2);

        // CRITICAL GUARD: Verify that the remaining bits were NOT shifted!
        assert_eq!(bv.get(0), Some(true), "Index 0 was corrupted by pop!");
        assert_eq!(bv.get(1), Some(false), "Index 1 was corrupted by pop!");
    }

    #[test]
    fn test_inline_full_capacity() {
        let mut bv = SmolBitVec::new();
        let cap = BITS_PER_WORD;

        for i in 0..cap {
            bv.push(i % 3 == 0);
        }

        assert_eq!(bv.len(), cap);

        for i in 0..cap {
            assert_eq!(bv.get(i), Some(i % 3 == 0));
        }
    }

    #[test]
    fn test_inline_set_return_value() {
        let mut bv = SmolBitVec::new();
        bv.push(true);

        assert_eq!(bv.set(0, false), Some(true));
        assert_eq!(bv.get(0), Some(false));
        assert_eq!(bv.set(0, true), Some(false));
        assert_eq!(bv.get(0), Some(true));

        assert_eq!(bv.set(1, true), None);
    }

    #[test]
    fn test_inline_memory_layout() {
        use std::mem::size_of;

        // On 64-bit systems:
        // len (8 bytes)
        // bits (16 bytes) -> SmolBitVecBits is 16 bytes because it contains a Box<[usize]> (16 bytes).
        //                    Rust uses the Box's pointer niche to fit the Inline(usize) variant.
        // Total = 24 bytes (3 usizes)

        let expected_total_size = size_of::<usize>() * 3;
        let expected_variant_size = size_of::<usize>() * 2;

        assert_eq!(
            size_of::<SmolBitVec>(),
            expected_total_size,
            "SmolBitVec size on stack should be 3 usizes (len + bits)"
        );

        assert_eq!(
            size_of::<SmolBitVecBits>(),
            expected_variant_size,
            "Internal variant enum should be 2 usizes (size of Box<[usize]>)"
        );
    }

    #[test]
    fn test_spillover_exactly_at_boundary() {
        let mut bv = SmolBitVec::new();
        let cap = BITS_PER_WORD;

        // Fill to capacity
        for _ in 0..cap {
            bv.push(true);
        }
        assert!(matches!(bv.bits, SmolBitVecBits::Inline(_)));

        // Push 65th bit (should spill)
        bv.push(false);
        assert_eq!(bv.len(), cap + 1);
        assert!(matches!(bv.bits, SmolBitVecBits::Heap(_)));

        // Verify all bits
        for i in 0..cap {
            assert_eq!(
                bv.get(i),
                Some(true),
                "Bit at index {} was corrupted during spill",
                i
            );
        }
        assert_eq!(bv.get(cap), Some(false));
    }

    #[test]
    fn test_heap_get_set_pop() {
        let mut bv = SmolBitVec::new();
        let size = (usize::BITS + 10) as usize;

        for i in 0..size {
            bv.push(i % 2 == 0);
        }

        // Test get
        for i in 0..size {
            assert_eq!(bv.get(i), Some(i % 2 == 0));
        }

        // Test set
        bv.set(0, false);
        bv.set(size - 1, true);
        assert_eq!(bv.get(0), Some(false));
        assert_eq!(bv.get(size - 1), Some(true));

        // Test pop
        assert_eq!(bv.pop(), Some(true));
        assert_eq!(bv.len(), size - 1);
    }

    #[test]
    fn test_very_large_bit_vec() {
        let mut bv = SmolBitVec::new();
        let size = 1000;

        for i in 0..size {
            bv.push(i % 7 == 0);
        }

        assert_eq!(bv.len(), size);
        for i in 0..size {
            assert_eq!(bv.get(i), Some(i % 7 == 0));
        }
    }

    #[test]
    fn test_from_iterator_and_extend_large() {
        let size = 200;
        let bits: Vec<bool> = (0..size).map(|i| i % 3 == 0).collect();

        let mut bv: SmolBitVec = bits.iter().copied().collect();
        assert_eq!(bv.len(), size);
        assert!(matches!(bv.bits, SmolBitVecBits::Heap(_)));

        let extra: Vec<bool> = (0..size).map(|i| i % 5 == 0).collect();
        bv.extend(extra.iter().copied());

        assert_eq!(bv.len(), size * 2);
        for i in 0..size {
            assert_eq!(bv.get(i), Some(bits[i]));
            assert_eq!(bv.get(i + size), Some(extra[i]));
        }
    }

    #[test]
    fn test_pop_back_to_inline_behavior() {
        let mut bv = SmolBitVec::new();
        let cap = BITS_PER_WORD;

        for _ in 0..cap + 1 {
            bv.push(true);
        }
        assert!(matches!(bv.bits, SmolBitVecBits::Heap(_)));

        bv.pop();

        assert_eq!(bv.len(), cap);

        // CRITICAL UPDATE: Assert that it successfully downsized back to the stack
        assert!(
            matches!(bv.bits, SmolBitVecBits::Inline(_)),
            "Should aggressively transition back to Inline variant to save memory"
        );

        // Verify that the 64 bits survived the structural transition intact
        for i in 0..cap {
            assert_eq!(
                bv.get(i),
                Some(true),
                "Bit {} was corrupted during the transition back to Inline",
                i
            );
        }
    }

    #[test]
    fn test_heap_compactness() {
        let mut bv = SmolBitVec::new();
        let cap = BITS_PER_WORD;
        let num_bits = cap + 1;
        for _ in 0..num_bits {
            bv.push(true);
        }

        match &bv.bits {
            SmolBitVecBits::Heap(items) => {
                // For cap + 1 bits, we should only need 2 usize blocks if packed.
                let expected_blocks = (num_bits + cap - 1) / cap;
                assert_eq!(
                    items.len(),
                    expected_blocks,
                    "Memory Inefficiency: Using {} blocks for {} bits. Expected {} blocks.",
                    items.len(),
                    num_bits,
                    expected_blocks
                );
            }
            _ => panic!("Should be in Heap variant"),
        }
    }

    #[test]
    fn test_equality_consistency() {
        let mut bv_inline = SmolBitVec::new();
        let cap = BITS_PER_WORD;
        for _ in 0..cap {
            bv_inline.push(true);
        }

        let mut bv_heap = SmolBitVec::new();
        for _ in 0..cap + 1 {
            bv_heap.push(true);
        }
        bv_heap.pop(); // Now it has 'cap' bits but is in Heap variant

        assert_eq!(
            bv_inline, bv_heap,
            "Logic Bug: Vectors with identical bits must be equal regardless of storage variant."
        );
    }

    #[test]
    fn test_large_iteration() {
        let size = 1000;
        let bv: SmolBitVec = (0..size).map(|i| i % 2 == 0).collect();
        let collected: Vec<bool> = (&bv).into_iter().collect();
        assert_eq!(collected.len(), size);
        for i in 0..size {
            assert_eq!(collected[i], i % 2 == 0);
        }
    }

    #[test]
    fn test_clone_independence() {
        let mut bv1 = SmolBitVec::new();
        let cap = BITS_PER_WORD;

        // Test Inline Clone
        bv1.push(true);
        let mut bv2 = bv1.clone();
        bv2.set(0, false);
        assert_eq!(bv1.get(0), Some(true));
        assert_eq!(bv2.get(0), Some(false));

        // Test Heap Clone
        for _ in 0..cap {
            bv1.push(false);
        }
        assert!(matches!(bv1.bits, SmolBitVecBits::Heap(_)));
        let mut bv3 = bv1.clone();
        bv3.set(cap, true);
        assert_eq!(bv1.get(cap), Some(false));
        assert_eq!(bv3.get(cap), Some(true));
    }

    #[test]
    fn test_multiple_transitions() {
        let mut bv = SmolBitVec::new();
        let cap = BITS_PER_WORD;

        // Inline -> Heap
        for _ in 0..cap + 1 {
            bv.push(true);
        }
        assert!(matches!(bv.bits, SmolBitVecBits::Heap(_)));

        // Heap -> Inline
        for _ in 0..cap + 1 {
            bv.pop();
        }
        assert!(matches!(bv.bits, SmolBitVecBits::Inline(_)));
        assert!(bv.is_empty());

        // Inline -> Heap again
        for _ in 0..cap + 1 {
            bv.push(false);
        }
        assert!(matches!(bv.bits, SmolBitVecBits::Heap(_)));
    }

    #[test]
    fn test_iterator_exactness() {
        let size = 100;
        let bv: SmolBitVec = (0..size).map(|_| true).collect();
        let mut iter = (&bv).into_iter();

        assert_eq!(iter.len(), size);
        let (min, max) = iter.size_hint();
        assert_eq!(min, size);
        assert_eq!(max, Some(size));

        iter.next();
        assert_eq!(iter.len(), size - 1);
        let (min, max) = iter.size_hint();
        assert_eq!(min, size - 1);
        assert_eq!(max, Some(size - 1));
    }

    #[test]
    fn test_pop_until_empty_from_heap() {
        let mut bv = SmolBitVec::new();
        let size = 200;
        for i in 0..size {
            bv.push(i % 2 == 0);
        }

        for i in (0..size).rev() {
            assert_eq!(bv.pop(), Some(i % 2 == 0));
        }

        assert!(bv.is_empty());
        assert_eq!(bv.len(), 0);
        assert!(matches!(bv.bits, SmolBitVecBits::Inline(0)));
        assert_eq!(bv.pop(), None);
    }

    #[test]
    fn test_bit_persistence_around_boundaries() {
        let mut bv = SmolBitVec::new();
        let cap = BITS_PER_WORD;
        let size = cap * 2;

        for _ in 0..size {
            bv.push(false);
        }

        // Set bit at boundary (e.g., bit 64)
        bv.set(cap, true);
        assert_eq!(bv.get(cap), Some(true));
        assert_eq!(bv.get(cap - 1), Some(false));
        assert_eq!(bv.get(cap + 1), Some(false));

        // Set bit at index 0
        bv.set(0, true);
        assert_eq!(bv.get(0), Some(true));
        assert_eq!(bv.get(1), Some(false));
    }

    #[test]
    fn test_out_of_bounds_explicit() {
        let mut bv = SmolBitVec::new();
        assert_eq!(bv.get(0), None);
        assert_eq!(bv.set(0, true), None);

        bv.push(true);
        assert_eq!(bv.get(1), None);
        assert_eq!(bv.set(1, false), None);

        let cap = BITS_PER_WORD;
        for _ in 0..cap {
            bv.push(true);
        }
        // Now it's Heap
        assert_eq!(bv.get(bv.len()), None);
        assert_eq!(bv.set(bv.len(), true), None);
    }

    #[test]
    fn test_dirty_bits_equality_guard() {
        // This test simulates "dirty" bits beyond the logical length
        // to ensure equality remains robust.
        let mut bv1 = SmolBitVec::new();
        bv1.push(true); // len 1, bit 0 is true

        let mut bv2 = bv1.clone();

        // Manually corrupt bv2's internal state with "garbage" in high bits
        unsafe {
            let ptr = &mut bv2 as *mut SmolBitVec;
            let variant_ptr = &mut (*ptr).bits as *mut SmolBitVecBits;
            if let SmolBitVecBits::Inline(ref mut bits) = *variant_ptr {
                *bits |= 0xAAAA_AAAA_AAAA_AAAA; // Set a bunch of high bits
            }
        }

        // Currently, our Inline PartialEq is `a == b`, so this SHOULD FAIL if bits are dirty.
        // This confirms that our current implementation RELIES on bit cleanliness.
        assert_ne!(
            bv1, bv2,
            "Equality should fail because high bits are dirty and we don't mask in PartialEq"
        );
    }

    #[test]
    fn test_heap_cleanliness() {
        let mut bv = SmolBitVec::new();
        let cap = BITS_PER_WORD;

        // Fill first block, then add one bit to second block
        for _ in 0..cap {
            bv.push(false);
        }
        bv.push(true); // Index cap is true

        // Pop the true bit. The second block (items[1]) should be cleared.
        bv.pop();

        // If we are still in Heap (we shouldn't be, it transitions to Inline at next_len == cap),
        // but let's check the transition logic.
        // In `pop`, if next_len == cap, it does `Inline(items[0])`.
        // If items[0] had any bits set > cap, they would persist.

        assert_eq!(bv.len(), cap);
        assert!(matches!(bv.bits, SmolBitVecBits::Inline(_)));

        // Test Heap cleanliness without triggering Inline transition
        let mut bv_large = SmolBitVec::new();
        // Use 2 full blocks + 1 bit
        for _ in 0..cap * 2 + 1 {
            bv_large.push(true);
        }
        // Bit at index cap*2 (bit 0 of block 2) is true.
        // Block 0: 0..64
        // Block 1: 64..128
        // Block 2: 128 (bit 0 is 1)

        bv_large.pop(); // Pop bit 128. Block 2 should be removed.

        if let SmolBitVecBits::Heap(ref items) = bv_large.bits {
            assert_eq!(
                items.len(),
                2,
                "Should have 2 blocks (0 and 1) after popping the only bit in block 2"
            );
        }

        // Now test clearing a bit WITHIN a block
        bv_large.pop(); // Pop bit 127. Bit 63 of block 1 should be zeroed.
        if let SmolBitVecBits::Heap(ref items) = bv_large.bits {
            let offset_in_block_1 = 63;
            assert_eq!(
                items[1] & (1usize << offset_in_block_1),
                0,
                "The popped bit 127 (offset 63 in block 1) should be zeroed"
            );
        }
    }

    #[test]
    fn test_bulk_extend_integrity() {
        let mut bv = SmolBitVec::new();
        let bits: Vec<bool> = (0..1000).map(|i| i % 3 == 0).collect();
        bv.extend(bits.clone());

        assert_eq!(bv.len(), 1000);
        for i in 0..1000 {
            assert_eq!(bv.get(i), Some(bits[i]));
        }
    }

    #[test]
    fn test_partial_eq_optimized_heap() {
        let mut bv1 = SmolBitVec::new();
        let mut bv2 = SmolBitVec::new();
        let size = (usize::BITS * 2 + 10) as usize;

        for i in 0..size {
            let b = i % 3 == 0;
            bv1.push(b);
            bv2.push(b);
        }

        // Test identical heap vectors
        assert_eq!(bv1, bv2);

        // Test difference in a single bit
        bv2.set(size - 1, !bv2.get(size - 1).unwrap());
        assert_ne!(bv1, bv2);

        // Reset and test again
        bv2.set(size - 1, bv1.get(size - 1).unwrap());
        assert_eq!(bv1, bv2);
    }

    #[test]
    fn test_partial_eq_cleanliness_after_set() {
        let mut bv1 = SmolBitVec::new();
        let mut bv2 = SmolBitVec::new();

        bv1.push(true);
        bv2.push(true);

        // Logical state: [true]
        assert_eq!(bv1, bv2);

        // set(0, true) should be a no-op logically,
        // but we must ensure it doesn't corrupt high bits.
        bv1.set(0, true);
        assert_eq!(bv1, bv2);

        // set(0, false) followed by set(0, true)
        bv1.set(0, false);
        bv1.set(0, true);
        assert_eq!(bv1, bv2);
    }

    #[test]
    fn test_partial_eq_canonical_variants() {
        let mut bv1 = SmolBitVec::new();
        let cap = BITS_PER_WORD;

        for _ in 0..cap {
            bv1.push(true);
        }

        let mut bv2 = SmolBitVec::new();
        for _ in 0..cap + 1 {
            bv2.push(true);
        }
        bv2.pop(); // Should transition back to Inline

        // Both are Inline(cap bits), so they should be equal
        assert_eq!(bv1, bv2);

        // Ensure they are both actually Inline
        assert!(matches!(bv1.bits, SmolBitVecBits::Inline(_)));
        assert!(matches!(bv2.bits, SmolBitVecBits::Inline(_)));
    }

    #[test]
    fn test_empty_state_exhaustive() {
        let bv1 = SmolBitVec::new();
        let bv2 = SmolBitVec::default();

        // Basic properties
        assert!(bv1.is_empty());
        assert_eq!(bv1.len(), 0);

        // Equality
        assert_eq!(bv1, bv2);

        // Iteration
        let mut iter = (&bv1).into_iter();
        assert_eq!(iter.next(), None);
        assert_eq!(iter.len(), 0);

        // Cloning
        let bv3 = bv1.clone();
        assert!(bv3.is_empty());
        assert_eq!(bv1, bv3);

        // Debug format
        assert_eq!(format!("{:?}", bv1), "[]");

        // FromIterator
        let bv4: SmolBitVec = std::iter::empty::<bool>().collect();
        assert!(bv4.is_empty());
        assert_eq!(bv1, bv4);

        // Extend
        let mut bv5 = SmolBitVec::new();
        bv5.extend(std::iter::empty::<bool>());
        assert!(bv5.is_empty());
        assert_eq!(bv1, bv5);
    }

    #[test]
    fn test_clear() {
        let mut bv = SmolBitVec::new();
        bv.push(true);
        bv.push(false);
        bv.push(true);
        assert_eq!(bv.len(), 3);
        bv.clear();
        assert_eq!(bv.len(), 0);
        assert!(bv.is_empty());
        assert_eq!(bv.get(0), None);

        // Test clear on heap
        for _ in 0..BITS_PER_WORD + 1 {
            bv.push(true);
        }
        assert!(matches!(bv.bits, SmolBitVecBits::Heap(_)));
        bv.clear();
        assert_eq!(bv.len(), 0);
        assert!(matches!(bv.bits, SmolBitVecBits::Inline(0)));
    }

    #[test]
    fn test_first_last() {
        let mut bv = SmolBitVec::new();
        assert_eq!(bv.first(), None);
        assert_eq!(bv.last(), None);

        bv.push(true);
        assert_eq!(bv.first(), Some(true));
        assert_eq!(bv.last(), Some(true));

        bv.push(false);
        assert_eq!(bv.first(), Some(true));
        assert_eq!(bv.last(), Some(false));

        // Test heap
        let mut bv2 = SmolBitVec::new();
        for i in 0..BITS_PER_WORD + 1 {
            bv2.push(i == 0); // first is true, others false
        }
        assert_eq!(bv2.first(), Some(true));
        assert_eq!(bv2.last(), Some(false));

        bv2.set(BITS_PER_WORD, true);
        assert_eq!(bv2.last(), Some(true));

        bv2.set(0, false);
        assert_eq!(bv2.first(), Some(false));
    }

    #[test]
    fn test_reserve_basic() {
        let mut bv = SmolBitVec::new();
        // Reserving on an empty vector
        bv.reserve(100);

        for i in 0..100 {
            bv.push(i % 3 == 0);
        }

        assert_eq!(bv.len(), 100);
        for i in 0..100 {
            assert_eq!(bv.get(i), Some(i % 3 == 0));
        }
    }

    #[test]
    fn test_reserve_inline_to_heap() {
        let mut bv = SmolBitVec::new();
        bv.push(true);
        // This should force a transition to heap if additional > capacity
        bv.reserve(BITS_PER_WORD + 1);

        assert_eq!(bv.len(), 1);
        assert_eq!(bv.get(0), Some(true));

        // Ensure further pushes work
        for _ in 0..BITS_PER_WORD {
            bv.push(false);
        }
        assert_eq!(bv.len(), BITS_PER_WORD + 1);
        assert_eq!(bv.get(0), Some(true));
        assert_eq!(bv.get(BITS_PER_WORD), Some(false));
    }

    #[test]
    fn test_reserve_equality_with_different_capacities() {
        let mut bv1 = SmolBitVec::new();
        let mut bv2 = SmolBitVec::new();

        for i in 0..BITS_PER_WORD {
            let b = i % 2 == 0;
            bv1.push(b);
            bv2.push(b);
        }

        // bv2 might transition or reallocate, but logically they are the same
        bv2.reserve(1000);

        assert_eq!(bv1, bv2, "Equality should ignore capacity differences");

        // Even after more pushes
        bv1.push(true);
        bv2.push(true);
        assert_eq!(bv1, bv2);
    }

    // Bug #1a: pop with item_offset==0 removes the wrong block when the heap is
    // over-allocated. The last over-allocated (empty) block is dropped instead of
    // block item_index, leaving a dirty bit that poisons the next get at that position.
    #[test]
    fn test_pop_after_reserve_block_boundary_dirty_bit() {
        let mut bv = SmolBitVec::new();
        let cap = BITS_PER_WORD;

        for _ in 0..cap * 2 + 1 {
            bv.push(true);
        }
        bv.reserve(cap * 4); // items.len() is now larger than item_index (2) + 1

        // last_index=cap*2, item_index=2, item_offset=0
        assert_eq!(bv.pop(), Some(true));
        assert_eq!(bv.len(), cap * 2);

        bv.push(false);
        assert_eq!(
            bv.get(cap * 2),
            Some(false),
            "dirty bit in items[2] caused get to return true after pushing false"
        );
    }

    // Bug #1b: pop with item_offset!=0 clears the bit in the wrong block when the
    // heap is over-allocated. items.last_mut() is used instead of items[item_index],
    // so the dirty bit persists and poisons the next get at that position.
    #[test]
    fn test_pop_after_reserve_mid_block_dirty_bit() {
        let mut bv = SmolBitVec::new();
        let cap = BITS_PER_WORD;

        for _ in 0..cap + 6 {
            bv.push(true);
        }
        bv.reserve(cap * 4); // items.len() is now larger than item_index (1) + 1

        // last_index=cap+5, item_index=1, item_offset=5
        assert_eq!(bv.pop(), Some(true));
        assert_eq!(bv.len(), cap + 5);

        bv.push(false);
        assert_eq!(
            bv.get(cap + 5),
            Some(false),
            "dirty bit in items[1] caused get to return true after pushing false"
        );
    }

    #[test]
    fn test_reserve_does_not_shrink_capacity() {
        let mut bv = SmolBitVec::new();
        let cap = BITS_PER_WORD;

        for _ in 0..cap + 1 {
            bv.push(true);
        }

        bv.reserve(cap * 10);
        let blocks_after_large_reserve = match &bv.bits {
            SmolBitVecBits::Heap(items) => items.len(),
            _ => panic!("expected Heap"),
        };

        bv.reserve(1);
        let blocks_after_small_reserve = match &bv.bits {
            SmolBitVecBits::Heap(items) => items.len(),
            _ => panic!("expected Heap"),
        };

        assert_eq!(
            blocks_after_small_reserve, blocks_after_large_reserve,
            "reserve(1) shrank capacity from {} to {} blocks",
            blocks_after_large_reserve, blocks_after_small_reserve
        );
    }

    #[test]
    fn test_eq_trait_bound() {
        fn requires_eq<T: Eq>(_: &T) {}
        let bv = SmolBitVec::new();
        requires_eq(&bv);
    }
}
