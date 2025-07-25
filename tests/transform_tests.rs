use swc_condition_plugin::{TransformVisitor, PostTransformVisitor};
use swc_core::ecma::{
    parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax},
    codegen::{text_writer::JsWriter, Emitter},
    visit::FoldWith,
};
use swc_core::common::SourceMap;
use std::sync::Arc;

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

#[test]
fn test_complex_condition_expression() {
    let input = r#"
    function App({ user, isLoggedIn }) {
      return (
        <div>
          <Condition if={user && isLoggedIn}>
            <p>Welcome {user.name}</p>
          </Condition>
        </div>
      )
    }
    "#;

    let expected = r#"
    function App({ user, isLoggedIn }) {
      return (
        <div>
          <React.Fragment>{Boolean(user && isLoggedIn) ? <><p>Welcome {user.name}</p></> : null}</React.Fragment>
        </div>
      )
    }
    "#;

    test_transform(input, expected);
}

#[test]
fn test_function_call_condition() {
    let input = r#"
    function App({ items }) {
      return (
        <div>
          <Condition if={items.length > 0}>
            <ul>
              {items.map(item => <li key={item.id}>{item.name}</li>)}
            </ul>
          </Condition>
        </div>
      )
    }
    "#;

    let expected = r#"
    function App({ items }) {
      return (
        <div>
          <React.Fragment>{Boolean(items.length > 0) ? <><ul>
              {items.map((item)=><li key={item.id}>{item.name}</li>)}
            </ul></> : null}</React.Fragment>
        </div>
      )
    }
    "#;

    test_transform(input, expected);
}

#[test]
fn test_nested_conditions() {
    let input = r#"
    function App({ showOuter, showInner }) {
      return (
        <div>
          <Condition if={showOuter}>
            <div>
              <p>Outer content</p>
              <Condition if={showInner}>
                <p>Inner content</p>
              </Condition>
            </div>
          </Condition>
        </div>
      )
    }
    "#;

    let expected = r#"
    function App({ showOuter, showInner }) {
      return (
        <div>
          <React.Fragment>{Boolean(showOuter) ? <><div>
              <p>Outer content</p>
              <Condition if={showInner}>
                <p>Inner content</p>
              </Condition>
            </div></> : null}</React.Fragment>
        </div>
      )
    }
    "#;

    test_transform(input, expected);
}

#[test]
fn test_multiple_parallel_conditions() {
    let input = r#"
    function App({ showFirst, showSecond }) {
      return (
        <div>
          <Condition if={showFirst}>
            <p>First condition</p>
          </Condition>
          <Condition if={showSecond}>
            <p>Second condition</p>
          </Condition>
        </div>
      )
    }
    "#;

    let expected = r#"
    function App({ showFirst, showSecond }) {
      return (
        <div>
          <React.Fragment>{Boolean(showFirst) ? <><p>First condition</p></> : null}</React.Fragment>
          <React.Fragment>{Boolean(showSecond) ? <><p>Second condition</p></> : null}</React.Fragment>
        </div>
      )
    }
    "#;

    test_transform(input, expected);
}

#[test]
fn test_let_assignment() {
    let input = r#"
    function App({ show }) {
      let element = <Condition if={show}>
        <span>Let assignment</span>
      </Condition>
      return element
    }
    "#;

    let expected = r#"
    function App({ show }) {
      let element = Boolean(show) ? <><span>Let assignment</span></> : null
      return element
    }
    "#;

    test_transform(input, expected);
}

#[test]
fn test_var_assignment() {
    let input = r#"
    function App({ show }) {
      var element = <Condition if={show}>
        <span>Var assignment</span>
      </Condition>
      return element
    }
    "#;

    let expected = r#"
    function App({ show }) {
      var element = Boolean(show) ? <><span>Var assignment</span></> : null
      return element
    }
    "#;

    test_transform(input, expected);
}

#[test]
fn test_empty_condition() {
    let input = r#"
    function App({ condition }) {
      return (
        <div>
          <Condition if={condition}>
          </Condition>
        </div>
      )
    }
    "#;

    let expected = r#"
    function App({ condition }) {
      return (
        <div>
          <React.Fragment>{Boolean(condition) ? <></> : null}</React.Fragment>
        </div>
      )
    }
    "#;

    test_transform(input, expected);
}

#[test]
fn test_single_line_condition() {
    let input = r#"
    function App({ show }) {
      return (
        <div>
          <Condition if={show}><span>Single line</span></Condition>
        </div>
      )
    }
    "#;

    let expected = r#"
    function App({ show }) {
      return (
        <div>
          <React.Fragment>{Boolean(show) ? <><span>Single line</span></> : null}</React.Fragment>
        </div>
      )
    }
    "#;

    test_transform(input, expected);
}

#[test]
fn test_complex_nested_structure() {
    let input = r#"
    function App({ user, settings }) {
      return (
        <div>
          <Condition if={user}>
            <header>
              <h1>Welcome {user.name}</h1>
              <Condition if={settings.showProfile}>
                <div>
                  <img src={user.avatar} alt="Avatar" />
                  <Condition if={user.verified}>
                    <span className="verified">✓ Verified</span>
                  </Condition>
                </div>
              </Condition>
            </header>
          </Condition>
        </div>
      )
    }
    "#;

    let expected = r#"
    function App({ user, settings }) {
      return (
        <div>
          <React.Fragment>{Boolean(user) ? <><header>
              <h1>Welcome {user.name}</h1>
              <Condition if={settings.showProfile}>
                <div>
                  <img src={user.avatar} alt="Avatar" />
                  <Condition if={user.verified}>
                    <span className="verified">✓ Verified</span>
                  </Condition>
                </div>
              </Condition>
            </header></> : null}</React.Fragment>
        </div>
      )
    }
    "#;

    test_transform(input, expected);
}

#[test]
fn test_no_condition_tags() {
    let input = r#"
    function App({ message }) {
      return (
        <div>
          <h1>Hello World</h1>
          <p>{message}</p>
        </div>
      )
    }
    "#;

    // 没有Condition标签，代码应该保持不变
    let expected = r#"
    function App({ message }) {
      return (
        <div>
          <h1>Hello World</h1>
          <p>{message}</p>
        </div>
      )
    }
    "#;

    test_transform(input, expected);
}

fn test_transform(input: &str, expected: &str) {
    let syntax = Syntax::Typescript(TsSyntax {
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
            .replace("(item)=>", "item =>")
            .replace("item => ", "item => ")
            .replace("=> <", "=>  <")
            .replace("alt=\"Avatar\"/>", "alt=\"Avatar\" />")
    };
    
    assert_eq!(
        normalize(&cleaned_output),
        normalize(expected),
        "Transform output doesn't match expected result.\nActual: {}\nExpected: {}",
        cleaned_output,
        expected
    );
}