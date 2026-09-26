use crate::syntax::ast::{BinOp, ExprKind, Program, StmtKind};
use crate::syntax::header::parse_header;
use crate::syntax::lexer::lex;
use crate::syntax::parser::parse;

fn load(body: &str) -> Result<Program, String> {
    let src = format!("~fold v1 256x256\n{body}");
    let (header, rest) = parse_header(&src)?;
    parse(header, lex(rest, 2)?)
}

fn first_draw(body: &str) -> ExprKind {
    let program = load(body).unwrap();
    match program.body.into_iter().last().unwrap().kind {
        StmtKind::Draw(expr) => expr.kind,
        other => panic!("expected draw, found {other:?}"),
    }
}

#[test]
fn cookbook_recipes_parse() {
    let book = include_str!("../../../docs/COOKBOOK.md");
    let recipes: Vec<&str> = book.split("```").skip(1).step_by(2).collect();
    assert_eq!(recipes.len(), book.matches("\n### ").count());
    for recipe in recipes {
        if let Err(e) = load(recipe.trim_start()) {
            panic!("{e}\n{recipe}");
        }
    }
}

#[test]
fn spec_example_parses() {
    let spec = include_str!("../../../docs/SPEC.md");
    let example = spec.split("```").nth(1).unwrap();
    let (header, rest) = parse_header(example.trim_start()).unwrap();
    parse(header, lex(rest, 2).unwrap()).unwrap();
}

#[test]
fn pipe_becomes_call_with_subject_first() {
    let ExprKind::Call { name, args } = first_draw("draw sun |> fill(#ffaa00)") else {
        panic!("expected a call");
    };
    assert_eq!(name, "fill");
    assert_eq!(args[0].kind, ExprKind::Name("sun".into()));
    assert_eq!(args[1].kind, ExprKind::Rgba(0xffaa00ff));
}

#[test]
fn pipe_binds_tighter_than_multiply() {
    let ExprKind::Binary { op, left, .. } = first_draw("draw sun |> fill(#ffaa00) * 0.5") else {
        panic!("expected a binary");
    };
    assert_eq!(op, BinOp::Mul);
    assert!(matches!(left.kind, ExprKind::Call { .. }));
}

#[test]
fn pipe_step_without_brackets() {
    let ExprKind::Call { name, args } = first_draw("draw petal |> mirror") else {
        panic!("expected a call");
    };
    assert_eq!(name, "mirror");
    assert_eq!(args.len(), 1);
}

#[test]
fn pipe_continues_on_next_line() {
    let ExprKind::Call { name, .. } = first_draw("draw sun\n    |> at(1, 2)\n    |> fill(#ffaa00)")
    else {
        panic!("expected a call");
    };
    assert_eq!(name, "fill");
}

#[test]
fn inline_function() {
    let ExprKind::Call { args, .. } = first_draw("draw shape(pt => length(pt) - 60)") else {
        panic!("expected a call");
    };
    assert!(matches!(&args[0].kind, ExprKind::Lambda { param, .. } if param == "pt"));
}

#[test]
fn colour_input_has_no_range() {
    let program = load("input accent = #3366ff").unwrap();
    assert!(matches!(
        program.body[0].kind,
        StmtKind::Input { range: None, .. }
    ));
}

#[test]
fn eight_digit_colour_keeps_alpha() {
    let ExprKind::Call { args, .. } = first_draw("draw sun |> fill(#00000066)") else {
        panic!("expected a call");
    };
    assert_eq!(args[1].kind, ExprKind::Rgba(0x00000066));
}

#[test]
fn reserved_word_is_an_error() {
    let error = load("let if = 1").unwrap_err();
    assert!(error.contains("`if` is reserved"), "{error}");
}
