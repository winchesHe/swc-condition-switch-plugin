# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-01-23

### Added
- Initial release of SWC Condition Plugin
- Transform `<Condition if={...}>` JSX elements into conditional expressions
- Context-aware transformations:
  - JSX context: `{Boolean(condition) ? <>content</> : null}`
  - Return context: `condition ? <>content</> : null`
  - Assignment context: `Boolean(condition) ? <>content</> : null`
- Support for nested conditions
- Support for complex condition expressions
- rsbuild integration example
- Vite integration example
- Comprehensive test suite
- TypeScript declaration files
- Documentation and examples

### Features
- Fast Rust-based implementation compiled to WebAssembly
- Zero runtime dependencies
- Compatible with all SWC-based build tools
- Handles edge cases like empty conditions and single-line syntax
- Recursive processing of nested conditions
