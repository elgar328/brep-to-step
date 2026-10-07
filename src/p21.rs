//! Part 21 encoding: entity ids, and the literal forms of REAL, STRING,
//! enumeration, list, and reference values.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "used by the writer as it is built up")
)]

use std::fmt::Write as _;

/// Reference to an entity already written (`#n`). The field is private to
/// this module, so only [`Data`] can mint one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Ref(u64);

/// One attribute value. Borrows its data — call sites already hold the slices.
pub(crate) enum Param<'a> {
    Real(f64),
    Int(i64),
    Str(&'a str),
    /// An enumeration item, given without the dots (`"MILLI"` → `.MILLI.`).
    /// A LOGICAL's unknown is `Enum("U")`.
    Enum(&'static str),
    /// A BOOLEAN or LOGICAL: `.T.` / `.F.`.
    Bool(bool),
    Ref(Ref),
    /// `$`
    Unset,
    /// `*`
    Derived,
    Reals(&'a [f64]),
    Ints(&'a [i64]),
    Refs(&'a [Ref]),
    /// Any other list, nested lists included.
    List(Vec<Param<'a>>),
    /// A typed real such as `LENGTH_MEASURE(1.E-7)`.
    Measure(&'static str, f64),
}

/// The DATA section being written. Ids are assigned in creation order from
/// `#1`, so every reference points back to an entity already written.
#[derive(Debug)]
pub(crate) struct Data {
    next: u64,
    body: String,
}

impl Data {
    pub(crate) fn new() -> Self {
        Self {
            next: 1,
            body: String::new(),
        }
    }

    /// Write `#n = NAME(params);` and return its reference.
    pub(crate) fn simple(&mut self, name: &'static str, params: &[Param<'_>]) -> Ref {
        debug_assert!(is_entity_name(name), "bad entity name {name:?}");
        let id = self.open();
        write_record(&mut self.body, name, params);
        self.body.push_str(";\n");
        id
    }

    /// Write a complex instance `#n = ( A(..) B(..) );` and return its reference.
    /// Part 21 orders the parts by name, so the caller passes them that way.
    pub(crate) fn complex(&mut self, parts: &[(&'static str, &[Param<'_>])]) -> Ref {
        debug_assert!(
            parts.iter().all(|(name, _)| is_entity_name(name)),
            "bad entity name in {:?}",
            parts.iter().map(|(name, _)| name).collect::<Vec<_>>()
        );
        debug_assert!(
            parts.windows(2).all(|w| w[0].0 < w[1].0),
            "complex parts out of order: {:?}",
            parts.iter().map(|(name, _)| name).collect::<Vec<_>>()
        );
        let id = self.open();
        self.body.push_str("( ");
        for (name, params) in parts {
            write_record(&mut self.body, name, params);
            self.body.push(' ');
        }
        self.body.push_str(");\n");
        id
    }

    /// The DATA section's lines, without the `DATA;` / `ENDSEC;` envelope.
    pub(crate) fn into_body(self) -> String {
        self.body
    }

    /// Allocate the next id and write `#n = `.
    fn open(&mut self) -> Ref {
        let id = Ref(self.next);
        self.next += 1;
        let _ = write!(self.body, "#{} = ", id.0);
        id
    }
}

/// An entity type name as Part 21 writes it: an upper-case keyword.
fn is_entity_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// `NAME(p,p)`
fn write_record(out: &mut String, name: &str, params: &[Param<'_>]) {
    out.push_str(name);
    out.push('(');
    write_params(out, params);
    out.push(')');
}

fn write_params(out: &mut String, params: &[Param<'_>]) {
    for (i, param) in params.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_param(out, param);
    }
}

fn write_param(out: &mut String, param: &Param<'_>) {
    match param {
        Param::Real(v) => write_real(out, *v),
        Param::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Param::Str(s) => write_str(out, s),
        Param::Enum(item) => {
            out.push('.');
            out.push_str(item);
            out.push('.');
        }
        Param::Bool(b) => out.push_str(if *b { ".T." } else { ".F." }),
        Param::Ref(id) => {
            let _ = write!(out, "#{}", id.0);
        }
        Param::Unset => out.push('$'),
        Param::Derived => out.push('*'),
        Param::Reals(vs) => write_list(out, vs, |out, v| write_real(out, *v)),
        Param::Ints(is) => write_list(out, is, |out, i| {
            let _ = write!(out, "{i}");
        }),
        Param::Refs(ids) => write_list(out, ids, |out, id| {
            let _ = write!(out, "#{}", id.0);
        }),
        Param::List(params) => {
            out.push('(');
            write_params(out, params);
            out.push(')');
        }
        Param::Measure(name, v) => {
            out.push_str(name);
            out.push('(');
            write_real(out, *v);
            out.push(')');
        }
    }
}

/// `(a,b,c)`
fn write_list<T>(out: &mut String, items: &[T], mut each: impl FnMut(&mut String, &T)) {
    out.push('(');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        each(out, item);
    }
    out.push(')');
}

/// A REAL literal: the shortest form that reads back to the same `f64`,
/// always with a decimal point (`0.`, `123.`, `1.5E0`, `1.E-7`). Signed zero
/// is written as `0.` — the same value and the same point.
///
/// The value must be finite; the writer's API rejects NaN and infinities
/// before they reach here.
// Exact comparisons intended: `0.0` catches both zeros, and `v == v.trunc()`
// with `|v| < 1e15` is an integer that `i64` holds exactly.
#[allow(clippy::float_cmp, clippy::cast_possible_truncation)]
pub(crate) fn write_real(out: &mut String, v: f64) {
    debug_assert!(v.is_finite(), "non-finite REAL {v}");
    if v == 0.0 {
        out.push_str("0.");
        return;
    }
    if v == v.trunc() && v.abs() < 1e15 {
        let _ = write!(out, "{}.", v as i64);
        return;
    }
    let start = out.len();
    let _ = write!(out, "{v:E}");
    let e = start
        + out[start..]
            .find('E')
            .expect("`{:E}` always writes an exponent");
    if !out[start..e].contains('.') {
        out.insert(e, '.');
    }
}

/// Which escape run a character of a STRING literal belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Run {
    /// Printable ASCII, written as itself.
    Ascii,
    /// `\X2\` — four hex digits per character (the BMP).
    X2,
    /// `\X4\` — eight hex digits per character (beyond the BMP).
    X4,
}

/// A STRING literal. Printable ASCII is written as itself (`'` doubled, `\`
/// doubled); every other character goes into a `\X2\…\X0\` run, or a
/// `\X4\…\X0\` run beyond the BMP, so the file stays plain ASCII.
pub(crate) fn write_str(out: &mut String, s: &str) {
    out.push('\'');
    let mut run = Run::Ascii;
    for ch in s.chars() {
        let code = u32::from(ch);
        let want = match ch {
            ' '..='~' => Run::Ascii,
            _ if code <= 0xFFFF => Run::X2,
            _ => Run::X4,
        };
        if want != run {
            if run != Run::Ascii {
                out.push_str("\\X0\\");
            }
            match want {
                Run::Ascii => {}
                Run::X2 => out.push_str("\\X2\\"),
                Run::X4 => out.push_str("\\X4\\"),
            }
            run = want;
        }
        match want {
            Run::Ascii => match ch {
                '\'' => out.push_str("''"),
                '\\' => out.push_str("\\\\"),
                _ => out.push(ch),
            },
            Run::X2 => {
                let _ = write!(out, "{code:04X}");
            }
            Run::X4 => {
                let _ = write!(out, "{code:08X}");
            }
        }
    }
    if run != Run::Ascii {
        out.push_str("\\X0\\");
    }
    out.push('\'');
}

#[cfg(test)]
mod tests {
    use super::{Data, Param, Ref, write_real, write_str};
    use step_io::StepBuilder;
    use step_io::build::HeaderInput;
    use step_io::parser::lexer::{TokenKind, tokenize};

    fn real(v: f64) -> String {
        let mut out = String::new();
        write_real(&mut out, v);
        out
    }

    fn string(s: &str) -> String {
        let mut out = String::new();
        write_str(&mut out, s);
        out
    }

    /// Values where the REAL rule has a branch or an edge.
    fn edge_values() -> Vec<f64> {
        vec![
            0.0,
            -0.0,
            1.0,
            -2.0,
            0.5,
            0.1,
            1.0 / 3.0,
            123.456,
            -1e-7,
            1e-7,
            1e15 - 1.0,
            1e15,
            -1e15,
            1e20,
            4_503_599_627_370_497.0,
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::MIN,
            -2.5e-300,
        ]
    }

    /// Deterministic xorshift64, so a failure reproduces.
    struct XorShift(u64);

    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        /// Alternately an arbitrary finite bit pattern and a coordinate-like
        /// value with a few decimals.
        fn finite(&mut self) -> f64 {
            loop {
                let bits = self.next();
                let v = if bits & 1 == 0 {
                    f64::from_bits(bits)
                } else {
                    #[allow(clippy::cast_precision_loss)] // below 2^21, exact
                    let n = (bits >> 43) as f64;
                    n / 1000.0 - 1000.0
                };
                if v.is_finite() {
                    return v;
                }
            }
        }
    }

    /// What step-io writes for `v`, read off the one `CARTESIAN_POINT` of a
    /// builder holding a single vertex (no part, so no origin placement).
    fn step_io_real(v: f64) -> String {
        let mut b = StepBuilder::new().expect("builder");
        b.header(&HeaderInput {
            timestamp: Some(String::new()),
            ..Default::default()
        });
        b.vertex([v, 0.0, 0.0]).expect("vertex");
        let text = b.finish().expect("finish");
        let points: Vec<&str> = text
            .lines()
            .filter(|l| l.contains(" = CARTESIAN_POINT("))
            .collect();
        assert_eq!(points.len(), 1, "expected one point in:\n{text}");
        let coords = points[0]
            .split_once("',(")
            .unwrap_or_else(|| panic!("unexpected point line {}", points[0]))
            .1;
        coords.split(',').next().expect("x").to_owned()
    }

    /// Lex `src` alone and return its single token.
    fn single_token(src: &str) -> TokenKind {
        let tokens = tokenize(src).unwrap_or_else(|e| panic!("{src} does not lex: {e}"));
        assert_eq!(tokens.len(), 1, "{src} lexes to {tokens:?}");
        tokens.into_iter().next().expect("one token").kind
    }

    #[test]
    fn real_matches_step_io() {
        let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
        let values = edge_values()
            .into_iter()
            .chain((0..1000).map(|_| rng.finite()));
        for v in values {
            assert_eq!(real(v), step_io_real(v), "value {v:e} ({:#x})", v.to_bits());
        }
    }

    #[test]
    fn real_reads_back_exactly() {
        let mut rng = XorShift(0x2545_F491_4F6C_DD1D);
        let values = edge_values()
            .into_iter()
            .chain((0..100_000).map(|_| rng.finite()));
        for v in values {
            let text = real(v);
            let TokenKind::Real(back) = single_token(&text) else {
                panic!("{text} is not a REAL token");
            };
            // `+ 0.0` folds -0 into +0, which is what `0.` reads back as.
            assert_eq!(
                back.to_bits(),
                (v + 0.0).to_bits(),
                "{v:e} wrote {text}, read {back:e}"
            );
        }
    }

    #[test]
    fn real_forms() {
        assert_eq!(real(0.0), "0.");
        assert_eq!(real(-0.0), "0.");
        assert_eq!(real(123.0), "123.");
        assert_eq!(real(-2.0), "-2.");
        assert_eq!(real(1.5), "1.5E0");
        assert_eq!(real(1e-7), "1.E-7");
        assert_eq!(real(1e15), "1.E15");
        assert_eq!(real(1e15 - 1.0), "999999999999999.");
    }

    #[test]
    fn strings() {
        let cases = [
            ("", "''"),
            ("abc", "'abc'"),
            ("it's", "'it''s'"),
            (r"a\b", r"'a\\b'"),
            ("é", r"'\X2\00E9\X0\'"),
            ("한글", r"'\X2\D55CAE00\X0\'"),
            ("a한글b", r"'a\X2\D55CAE00\X0\b'"),
            ("😀", r"'\X4\0001F600\X0\'"),
            ("한😀a", r"'\X2\D55C\X0\\X4\0001F600\X0\a'"),
            ("😀한", r"'\X4\0001F600\X0\\X2\D55C\X0\'"),
            ("line1\nline2", r"'line1\X2\000A\X0\line2'"),
            ("\u{7F}", r"'\X2\007F\X0\'"),
        ];
        for (input, expected) in cases {
            let text = string(input);
            assert_eq!(text, expected, "input {input:?}");
            // The lexer strips the quotes and undoes `''`, nothing else.
            let raw = expected[1..expected.len() - 1].replace("''", "'");
            assert_eq!(
                single_token(&text),
                TokenKind::String(raw),
                "input {input:?}"
            );
        }
    }

    #[test]
    fn lines() {
        let mut d = Data::new();
        let a = d.simple(
            "CARTESIAN_POINT",
            &[Param::Str(""), Param::Reals(&[0.0, 1.5, -2.0])],
        );
        let b = d.simple(
            "EVERY_PARAM",
            &[
                Param::Real(0.25),
                Param::Int(-3),
                Param::Str("x"),
                Param::Enum("MILLI"),
                Param::Enum("U"),
                Param::Bool(true),
                Param::Bool(false),
                Param::Ref(a),
                Param::Unset,
                Param::Derived,
                Param::Reals(&[]),
                Param::Ints(&[4, 4]),
                Param::Refs(&[a, a]),
                Param::List(vec![Param::Refs(&[a]), Param::Reals(&[1.0])]),
                Param::Measure("LENGTH_MEASURE", 1e-7),
            ],
        );
        let c = d.complex(&[
            ("LENGTH_UNIT", &[]),
            ("NAMED_UNIT", &[Param::Derived]),
            ("SI_UNIT", &[Param::Enum("MILLI"), Param::Enum("METRE")]),
        ]);
        assert_eq!((a, b, c), (Ref(1), Ref(2), Ref(3)));
        assert_eq!(
            d.into_body(),
            "#1 = CARTESIAN_POINT('',(0.,1.5E0,-2.));\n\
             #2 = EVERY_PARAM(2.5E-1,-3,'x',.MILLI.,.U.,.T.,.F.,#1,$,*,(),(4,4),(#1,#1),\
             ((#1),(1.)),LENGTH_MEASURE(1.E-7));\n\
             #3 = ( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) );\n"
        );
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "complex parts out of order")]
    fn complex_parts_must_be_ordered() {
        Data::new().complex(&[("NAMED_UNIT", &[]), ("LENGTH_UNIT", &[])]);
    }
}
