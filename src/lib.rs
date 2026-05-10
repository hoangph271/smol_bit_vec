#[derive(Debug, Clone, PartialEq, Eq)]
enum SmolBitVecVariant {
    Inline(usize),
}

#[derive(Clone, PartialEq, Eq)]
pub struct SmolBitVec {
    len: usize,
    bits: SmolBitVecVariant,
}

impl SmolBitVec {
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, value: bool) {
        if value {
            match self.bits {
                SmolBitVecVariant::Inline(bits) => {
                    let mask = 1usize << self.len();

                    self.bits = SmolBitVecVariant::Inline(bits | mask);
                }
            }
        }

        self.len += 1;
    }

    pub fn get(&self, index: usize) -> Option<bool> {
        if self.is_empty() {
            return None;
        }

        match self.bits {
            SmolBitVecVariant::Inline(bits) => {
                if index >= self.len() {
                    return None;
                }

                let mask = 1usize << index;

                Some(bits & mask != 0)
            }
        }
    }

    pub fn set(&mut self, index: usize, value: bool) -> Option<bool> {
        if index >= self.len() {
            return None;
        }

        let old_value = self.get(index);

        match self.bits {
            SmolBitVecVariant::Inline(ref mut bits) => {
                let mask = 1usize << index;

                if value {
                    // Toggle the bit ON if it was OFF, leave it on otherwise
                    *bits |= mask
                } else {
                    // Invert the mask, making every bits ON except at index
                    // &= will turn the bit at index in bits to OFF
                    *bits &= !mask
                }
            }
        }

        old_value
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn pop(&mut self) -> Option<bool> {
        if self.is_empty() {
            return None;
        }

        let last_index = self.len() - 1;
        let value = self.get(self.len() - 1);

        match self.bits {
            SmolBitVecVariant::Inline(ref mut bits) => {
                // All bits are ON except for the target bit, which is OFF after !
                let mask = !(1usize << last_index);

                *bits &= mask;
            }
        }

        self.len -= 1;

        value
    }
}

impl Default for SmolBitVec {
    fn default() -> Self {
        Self {
            len: 0,
            bits: SmolBitVecVariant::Inline(0),
        }
    }
}

impl std::fmt::Debug for SmolBitVec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmolBitVec").finish()
    }
}

impl FromIterator<bool> for SmolBitVec {
    fn from_iter<T: IntoIterator<Item = bool>>(iter: T) -> Self {
        todo!()
    }
}

pub struct SmolBitVecIter<'a> {
    vec: &'a SmolBitVec,
}

impl<'a> Iterator for SmolBitVecIter<'a> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

impl<'a> IntoIterator for &'a SmolBitVec {
    type Item = bool;
    type IntoIter = SmolBitVecIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        todo!()
    }
}

impl Extend<bool> for SmolBitVec {
    fn extend<T: IntoIterator<Item = bool>>(&mut self, _iter: T) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_empty() {
        let bv = SmolBitVec::new();
        assert!(bv.is_empty());
        assert_eq!(bv.len(), 0);
    }

    #[test]
    fn test_push_pop_inline() {
        let mut bv = SmolBitVec::new();
        let limit = usize::BITS as usize;

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
        let limit = usize::BITS as usize;

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
        bv.set(usize::BITS as usize, true);
        bv.set(large_size - 1, true);

        assert_eq!(bv.get(0), Some(true));
        assert_eq!(bv.get(1), Some(false));
        assert_eq!(bv.get(usize::BITS as usize), Some(true));
        assert_eq!(bv.get(usize::BITS as usize + 1), Some(false));
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
        let cap = usize::BITS as usize;

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
        // bits (8 bytes) -> SmolBitVecVariant is optimized to 8 bytes because it has only one variant
        // Total = 16 bytes (2 usizes)

        // When we add another variant later, this will likely jump to 3 usizes (24 bytes)
        // to accommodate the enum tag.

        let expected_total_size = size_of::<usize>() * 2;
        let expected_variant_size = size_of::<usize>();

        assert_eq!(
            size_of::<SmolBitVec>(),
            expected_total_size,
            "SmolBitVec size on stack should be 2 usizes (optimized single-variant enum)"
        );

        assert_eq!(
            size_of::<SmolBitVecVariant>(),
            expected_variant_size,
            "Internal variant enum should be 1 usize (optimized)"
        );
    }
}
