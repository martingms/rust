use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::higher::VecArgs;
use clippy_utils::last_path_segment;
use clippy_utils::macros::root_macro_call_first_node;
use clippy_utils::source::{indent_of, snippet};
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind, QPath, TyKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_session::{declare_lint_pass, declare_tool_lint};
use rustc_span::{sym, Span, Symbol};

declare_clippy_lint! {
    /// ### What it does
    /// Checks for `Arc::new` or `Rc::new` in `vec![elem; len]`
    ///
    /// ### Why is this bad?
    /// This will create `elem` once and clone it `len` times - doing so with `Arc` or `Rc`
    /// is a bit misleading, as it will create references to the same pointer, rather
    /// than different instances.
    ///
    /// ### Example
    /// ```rust
    /// let v = vec![std::sync::Arc::new("some data".to_string()); 100];
    /// // or
    /// let v = vec![std::rc::Rc::new("some data".to_string()); 100];
    /// ```
    /// Use instead:
    /// ```rust
    ///
    /// // Initialize each value separately:
    /// let mut data = Vec::with_capacity(100);
    /// for _ in 0..100 {
    ///     data.push(std::rc::Rc::new("some data".to_string()));
    /// }
    ///
    /// // Or if you want clones of the same reference,
    /// // Create the reference beforehand to clarify that
    /// // it should be cloned for each value
    /// let data = std::rc::Rc::new("some data".to_string());
    /// let v = vec![data; 100];
    /// ```
    #[clippy::version = "1.62.0"]
    pub RC_CLONE_IN_VEC_INIT,
    suspicious,
    "initializing `Arc` or `Rc` in `vec![elem; len]`"
}
declare_lint_pass!(RcCloneInVecInit => [RC_CLONE_IN_VEC_INIT]);

impl LateLintPass<'_> for RcCloneInVecInit {
    fn check_expr(&mut self, cx: &LateContext<'_>, expr: &Expr<'_>) {
        let Some(macro_call) = root_macro_call_first_node(cx, expr) else { return; };
        let Some(VecArgs::Repeat(elem, len)) = VecArgs::hir(cx, expr) else { return; };
        let Some(symbol) = new_reference_call(cx, elem) else { return; };

        emit_lint(cx, symbol, macro_call.span, elem, len);
    }
}

struct LintSuggestion {
    message: String,
    snippet: String,
}

fn construct_lint_suggestions(
    cx: &LateContext<'_>,
    span: Span,
    symbol_name: &str,
    elem: &Expr<'_>,
    len: &Expr<'_>,
) -> Vec<LintSuggestion> {
    let len_snippet = snippet(cx, len.span, "..");
    let elem_snippet = elem_snippet(cx, elem, symbol_name);
    let indentation = indent_of(cx, span).unwrap_or(0);
    let indentation = " ".repeat(indentation);
    let loop_init_suggestion = loop_init_suggestion(&elem_snippet, len_snippet.as_ref(), &indentation);
    let extract_suggestion = extract_suggestion(&elem_snippet, len_snippet.as_ref(), &indentation);

    vec![
        LintSuggestion {
            message: format!("consider initializing each `{symbol_name}` element individually"),
            snippet: loop_init_suggestion,
        },
        LintSuggestion {
            message: format!(
                "or if this is intentional, consider extracting the `{symbol_name}` initialization to a variable"
            ),
            snippet: extract_suggestion,
        },
    ]
}

fn elem_snippet(cx: &LateContext<'_>, elem: &Expr<'_>, symbol_name: &str) -> String {
    let elem_snippet = snippet(cx, elem.span, "..").to_string();
    if elem_snippet.contains('\n') {
        // This string must be found in `elem_snippet`, otherwise we won't be constructing
        // the snippet in the first place.
        let reference_creation = format!("{symbol_name}::new");
        let (code_until_reference_creation, _right) = elem_snippet.split_once(&reference_creation).unwrap();

        return format!("{code_until_reference_creation}{reference_creation}(..)");
    }

    elem_snippet
}

fn loop_init_suggestion(elem: &str, len: &str, indent: &str) -> String {
    format!(
        r#"{{
{indent}{indent}let mut v = Vec::with_capacity({len});
{indent}{indent}(0..{len}).for_each(|_| v.push({elem}));
{indent}{indent}v
{indent}}}"#
    )
}

fn extract_suggestion(elem: &str, len: &str, indent: &str) -> String {
    format!(
        "{{
{indent}{indent}let data = {elem};
{indent}{indent}vec![data; {len}]
{indent}}}"
    )
}

fn emit_lint(cx: &LateContext<'_>, symbol: Symbol, lint_span: Span, elem: &Expr<'_>, len: &Expr<'_>) {
    let symbol_name = symbol.as_str();

    span_lint_and_then(
        cx,
        RC_CLONE_IN_VEC_INIT,
        lint_span,
        &format!("calling `{symbol_name}::new` in `vec![elem; len]`"),
        |diag| {
            let suggestions = construct_lint_suggestions(cx, lint_span, symbol_name, elem, len);

            diag.note(format!("each element will point to the same `{symbol_name}` instance"));
            for suggestion in suggestions {
                diag.span_suggestion(
                    lint_span,
                    &suggestion.message,
                    &suggestion.snippet,
                    Applicability::Unspecified,
                );
            }
        },
    );
}

/// Checks whether the given `expr` is a call to `Arc::new` or `Rc::new`
fn new_reference_call(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<Symbol> {
    if_chain! {
        if let ExprKind::Call(func, _args) = expr.kind;
        if let ExprKind::Path(ref func_path @ QPath::TypeRelative(ty, _)) = func.kind;
        if let TyKind::Path(ref ty_path) = ty.kind;
        if let Some(def_id) = cx.qpath_res(ty_path, ty.hir_id).opt_def_id();
        if last_path_segment(func_path).ident.name == sym::new;

        then {
            return cx.tcx.get_diagnostic_name(def_id).filter(|symbol| symbol == &sym::Arc || symbol == &sym::Rc);
        }
    }

    None
}
