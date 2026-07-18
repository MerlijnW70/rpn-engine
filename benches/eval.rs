//! Benchmarks: the zero-allocation `evaluate` against two conventional baselines — a
//! `Vec`-backed iterative evaluator (heap stack) and a recursive expression-tree
//! evaluator (heap tree + call stack). Same programs, same results; the numbers show
//! what owning the memory and staying iterative buys you.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rpn_engine::{evaluate, Token};

/// Build the RPN program for `(((1 + 2) + 3) + ... + n)` — `n - 1` additions, stack
/// depth never exceeds two.
fn left_fold_sum(n: i64) -> Vec<Token> {
    let mut program = vec![Token::Val(1), Token::Val(2), Token::Add];
    for v in 3..=n {
        program.push(Token::Val(v));
        program.push(Token::Add);
    }
    program
}

/// Baseline A: an iterative evaluator using a heap-allocated `Vec` as its stack.
fn vec_eval(tokens: &[Token]) -> i64 {
    let mut stack: Vec<i64> = Vec::new();
    for &tok in tokens {
        match tok {
            Token::Val(v) => stack.push(v),
            op => {
                let b = stack.pop().expect("well-formed program");
                let a = stack.pop().expect("well-formed program");
                let r = match op {
                    Token::Add => a + b,
                    Token::Sub => a - b,
                    Token::Mul => a * b,
                    Token::Div => a / b,
                    Token::Val(_) => unreachable!(),
                };
                stack.push(r);
            }
        }
    }
    stack.pop().expect("one result")
}

/// A parsed expression tree — the classic representation a recursive evaluator walks.
enum Expr {
    Val(i64),
    Op(Token, Box<Expr>, Box<Expr>),
}

/// Parse an RPN token stream into an expression tree (heap allocation per node).
fn parse_tree(tokens: &[Token]) -> Expr {
    let mut stack: Vec<Expr> = Vec::new();
    for &tok in tokens {
        match tok {
            Token::Val(v) => stack.push(Expr::Val(v)),
            op => {
                let b = stack.pop().expect("well-formed program");
                let a = stack.pop().expect("well-formed program");
                stack.push(Expr::Op(op, Box::new(a), Box::new(b)));
            }
        }
    }
    stack.pop().expect("one root")
}

/// Baseline B: evaluate the expression tree by recursion (heap tree + call stack).
fn tree_eval(expr: &Expr) -> i64 {
    match expr {
        Expr::Val(v) => *v,
        Expr::Op(op, a, b) => {
            let a = tree_eval(a);
            let b = tree_eval(b);
            match op {
                Token::Add => a + b,
                Token::Sub => a - b,
                Token::Mul => a * b,
                Token::Div => a / b,
                Token::Val(_) => unreachable!(),
            }
        }
    }
}

fn bench_evaluators(c: &mut Criterion) {
    let program = left_fold_sum(256);
    let mut group = c.benchmark_group("rpn/left_fold_sum_256");

    group.bench_function("zero_alloc_evaluate", |bench| {
        let mut stack = [0i64; 4];
        bench.iter(|| evaluate(black_box(&program), black_box(&mut stack)));
    });

    group.bench_function("vec_stack_iterative", |bench| {
        bench.iter(|| vec_eval(black_box(&program)));
    });

    group.bench_function("recursive_tree", |bench| {
        bench.iter(|| {
            let tree = parse_tree(black_box(&program));
            tree_eval(black_box(&tree))
        });
    });

    group.finish();
}

criterion_group!(benches, bench_evaluators);
criterion_main!(benches);
