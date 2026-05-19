use std::{iter::FusedIterator, mem::replace};

#[derive(Debug, Clone, PartialEq, Eq)]
enum SmolBitVecBits {
    Inline(usize),
    Heap(Box<[usize]>),
}

impl SmolBitVecBits {
    /// Returns a slice view of the underlying words.
    ///
    /// Note: The returned slice may contain "dirty bits" (data beyond the logical length
    /// of the bit vector) and unused trailing words if the capacity exceeds the length.
    fn as_slice(&self) -> &[usize] {
        match self {
            SmolBitVecBits::Inline(bits) => std::slice::from_ref(bits),
            SmolBitVecBits::Heap(chunks) => chunks,
        }
    }
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

        let a_used_bits = self.bits.as_slice();
        let b_used_bits = other.bits.as_slice();

        let full_chunks = self.len() / BITS_PER_WORD;

        if a_used_bits[..full_chunks] != b_used_bits[..full_chunks] {
            return false;
        }

        let remainder = self.len() % BITS_PER_WORD;
        if remainder > 0 {
            let mask = (1usize << remainder) - 1;

            if a_used_bits[full_chunks] & mask != b_used_bits[full_chunks] & mask {
                return false;
            }
        }

        true
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
                    // At spillover, len == BITS_PER_WORD, so the new bit (value)
                    // is always at offset 0 of the second block.
                    self.bits = SmolBitVecBits::Heap(Box::new([*bits, value as usize]));
                }
            }
            SmolBitVecBits::Heap(bits_chunks) => {
                let reserved_len = bits_chunks.len() * BITS_PER_WORD;
                let needs_new_chunk = next_len > reserved_len;

                if needs_new_chunk {
                    *bits_chunks = bits_chunks
                        .iter()
                        .copied()
                        .chain([if value { 1usize } else { 0usize }])
                        .collect::<Vec<usize>>()
                        .into_boxed_slice();
                } else if value {
                    let chunk_offset = len % BITS_PER_WORD;
                    let chunk_index = len / BITS_PER_WORD;
                    let mask = 1usize << chunk_offset;

                    bits_chunks[chunk_index] |= mask;
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

    pub fn capacity(&self) -> usize {
        match &self.bits {
            SmolBitVecBits::Inline(_) => BITS_PER_WORD,
            SmolBitVecBits::Heap(items) => items.len() * BITS_PER_WORD,
        }
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
                    // Transition back to Inline. We use items[0] directly without
                    // clearing the popped bit because when transitioning from Heap
                    // (where len was BITS_PER_WORD + 1), the popped bit was
                    // at index 0 of items[1] or beyond.
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

    /// Reserves capacity for at least `additional` more bits.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() + additional` overflows `usize`.
    pub fn reserve(&mut self, additional: usize) {
        let next_len = self.len.checked_add(additional).unwrap_or_else(|| {
            panic!(
                "Capacity overflow, the len is already {} and tried to add {}",
                self.len, additional
            )
        });

        match &mut self.bits {
            SmolBitVecBits::Inline(inline_bits) => {
                if is_inlineable_len(next_len) {
                    return;
                }

                let next_bits_array_len = next_len.div_ceil(BITS_PER_WORD);
                let mut bits_vec = Vec::with_capacity(next_bits_array_len);

                bits_vec.push(*inline_bits);
                bits_vec.resize(next_bits_array_len, 0);

                self.bits = SmolBitVecBits::Heap(bits_vec.into_boxed_slice());
            }
            SmolBitVecBits::Heap(items) => {
                let next_bits_array_len = next_len.div_ceil(BITS_PER_WORD);

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
        let iter = iter.into_iter();

        let (lower_bound, _) = iter.size_hint();
        let expected_bits_chunks = lower_bound.div_ceil(BITS_PER_WORD);
        let mut bits_chunks = Vec::with_capacity(expected_bits_chunks);

        let mut current_bits_chunk = 0usize;
        let mut bit_offset = 0;
        let mut total_len = 0;

        for bit in iter {
            if bit_offset == BITS_PER_WORD {
                bits_chunks.push(current_bits_chunk);
                current_bits_chunk = 0;
                bit_offset = 0;
            }

            if bit {
                current_bits_chunk |= 1 << bit_offset;
            }

            bit_offset += 1;
            total_len += 1;
        }

        let mut smol_bit_vec = Self::new();

        if total_len <= BITS_PER_WORD {
            smol_bit_vec.bits = SmolBitVecBits::Inline(current_bits_chunk);
            smol_bit_vec.len = total_len;
        } else {
            if bit_offset != 0 {
                bits_chunks.push(current_bits_chunk);
            }
            smol_bit_vec.bits = SmolBitVecBits::Heap(bits_chunks.into_boxed_slice());
            smol_bit_vec.len = total_len;
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
        let mut iter = iter.into_iter();

        if let SmolBitVecBits::Inline(ref mut bits) = self.bits {
            for item in iter.by_ref() {
                if self.len == BITS_PER_WORD {
                    self.bits = SmolBitVecBits::Heap(Box::new([*bits, if item { 1 } else { 0 }]));

                    self.len += 1;

                    break;
                } else {
                    let bit_offset = self.len % BITS_PER_WORD;

                    if item {
                        *bits |= 1usize << bit_offset;
                    };

                    self.len += 1;
                }
            }
        }

        if let SmolBitVecBits::Heap(ref mut bits_chunk) = self.bits {
            let mut bits_chunks_vec = Vec::from(replace(bits_chunk, Box::new([])));

            for item in iter {
                let len = self.len;
                let chunk_index = len / BITS_PER_WORD;
                let bit_offset = len % BITS_PER_WORD;

                if bit_offset == 0 && chunk_index == bits_chunks_vec.len() {
                    bits_chunks_vec.push(if item { 1 } else { 0 });
                } else if item {
                    bits_chunks_vec[chunk_index] |= 1usize << bit_offset;
                }

                self.len += 1;
            }

            *bits_chunk = bits_chunks_vec.into_boxed_slice();
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

        assert_eq!(
            bv1, bv2,
            "PartialEq should mask dirty high bits and treat vectors as equal"
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

    #[test]
    fn test_extend_after_reserve_does_not_corrupt() {
        let mut bv = SmolBitVec::new();
        bv.reserve(BITS_PER_WORD * 4); // over-allocate: 4 zeroed blocks

        let bits: Vec<bool> = (0..BITS_PER_WORD * 2 + 5).map(|i| i % 3 == 0).collect();
        bv.extend(bits.iter().copied());

        assert_eq!(bv.len(), bits.len());
        for (i, &b) in bits.iter().enumerate() {
            assert_eq!(
                bv.get(i),
                Some(b),
                "bit {i} corrupted: extend on over-allocated heap wrote to the wrong block"
            );
        }
    }

    // Requesting capacity that overflows usize is a programmer error and must panic.
    #[test]
    #[should_panic]
    fn test_reserve_panics_on_overflow() {
        let mut bv = SmolBitVec::new();
        bv.push(true);
        bv.reserve(usize::MAX); // len(1) + usize::MAX overflows
    }

    #[test]
    fn test_eq_masks_inline_vs_inline_dirty_bits() {
        let mut bv1 = SmolBitVec::new();
        bv1.push(true); // len=1, only bit 0 is meaningful

        let mut bv2 = bv1.clone();
        unsafe {
            let ptr = &mut bv2 as *mut SmolBitVec;
            if let SmolBitVecBits::Inline(ref mut bits) = (*ptr).bits {
                *bits |= !1usize; // dirty all high bits
            }
        }

        assert_eq!(
            bv1, bv2,
            "Inline vs Inline: dirty high bits should be ignored"
        );
    }

    #[test]
    fn test_eq_masks_heap_vs_heap_dirty_last_block() {
        // Push BITS_PER_WORD + 1 bits so we have 2 blocks; the second block has 1 used bit.
        // Dirty a high bit in the second block (beyond logical length) — currently leaks through.
        let bv1: SmolBitVec = (0..BITS_PER_WORD + 1).map(|i| i % 2 == 0).collect();
        let mut bv2 = bv1.clone();

        unsafe {
            let ptr = &mut bv2 as *mut SmolBitVec;
            if let SmolBitVecBits::Heap(ref mut items) = (*ptr).bits {
                items[1] |= 0xFFFF_FFFF_FFFF_FFFE; // dirty bits 1..63 of last block
            }
        }

        assert_eq!(
            bv1, bv2,
            "Heap vs Heap: dirty bits in last partial block should be ignored"
        );
    }

    #[test]
    fn test_eq_masks_inline_vs_heap_dirty_bits() {
        // Use reserve to force one vec into Heap while keeping len <= BITS_PER_WORD.
        let mut bv_inline = SmolBitVec::new();
        bv_inline.push(true);
        bv_inline.push(false);

        let mut bv_heap = bv_inline.clone();
        bv_heap.reserve(BITS_PER_WORD * 3); // now Heap with len=2

        unsafe {
            let ptr = &mut bv_heap as *mut SmolBitVec;
            if let SmolBitVecBits::Heap(ref mut items) = (*ptr).bits {
                items[0] |= 0xFFFF_FFFF_FFFF_FFFC; // dirty bits 2..63
            }
        }

        assert_eq!(
            bv_inline, bv_heap,
            "Inline vs Heap: dirty high bits in heap block[0] should be ignored"
        );
        assert_eq!(
            bv_heap, bv_inline,
            "Heap vs Inline: dirty high bits in heap block[0] should be ignored"
        );
    }

    #[test]
    fn test_capacity() {
        // Inline: capacity is always BITS_PER_WORD regardless of len
        let mut bv = SmolBitVec::new();
        assert_eq!(bv.capacity(), BITS_PER_WORD);

        bv.push(true);
        assert_eq!(bv.capacity(), BITS_PER_WORD);

        // Filling inline to the brim does not change capacity
        for _ in 1..BITS_PER_WORD {
            bv.push(false);
        }
        assert_eq!(bv.len(), BITS_PER_WORD);
        assert_eq!(bv.capacity(), BITS_PER_WORD);

        // Spillover to Heap: capacity grows to the next block boundary
        bv.push(true);
        assert!(matches!(bv.bits, SmolBitVecBits::Heap(_)));
        assert_eq!(bv.capacity(), BITS_PER_WORD * 2);

        // reserve increases capacity without changing len
        let mut bv2 = SmolBitVec::new();
        bv2.push(true);
        bv2.reserve(BITS_PER_WORD * 3);
        assert_eq!(bv2.len(), 1);
        assert!(bv2.capacity() >= BITS_PER_WORD * 3 + 1);

        // capacity never drops below len
        bv2.reserve(1);
        assert!(bv2.capacity() >= bv2.len());
    }

    #[test]
    fn test_bit_isolation_and_equality() {
        let mut bv1 = SmolBitVec::new();
        bv1.push(true); // len = 1, bits = ...0001

        let mut bv2 = bv1.clone();

        // Manually corrupt bv2 with "dirty" bits beyond the logical length.
        unsafe {
            let ptr = &mut bv2 as *mut SmolBitVec;
            match &mut (*ptr).bits {
                SmolBitVecBits::Inline(bits) => *bits |= 0b1111_1110,
                _ => unreachable!(),
            }
        }

        // This should pass if PartialEq uses proper masking
        assert_eq!(
            bv1, bv2,
            "PartialEq must ignore bits beyond the logical length"
        );
    }

    #[test]
    fn test_consuming_iterator() {
        let bits = vec![true, false, true];
        let bv: SmolBitVec = bits.iter().copied().collect();

        let collected: Vec<bool> = bv.into_iter().collect();
        assert_eq!(collected, bits);
    }
}
