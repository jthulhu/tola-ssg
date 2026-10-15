//! Schema diagnostics located at their caller or the source metadata they describe.

use typst::World;
use typst::comemo::Tracked;
use typst::diag::{At, SourceDiagnostic, SourceResult, bail, eco_format};
use typst::ecow::{EcoString, EcoVec, eco_vec};
use typst::engine::Engine;
use typst::foundations::{Array, Context, Dict, Func, Repr, Value, func};
use typst::syntax::ast::AstNode;
use typst::syntax::{DiagSpan, FileId, LinkedNode, RootedPath, Source, Span, ast};

const SCHEMA_ARGUMENT: &str = "SchemaArgument";
const SCHEMA_WRAPPERS: [crate::TolaPackage; 3] = [
    crate::TolaPackage::Schema,
    crate::TolaPackage::Source,
    crate::TolaPackage::Host,
];

/// The packages Tola resolves source metadata through, which no site author wrote: diagnostics
/// reported from inside them point at the author's call and omit their own frames.
fn wrapper_projection() -> tola_typst::diagnostic::DiagnosticProjection {
    tola_typst::diagnostic::DiagnosticProjection::through(
        SCHEMA_WRAPPERS.into_iter().map(crate::TolaPackage::spec),
    )
}

#[func]
pub(super) fn tola_report_schema_error(
    span: Span,
    /// The value supplied where a schema declaration was required.
    candidate: Value,
    /// The schema operation whose argument was invalid.
    owner: typst::foundations::Str,
) -> SourceResult<Value> {
    let actual = candidate.ty().short_name();
    let mut error = tola_typst::diagnostic::producer_error(
        span,
        SCHEMA_ARGUMENT,
        &[("owner", owner.as_str()), ("actual", actual)],
        eco_format!("{owner}: expected a Typst type or schema declaration, found `{actual}`"),
        wrapper_projection(),
    );
    error.hint(if matches!(candidate, Value::Dict(_)) {
        "wrap the field dictionary with `schema(...)`"
    } else {
        "pass a Typst type such as `str` or a declaration such as `schema((title: str))`"
    });
    Err(eco_vec![error])
}

#[func(contextual)]
pub(super) fn tola_parse_source(
    engine: &mut Engine,
    context: Tracked<Context>,
    span: Span,
    /// The complete source record whose metadata is being resolved.
    source: Dict,
    /// A declaration or a function choosing that source's declaration.
    schema: Value,
    /// The schema package's parser.
    parse: Func,
) -> SourceResult<Value> {
    let path = source
        .get("path")
        .at(span)?
        .clone()
        .cast::<typst::foundations::Str>()
        .at(span)?;
    let (file, _) = source_origin(engine, path.as_str()).at(span)?;
    let path = file.vpath().get_without_slash();
    let is_factory = matches!(schema, Value::Func(_));
    let declaration = if let Value::Func(factory) = schema {
        factory
            .call(engine, context, [Value::Dict(source.clone())])
            .map_err(|mut errors| {
                for error in errors.make_mut() {
                    error.hint(eco_format!("while choosing the schema for `{path}`"));
                }
                errors
            })?
    } else {
        schema
    };
    let metadata = match source.get("meta").at(span)? {
        Value::None => Value::Dict(Dict::new()),
        value => value.clone(),
    };
    parse
        .call(engine, context, [metadata, declaration])
        .map_err(|errors| source_schema_errors(errors, path, is_factory))
}

fn source_schema_errors(
    mut errors: EcoVec<SourceDiagnostic>,
    path: &str,
    is_factory: bool,
) -> EcoVec<SourceDiagnostic> {
    for error in errors.make_mut() {
        let native = tola_typst::diagnostic::NativeDiagnostic::from(error.clone());
        if let tola_typst::diagnostic::DiagnosticOrigin::Producer(origin) = native.origin()
            && origin.name() == SCHEMA_ARGUMENT
            && origin
                .fields()
                .iter()
                .any(|(key, value)| key == "owner" && value == "try-parse")
            && !error.trace.iter().any(|trace| {
                trace.span.id().is_some_and(|id| match id.root() {
                    typst::syntax::VirtualRoot::Project => true,
                    typst::syntax::VirtualRoot::Package(package) => !SCHEMA_WRAPPERS
                        .into_iter()
                        .any(|owned| owned.spec() == *package),
                })
            })
            && let Some((_, actual)) = origin.fields().iter().find(|(key, _)| key == "actual")
        {
            error.message = if is_factory {
                eco_format!(
                    "schema function for `{path}` returned `{actual}`; expected a Typst type or schema declaration"
                )
            } else {
                eco_format!(
                    "parse-sources schema for `{path}` has type `{actual}`; expected a Typst type or schema declaration"
                )
            };
        } else {
            error.hint(eco_format!("while resolving the schema for `{path}`"));
        }
    }
    errors
}

/// Report every issue of one `parse-sources` call against its source metadata.
#[func]
pub(super) fn tola_report_source_issues(
    engine: &mut Engine,
    span: Span,
    /// One `(path, issue)` record per schema Issue, in source order.
    issues: Array,
) -> SourceResult<Value> {
    let mut diagnostics = EcoVec::new();
    for issue in issues.iter() {
        diagnostics.push(source_issue(engine, span, issue)?);
    }
    if diagnostics.is_empty() {
        return Ok(Value::None);
    }
    Err(diagnostics)
}

fn source_issue(engine: &Engine<'_>, span: Span, issue: &Value) -> SourceResult<SourceDiagnostic> {
    let described = "parse-sources issues are `(path, issue)` records";
    let Value::Dict(issue) = issue else {
        bail!(span, "{described}");
    };
    let Ok(Value::Str(path)) = issue.get("path") else {
        bail!(span, "{described}");
    };
    let problem = Issue::decode(issue.get("issue").at(span)?, span)?;
    let (file, declaration) = source_origin(engine, path.as_str()).at(span)?;
    let site_path = file.vpath().get_without_slash().to_owned();
    let file = FileId::new(file);
    let source = engine.world.source(file).at(span)?;
    let origin = IssueOrigin {
        source: &source,
        declaration,
    };
    let mut diagnostic = SourceDiagnostic::error(
        tola_typst::diagnostic::projected_span(
            source.id(),
            origin.locate(&problem, &problem.path),
            wrapper_projection(),
        ),
        eco_format!("`{site_path}`: {}", problem.describe(&problem.path, None)),
    );
    problem.append_hints(&mut diagnostic, &origin, &problem.path, None);
    Ok(diagnostic)
}

struct Issue<'a> {
    code: &'a str,
    path: Vec<Value>,
    message: &'a str,
    notes: Vec<&'a str>,
    children: Vec<Issue<'a>>,
}

impl<'a> Issue<'a> {
    fn decode(value: &'a Value, span: Span) -> SourceResult<Self> {
        let described = "expected an Issue with `code`, `path`, `message`, `notes`, and `children`";
        let Value::Dict(issue) = value else {
            bail!(span, "{described}")
        };
        let string = |key| match issue.get(key) {
            Ok(Value::Str(value)) => Ok(value.as_str()),
            _ => Err(eco_vec![SourceDiagnostic::error(span, described)]),
        };
        let array = |key| match issue.get(key) {
            Ok(Value::Array(value)) => Ok(value),
            _ => Err(eco_vec![SourceDiagnostic::error(span, described)]),
        };
        let path = array("path")?;
        if !path.iter().all(|segment| {
            matches!(segment, Value::Str(_)) || matches!(segment, Value::Int(index) if *index >= 0)
        }) {
            bail!(
                span,
                "Issue paths contain string keys or nonnegative indices"
            );
        }
        Ok(Self {
            code: string("code")?,
            path: path.iter().cloned().collect(),
            message: string("message")?,
            notes: array("notes")?
                .iter()
                .map(|note| match note {
                    Value::Str(note) => Ok(note.as_str()),
                    _ => Err(eco_vec![SourceDiagnostic::error(
                        span,
                        "Issue notes must be strings"
                    )]),
                })
                .collect::<SourceResult<_>>()?,
            children: array("children")?
                .iter()
                .map(|child| Self::decode(child, span))
                .collect::<SourceResult<_>>()?,
        })
    }

    fn generated(&self) -> bool {
        matches!(self.code, "schema.default" | "schema.output")
    }

    fn describe(&self, path: &[Value], provenance: Option<&str>) -> EcoString {
        let provenance = provenance
            .map(|label| eco_format!("{label} "))
            .unwrap_or_default();
        eco_format!("{provenance}`{}`: {}", issue_path(path), self.message)
    }

    fn append_hints(
        &self,
        diagnostic: &mut SourceDiagnostic,
        origin: &IssueOrigin<'_>,
        path: &[Value],
        provenance: Option<&str>,
    ) {
        for note in &self.notes {
            diagnostic.hint(*note);
        }
        let provenance = match self.code {
            "schema.default" => Some("declared default"),
            "schema.output" => Some("converted value"),
            _ => provenance,
        };
        for child in &self.children {
            let child_path = if self.generated() {
                child.path.clone()
            } else {
                [path, &child.path].concat()
            };
            let location = origin.span(if provenance.is_some() {
                origin.declaration_range()
            } else {
                origin.locate(child, &child_path)
            });
            diagnostic.spanned_hint(child.describe(&child_path, provenance), location);
            child.append_hints(diagnostic, origin, &child_path, provenance);
        }
    }
}

fn issue_path(path: &[Value]) -> String {
    let mut rendered = "$".to_owned();
    for segment in path {
        match segment {
            Value::Int(index) => rendered.push_str(&format!("[{index}]")),
            Value::Str(key)
                if key.chars().enumerate().all(|(index, ch)| {
                    ch.is_ascii_alphabetic()
                        || ch == '_'
                        || (index > 0 && (ch.is_ascii_digit() || ch == '-'))
                }) && !key.is_empty() =>
            {
                rendered.push('.');
                rendered.push_str(key.as_str());
            }
            Value::Str(key) => {
                rendered.push_str(&format!("[{}]", key.repr().replace("`", "\\u{60}")))
            }
            _ => unreachable!("Issue path segments were checked on ingress"),
        }
    }
    rendered
}

struct IssueOrigin<'a> {
    source: &'a Source,
    declaration: std::ops::Range<usize>,
}

impl IssueOrigin<'_> {
    /// The diagnostic span of one byte range in this source.
    fn span(&self, range: std::ops::Range<usize>) -> DiagSpan {
        DiagSpan::from_range(self.source.id(), range)
    }

    /// The byte range of the source's `<tola-meta>` declaration.
    fn declaration_range(&self) -> std::ops::Range<usize> {
        self.declaration.clone()
    }

    /// The byte range `path` lives at, or the declaration itself when nothing literal declares it.
    fn locate(&self, issue: &Issue<'_>, path: &[Value]) -> std::ops::Range<usize> {
        if !issue.generated()
            && !path.is_empty()
            && let Some(call) =
                declaration_call(LinkedNode::new(self.source.root()), &self.declaration)
            && let Some(ast::Arg::Pos(metadata)) = call.args().items().next()
            && let Some(value) = literal_field(metadata, path)
            && let Some(node) = self.source.find(value.span())
        {
            return node.range();
        }
        self.declaration_range()
    }
}

fn declaration_call<'a>(
    node: LinkedNode<'a>,
    range: &std::ops::Range<usize>,
) -> Option<ast::FuncCall<'a>> {
    if node.range() == *range
        && let Some(call) = node.get().cast()
    {
        return Some(call);
    }
    node.children()
        .filter(|child| child.range().start <= range.start && child.range().end >= range.end)
        .find_map(|child| declaration_call(child, range))
}

fn literal_field<'a>(expression: ast::Expr<'a>, path: &[Value]) -> Option<ast::Expr<'a>> {
    if let ast::Expr::Parenthesized(group) = expression {
        return literal_field(group.expr(), path);
    }
    let Some((segment, remaining)) = path.split_first() else {
        return Some(expression);
    };
    match (expression, segment) {
        (ast::Expr::Dict(dictionary), Value::Str(key)) => {
            for member in dictionary.items().rev() {
                let value = match member {
                    ast::DictItem::Named(named) if named.name().as_str() == key.as_str() => {
                        named.expr()
                    }
                    ast::DictItem::Keyed(keyed) => match keyed.key() {
                        ast::Expr::Str(name) if name.get().as_str() == key.as_str() => keyed.expr(),
                        ast::Expr::Str(_) => continue,
                        _ => return None,
                    },
                    ast::DictItem::Named(_) => continue,
                    ast::DictItem::Spread(_) => return None,
                };
                return literal_field(value, remaining);
            }
            None
        }
        (ast::Expr::Array(array), Value::Int(index)) => {
            for (offset, member) in array.items().enumerate() {
                let ast::ArrayItem::Pos(value) = member else {
                    return None;
                };
                if offset == *index as usize {
                    return literal_field(value, remaining);
                }
            }
            None
        }
        _ => None,
    }
}

/// Where `path` lives and declares its metadata, as the file it is addressed by and the byte range
/// of its `<tola-meta>` declaration.
///
/// A source that declares no metadata has no range, and the file's start answers for it: the
/// diagnostic still names the file and the field, and there is no declaration to point at. A path
/// no origin covers is not one of this compilation's sources, which is a programming error rather
/// than a position to guess.
fn source_origin(
    engine: &Engine<'_>,
    path: &str,
) -> Result<(RootedPath, std::ops::Range<usize>), EcoString> {
    let Some(Value::Dict(origin)) =
        super::library::source_origins(engine).and_then(|origins| origins.get(path).ok())
    else {
        return Err(eco_format!("`{path}` is not a source of this compilation"));
    };
    let file = origin
        .get("file")
        .map_err(|_| eco_format!("`{path}` has an origin without a file"))?
        .clone()
        .cast::<RootedPath>()
        .map_err(|_| eco_format!("`{path}` has an origin whose file is not a path"))?;
    let range = match origin.get("range") {
        Ok(Value::None) => 0..0,
        Ok(Value::Array(range)) => declaration_bounds(path, range)?,
        _ => {
            return Err(eco_format!(
                "`{path}` has an origin whose range is not an array or `none`"
            ));
        }
    };
    Ok((file, range))
}

/// The byte range of one source's `<tola-meta>` declaration, as its two integer bounds.
fn declaration_bounds(path: &str, range: &Array) -> Result<std::ops::Range<usize>, EcoString> {
    let mut bounds = range.iter();
    let bound = |value: Option<&Value>, which: &str| -> Result<i64, EcoString> {
        match value {
            Some(Value::Int(bound)) => Ok(*bound),
            _ => Err(eco_format!(
                "`{path}` declares metadata at a range without a {which}"
            )),
        }
    };
    let start = bound(bounds.next(), "start")?;
    let end = bound(bounds.next(), "end")?;
    if start < 0 || end < start {
        return Err(eco_format!(
            "`{path}` declares metadata at a range that runs backwards: {start}..{end}"
        ));
    }
    Ok(start as usize..end as usize)
}
