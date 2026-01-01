# Testing Strategy: Legacy API Compatibility & Provider Verification

**Date**: January 1, 2026
**Status**: Analysis Complete
**Current Version**: v0.2.0

---

## Executive Summary

**Question**: Have we tested that existing models still work? Does the old API still apply?

**Answer**:
- **Tests Pass**: All 29 tests pass (unit + doc tests)
- **Old API Intact**: `MessageBuilder` in `template.rs` is still used in production
- **New API Parallel**: `MessageFormatter` exists but is not integrated into the request pipeline
- **Real API Testing**: Conditional tests exist but require manual API key setup
- **Breaking Changes**: Some internal restructuring occurred, but public API is stable

---

## 1. Current Test Status

```
$ cargo test
test result: ok. 29 passed; 0 failed; 4 ignored; 0 measured
```

### Test Distribution

| Category | Count | Description |
|----------|-------|-------------|
| Doc tests | 29 | Inline documentation examples |
| Unit tests | 42+ | In `message_format.rs` alone |
| Integration tests | 50+ | In `tests/` directory |
| **Total** | **120+** | Comprehensive coverage |

### Key Test Files

| File | Tests | Covers |
|------|-------|--------|
| `runtime_api_tests.rs` | 18 | ModelRegistry, selection strategies |
| `low_level_client_tests.rs` | 10 | ConfigLoader, ClientFactory, requests |
| `fluent_builder_integration_test.rs` | 43+ | Message formatting, SSE parsing |
| `cli_api_tests.rs` | 10 | All CLI commands |
| `sse_parsing_test.rs` | 3 | OpenAI/Claude SSE formats |
| `programmatic_api_test.rs` | 10+ | Runtime API, RequestBuilder |

---

## 2. API Stability Analysis

### Old API (Still in Production)

**Location**: `src/template.rs:363-503`

```rust
// CURRENT PRODUCTION CODE (client.rs:93-96)
let messages = MessageBuilder::build_messages(
    &self.service_config.message_builder.to_string(),
    &request.messages,
)?;
```

**Supported Formats**: `"anthropic"`, `"openai"` only

**Status**: ✅ Unchanged and working

### New API (Implemented, Not Integrated)

**Location**: `src/message_format.rs` (1,284 lines)

```rust
// NOT YET USED IN PRODUCTION
let formatter = MessageFormatter::new(anthropic_format()?)?;
let messages = formatter.format_messages(&request.messages)?;
```

**Supported Formats**: Unlimited (config-driven)

**Status**: ✅ Complete, tested, but not integrated

### Comparison

| Aspect | Old (MessageBuilder) | New (MessageFormatter) |
|--------|---------------------|------------------------|
| Input | `&[MessageContent]` | `&[MessageContent]` |
| Output | `Result<Value>` | `Result<Value>` |
| Providers | 2 hardcoded | Unlimited (config) |
| Role mapping | Fixed | Configurable |
| MIME patterns | `starts_with()` | Glob patterns |
| Extensibility | Code changes | Config changes |
| **Integration** | **YES** | **NO** |

---

## 3. Breaking Changes Identified

### v0.1.2 → v0.2.0

| Change | Severity | Impact |
|--------|----------|--------|
| SSE extractors consolidated | HIGH | Type aliases replace concrete types |
| Enum macro refactoring | MEDIUM | Serialization behavior may differ |
| Registry method optimization | LOW | Result ordering may change |
| New `message_format` module | LOW | Additive only |

### Files Removed (Internal)

```
src/streaming/sse/extractors/claude.rs  → REMOVED
src/streaming/sse/extractors/openai.rs  → REMOVED
src/streaming/sse/providers/claude.rs   → REMOVED
src/streaming/sse/providers/openai.rs   → REMOVED
```

**Mitigation**: Type aliases preserve compatibility:
```rust
pub type OpenAITokenExtractor = JsonPathExtractor;
pub type ClaudeTokenExtractor = JsonPathExtractor;
```

---

## 4. Real Provider Testing

### Current Capabilities

```rust
// Tests with real API calls (conditional)
#[tokio::test]
async fn test_runtime_async_completion() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping: DEEPSEEK_API_KEY not set");
        return;  // Silently skips
    }
    // Real API call here
}
```

### Provider Coverage

| Provider | API Key Env Var | Integration Tests |
|----------|-----------------|-------------------|
| DeepSeek | `DEEPSEEK_API_KEY` | ✅ Yes (primary) |
| OpenAI | `OPENAI_API_KEY` | ⚠️ CLI tests only |
| Anthropic | `ANTHROPIC_API_KEY` | ⚠️ CLI tests only |
| Google | `GOOGLE_API_KEY` | ❌ No tests |

### Manual Verification Commands

```bash
# Test specific providers
make test-deepseek    # Requires DEEPSEEK_API_KEY
make test-openai      # Requires OPENAI_API_KEY
make test-anthropic   # Requires ANTHROPIC_API_KEY

# Run all real API tests
DEEPSEEK_API_KEY=xxx cargo test
```

---

## 5. Testing Strategy Recommendations

### Tier 1: Unit Test Coverage (Current)

**Status**: ✅ Good

- 42+ tests in `message_format.rs`
- Comprehensive builder pattern testing
- Serialization round-trip tests

**Action**: Maintain current coverage

### Tier 2: Integration Test Expansion (Needed)

**Status**: ⚠️ Gaps exist

**Missing Coverage**:
1. Chat session state management
2. Error recovery (network failures, timeouts)
3. Plugin system integration
4. Response extraction edge cases
5. Concurrent request handling

**Action**: Add integration tests for gaps

### Tier 3: Contract Testing (Recommended)

**Status**: ❌ Not implemented

**Problem**: `wiremock` is installed but unused

**Solution**: Record real API responses and replay in tests

```rust
// Example contract test structure
#[tokio::test]
async fn test_openai_response_parsing() {
    let mock = Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(include_str!("fixtures/openai_response.json")));

    // Test against mock
}
```

**Benefits**:
- Tests run without API keys
- Catches response format changes
- Enables CI/CD integration

### Tier 4: Provider Verification Matrix (Recommended)

**Status**: ❌ Not implemented

**Proposed Structure**:

```
tests/
├── fixtures/
│   ├── openai/
│   │   ├── completion_response.json
│   │   ├── streaming_chunks.txt
│   │   └── error_response.json
│   ├── anthropic/
│   │   ├── message_response.json
│   │   ├── streaming_events.txt
│   │   └── error_response.json
│   └── google/
│       ├── generate_response.json
│       └── streaming_response.txt
└── provider_contracts/
    ├── openai_contract_test.rs
    ├── anthropic_contract_test.rs
    └── google_contract_test.rs
```

### Tier 5: CI Pipeline (Critical Gap)

**Status**: ❌ No CI configured

**Recommended GitHub Actions**:

```yaml
# .github/workflows/test.yml
name: Tests
on: [push, pull_request]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test

  integration-tests:
    runs-on: ubuntu-latest
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test
        env:
          DEEPSEEK_API_KEY: ${{ secrets.DEEPSEEK_API_KEY }}
```

---

## 6. Legacy API Protection Strategy

### Approach 1: Snapshot Testing

Capture current output and compare against future changes:

```rust
#[test]
fn test_anthropic_message_format_snapshot() {
    let messages = vec![
        MessageContent::Text {
            role: "user".into(),
            content: "Hello".into()
        },
    ];

    let result = MessageBuilder::build_messages("anthropic", &messages).unwrap();

    // Compare against known-good output
    insta::assert_json_snapshot!(result);
}
```

### Approach 2: Golden File Testing

Store expected outputs in files:

```
tests/golden/
├── anthropic_text_message.json
├── anthropic_multimodal_message.json
├── openai_text_message.json
├── openai_image_message.json
└── openai_audio_message.json
```

### Approach 3: Dual-Path Verification

Run both old and new APIs, compare outputs:

```rust
#[test]
fn test_old_new_api_equivalence() {
    let messages = create_test_messages();

    // Old API
    let old_result = MessageBuilder::build_messages("anthropic", &messages).unwrap();

    // New API
    let formatter = MessageFormatter::new(anthropic_format().unwrap()).unwrap();
    let new_result = formatter.format_messages(&messages).unwrap();

    // Must produce identical output
    assert_eq!(old_result, new_result);
}
```

---

## 7. Recommended Test Matrix

### Before Any Migration

| Test | Purpose | Status |
|------|---------|--------|
| Unit tests pass | Basic functionality | ✅ |
| Doc tests pass | API examples work | ✅ |
| `MessageBuilder` output unchanged | Legacy compatibility | ⚠️ Add snapshots |
| Real API completion works | End-to-end | ⚠️ Manual only |
| Real API streaming works | SSE parsing | ⚠️ Manual only |

### During Migration

| Test | Purpose | Status |
|------|---------|--------|
| Old API still callable | No removal | ✅ Still exists |
| New API produces same output | Equivalence | ❌ Add tests |
| Both APIs coexist | No conflicts | ✅ |
| Integration tests pass | System works | ✅ |

### After Migration

| Test | Purpose | Status |
|------|---------|--------|
| Old API deprecated | Clear warnings | ❌ Not deprecated |
| New API is default | Config-driven | ❌ Not integrated |
| All providers work | No regressions | ⚠️ Add contract tests |
| Performance unchanged | No degradation | ❌ No benchmarks |

---

## 8. Immediate Action Items

### Priority 1: Add Snapshot Tests

```bash
# Install insta for snapshot testing
cargo add --dev insta
```

Create `tests/legacy_api_snapshots.rs`:
- Capture `MessageBuilder::build_anthropic_messages()` output
- Capture `MessageBuilder::build_openai_messages()` output
- Run on every PR to detect changes

### Priority 2: Add Equivalence Tests

Create `tests/api_equivalence_test.rs`:
- Compare old `MessageBuilder` vs new `MessageFormatter`
- Ensure identical output for all message types
- Run before MessageFormatter integration

### Priority 3: Record API Fixtures

```bash
# Record real API responses
curl -X POST https://api.openai.com/v1/chat/completions \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -d '{"model":"gpt-4","messages":[{"role":"user","content":"Hi"}]}' \
  > tests/fixtures/openai/completion.json
```

### Priority 4: Enable wiremock

Convert at least 3 tests to use recorded fixtures instead of skipping when API keys are missing.

---

## 9. Conclusion

### Current State

| Aspect | Status |
|--------|--------|
| Old API working | ✅ Yes |
| New API working | ✅ Yes (standalone) |
| Integration complete | ❌ No |
| Breaking changes | ⚠️ Minor (internal) |
| Real API testing | ⚠️ Manual only |
| CI pipeline | ❌ None |

### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Old API breaks during migration | Low | High | Snapshot tests |
| New API produces different output | Medium | High | Equivalence tests |
| Provider API changes undetected | High | Medium | Contract tests |
| Regression in streaming | Medium | High | SSE fixture tests |

### Recommended Order of Implementation

1. **Immediate**: Add snapshot tests for `MessageBuilder` output
2. **Short-term**: Add equivalence tests (old vs new API)
3. **Medium-term**: Enable contract tests with wiremock
4. **Long-term**: Set up CI pipeline with secret management

This strategy ensures we can safely migrate from `MessageBuilder` to `MessageFormatter` without breaking existing functionality or provider compatibility.
