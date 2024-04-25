# Swift Check

Swift Check is a high-performance library designed for searching and validating data via expressive conditions.

### Supported Acceleration

- x86_64  (SSE2 and if available SSE4.1)
- aarch64 (NEON)
- WASM    (simd128) (currently requires the `experimental` feature)

### Installation

Add Swift Check to your project by including it in your Cargo.toml:

```toml
[dependencies]
swift-check = "0.1.4"
```

### Quick Start

Using this library is fairly straight forward, `swift-check` exposes various conditions that can be composed with each 
other to create complex and performant searches / validators.

Search for the first comma:
```rust
use swift_check::{search, eq};

fn main() {
    let input = b"hello, world!";
  
    let Some(first_comma) = search(input, eq(b',')) else {
        unreachable!("There's a comma!")
    };

    assert_eq!(input[first_comma], b',');
}
```

Ensure every byte is a letter, number, or space:
```rust
use swift_check::{for_all_ensure, any, range, eq};

fn main() {
    let input = b"Hello World 12345";
  
    let res = for_all_ensure(input, any!(
        range!(b'A'..=b'Z'), range!(b'a'..=b'z'),
        range!(b'0'..=b'9'), eq(b' ')
    ));
    assert!(res);
}
```

### Minimum Supported Rust Version

This crate's minimum supported `rustc` version is `1.61.0`

### Testing

To compensate for the great deal of `unsafe` this crate leverages rigorous testing methodologies:

- **Design by Contract**: While not formal verification, it brings more confidence than simple property testing. In 
  critical areas where the most things can go wrong (`arch/simd_scan.rs`) each function is annotated with pre- and 
  post-conditions which are statically verified.
- **Property Testing**: To ensure this crate behaves the same on all supported architectures there is a suite of 
  property tests targeting the slight nuances between them.
- **Fuzz Testing**: LibFuzzer is used with runtime contract assertions to ensure no undefined behavior takes place.
- **Unit Testing**: For basic sanity checks

### Contributing

We warmly welcome contributions from the community! If you're interested in helping Swift Check grow and improve, here 
are the areas we're currently focusing on:

- **Expanding the Test Suite**: Ensuring robustness through comprehensive tests is vital. We're looking to expand our 
  test suite to cover more edge cases and usage scenarios.
- **Optimizing the Fallback Implementation**: We aim to optimize our fallback mechanisms to ensure that Swift Check 
  remains efficient even when SIMD is not available / supported. 
- **Optimization**: Once we've solidified our foundational code, we plan to refine our search and validation procedures.

### Note

This library is in its early stages, having started as an exploratory project. We've been delighted to find that our
"what if" scenario is not only possible but promising. Currently, our primary focus is on ensuring that the library
functions correctly, while recognizing that there are several opportunities for optimization in the scanning process.

In our performance evaluations, we discovered that the use of higher-order functions do not adversely affect
performance. We experimented with various implementations, including one where variadic macros created structs to
maintain SIMD registers throughout the search process. This approach performed comparably to our higher-order 
implementation. The main advantage of the struct-based approach is its ability to selectively utilize SIMD; for example,
it enables us to perform simple searches over smaller inputs instead of relying on partial SIMD loads. However, the
trade-off is complexity: adopting this method could require shifting to procedural macros or requiring users to provide
identifiers for each condition in our declarative macros.