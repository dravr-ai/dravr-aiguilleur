#!/bin/bash
# ABOUTME: Pre-push validation script for dravr-aiguilleur.
# ABOUTME: Runs fmt, clippy, tests, architectural validation, and writes the validation marker the pre-push hook checks.
#
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 dravr.ai

set -e

GIT_DIR="$(git rev-parse --git-dir)"
MARKER_FILE="$GIT_DIR/validation-passed"

echo "🔍 Running pre-push validation..."
echo ""

# Tier 0: Format check
echo "━━━ Tier 0: Format Check ━━━"
if ! cargo fmt --all -- --check; then
    echo "❌ Format check failed. Run: cargo fmt --all"
    exit 1
fi
echo "✅ Format OK"
echo ""

# Tier 1: Clippy (workspace, all targets, warnings as errors)
echo "━━━ Tier 1: Clippy ━━━"
if ! cargo clippy --workspace --all-targets -- -D warnings; then
    echo "❌ Clippy failed"
    exit 1
fi
echo "✅ Clippy OK"
echo ""

# Tier 2: Architectural validation (shared dravr rules)
echo "━━━ Tier 2: Architectural Validation ━━━"
if [ -x .build/validation/validate.sh ]; then
    if ! .build/validation/validate.sh; then
        echo "❌ Architectural validation failed"
        exit 1
    fi
    echo "✅ Architectural validation OK"
else
    echo "⚠️  .build/validation/validate.sh missing — run: git submodule update --init --recursive"
    exit 1
fi
echo ""

# Tier 3: Tests
echo "━━━ Tier 3: Tests ━━━"
if ! cargo test --workspace; then
    echo "❌ Tests failed"
    exit 1
fi
echo "✅ Tests OK"
echo ""

# Create validation marker the pre-push hook checks against HEAD.
CURRENT_COMMIT=$(git rev-parse HEAD)
CURRENT_TIMESTAMP=$(date +%s)
echo "$CURRENT_TIMESTAMP $CURRENT_COMMIT" > "$MARKER_FILE"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ All validation passed!"
echo "   Marker created: $MARKER_FILE"
echo "   You can now: git push"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━"
