// facade.rs — writes the typed wrapper of each JSON op into src/ops/<module>.ts, from the tables in facades/.
// A reader would get wrong where a wrapper's parameters come from: the args struct's fields, in order, as ts-rs types them.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use ts_rs::TS;

/// How a wrapper's parameters map to the op's args.
pub enum Args {
    /// One parameter per field of the args struct, in its order.
    Fields(fn() -> String),
    /// The args struct itself, as one parameter.
    Whole(fn() -> String),
    /// No parameter and no args.
    Bare,
}

/// One wrapper: its TypeScript name, the op it calls, its parameters, its answer's type and its one-line doc.
pub struct Op {
    name: &'static str,
    op: &'static str,
    args: Args,
    answer: &'static str,
    doc: &'static str,
}

/// A wrapper whose parameters are the fields of `A`, answering the TypeScript type `answer`.
pub fn op<A: TS>(name: &'static str, op: &'static str, answer: &'static str, doc: &'static str) -> Op {
    Op {
        name,
        op,
        args: Args::Fields(A::inline),
        answer,
        doc,
    }
}

/// A wrapper that takes the args struct `A` whole.
pub fn whole<A: TS>(name: &'static str, op: &'static str, answer: &'static str, doc: &'static str) -> Op {
    Op {
        name,
        op,
        args: Args::Whole(A::name),
        answer,
        doc,
    }
}

/// A wrapper with no parameter.
pub fn bare(name: &'static str, op: &'static str, answer: &'static str, doc: &'static str) -> Op {
    Op {
        name,
        op,
        args: Args::Bare,
        answer,
        doc,
    }
}

/// One generated module: its file name, the request enums its op names are checked against, its wrappers.
pub struct Facade {
    module: &'static str,
    requests: Vec<String>,
    ops: Vec<Op>,
}

/// A module's facade from its request enums' TypeScript names and its wrappers.
pub fn facade(module: &'static str, requests: Vec<String>, ops: Vec<Op>) -> Facade {
    Facade { module, requests, ops }
}

const RESERVED: &[&str] = &[
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
];

struct Param {
    field: String,
    name: String,
    ty: String,
    optional: bool,
}

/// Splits `text` on `sep` where no bracket is open.
fn split_top(text: &str, seps: &[char]) -> Vec<String> {
    let (mut depth, mut cur, mut out) = (0i32, String::new(), Vec::new());
    for c in text.chars() {
        match c {
            '<' | '{' | '(' | '[' => depth += 1,
            '>' | '}' | ')' | ']' => depth -= 1,
            _ => {}
        }
        if depth == 0 && seps.contains(&c) {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out.into_iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Drops the `/** … */` docs ts-rs writes into an inline struct.
fn strip_docs(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("/**") {
        out.push_str(&rest[..at]);
        rest = rest[at..].find("*/").map_or("", |end| &rest[at + end + 2..]);
    }
    out.push_str(rest);
    out
}

/// The fields of an inline struct (`{ a: T, b?: U, }`), as parameters; trailing nullable ones may be left out.
fn params(op: &str, inline: &str) -> Result<Vec<Param>, String> {
    let body = strip_docs(inline);
    let body = body.trim();
    let inner = body
        .strip_prefix('{')
        .and_then(|b| b.strip_suffix('}'))
        .ok_or_else(|| format!("{op}: args are not a plain struct: {body}"))?;
    let mut out: Vec<Param> = split_top(inner, &[',', ';'])
        .into_iter()
        .map(|field| {
            let (key, ty) = field
                .split_once(':')
                .ok_or_else(|| format!("{op}: bad field {field}"))?;
            let key = key.trim();
            let optional = key.ends_with('?');
            let key = key.trim_end_matches('?').trim_matches('"').to_owned();
            let name = if RESERVED.contains(&key.as_str()) {
                format!("{key}_")
            } else {
                key.clone()
            };
            Ok(Param {
                field: key,
                name,
                ty: ty.trim().replace("Array<", "ReadonlyArray<"),
                optional,
            })
        })
        .collect::<Result<_, String>>()?;
    let mut tail = true;
    for p in out.iter_mut().rev() {
        // An optional field is an `Option`, which reads a null as well as a missing key.
        if p.optional && !p.ty.ends_with("| null") {
            p.ty.push_str(" | null");
        }
        tail &= p.optional || p.ty.ends_with("| null");
        if tail {
            p.optional = true;
        } else if p.optional {
            p.ty.push_str(" | undefined");
            p.optional = false;
        }
    }
    Ok(out)
}

/// The capitalised names a type text mentions.
fn idents(text: &str, into: &mut Vec<String>) {
    let mut cur = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_alphanumeric() || c == '_' {
            cur.push(c);
        } else {
            if cur.starts_with(|c: char| c.is_ascii_uppercase()) {
                into.push(std::mem::take(&mut cur));
            }
            cur.clear();
        }
    }
}

fn render(f: &Facade, types: &Path) -> Result<String, String> {
    let mut names = f.requests.clone();
    let mut body = String::new();
    for o in &f.ops {
        idents(o.answer, &mut names);
        let (sig, args) = match &o.args {
            Args::Bare => (String::new(), String::new()),
            Args::Whole(ty) => {
                let ty = ty();
                idents(&ty, &mut names);
                (format!("args: {ty}"), ", args".to_owned())
            }
            Args::Fields(inline) => {
                let ps = params(o.op, &inline())?;
                let sig: Vec<String> = ps
                    .iter()
                    .map(|p| {
                        idents(&p.ty, &mut names);
                        format!("{}{}: {}", p.name, if p.optional { "?" } else { "" }, p.ty)
                    })
                    .collect();
                let fields: Vec<String> = ps
                    .iter()
                    .map(|p| {
                        if p.name == p.field {
                            p.name.clone()
                        } else {
                            format!("{}: {}", p.field, p.name)
                        }
                    })
                    .collect();
                (sig.join(", "), format!(", {{ {} }}", fields.join(", ")))
            }
        };
        let _ = write!(
            body,
            "\n/** {} */\nexport const {} = ({sig}): {} =>\n  ask(\"{}\"{args});\n",
            o.doc, o.name, o.answer, o.op
        );
    }
    names.retain(|n| types.join(format!("{n}.ts")).exists());
    names.sort_by(|a, b| crate::biome_order(a, b));
    names.dedup();
    let requests = f.requests.join(" | ");
    Ok(format!(
        "// {m}.ts — GENERATED by bindings/facade.rs from bindings/facades/: one typed wrapper per op. Do not edit.\n\
         // The op names are checked against the Rust request enums: a renamed op fails the type check here.\n\n\
         import {{ call }} from \"../abi.ts\";\n\
         import type {{ {imports} }} from \"../types/index.ts\";\n\n\
         type Op = ({requests})[\"op\"];\n\n\
         const ask = <R>(op: Op, args?: object): R => call<R>({{ op, args }});\n{body}",
        m = f.module,
        imports = names.join(", "),
    ))
}

/// Writes every facade into `dir`, emptied first so a module no table names does not linger.
pub fn write_facades(dir: &Path, types: &Path, facades: &[Facade]) -> Result<(), Box<dyn std::error::Error>> {
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    fs::create_dir_all(dir)?;
    for f in facades {
        fs::write(dir.join(format!("{}.ts", f.module)), render(f, types)?)?;
    }
    Ok(())
}
