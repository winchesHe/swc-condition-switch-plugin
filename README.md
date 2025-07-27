# SWC Condition Switch Plugin

A SWC plugin that transforms `<Condition if={...}>` and `<Switch>` JSX elements into optimized conditional expressions, providing a cleaner syntax for conditional rendering in React applications.

## Features

- 🚀 **Fast**: Built with Rust and compiled to WebAssembly for maximum performance
- 🎯 **Context-aware**: Automatically detects the context and applies appropriate transformations
- 🔄 **Nested support**: Handles nested conditions seamlessly
- 🔀 **Switch expressions**: Advanced multi-case conditional rendering with short-circuit evaluation
- 📦 **Easy integration**: Works with rsbuild, Vite, and other SWC-based build tools

## Installation

```bash
npm install swc-condition-switch-plugin
```

## Usage

### With rsbuild

Add the plugin to your `rsbuild.config.ts`:

```typescript
import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';

export default defineConfig({
  plugins: [pluginReact()],
  tools: {
    swc: {
      jsc: {
        experimental: {
          plugins: [
            ['swc-condition-switch-plugin/swc_condition_plugin.wasm', {}]
          ]
        }
      }
    }
  }
});
```

### With Vite

Add the plugin to your `vite.config.ts`:

```typescript
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react-swc';

export default defineConfig({
  plugins: [
    react({
      plugins: [
        ['swc-condition-switch-plugin/swc_condition_plugin.wasm', {}]
      ]
    })
  ]
});
```

## Syntax

### Condition Component

Use the `<Condition if={...}>` component to conditionally render JSX:

```tsx
function App({ showMessage, user }) {
  return (
    <div>
      <Condition if={showMessage}>
        <p>Hello World!</p>
      </Condition>
      
      <Condition if={user && user.isLoggedIn}>
        <p>Welcome back, {user.name}!</p>
      </Condition>
    </div>
  );
}
```

### Switch Component

Use the `<Switch>` component for multiple conditional cases:

```tsx
function StatusIndicator({ status, priority }) {
  return (
    <Switch shortCircuit>
      <Switch.Case if={status === 'loading'}>
        <Spinner />
      </Switch.Case>
      <Switch.Case if={status === 'error'}>
        <ErrorMessage />
      </Switch.Case>
      <Switch.Case if={status === 'success'}>
        <SuccessIcon />
      </Switch.Case>
    </Switch>
  );
}
```

## Transformations

The plugin applies different transformations based on the context:

### JSX Context (inside JSX elements)

**Input:**
```tsx
<div>
  <Condition if={showMessage}>
    <p>Hello World</p>
  </Condition>
</div>
```

**Output:**
```tsx
<div>
  {Boolean(showMessage) ? <>
    <p>Hello World</p>
  </> : null}
</div>
```

### Return Context (direct return statements)

**Input:**
```tsx
function Component({ condition }) {
  return <Condition if={condition}>
    <div>Content</div>
  </Condition>;
}
```

**Output:**
```tsx
function Component({ condition }) {
  return condition ? <>
    <div>Content</div>
  </> : null;
}
```

### Assignment Context (variable assignments)

**Input:**
```tsx
const element = <Condition if={condition}>
  <span>Content</span>
</Condition>;
```

**Output:**
```tsx
const element = Boolean(condition) ? <>
  <span>Content</span>
</> : null;
```

## Switch Transformations

The `<Switch>` component supports two modes: **parallel evaluation** (default) and **short-circuit evaluation**.

### Parallel Evaluation (Default)

All conditions are evaluated and rendered independently:

**Input:**
```tsx
<Switch>
  <Switch.Case if={condition1}>
    <p>Case 1</p>
  </Switch.Case>
  <Switch.Case if={condition2}>
    <p>Case 2</p>
  </Switch.Case>
</Switch>
```

**Output:**
```tsx
<React.Fragment>
  {condition1 ? <><p>Case 1</p></> : null}
  {condition2 ? <><p>Case 2</p></> : null}
</React.Fragment>
```

### Short-Circuit Evaluation

Only the first truthy condition is rendered (add `shortCircuit` attribute):

**Input:**
```tsx
<Switch shortCircuit>
  <Switch.Case if={condition1}>
    <p>Case 1</p>
  </Switch.Case>
  <Switch.Case if={condition2}>
    <p>Case 2</p>
  </Switch.Case>
</Switch>
```

**Output:**
```tsx
condition1 ? <p>Case 1</p> : condition2 ? <p>Case 2</p> : null
```

### When to Use Switch vs Condition

**Use `<Switch>` when:**
- You have multiple mutually exclusive conditions (especially with `shortCircuit`)
- You want to implement if-else-if logic patterns
- You need to handle multiple states (loading, error, success, etc.)
- You want cleaner code for complex conditional rendering

**Use `<Condition>` when:**
- You have simple single conditions
- You need independent conditional rendering (multiple conditions can be true)
- You're doing basic show/hide logic

## Advanced Examples

### Nested Conditions

```tsx
<Condition if={user}>
  <div>
    <h1>Welcome {user.name}</h1>
    <Condition if={user.verified}>
      <span className="verified">✓ Verified</span>
    </Condition>
  </div>
</Condition>
```

### Complex Switch Logic

```tsx
function UserDashboard({ user, items, loading }) {
  return (
    <div>
      <Switch shortCircuit>
        <Switch.Case if={loading}>
          <div className="spinner">Loading...</div>
        </Switch.Case>
        <Switch.Case if={!user}>
          <LoginForm />
        </Switch.Case>
        <Switch.Case if={items?.length > 0}>
          <ItemList items={items} />
        </Switch.Case>
        <Switch.Case if={items?.length === 0}>
          <EmptyState message="No items found" />
        </Switch.Case>
      </Switch>
    </div>
  );
}
```

### Mixed Condition and Switch

```tsx
function AppLayout({ user, notifications, theme }) {
  return (
    <div className={theme}>
      <header>
        <Condition if={user}>
          <UserMenu user={user} />
        </Condition>
      </header>
      
      <main>
        <Switch>
          <Switch.Case if={notifications?.length > 0}>
            <NotificationBanner notifications={notifications} />
          </Switch.Case>
          <Switch.Case if={user?.isFirstTime}>
            <WelcomeTour />
          </Switch.Case>
        </Switch>
        
        <Content />
      </main>
    </div>
  );
}
```

### Complex Conditions

```tsx
<Condition if={items && items.length > 0}>
  <ul>
    {items.map(item => (
      <li key={item.id}>{item.name}</li>
    ))}
  </ul>
</Condition>
```

## Development

### Building the Plugin

```bash
# Install Rust and wasm32-wasip1 target
rustup target add wasm32-wasip1

# Build the plugin
./build.sh
```

### Running Tests

```bash
cargo test
```

### Testing with Example Project

```bash
cd examples
npm install
npm run dev
```

## Configuration

The plugin currently doesn't require any configuration options. Simply add it to your SWC plugins array:

```typescript
plugins: [
  ['swc-condition-switch-plugin/swc_condition_plugin.wasm', {}]
]
```

## TypeScript Support

For TypeScript projects, you may want to add a declaration file to avoid type errors:

```typescript
// types/condition.d.ts
declare namespace JSX {
  interface IntrinsicElements {
    Condition: {
      if: any;
      children?: React.ReactNode;
    };
    Switch: {
      shortCircuit?: boolean;
      children?: React.ReactNode;
    };
  }
}

declare namespace Switch {
  interface Case {
    if: any;
    children?: React.ReactNode;
  }
}
```

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
