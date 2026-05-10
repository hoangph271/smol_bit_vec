# SmolBitVec

## Project Goals

### Features

* **Small Vector Optimization (SVO):** Stack-allocated inline storage up to `usize::BITS`. Zero heap allocations for vectors under the machine word limit (64 bits on x86_64/aarch64, 32 bits on x86/arm32).
* **Bit-Packed Heap Spillover:** Seamless transition to a heap-allocated `Vec<usize>` backing store when capacity is exceeded, strictly maintaining a 1-bit-per-boolean memory footprint.
* **Safe API Design:** Strict enforcement of memory safety and standard Rust aliasing rules without unsafe blocks.

### Non-Features

* **No `IndexMut` Implementation:** Will not implement `std::ops::IndexMut`.
* **No Proxy Objects:** Explicitly rejects the C++ `std::vector<bool>` proxy-object paradigm to prevent dangling references and undefined behavior.
* **No Bit-Level Concurrency:** Does not support concurrent mutable access to distinct bits within the same byte/word to prevent data races.

---

## API Surface

### Core Functions

Implement the following methods using explicit bitwise operations:

* `pub fn new() -> Self`
* `pub fn len(&self) -> usize`
* `pub fn is_empty(&self) -> bool`
* `pub fn push(&mut self, val: bool)`
* `pub fn pop(&mut self) -> Option<bool>`
* `pub fn get(&self, index: usize) -> Option<bool>`
* `pub fn set(&mut self, index: usize, val: bool)`

### Traits

Implement the following standard traits to ensure idiomatic interoperability:

* `std::default::Default`
* `std::iter::Extend<bool>`
* `std::iter::FromIterator<bool>`
* `std::iter::IntoIterator` (Implement for both `SmolBitVec` and `&SmolBitVec`)
* `std::fmt::Debug` (Output logical boolean states, not the raw binary of the backing `usize` blocks)

---

## Testing Strategy

The test suite must rigorously verify the following conditions:

* **Inline Bitwise Logic:** Validate `push`, `pop`, `get`, and `set` operations while `len < usize::BITS` to ensure correct bit shifting and masking.
* **Heap Spillover Boundary:** Assert correct state transition and memory allocation at exactly `usize::BITS`, `usize::BITS - 1`, and `usize::BITS + 1`.
* **Data Integrity Across Blocks:** Verify `get` and `set` accurately address bits spanning multiple `usize` blocks in the spilled state.
* **Trait Contract Verification:** Assert `FromIterator` and `Extend` correctly construct and mutate the vector, matching the exact behavior of `Vec<bool>`.
*
