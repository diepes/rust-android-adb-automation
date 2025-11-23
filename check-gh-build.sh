#!/bin/bash
# Script to check latest GitHub Actions build status

echo "# Fetching latest GitHub Actions run..."
echo ""

# Enable verbose API logging for GitHub CLI to aid debugging on large outputs
# export GH_DEBUG=api

# Get the latest run
RUN_DATA=$(gh api repos/diepes/rust-android-adb-automation/actions/runs?per_page=1)

# Extract details
RUN_ID=$(echo "$RUN_DATA" | jq -r '.workflow_runs[0].id')
STATUS=$(echo "$RUN_DATA" | jq -r '.workflow_runs[0].status')
CONCLUSION=$(echo "$RUN_DATA" | jq -r '.workflow_runs[0].conclusion')
BRANCH=$(echo "$RUN_DATA" | jq -r '.workflow_runs[0].head_branch')
TITLE=$(echo "$RUN_DATA" | jq -r '.workflow_runs[0].display_title')
URL=$(echo "$RUN_DATA" | jq -r '.workflow_runs[0].html_url')

echo "# Latest Run:"
echo "  ID: $RUN_ID"
echo "  Status: $STATUS"
echo "  Conclusion: $CONCLUSION"
echo "  Branch: $BRANCH"
echo "  Title: $TITLE"
echo "  URL: $URL"
echo ""

if [ "$CONCLUSION" = "failure" ]; then
    echo "# Build FAILED! 1/2 $ gh run view $RUN_ID"
    echo ""
    gh run view $RUN_ID
    echo ""
    echo "# Build FAILED! 2/2 $ gh run view $RUN_ID --log-failed "
    echo ""
    gh run view $RUN_ID --log-failed

    echo ""
    echo "# Fetching failed job logs via gh api"
    JOB_DATA=$(gh api repos/diepes/rust-android-adb-automation/actions/runs/$RUN_ID/jobs?per_page=100)
    FAILED_JOBS=$(echo "$JOB_DATA" | jq -r '.jobs[] | select(.conclusion != "success") | "\(.id):::\(.name)"')

    if [ -z "$FAILED_JOBS" ]; then
        echo "No failing jobs detected in API response."
    else
        while IFS= read -r job_entry; do
            JOB_ID="${job_entry%%:::*}"
            JOB_NAME="${job_entry#*:::}"
            LOG_FILE=$(mktemp)

            echo ""
            echo "## Job $JOB_ID - $JOB_NAME"
            gh api repos/diepes/rust-android-adb-automation/actions/jobs/$JOB_ID/logs > "$LOG_FILE"

            if command -v rg >/dev/null 2>&1; then
                echo "-- Top error lines (ripgrep) --"
                rg -n --ignore-case "error" "$LOG_FILE" | head -n 20 || true
            else
                echo "-- Top error lines (grep fallback) --"
                grep -n "error" "$LOG_FILE" | head -n 20 || true
            fi

            rm -f "$LOG_FILE"
        done <<EOF
$FAILED_JOBS
EOF
    fi
fi
echo "# The End. $0"
