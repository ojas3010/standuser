// Drives the real page: feeds the synthetic export to the file input, checks the card, the stats table and the three
// downloads (PNG made by canvas, SVG, JSON), a theme change, and that the page makes no network request while it
// processes an export (only the files the page itself loads). Every line is asserted: the output is "key: value [ok]"
// or "key: value [FAIL: expected ...]", and tools/site_test/run.sh exits non-zero on any FAIL or ERROR line.
// Nothing waits a fixed time for the page: each step waits for the element it changes (until / settled) and times out
// with an error instead of reading a half-finished page.
import { ACCENTS, GROUND, GROUND2, contrast } from './accent.js';
const log = [];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const same = (v, e) => (e instanceof RegExp ? e.test(String(v)) : e instanceof Function ? e(v) : String(v) === String(e));
const out = (k, v, expect) => {
  const ok = same(v, expect);
  log.push(`${k}: ${v} [${ok ? 'ok' : 'FAIL: expected ' + (expect instanceof Function ? 'predicate' : expect)}]`);
};
const until = async (what, cond, ms = 20000) => {
  const t0 = performance.now();
  while (!cond()) {
    if (performance.now() - t0 > ms) throw new Error('timed out waiting for ' + what);
    await sleep(20);
  }
};
const statusEl = () => document.getElementById('status');
// Resolves when the page has finished handling one file: its status text went to "Reading the export…" and then left it
// (empty on success, the message on an error). Start it before the change event is dispatched.
const settled = (what) => new Promise((res, rej) => {
  let started = false;
  const obs = new MutationObserver(() => {
    const t = statusEl().textContent;
    if (t === 'Reading the export…') started = true;
    else if (started) { obs.disconnect(); clearTimeout(to); res(); }
  });
  obs.observe(statusEl(), { childList: true, characterData: true, subtree: true });
  const to = setTimeout(() => { obs.disconnect(); rej(new Error('timed out waiting for the page to finish ' + what)); }, 20000);
});
try {
  const bytes = new Uint8Array(await (await fetch('synth.zip')).arrayBuffer());
  const seen = () => performance.getEntriesByType('resource').map((r) => new URL(r.name).pathname).filter((n) => n !== '/synth.zip' && n !== '/favicon.ico');
  // the page is booted when it has loaded the table and the wasm and its status line is empty again
  await until('the page to boot', () => seen().includes('/films.json') && seen().includes('/names.json') && seen().includes('/pkg/standuser_bg.wasm') && statusEl().textContent === '');
  // the privacy line sits in the drop zone: the three files read and the two columns of profile.csv
  const dropText = document.getElementById('drop').textContent;
  for (const w of ['watched.csv', 'ratings.csv', 'diary.csv', 'two columns of', 'profile.csv', 'Date Joined', 'Favorite Films']) out('drop zone says ' + w, dropText.includes(w), true);
  const before = seen();
  const result = document.getElementById('result');
  const input = document.getElementById('file');
  const feed = async (url, name) => {
    const b = new Uint8Array(await (await fetch(url)).arrayBuffer());
    const dt = new DataTransfer();
    dt.items.add(new File([b], name, { type: 'application/zip' }));
    input.files = dt.files;
    const done = settled(name);
    input.dispatchEvent(new Event('change'));
    await done;
  };
  await feed('synth.zip', 'letterboxd-synthuser-2026-06-30-12-00-utc.zip');
  out('result visible', !result.hidden, true);
  const after = seen();
  const added = after.filter((n) => !before.includes(n));
  out('requests while processing', added.length + (added.length ? ' ' + added.join(' ') : ''), '0');
  out('page resources', [...new Set(after)].sort().join(' '), '/accent.js /app.js /films.json /fonts/archivo-black.woff2 /fonts/archivo.woff2 /names.json /pkg/standuser.js /pkg/standuser_bg.wasm /selftest.js /style.css /tmdb.svg');
  out('status', statusEl().textContent, '');
  const rows = document.getElementById('stats').tBodies[0].rows;
  out('rows', rows.length, 6);
  out('row0', [...rows[0].cells].map((c) => c.textContent).join(' | '), 'Power | C | -0.049 | 210 of 250 films | measured: 210 rated films with crowd data');
  out('notes', document.getElementById('notes').children.length, 2);
  out('coverage cell', rows[0].cells[3].textContent, '210 of 250 films');
  const img = document.getElementById('card');
  await img.decode();
  out('img', `${img.naturalWidth}x${img.naturalHeight} ${img.src.slice(0, 5)}`, '1080x1350 blob:');
  // downloads through the same buttons; each click is followed until its file has been produced
  const dls = [];
  HTMLAnchorElement.prototype.click = function () { dls.push([this.download, fetch(this.href).then((r) => r.arrayBuffer())]); };
  const heads = { 'card.png': '89 50 4e 47 d a 1a a', 'card.svg': '3c 73 76 67 20 78 6d 6c', 'stand.json': '7b a 20 20 22 61 62 69' };
  for (const [n, id] of [[1, 'dl-png'], [2, 'dl-svg'], [3, 'dl-json']]) {
    document.getElementById(id).click();
    await until(id + ' to produce a file', () => dls.length === n);
  }
  for (const [name, href] of dls) {
    const b = new Uint8Array(await href);
    const head = [...b.slice(0, 8)].map((x) => x.toString(16)).join(' ');
    out('download ' + name, b.length + ' bytes, head=' + head, (v) => b.length > 10000 && head === heads[name]);
    if (name === 'stand.json') {
      const a = JSON.parse(new TextDecoder().decode(b));
      out('name route', a.name.route, 'titles');
      out('name has a why line on the card data', a.card.why.startsWith('Why 「' + a.name.name + '」'), true);
    }
  }
  // colour per user: the accent was applied after the reveal (or at once with reduced motion) and every accent passes WCAG AA on the grounds
  await until('the accent to shift', () => document.documentElement.dataset.accent, 3000);
  out('accent key', document.documentElement.dataset.accent, /^(PWR|SPD|RNG|STA|PRC|DEV)$/);
  for (const [k, v] of Object.entries(ACCENTS)) {
    for (const scheme of ['light', 'dark']) {
      const worst = Math.min(contrast(v[scheme], GROUND[scheme]), contrast(v[scheme], GROUND2[scheme]));
      out(`accent ${k} ${scheme} contrast on the ground`, worst.toFixed(2), (x) => Number(x) >= 4.5);
    }
  }
  out('Download PNG is the one primary button', document.querySelectorAll('button.primary').length, 1);
  // malformed input: an error message for a corrupt file, a card for a file with a bad date and a NaN rating, and the page keeps working
  await feed('bad.zip', 'bad.zip');
  out('corrupt file: result hidden', result.hidden, true);
  out('corrupt file: status', statusEl().textContent.slice(0, 60), /^cannot read the ZIP file/);
  out('corrupt file: status is an error', statusEl().classList.contains('error'), true);
  await feed('weird.zip', 'letterboxd-weird-2026-10-19-13-22-utc.zip');
  out('bad date + NaN rating: result visible', !result.hidden, true);
  out('bad date + NaN rating: status', JSON.stringify(statusEl().textContent), '""');
  out('bad date + NaN rating: rows', rows.length, 6);
  await feed('synth.zip', 'letterboxd-synthuser-2026-06-30-12-00-utc.zip');
  out('good file afterwards: result visible', !result.hidden, true);
  out('good file afterwards: status', JSON.stringify(statusEl().textContent), '""');
  // theme switch: the card image changes to a new blob
  const sel = document.getElementById('theme');
  const was = img.src;
  sel.value = 'dark';
  sel.dispatchEvent(new Event('change'));
  await until('the card to change theme', () => img.src !== was);
  await img.decode();
  out('after theme change img', img.src.slice(0, 5) + (img.naturalWidth === 1080 ? ' 1080 wide' : ' wrong size'), 'blob: 1080 wide');
} catch (e) {
  log.push(`ERROR: ${(e && e.stack) || e}`);
}
document.body.insertAdjacentHTML('beforeend', `<pre id="selftest">${log.join('\n').replace(/</g, '&lt;')}</pre>`);
