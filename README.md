# SmolBitVec

A tiny, bit-packed bit vector with Small Vector Optimization (SVO).

## Why this exists?

This crate was created primarily as a **learning exercise** to understand bitwise manipulation, memory layout optimizations in Rust, and the trade-offs of Small Vector Optimization.

In most real-world scenarios, you should probably use `Vec<bool>` (for simplicity) or the `bitvec` crate (for features). This implementation makes sense only in **extreme cases** where you are managing millions of bit vectors that are usually small (≤ 64 bits) and you need to minimize stack usage and heap overhead at all costs.

### Inspiration
This project was inspired by the following deep dives:
* [Why Windows Gets Slower Over Time (And Why Linux Doesn't)](https://www.youtube.com/watch?v=4I6Gk8TazC4)
* [A vector of bools is a vector of bools... right? | C++ Deep Dive](https://www.youtube.com/watch?v=k_lOw_hwkM0)

---

## Pros & Cons

### Pros
* **Tiny Stack Footprint:** Only **24 bytes** on the stack (on 64-bit systems).
* **Zero Heap for Small Sets:** No heap allocation is performed if the length is ≤ 64 bits.
* **Extreme Memory Density:** Strictly uses 1 bit per boolean, even when spilled to the heap.
* **Niche Optimization:** Utilizes Rust's enum niche optimization to keep the internal variant representation compact.

### Cons
* **Slow Growth/Shrinkage:** To avoid storing a `capacity` field (saving 8 bytes of stack space), we use `Box<[usize]>`. This means every 64th `push` or `pop` (at a block boundary) triggers an **O(N) reallocation** instead of the amortized O(1) growth of a `Vec`. Use `reserve()` to pre-allocate and amortize this cost when the final size is known ahead of time.
* **Complexity:** Bitwise indexing is significantly more complex and error-prone than standard indexing.
* **Limited API:** Focused on core operations; does not support advanced bit-tracking features found in more mature crates.

---

## Project Goals

### Features
* **Small Vector Optimization (SVO):** Stack-allocated inline storage up to `usize::BITS`.
* **Bit-Packed Heap Spillover:** Seamless transition to a heap-allocated backing store when capacity is exceeded.
* **Safe API Design:** Strict enforcement of memory safety without `unsafe` blocks in the public API.

### Non-Features
* **No `IndexMut` Implementation:** Will not implement `std::ops::IndexMut`.
* **No Proxy Objects:** Explicitly rejects the C++ `std::vector<bool>` proxy-object paradigm.
* **No Bit-Level Concurrency:** Does not support concurrent mutable access to distinct bits within the same block.

---

## API Surface

### Core Functions
* `pub fn new() -> Self`
* `pub fn len(&self) -> usize`
* `pub fn capacity(&self) -> usize`
* `pub fn is_empty(&self) -> bool`
* `pub fn push(&mut self, val: bool)`
* `pub fn pop(&mut self) -> Option<bool>`
* `pub fn get(&self, index: usize) -> Option<bool>`
* `pub fn set(&mut self, index: usize, val: bool) -> Option<bool>`
* `pub fn first(&self) -> Option<bool>`
* `pub fn last(&self) -> Option<bool>`
* `pub fn clear(&mut self)`
* `pub fn reserve(&mut self, additional: usize)`

### Traits
* `std::clone::Clone`
* `std::cmp::PartialEq` / `std::cmp::Eq`
* `std::default::Default`
* `std::iter::Extend<bool>`
* `std::iter::FromIterator<bool>`
* `std::iter::IntoIterator` (for `SmolBitVec` and `&SmolBitVec`)
* `std::fmt::Debug`

---

## Testing Strategy
The test suite rigorously verifies:
* **Inline Bitwise Logic:** Correct shifting and masking for small vectors.
* **Heap Spillover Boundary:** Exact state transitions at the 64-bit limit.
* **Data Integrity:** Accurate addressing across multiple `usize` blocks on the heap.
* **Memory Efficiency:** Constant stack footprint and 1-bit-per-bool packing.
* **Equality Correctness:** `PartialEq` masks bits beyond the logical length in all four variant combinations (Inline/Heap × Inline/Heap).
* **Reserve Semantics:** Pre-allocation does not corrupt `extend`, `push`, or `pop`; overflow panics in all build profiles.
