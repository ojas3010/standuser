// Drives the real page: feeds the synthetic export to the file input, checks the card, the stats table and the three
// downloads (PNG made by canvas, SVG, JSON), and a theme change. Output is written into <pre id="selftest">.
const log = [];
const out = (k, v) => log.push(`${k}: ${v}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
try {
  const bytes = new Uint8Array(await (await fetch('synth.zip')).arrayBuffer());
  const dt = new DataTransfer();
  dt.items.add(new File([bytes], 'letterboxd-synthuser-2026-06-30-12-00-utc.zip', { type: 'application/zip' }));
  const input = document.getElementById('file');
  input.files = dt.files;
  input.dispatchEvent(new Event('change'));
  for (let i = 0; i < 100 && document.getElementById('result').hidden; i++) await sleep(100);
  out('result visible', !document.getElementById('result').hidden);
  out('status', document.getElementById('status').textContent);
  out('rows', document.getElementById('stats').tBodies[0].rows.length);
  out('row0', [...document.getElementById('stats').tBodies[0].rows[0].cells].map((c) => c.textContent).join(' | '));
  out('notes', document.getElementById('notes').children.length);
  const img = document.getElementById('card');
  await img.decode();
  out('img', `${img.naturalWidth}x${img.naturalHeight} ${img.src.slice(0, 5)}`);
  // PNG export through the same canvas path as the button
  const dls = [];
  HTMLAnchorElement.prototype.click = function () { dls.push([this.download, fetch(this.href).then((r) => r.arrayBuffer())]); };
  for (const id of ['dl-png', 'dl-svg', 'dl-json']) {
    document.getElementById(id).click();
    await sleep(1200);
  }
  for (const [name, href] of dls) {
    const b = new Uint8Array(await href);
    out('download ' + name, b.length + ' bytes, head=' + [...b.slice(0, 8)].map((x) => x.toString(16)).join(' '));
  }
  // theme switch
  const sel = document.getElementById('theme');
  sel.value = 'dark';
  sel.dispatchEvent(new Event('change'));
  await sleep(300);
  out('after theme change img', img.src.slice(0, 5));
} catch (e) {
  out('ERROR', e && e.stack || e);
}
document.body.insertAdjacentHTML('beforeend', `<pre id="selftest">${log.join('\n').replace(/</g, '&lt;')}</pre>`);
