# Phase 4 Quick Reference

## Files Created

1. **`/workspace/tests/v1_v2_equivalence_test.rs`** (262 lines, 8.6K)
   - Comprehensive v1-v2 equivalence test suite

2. **`/workspace/run_v1_v2_equivalence_tests.sh`** (executable)
   - Helper script to run tests with prerequisites documented

3. **`/workspace/PHASE_4_EQUIVALENCE_TESTING_SUMMARY.md`** (7.2K)
   - Detailed documentation of implementation

## Test Coverage

### 10 Test Functions

1. `test_anthropic_text_message_equivalence` - Simple text for Anthropic
2. `test_anthropic_multimodal_equivalence` - Text + binary image for Anthropic
3. `test_anthropic_url_image_equivalence` - URL-based images for Anthropic
4. `test_openai_text_message_equivalence` - Simple text for OpenAI
5. `test_openai_image_url_equivalence` - Multimodal with images for OpenAI
6. `test_special_characters_equivalence` - Quotes, newlines, tabs (both providers)
7. `test_multiple_messages_equivalence` - Conversations (both providers)
8. `test_empty_messages_equivalence` - Edge case: empty arrays (both providers)
9. `test_mixed_content_blocks_equivalence` - Complex: text+binary+text+URL (both providers)
10. `test_unicode_content_equivalence` - UTF-8 in Chinese, Russian, Arabic (both providers)

## How to Run

### Prerequisites
```bash
# Ubuntu/Debian
apt-get install pkg-config libssl-dev

# Fedora/RHEL
yum install pkg-config openssl-devel

# macOS
brew install pkg-config openssl
```

### Execute Tests
```bash
# Using helper script
./run_v1_v2_equivalence_tests.sh

# Or directly
cargo test --test v1_v2_equivalence_test

# With verbose output
cargo test --test v1_v2_equivalence_test -- --nocapture
```

## Success Criteria

- ✅ All 10 tests implemented
- ✅ Helper function provides detailed diff output
- ✅ Tests cover both Anthropic and OpenAI formats
- ✅ Tests cover text, multimodal, binary, URL, mixed content
- ✅ Edge cases included (empty arrays, special chars, unicode)

## Next Steps

### If Tests PASS ✅
→ Proceed to Phase 5: Systematic replacement of v1 with v2

### If Tests FAIL ❌
→ **STOP**: Fix MessageFormatter templates to match v1 output
→ Add regression tests for each failure
→ Re-run until all pass

## Critical Gate

These tests are the **CRITICAL GATE** for migration:
- Must pass 100% before proceeding to Phase 5
- Even one failure indicates potential production bugs
- JSON output must match byte-for-byte (whitespace aside)
