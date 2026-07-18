# rpn-engine

**A zero-allocation, `no_std`, `const`-evaluable Reverse Polish Notation evaluator over a
caller-provided stack buffer.**

The caller owns the memory. `evaluate` takes a token slice and a scratch stack buffer
(`&mut [i64]`) and never allocates — `O(n)` time, `O(1)` space beyond the two slices you
hand it. It is a `const fn`, so an expression can be folded away entirely at compile time.
Every failure is a named `EvalError` carrying the token index where it occurred, never a
panic; all arithmetic is checked, so the result is identical on every architecture.

```rust
use rpn_engine::{evaluate, Token::*};

let mut stack = [0i64; 8];
// (2 + 3) * 4
assert_eq!(evaluate(&[Val(2), Val(3), Add, Val(4), Mul], &mut stack), Ok(20));
```

## Why

- **`no_std`, no-alloc.** Runs on bare metal, in interrupt handlers, anywhere. You size the
  stack; the library never touches the heap. Zero runtime dependencies.
- **`const fn`.** Evaluate expressions at compile time for zero runtime cost — the ultimate
  proof the crate is pure:
  ```rust
  use rpn_engine::{evaluate, Token::*};
  const AREA: i64 = {
      let mut stack = [0i64; 4];
      match evaluate(&[Val(7), Val(6), Mul], &mut stack) {
          Ok(v) => v,
          Err(_) => 0,
      }
  };
  assert_eq!(AREA, 42); // computed by the compiler
  ```
- **`#![forbid(unsafe_code)]`.** Zero unsafe.
- **No panics, ever.** A too-small buffer, a malformed program, division by zero, and
  integer overflow (including `i64::MIN / -1`) are all named `EvalError` variants.
- **Diagnostic, not a black box.** Every error carries the token index at which it was
  detected — `StackUnderflow at index 12`, not just `StackUnderflow`.
- **Deterministic.** Checked integer arithmetic gives bit-identical results on every target.

## The contract

`const fn evaluate(tokens: &[Token], stack: &mut [i64]) -> Result<i64, EvalError>`

- `Token` — `Val(i64)`, `Add`, `Sub`, `Mul`, `Div`. Operators pop `b`, pop `a`, push `a op b`.
- A program is well-formed when it leaves **exactly one** value on the stack.
- `EvalError { kind, at }` — `kind` is one of `StackOverflow`, `StackUnderflow`, `DivByZero`,
  `Overflow`, or `MalformedProgram { remaining }`; `at` is the token index where it was found.

## Performance

Measured on a 256-token program (criterion, `cargo bench`). The zero-allocation evaluator
matches a heap-`Vec` evaluator's throughput while allocating nothing, and is roughly **39×
faster** than the classic parse-to-tree recursive approach:

| Evaluator | Median | Allocates | `no_std` |
| --- | --- | --- | --- |
| `evaluate` (this crate) | **~0.43 µs** | never | ✅ |
| `Vec`-backed iterative | ~0.43 µs | heap stack | ❌ |
| recursive expression tree | ~16.8 µs | heap tree + call stack | ❌ |

Run them yourself with `cargo bench`.

## Tested exhaustively

Every operator, every stack and buffer boundary, the `a op b` operand order,
truncate-toward-zero division, every error path (including each integer-overflow edge and
`i64::MIN / -1`), the reported error index, and the compile-time `const` path are exercised
by the test suite. A green build means the behavior is pinned, not merely line-covered.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
