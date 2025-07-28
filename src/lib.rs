use swc_core::ecma::{
    ast::*,
    visit::{Fold, FoldWith},
};
use swc_core::common::SyntaxContext;
use swc_core::atoms::Atom;
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};
use serde::Deserialize;
// removed Arc usage after switching to by-value caching of frequently used nodes

static CONDITION_TAG: &str = "Condition";
static SWITCH_TAG: &str = "Switch";
static IF_ATTR: &str = "if";
static ELSE_ATTR: &str = "else";
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
    // Cache frequently used small nodes directly; `Arc` adds atomic ref-counting overhead that
    // isn’t required because the visitor lives on a single thread. Storing the values by
    // value keeps them in the same cache line and makes `clone()` just a cheap `Copy` of a
    // few bytes instead of an atomic ref-count update.
    null_expr: Expr,
    boolean_ident: Ident,
    react_fragment_ident: Ident,
    condition_placeholder_ident: Ident,
    syntax_context: SyntaxContext,
    // Pre-computed atoms for fast string comparison
    condition_atom: Atom,
    switch_atom: Atom,
    if_atom: Atom,
    else_atom: Atom,
    short_circuit_atom: Atom,
}

impl Default for TransformVisitor {
    fn default() -> Self {
        let span = swc_core::common::DUMMY_SP;
        let syntax_context = SyntaxContext::empty();
        Self {
            current_context: WrapperType::Jsx,
            null_expr: Expr::Lit(Lit::Null(Null { span })),
            boolean_ident: Ident::new(BOOLEAN_FUNC.into(), span, syntax_context),
            react_fragment_ident: Ident::new(REACT_FRAGMENT.into(), span, syntax_context),
            condition_placeholder_ident: Ident::new(CONDITION_PLACEHOLDER.into(), span, syntax_context),
            syntax_context,
            condition_atom: CONDITION_TAG.into(),
            switch_atom: SWITCH_TAG.into(),
            if_atom: IF_ATTR.into(),
            else_atom: ELSE_ATTR.into(),
            short_circuit_atom: SHORT_CIRCUIT_ATTR.into(),
        }
    }
}

impl Fold for TransformVisitor {
    fn fold_jsx_element(&mut self, element: JSXElement) -> JSXElement {
        if let JSXElementName::Ident(ident) = &element.opening.name {
            if ident.sym == self.condition_atom {
                if let Some(condition_expr) = self.extract_condition_from_attrs(&element.opening.attrs) {
                    return self.create_conditional_jsx(condition_expr, element.children, element.span);
                }
            } else if ident.sym == self.switch_atom {
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
                JSXElementChild::JSXElement(Box::new(self.with_jsx_context(|visitor| visitor.fold_jsx_element(*element))))
            }
            JSXElementChild::JSXFragment(fragment) => {
                JSXElementChild::JSXFragment(self.with_jsx_context(|visitor| visitor.fold_jsx_fragment(fragment)))
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
        container.expr = self.with_jsx_context(|visitor| container.expr.fold_with(visitor));
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
        for attr in attrs {
            if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                if let JSXAttrName::Ident(name) = &jsx_attr.name {
                    if name.sym == self.if_atom {
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

    fn has_else_attr(&self, attrs: &[JSXAttrOrSpread]) -> bool {
        attrs.iter().any(|attr| {
            matches!(attr, JSXAttrOrSpread::JSXAttr(jsx_attr) 
                if matches!(&jsx_attr.name, JSXAttrName::Ident(name) 
                    if name.sym == self.else_atom))
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
                    callee: Callee::Expr(Box::new(Expr::Ident(self.boolean_ident.clone()))),
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
            alt: Box::new(self.null_expr.clone()),
        });

        match current_context {
            WrapperType::Jsx => {
                JSXElement {
                    span,
                    opening: JSXOpeningElement {
                        span,
                        name: JSXElementName::Ident(self.react_fragment_ident.clone()),
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
                        name: JSXElementName::Ident(self.react_fragment_ident.clone()),
                    }),
                }
            }
            WrapperType::Return | WrapperType::Assignment => {
                JSXElement {
                    span,
                    opening: JSXOpeningElement {
                        span,
                        name: JSXElementName::Ident(self.condition_placeholder_ident.clone()),
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
                        name: JSXElementName::Ident(self.condition_placeholder_ident.clone()),
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

    #[inline]
    fn has_switch_case_children(&self, children: &[JSXElementChild]) -> bool {
        children.iter().any(|child| matches!(child, JSXElementChild::JSXElement(elem) if self.is_switch_case_element(elem)))
    }

    fn extract_short_circuit_attr(&self, attrs: &[JSXAttrOrSpread]) -> bool {
        attrs.iter().any(|attr| {
            matches!(attr, JSXAttrOrSpread::JSXAttr(jsx_attr) 
                if matches!(&jsx_attr.name, JSXAttrName::Ident(name) 
                    if name.sym == self.short_circuit_atom))
        })
    }

    #[inline]
    fn is_non_whitespace_child(child: &JSXElementChild) -> bool {
        match child {
            JSXElementChild::JSXText(text) => !text.value.trim().is_empty(),
            _ => true,
        }
    }

    fn filter_non_whitespace_children(children: Vec<JSXElementChild>) -> Vec<JSXElementChild> {
        children.into_iter()
            .filter(Self::is_non_whitespace_child)
            .collect()
    }

    #[inline]
    fn with_jsx_context<T, F>(&mut self, f: F) -> T 
    where F: FnOnce(&mut Self) -> T {
        if self.current_context == WrapperType::Jsx {
            f(self)
        } else {
            let prev_context = std::mem::replace(&mut self.current_context, WrapperType::Jsx);
            let result = f(self);
            self.current_context = prev_context;
            result
        }
    }

    fn create_switch_transformation(&self, children: Vec<JSXElementChild>, short_circuit: bool, span: swc_core::common::Span) -> JSXElement {
        let mut switch_cases: Vec<_> = Vec::new();
        let mut else_case: Option<Vec<JSXElementChild>> = None;

        for child in children {
            if let JSXElementChild::JSXElement(element) = child {
                if self.is_switch_case_element(&element) {
                    if let Some(condition_expr) = self.extract_condition_from_attrs(&element.opening.attrs) {
                        switch_cases.push((condition_expr, element.children));
                    } else if self.has_else_attr(&element.opening.attrs) {
                        else_case = Some(element.children);
                    }
                }
            }
        }

        if switch_cases.is_empty() && else_case.is_none() {
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

        // 如果只有 else case，直接返回 else case 的内容
        if switch_cases.is_empty() && else_case.is_some() {
            let else_children = else_case.unwrap();
            let current_context = self.get_current_context();
            
            let non_whitespace_children = Self::filter_non_whitespace_children(else_children);
            
            if non_whitespace_children.len() == 1 {
                let mut children = non_whitespace_children;
                let first_child = children.into_iter().next().unwrap();
                if let JSXElementChild::JSXElement(element) = first_child {
                    match current_context {
                        WrapperType::Return | WrapperType::Assignment => {
                            return JSXElement {
                                span,
                                opening: JSXOpeningElement {
                                    span,
                                    name: JSXElementName::Ident(self.condition_placeholder_ident.clone()),
                                    attrs: vec![],
                                    self_closing: false,
                                    type_args: None,
                                },
                                children: vec![JSXElementChild::JSXExprContainer(JSXExprContainer {
                                    span,
                                    expr: JSXExpr::Expr(Box::new(Expr::JSXElement(element))),
                                })],
                                closing: Some(JSXClosingElement {
                                    span,
                                    name: JSXElementName::Ident(self.condition_placeholder_ident.clone()),
                                }),
                            };
                        }
                        WrapperType::Jsx => {
                            return *element;
                        }
                    }
                } else {
                    // 不是单个JSX元素，使用fragment
                    let fragment = JSXFragment {
                        span,
                        opening: JSXOpeningFragment { span },
                        children: vec![first_child],
                        closing: JSXClosingFragment { span },
                    };
                    
                    match current_context {
                        WrapperType::Return | WrapperType::Assignment => {
                            return JSXElement {
                                span,
                                opening: JSXOpeningElement {
                                    span,
                                    name: JSXElementName::Ident(self.condition_placeholder_ident.clone()),
                                    attrs: vec![],
                                    self_closing: false,
                                    type_args: None,
                                },
                                children: vec![JSXElementChild::JSXExprContainer(JSXExprContainer {
                                    span,
                                    expr: JSXExpr::Expr(Box::new(Expr::JSXFragment(fragment))),
                                })],
                                closing: Some(JSXClosingElement {
                                    span,
                                    name: JSXElementName::Ident(self.condition_placeholder_ident.clone()),
                                }),
                            };
                        }
                        WrapperType::Jsx => {
                            return JSXElement {
                                span,
                                opening: JSXOpeningElement {
                                    span,
                                    name: JSXElementName::Ident(self.react_fragment_ident.clone()),
                                    attrs: vec![],
                                    self_closing: false,
                                    type_args: None,
                                },
                                children: fragment.children,
                                closing: Some(JSXClosingElement {
                                    span,
                                    name: JSXElementName::Ident(self.react_fragment_ident.clone()),
                                }),
                            };
                        }
                    }
                }
            } else {
                // 多个子元素，使用fragment
                let fragment = JSXFragment {
                    span,
                    opening: JSXOpeningFragment { span },
                    children: non_whitespace_children,
                    closing: JSXClosingFragment { span },
                };
                
                match current_context {
                    WrapperType::Return | WrapperType::Assignment => {
                        return JSXElement {
                            span,
                            opening: JSXOpeningElement {
                                span,
                                name: JSXElementName::Ident(self.condition_placeholder_ident.clone()),
                                attrs: vec![],
                                self_closing: false,
                                type_args: None,
                            },
                            children: vec![JSXElementChild::JSXExprContainer(JSXExprContainer {
                                span,
                                expr: JSXExpr::Expr(Box::new(Expr::JSXFragment(fragment))),
                            })],
                            closing: Some(JSXClosingElement {
                                span,
                                name: JSXElementName::Ident(self.condition_placeholder_ident.clone()),
                            }),
                        };
                    }
                    WrapperType::Jsx => {
                        return JSXElement {
                            span,
                            opening: JSXOpeningElement {
                                span,
                                name: JSXElementName::Ident(self.react_fragment_ident.clone()),
                                attrs: vec![],
                                self_closing: false,
                                type_args: None,
                            },
                            children: fragment.children,
                            closing: Some(JSXClosingElement {
                                span,
                                name: JSXElementName::Ident(self.react_fragment_ident.clone()),
                            }),
                        };
                    }
                }
            }
        }

        let current_context = self.get_current_context();
        // 只有在用户明确指定 shortCircuit 时才使用短路模式
        // 或者在 return/assignment 上下文中只有一个 case 且没有 else 时
        let effective_short_circuit = short_circuit || 
            (matches!(current_context, WrapperType::Return | WrapperType::Assignment) && switch_cases.len() <= 1 && else_case.is_none());
        
        if effective_short_circuit {
            self.create_short_circuit_switch(switch_cases, else_case, span)
        } else {
            self.create_parallel_switch(switch_cases, else_case, span)
        }
    }

    fn create_short_circuit_switch(&self, switch_cases: Vec<(Box<Expr>, Vec<JSXElementChild>)>, else_case: Option<Vec<JSXElementChild>>, span: swc_core::common::Span) -> JSXElement {
        let mut result_expr = if let Some(else_children) = else_case {
            let non_whitespace_children = Self::filter_non_whitespace_children(else_children);
            if non_whitespace_children.len() == 1 {
                let first_child = non_whitespace_children.into_iter().next().unwrap();
                if let JSXElementChild::JSXElement(element) = first_child {
                    Box::new(Expr::JSXElement(element))
                } else {
                    let fragment = JSXFragment {
                        span,
                        opening: JSXOpeningFragment { span },
                        children: vec![first_child],
                        closing: JSXClosingFragment { span },
                    };
                    Box::new(Expr::JSXFragment(fragment))
                }
            } else {
                let fragment = JSXFragment {
                    span,
                    opening: JSXOpeningFragment { span },
                    children: non_whitespace_children,
                    closing: JSXClosingFragment { span },
                };
                Box::new(Expr::JSXFragment(fragment))
            }
        } else {
            Box::new(self.null_expr.clone())
        };
        let current_context = self.get_current_context();
        
        for (condition, children) in switch_cases.into_iter().rev() {
            let test_expr = match current_context {
                WrapperType::Return | WrapperType::Assignment => *condition,
                WrapperType::Jsx => {
                    Expr::Call(CallExpr {
                        span,
                        callee: Callee::Expr(Box::new(Expr::Ident(self.boolean_ident.clone()))),
                        args: vec![ExprOrSpread {
                            spread: None,
                            expr: condition,
                        }],
                        type_args: None,
                        ctxt: self.syntax_context,
                    })
                }
            };

            let non_whitespace_children = Self::filter_non_whitespace_children(children);

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
                        name: JSXElementName::Ident(self.condition_placeholder_ident.clone()),
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
                        name: JSXElementName::Ident(self.condition_placeholder_ident.clone()),
                    }),
                }
            }
            WrapperType::Jsx => {
                JSXElement {
                    span,
                    opening: JSXOpeningElement {
                        span,
                        name: JSXElementName::Ident(self.react_fragment_ident.clone()),
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
                        name: JSXElementName::Ident(self.react_fragment_ident.clone()),
                    }),
                }
            }
        }
    }

    fn create_parallel_switch(&self, switch_cases: Vec<(Box<Expr>, Vec<JSXElementChild>)>, else_case: Option<Vec<JSXElementChild>>, span: swc_core::common::Span) -> JSXElement {
        // 预先分配，避免 push 时多次扩容
        let mut result_children = Vec::with_capacity(switch_cases.len() + if else_case.is_some() { 1 } else { 0 });

        // 收集所有条件表达式用于 else case
        let mut all_conditions: Vec<Box<Expr>> = Vec::new();

        for (condition, children) in switch_cases {
            // 克隆条件用于后续 else case 的计算
            all_conditions.push(condition.clone());

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
                alt: Box::new(self.null_expr.clone()),
            });

            result_children.push(JSXElementChild::JSXExprContainer(JSXExprContainer {
                span,
                expr: JSXExpr::Expr(Box::new(conditional_expr)),
            }));
        }

        // 在非短路模式下，else case 只在所有条件都不满足时显示
        if let Some(else_children) = else_case {
            let fragment_expr = JSXFragment {
                span,
                opening: JSXOpeningFragment { span },
                children: else_children,
                closing: JSXClosingFragment { span },
            };

            // 创建 !condition1 && !condition2 && ... 的表达式
            let else_condition = if all_conditions.is_empty() {
                // 如果没有其他条件，else 总是显示
                Box::new(Expr::Lit(Lit::Bool(Bool { span, value: true })))
            } else {
                // 创建所有条件的否定的逻辑与
                let mut combined_condition = Box::new(Expr::Unary(UnaryExpr {
                    span,
                    op: UnaryOp::Bang,
                    arg: all_conditions[0].clone(),
                }));

                for condition in all_conditions.into_iter().skip(1) {
                    let negated_condition = Box::new(Expr::Unary(UnaryExpr {
                        span,
                        op: UnaryOp::Bang,
                        arg: condition,
                    }));

                    combined_condition = Box::new(Expr::Bin(BinExpr {
                        span,
                        left: combined_condition,
                        op: BinaryOp::LogicalAnd,
                        right: negated_condition,
                    }));
                }

                combined_condition
            };

            let else_conditional_expr = Expr::Cond(CondExpr {
                span,
                test: else_condition,
                cons: Box::new(Expr::JSXFragment(fragment_expr)),
                alt: Box::new(self.null_expr.clone()),
            });

            result_children.push(JSXElementChild::JSXExprContainer(JSXExprContainer {
                span,
                expr: JSXExpr::Expr(Box::new(else_conditional_expr)),
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
                    let non_whitespace_count = fragment.children.iter()
                        .filter(|child| TransformVisitor::is_non_whitespace_child(child))
                        .count();
                    
                    if non_whitespace_count == 1 {
                        if let Some(child) = fragment.children.iter()
                            .find(|child| TransformVisitor::is_non_whitespace_child(child)) {
                            if let JSXElementChild::JSXElement(element) = child {
                                cond_expr.cons = Box::new(Expr::JSXElement((*element).clone()));
                            }
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
                        if !element.children.is_empty() {
                            if let JSXElementChild::JSXExprContainer(container) = &element.children[0] {
                                if let JSXExpr::Expr(inner_expr) = &container.expr {
                                    return (**inner_expr).clone();
                                }
                            }
                        }
                    } else if ident.sym.as_ref() == SWITCH_PLACEHOLDER {
                        if !element.children.is_empty() {
                            if let JSXElementChild::JSXExprContainer(container) = &element.children[0] {
                                if let JSXExpr::Expr(inner_expr) = &container.expr {
                                    return self.unwrap_single_element_fragments((**inner_expr).clone());
                                }
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
                            if let Expr::Cond(mut new_cond) = inner {
                                let span = new_cond.span;
                                let cons = std::mem::take(&mut new_cond.cons);
                                new_cond.cons = Box::new(Expr::Paren(ParenExpr {
                                    span,
                                    expr: cons,
                                }));
                                Expr::Cond(new_cond)
                            } else {
                                inner
                            }
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

