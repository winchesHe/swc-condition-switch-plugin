use swc_core::ecma::{
    ast::*,
    visit::{Fold, FoldWith},
};
use swc_core::common::SyntaxContext;
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};
use serde::Deserialize;
use std::sync::Arc;

static CONDITION_TAG: &str = "Condition";
static SWITCH_TAG: &str = "Switch";
static IF_ATTR: &str = "if";
static SHORT_CIRCUIT_ATTR: &str = "shortCircuit";
static BOOLEAN_FUNC: &str = "Boolean";
static REACT_FRAGMENT: &str = "React.Fragment";
static CONDITION_PLACEHOLDER: &str = "__CONDITION_PLACEHOLDER__";
static SWITCH_PLACEHOLDER: &str = "__SWITCH_PLACEHOLDER__";

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
    null_expr: Arc<Expr>,
    boolean_ident: Arc<Ident>,
    react_fragment_ident: Arc<Ident>,
    condition_placeholder_ident: Arc<Ident>,
    syntax_context: SyntaxContext,
}

impl Default for TransformVisitor {
    fn default() -> Self {
        let span = swc_core::common::DUMMY_SP;
        let syntax_context = SyntaxContext::empty();
        Self {
            current_context: WrapperType::Jsx,
            null_expr: Arc::new(Expr::Lit(Lit::Null(Null { span }))),
            boolean_ident: Arc::new(Ident::new(BOOLEAN_FUNC.into(), span, syntax_context)),
            react_fragment_ident: Arc::new(Ident::new(REACT_FRAGMENT.into(), span, syntax_context)),
            condition_placeholder_ident: Arc::new(Ident::new(CONDITION_PLACEHOLDER.into(), span, syntax_context)),
            syntax_context,
        }
    }
}

impl Fold for TransformVisitor {
    fn fold_jsx_element(&mut self, element: JSXElement) -> JSXElement {
        if let JSXElementName::Ident(ident) = &element.opening.name {
            let tag_name = ident.sym.as_ref();
            if tag_name == CONDITION_TAG {
                if let Some(condition_expr) = self.extract_condition_from_attrs(&element.opening.attrs) {
                    return self.create_conditional_jsx(condition_expr, element.children, element.span);
                }
            } else if tag_name == SWITCH_TAG {
                if self.has_switch_case_children(&element.children) {
                    let short_circuit = self.extract_short_circuit_attr(&element.opening.attrs);
                    return self.create_switch_transformation(element.children, short_circuit, element.span);
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
                if self.current_context != WrapperType::Jsx {
                    let prev_context = std::mem::replace(&mut self.current_context, WrapperType::Jsx);
                    let result = JSXElementChild::JSXElement(Box::new(self.fold_jsx_element(*element)));
                    self.current_context = prev_context;
                    result
                } else {
                    JSXElementChild::JSXElement(Box::new(self.fold_jsx_element(*element)))
                }
            }
            JSXElementChild::JSXFragment(fragment) => {
                if self.current_context != WrapperType::Jsx {
                    let prev_context = std::mem::replace(&mut self.current_context, WrapperType::Jsx);
                    let result = JSXElementChild::JSXFragment(self.fold_jsx_fragment(fragment));
                    self.current_context = prev_context;
                    result
                } else {
                    JSXElementChild::JSXFragment(self.fold_jsx_fragment(fragment))
                }
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
        if self.current_context != WrapperType::Jsx {
            let prev_context = std::mem::replace(&mut self.current_context, WrapperType::Jsx);
            container.expr = container.expr.fold_with(self);
            self.current_context = prev_context;
        } else {
            container.expr = container.expr.fold_with(self);
        }
        container
    }

    fn fold_jsx_expr(&mut self, expr: JSXExpr) -> JSXExpr {
        match expr {
            JSXExpr::Expr(e) => JSXExpr::Expr(e.fold_with(self)),
            _ => expr,
        }
    }

    fn fold_return_stmt(&mut self, mut stmt: ReturnStmt) -> ReturnStmt {
        let prev_context = std::mem::replace(&mut self.current_context, WrapperType::Return);
        stmt.arg = stmt.arg.fold_with(self);
        self.current_context = prev_context;
        stmt
    }

    fn fold_var_declarator(&mut self, mut declarator: VarDeclarator) -> VarDeclarator {
        let prev_context = std::mem::replace(&mut self.current_context, WrapperType::Assignment);
        declarator.init = declarator.init.fold_with(self);
        self.current_context = prev_context;
        declarator
    }

    fn fold_assign_expr(&mut self, mut expr: AssignExpr) -> AssignExpr {
        let prev_context = std::mem::replace(&mut self.current_context, WrapperType::Assignment);
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
        attrs.iter().find_map(|attr| {
            if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                if let JSXAttrName::Ident(name) = &jsx_attr.name {
                    if name.sym.as_ref() == IF_ATTR {
                        if let Some(JSXAttrValue::JSXExprContainer(expr_container)) = &jsx_attr.value {
                            if let JSXExpr::Expr(condition_expr) = &expr_container.expr {
                                return Some(condition_expr.clone());
                            }
                        }
                    }
                }
            }
            None
        })
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
                    callee: Callee::Expr(Box::new(Expr::Ident((*self.boolean_ident).clone()))),
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
            alt: Box::new((*self.null_expr).clone()),
        });

        match current_context {
            WrapperType::Jsx => {
                JSXElement {
                    span,
                    opening: JSXOpeningElement {
                        span,
                        name: JSXElementName::Ident((*self.react_fragment_ident).clone()),
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
                        name: JSXElementName::Ident((*self.react_fragment_ident).clone()),
                    }),
                }
            }
            WrapperType::Return | WrapperType::Assignment => {
                JSXElement {
                    span,
                    opening: JSXOpeningElement {
                        span,
                        name: JSXElementName::Ident((*self.condition_placeholder_ident).clone()),
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
                        name: JSXElementName::Ident((*self.condition_placeholder_ident).clone()),
                    }),
                }
            }
        }
    }

    fn is_switch_case_element(&self, element: &JSXElement) -> bool {
        matches!(&element.opening.name, 
            JSXElementName::JSXMemberExpr(member) 
                if matches!(&member.obj, JSXObject::Ident(obj) 
                    if obj.sym.as_ref() == "Switch" && member.prop.sym.as_ref() == "Case"))
    }

    fn has_switch_case_children(&self, children: &[JSXElementChild]) -> bool {
        children.iter().any(|child| {
            if let JSXElementChild::JSXElement(element) = child {
                self.is_switch_case_element(element)
            } else {
                false
            }
        })
    }

    fn extract_short_circuit_attr(&self, attrs: &[JSXAttrOrSpread]) -> bool {
        attrs.iter().any(|attr| {
            matches!(attr, JSXAttrOrSpread::JSXAttr(jsx_attr) 
                if matches!(&jsx_attr.name, JSXAttrName::Ident(name) 
                    if name.sym.as_ref() == SHORT_CIRCUIT_ATTR))
        })
    }

    fn create_switch_transformation(&self, children: Vec<JSXElementChild>, short_circuit: bool, span: swc_core::common::Span) -> JSXElement {
        let switch_cases: Vec<_> = children.into_iter()
            .filter_map(|child| {
                if let JSXElementChild::JSXElement(element) = child {
                    if self.is_switch_case_element(&element) {
                        if let Some(condition_expr) = self.extract_condition_from_attrs(&element.opening.attrs) {
                            return Some((condition_expr, element.children));
                        }
                    }
                }
                None
            })
            .collect();

        if switch_cases.is_empty() {
            return JSXElement {
                span,
                opening: JSXOpeningElement {
                    span,
                    name: JSXElementName::Ident(Ident::new(REACT_FRAGMENT.into(), span, self.syntax_context)),
                    attrs: vec![],
                    self_closing: false,
                    type_args: None,
                },
                children: vec![],
                closing: Some(JSXClosingElement {
                    span,
                    name: JSXElementName::Ident(Ident::new(REACT_FRAGMENT.into(), span, self.syntax_context)),
                }),
            };
        }

        let current_context = self.get_current_context();
        let effective_short_circuit = short_circuit || 
            (matches!(current_context, WrapperType::Return | WrapperType::Assignment) && switch_cases.len() == 1);
        
        if effective_short_circuit {
            self.create_short_circuit_switch(switch_cases, span)
        } else {
            self.create_parallel_switch(switch_cases, span)
        }
    }

    fn create_short_circuit_switch(&self, switch_cases: Vec<(Box<Expr>, Vec<JSXElementChild>)>, span: swc_core::common::Span) -> JSXElement {
        let mut result_expr = Box::new((*self.null_expr).clone());
        let current_context = self.get_current_context();
        
        for (condition, children) in switch_cases.into_iter().rev() {
            let test_expr = match current_context {
                WrapperType::Return | WrapperType::Assignment => *condition,
                WrapperType::Jsx => {
                    Expr::Call(CallExpr {
                        span,
                        callee: Callee::Expr(Box::new(Expr::Ident((*self.boolean_ident).clone()))),
                        args: vec![ExprOrSpread {
                            spread: None,
                            expr: condition,
                        }],
                        type_args: None,
                        ctxt: self.syntax_context,
                    })
                }
            };

            let non_whitespace_children: Vec<_> = children.into_iter()
                .filter(|child| {
                    match child {
                        JSXElementChild::JSXText(text) => !text.value.trim().is_empty(),
                        _ => true,
                    }
                })
                .collect();

            let fragment_expr = if non_whitespace_children.len() == 1 {
                let first_child = non_whitespace_children.into_iter().next().unwrap();
                if let JSXElementChild::JSXElement(element) = first_child {
                    Expr::JSXElement(element)
                } else {
                    let fragment = JSXFragment {
                        span,
                        opening: JSXOpeningFragment { span },
                        children: vec![first_child],
                        closing: JSXClosingFragment { span },
                    };
                    Expr::JSXFragment(fragment)
                }
            } else {
                let fragment = JSXFragment {
                    span,
                    opening: JSXOpeningFragment { span },
                    children: non_whitespace_children,
                    closing: JSXClosingFragment { span },
                };
                Expr::JSXFragment(fragment)
            };

            result_expr = Box::new(Expr::Cond(CondExpr {
                span,
                test: Box::new(test_expr),
                cons: Box::new(fragment_expr),
                alt: result_expr,
            }));
        }

        match current_context {
            WrapperType::Return | WrapperType::Assignment => {
                JSXElement {
                    span,
                    opening: JSXOpeningElement {
                        span,
                        name: JSXElementName::Ident((*self.condition_placeholder_ident).clone()),
                        attrs: vec![],
                        self_closing: false,
                        type_args: None,
                    },
                    children: vec![JSXElementChild::JSXExprContainer(JSXExprContainer {
                        span,
                        expr: JSXExpr::Expr(result_expr),
                    })],
                    closing: Some(JSXClosingElement {
                        span,
                        name: JSXElementName::Ident((*self.condition_placeholder_ident).clone()),
                    }),
                }
            }
            WrapperType::Jsx => {
                JSXElement {
                    span,
                    opening: JSXOpeningElement {
                        span,
                        name: JSXElementName::Ident((*self.react_fragment_ident).clone()),
                        attrs: vec![],
                        self_closing: false,
                        type_args: None,
                    },
                    children: vec![JSXElementChild::JSXExprContainer(JSXExprContainer {
                        span,
                        expr: JSXExpr::Expr(result_expr),
                    })],
                    closing: Some(JSXClosingElement {
                        span,
                        name: JSXElementName::Ident((*self.react_fragment_ident).clone()),
                    }),
                }
            }
        }
    }

    fn create_parallel_switch(&self, switch_cases: Vec<(Box<Expr>, Vec<JSXElementChild>)>, span: swc_core::common::Span) -> JSXElement {
        let mut result_children = vec![];

        for (condition, children) in switch_cases {
            let fragment = JSXFragment {
                span,
                opening: JSXOpeningFragment { span },
                children,
                closing: JSXClosingFragment { span },
            };

            let conditional_expr = Expr::Cond(CondExpr {
                span,
                test: condition,
                cons: Box::new(Expr::JSXFragment(fragment)),
                alt: Box::new((*self.null_expr).clone()),
            });

            result_children.push(JSXElementChild::JSXExprContainer(JSXExprContainer {
                span,
                expr: JSXExpr::Expr(Box::new(conditional_expr)),
            }));
        }

        JSXElement {
            span,
            opening: JSXOpeningElement {
                span,
                name: JSXElementName::Ident(Ident::new(REACT_FRAGMENT.into(), span, self.syntax_context)),
                attrs: vec![],
                self_closing: false,
                type_args: None,
            },
            children: result_children,
            closing: Some(JSXClosingElement {
                span,
                name: JSXElementName::Ident(Ident::new(REACT_FRAGMENT.into(), span, self.syntax_context)),
            }),
        }
    }
}

pub struct PostTransformVisitor;

impl PostTransformVisitor {
    fn unwrap_single_element_fragments(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Cond(mut cond_expr) => {
                if let Expr::JSXFragment(fragment) = &*cond_expr.cons {
                    let non_whitespace_children: Vec<_> = fragment.children.iter()
                        .filter(|child| {
                            match child {
                                JSXElementChild::JSXText(text) => !text.value.trim().is_empty(),
                                _ => true,
                            }
                        })
                        .collect();
                    
                    if non_whitespace_children.len() == 1 {
                        if let Some(JSXElementChild::JSXElement(element)) = non_whitespace_children.first() {
                            cond_expr.cons = Box::new(Expr::JSXElement((*element).clone()));
                        }
                    }
                }
                
                cond_expr.alt = Box::new(self.unwrap_single_element_fragments(*cond_expr.alt));
                
                Expr::Cond(cond_expr)
            }
            _ => expr,
        }
    }
}

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
                    } else if ident.sym.as_ref() == SWITCH_PLACEHOLDER {
                        if let Some(JSXElementChild::JSXExprContainer(container)) = element.children.first() {
                            if let JSXExpr::Expr(inner_expr) = &container.expr {
                                return self.unwrap_single_element_fragments((**inner_expr).clone());
                            }
                        }
                    }
                }
                Expr::JSXElement(Box::new(self.fold_jsx_element(*element)))
            }
            Expr::Paren(paren_expr) => {
                let inner = self.fold_expr(*paren_expr.expr);
                match &inner {
                    Expr::Cond(cond_expr) => {
                        let needs_inner_parens = match cond_expr.cons.as_ref() {
                            Expr::JSXElement(elem) => {
                                elem.children.len() > 1 || 
                                elem.children.iter().any(|child| match child {
                                    JSXElementChild::JSXText(text) => text.value.contains('\n'),
                                    _ => false,
                                })
                            },
                            Expr::JSXFragment(_) => true,
                            _ => false
                        };
                        
                        if needs_inner_parens {
                            let mut new_cond = cond_expr.clone();
                            new_cond.cons = Box::new(Expr::Paren(ParenExpr {
                                span: cond_expr.span,
                                expr: cond_expr.cons.clone(),
                            }));
                            Expr::Cond(new_cond)
                        } else {
                            inner
                        }
                    }
                    _ => Expr::Paren(ParenExpr {
                        span: paren_expr.span,
                        expr: Box::new(inner),
                    })
                }
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

