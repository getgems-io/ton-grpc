use crate::FieldType::{Plain, Repetition};
use anyhow::{anyhow, bail};
use nom::branch::alt;
use nom::bytes::complete::{tag, take_till, take_until, take_while, take_while1, take_while_m_n};
use nom::character::complete::{line_ending, multispace0, multispace1, satisfy, space0};
use nom::combinator::{map, opt, recognize};
use nom::error::{Error, ErrorKind};
use nom::multi::{many0, many1, separated_list1};
use nom::sequence::{delimited, pair, preceded, separated_pair, terminated};
use nom::{AsChar, Parser};

pub type ConstructorNumber = u32;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Combinator {
    functional: bool,
    builtin: bool,
    id: String,
    r#type: String,
    constructor_number: Option<ConstructorNumber>,
    optional_fields: Vec<OptionalField>,
    fields: Vec<Field>,
}

impl Combinator {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn result_type(&self) -> &str {
        &self.r#type
    }

    pub fn fields(&self) -> &Vec<Field> {
        &self.fields
    }

    pub fn is_functional(&self) -> bool {
        self.functional
    }

    pub fn is_builtin(&self) -> bool {
        self.builtin
    }

    pub fn constructor_number_form(&self) -> String {
        let optional = self
            .optional_fields
            .iter()
            .map(OptionalField::constructor_number_form)
            .collect::<Vec<_>>()
            .join(" ");

        let fields = self
            .fields
            .iter()
            .map(Field::constructor_number_form)
            .collect::<Vec<_>>()
            .join(" ");

        let lhs = vec![self.id.as_str(), optional.as_str(), fields.as_str()]
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        format!("{} = {}", lhs, self.result_type())
    }

    pub fn constructor_number_be(&self) -> u32 {
        self.constructor_number
            .unwrap_or_else(|| crc32fast::hash(self.constructor_number_form().as_bytes()))
            .to_be()
    }

    pub fn constructor_number_le(&self) -> u32 {
        self.constructor_number
            .unwrap_or_else(|| crc32fast::hash(self.constructor_number_form().as_bytes()))
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Condition {
    pub field_ref: String,
    pub bit_selector: Option<u32>,
}

impl Condition {
    pub fn constructor_number_form(&self) -> String {
        match self.bit_selector {
            None => self.field_ref.clone(),
            Some(bit_selector) => {
                format!("{}.{}", self.field_ref, bit_selector)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum FieldType {
    Plain {
        name: String,
        condition: Option<Condition>,
    },
    Repetition {
        multiplicity: Option<String>,
        fields: Vec<Field>,
    },
}

impl FieldType {
    pub fn constructor_number_form(&self) -> String {
        match self {
            Plain {
                name,
                condition: None,
            } => name.to_string(),
            Plain {
                name,
                condition: Some(condition),
            } => {
                format!("{}?{}", condition.constructor_number_form(), name)
            }
            Repetition {
                multiplicity: None,
                fields,
            } => {
                let fields = fields
                    .iter()
                    .map(Field::constructor_number_form)
                    .collect::<Vec<_>>()
                    .join(" ");

                format!("[ {fields} ]")
            }
            Repetition {
                multiplicity: Some(multiplicity),
                fields,
            } => {
                let fields = fields
                    .iter()
                    .map(Field::constructor_number_form)
                    .collect::<Vec<_>>()
                    .join(" ");

                format!("{multiplicity}*[ {fields} ]")
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Field {
    name: Option<String>,
    r#type: FieldType,
    exclamation_point_modifier: bool,
}

// TODO[akostylev0] TypeDefinition
impl Field {
    pub fn id(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn field_type(&self) -> Option<&str> {
        let Plain { name, .. } = &self.r#type else {
            return None;
        };

        if !self.type_is_polymorphic() {
            return Some(name.as_str());
        }

        if let Some((left, _)) = name.split_once('<') {
            return Some(left);
        }

        let (left, _) = name.split_once(' ').unwrap();
        Some(left)
    }

    pub fn type_is_optional(&self) -> bool {
        let Plain { condition, .. } = &self.r#type else {
            return false;
        };

        condition.is_some()
    }

    pub fn type_condition(&self) -> Option<&Condition> {
        let Plain { ref condition, .. } = self.r#type else {
            return None;
        };

        condition.as_ref()
    }

    pub fn type_is_polymorphic(&self) -> bool {
        let Plain { name, .. } = &self.r#type else {
            return false;
        };

        name.contains('<') || name.contains(' ')
    }

    pub fn type_variables(&self) -> Option<Vec<String>> {
        let Plain { name, .. } = &self.r#type else {
            return None;
        };

        if name.contains(' ') {
            let Some((_, tail)) = name.split_once(' ') else {
                return Some(vec![]);
            };

            Some(tail.split(' ').map(|s| s.trim().to_owned()).collect())
        } else {
            let Some((_, tail)) = name.split_once('<') else {
                return Some(vec![]);
            };

            Some(
                tail.replace('>', "")
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .collect(),
            )
        }
    }

    pub fn constructor_number_form(&self) -> String {
        match &self.name {
            None => self.r#type.constructor_number_form().to_string(),
            Some(name) => {
                format!("{}:{}", name, self.r#type.constructor_number_form())
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct OptionalField {
    name: String,
    r#type: String,
}

impl OptionalField {
    pub fn constructor_number_form(&self) -> String {
        format!("{}:{}", self.name, self.r#type)
    }
}

pub fn parse(input: &str) -> anyhow::Result<Vec<Combinator>> {
    let mut input = input;
    let mut collect = Vec::new();

    loop {
        let prev = input;
        let types;
        let funcs;
        (input, types) = opt(preceded(
            opt(delimited(
                opt(space_or_comment),
                tag("---types---"),
                opt(space_or_comment),
            )),
            many0(delimited(
                opt(space_or_comment),
                alt((combinator_decl, builtin_combinator_decl)),
                opt(space_or_comment),
            )),
        ))
        .parse(input)
        .map_err(|e| anyhow!("parse error: {}", e))?;

        if let Some(types) = types {
            collect.extend(types)
        }

        (input, funcs) = opt(preceded(
            opt(delimited(
                opt(space_or_comment),
                tag("---functions---"),
                opt(space_or_comment),
            )),
            many0(delimited(
                opt(space_or_comment),
                alt((functional_combinator_decl, builtin_combinator_decl)),
                opt(space_or_comment),
            )),
        ))
        .parse(input)
        .map_err(|e: nom::Err<Error<&str>>| anyhow!("parse error: {}", e))?;

        if let Some(funcs) = funcs {
            collect.extend(funcs);
        }

        if input.is_empty() {
            return Ok(collect);
        }

        if prev == input {
            bail!("infinity loop")
        }
    }
}

fn is_lc_letter(c: char) -> bool {
    c.is_ascii_lowercase()
}

fn is_uc_letter(c: char) -> bool {
    c.is_ascii_uppercase()
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

fn is_hex_digit(c: char) -> bool {
    c.is_hex_digit()
}

fn is_underscore(c: char) -> bool {
    c == '_'
}

fn is_letter(c: char) -> bool {
    is_lc_letter(c) || is_uc_letter(c)
}

fn is_ident_char(c: char) -> bool {
    is_letter(c) || is_digit(c) || is_underscore(c)
}

fn single_line_comment(input: &str) -> nom::IResult<&str, &str> {
    preceded(tag("//"), take_till(|c| c == '\n')).parse(input)
}

fn multi_line_comment(input: &str) -> nom::IResult<&str, &str> {
    delimited(tag("/*"), take_until("*/"), tag("*/")).parse(input)
}

fn space_or_comment(input: &str) -> nom::IResult<&str, ()> {
    let (input, _) = many0(alt((
        multispace1,
        single_line_comment,
        multi_line_comment,
        line_ending,
    )))
    .parse(input)?;

    Ok((input, ()))
}

fn lc_ident(input: &str) -> nom::IResult<&str, String> {
    let (input, head) = satisfy(is_lc_letter)(input)?;
    let (input, tail) = take_while(is_ident_char)(input)?;

    Ok((input, format!("{head}{tail}")))
}

fn uc_ident(input: &str) -> nom::IResult<&str, String> {
    let (input, head) = satisfy(is_uc_letter)(input)?;
    let (input, tail) = take_while(is_ident_char)(input)?;

    Ok((input, format!("{head}{tail}")))
}

fn namespace_ident(input: &str) -> nom::IResult<&str, String> {
    lc_ident(input)
}

fn lc_ident_ns(input: &str) -> nom::IResult<&str, String> {
    map(separated_list1(tag("."), lc_ident), |vec| vec.join(".")).parse(input)
}

fn uc_ident_ns(input: &str) -> nom::IResult<&str, String> {
    let (input, ns) = opt(terminated(
        separated_list1(tag("."), namespace_ident),
        tag("."),
    ))
    .parse(input)?;
    let (input, head) = uc_ident(input)?;

    match ns {
        None => Ok((input, head)),
        Some(ns) => Ok((input, format!("{}.{}", ns.join("."), head))),
    }
}

fn lc_ident_full(input: &str) -> nom::IResult<&str, (String, Option<ConstructorNumber>)> {
    let (input, ident) = lc_ident_ns(input)?;
    let (input, combinator_number) =
        opt(preceded(tag("#"), take_while_m_n(8, 8, is_hex_digit))).parse(input)?;

    match combinator_number {
        None => Ok((input, (ident, None))),
        Some(combinator_number) => {
            let combinator_number = ConstructorNumber::from_str_radix(combinator_number, 16)
                .expect("invalid combinator number");

            Ok((input, (ident, Some(combinator_number))))
        }
    }
}

fn full_combinator_id(input: &str) -> nom::IResult<&str, (String, Option<ConstructorNumber>)> {
    alt((lc_ident_full, map(tag("_"), |s: &str| (s.to_owned(), None)))).parse(input)
}

fn boxed_type_ident(input: &str) -> nom::IResult<&str, String> {
    uc_ident_ns(input)
}

fn result_type(input: &str) -> nom::IResult<&str, (bool, String)> {
    let (input, exclamation) = opt(tag("!")).parse(input)?;
    let (input, ident) = boxed_type_ident(input)?;
    let (input, exprs) = many0(delimited(space0, subexpr, space_or_comment)).parse(input)?;

    if !exprs.is_empty() {
        Ok((
            input,
            (
                exclamation.is_some(),
                format!("{} {}", ident, exprs.join(" ")),
            ),
        ))
    } else {
        Ok((input, (exclamation.is_some(), ident)))
    }
}

fn expr(input: &str) -> nom::IResult<&str, String> {
    map(separated_list1(tag(" "), subexpr), |vs| vs.join(" ")).parse(input)
}

fn type_expr(input: &str) -> nom::IResult<&str, String> {
    expr(input)
}

fn opt_args(input: &str) -> nom::IResult<&str, Vec<OptionalField>> {
    let (input, names) = preceded(
        tag("{"),
        many1(delimited(space0, var_ident, space_or_comment)),
    )
    .parse(input)?;
    let (input, _) = delimited(space0, tag(":"), space_or_comment).parse(input)?;
    let (input, type_name) = terminated(type_expr, tag("}")).parse(input)?;

    Ok((
        input,
        names
            .into_iter()
            .map(|name| OptionalField {
                name,
                r#type: type_name.clone(),
            })
            .collect(),
    ))
}

fn combinator_decl(input: &str) -> nom::IResult<&str, Combinator> {
    let (input, (combinator_id, constructor_number)) =
        preceded(multispace0, full_combinator_id).parse(input)?;
    let (input, opts) = opt(delimited(space0, opt_args, space_or_comment)).parse(input)?;
    let (input, fields) = many0(delimited(space0, args, space_or_comment)).parse(input)?;
    let (input, _) = delimited(multispace0, tag("="), space_or_comment).parse(input)?;
    let (input, (functional, combinator_type)) = result_type(input)?;
    let (input, _) = preceded(multispace0, tag(";")).parse(input)?;

    Ok((
        input,
        Combinator {
            id: combinator_id,
            r#type: combinator_type,
            builtin: false,
            constructor_number,
            fields: fields.into_iter().flatten().collect(),
            optional_fields: opts.unwrap_or_default(),
            functional,
        },
    ))
}

fn functional_combinator_decl(input: &str) -> nom::IResult<&str, Combinator> {
    let (input, (combinator_id, constructor_number)) =
        preceded(multispace0, full_combinator_id).parse(input)?;
    let (input, opts) =
        opt(delimited(space_or_comment, opt_args, space_or_comment)).parse(input)?;
    let (input, fields) =
        many0(delimited(space_or_comment, args, space_or_comment)).parse(input)?;
    let (input, _) = delimited(multispace0, tag("="), multispace0).parse(input)?;
    let (input, (_, combinator_type)) = result_type(input)?;
    let (input, _) = preceded(multispace0, tag(";")).parse(input)?;

    Ok((
        input,
        Combinator {
            id: combinator_id,
            r#type: combinator_type,
            builtin: false,
            constructor_number,
            fields: fields.into_iter().flatten().collect(),
            optional_fields: opts.unwrap_or_default(),
            functional: true,
        },
    ))
}

fn builtin_combinator_decl(input: &str) -> nom::IResult<&str, Combinator> {
    let (input, (combinator_id, constructor_number)) =
        preceded(multispace0, full_combinator_id).parse(input)?;
    let (input, _) = delimited(multispace0, tag("?"), space_or_comment).parse(input)?;
    let (input, _) = delimited(multispace0, tag("="), space_or_comment).parse(input)?;
    let (input, combinator_type) = boxed_type_ident(input)?;
    let (input, _) = preceded(multispace0, tag(";")).parse(input)?;

    Ok((
        input,
        Combinator {
            id: combinator_id,
            r#type: combinator_type,
            builtin: true,
            constructor_number,
            fields: vec![],
            optional_fields: vec![],
            functional: false,
        },
    ))
}

fn var_ident(input: &str) -> nom::IResult<&str, String> {
    alt((lc_ident, uc_ident)).parse(input)
}

fn var_ident_opt(input: &str) -> nom::IResult<&str, String> {
    alt((var_ident, map(tag("_"), |s: &str| s.to_owned()))).parse(input)
}

fn nat_const(input: &str) -> nom::IResult<&str, &str> {
    take_while1(is_digit)(input)
}

fn conditional_def(input: &str) -> nom::IResult<&str, Condition> {
    let (input, field_ref) = var_ident(input)?;
    let (input, bit_selector) = opt(preceded(tag("."), nat_const)).parse(input)?;
    let bit_selector = bit_selector
        .map(|n| n.parse::<u32>())
        .transpose()
        .map_err(|_| nom::Err::Failure(Error::new(input, ErrorKind::Fail)))?;
    let (input, _) = tag("?")(input)?;

    Ok((
        input,
        Condition {
            field_ref,
            bit_selector,
        },
    ))
}

fn subexpr(input: &str) -> nom::IResult<&str, String> {
    alt((
        term,
        map(
            many1(map(
                separated_pair(
                    alt((term, map(nat_const, |s: &str| s.to_owned()))),
                    map(tag("+"), |s: &str| s.to_owned()),
                    alt((term, map(nat_const, |s: &str| s.to_owned()))),
                ),
                |(s1, s2)| format!("{s1} + {s2}"),
            )),
            |vs: Vec<String>| vs.join("+"),
        ),
    ))
    .parse(input)
}

fn type_ident(input: &str) -> nom::IResult<&str, String> {
    alt((
        boxed_type_ident,
        lc_ident_ns,
        map(tag("#"), |s: &str| s.to_owned()),
    ))
    .parse(input)
}

fn term(input: &str) -> nom::IResult<&str, String> {
    alt((
        delimited(tag("("), expr, tag(")")),
        map(
            recognize(pair(
                type_ident,
                delimited(tag("<"), separated_list1(tag(","), type_ident), tag(">")),
            )),
            |s| s.to_owned(),
        ),
        type_ident,
        var_ident,
        map(nat_const, |s| s.to_owned()),
        preceded(tag("%"), term),
    ))
    .parse(input)
}

fn type_term(input: &str) -> nom::IResult<&str, (bool, String)> {
    pair(map(opt(tag("!")), |s| s.is_some()), term).parse(input)
}

fn args_1(input: &str) -> nom::IResult<&str, Vec<Field>> {
    let (input, id) = var_ident_opt(input)?;
    let (input, _) = tag(":")(input)?;
    let (input, condition) = opt(conditional_def).parse(input)?;
    let (input, (exclamation_point_modifier, name)) = type_term(input)?;

    Ok((
        input,
        vec![Field {
            name: Some(id),
            r#type: Plain { name, condition },
            exclamation_point_modifier,
        }],
    ))
}

fn nat_term(input: &str) -> nom::IResult<&str, String> {
    term(input)
}

fn multiplicity(input: &str) -> nom::IResult<&str, String> {
    terminated(nat_term, tag("*")).parse(input)
}

fn args_2(input: &str) -> nom::IResult<&str, Vec<Field>> {
    let (input, id) = opt(terminated(var_ident_opt, tag(":"))).parse(input)?;
    let (input, multiplicity) = opt(multiplicity).parse(input)?;
    let (input, _) = tag("[")(input)?;
    let (input, fields) = many1(delimited(space0, args, space0)).parse(input)?;
    let (input, _) = tag("]")(input)?;

    Ok((
        input,
        vec![Field {
            name: id,
            r#type: Repetition {
                multiplicity,
                fields: fields.into_iter().flatten().collect(),
            },
            exclamation_point_modifier: false,
        }],
    ))
}

fn args_3(input: &str) -> nom::IResult<&str, Vec<Field>> {
    let (input, _) = tag("(")(input)?;
    let (input, fields) = many1(delimited(space0, var_ident_opt, space0)).parse(input)?;
    let (input, _) = tag(":")(input)?;
    let (input, (exclamation_point_modifier, type_term)) = type_term(input)?;
    let (input, _) = tag(")")(input)?;

    Ok((
        input,
        fields
            .into_iter()
            .map(|id| Field {
                name: Some(id),
                r#type: Plain {
                    name: type_term.clone(),
                    condition: None,
                },
                exclamation_point_modifier,
            })
            .collect(),
    ))
}

fn args_4(input: &str) -> nom::IResult<&str, Vec<Field>> {
    let (input, (exclamation_point_modifier, type_term)) = type_term(input)?;

    Ok((
        input,
        vec![Field {
            name: None,
            r#type: Plain {
                name: type_term,
                condition: None,
            },
            exclamation_point_modifier,
        }],
    ))
}

fn args(input: &str) -> nom::IResult<&str, Vec<Field>> {
    alt((args_1, args_2, args_3, args_4)).parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Combinator {
        fn new(name: &str, r#type: &str) -> Self {
            Self {
                id: name.to_owned(),
                r#type: r#type.to_owned(),
                builtin: false,
                constructor_number: None,
                fields: vec![],
                optional_fields: vec![],
                functional: false,
            }
        }

        fn builtin(name: &str, r#type: &str) -> Self {
            Self {
                id: name.to_owned(),
                r#type: r#type.to_owned(),
                builtin: true,
                constructor_number: None,
                fields: vec![],
                optional_fields: vec![],
                functional: false,
            }
        }

        fn functional(mut self) -> Self {
            self.functional = true;

            self
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
        fn plain(name: &str, r#type: &str) -> Self {
            Self {
                name: Some(name.to_owned()),
                r#type: Plain {
                    name: r#type.to_owned(),
                    condition: None,
                },
                exclamation_point_modifier: false,
            }
        }

        fn unnamed_plain(r#type: &str) -> Self {
            Self {
                name: None,
                r#type: Plain {
                    name: r#type.to_owned(),
                    condition: None,
                },
                exclamation_point_modifier: false,
            }
        }

        fn repetition(
            name: Option<String>,
            multiplicity: Option<String>,
            fields: Vec<Field>,
        ) -> Self {
            Self {
                name,
                r#type: Repetition {
                    multiplicity,
                    fields,
                },
                exclamation_point_modifier: false,
            }
        }
    }

    impl OptionalField {
        fn new(name: &str, r#type: &str) -> Self {
            OptionalField {
                name: name.to_owned(),
                r#type: r#type.to_owned(),
            }
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

        assert_eq!(
            output,
            Ok((
                "",
                " multi
        line
comment "
            ))
        );
    }

    #[test]
    fn args_1_test() {
        let input = "first_name:fields.0?string";

        let output = args_1(input);

        assert_eq!(
            output,
            Ok((
                "",
                vec![Field {
                    name: Some("first_name".to_owned()),
                    r#type: Plain {
                        name: "string".to_owned(),
                        condition: Some(Condition {
                            field_ref: "fields".to_string(),
                            bit_selector: Some(0)
                        })
                    },
                    exclamation_point_modifier: false
                }]
            ))
        );
    }

    #[test]
    fn args_2_test() {
        let input = "a:m*[n*[double]]";

        let output = args_2(input);

        assert_eq!(
            output,
            Ok((
                "",
                vec![Field {
                    name: Some("a".to_owned()),
                    r#type: Repetition {
                        multiplicity: Some("m".to_owned()),
                        fields: vec![Field {
                            name: None,
                            r#type: Repetition {
                                multiplicity: Some("n".to_owned()),
                                fields: vec![Field {
                                    name: None,
                                    r#type: Plain {
                                        name: "double".to_owned(),
                                        condition: None
                                    },
                                    exclamation_point_modifier: false
                                }],
                            },
                            exclamation_point_modifier: false
                        }],
                    },
                    exclamation_point_modifier: false
                }]
            ))
        );
    }

    #[test]
    fn args_3_test() {
        let input = "(x y z:int32)";

        let output = args_3(input);

        assert_eq!(
            output,
            Ok((
                "",
                vec![
                    Field {
                        name: Some("x".to_string()),
                        r#type: Plain {
                            name: "int32".to_string(),
                            condition: None
                        },
                        exclamation_point_modifier: false
                    },
                    Field {
                        name: Some("y".to_string()),
                        r#type: Plain {
                            name: "int32".to_string(),
                            condition: None
                        },
                        exclamation_point_modifier: false
                    },
                    Field {
                        name: Some("z".to_string()),
                        r#type: Plain {
                            name: "int32".to_string(),
                            condition: None
                        },
                        exclamation_point_modifier: false
                    },
                ]
            ))
        );
    }

    #[test]
    fn args_4_test() {
        let input = "double";

        let output = args_4(input);

        assert_eq!(
            output,
            Ok((
                "",
                vec![Field {
                    name: None,
                    r#type: Plain {
                        name: "double".to_owned(),
                        condition: None
                    },
                    exclamation_point_modifier: false
                }]
            ))
        );
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

        assert_eq!(
            output,
            vec![
                Combinator::new("boolFalse", "Bool"),
                Combinator::new("boolTrue", "Bool"),
            ]
        );
    }

    #[test]
    fn builtin() {
        let input = "int#a8509bda ? = Int;
long ? = Long;
double ? = Double;
string ? = String;";

        let output = parse(input).unwrap();

        assert_eq!(
            output,
            vec![
                Combinator::builtin("int", "Int").with_constructor_number(2823855066),
                Combinator::builtin("long", "Long"),
                Combinator::builtin("double", "Double"),
                Combinator::builtin("string", "String")
            ]
        );
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

        assert_eq!(
            output,
            vec![
                Combinator::builtin("int", "Int"),
                Combinator::builtin("long", "Long"),
                Combinator::builtin("double", "Double"),
                Combinator::builtin("string", "String"),
                Combinator::new("boolFalse", "Bool"),
                Combinator::new("boolTrue", "Bool")
            ]
        );
    }

    #[test]
    fn bool_stat() {
        let input = "// Boolean for diagonal queries
boolStat statTrue:int statFalse:int statUnknown:int = BoolStat;";

        let output = parse(input).unwrap();

        assert_eq!(
            output,
            vec![Combinator::new("boolStat", "BoolStat").with_fields(vec![
                Field::plain("statTrue", "int"),
                Field::plain("statFalse", "int"),
                Field::plain("statUnknown", "int"),
            ])]
        );
    }

    #[test]
    fn type_term_hash_test() {
        let input = "#";

        let output = type_term(input).unwrap();

        assert_eq!(output, ("", (false, "#".to_owned())))
    }

    #[test]
    fn type_term_marked_hash_test() {
        let input = "!A";

        let output = type_term(input).unwrap();

        assert_eq!(output, ("", (true, "A".to_owned())))
    }

    #[test]
    fn vector_test() {
        let input = "vector {t:Type} # [ t ] = Vector t;";

        let output = parse(input).unwrap();

        assert_eq!(
            output,
            vec![Combinator::new("vector", "Vector t")
                .with_optional_fields(vec![OptionalField::new("t", "Type")])
                .with_fields(vec![
                    Field::unnamed_plain("#"),
                    Field::repetition(None, None, vec![Field::unnamed_plain("t")]),
                ])]
        );
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

    #[test]
    fn field_vector_of_test() {
        let input = "exportedKey word_list:vector<secureString> = ExportedKey;";

        let output = parse(input).unwrap();

        assert_eq!(
            output,
            vec![Combinator::new("exportedKey", "ExportedKey")
                .with_fields(vec![Field::plain("word_list", "vector<secureString>")])]
        );
    }

    #[test]
    fn field_vector_of_test_spaces() {
        let input = "smc.libraryResult result:(vector smc.libraryEntry) = smc.LibraryResult;";

        let output = parse(input).unwrap();

        assert_eq!(
            output,
            vec![Combinator::new("smc.libraryResult", "smc.LibraryResult")
                .with_fields(vec![Field::plain("result", "vector smc.libraryEntry")])]
        );
    }

    #[test]
    fn functional_combinator_test() {
        let input = "a = A;
c = !C;
---functions---
b = B;
d = !D;";

        let output = parse(input).unwrap();

        assert_eq!(
            output,
            vec![
                Combinator::new("a", "A"),
                Combinator::new("c", "C").functional(),
                Combinator::new("b", "B").functional(),
                Combinator::new("d", "D").functional()
            ]
        );
    }

    #[test]
    fn functional_combinator_types_test() {
        let input = "a = A;
---functions---
---types---
c = !C;
---functions---
d = !D;
";

        let output = parse(input).unwrap();

        assert_eq!(
            output,
            vec![
                Combinator::new("a", "A"),
                Combinator::new("c", "C").functional(),
                Combinator::new("d", "D").functional()
            ]
        );
    }

    #[test]
    fn functional_combinator_ok_test() {
        let input = "ok = Ok;";

        let output = parse(input).unwrap();

        assert_eq!(output, vec![Combinator::new("ok", "Ok"),]);
    }

    #[test]
    fn vector_parse_test() {
        let input = "blocks.shardBlockProof from:ton.blockIdExt mc_id:ton.blockIdExt links:(vector blocks.shardBlockLink) mc_proof:(vector blocks.blockLinkBack) = blocks.ShardBlockProof;";

        let output = parse(input).unwrap();

        assert_eq!(
            output,
            vec![
                Combinator::new("blocks.shardBlockProof", "blocks.ShardBlockProof").with_fields(
                    vec![
                        Field::plain("from", "ton.blockIdExt"),
                        Field::plain("mc_id", "ton.blockIdExt"),
                        Field::plain("links", "vector blocks.shardBlockLink"),
                        Field::plain("mc_proof", "vector blocks.blockLinkBack"),
                    ]
                ),
            ]
        );
    }

    #[test]
    fn parse_multiline_test() {
        let input = "storage.daemon.getTorrentPiecesInfo hash:int256
    flags:# // 0 - with file ranges
    offset:long max_pieces:long
    = storage.daemon.TorrentPiecesInfo;";

        let output = parse(input).unwrap();

        assert_eq!(
            output,
            vec![Combinator::new(
                "storage.daemon.getTorrentPiecesInfo",
                "storage.daemon.TorrentPiecesInfo"
            )
            .with_fields(vec![
                Field::plain("hash", "int256"),
                Field::plain("flags", "#"),
                Field::plain("offset", "long"),
                Field::plain("max_pieces", "long"),
            ]),]
        );
    }

    #[test]
    fn parse_ping_crc32() {
        let input = "tcp.ping random_id:long = tcp.Pong;";

        let output = parse(input).unwrap();

        assert_eq!(
            output[0].constructor_number_form(),
            "tcp.ping random_id:long = tcp.Pong"
        );
        assert_eq!(output[0].constructor_number_be(), 0x9a2b084d);
        assert_eq!(
            output,
            vec![Combinator::new("tcp.ping", "tcp.Pong")
                .with_fields(vec![Field::plain("random_id", "long")])]
        );
    }

    #[test]
    fn vector_constructor_number_form() {
        let input = "vector {t:Type} # [ t ] = Vector t;";

        let output = parse(input).unwrap();

        assert_eq!(
            output[0].constructor_number_form(),
            "vector t:Type # [ t ] = Vector t"
        );
        assert_eq!(output[0].constructor_number_le(), 0x1cb5c415);
    }

    #[test]
    fn liteserver_query_constructor_number_form() {
        let input = "liteServer.query data:bytes = Object;";

        let output = parse(input).unwrap();

        assert_eq!(
            output[0].constructor_number_form(),
            "liteServer.query data:bytes = Object"
        );
        assert_eq!(output[0].constructor_number_be(), 0xdf068c79);
    }

    #[test]
    fn adnl_query_constructor_number_form() {
        let input = "adnl.message.query query_id:int256 query:bytes = adnl.Message;";

        let output = parse(input).unwrap();

        assert_eq!(
            output[0].constructor_number_form(),
            "adnl.message.query query_id:int256 query:bytes = adnl.Message"
        );
        assert_eq!(output[0].constructor_number_be(), 0x7af98bb4);
    }
}
