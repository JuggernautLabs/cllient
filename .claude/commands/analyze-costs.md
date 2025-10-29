---
description: Analyze model pricing and find cost-optimal options
---

I need you to analyze model pricing and help find cost-optimal options.

**Steps to perform:**

## 1. Check Current Pricing Data

```bash
# See what pricing data we have
make pricing-cheapest

# Or explore the JSON directly
cat openrouter_data.json | jq '.[0:5]'
```

## 2. Analyze the Data

Read through the pricing configs in:
- `config/family/*/*.yaml` - Look at the `pricing:` section
- `openrouter_data.json` - Bulk pricing from OpenRouter

Calculate for the user:
- **Cheapest models overall** (input + output costs)
- **Best value by capability** (e.g., cheapest with vision, cheapest with 100k+ context)
- **Cost per provider** (aggregate by family/provider)

## 3. Compare Tested vs. Untested

Break down costs by validation status:
- How many cheap models are actually tested?
- What's the cheapest *verified* model?
- What's the price difference between tested and untested options?

## 4. Create Recommendations

Provide cost recommendations like:

**For development/testing:**
- Cheapest tested model: `<model>` at $X per 1M tokens
- Best tested value: `<model>` with good performance/cost ratio

**For production (if testing):**
- Consider these tested models: `<list>`
- Estimated cost for 1M input + 100k output tokens: $X

**High-risk cheap options (untested):**
- These *might* work but haven't been validated: `<list>`

## 5. Check for Pricing Updates

Suggest:
```bash
# Update pricing data from OpenRouter
make models-openrouter

# Or manually check provider APIs
make models-openai
make models-anthropic
```

## 6. Document Findings

Create a cost analysis report if the user wants it, or just provide a summary.

**Be honest:** Note which pricing data is accurate (from tested providers) vs. scraped/uncertain (from OpenRouter auto-generation).
