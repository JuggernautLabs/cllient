---
description: Review the current project status and what's actually working
---

I need you to review the current project status and give an honest assessment of what's working.

**Perform a comprehensive audit:**

## 1. Provider Status Audit

For each service in `config/service/`, check:
- Does it have a corresponding family in `config/family/`?
- How many models reference this service?
- What's the test status (verified, untested, known-broken)?

Create a summary table:
```
Service       | Models | Tested | Untested | Status
------------- | ------ | ------ | -------- | ------
openai        | 35     | 35     | 0        | ✅ Working
anthropic     | 12     | 12     | 0        | ✅ Working
deepseek      | 22     | 22     | 0        | ✅ Working
openrouter    | 242    | 0      | 242      | ⚠️ Untested
google        | 25     | 0      | 25       | ⚠️ Unknown
```

## 2. Configuration Health Check

Check for common issues:
```bash
# Find configs with missing required fields
grep -r "service:" config/family/ | grep -v "#" | wc -l

# Check for template syntax consistency
grep -r "\${[A-Z_]*}" config/service/ | head -5
grep -r "{{[a-z_]*}}" config/service/ | head -5
```

Look for:
- Configs missing `service:` references
- Inconsistent template syntax
- Missing pricing data
- Outdated model IDs

## 3. Documentation Consistency

Compare claims across:
- `README.md` - What does it promise?
- `PLAN.md` - What was the original scope?
- `.claude/project.md` - What's the current reality?
- Model configs - What's marked as verified?

Flag any discrepancies.

## 4. Test Coverage Reality

Identify gaps:
- How many models can actually be tested right now?
- What API keys would be needed for full testing?
- What's the cost to test all claimed providers?
- Which providers are genuinely impossible to test (deprecated, invite-only, etc.)?

## 5. Technical Debt Assessment

Review the codebase for:
- TODO comments or FIXMEs
- Error handling gaps (look for `unwrap()`, `expect()`)
- Untested code paths
- Deprecated dependencies

## 6. Recommendations

Provide actionable next steps:

**Quick wins:**
- Low-effort improvements that would help immediately

**Testing priorities:**
- Which untested providers to validate first

**Code quality:**
- Critical technical debt to address

**Documentation:**
- What needs updating to reflect reality

**Be brutally honest** - this is an experimental project, and it's better to know the truth than to have false confidence.
