#!/bin/bash
# Start a local server for the GLB debug viewer.
# Usage: ./tools/serve.sh
# Then open http://localhost:8090/viewer.html

PORT="${1:-8090}"
DIR="artifacts"

# Copy viewer if needed
cp -n tools/viewer.html "$DIR/viewer.html" 2>/dev/null

echo "Open http://localhost:$PORT/viewer.html"
echo "Then: cargo run --example debug_scene  (browser auto-reloads)"
echo "Ctrl+C to stop."
echo ""

node -e "
const http = require('http');
const fs = require('fs');
const path = require('path');
const types = {'.html':'text/html','.glb':'model/gltf-binary','.png':'image/png','.json':'application/json'};
http.createServer((req, res) => {
  const url = req.url.split('?')[0];
  const file = path.join('$DIR', url === '/' ? 'viewer.html' : url);
  fs.stat(file, (e, stat) => {
    if (e) { res.writeHead(404); res.end('not found'); return; }
    if (req.method === 'HEAD') {
      res.writeHead(200, {'Last-Modified': stat.mtime.toUTCString(), 'Content-Length': stat.size, 'Access-Control-Allow-Origin':'*'});
      res.end();
      return;
    }
    fs.readFile(file, (err, data) => {
      if (err) { res.writeHead(500); res.end(); return; }
      const ext = path.extname(file);
      res.writeHead(200, {'Content-Type': types[ext]||'application/octet-stream', 'Last-Modified': stat.mtime.toUTCString(), 'Access-Control-Allow-Origin':'*'});
      res.end(data);
    });
  });
}).listen($PORT, () => console.log('Serving at http://localhost:$PORT/viewer.html'));
"
