#![no_std]
#![forbid(unsafe_code)]
//! A zero-allocation, `no_std`, **`const`-evaluable** Reverse Polish Notation evaluator.
//!
//! The caller owns the memory. [`evaluate`] takes a token slice and a scratch stack
//! buffer (`&mut [i64]`) and never allocates: it runs in `O(n)` time and `O(1)` space
//! beyond the two slices you hand it. It is a `const fn`, so an expression can be folded
//! away entirely at compile time — zero runtime cost. Every failure is a named
//! [`EvalError`] carrying the token index where it was detected, never a panic; all
//! arithmetic is checked, so the result is identical on every architecture.
//!
//! ## Runtime
//!
//! ```
//! use rpn_engine::{evaluate, Token::*};
//! let mut stack = [0i64; 8];
//! // (2 + 3) * 4
//! assert_eq!(evaluate(&[Val(2), Val(3), Add, Val(4), Mul], &mut stack), Ok(20));
//! ```
//!
//! ## Compile time (zero runtime cost)
//!
//! ```
//! use rpn_engine::{evaluate, Token::*};
//! const AREA: i64 = {
//!     let mut stack = [0i64; 4];
//!     match evaluate(&[Val(7), Val(6), Mul], &mut stack) {
//!         Ok(v) => v,
//!         Err(_) => 0,
//!     }
//! };
//! assert_eq!(AREA, 42); // computed by the compiler, not at runtime
//! ```
//!
//! ## Diagnostics
//!
//! ```
//! use rpn_engine::{evaluate, EvalError, ErrorKind, Token::*};
//! let mut stack = [0i64; 8];
//! // an operator with only one operand, at token index 1
//! assert_eq!(
//!     evaluate(&[Val(5), Add], &mut stack),
//!     Err(EvalError { kind: ErrorKind::StackUnderflow, at: 1 })
//! );
//! ```

/// A single Reverse Polish Notation token: a literal value or a binary operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    /// Push this literal onto the stack.
    Val(i64),
    /// Pop `b`, pop `a`, push `a + b`.
    Add,
    /// Pop `b`, pop `a`, push `a - b`.
    Sub,
    /// Pop `b`, pop `a`, push `a * b`.
    Mul,
    /// Pop `b`, pop `a`, push `a / b` (truncating toward zero).
    Div,
}

/// What went wrong during evaluation, independent of where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// A `Val` push had no room left in the caller's stack buffer.
    StackOverflow,
    /// An operator was applied with fewer than two operands on the stack.
    StackUnderflow,
    /// A `Div` operator's divisor was zero.
    DivByZero,
    /// A checked arithmetic operation overflowed `i64` (includes `i64::MIN / -1`).
    Overflow,
    /// Evaluation finished without exactly one value on the stack.
    MalformedProgram {
        /// How many values were left on the stack (0 = no result, >1 = leftover operands).
        remaining: usize,
    },
}

/// A failure and the token index at which it was detected.
///
/// For [`ErrorKind::MalformedProgram`], `at` is `tokens.len()` — the position just past
/// the final token, where the stack was found not to hold exactly one value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvalError {
    /// What went wrong.
    pub kind: ErrorKind,
    /// The zero-based token index where the failure was detected.
    pub at: usize,
}

/// Evaluate `tokens`, using `stack` as scratch space, and return the single result.
///
/// A `const fn`: call it in a `const`/`static` initializer and the whole computation is
/// performed by the compiler. The program is well-formed when it leaves exactly one value
/// on the stack. Runs in `O(tokens.len())` time and allocates nothing.
pub const fn evaluate(tokens: &[Token], stack: &mut [i64]) -> Result<i64, EvalError> {
    let mut top: usize = 0;
    let mut i: usize = 0;
    while i < tokens.len() {
        match tokens[i] {
            Token::Val(v) => {
                if top == stack.len() {
                    return Err(EvalError {
                        kind: ErrorKind::StackOverflow,
                        at: i,
                    });
                }
                stack[top] = v;
                top += 1;
            }
            Token::Add => {
                let (a, b) = match pop2(stack, top) {
                    Some(p) => p,
                    None => return underflow(i),
                };
                let r = match a.checked_add(b) {
                    Some(x) => x,
                    None => return overflow(i),
                };
                top = settle(stack, top, r);
            }
            Token::Sub => {
                let (a, b) = match pop2(stack, top) {
                    Some(p) => p,
                    None => return underflow(i),
                };
                let r = match a.checked_sub(b) {
                    Some(x) => x,
                    None => return overflow(i),
                };
                top = settle(stack, top, r);
            }
            Token::Mul => {
                let (a, b) = match pop2(stack, top) {
                    Some(p) => p,
                    None => return underflow(i),
                };
                let r = match a.checked_mul(b) {
                    Some(x) => x,
                    None => return overflow(i),
                };
                top = settle(stack, top, r);
            }
            Token::Div => {
                let (a, b) = match pop2(stack, top) {
                    Some(p) => p,
                    None => return underflow(i),
                };
                if b == 0 {
                    return Err(EvalError {
                        kind: ErrorKind::DivByZero,
                        at: i,
                    });
                }
                let r = match a.checked_div(b) {
                    Some(x) => x,
                    None => return overflow(i),
                };
                top = settle(stack, top, r);
            }
        }
        i += 1;
    }
    if top != 1 {
        return Err(EvalError {
            kind: ErrorKind::MalformedProgram { remaining: top },
            at: tokens.len(),
        });
    }
    Ok(stack[0])
}

/// The top two stack values as `(a, b)` — `a` the deeper operand, `b` the shallower,
/// matching RPN order (`a op b`) — or `None` when fewer than two are present.
const fn pop2(stack: &[i64], top: usize) -> Option<(i64, i64)> {
    if top < 2 {
        return None;
    }
    Some((stack[top - 2], stack[top - 1]))
}

/// Write a binary-operator result into the deeper of its two operand slots and return the
/// new top index (one smaller than before).
const fn settle(stack: &mut [i64], top: usize, r: i64) -> usize {
    stack[top - 2] = r;
    top - 1
}

/// A `StackUnderflow` at token index `at`.
const fn underflow(at: usize) -> Result<i64, EvalError> {
    Err(EvalError {
        kind: ErrorKind::StackUnderflow,
        at,
    })
}

/// An `Overflow` at token index `at`.
const fn overflow(at: usize) -> Result<i64, EvalError> {
    Err(EvalError {
        kind: ErrorKind::Overflow,
        at,
    })
}

#[cfg(test)]
mod tests {
    use super::Token::*;
    use super::*;

    fn eval(tokens: &[Token]) -> Result<i64, EvalError> {
        let mut stack = [0i64; 16];
        evaluate(tokens, &mut stack)
    }

    fn err(kind: ErrorKind, at: usize) -> Result<i64, EvalError> {
        Err(EvalError { kind, at })
    }

    #[test]
    fn a_lone_value_is_its_own_result() {
        assert_eq!(eval(&[Val(42)]), Ok(42));
        assert_eq!(eval(&[Val(-7)]), Ok(-7));
    }

    #[test]
    fn the_four_operators_compute_correctly() {
        assert_eq!(eval(&[Val(3), Val(4), Add]), Ok(7));
        assert_eq!(eval(&[Val(6), Val(7), Mul]), Ok(42));
        assert_eq!(eval(&[Val(20), Val(4), Div]), Ok(5));
        assert_eq!(eval(&[Val(10), Val(3), Sub]), Ok(7));
    }

    #[test]
    fn operand_order_is_a_op_b_not_b_op_a() {
        // subtraction and division are non-commutative: these catch an operand swap
        assert_eq!(eval(&[Val(10), Val(3), Sub]), Ok(7), "10 - 3, not 3 - 10");
        assert_eq!(eval(&[Val(20), Val(3), Div]), Ok(6), "20 / 3, not 3 / 20");
    }

    #[test]
    fn division_truncates_toward_zero() {
        assert_eq!(eval(&[Val(7), Val(2), Div]), Ok(3));
        assert_eq!(eval(&[Val(-7), Val(2), Div]), Ok(-3), "toward zero, not floor");
    }

    #[test]
    fn nested_programs_thread_the_stack() {
        // (2 + 3) * 4 = 20
        assert_eq!(eval(&[Val(2), Val(3), Add, Val(4), Mul]), Ok(20));
        // 2 + 3 * 4 = 14
        assert_eq!(eval(&[Val(2), Val(3), Val(4), Mul, Add]), Ok(14));
    }

    #[test]
    fn an_empty_program_leaves_no_result() {
        assert_eq!(
            eval(&[]),
            err(ErrorKind::MalformedProgram { remaining: 0 }, 0)
        );
    }

    #[test]
    fn leftover_operands_are_a_malformed_program_past_the_last_token() {
        assert_eq!(
            eval(&[Val(1), Val(2)]),
            err(ErrorKind::MalformedProgram { remaining: 2 }, 2),
            "the index is tokens.len(), and remaining counts the leftovers"
        );
    }

    #[test]
    fn each_operator_underflows_with_fewer_than_two_operands() {
        // no operands
        assert_eq!(eval(&[Add]), err(ErrorKind::StackUnderflow, 0));
        assert_eq!(eval(&[Sub]), err(ErrorKind::StackUnderflow, 0));
        assert_eq!(eval(&[Mul]), err(ErrorKind::StackUnderflow, 0));
        assert_eq!(eval(&[Div]), err(ErrorKind::StackUnderflow, 0));
        // exactly one operand — still underflow, reported at the operator's index
        assert_eq!(eval(&[Val(1), Add]), err(ErrorKind::StackUnderflow, 1));
        assert_eq!(eval(&[Val(1), Sub]), err(ErrorKind::StackUnderflow, 1));
        assert_eq!(eval(&[Val(1), Mul]), err(ErrorKind::StackUnderflow, 1));
        assert_eq!(eval(&[Val(1), Div]), err(ErrorKind::StackUnderflow, 1));
    }

    #[test]
    fn the_underflow_boundary_is_exact_at_two_operands() {
        // top == 1 underflows; top == 2 succeeds — pins the `top < 2` boundary both ways
        assert_eq!(eval(&[Val(5), Add]), err(ErrorKind::StackUnderflow, 1));
        assert_eq!(eval(&[Val(5), Val(6), Add]), Ok(11));
    }

    #[test]
    fn division_by_zero_is_named_at_the_operator() {
        assert_eq!(
            eval(&[Val(1), Val(0), Div]),
            err(ErrorKind::DivByZero, 2)
        );
    }

    #[test]
    fn every_operator_overflow_is_caught_not_wrapped() {
        assert_eq!(
            eval(&[Val(i64::MAX), Val(1), Add]),
            err(ErrorKind::Overflow, 2)
        );
        assert_eq!(
            eval(&[Val(i64::MIN), Val(1), Sub]),
            err(ErrorKind::Overflow, 2)
        );
        assert_eq!(
            eval(&[Val(i64::MAX), Val(2), Mul]),
            err(ErrorKind::Overflow, 2)
        );
        assert_eq!(
            eval(&[Val(i64::MIN), Val(-1), Div]),
            err(ErrorKind::Overflow, 2),
            "i64::MIN / -1 has no representable result"
        );
    }

    #[test]
    fn the_stack_buffer_boundary_is_exact() {
        // a buffer of exactly two holds two pushes; a third push overflows at its index
        let mut two = [0i64; 2];
        assert_eq!(evaluate(&[Val(1), Val(2), Add], &mut two), Ok(3));
        assert_eq!(
            evaluate(&[Val(1), Val(2), Val(3)], &mut two),
            Err(EvalError {
                kind: ErrorKind::StackOverflow,
                at: 2
            })
        );
        // a one-slot buffer overflows on the second push
        let mut one = [0i64; 1];
        assert_eq!(evaluate(&[Val(1)], &mut one), Ok(1));
        assert_eq!(
            evaluate(&[Val(1), Val(2)], &mut one),
            Err(EvalError {
                kind: ErrorKind::StackOverflow,
                at: 1
            })
        );
    }

    #[test]
    fn a_zero_length_buffer_overflows_on_the_first_push() {
        let mut none: [i64; 0] = [];
        assert_eq!(
            evaluate(&[Val(1)], &mut none),
            Err(EvalError {
                kind: ErrorKind::StackOverflow,
                at: 0
            })
        );
        // …but an empty program over an empty buffer is simply resultless
        assert_eq!(
            evaluate(&[], &mut none),
            Err(EvalError {
                kind: ErrorKind::MalformedProgram { remaining: 0 },
                at: 0
            })
        );
    }

    #[test]
    fn pop2_reports_the_operands_in_a_then_b_order_and_guards_the_boundary() {
        let stack = [8i64, 5, 0, 0];
        assert_eq!(pop2(&stack, 2), Some((8, 5)), "a is deeper, b is shallower");
        assert_eq!(pop2(&stack, 1), None, "one operand is too few");
        assert_eq!(pop2(&stack, 0), None, "none is too few");
    }

    #[test]
    fn settle_writes_the_deeper_slot_and_shrinks_the_stack_by_one() {
        let mut stack = [8i64, 5, 99, 0];
        let new_top = settle(&mut stack, 2, 3);
        assert_eq!(new_top, 1, "top shrinks from 2 to 1");
        assert_eq!(stack[0], 3, "the result lands in the deeper slot");
        assert_eq!(stack[1], 5, "the shallower slot is left untouched");
    }

    #[test]
    fn evaluate_runs_at_compile_time() {
        // this const is folded by the compiler; a wrong result would fail the assert
        const AT_COMPILE_TIME: i64 = {
            let mut stack = [0i64; 4];
            match evaluate(&[Val(2), Val(3), Add, Val(4), Mul], &mut stack) {
                Ok(v) => v,
                Err(_) => 0,
            }
        };
        assert_eq!(AT_COMPILE_TIME, 20);
    }
}
