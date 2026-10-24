// Drives the real page: feeds the synthetic export to the file input, checks the card, the stats table and the three
// downloads (PNG made by canvas, SVG, JSON), a theme change, and that the page makes no network request while it
// processes an export (only the files the page itself loads). Output is written into <pre id="selftest">.
const log = [];
const out = (k, v) => log.push(`${k}: ${v}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
try {
  const bytes = new Uint8Array(await (await fetch('synth.zip')).arrayBuffer());
  const seen = () => performance.getEntriesByType('resource').map((r) => new URL(r.name).pathname).filter((n) => n !== '/synth.zip' && n !== '/favicon.ico');
  for (let i = 0; i < 100 && !(seen().includes('/films.json') && seen().includes('/pkg/standuser_bg.wasm')); i++) await sleep(100);
  await sleep(500);
  const before = seen();
  const dt = new DataTransfer();
  dt.items.add(new File([bytes], 'letterboxd-synthuser-2026-06-30-12-00-utc.zip', { type: 'application/zip' }));
  const input = document.getElementById('file');
  input.files = dt.files;
  input.dispatchEvent(new Event('change'));
  for (let i = 0; i < 100 && document.getElementById('result').hidden; i++) await sleep(100);
  out('result visible', !document.getElementById('result').hidden);
  const after = seen();
  const added = after.filter((n) => !before.includes(n));
  out('requests while processing', added.length + (added.length ? ' ' + added.join(' ') : ''));
  out('page resources', [...new Set(after)].sort().join(' '));
  out('status', document.getElementById('status').textContent);
  out('rows', document.getElementById('stats').tBodies[0].rows.length);
  out('row0', [...document.getElementById('stats').tBodies[0].rows[0].cells].map((c) => c.textContent).join(' | '));
  out('notes', document.getElementById('notes').children.length);
  out('coverage cell', document.getElementById('stats').tBodies[0].rows[0].cells[3].textContent);
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
  // malformed input: an error message for a corrupt file, a card for a file with a bad date and a NaN rating, and the page keeps working
  const feed = async (url, name) => {
    const b = new Uint8Array(await (await fetch(url)).arrayBuffer());
    const dt2 = new DataTransfer();
    dt2.items.add(new File([b], name, { type: 'application/zip' }));
    input.files = dt2.files;
    input.dispatchEvent(new Event('change'));
    await sleep(1500);
  };
  await feed('bad.zip', 'bad.zip');
  out('corrupt file: result hidden', document.getElementById('result').hidden);
  out('corrupt file: status', document.getElementById('status').textContent.slice(0, 60));
  await feed('weird.zip', 'letterboxd-weird-2026-10-19-13-22-utc.zip');
  out('bad date + NaN rating: result visible', !document.getElementById('result').hidden);
  out('bad date + NaN rating: status', JSON.stringify(document.getElementById('status').textContent));
  await feed('synth.zip', 'letterboxd-synthuser-2026-06-30-12-00-utc.zip');
  out('good file afterwards: result visible', !document.getElementById('result').hidden);
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
