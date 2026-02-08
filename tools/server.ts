import { join, extname } from "path";

const ARTIFACTS_DIR = join(import.meta.dir, "../artifacts");
const TOOLS_DIR = import.meta.dir;

const MIME_TYPES: Record<string, string> = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".glb": "model/gltf-binary",
  ".png": "image/png",
  ".json": "application/json",
};

Bun.serve({
  port: 8090,
  development: {
    hmr: true,
    console: true,
  },
  async fetch(req) {
    const url = new URL(req.url);
    let pathname = url.pathname === "/" ? "/viewer.html" : url.pathname;

    // Try tools/ first (for viewer.html), then artifacts/ (for GLB files)
    let file = Bun.file(join(TOOLS_DIR, pathname));
    if (!(await file.exists())) {
      file = Bun.file(join(ARTIFACTS_DIR, pathname));
    }

    if (!(await file.exists())) {
      return new Response("not found", { status: 404 });
    }

    const ext = extname(pathname);
    const contentType = MIME_TYPES[ext] || "application/octet-stream";

    const headers: Record<string, string> = {
      "Content-Type": contentType,
      "Cache-Control": "no-cache",
    };

    if (req.method === "HEAD") {
      headers["Content-Length"] = String(file.size);
      headers["Last-Modified"] = new Date(file.lastModified).toUTCString();
      return new Response(null, { headers });
    }

    return new Response(file, { headers });
  },
});

console.log("Viewer at http://localhost:8090/viewer.html");
