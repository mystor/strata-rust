#![feature(conservative_impl_trait)]
#![feature(pattern)]

#[macro_use]
extern crate peresil;

// define what you want to parse; likely a string
// create an error type
// definte type aliases
type Point<'s> = peresil::StringPoint<'s>;
type Master<'s> = peresil::ParseMaster<Point<'s>, Error>;
type Progress<'s, T> = peresil::Progress<Point<'s>, T, Error>;

// define an error type - emphasis on errors. Need to implement Recoverable (more to discuss.
#[derive(Debug)]
enum Error {
    Literal(&'static str),
    IdentNotFound,
}

impl peresil::Recoverable for Error {
    fn recoverable(&self) -> bool { true }
}

// Construct a point, initialize  the master. This is what stores errors
// todo: rename?

pub fn parse_rust_file(file: &str) {
    let mut pt = Point::new(file);
    let mut pm = Master::new();

    loop {
        let next_pt;

        let top_level = top_level(&mut pm, pt);
        let top_level = pm.finish(top_level);

        match top_level.status {
            peresil::Status::Success(s) => {
                println!("Ok {:#?}", s);
                next_pt = top_level.point;
            },
            peresil::Status::Failure(e) => {
                println!("Err {:?}", e);
                println!(">>{}<<", &file[top_level.point.offset..]);
                break;
            },
        }

        if next_pt.offset <= pt.offset {
            let end = std::cmp::min(pt.offset + 10, file.len());
            panic!("Could not make progress: {}...", &file[pt.offset..end]);
        }
        pt = next_pt;

        if pt.s.is_empty() { break }
    }

    // TODO: add `expect` to progress?
}

type Extent = (usize, usize);

#[derive(Debug)]
enum TopLevel {
    Comment(Extent),
    Function(Function),
    Enum(Enum),
    Trait(Trait),
    Impl(Impl),
    Attribute(Extent),
    ExternCrate(Crate),
    TypeAlias(TypeAlias),
    Whitespace(Extent),
}

#[derive(Debug)]
struct Function {
    extent: Extent,
    header: FunctionHeader,
    body: FunctionBody,
}

#[derive(Debug)]
struct FunctionHeader {
    extent: Extent,
    visibility: Option<Extent>,
    name: Extent,
    generics: Vec<Generic>,
    arguments: Vec<Argument>,
    return_type: Option<Type>,
    wheres: Vec<Where>,
}

//#[derive(Debug)]
type Generic = Extent;

type Type = Extent;

fn ex(start: Point, end: Point) -> Extent {
    let ex = (start.offset, end.offset);
    assert!(ex.1 > ex.0, "{} does not come before {}", ex.1, ex.0);
    ex
}

#[derive(Debug)]
struct Enum {
    extent: Extent,
    name: Extent,
    variants: Vec<EnumVariant>,
}

#[derive(Debug)]
struct EnumVariant {
    extent: Extent,
    name: Extent,
    body: Vec<EnumVariantBody>,
}

type EnumVariantBody = Extent;

#[derive(Debug)]
enum Argument {
    SelfArgument,
    Named { name: Extent, typ: Type }
}

#[derive(Debug)]
struct Where {
    name: Type,
    bounds: Extent,
}

#[derive(Debug)]
struct FunctionBody {
    extent: Extent,
    statements: Vec<Statement>,
    expression: Option<Expression>,
}

#[derive(Debug)]
struct Statement(Expression);

#[derive(Debug)]
struct Expression {
    extent: Extent,
    kind: ExpressionKind,
}

#[derive(Debug)]
enum ExpressionKind {
    MacroCall { name: Extent, args: Extent },
    Let { pattern: Pattern, value: Option<Box<Expression>> },
    Value { extent: Extent },
    FunctionCall { name: Extent, args: Vec<Expression> },
    Loop { body: Box<FunctionBody>},
    True,
}

type Pattern = Extent;

#[derive(Debug)]
struct Trait {
    extent: Extent,
    name: Extent,
}

#[derive(Debug)]
struct Impl {
    extent: Extent,
    trait_name: Type,
    type_name: Type,
    body: Vec<ImplFunction>,
}

#[derive(Debug)]
struct ImplFunction {
    extent: Extent,
    header: FunctionHeader,
    body: Option<FunctionBody>,
}

#[derive(Debug)]
struct Crate {
    extent: Extent,
    name: Extent,
}

#[derive(Debug)]
struct TypeAlias {
    extent: Extent,
    name: Type,
    defn: Type,
}

// TODO: extract to peresil?
fn parse_until<'s, P>(pt: Point<'s>, p: P) -> (Point<'s>, Extent)
    where P: std::str::pattern::Pattern<'s>
{
    let end = pt.s.find(p).unwrap_or(pt.s.len());
    let k = &pt.s[end..];
    (Point { s: k, offset: pt.offset + end }, (pt.offset, pt.offset + end))
}

// TODO: extract to peresil
fn one_or_more<'s, F, T>(pm: &mut Master<'s>, pt: Point<'s>, mut f: F) -> Progress<'s, Vec<T>>
    where F: FnMut(&mut Master<'s>, Point<'s>) -> Progress<'s, T>,
{
    let (pt, head) = try_parse!(f(pm, pt));
    let (pt, mut tail) = try_parse!(pm.zero_or_more(pt, f));

    tail.insert(0, head);
    Progress::success(pt, tail)
}

// TODO: promote?
fn comma_tail<'s, F, T>(f: F) -> impl Fn(&mut Master<'s>, Point<'s>) -> Progress<'s, T>
    where F: Fn(&mut Master<'s>, Point<'s>) -> Progress<'s, T>
{
    move |pm, pt| {
        let (pt, v) = try_parse!(f(pm, pt));
        let (pt, _) = try_parse!(optional(whitespace)(pm, pt));
        let (pt, _) = try_parse!(optional(literal(","))(pm, pt));
        let (pt, _) = try_parse!(optional(whitespace)(pm, pt));

        Progress::success(pt, v)
    }
}

// TODO: promote?
fn optional<'s, F, T>(f: F) -> impl Fn(&mut Master<'s>, Point<'s>) -> Progress<'s, Option<T>>
    where F: Fn(&mut Master<'s>, Point<'s>) -> Progress<'s, T>
{
    move |pm, pt| pm.optional(pt, &f) // what why ref?
}

// TODO: promote?
fn zero_or_more<'s, F, T>(f: F) -> impl Fn(&mut Master<'s>, Point<'s>) -> Progress<'s, Vec<T>>
    where F: Fn(&mut Master<'s>, Point<'s>) -> Progress<'s, T>
{
    move |pm, pt| pm.zero_or_more(pt, &f) // what why ref?
}

// TODO: can we transofrm this to (pm, pt)?
fn top_level<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, TopLevel> {
    pm.alternate(pt)
        .one(comment)
        .one(function)
        .one(p_enum)
        .one(p_trait)
        .one(p_impl)
        .one(attribute)
        .one(extern_crate)
        .one(type_alias)
        .one(whitespace)
        .finish()
}

fn literal<'s>(expected: &'static str) -> impl Fn(&mut Master<'s>, Point<'s>) -> Progress<'s, &'s str> {
    move |_pm, pt| pt.consume_literal(expected).map_err(|_| Error::Literal(expected))
}

fn comment<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, TopLevel> {
    let (pt, _) = try_parse!(literal("//")(pm, pt));
    let spt = pt;
    let (pt, _) = parse_until(pt, "\n");

    Progress::success(pt, TopLevel::Comment(ex(spt, pt)))
}

fn function<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, TopLevel> {
    let spt          = pt;
    let (pt, header) = try_parse!(function_header(pm, pt));
    let (pt, _)      = try_parse!(optional(whitespace)(pm, pt));
    let (pt, body)   = try_parse!(function_body(pm, pt));

    Progress::success(pt, TopLevel::Function(Function {
        extent: ex(spt, pt),
        header: header,
        body: body,
    }))
}

fn ext<'s, F, T>(f: F) -> impl Fn(&mut Master<'s>, Point<'s>) -> Progress<'s, Extent>
    where F: Fn(&mut Master<'s>, Point<'s>) -> Progress<'s, T>
{
    move |pm, pt| {
        let spt = pt;
        let (pt, _) = try_parse!(f(pm, pt));
        Progress::success(pt, ex(spt, pt))
    }
}

fn function_header<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, FunctionHeader> {
    let spt = pt;
    let (pt, visib)    = try_parse!(optional(ext(literal("pub")))(pm, pt));
    let (pt, _)        = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _)        = try_parse!(literal("fn")(pm, pt));
    let (pt, _)        = try_parse!(optional(whitespace)(pm, pt));
    let (pt, name)     = try_parse!(ident(pm, pt));
    let (pt, generics) = try_parse!(optional(function_generic_declarations)(pm, pt));
    let (pt, args)     = try_parse!(function_arglist(pm, pt));
    let (pt, _)        = try_parse!(optional(whitespace)(pm, pt));
    let (pt, ret)      = try_parse!(optional(function_return_type)(pm, pt));
    let (pt, _)        = try_parse!(optional(whitespace)(pm, pt));
    let (pt, wheres)   = try_parse!(optional(function_where_clause)(pm, pt));

    Progress::success(pt, FunctionHeader {
        extent: ex(spt, pt),
        visibility: visib,
        name: name,
        generics: generics.unwrap_or_else(Vec::new),
        arguments: args,
        return_type: ret,
        wheres: wheres.unwrap_or_else(Vec::new),
    })
}

fn ident<'s>(_pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Extent> {
    let spt = pt;
    let (pt, ex) = parse_until(pt, |c| {
        ['!', '(', ')', ' ', '<', '>', '{', '}', ':', ',', ';', '/'].contains(&c)
    });
    if pt.offset <= spt.offset {
        Progress::failure(pt, Error::IdentNotFound)
    } else {
        Progress::success(pt, ex)
    }
}

fn function_generic_declarations<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Vec<Generic>> {
    let (pt, _)     = try_parse!(literal("<")(pm, pt));
    let (pt, decls) = try_parse!(one_or_more(pm, pt, generic_declaration));
    let (pt, _)     = try_parse!(literal(">")(pm, pt));

    Progress::success(pt, decls)
}

fn generic_declaration<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Generic> {
    ident(pm, pt)
}

fn function_arglist<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Vec<Argument>> {
    let (pt, _)        = try_parse!(literal("(")(pm, pt));
    let (pt, self_arg) = try_parse!(optional(self_argument)(pm, pt));
    let (pt, mut args) = try_parse!(zero_or_more(function_argument)(pm, pt));
    let (pt, _)        = try_parse!(literal(")")(pm, pt));

    if let Some(arg) = self_arg {
        args.insert(0, arg);
    }
    Progress::success(pt, args)
}

fn self_argument<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Argument> {
    let (pt, _) = try_parse!(optional(literal("&"))(pm, pt));
    let (pt, _) = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _) = try_parse!(optional(literal("mut"))(pm, pt));
    let (pt, _) = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _) = try_parse!(literal("self")(pm, pt));
    let (pt, _) = try_parse!(optional(literal(","))(pm, pt));

    Progress::success(pt, Argument::SelfArgument)
}

fn function_argument<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Argument> {
    let (pt, name) = try_parse!(ident(pm, pt));
    let (pt, _)    = try_parse!(literal(":")(pm, pt));
    let (pt, _)    = try_parse!(optional(whitespace)(pm, pt));
    let (pt, typ)  = try_parse!(ident(pm, pt));
    let (pt, _)    = try_parse!(optional(literal(","))(pm, pt));

    Progress::success(pt, Argument::Named { name: name, typ: typ })
}

fn function_return_type<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Type> {
    let (pt, _) = try_parse!(literal("->")(pm, pt));
    let (pt, _) = try_parse!(optional(whitespace)(pm, pt));
    let (pt, t) = try_parse!(typ(pm, pt));

    Progress::success(pt, t)
}

fn function_where_clause<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Vec<Where>> {
    let (pt, _) = try_parse!(literal("where")(pm, pt));
    let (pt, _) = try_parse!(whitespace(pm, pt));

    one_or_more(pm, pt, function_where)
}

fn function_where<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Where> {
    let (pt, name)   = try_parse!(ident(pm, pt));
    let (pt, _)      = try_parse!(literal(":")(pm, pt));
    let (pt, _)      = try_parse!(optional(whitespace)(pm, pt));
    let (pt, bounds) = try_parse!(ident(pm, pt));
    let (pt, _)      = try_parse!(optional(literal(","))(pm, pt));

    Progress::success(pt, Where { name: name, bounds: bounds })
}

fn function_body<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, FunctionBody> {
    let spt = pt;
    let (pt, _)     = try_parse!(literal("{")(pm, pt));
    let (pt, stmts) = try_parse!(zero_or_more(statement)(pm, pt));
    let (pt, expr)  = try_parse!(optional(expression)(pm, pt));
    let (pt, _)     = try_parse!(literal("}")(pm, pt));

    Progress::success(pt, FunctionBody {
        extent: ex(spt, pt),
        statements: stmts,
        expression: expr,
    })
}

fn statement<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Statement> {
    let (pt, expr) = try_parse!(expression(pm, pt));
    let (pt, _)    = try_parse!(literal(";")(pm, pt));
    let (pt, _)    = try_parse!(optional(whitespace)(pm, pt));

    Progress::success(pt, Statement(expr))
}

fn expression<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Expression> {
    let spt        = pt;
    let (pt, _)    = try_parse!(optional(whitespace)(pm, pt));
    let (pt, kind) = try_parse!({
        pm.alternate(pt)
            .one(macro_call)
            .one(expr_let)
            .one(expr_function_call)
            .one(expr_loop)
            .one(expr_value)
            .one(expr_true)
            .finish()
    });
    let (pt, _)    = try_parse!(optional(whitespace)(pm, pt));

    Progress::success(pt, Expression {
        extent: ex(spt, pt),
        kind: kind,
    })
}

fn macro_call<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, ExpressionKind> {
    let (pt, name) = try_parse!(ident(pm, pt));
    let (pt, _)    = try_parse!(literal("!")(pm, pt));
    let (pt, _)    = try_parse!(literal("(")(pm, pt));
    let (pt, args) = parse_until(pt, ")");
    let (pt, _)    = try_parse!(literal(")")(pm, pt));

    Progress::success(pt, ExpressionKind::MacroCall { name: name, args: args })
}

fn expr_let<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, ExpressionKind> {
    let (pt, _)       = try_parse!(literal("let")(pm, pt));
    let (pt, _)       = try_parse!(whitespace(pm, pt));
    let (pt, pattern) = try_parse!(pattern(pm, pt));
    let (pt, _)       = try_parse!(optional(whitespace)(pm, pt));
    let (pt, value)   = try_parse!(optional(expr_let_rhs)(pm, pt));

    Progress::success(pt, ExpressionKind::Let {
        pattern: pattern,
        value: value.map(Box::new),
    })
}

fn expr_let_rhs<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Expression> {
    let (pt, _)     = try_parse!(literal("=")(pm, pt));
    let (pt, _)     = try_parse!(optional(whitespace)(pm, pt));
    let (pt, value) = try_parse!(expression(pm, pt));

    Progress::success(pt, value)
}

fn expr_loop<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, ExpressionKind> {
    let (pt, _)    = try_parse!(literal("loop")(pm, pt));
    let (pt, _)    = try_parse!(optional(whitespace)(pm, pt));
    let (pt, body) = try_parse!(function_body(pm, pt));

    Progress::success(pt, ExpressionKind::Loop {
        body: Box::new(body),
    })
}

fn expr_value<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, ExpressionKind> {
    let spt = pt;
    let (pt, _) = try_parse!(path(pm, pt));
    let (pt, _) = try_parse!(ident(pm, pt));

    Progress::success(pt, ExpressionKind::Value {
        extent: ex(spt, pt),
    })
}

fn expr_function_call<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, ExpressionKind> {
    let spt = pt;
    let (pt, _) = try_parse!(path(pm, pt));
    let (pt, _) = try_parse!(ident(pm, pt));
    let mpt = pt;
    let (pt, _) = try_parse!(literal("(")(pm, pt));
    let (pt, args) = try_parse!(zero_or_more(comma_tail(expression))(pm, pt));
    let (pt, _) = try_parse!(literal(")")(pm, pt));

    Progress::success(pt, ExpressionKind::FunctionCall {
        name: ex(spt, mpt),
        args: args,
    })
}

fn expr_true<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, ExpressionKind> {
    let (pt, _) = try_parse!(literal("true")(pm, pt));

    Progress::success(pt, ExpressionKind::True)
}

fn pattern<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Pattern> {
    let (pt, _) = try_parse!(optional(literal("mut"))(pm, pt));
    let (pt, _) = try_parse!(optional(whitespace)(pm, pt));
    ident(pm, pt)
}

fn p_enum<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, TopLevel> {
    p_enum_inner(pm, pt).map(TopLevel::Enum)
}

fn p_enum_inner<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Enum> {
    let spt            = pt;
    let (pt, _)        = try_parse!(literal("enum")(pm, pt));
    let (pt, _)        = try_parse!(whitespace(pm, pt));
    let (pt, name)     = try_parse!(ident(pm, pt));
    let (pt, _)        = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _)        = try_parse!(literal("{")(pm, pt));
    let (pt, _)        = try_parse!(optional(whitespace)(pm, pt));
    let (pt, variants) = try_parse!(zero_or_more(comma_tail(enum_variant))(pm, pt));
    let (pt, _)        = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _)        = try_parse!(literal("}")(pm, pt));

    Progress::success(pt, Enum {
        extent: ex(spt, pt),
        name: name,
        variants: variants,
    })
}

fn enum_variant<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, EnumVariant> {
    let spt        = pt;
    let (pt, name) = try_parse!(ident(pm, pt));
    let (pt, _)    = try_parse!(optional(whitespace)(pm, pt));
    let (pt, body) = try_parse!(optional(enum_variant_body)(pm, pt));

    Progress::success(pt,  EnumVariant {
        extent: ex(spt, pt),
        name: name,
        body: body.unwrap_or_else(Vec::new),
    })
}

fn enum_variant_body<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Vec<EnumVariantBody>> {
    let (pt, _)     = try_parse!(literal("(")(pm, pt));
    let (pt, types) = try_parse!(zero_or_more(comma_tail(typ))(pm, pt));
    let (pt, _)     = try_parse!(literal(")")(pm, pt));

    Progress::success(pt, types)
}

fn p_trait<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, TopLevel> {
    let spt        = pt;
    let (pt, _)    = try_parse!(literal("trait")(pm, pt));
    let (pt, _)    = try_parse!(whitespace(pm, pt));
    let (pt, name) = try_parse!(ident(pm, pt));
    let (pt, _)    = try_parse!(whitespace(pm, pt));
    let (pt, _)    = try_parse!(literal("{}")(pm, pt));

    Progress::success(pt, TopLevel::Trait(Trait {
        extent: ex(spt, pt),
        name: name,
    }))
}

fn p_impl<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, TopLevel> {
    p_impl_inner(pm, pt).map(TopLevel::Impl)
}

fn p_impl_inner<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Impl> {
    let spt              = pt;
    let (pt, _)          = try_parse!(literal("impl")(pm, pt));
    let (pt, _)          = try_parse!(whitespace(pm, pt));
    let (pt, trait_name) = try_parse!(typ(pm, pt));
    let (pt, _)          = try_parse!(whitespace(pm, pt));
    let (pt, _)          = try_parse!(literal("for")(pm, pt));
    let (pt, _)          = try_parse!(whitespace(pm, pt));
    let (pt, type_name)  = try_parse!(typ(pm, pt));
    let (pt, _)          = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _)          = try_parse!(literal("{")(pm, pt));
    let (pt, _)          = try_parse!(optional(whitespace)(pm, pt));
    let (pt, body)       = try_parse!(zero_or_more(impl_function)(pm, pt));
    let (pt, _)          = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _)          = try_parse!(literal("}")(pm, pt));

    Progress::success(pt, Impl {
        extent: ex(spt, pt),
        trait_name: trait_name,
        type_name: type_name,
        body: body,
    })
}

fn impl_function<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, ImplFunction> {
    let spt = pt;
    let (pt, header) = try_parse!(function_header(pm, pt));
    let (pt, body)   = try_parse!(optional(function_body)(pm, pt));

    Progress::success(pt, ImplFunction {
        extent: ex(spt, pt),
        header: header,
        body: body,
    })
}

// TODO: optional could take E that is `into`, or just a different one

fn attribute<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, TopLevel> {
    let spt = pt;
    let (pt, _) = try_parse!(literal("#")(pm, pt));
    let (pt, _) = try_parse!(optional(literal("!"))(pm, pt));
    let (pt, _) = try_parse!(literal("[")(pm, pt));
    let (pt, _) = parse_until(pt, "]");
    let (pt, _) = try_parse!(literal("]")(pm, pt));

    Progress::success(pt, TopLevel::Attribute(ex(spt, pt)))
}

fn extern_crate<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, TopLevel> {
    let spt = pt;
    let (pt, _)    = try_parse!(literal("extern")(pm, pt));
    let (pt, _)    = try_parse!(whitespace(pm, pt));
    let (pt, _)    = try_parse!(literal("crate")(pm, pt));
    let (pt, _)    = try_parse!(whitespace(pm, pt));
    let (pt, name) = try_parse!(ident(pm, pt));
    let (pt, _)    = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _)    = try_parse!(literal(";")(pm, pt));

    Progress::success(pt, TopLevel::ExternCrate(Crate {
        extent: ex(spt, pt),
        name: name,
    }))
}

fn type_alias<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, TopLevel> {
    let spt = pt;
    let (pt, _)    = try_parse!(literal("type")(pm, pt));
    let (pt, _)    = try_parse!(whitespace(pm, pt));
    let (pt, name) = try_parse!(typ(pm, pt));
    let (pt, _)    = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _)    = try_parse!(literal("=")(pm, pt));
    let (pt, _)    = try_parse!(optional(whitespace)(pm, pt));
    let (pt, defn) = try_parse!(typ(pm, pt));
    let (pt, _)    = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _)    = try_parse!(literal(";")(pm, pt));

    Progress::success(pt, TopLevel::TypeAlias(TypeAlias {
        extent: ex(spt, pt),
        name: name,
        defn: defn,
    }))
}

fn typ<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Extent> {
    let spt = pt;
    let (pt, _) = try_parse!(optional(literal("&"))(pm, pt));
    let (pt, _) = try_parse!(optional(lifetime)(pm, pt));
    let (pt, _) = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _) = try_parse!(path(pm, pt));
    let (pt, _) = try_parse!(ident(pm, pt));
    let (pt, _) = try_parse!(optional(typ_generics)(pm, pt));

    Progress::success(pt, ex(spt, pt))
}

fn path<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Vec<Extent>> {
    zero_or_more(path_component)(pm, pt)
}

fn path_component<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Extent> {
    let spt = pt;
    let (pt, _) = try_parse!(ident(pm, pt));
    let (pt, _) = try_parse!(literal("::")(pm, pt));

    Progress::success(pt, ex(spt, pt))
}

fn typ_generics<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Extent> {
    let spt = pt;
    let (pt, _) = try_parse!(literal("<")(pm, pt));
    let (pt, _) = try_parse!(optional(whitespace)(pm, pt));
    let (pt, _) = try_parse!(optional(typ_generic_lifetimes)(pm, pt));
    let (pt, _) = try_parse!(optional(type_generic_types)(pm, pt));
    let (pt, _) = try_parse!(literal(">")(pm, pt));

    Progress::success(pt, ex(spt, pt))
}

fn typ_generic_lifetimes<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Vec<Extent>> {
    one_or_more(pm, pt, comma_tail(lifetime))
}

fn lifetime<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Extent> {
    let (pt, _) = try_parse!(literal("'")(pm, pt));
    let (pt, _) = try_parse!(optional(whitespace)(pm, pt));
    ident(pm, pt)
}

fn type_generic_types<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, Vec<Extent>> {
    one_or_more(pm, pt, comma_tail(typ))
}

fn whitespace<'s>(pm: &mut Master<'s>, pt: Point<'s>) -> Progress<'s, TopLevel> {
    let spt = pt;

    let (pt, _) = try_parse!(one_or_more(pm, pt, |pm, pt| {
        pm.alternate(pt)
            .one(literal(" "))
            .one(literal("\t"))
            .one(literal("\r"))
            .one(literal("\n"))
            .finish()

    }));

    Progress::success(pt, TopLevel::Whitespace(ex(spt, pt)))
}

#[cfg(test)]
mod test {
    use super::*;

    fn qp<'s, F, T>(f: F, s: &'s str) -> peresil::Progress<Point<'s>, T, Vec<Error>>
        where F: FnOnce(&mut Master<'s>, Point<'s>) -> Progress<'s, T>
    {
        // TODO: Master::once()?
        let mut pm = Master::new();
        let pt = Point::new(s);
        let r = f(&mut pm, pt);
        pm.finish(r)
    }

    #[test]
    fn enum_with_trailing_stuff() {
        let p = qp(p_enum_inner, "enum A {} impl Foo for Bar {}");
        assert_eq!(unwrap_progress(p).extent, (0, 9))
    }

    #[test]
    fn fn_with_public_modifier() {
        let p = qp(function_header, "pub fn foo()");
        assert_eq!(unwrap_progress(p).extent, (0, 12))
    }

    #[test]
    fn fn_with_self_type() {
        let p = qp(function_header, "fn foo(&self)");
        assert_eq!(unwrap_progress(p).extent, (0, 13))
    }

    #[test]
    fn fn_with_return_type() {
        let p = qp(function_header, "fn foo() -> bool");
        assert_eq!(unwrap_progress(p).extent, (0, 16))
    }

    #[test]
    fn expr_true() {
        let p = qp(expression, "true");
        assert_eq!(unwrap_progress(p).extent, (0, 4))
    }

    #[test]
    fn expr_let_mut() {
        let p = qp(expression, "let mut pm = Master::new()");
        assert_eq!(unwrap_progress(p).extent, (0, 26))
    }

    #[test]
    fn expr_let_no_value() {
        let p = qp(expression, "let pm");
        assert_eq!(unwrap_progress(p).extent, (0, 6))
    }

    #[test]
    fn expr_value_with_path() {
        let p = qp(expression, "Master::new()");
        assert_eq!(unwrap_progress(p).extent, (0, 13))
    }

    #[test]
    fn expr_function_call() {
        let p = qp(expression, "foo()");
        assert_eq!(unwrap_progress(p).extent, (0, 5))
    }

    #[test]
    fn expr_function_call_with_args() {
        let p = qp(expression, "foo(true)");
        assert_eq!(unwrap_progress(p).extent, (0, 9))
    }

    #[test]
    fn expr_loop() {
        let p = qp(expression, "loop {}");
        assert_eq!(unwrap_progress(p).extent, (0, 7))
    }

    fn unwrap_progress<P, T, E>(p: peresil::Progress<P, T, E>) -> T
        where P: std::fmt::Debug,
              E: std::fmt::Debug,
    {
        match p {
            peresil::Progress { status: peresil::Status::Success(v), .. } => v,
            peresil::Progress { status: peresil::Status::Failure(e), point } => {
                panic!("Failed parsing at {:?}: {:?}", point, e)
            }
        }
    }
}
