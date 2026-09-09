// Post-build script: strip crossorigin from dist/index.html
// Vite always adds crossorigin to script/link tags, which breaks Tauri WebView2
const fs = require('fs');
const path = require('path');

const distFile = path.join(__dirname, 'dist', 'index.html');
if (fs.existsSync(distFile)) {
  let html = fs.readFileSync(distFile, 'utf-8');
  const before = html.length;
  html = html.replace(/\s+crossorigin/g, '');
  html = html.replace(/crossorigin\s+/g, '');
  html = html.replace(/crossorigin/g, '');
  fs.writeFileSync(distFile, html, 'utf-8');
  console.log(`[postbuild] Stripped crossorigin (${before} -> ${html.length} bytes)`);
} else {
  console.error('[postbuild] dist/index.html not found');
  process.exit(1);
}
