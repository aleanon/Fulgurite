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

// ---- T61.2: lowering -------------------------------------------------

use crate::compile;
use crate::lower::{Align, Cursor, Edges, Length, Role, Rule as LRule, Setter, TextOverflow, Val, PROPERTIES};

fn lowered(src: &str) -> crate::Sheet {
    compile(src).unwrap_or_else(|e| panic!("{:#?}", e.iter().map(|e| e.at("t.lss")).collect::<Vec<_>>()))
}

fn set(method: &'static str, value: Val) -> Setter {
    Setter { method, value }
}

#[test]
fn every_property_lowers_to_its_setter() {
    let s = lowered(
        "@tokens { ink: #f1f5fbff; gap: 6px; }
         #a {
            width: 12px; height: fixed(40px); padding: 4px 8px; spacing: $gap;
            align: center; visible: false; cursor: not-allowed; focus-ring: 2px;
            color: $ink; background: $surface; border-width: 1px; border-color: $focus;
            border-radius: 8px; font-size: 15px; font-weight: 600; box-shadow: 12px;
            pressed-dim: 0.15; hover-dim: 0.07; disabled: true; line-height: 1.5;
            text-overflow: ellipsis; background-fill: 1;
         }
         #b { width: fill; height: shrink; padding: 1px 2px 3px; color: #10203040; }
         #c { width: fill(3); padding: 1px 2px 3px 4px; }",
    );
    let want_a = vec![
        set("width", Val::Length(Length::Fixed(12.0))),
        set("height", Val::Length(Length::Fixed(40.0))),
        set("padding", Val::Edges(Edges { left: 8.0, right: 8.0, top: 4.0, bottom: 4.0 })),
        set("spacing", Val::F32(6.0)),
        set("align", Val::Align(Align::Center)),
        set("visible", Val::Bool(false)),
        set("cursor", Val::Cursor(Cursor::NotAllowed)),
        set("focus_ring", Val::U8(2)),
        set("color", Val::Color([0xf1, 0xf5, 0xfb, 0xff])),
        set("background_role", Val::Role(Role::Surface)),
        set("border_width", Val::U8(1)),
        set("border_color_role", Val::Role(Role::Focus)),
        set("radius", Val::F32(8.0)),
        set("font_size", Val::F32(15.0)),
        set("font_weight", Val::F32(600.0)),
        set("shadow", Val::F32(12.0)),
        set("pressed_dim", Val::F32(0.15)),
        set("hover_dim", Val::F32(0.07)),
        set("disabled", Val::Bool(true)),
        set("line_height", Val::F32(1.5)),
        set("overflow", Val::TextOverflow(TextOverflow::Ellipsis)),
        set("background_fill", Val::U8(1)),
    ];
    assert_eq!(s.base[0].setters, want_a);
    assert_eq!(
        s.base[1].setters,
        vec![
            set("width", Val::Length(Length::FillPortion(1))),
            set("height", Val::Length(Length::Shrink)),
            set("padding", Val::Edges(Edges { left: 2.0, right: 2.0, top: 1.0, bottom: 3.0 })),
            set("color", Val::Color([0x10, 0x20, 0x30, 0x40])),
        ]
    );
    assert_eq!(
        s.base[2].setters,
        vec![
            set("width", Val::Length(Length::FillPortion(3))),
            set("padding", Val::Edges(Edges { left: 4.0, right: 2.0, top: 1.0, bottom: 3.0 })),
        ]
    );
    // Every property in the table was exercised above.
    let used: Vec<&str> = s.base.iter().flat_map(|r| r.setters.iter().map(|x| x.method)).collect();
    for p in PROPERTIES {
        assert!(
            used.contains(&p.setter) || p.role_setter.is_some_and(|r| used.contains(&r)),
            "property `{}` is not exercised by this test",
            p.name
        );
    }
}

#[test]
fn a_decimal_is_the_f32_a_person_would_type() {
    // Through f64 first, 0.07 and 0.15 could round twice; the text goes
    // straight to f32, so each is the literal's value bit for bit.
    let s = lowered("#a { hover-dim: 0.07; pressed-dim: 0.15; font-size: 13.3px; }");
    let bits: Vec<u32> = s.base[0]
        .setters
        .iter()
        .map(|x| match x.value {
            Val::F32(v) => v.to_bits(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(bits, [0.07f32.to_bits(), 0.15f32.to_bits(), 13.3f32.to_bits()]);
}

#[test]
fn comma_lists_and_sheets_lower_in_declaration_order() {
    let s = lowered(
        "* { spacing: 5px; }\n#a, .b, label { font-size: 12px; }\n@sheet big { #a { font-size: 24px; } }",
    );
    let sels: Vec<String> = s.base.iter().map(|r| r.selector.to_string()).collect();
    assert_eq!(sels, ["*", "#a", ".b", "label"]);
    let big: Vec<&LRule> = s.select(Some("big")).expect("big");
    assert_eq!(big.len(), 5, "the four base rules, then the block's");
    assert_eq!(big[4].setters, vec![set("font_size", Val::F32(24.0))]);
    assert_eq!(s.select(None).expect("base").len(), 4);
    let err = s.select(Some("small")).expect_err("no such block");
    assert_eq!(err, "no `@sheet small`: the file has `big`");
}

/// `(what, source, the message must contain, line, col)`.
const REFUSED_LOWERING: &[(&str, &str, &str, u32, u32)] = &[
    ("unknown property", "#t {\n    colour: #ffffff;\n}", "unknown property `colour`; did you mean `color`?", 2, 5),
    ("css alias", "#t { gap: 4px; }", "unknown property `gap`; did you mean `spacing`?", 1, 6),
    ("colour given px", "#t { color: 12px; }", "`color` takes a colour", 1, 13),
    ("length keyword", "#t { width: wide; }", "`width` takes a length", 1, 13),
    ("px without unit", "#t { font-size: 12; }", "`font-size` takes a length in px", 1, 17),
    ("number with unit", "#t { font-weight: 600px; }", "`font-weight` takes a bare number", 1, 19),
    ("byte too big", "#t { border-width: 300px; }", "`border-width` takes a whole number of px", 1, 20),
    ("byte fraction", "#t { focus-ring: 1.5px; }", "`focus-ring` takes a whole number of px", 1, 18),
    // M82/T82.3: a table index has no unit, so a length is refused with
    // `Takes::Index`'s own description, as are a fraction and 256.
    ("index in px", "#t { background-fill: 1px; }", "`background-fill` takes a whole number from 0 to 255, no unit, like `1`", 1, 23),
    ("index fraction", "#t { background-fill: 1.5; }", "`background-fill` takes a whole number from 0 to 255", 1, 23),
    ("index too big", "#t { background-fill: 256; }", "`background-fill` takes a whole number from 0 to 255", 1, 23),
    ("five edges", "#t { padding: 1px 2px 3px 4px 5px; }", "`padding` takes one to four lengths", 1, 15),
    ("align", "#t { align: middle; }", "`align` takes `start`, `center`, `end` or `fill`", 1, 13),
    ("cursor", "#t { cursor: hand; }", "`cursor` takes `default`", 1, 14),
    ("bool", "#t { visible: yes; }", "`visible` takes `true` or `false`", 1, 15),
    ("fill zero", "#t { width: fill(0); }", "`width` takes a length", 1, 13),
    ("string", "#t { color: \"red\"; }", "or a palette role like `$accent`, not `\"red\"`", 1, 13),
    ("percent", "#t { width: 50%; }", "`shrink`, not `50%`", 1, 13),
    ("unknown token", "@tokens { ink: #ffffff; }\n#t { color: $inkk; }", "unknown token `$inkk`; did you mean `$ink`?", 2, 13),
    ("role shadowed", "@tokens {\n    accent: #4d7cfeff;\n}", "`@tokens` name `accent` shadows the palette role `$accent`", 2, 5),
    ("role in a length", "#t { padding: $accent; }", "`$accent` is a palette role, which is a colour; `padding` takes", 1, 15),
    ("role in a list", "#t { padding: 1px $text; }", "`$text` is a palette role", 1, 15),
    ("token of the wrong type", "@tokens { gap: 6px; }\n#t { color: $gap; }", "not `$gap` (`$gap` is `6px`)", 2, 13),
    ("token names a token", "@tokens { a: 1px; b: $a; }", "token `$b`: a token's value cannot name another token", 1, 19),
    ("sheet name", "@sheet on-accent { }", "a sheet's name must be a Rust identifier", 1, 1),
];

#[test]
fn every_value_refusal_is_an_error_naming_its_reason_at_the_value() {
    let mut failures = Vec::new();
    for &(what, src, want, line, col) in REFUSED_LOWERING {
        let errs = match compile(src) {
            Ok(s) => {
                failures.push(format!("{what}: compiled, to {s:?}"));
                continue;
            }
            Err(e) => e,
        };
        let shown: Vec<String> = errs.iter().map(|e| e.at("t.lss")).collect();
        println!("{what:>24}: {shown:?}");
        match errs.iter().find(|e| e.msg.contains(want)) {
            None => failures.push(format!("{what}: no error contains {want:?}; got {shown:?}")),
            Some(e) if (e.span.line, e.span.col) != (line, col) => {
                failures.push(format!("{what}: at {}:{}, want {line}:{col}", e.span.line, e.span.col))
            }
            Some(_) => {}
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} refusals wrong:\n{}",
        failures.len(),
        REFUSED_LOWERING.len(),
        failures.join("\n")
    );
}
