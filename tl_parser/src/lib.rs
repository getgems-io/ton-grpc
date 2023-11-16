use anyhow::anyhow;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, take_till, take_until, take_while, take_while1, take_while_m_n};
use nom::character::complete::{char, line_ending, multispace0, multispace1, newline, satisfy, space0};
use nom::combinator::{map, opt, recognize, value};
use nom::error::Error;
use nom::multi::{fold_many0, many0, many1, separated_list1};
use nom::Parser;
use nom::sequence::{delimited, pair, preceded, separated_pair, terminated};
use crate::FieldType::{Bare, ConditionalField, Repetition};

pub type ConstructorNumber = u32;

#[derive(Debug, PartialEq, Eq)]
pub struct Combinator {
    builtin: bool,
    id: String,
    r#type: String,
    constructor_number: Option<ConstructorNumber>,
    optional_fields: Vec<OptionalField>,
    fields: Vec<Field>
}

#[derive(Debug, PartialEq, Eq)]
enum FieldType {
    ConditionalField {
        condition: String,
        name: String
    },
    Repetition {
        multiplicity: Option<String>,
        fields: Vec<Field>,
    },
    Bare {
        name: String
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Field {
    name: Option<String>,
    r#type: FieldType,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OptionalField {
    name: String,
    r#type: String
}

pub fn parse(input: &str) -> anyhow::Result<Vec<Combinator>> {
    let (input, vecs) = many0(
        delimited(opt(space_or_comment), alt((combinator_decl, builtin_combinator_decl)), opt(space_or_comment))
    )(input).map_err(|e| anyhow!("parse error: {}", e))?;

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
    let (input, tail) = take_while(is_ident_char)(input)?;

    Ok((input, format!("{}{}", head, tail)))
}

fn uc_ident(input: &str) -> nom::IResult<&str, String> {
    let (input, head) = satisfy(is_uc_letter)(input)?;
    let (input, tail) = take_while(is_ident_char)(input)?;

    Ok((input, format!("{}{}", head, tail)))
}

fn namespace_ident(input: &str) -> nom::IResult<&str, String> { lc_ident(input) }

fn lc_ident_ns(input: &str) -> nom::IResult<&str, String> {
    map(separated_list1(tag("."), lc_ident), |vec| vec.join("."))(input)
}

fn uc_ident_ns(input: &str) -> nom::IResult<&str, String> {
    let (input, ns) = opt(terminated(separated_list1(tag("."), namespace_ident), tag(".")))(input)?;
    let (input, head) = uc_ident(input)?;

    match ns {
        None => Ok((input, head)),
        Some(ns) => Ok((input, format!("{}.{}", ns.join("."), head)))
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
    let (input, exprs) = many0(delimited(space0, subexpr, space0))(input)?;

    if !exprs.is_empty() {
        Ok((input, format!("{} {}", ident, exprs.join(" "))))
    } else {
        Ok((input, ident))
    }
}

fn expr(input: &str) -> nom::IResult<&str, String> {
    map(
        many0(delimited(space0, subexpr, space0)),
        |vs| vs.join(" ")
    )(input)
}

fn type_expr(input: &str) -> nom::IResult<&str, String> {
    expr(input)
}

fn opt_args(input: &str) -> nom::IResult<&str, Vec<OptionalField>> {
    let (input, names) = preceded(tag("{"), many1(delimited(space0, var_ident, space0)))(input)?;
    let (input, _) = delimited(space0, tag(":"), space0)(input)?;
    let (input, type_name) = terminated(type_expr, tag("}"))(input)?;

    Ok((input, names.into_iter().map(|name| OptionalField { name, r#type: type_name.clone() }).collect()))
}

fn combinator_decl(input: &str) -> nom::IResult<&str, Combinator> {
    let (input, (combinator_id, constructor_number)) = preceded(multispace0, full_combinator_id)(input)?;
    let (input, opts) = opt(delimited(space0, opt_args, space0))(input)?;
    let (input, fields) = many0(delimited(space0, args, space0))(input)?;
    let (input, _) = delimited(multispace0, tag("="), multispace0)(input)?;
    let (input, combinator_type) = result_type(input)?;
    let (input, _) = preceded(multispace0, tag(";"))(input)?;

    Ok((input, Combinator { id: combinator_id, r#type: combinator_type, builtin: false, constructor_number, fields, optional_fields: opts.unwrap_or_default() }))
}

fn builtin_combinator_decl(input: &str) -> nom::IResult<&str, Combinator> {
    let (input, (combinator_id, constructor_number)) = preceded(multispace0, full_combinator_id)(input)?;
    let (input, _) = delimited(multispace0, tag("?"), multispace0)(input)?;
    let (input, _) = delimited(multispace0, tag("="), multispace0)(input)?;
    let (input, combinator_type) = boxed_type_ident(input)?;
    let (input, _) = preceded(multispace0, tag(";"))(input)?;

    Ok((input, Combinator { id: combinator_id, r#type: combinator_type, builtin: true, constructor_number, fields: vec![], optional_fields: vec![] }))
}

fn var_ident(input: &str) -> nom::IResult<&str, String> {
    alt((lc_ident, uc_ident))(input)
}

fn var_ident_opt(input: &str) -> nom::IResult<&str, String> {
    alt((
        var_ident,
        map(tag("_"), |s: &str| s.to_owned())
    ))(input)
}

fn nat_const(input: &str) -> nom::IResult<&str, &str> {
    take_while1(is_digit)(input)
}

fn conditional_def(input: &str) -> nom::IResult<&str, String> {
    let (input, var) = var_ident(input)?;
    let (input, opt) = opt(preceded(tag("."), nat_const))(input)?;
    let (input, _) = tag("?")(input)?;

    match opt {
        None => Ok((input, var)),
        Some(opt) => Ok((input, format!("{}.{}", var, opt)))
    }
}

fn subexpr(input: &str) -> nom::IResult<&str, String> {
    Ok(alt(
        (term, map(many1(map(separated_pair(
            alt((term, map(nat_const, |s: &str| s.to_owned()))),
            map(tag("+"), |s: &str| s.to_owned()),
            alt((term, map(nat_const, |s: &str| s.to_owned())))
        ), |(s1, s2)| format!("{} + {}", s1, s2))), |vs: Vec<String>| vs.join("+")))
    )(input)?)
}

fn type_ident(input: &str) -> nom::IResult<&str, String> {
    alt((boxed_type_ident, lc_ident_ns, map(tag("#"), |s: &str| s.to_owned())))(input)
}

fn term(input: &str) -> nom::IResult<&str, String> {
    alt((
        delimited(tag("("), subexpr, tag(")")),
        map(
            recognize(pair(type_ident, delimited(tag("<"), separated_list1(tag(","), type_ident), tag(">")))),
            |s| s.to_owned()
        ),
        type_ident,
        var_ident,
        map(nat_const, |s| s.to_owned()),
        preceded(tag("%"), term)
    ))(input)
}

fn type_term(input: &str) -> nom::IResult<&str, String> {
    term(input)
}

fn args_1(input: &str) -> nom::IResult<&str, Field> {
    let (input, id) = var_ident_opt(input)?;
    let (input, _) = tag(":")(input)?;
    let (input, conditional_def) = opt(conditional_def)(input)?;
    let (input, marker) = opt(tag("!"))(input)?;
    let (input, name) = type_term(input)?;

    match conditional_def {
        None => {
            Ok((input, Field { name: Some(id), r#type: Bare { name: name }}))
        }
        Some(condition) => {
            Ok((input, Field { name: Some(id), r#type: ConditionalField { condition, name }}))
        }
    }


}

fn nat_term(input: &str) -> nom::IResult<&str, String> {
    term(input)
}

fn multiplicity(input: &str) -> nom::IResult<&str, String> {
    terminated(nat_term, tag("*"))(input)
}
fn args_2(input: &str) -> nom::IResult<&str, Field> {
    let (input, id) = opt(terminated(var_ident_opt, tag(":")))(input)?;
    let (input, multiplicity) = opt(multiplicity)(input)?;
    let (input, _) = tag("[")(input)?;
    let (input, fields) = many1(delimited(space0, args, space0))(input)?;
    let (input, _) = tag("]")(input)?;

    Ok((input, Field { name: id, r#type: Repetition { multiplicity, fields } }))
}

fn args_3(input: &str) -> nom::IResult<&str, Vec<Field>> {
    let (input, _) = tag("(")(input)?;
    let (input, fields) = many1(delimited(space0, var_ident_opt, space0))(input)?;
    let (input, _) = tag(":")(input)?;
    let (input, mark) = opt(tag("!"))(input)?;
    let (input, type_term) = type_term(input)?;
    let (input, _) = tag(")")(input)?;

    Ok((input, fields.into_iter().map(|id| Field {
        name: Some(id),
        r#type: Bare {
            name: type_term.clone()
        }
    }).collect()))
}

fn args_4(input: &str) -> nom::IResult<&str, Field> {
    let (input, mark) = opt(tag("!"))(input)?;
    let (input, type_term) = type_term(input)?;

    Ok((input, Field { name: None, r#type: Bare { name: type_term } }))
}

fn args(input: &str) -> nom::IResult<&str, Field> {
    alt((args_1, args_2, args_4))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Combinator {
        fn new(name: &str, r#type: &str) -> Self {
            Self { id: name.to_owned(), r#type: r#type.to_owned(), builtin: false, constructor_number: None, fields: vec![], optional_fields: vec![] }
        }

        fn builtin(name: &str, r#type: &str) -> Self {
            Self { id: name.to_owned(), r#type: r#type.to_owned(), builtin: true, constructor_number: None, fields: vec![], optional_fields: vec![] }
        }

        fn with_constructor_number(mut self, constructor_number: ConstructorNumber) -> Self {
            self.constructor_number.replace(constructor_number);

            self
        }

        fn with_fields(mut self, fields: Vec<Field>) -> Self {
            self.fields = fields;

            self
        }

        fn with_optional_fields(mut self, optional_fields: Vec<OptionalField>) -> Self {
            self.optional_fields = optional_fields;

            self
        }
    }

    impl Field {
        fn bare(name: &str, r#type: &str) -> Self {
            Self { name: Some(name.to_owned()), r#type: Bare { name: r#type.to_owned() } }
        }

        fn unnamed_bare(r#type: &str) -> Self {
            Self { name: None, r#type: Bare { name: r#type.to_owned() } }
        }

        fn repetition(name: Option<String>, multiplicity: Option<String>, fields: Vec<Field>) -> Self {
            Self { name, r#type: Repetition { multiplicity, fields }}
        }
    }

    impl OptionalField {
        fn new(name: &str, r#type: &str) -> Self {
            OptionalField { name: name.to_owned(), r#type: r#type.to_owned() }
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
    fn args_1_test() {
        let input = "first_name:fields.0?string";

        let output = args_1(input);

        assert_eq!(output, Ok(("", Field { name: Some("first_name".to_owned()), r#type: ConditionalField { condition: "fields.0".to_owned(), name: "string".to_owned() }})));
    }

    #[test]
    fn args_2_test() {
        let input = "a:m*[n*[double]]";

        let output = args_2(input);

        assert_eq!(output, Ok(("", Field { name: Some("a".to_owned()), r#type: Repetition {
            multiplicity: Some("m".to_owned()),
            fields: vec![Field {
                name: None,
                r#type: Repetition {
                    multiplicity: Some("n".to_owned()),
                    fields: vec![Field {
                        name: None,
                        r#type: Bare {name: "double".to_owned()}
                    }]
                },
            }]
        }})));
    }

    #[test]
    fn args_3_test() {
        let input = "(x y z:int32)";

        let output = args_3(input);

        assert_eq!(output, Ok(("", vec![
            Field {
                name: Some("x".to_string()),
                r#type: Bare {
                    name: "int32".to_string()
                }
            },
            Field {
                name: Some("y".to_string()),
                r#type: Bare {
                    name: "int32".to_string()
                }
            },
            Field {
                name: Some("z".to_string()),
                r#type: Bare {
                    name: "int32".to_string()
                }
            }
        ])));
    }

    #[test]
    fn args_4_test() {
        let input = "double";

        let output = args_4(input);

        assert_eq!(output, Ok(("", Field { name: None, r#type: Bare { name: "double".to_owned() } })));
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
            Combinator::builtin("int", "Int").with_constructor_number(2823855066),
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

    #[test]
    fn bool_stat() {
        let input = "// Boolean for diagonal queries
boolStat statTrue:int statFalse:int statUnknown:int = BoolStat;";

        let output = parse(input).unwrap();

        assert_eq!(output, vec![
            Combinator::new("boolStat", "BoolStat")
                .with_fields(vec![
                    Field::bare("statTrue", "int"),
                    Field::bare("statFalse", "int"),
                    Field::bare("statUnknown", "int")
                ])
        ]);
    }

    #[test]
    fn type_term_hash_test() {
        let input = "#";

        let output = type_term(input).unwrap();

        assert_eq!(output, ("", "#".to_owned()))
    }

    #[test]
    fn vector_test() {
        let input = "vector {t:Type} # [ t ] = Vector t;";

        let output = parse(input).unwrap();

        assert_eq!(output, vec![
            Combinator::new("vector", "Vector t")
                .with_optional_fields(vec![
                    OptionalField::new("t", "Type")
                ])
                .with_fields(vec![
                    Field::unnamed_bare("#"),
                    Field::repetition(None, None, vec![Field::unnamed_bare("t")])
                ])
        ]);
    }

    #[test]
    fn gen_vector_test() {
        let input = "exportedKey word_list:vector<secureString> = ExportedKey;";

        let output = parse(input).unwrap();

        assert_eq!(output, vec![
            Combinator::new("exportedKey", "ExportedKey")
                .with_fields(vec![
                    Field::bare("word_list", "vector<secureString>")
                ])
        ]);
    }

    #[test]
    fn nested_lc_namespaces_test() {
        let input = "n1.n2.n3.input";

        let output = lc_ident_ns(input);

        assert_eq!(output, Ok(("", "n1.n2.n3.input".to_owned())));
    }

    #[test]
    fn nested_uc_namespaces_test() {
        let input = "n1.n2.n3.Input";

        let output = uc_ident_ns(input);

        assert_eq!(output, Ok(("", "n1.n2.n3.Input".to_owned())));
    }
}
