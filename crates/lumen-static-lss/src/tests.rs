//! T61.1's gate (`docs/plan-lss.md`): the parser round-trips every
//! construct v1 accepts, and refuses every §2.1 refusal with its own
//! message, at the line and column where it starts.

use crate::ast::*;
use crate::parse;

/// Every construct the grammar accepts, once.
const ACCEPTED: &str = r#"
/* a block comment */
// a line comment
@tokens {
    ink: #f1f5fbff;
    gap: 6px;
    weight: 600;
}

* { spacing: $gap; }
label { color: $ink; font-weight: $weight; }
.note { color: $muted; background: #16202c; }
#title, #badge, Page {
    width: fixed(120px);
    height: 40px;
    padding: 4px 8px 2px 1px;
    align: center;
    hover-dim: 0.07;
    width: fill(2)
}

@sheet loud {
    #title { font-size: 24px; color: $accent; }
    .note { visible: false; }
}
"#;

fn ok(src: &str) -> Stylesheet {
    let (sheet, errs) = parse(src);
    assert!(errs.is_empty(), "unexpected errors: {:#?}", errs.iter().map(|e| e.at("t.lss")).collect::<Vec<_>>());
    sheet
}

#[test]
fn every_accepted_construct_parses_and_round_trips() {
    let sheet = ok(ACCEPTED);
    let printed = sheet.to_string();
    println!("{printed}");
    // Round trip: printing and re-parsing gives the same text again.
    let again = ok(&printed);
    assert_eq!(again.to_string(), printed, "print(parse(print(parse(src)))) differs");

    // And the structure is the one written, not merely a stable one.
    assert_eq!(sheet.items.len(), 6, "tokens, four rules, one sheet: {:#?}", sheet.items);
    let Item::Tokens(tokens) = &sheet.items[0] else { panic!("item 0 is {:?}", sheet.items[0]) };
    assert_eq!(tokens.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["ink", "gap", "weight"]);
    assert_eq!(tokens[0].value, Value::Color([0xf1, 0xf5, 0xfb, 0xff]));
    assert_eq!(tokens[1].value, Value::Number("6".into(), Unit::Px));
    let Item::Rule(any) = &sheet.items[1] else { panic!() };
    assert_eq!(any.selectors[0].part, Part::Any);
    assert_eq!(any.declarations[0].value, Value::Var("gap".into()));
    let Item::Rule(list) = &sheet.items[4] else { panic!() };
    assert_eq!(
        list.selectors.iter().map(|s| s.part.clone()).collect::<Vec<_>>(),
        [Part::Id("title".into()), Part::Id("badge".into()), Part::Kind("Page".into())]
    );
    assert_eq!(list.selectors[1].span, Span { line: 13, col: 9 }, "a selector's span is where it starts");
    let d = &list.declarations;
    assert_eq!(d[0].value, Value::Function("fixed".into(), vec![Value::Number("120".into(), Unit::Px)]));
    assert_eq!(
        d[2].value,
        Value::List(vec![
            Value::Number("4".into(), Unit::Px),
            Value::Number("8".into(), Unit::Px),
            Value::Number("2".into(), Unit::Px),
            Value::Number("1".into(), Unit::Px),
        ])
    );
    assert_eq!(d[4].value, Value::Number("0.07".into(), Unit::None), "the number's text is kept");
    assert_eq!(d.len(), 6, "the last declaration may omit its `;`");
    let Item::Sheet(loud) = &sheet.items[5] else { panic!() };
    assert_eq!(loud.name, "loud");
    assert_eq!(loud.rules.len(), 2);
    assert_eq!(loud.span, Span { line: 22, col: 1 });
}

/// `(what, source, the message must contain, line, col)`.
const REFUSED: &[(&str, &str, &str, u32, u32)] = &[
    ("compound", ".value.pos { color: #ffffff; }", "compound selector `.value.pos`", 1, 1),
    ("compound id", "\n  button#go { color: #ffffff; }", "compound selector `button#go`", 2, 3),
    ("descendant", "#title label { color: #ffffff; }", "descendant combinator in `#title label`", 1, 1),
    ("child", "#title > label { color: #ffffff; }", "child combinator `>` in `#title > label`", 1, 1),
    ("state", "#title:hovered { color: #ffffff; }", "`:hovered` is a state selector", 1, 1),
    ("nested", "#title {\n    &:hovered { color: #000000; }\n}", "nested rule `&`", 2, 5),
    ("media", "@media (width > 400px) { #t { font-size: 20px; } }", "`@media` is not in v1", 1, 1),
    ("keyframes", "@keyframes pulse { 0% { color: #ffffff; } }", "`@keyframes` is not in v1", 1, 1),
    ("theme", "@theme dark { ink: #ffffff; }", "`@theme` is not in v1", 1, 1),
    ("unknown at-rule", "@import \"x.lss\";", "unknown at-rule `@import`", 1, 1),
    ("important", "#t { color: #ffffff !important; }", "`!important` is not in v1", 1, 21),
    ("bad hex", "#t { color: #12345; }", "invalid colour `#12345`", 1, 13),
    ("bad unit", "#t { font-size: 12em; }", "unknown unit in `12em`", 1, 17),
    ("comma value", "#t { padding: 1px, 2px; }", "`,` in the value of `padding`", 1, 18),
    ("no value", "#t { color: ; }", "expected a value for `color`", 1, 13),
    ("no colon", "#t { color #ffffff; }", "expected `:` after `color`", 1, 12),
    ("tokens in sheet", "@sheet a { @tokens { x: 1; } }", "`@tokens` belongs at the top level", 1, 12),
    ("sheet twice", "@sheet a { }\n@sheet a { }", "`@sheet a` is defined twice (first at 1:1)", 2, 1),
    ("token twice", "@tokens { x: 1px; x: 2px; }", "token `$x` is defined twice (first at 1:11)", 1, 19),
    ("stray brace", "}", "unmatched `}`", 1, 1),
    ("unclosed", "#t { color: #ffffff;", "expected `}` before the end of the file", 1, 21),
];

#[test]
fn every_refusal_is_an_error_naming_its_reason_at_its_line_and_column() {
    let mut failures = Vec::new();
    for &(what, src, want, line, col) in REFUSED {
        let (_, errs) = parse(src);
        let shown: Vec<String> = errs.iter().map(|e| e.at("t.lss")).collect();
        println!("{what:>16}: {shown:?}");
        let hit = errs.iter().find(|e| e.msg.contains(want));
        match hit {
            None => failures.push(format!("{what}: no error contains {want:?}; got {shown:?}")),
            Some(e) if e.span != (Span { line, col }) => failures.push(format!(
                "{what}: at {}:{}, want {line}:{col}",
                e.span.line, e.span.col
            )),
            Some(_) => {}
        }
    }
    assert!(failures.is_empty(), "{} of {} refusals wrong:\n{}", failures.len(), REFUSED.len(), failures.join("\n"));
}

/// Refused, not ignored: a refused rule contributes no rule, and the
/// rules around it still parse — and still report their own errors.
#[test]
fn a_refusal_drops_its_rule_and_keeps_parsing() {
    let (sheet, errs) = parse(".a.b { color: #ffffff; }\n#ok { color: #000000; }\n#t { color: #1234; }");
    let msgs: Vec<String> = errs.iter().map(|e| e.at("t.lss")).collect();
    println!("{msgs:?}");
    assert_eq!(errs.len(), 2, "the compound and the bad colour: {msgs:?}");
    // The compound rule is gone; `#t` keeps its selector and loses the
    // declaration that was refused. (A sheet with any error is rejected
    // whole by every consumer, so this is only about finding the rest.)
    let kept: Vec<(String, usize)> = sheet
        .items
        .iter()
        .map(|i| match i {
            Item::Rule(r) => (r.selectors[0].part.to_string(), r.declarations.len()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(kept, [("#ok".to_string(), 1), ("#t".to_string(), 0)]);
}
