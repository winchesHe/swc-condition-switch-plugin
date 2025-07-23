# SWC Condition Plugin

A SWC plugin that transforms `<Condition if={...}>` JSX elements into conditional expressions, providing a cleaner syntax for conditional rendering in React applications.

## Features

- 🚀 **Fast**: Built with Rust and compiled to WebAssembly for maximum performance
- 🎯 **Context-aware**: Automatically detects the context and applies appropriate transformations
- 🔄 **Nested support**: Handles nested conditions seamlessly
- 📦 **Easy integration**: Works with rsbuild, Vite, and other SWC-based build tools

## Installation

```bash
npm install swc-condition-plugin
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
            ['swc-condition-plugin/swc_condition_plugin.wasm', {}]
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
        ['swc-condition-plugin/swc_condition_plugin.wasm', {}]
      ]
    })
  ]
});
```

## Syntax

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
# Install Rust and wasm32-wasi target
rustup target add wasm32-wasi

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
  ['swc-condition-plugin/swc_condition_plugin.wasm', {}]
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
  }
}
```

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
