#!/bin/bash
ROOT=$(cd $(dirname $0); pwd)

# https://betterprogramming.pub/best-practices-for-bash-scripts-17229889774d
set -o errexit
set -o nounset
set -o pipefail

# 1. One-shot development build (blocking) to ensure artifacts exist
# Only build if expected outputs are missing (speeds up dev loop)
cd "${ROOT}/../renderer"
# Respect VITE_OUT_DIR override to match Vite's actual output directory
DIST_DIR="${VITE_OUT_DIR:-${ROOT}/../crates/arto/assets/dist}"
if [ ! -f "${DIST_DIR}/main.js" ] || [ ! -f "${DIST_DIR}/main.css" ]; then
  echo "Building renderer artifacts..."
  pnpm exec vite build --mode development --minify false
else
  echo "Renderer artifacts found, skipping initial build..."
fi

# 2. Start Vite in watch mode in background for hot reload
pnpm run dev --logLevel silent >/dev/null 2>&1 &
VITE_PID=$!

# Trap to kill Vite on exit
trap "kill $VITE_PID 2>/dev/null || true" EXIT

# 3. Start Dioxus dev server
cd "${ROOT}/../crates/arto"
dx serve

# Kill Vite when dx serve exits
kill $VITE_PID 2>/dev/null || true
