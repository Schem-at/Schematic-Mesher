#!/bin/bash
# Start the GLB debug viewer with hot-reload.
# Usage: ./tools/serve.sh
# Then open http://localhost:8090/viewer.html

cd "$(dirname "$0")"
exec bun --hot server.ts
