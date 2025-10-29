---
description: Test a specific LLM provider integration thoroughly
---

I need you to thoroughly test a specific LLM provider integration for this project.

**Steps to follow:**

1. **Ask me which provider to test** - Get the provider/model name (e.g., "gpt-4o-mini", "claude-3-haiku", or one of the OpenRouter models)

2. **Check the configuration files:**
   - Look at the model config in `config/family/*/`
   - Look at the service config it references in `config/service/`
   - Verify the config syntax is correct

3. **Test basic completion:**
   ```bash
   cargo run --bin cllient -- ask <model-id> "Hello, please respond with a simple greeting"
   ```

4. **Test streaming:**
   ```bash
   cargo run --bin cllient -- stream <model-id> "Count from 1 to 5 slowly"
   ```

5. **Test error handling:**
   - Try with an invalid API key
   - Try with a malformed prompt
   - Check what error messages users see

6. **Document the results:**
   - Create a test report showing what worked and what failed
   - Note any error messages or issues
   - Suggest fixes if things broke

7. **Update the project status:**
   - If it works, add it to the "tested" list in README
   - If it fails, document the failure mode
   - Update `.claude/project.md` with findings

Be thorough but realistic - this is experimental code, so failures are expected and valuable to document.
