# Advanced Scripting Examples

Production-ready scripts and automation patterns using cllient.

> **CLI Reference**: [CLI Usage Guide](../cli-usage.md) | **Scripting Tips**: [Development Guide](../development.md)

## Shell Integration

### Universal AI Function

```bash
#!/bin/bash
# ~/.bashrc or ~/.zshrc

ai() {
    local model="${AI_MODEL:-deepseek-chat}"
    local prompt="$*"
    
    if [ -z "$prompt" ]; then
        echo "Usage: ai <prompt>"
        echo "Current model: $model"
        echo "Set AI_MODEL environment variable to change default"
        return 1
    fi
    
    cllient ask "$model" "$prompt"
}

# Usage:
# ai "explain docker containers"
# AI_MODEL=gpt-4o-mini ai "what is rust ownership?"
```

### Smart Model Selection

```bash
#!/bin/bash
# smart-ai.sh - Automatically select best model for task

smart_ai() {
    local prompt="$1"
    local model=""
    
    # Code-related queries
    if echo "$prompt" | grep -iE "(code|function|algorithm|debug|programming)" > /dev/null; then
        model="deepseek-coder"
        echo "🔧 Using DeepSeek Coder for code-related query"
    
    # Creative writing
    elif echo "$prompt" | grep -iE "(write|story|creative|poem|article)" > /dev/null; then
        model="claude-3-opus-20240229"
        echo "✍️  Using Claude Opus for creative writing"
    
    # Analysis and reasoning
    elif echo "$prompt" | grep -iE "(analyze|compare|evaluate|reason)" > /dev/null; then
        model="gpt-4o-mini"
        echo "🧠 Using GPT-4o Mini for analysis"
    
    # Default to fastest/cheapest
    else
        model="deepseek-chat"
        echo "⚡ Using DeepSeek Chat (default)"
    fi
    
    cllient ask "$model" "$prompt"
}

# Examples:
# smart_ai "write a function to sort arrays"      # → deepseek-coder
# smart_ai "write a short story about AI"        # → claude-opus  
# smart_ai "analyze the pros and cons of React"  # → gpt-4o-mini
# smart_ai "what is machine learning?"           # → deepseek-chat
```

---

## Batch Processing

### Process Multiple Files

```bash
#!/bin/bash
# code-summarizer.sh - Summarize all code files in a directory

DIRECTORY="${1:-.}"
OUTPUT_FILE="code-summary.md"

echo "# Code Summary for $DIRECTORY" > "$OUTPUT_FILE"
echo "Generated on $(date)" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

find "$DIRECTORY" -name "*.rs" -o -name "*.py" -o -name "*.js" -o -name "*.ts" | while read -r file; do
    echo "Processing: $file"
    
    echo "## $(basename "$file")" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    
    # Get file summary
    summary=$(cllient ask claude-3-haiku-20240307 "Summarize this code file in 2-3 sentences: $(cat "$file")")
    echo "$summary" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
done

echo "✅ Summary written to $OUTPUT_FILE"
```

### Multi-Model Comparison

```bash
#!/bin/bash
# compare-models.sh - Compare multiple models on the same prompt

PROMPT="$1"
if [ -z "$PROMPT" ]; then
    echo "Usage: $0 '<prompt>'"
    exit 1
fi

MODELS=(
    "gpt-4o-mini"
    "claude-3-haiku-20240307"
    "deepseek-chat"
)

echo "🔍 Comparing models on: $PROMPT"
echo "========================================="

for model in "${MODELS[@]}"; do
    echo ""
    echo "📱 $model:"
    echo "$(printf '─%.0s' {1..50})"
    
    # Get response and measure time
    start_time=$(date +%s.%N)
    response=$(cllient ask "$model" "$PROMPT" 2>&1)
    end_time=$(date +%s.%N)
    duration=$(echo "$end_time - $start_time" | bc)
    
    echo "$response"
    echo ""
    echo "⏱️  Response time: ${duration}s"
    echo ""
done
```

### Parallel Processing

```bash
#!/bin/bash
# parallel-ai.sh - Process multiple prompts in parallel

declare -a prompts=(
    "Explain machine learning"
    "What is quantum computing?"
    "How does blockchain work?"
    "Describe artificial intelligence"
    "What is cloud computing?"
)

declare -a models=(
    "gpt-4o-mini"
    "claude-3-haiku-20240307"  
    "deepseek-chat"
    "gpt-4o-mini"
    "claude-3-haiku-20240307"
)

# Function to process single prompt
process_prompt() {
    local index=$1
    local prompt="${prompts[$index]}"
    local model="${models[$index]}"
    local output_file="response_$index.txt"
    
    echo "🚀 Processing prompt $((index + 1)): '$prompt' with $model"
    cllient ask "$model" "$prompt" > "$output_file" 2>&1
    echo "✅ Completed prompt $((index + 1))"
}

# Export function for parallel execution
export -f process_prompt
export prompts
export models

# Run in parallel (adjust -P for number of parallel jobs)
seq 0 $((${#prompts[@]} - 1)) | xargs -P 3 -I {} bash -c 'process_prompt {}'

echo "🎉 All prompts processed. Check response_*.txt files"
```

---

## Data Processing Pipelines

### Log Analysis

```bash
#!/bin/bash
# log-analyzer.sh - Analyze application logs with AI

LOG_FILE="$1"
if [ ! -f "$LOG_FILE" ]; then
    echo "Usage: $0 <log-file>"
    exit 1
fi

echo "🔍 Analyzing log file: $LOG_FILE"

# Extract errors
echo "📊 Error Analysis:"
echo "=================="
errors=$(grep -i "error\|exception\|failed" "$LOG_FILE" | head -20)
if [ -n "$errors" ]; then
    cllient ask deepseek-chat "Analyze these application errors and suggest fixes: $errors"
else
    echo "No errors found in logs"
fi

echo ""
echo "📈 Pattern Analysis:"
echo "==================="
patterns=$(awk '{print $1, $4}' "$LOG_FILE" | sort | uniq -c | sort -nr | head -10)
cllient ask gpt-4o-mini "Analyze these log patterns and identify potential issues: $patterns"

echo ""
echo "💡 Recommendations:"
echo "==================="
summary=$(tail -100 "$LOG_FILE")
cllient ask claude-3-haiku-20240307 "Based on these recent log entries, provide monitoring and optimization recommendations: $summary"
```

### CSV Data Analysis

```bash
#!/bin/bash
# csv-analyzer.sh - Analyze CSV data with AI

CSV_FILE="$1"
if [ ! -f "$CSV_FILE" ]; then
    echo "Usage: $0 <csv-file>"
    exit 1
fi

echo "📊 Analyzing CSV: $CSV_FILE"

# Get basic stats
echo "📈 Basic Statistics:"
echo "==================="
wc -l "$CSV_FILE"
head -1 "$CSV_FILE" | tr ',' '\n' | nl

# Sample data for analysis
echo ""
echo "🔍 Data Sample Analysis:"
echo "======================="
sample=$(head -20 "$CSV_FILE")
cllient ask gpt-4o-mini "Analyze this CSV data sample and describe patterns, potential issues, and insights: $sample"

# Suggest analysis
echo ""
echo "💡 Analysis Suggestions:"
echo "======================="
header=$(head -1 "$CSV_FILE")
cllient ask claude-3-haiku-20240307 "Based on these CSV columns, suggest useful data analysis and visualization approaches: $header"
```

---

## API Integration

### GitHub Integration

```bash
#!/bin/bash
# gh-ai-review.sh - AI-powered GitHub PR review

PR_NUMBER="$1"
REPO="$2"

if [ -z "$PR_NUMBER" ] || [ -z "$REPO" ]; then
    echo "Usage: $0 <pr-number> <owner/repo>"
    exit 1
fi

echo "🔍 Reviewing PR #$PR_NUMBER in $REPO"

# Get PR diff
diff_content=$(gh pr diff "$PR_NUMBER" --repo "$REPO")

if [ -z "$diff_content" ]; then
    echo "Error: Could not fetch PR diff"
    exit 1
fi

echo ""
echo "🐛 Code Review:"
echo "=============="
cllient ask deepseek-coder "Review this code change for bugs, security issues, and best practices: $diff_content"

echo ""
echo "📚 Documentation:"
echo "================"
cllient ask claude-3-haiku-20240307 "Suggest documentation improvements for this code change: $diff_content"

echo ""
echo "⚡ Performance:"
echo "=============="
cllient ask gpt-4o-mini "Analyze this code change for performance implications: $diff_content"
```

### Slack Integration

```bash
#!/bin/bash
# slack-ai.sh - Post AI responses to Slack

CHANNEL="$1"
PROMPT="$2"

if [ -z "$CHANNEL" ] || [ -z "$PROMPT" ]; then
    echo "Usage: $0 <#channel> '<prompt>'"
    exit 1
fi

echo "🤖 Getting AI response for Slack..."

# Get response
response=$(cllient ask deepseek-chat "$PROMPT")

# Format for Slack
slack_message="🤖 *AI Assistant*

*Question:* $PROMPT

*Response:*
\`\`\`
$response
\`\`\`

_Generated with cllient_"

# Post to Slack (requires slack CLI)
echo "$slack_message" | slack chat send --channel "$CHANNEL" --stdin

echo "✅ Posted to $CHANNEL"
```

---

## Monitoring and Alerting

### AI-Powered System Monitor

```bash
#!/bin/bash
# system-monitor.sh - AI-enhanced system monitoring

check_system() {
    echo "🖥️  System Status Check"
    echo "====================="
    
    # Gather system info
    cpu_usage=$(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | cut -d'%' -f1)
    memory_usage=$(free | grep Mem | awk '{printf "%.1f", $3/$2 * 100.0}')
    disk_usage=$(df -h / | awk 'NR==2 {print $5}' | cut -d'%' -f1)
    load_avg=$(uptime | awk -F'load average:' '{print $2}')
    
    system_info="CPU: ${cpu_usage}%, Memory: ${memory_usage}%, Disk: ${disk_usage}%, Load: ${load_avg}"
    
    echo "Current metrics: $system_info"
    
    # AI analysis
    echo ""
    echo "🧠 AI Analysis:"
    echo "=============="
    analysis=$(cllient ask gpt-4o-mini "Analyze these system metrics and recommend actions if needed: $system_info")
    echo "$analysis"
    
    # Check for alerts
    if (( $(echo "$cpu_usage > 80" | bc -l) )) || (( $(echo "$memory_usage > 85" | bc -l) )) || (( disk_usage > 90 )); then
        echo ""
        echo "🚨 ALERT: High resource usage detected!"
        alert_analysis=$(cllient ask deepseek-chat "System resources are high: $system_info. Provide immediate troubleshooting steps.")
        echo "$alert_analysis"
        
        # Send notification (customize as needed)
        echo "Alert: High system resource usage - $system_info" | mail -s "System Alert" admin@example.com
    fi
}

# Run check
check_system

# Add to crontab for regular monitoring:
# */15 * * * * /path/to/system-monitor.sh >> /var/log/ai-monitor.log 2>&1
```

### Application Health Check

```bash
#!/bin/bash
# health-check.sh - AI-powered application health monitoring

APP_NAME="$1"
APP_URL="$2"

if [ -z "$APP_NAME" ] || [ -z "$APP_URL" ]; then
    echo "Usage: $0 <app-name> <app-url>"
    exit 1
fi

echo "🩺 Health Check: $APP_NAME"
echo "========================="

# Check HTTP response
http_status=$(curl -s -o /dev/null -w "%{http_code}" "$APP_URL")
response_time=$(curl -s -o /dev/null -w "%{time_total}" "$APP_URL")

# Check if service is running
service_status=$(systemctl is-active "$APP_NAME" 2>/dev/null || echo "unknown")

# Gather logs
recent_logs=$(journalctl -u "$APP_NAME" --since "5 minutes ago" --no-pager 2>/dev/null | tail -20)

health_info="HTTP Status: $http_status, Response Time: ${response_time}s, Service Status: $service_status"

echo "Metrics: $health_info"

# AI analysis
echo ""
echo "🤖 Health Analysis:"
echo "=================="
if [ "$http_status" = "200" ] && [ "$service_status" = "active" ]; then
    cllient ask claude-3-haiku-20240307 "Application health check passed: $health_info. Any optimization suggestions?"
else
    echo "🚨 Issues detected!"
    diagnostic=$(cllient ask deepseek-chat "Application health issues detected: $health_info. Recent logs: $recent_logs. Provide diagnostic steps and solutions.")
    echo "$diagnostic"
fi
```

---

## Content Generation

### Documentation Generator

```bash
#!/bin/bash
# doc-generator.sh - Generate documentation from code

PROJECT_DIR="$1"
if [ ! -d "$PROJECT_DIR" ]; then
    echo "Usage: $0 <project-directory>"
    exit 1
fi

OUTPUT_DIR="$PROJECT_DIR/docs/auto-generated"
mkdir -p "$OUTPUT_DIR"

echo "📝 Generating documentation for: $PROJECT_DIR"

# Generate README if missing
if [ ! -f "$PROJECT_DIR/README.md" ]; then
    echo "Creating README.md..."
    project_files=$(find "$PROJECT_DIR" -name "*.rs" -o -name "*.py" -o -name "*.js" | head -10)
    file_contents=""
    for file in $project_files; do
        file_contents="$file_contents\n\n=== $file ===\n$(head -20 "$file")"
    done
    
    readme=$(cllient ask claude-3-opus-20240229 "Generate a comprehensive README.md for this project based on these source files: $file_contents")
    echo "$readme" > "$PROJECT_DIR/README.md"
    echo "✅ README.md created"
fi

# Generate API documentation
echo "Creating API documentation..."
api_files=$(find "$PROJECT_DIR" -name "*.rs" -path "*/src/*" | head -5)
for file in $api_files; do
    if [ -f "$file" ]; then
        base_name=$(basename "$file" .rs)
        echo "Documenting: $file"
        
        file_content=$(cat "$file")
        doc_content=$(cllient ask deepseek-coder "Generate detailed API documentation for this code file: $file_content")
        echo "$doc_content" > "$OUTPUT_DIR/${base_name}-api.md"
    fi
done

echo "✅ Documentation generated in $OUTPUT_DIR"
```

### Test Case Generator

```bash
#!/bin/bash
# test-generator.sh - Generate test cases from source code

SOURCE_FILE="$1"
if [ ! -f "$SOURCE_FILE" ]; then
    echo "Usage: $0 <source-file>"
    exit 1
fi

echo "🧪 Generating tests for: $SOURCE_FILE"

# Determine language and test framework
extension="${SOURCE_FILE##*.}"
case $extension in
    "rs")
        test_framework="Rust with #[test] and assert!"
        ;;
    "py")
        test_framework="Python with pytest"
        ;;
    "js"|"ts")
        test_framework="JavaScript with Jest"
        ;;
    *)
        test_framework="appropriate testing framework"
        ;;
esac

# Generate tests
source_content=$(cat "$SOURCE_FILE")
tests=$(cllient ask deepseek-coder "Generate comprehensive unit tests for this code using $test_framework. Include edge cases, error conditions, and integration tests: $source_content")

# Write test file
test_file="${SOURCE_FILE%.*}_test.${extension}"
echo "$tests" > "$test_file"

echo "✅ Tests generated: $test_file"

# Generate test documentation
test_doc=$(cllient ask claude-3-haiku-20240307 "Create documentation explaining the test strategy and test cases for: $tests")
echo "$test_doc" > "${SOURCE_FILE%.*}_test_documentation.md"

echo "✅ Test documentation: ${SOURCE_FILE%.*}_test_documentation.md"
```

---

**Next**: [1. Basic Usage](1_basic-usage.md) | [2. CLI Usage Guide](../2_cli-usage.md)