use swc_core::ecma::{
    ast::*,
    visit::{Fold, FoldWith},
};
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {}

#[derive(Debug, Clone, PartialEq)]
enum WrapperType {
    Jsx,
    Assignment,
    Return,
}

pub struct TransformVisitor {
    context_stack: Vec<WrapperType>,
}

impl Default for TransformVisitor {
    fn default() -> Self {
        Self {
            context_stack: vec![WrapperType::Jsx],
        }
    }
}

impl Fold for TransformVisitor {
    fn fold_jsx_element(&mut self, element: JSXElement) -> JSXElement {
        if let JSXElementName::Ident(ident) = &element.opening.name {
            if ident.sym.as_ref() == "Condition" {
                if let Some(condition_expr) = self.extract_condition_from_attrs(&element.opening.attrs) {
                    return self.create_conditional_jsx(condition_expr, element.children, element.span);
                }
            }
        }

        let mut new_element = element;
        new_element.children = new_element.children.fold_with(self);
        new_element
    }

    fn fold_jsx_element_child(&mut self, child: JSXElementChild) -> JSXElementChild {
        match child {
            JSXElementChild::JSXElement(element) => {
                self.context_stack.push(WrapperType::Jsx);
                let result = JSXElementChild::JSXElement(Box::new(self.fold_jsx_element(*element)));
                self.context_stack.pop();
                result
            }
            JSXElementChild::JSXFragment(fragment) => {
                self.context_stack.push(WrapperType::Jsx);
                let result = JSXElementChild::JSXFragment(self.fold_jsx_fragment(fragment));
                self.context_stack.pop();
                result
            }
            JSXElementChild::JSXExprContainer(container) => {
                JSXElementChild::JSXExprContainer(self.fold_jsx_expr_container(container))
            }
            _ => child,
        }
    }

    fn fold_jsx_fragment(&mut self, mut fragment: JSXFragment) -> JSXFragment {
        fragment.children = fragment.children.fold_with(self);
        fragment
    }

    fn fold_jsx_expr_container(&mut self, mut container: JSXExprContainer) -> JSXExprContainer {
        self.context_stack.push(WrapperType::Jsx);
        container.expr = container.expr.fold_with(self);
        self.context_stack.pop();
        container
    }

    fn fold_jsx_expr(&mut self, expr: JSXExpr) -> JSXExpr {
        match expr {
            JSXExpr::Expr(e) => JSXExpr::Expr(e.fold_with(self)),
            _ => expr,
        }
    }

    fn fold_return_stmt(&mut self, mut stmt: ReturnStmt) -> ReturnStmt {
        self.context_stack.push(WrapperType::Return);
        stmt.arg = stmt.arg.fold_with(self);
        self.context_stack.pop();
        stmt
    }

    fn fold_var_declarator(&mut self, mut declarator: VarDeclarator) -> VarDeclarator {
        self.context_stack.push(WrapperType::Assignment);
        declarator.init = declarator.init.fold_with(self);
        self.context_stack.pop();
        declarator
    }

    fn fold_assign_expr(&mut self, mut expr: AssignExpr) -> AssignExpr {
        self.context_stack.push(WrapperType::Assignment);
        expr.right = expr.right.fold_with(self);
        self.context_stack.pop();
        expr
    }

    fn fold_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::JSXElement(element) => {
                Expr::JSXElement(Box::new(self.fold_jsx_element(*element)))
            }
            Expr::JSXFragment(fragment) => {
                Expr::JSXFragment(self.fold_jsx_fragment(fragment))
            }
            _ => expr.fold_children_with(self),
        }
    }
}

impl TransformVisitor {
    fn extract_condition_from_attrs(&self, attrs: &[JSXAttrOrSpread]) -> Option<Box<Expr>> {
        for attr in attrs {
            if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                if let JSXAttrName::Ident(name) = &jsx_attr.name {
                    if name.sym.as_ref() == "if" {
                        if let Some(JSXAttrValue::JSXExprContainer(expr_container)) = &jsx_attr.value {
                            if let JSXExpr::Expr(condition_expr) = &expr_container.expr {
                                return Some(condition_expr.clone());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn get_current_context(&self) -> &WrapperType {
        self.context_stack.last().unwrap_or(&WrapperType::Jsx)
    }

    fn create_conditional_jsx(&self, condition: Box<Expr>, children: Vec<JSXElementChild>, span: swc_core::common::Span) -> JSXElement {
        let fragment = JSXFragment {
            span,
            opening: JSXOpeningFragment { span },
            children,
            closing: JSXClosingFragment { span },
        };

        let current_context = self.get_current_context();
        
        let test_expr = match current_context {
            WrapperType::Return => *condition,
            WrapperType::Assignment | WrapperType::Jsx => {
                Expr::Call(CallExpr {
                    span,
                    callee: Callee::Expr(Box::new(Expr::Ident(Ident::new("Boolean".into(), span)))),
                    args: vec![ExprOrSpread {
                        spread: None,
                        expr: condition,
                    }],
                    type_args: None,
                })
            }
        };

        let conditional_expr = Expr::Cond(CondExpr {
            span,
            test: Box::new(test_expr),
            cons: Box::new(Expr::JSXFragment(fragment)),
            alt: Box::new(Expr::Lit(Lit::Null(Null { span }))),
        });

        match current_context {
            WrapperType::Jsx => {
                JSXElement {
                    span,
                    opening: JSXOpeningElement {
                        span,
                        name: JSXElementName::Ident(Ident::new("React.Fragment".into(), span)),
                        attrs: vec![],
                        self_closing: false,
                        type_args: None,
                    },
                    children: vec![JSXElementChild::JSXExprContainer(JSXExprContainer {
                        span,
                        expr: JSXExpr::Expr(Box::new(conditional_expr)),
                    })],
                    closing: Some(JSXClosingElement {
                        span,
                        name: JSXElementName::Ident(Ident::new("React.Fragment".into(), span)),
                    }),
                }
            }
            WrapperType::Return | WrapperType::Assignment => {
                JSXElement {
                    span,
                    opening: JSXOpeningElement {
                        span,
                        name: JSXElementName::Ident(Ident::new("__CONDITION_PLACEHOLDER__".into(), span)),
                        attrs: vec![],
                        self_closing: false,
                        type_args: None,
                    },
                    children: vec![JSXElementChild::JSXExprContainer(JSXExprContainer {
                        span,
                        expr: JSXExpr::Expr(Box::new(conditional_expr)),
                    })],
                    closing: Some(JSXClosingElement {
                        span,
                        name: JSXElementName::Ident(Ident::new("__CONDITION_PLACEHOLDER__".into(), span)),
                    }),
                }
            }
        }
    }
}

pub struct PostTransformVisitor;

impl Fold for PostTransformVisitor {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::JSXElement(element) => {
                if let JSXElementName::Ident(ident) = &element.opening.name {
                    if ident.sym.as_ref() == "__CONDITION_PLACEHOLDER__" {
                        if let Some(JSXElementChild::JSXExprContainer(container)) = element.children.first() {
                            if let JSXExpr::Expr(inner_expr) = &container.expr {
                                return *inner_expr.clone();
                            }
                        }
                    }
                }
                Expr::JSXElement(Box::new(self.fold_jsx_element(*element)))
            }
            _ => expr.fold_children_with(self),
        }
    }

    fn fold_jsx_element(&mut self, mut element: JSXElement) -> JSXElement {
        element.children = element.children.fold_with(self);
        element
    }
}

#[plugin_transform]
pub fn process_transform(program: Program, _metadata: TransformPluginProgramMetadata) -> Program {
    let transformed = program.fold_with(&mut TransformVisitor::default());
    transformed.fold_with(&mut PostTransformVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_jsx_condition() {
        let input = r#"
        function App({ showMessage }) {
          return (
            <div>
              <Condition if={showMessage}>
                <p>Hello World</p>
              </Condition>
            </div>
          )
        }
        "#;

        let expected = r#"
        function App({ showMessage }) {
          return (
            <div>
              <React.Fragment>{Boolean(showMessage) ? <><p>Hello World</p></> : null}</React.Fragment>
            </div>
          )
        }
        "#;

        test_transform(input, expected);
    }

    #[test]
    fn test_return_condition() {
        let input = r#"
        function App({ condition }) {
          return <Condition if={condition}>
            <div>Return context</div>
          </Condition>
        }
        "#;

        let expected = r#"
        function App({ condition }) {
          return condition ? <><div>Return context</div></> : null
        }
        "#;

        test_transform(input, expected);
    }

    #[test]
    fn test_assignment_condition() {
        let input = r#"
        function App({ condition }) {
          const element = <Condition if={condition}>
            <span>Expression context</span>
          </Condition>
          return element
        }
        "#;

        let expected = r#"
        function App({ condition }) {
          const element = Boolean(condition) ? <><span>Expression context</span></> : null
          return element
        }
        "#;

        test_transform(input, expected);
    }

    fn test_transform(input: &str, expected: &str) {
        use swc_core::ecma::{
            parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig},
            codegen::{text_writer::JsWriter, Emitter},
        };
        use swc_core::common::SourceMap;
        use std::sync::Arc;

        let syntax = Syntax::Typescript(TsConfig {
            tsx: true,
            ..Default::default()
        });

        let cm = Arc::new(SourceMap::default());
        let lexer = Lexer::new(
            syntax,
            Default::default(),
            StringInput::new(input, Default::default(), Default::default()),
            None,
        );
        let mut parser = Parser::new_from(lexer);
        let module = parser.parse_module().expect("Failed to parse input");

        let transformed = module.fold_with(&mut TransformVisitor::default());
        let final_result = transformed.fold_with(&mut PostTransformVisitor);

        let mut buf = vec![];
        {
            let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
            let mut emitter = Emitter {
                cfg: Default::default(),
                cm: cm.clone(),
                comments: None,
                wr: writer,
            };
            emitter.emit_module(&final_result).expect("Failed to emit");
        }

        let output = String::from_utf8(buf).expect("Invalid UTF-8");
        
        let cleaned_output = output
            .replace("<__CONDITION_PLACEHOLDER__>", "")
            .replace("</__CONDITION_PLACEHOLDER__>", "")
            .replace("<__DIRECT_EXPR__>", "")
            .replace("</__DIRECT_EXPR__>", "");

        let normalize = |s: &str| {
            s.trim()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .replace("< >", "<>")
                .replace("< / >", "</>")
                .replace("<> ", "<>")
                .replace(" </>", "</>")
                .replace("( ", "(")
                .replace(" )", ")")
                .replace("{ ", "{")
                .replace(" }", "}")
                .replace(" ;", "")
                .replace(";", "")
        };
        
        assert_eq!(
            normalize(&cleaned_output),
            normalize(expected),
            "Transform output doesn't match expected result.\nActual: {}\nExpected: {}",
            cleaned_output,
            expected
        );
    }
}
