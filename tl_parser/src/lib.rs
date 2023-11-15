use anyhow::anyhow;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, take_till, take_until, take_while, take_while1, take_while_m_n};
use nom::character::complete::{char, line_ending, multispace0, multispace1, newline, satisfy};
use nom::combinator::{map, opt, recognize, value};
use nom::error::Error;
use nom::multi::{fold_many0, many0};
use nom::Parser;
use nom::sequence::{delimited, pair, preceded, terminated};

pub type ConstructorNumber = u32;

#[derive(Debug, PartialEq, Eq)]
pub struct Combinator {
    builtin: bool,
    id: String,
    r#type: String,
    constructor_number: Option<ConstructorNumber>,
}

pub fn parse(input: &str) -> anyhow::Result<Vec<Combinator>> {
    let (input, vecs) = many0(
        delimited(opt(space_or_comment), alt((combinator_decl, builtin_combinator_decl)), opt(space_or_comment))
    )(input).map_err(|e| anyhow!("parse error: {}", e))?;

    println!("{}", input);

    Ok(vecs)
}

fn is_lc_letter(c: char) -> bool { c.is_ascii_lowercase() }

fn is_uc_letter(c: char) -> bool { c.is_ascii_uppercase() }

fn is_digit(c: char) -> bool { c.is_digit(10) }

fn is_hex_digit(c: char) -> bool { c.is_digit(16) }

fn is_underscore(c: char) -> bool { c == '_' }

fn is_letter(c: char) -> bool { is_lc_letter(c) || is_uc_letter(c) }

fn is_ident_char(c: char) -> bool { is_letter(c) || is_digit(c) || is_underscore(c) }

fn single_line_comment(input: &str) -> nom::IResult<&str, &str> {
    preceded(tag("//"), take_till(|c| c == '\n'))(input)
}

fn multi_line_comment(input: &str) -> nom::IResult<&str, &str> {
    delimited(tag("/*"), take_until("*/"), tag("*/"))(input)
}

fn space_or_comment(input: &str) -> nom::IResult<&str, ()> {
    let (input, _) = many0(alt((
        multispace1,
        single_line_comment,
        multi_line_comment,
        line_ending
    )))(input)?;

    Ok((input, ()))
}

fn lc_ident(input: &str) -> nom::IResult<&str, String> {
    let (input, head) = satisfy(is_lc_letter)(input)?;
    let (input, tail) = take_while1(is_ident_char)(input)?;

    Ok((input, format!("{}{}", head, tail)))
}

fn uc_ident(input: &str) -> nom::IResult<&str, String> {
    let (input, head) = satisfy(is_uc_letter)(input)?;
    let (input, tail) = take_while1(is_ident_char)(input)?;

    Ok((input, format!("{}{}", head, tail)))
}

fn namespace_ident(input: &str) -> nom::IResult<&str, String> { lc_ident(input) }

fn lc_ident_ns(input: &str) -> nom::IResult<&str, String> {
    let (input, ns) = opt(terminated(namespace_ident, tag(".")))(input)?;
    let (input, head) = lc_ident(input)?;

    match ns {
        None => Ok((input, head)),
        Some(ns) => Ok((input, format!("{}.{}", ns, head)))
    }
}

fn uc_ident_ns(input: &str) -> nom::IResult<&str, String> {
    let (input, ns) = opt(terminated(namespace_ident, tag(".")))(input)?;
    let (input, head) = uc_ident(input)?;

    match ns {
        None => Ok((input, head)),
        Some(ns) => Ok((input, format!("{}.{}", ns, head)))
    }
}

fn lc_ident_full(input: &str) -> nom::IResult<&str, (String, Option<ConstructorNumber>)> {
    let (input, ident) = lc_ident_ns(input)?;
    let (input, combinator_number) = opt(preceded(tag("#"), take_while_m_n(8, 8, is_hex_digit)))(input)?;

    match combinator_number {
        None => Ok((input, (ident, None))),
        Some(combinator_number) => {
            let combinator_number = ConstructorNumber::from_str_radix(combinator_number, 16).expect("invalid combinator number");

            Ok((input, (ident, Some(combinator_number))))
        }
    }
}

fn full_combinator_id(input: &str) -> nom::IResult<&str, (String, Option<ConstructorNumber>)> {
    Ok(alt((
        lc_ident_full,
        map(tag("_"), |s: &str| (s.to_owned(), None))
    ))(input)?)
}

fn boxed_type_ident(input: &str) -> nom::IResult<&str, String> {
    uc_ident_ns(input)
}

fn result_type(input: &str) -> nom::IResult<&str, String> {
    let (input, ident) = boxed_type_ident(input)?;

    Ok((input, ident))
}

fn combinator_decl(input: &str) -> nom::IResult<&str, Combinator> {
    let (input, (combinator_id, constructor_number)) = preceded(multispace0, full_combinator_id)(input)?;
    let (input, _) = delimited(multispace0, tag("="), multispace0)(input)?;
    let (input, combinator_type) = result_type(input)?;
    let (input, _) = preceded(multispace0, tag(";"))(input)?;

    Ok((input, Combinator { id: combinator_id, r#type: combinator_type, builtin: false, constructor_number }))
}

fn builtin_combinator_decl(input: &str) -> nom::IResult<&str, Combinator> {
    let (input, (combinator_id, constructor_number)) = preceded(multispace0, full_combinator_id)(input)?;
    let (input, _) = delimited(multispace0, tag("?"), multispace0)(input)?;
    let (input, _) = delimited(multispace0, tag("="), multispace0)(input)?;
    let (input, combinator_type) = boxed_type_ident(input)?;
    let (input, _) = preceded(multispace0, tag(";"))(input)?;

    Ok((input, Combinator { id: combinator_id, r#type: combinator_type, builtin: true, constructor_number }))
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Combinator {
        fn new(name: &str, r#type: &str) -> Self {
            Self { id: name.to_owned(), r#type: r#type.to_owned(), builtin: false, constructor_number: None }
        }

        fn builtin(name: &str, r#type: &str) -> Self {
            Self { id: name.to_owned(), r#type: r#type.to_owned(), builtin: true, constructor_number: None }
        }

        fn set_constructor_number(mut self, constructor_number: ConstructorNumber) -> Self {
            self.constructor_number.replace(constructor_number);

            self
        }
    }

    #[test]
    fn lc_ident_test() {
        let input = "input";

        let output = lc_ident(input);

        assert_eq!(output, Ok(("", "input".to_owned())));
    }

    #[test]
    fn uc_ident_test() {
        let input = "Input";

        let output = uc_ident(input);

        assert_eq!(output, Ok(("", "Input".to_owned())));
    }

    #[test]
    fn lc_ident_ns_test() {
        let input = "namespace.input";

        let output = lc_ident_ns(input);

        assert_eq!(output, Ok(("", "namespace.input".to_owned())));
    }

    #[test]
    fn uc_ident_ns_test() {
        let input = "namespace.Input";

        let output = uc_ident_ns(input);

        assert_eq!(output, Ok(("", "namespace.Input".to_owned())));
    }

    #[test]
    fn lc_ident_ns_partial_test() {
        let input = "input";

        let output = lc_ident_ns(input);

        assert_eq!(output, Ok(("", "input".to_owned())));
    }

    #[test]
    fn lc_ident_full_test() {
        let input = "input#a8509bda";

        let output = lc_ident_full(input);

        assert_eq!(output, Ok(("", ("input".to_owned(), Some(2823855066)))));
    }

    #[test]
    fn full_combinator_id_skip_test() {
        let input = "_";

        let output = full_combinator_id(input);

        assert_eq!(output, Ok(("", ("_".to_owned(), None))));
    }

    #[test]
    fn full_combinator_id_test() {
        let input = "input#a8509bda";

        let output = full_combinator_id(input);

        assert_eq!(output, Ok(("", ("input".to_owned(), Some(2823855066)))));
    }

    #[test]
    fn combinator_decl_test() {
        let input = "null = Null;";

        let output = combinator_decl(input);

        assert_eq!(output, Ok(("", Combinator::new("null", "Null"))));
    }

    #[test]
    fn single_line_comment_test() {
        let input = "// comment";

        let output = single_line_comment(input);

        assert_eq!(output, Ok(("", " comment")));
    }

    #[test]
    fn multi_line_comment_test() {
        let input = "/* multi
        line
comment */";

        let output = multi_line_comment(input);

        assert_eq!(output, Ok(("", " multi
        line
comment ")));
    }

    #[test]
    fn empty_input() {
        let input = "";

        let output = parse(input).unwrap();

        assert_eq!(output, vec![]);
    }

    #[test]
    fn boolean() {
        let input = "
boolFalse = Bool;
boolTrue = Bool;
";

        let output = parse(input).unwrap();

        assert_eq!(output, vec![
            Combinator::new("boolFalse", "Bool"),
            Combinator::new("boolTrue", "Bool")
        ]);
    }

    #[test]
    fn builtin() {
        let input = "int#a8509bda ? = Int;
long ? = Long;
double ? = Double;
string ? = String;";

        let output = parse(input).unwrap();

        assert_eq!(output, vec![
            Combinator::builtin("int", "Int").set_constructor_number(2823855066),
            Combinator::builtin("long", "Long"),
            Combinator::builtin("double", "Double"),
            Combinator::builtin("string", "String")]);
    }

    #[test]
    fn comments() {
        let input = "/////
//
// Common Types
//
/////

// Built-in types
int ? = Int;
long ? = Long;
double ? = Double;
string ? = String;

/* multi
    line
comment */

// Boolean emulation
boolFalse = Bool;
boolTrue = Bool;";

        let output = parse(input).unwrap();

        assert_eq!(output, vec![
            Combinator::builtin("int", "Int"),
            Combinator::builtin("long", "Long"),
            Combinator::builtin("double", "Double"),
            Combinator::builtin("string", "String"),
            Combinator::new("boolFalse", "Bool"),
            Combinator::new("boolTrue", "Bool")]
        );
    }
}
