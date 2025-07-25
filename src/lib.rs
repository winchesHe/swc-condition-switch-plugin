use swc_core::ecma::{
    ast::*,
    visit::{Fold, FoldWith},
};
use swc_core::common::SyntaxContext;
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};
use serde::Deserialize;

const CONDITION_TAG: &str = "Condition";
const IF_ATTR: &str = "if";
const BOOLEAN_FUNC: &str = "Boolean";
const REACT_FRAGMENT: &str = "React.Fragment";
const CONDITION_PLACEHOLDER: &str = "__CONDITION_PLACEHOLDER__";

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
    current_context: WrapperType,
    null_expr: Box<Expr>,
    syntax_context: SyntaxContext,
}

impl Default for TransformVisitor {
    fn default() -> Self {
        let span = swc_core::common::DUMMY_SP;
        Self {
            current_context: WrapperType::Jsx,
            null_expr: Box::new(Expr::Lit(Lit::Null(Null { span }))),
            syntax_context: SyntaxContext::empty(),
        }
    }
}

impl Fold for TransformVisitor {
    fn fold_jsx_element(&mut self, element: JSXElement) -> JSXElement {
        if let JSXElementName::Ident(ident) = &element.opening.name {
            if ident.sym.as_ref() == CONDITION_TAG {
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
                let prev_context = self.current_context.clone();
                self.current_context = WrapperType::Jsx;
                let result = JSXElementChild::JSXElement(Box::new(self.fold_jsx_element(*element)));
                self.current_context = prev_context;
                result
            }
            JSXElementChild::JSXFragment(fragment) => {
                let prev_context = self.current_context.clone();
                self.current_context = WrapperType::Jsx;
                let result = JSXElementChild::JSXFragment(self.fold_jsx_fragment(fragment));
                self.current_context = prev_context;
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
        let prev_context = self.current_context.clone();
        self.current_context = WrapperType::Jsx;
        container.expr = container.expr.fold_with(self);
        self.current_context = prev_context;
        container
    }

    fn fold_jsx_expr(&mut self, expr: JSXExpr) -> JSXExpr {
        match expr {
            JSXExpr::Expr(e) => JSXExpr::Expr(e.fold_with(self)),
            _ => expr,
        }
    }

    fn fold_return_stmt(&mut self, mut stmt: ReturnStmt) -> ReturnStmt {
        let prev_context = self.current_context.clone();
        self.current_context = WrapperType::Return;
        stmt.arg = stmt.arg.fold_with(self);
        self.current_context = prev_context;
        stmt
    }

    fn fold_var_declarator(&mut self, mut declarator: VarDeclarator) -> VarDeclarator {
        let prev_context = self.current_context.clone();
        self.current_context = WrapperType::Assignment;
        declarator.init = declarator.init.fold_with(self);
        self.current_context = prev_context;
        declarator
    }

    fn fold_assign_expr(&mut self, mut expr: AssignExpr) -> AssignExpr {
        let prev_context = self.current_context.clone();
        self.current_context = WrapperType::Assignment;
        expr.right = expr.right.fold_with(self);
        self.current_context = prev_context;
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
                    if name.sym.as_ref() == IF_ATTR {
                        if let Some(JSXAttrValue::JSXExprContainer(expr_container)) = &jsx_attr.value {
                            if let JSXExpr::Expr(condition_expr) = &expr_container.expr {
                                return Some(condition_expr.clone());
                            }
                        }
                        return None;
                    }
                }
            }
        }
        None
    }

    fn get_current_context(&self) -> &WrapperType {
        &self.current_context
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
                    callee: Callee::Expr(Box::new(Expr::Ident(Ident::new(BOOLEAN_FUNC.into(), span, self.syntax_context)))),
                    args: vec![ExprOrSpread {
                        spread: None,
                        expr: condition,
                    }],
                    type_args: None,
                    ctxt: self.syntax_context,
                })
            }
        };

        let conditional_expr = Expr::Cond(CondExpr {
            span,
            test: Box::new(test_expr),
            cons: Box::new(Expr::JSXFragment(fragment)),
            alt: self.null_expr.clone(),
        });

        match current_context {
            WrapperType::Jsx => {
                JSXElement {
                    span,
                    opening: JSXOpeningElement {
                        span,
                        name: JSXElementName::Ident(Ident::new(REACT_FRAGMENT.into(), span, self.syntax_context)),
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
                        name: JSXElementName::Ident(Ident::new(REACT_FRAGMENT.into(), span, self.syntax_context)),
                    }),
                }
            }
            WrapperType::Return | WrapperType::Assignment => {
                JSXElement {
                    span,
                    opening: JSXOpeningElement {
                        span,
                        name: JSXElementName::Ident(Ident::new(CONDITION_PLACEHOLDER.into(), span, self.syntax_context)),
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
                        name: JSXElementName::Ident(Ident::new(CONDITION_PLACEHOLDER.into(), span, self.syntax_context)),
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
                    if ident.sym.as_ref() == CONDITION_PLACEHOLDER {
                        if let Some(JSXElementChild::JSXExprContainer(container)) = element.children.first() {
                            if let JSXExpr::Expr(inner_expr) = &container.expr {
                                return (**inner_expr).clone();
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

