#!/usr/bin/env bash

# Get project root - if we're in scripts/, go up one level
if [[ "$PWD" == */scripts ]]; then
    PROJECT_ROOT="$(dirname "$PWD")"
else
    PROJECT_ROOT="$PWD"
fi

cd "$PROJECT_ROOT"
echo "Building from: $PROJECT_ROOT"

docker build -f ./scripts/Dockerfile-gemini . -t gemini

docker run --rm -it --env-file=scripts/.env_gemini.env -v "$PROJECT_ROOT/android-adb-run:/workspace" gemini
