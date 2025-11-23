#!/bin/bash
# Script to check latest GitHub Actions build status

echo "# Fetching latest GitHub Actions run..."
echo ""

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
fi
echo "# The End. $0"
