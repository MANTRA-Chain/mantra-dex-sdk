# AST-grep Rules for MANTRA SDK

Semantic code analysis rules for the MANTRA blockchain SDK.

## Quick Start

```bash
# Scan entire codebase
ast-grep scan

# Scan specific category
ast-grep scan -c rules/security.yml
ast-grep scan -c rules/error-handling.yml

# Find protocol implementations
ast-grep run -p 'impl Protocol for $TYPE { $$$ }' --lang rust

# Find all TODO comments
ast-grep scan -c rules/code-quality.yml | grep "TODO"
```

## Rule Categories

### 🏗️ Protocol Patterns (`protocol-patterns.yml`)
- Protocol trait implementations
- Protocol registry access patterns
- Protocol registration verification
- Error conversion compliance

### ⚠️ Error Handling (`error-handling.yml`)
- Unwrap/expect usage in production
- Panic detection
- Silent error ignoring
- Result type handling
- Proper error type usage

### 🔒 Security (`security.yml`)
- Unsafe code blocks
- Credential handling
- Wallet encryption
- Sensitive data logging
- SQL injection risks
- Slippage protection in swaps

### ✨ Code Quality (`code-quality.yml`)
- TODO/FIXME tracking
- Debug macro removal
- Unnecessary clones
- Documentation coverage
- String concatenation efficiency

### 🧪 Testing (`testing.yml`)
- Test naming conventions
- Assertion presence
- TUI test prevention
- Integration test placement
- Test cleanup verification

### 🔌 MCP Conventions (`mcp-conventions.yml`)
- Tool naming prefixes (network_*, wallet_*, dex_*, etc.)
- Feature gate compliance
- JSON-RPC error handling
- Tool documentation
- Parameter validation

### ⚙️ Configuration (`configuration.yml`)
- Config file locations
- Environment variable overrides
- Address validation
- Endpoint completeness
- Default implementations

## Usage Examples

### Find all unsafe code
```bash
ast-grep scan -c rules/security.yml | grep "unsafe"
```

### Check error handling
```bash
ast-grep scan -c rules/error-handling.yml --json
```

### Verify MCP tool naming
```bash
ast-grep scan -c rules/mcp-conventions.yml
```

### Find all TODOs
```bash
ast-grep run -p '// TODO: $$$MSG' --lang rust
```

### Check protocol implementations
```bash
ast-grep run -p 'impl Protocol for $TYPE { $$$ }' --lang rust -l 0
```

## Rule Severity Levels

- **error**: Must be fixed before merge
- **warning**: Should be reviewed and addressed
- **info**: Informational, for awareness

## CI Integration

Add to `.github/workflows/lint.yml`:
```yaml
- name: Run AST-grep
  run: |
    cargo install ast-grep
    ast-grep scan --error-on-warning
```

## Custom Searches

### Find all wallet operations
```bash
ast-grep run -p '$CLIENT.wallet.$METHOD($$$)' --lang rust
```

### Find swap operations
```bash
ast-grep run -p 'fn $NAME($$$) { $$$ }' --lang rust | grep swap
```

### Find contract calls
```bash
ast-grep run -p '$VAR.call_contract($$$)' --lang rust
```

## Writing New Rules

See [ast-grep documentation](https://ast-grep.github.io/) for pattern syntax.

Example rule structure:
```yaml
rules:
  - id: my-rule
    message: "Description of the issue"
    severity: warning  # error, warning, or info
    language: rust
    pattern: |
      code pattern to match
    note: "Guidance on how to fix"
```

## Performance Tips

1. Use specific rule files instead of scanning all rules
2. Use `--json` for programmatic processing
3. Combine with `jq` for filtering: `ast-grep scan --json | jq '.[] | select(.severity=="error")'`
4. Use `--threads` for parallel processing on large codebases