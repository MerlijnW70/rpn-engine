#![no_std]
#![forbid(unsafe_code)]
//! A zero-allocation, `no_std` evaluator for Reverse Polish Notation.
//!
//! The caller owns the memory. [`evaluate`] takes a token slice and a scratch stack
//! buffer (`&mut [i64]`) and never allocates: it runs in `O(n)` time and `O(1)` space
//! beyond the two slices you hand it. Every failure — a too-small buffer, a malformed
//! program, division by zero, an integer overflow — is a named [`EvalError`], never a
//! panic. All arithmetic is checked, so the result is identical on every architecture.
//!
//! ```
//! use rpn_engine::{evaluate, Token::*};
//! let mut stack = [0i64; 8];
//! // (2 + 3) * 4
//! assert_eq!(evaluate(&[Val(2), Val(3), Add, Val(4), Mul], &mut stack), Ok(20));
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

/// Every way [`evaluate`] can refuse — each carries exactly what went wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvalError {
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

/// Evaluate `tokens`, using `stack` as scratch space, and return the single result.
///
/// The program is well-formed when it leaves exactly one value on the stack. Runs in
/// `O(tokens.len())` time and allocates nothing.
pub fn evaluate(tokens: &[Token], stack: &mut [i64]) -> Result<i64, EvalError> {
    let mut top: usize = 0;
    for &tok in tokens {
        match tok {
            Token::Val(v) => {
                if top == stack.len() {
                    return Err(EvalError::StackOverflow);
                }
                stack[top] = v;
                top += 1;
            }
            Token::Add => top = fold(stack, top, |a, b| a.checked_add(b).ok_or(EvalError::Overflow))?,
            Token::Sub => top = fold(stack, top, |a, b| a.checked_sub(b).ok_or(EvalError::Overflow))?,
            Token::Mul => top = fold(stack, top, |a, b| a.checked_mul(b).ok_or(EvalError::Overflow))?,
            Token::Div => {
                top = fold(stack, top, |a, b| {
                    if b == 0 {
                        Err(EvalError::DivByZero)
                    } else {
                        a.checked_div(b).ok_or(EvalError::Overflow)
                    }
                })?
            }
        }
    }
    if top != 1 {
        return Err(EvalError::MalformedProgram { remaining: top });
    }
    Ok(stack[0])
}

/// Apply a binary operator to the top two stack values, replacing them with the result.
///
/// `a` is the deeper operand, `b` the shallower, matching RPN order (`a op b`). Returns
/// the new top index, or [`EvalError::StackUnderflow`] when fewer than two operands are
/// present, or whatever `op` refuses with.
fn fold(
    stack: &mut [i64],
    top: usize,
    op: impl FnOnce(i64, i64) -> Result<i64, EvalError>,
) -> Result<usize, EvalError> {
    if top < 2 {
        return Err(EvalError::StackUnderflow);
    }
    let b = stack[top - 1];
    let a = stack[top - 2];
    let r = op(a, b)?;
    stack[top - 2] = r;
    Ok(top - 1)
}

#[cfg(test)]
mod tests {
    use super::Token::*;
    use super::*;

    fn eval(tokens: &[Token]) -> Result<i64, EvalError> {
        let mut stack = [0i64; 16];
        evaluate(tokens, &mut stack)
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
        assert_eq!(eval(&[]), Err(EvalError::MalformedProgram { remaining: 0 }));
    }

    #[test]
    fn leftover_operands_are_a_malformed_program() {
        assert_eq!(
            eval(&[Val(1), Val(2)]),
            Err(EvalError::MalformedProgram { remaining: 2 })
        );
    }

    #[test]
    fn an_operator_without_two_operands_underflows() {
        assert_eq!(eval(&[Add]), Err(EvalError::StackUnderflow));
        assert_eq!(eval(&[Val(1), Add]), Err(EvalError::StackUnderflow));
    }

    #[test]
    fn the_underflow_boundary_is_exact_at_two_operands() {
        // top == 1 underflows; top == 2 succeeds — pins the `top < 2` boundary both ways
        assert_eq!(eval(&[Val(5), Add]), Err(EvalError::StackUnderflow));
        assert_eq!(eval(&[Val(5), Val(6), Add]), Ok(11));
    }

    #[test]
    fn division_by_zero_is_named() {
        assert_eq!(eval(&[Val(1), Val(0), Div]), Err(EvalError::DivByZero));
    }

    #[test]
    fn every_overflow_is_caught_not_wrapped() {
        assert_eq!(
            eval(&[Val(i64::MAX), Val(1), Add]),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            eval(&[Val(i64::MIN), Val(1), Sub]),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            eval(&[Val(i64::MAX), Val(2), Mul]),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            eval(&[Val(i64::MIN), Val(-1), Div]),
            Err(EvalError::Overflow),
            "i64::MIN / -1 has no representable result"
        );
    }

    #[test]
    fn the_stack_buffer_boundary_is_exact() {
        // a buffer of exactly two holds two pushes; a third push overflows
        let mut two = [0i64; 2];
        assert_eq!(evaluate(&[Val(1), Val(2), Add], &mut two), Ok(3));
        assert_eq!(
            evaluate(&[Val(1), Val(2), Val(3)], &mut two),
            Err(EvalError::StackOverflow)
        );
        // a one-slot buffer overflows on the second push
        let mut one = [0i64; 1];
        assert_eq!(evaluate(&[Val(1)], &mut one), Ok(1));
        assert_eq!(
            evaluate(&[Val(1), Val(2)], &mut one),
            Err(EvalError::StackOverflow)
        );
    }

    #[test]
    fn a_zero_length_buffer_overflows_on_the_first_push() {
        let mut none: [i64; 0] = [];
        assert_eq!(evaluate(&[Val(1)], &mut none), Err(EvalError::StackOverflow));
        // …but an empty program over an empty buffer is simply resultless
        assert_eq!(
            evaluate(&[], &mut none),
            Err(EvalError::MalformedProgram { remaining: 0 })
        );
    }

    #[test]
    fn fold_reuses_the_deeper_slot_and_shrinks_the_stack_by_one() {
        // after a binary op the result sits where `a` was, and top drops by exactly one
        let mut stack = [0i64; 4];
        stack[0] = 8;
        stack[1] = 5;
        let new_top = fold(&mut stack, 2, |a, b| Ok(a - b)).expect("two operands present");
        assert_eq!(new_top, 1, "top shrinks from 2 to 1");
        assert_eq!(stack[0], 3, "the result lands in the deeper slot");
    }
}
