# rpn-engine

**A zero-allocation, `no_std` Reverse Polish Notation evaluator over a caller-provided stack buffer.**

The caller owns the memory. `evaluate` takes a token slice and a scratch stack buffer
(`&mut [i64]`) and never allocates — `O(n)` time, `O(1)` space beyond the two slices you
hand it. Every failure is a named `EvalError`, never a panic; all arithmetic is checked, so
the result is identical on every architecture.

```rust
use rpn_engine::{evaluate, Token::*};

let mut stack = [0i64; 8];
// (2 + 3) * 4
assert_eq!(evaluate(&[Val(2), Val(3), Add, Val(4), Mul], &mut stack), Ok(20));
```

## Why

- **`no_std`, no-alloc.** Runs on bare metal, in interrupt handlers, anywhere. You size the
  stack; the library never touches the heap.
- **`#![forbid(unsafe_code)]`.** Zero unsafe, zero dependencies.
- **No panics.** A too-small buffer, a malformed program, division by zero, and integer
  overflow are all named `EvalError` variants — including the `i64::MIN / -1` edge.
- **Deterministic.** Checked integer arithmetic gives bit-identical results on every target.

## The contract

`evaluate(tokens: &[Token], stack: &mut [i64]) -> Result<i64, EvalError>`

- `Token` — `Val(i64)`, `Add`, `Sub`, `Mul`, `Div`. Operators pop `b`, pop `a`, push `a op b`.
- A program is well-formed when it leaves **exactly one** value on the stack.
- `EvalError` — `StackOverflow` (buffer full), `StackUnderflow` (operator without two
  operands), `DivByZero`, `Overflow` (a checked op overflowed `i64`), and
  `MalformedProgram { remaining }` (not exactly one result).

## Tested exhaustively

Every operator, every stack and buffer boundary, the `a op b` operand order,
truncate-toward-zero division, and every error path — including each integer-overflow
edge and `i64::MIN / -1` — is exercised by the test suite. A green build means the
behavior is pinned, not merely line-covered.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
