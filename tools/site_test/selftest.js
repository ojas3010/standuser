// Drives the real page: feeds the synthetic export to the file input, checks the card, the stats table and the three
// downloads (PNG made by canvas, SVG, JSON), a theme change, and that the page makes no network request while it
// processes an export (only the files the page itself loads). Every line is asserted: the output is "key: value [ok]"
// or "key: value [FAIL: expected ...]", and tools/site_test/run.sh exits non-zero on any FAIL or ERROR line.
// Nothing waits a fixed time for the page: each step waits for the element it changes (until / settled) and times out
// with an error instead of reading a half-finished page.
import { cardColours, contrast } from './accent.js';
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
  // before a drop: the sample card, labelled Sample, sits above the drop zone and the accent is still the ink colour
  const card0 = document.getElementById('card');
  await until('the sample card', () => card0.src.startsWith('blob:'));
  await card0.decode();
  out('sample label', document.getElementById('cardlabel').textContent, 'Sample');
  out('sample card', `${card0.naturalWidth}x${card0.naturalHeight}`, '1080x1350');
  out('sample card is above the drop zone', card0.getBoundingClientRect().top < document.getElementById('drop').getBoundingClientRect().top, true);
  out('no accent before a drop', document.documentElement.dataset.accent || '', '');
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
  out('drop zone is replaced', document.getElementById('drop').hidden, true);
  out('file line', document.getElementById('fileinfo').textContent, /^letterboxd-synthuser-2026-06-30-12-00-utc\.zip · \d[\d,]* films read$/);
  out('Change file button', document.getElementById('change').textContent, 'Change file');
  out('card scrolled into view', (() => { const r = document.getElementById('figure').getBoundingClientRect(); return r.top >= -2 && r.top < innerHeight; })(), true);
  const after = seen();
  const added = after.filter((n) => !before.includes(n));
  out('requests while processing', added.length + (added.length ? ' ' + added.join(' ') : ''), '0');
  out('page resources', [...new Set(after)].sort().join(' '), '/accent.js /app.js /films.json /fonts/archivo-black.woff2 /fonts/archivo.woff2 /names.json /pkg/standuser.js /pkg/standuser_bg.wasm /sample.json /selftest.js /style.css /tmdb.svg');
  out('status', statusEl().textContent, '');
  const rows = document.getElementById('stats').tBodies[0].rows;
  out('rows', rows.length, 6);
  out('row0', [...rows[0].cells].map((c) => c.textContent).join(' | '), /^Power \| C\*? \| -0\.049.+ \| .+ \| 210 of 250 films \| measured: 210 rated films with crowd data$/);
  out('Why these grades: six stats, each with value, film, graded on, status', [...rows].every((r) => r.cells.length === 6 && r.cells[0].textContent && r.cells[2].textContent && r.cells[3].textContent && r.cells[4].textContent && r.cells[5].textContent), true);
  for (const id of ['why-ability', 'why-weakness', 'why-name', 'why-film', 'why-type', 'why-credit']) out('Why these grades: ' + id, document.getElementById(id).textContent.length > 10, true);
  out('Why these grades: the credits', document.getElementById('why-credit').textContent, /^Data: Letterboxd, TMDB, MusicBrainz\./);
  out('Why these grades: the archetype note', document.getElementById('why-type').textContent.includes('archetype by stats'), true);
  out('Why these grades is under the share bar', document.getElementById('share').getBoundingClientRect().bottom <= document.querySelector('#result h2').getBoundingClientRect().top, true);
  out('notes', document.getElementById('notes').children.length, 2);
  out('coverage cell', rows[0].cells[4].textContent, '210 of 250 films');
  const img = document.getElementById('card');
  await img.decode();
  out('img', `${img.naturalWidth}x${img.naturalHeight} ${img.src.slice(0, 5)}`, '1080x1350 blob:');
  // downloads through the same buttons; each click is followed until its file has been produced
  const dls = [];
  HTMLAnchorElement.prototype.click = function () { dls.push([this.download, fetch(this.href).then((r) => r.arrayBuffer())]); };
  const heads = { 'card.png': '89 50 4e 47 d a 1a a', 'card.svg': '3c 73 76 67 20 78 6d 6c', 'stand.json': '7b a 20 20 22 61 62 69' };
  out('share bar is under the card', document.getElementById('card').getBoundingClientRect().bottom <= document.getElementById('share').getBoundingClientRect().top, true);
  out('share bar order', [...document.querySelectorAll('#share button:not([hidden])')].map((b) => b.id).join(' '), (v) => /^dl-png copy-img( share-btn)?( copy-link)?$/.test(v));
  out('Download SVG and JSON are text links in the method section', ['dl-svg', 'dl-json'].map((id) => document.getElementById(id).tagName + document.getElementById(id).closest('#result').tagName).join(' '), 'ASECTION ASECTION');
  out('Share is hidden exactly where the API cannot share files', document.getElementById('share-btn').hidden, !(navigator.share && navigator.canShare && navigator.canShare({ files: [new File([new Uint8Array(1)], 'a.png', { type: 'image/png' })] })));
  // Copy image puts a PNG on the clipboard
  if (!document.getElementById('copy-img').hidden) {
    let copied = null;
    Object.defineProperty(navigator.clipboard, 'write', { configurable: true, value: async (items) => { copied = items; } });
    document.getElementById('copy-img').click();
    await until('Copy image to write', () => copied, 5000);
    const blob = await copied[0].getType('image/png');
    const head = [...new Uint8Array(await blob.arrayBuffer()).slice(0, 8)].map((x) => x.toString(16)).join(' ');
    out('Copy image puts a PNG on the clipboard', `${copied[0].types.join()} ${head}`, 'image/png 89 50 4e 47 d a 1a a');
    out('Copy image says so', document.getElementById('sharenote').textContent, 'Image copied.');
  }
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
  // colour per user: the accent applied after the reveal (or at once with reduced motion) is the exact ground colour of the card on show,
  // with the card's ink for the text on it, at WCAG AA
  await until('the accent to shift', () => document.documentElement.dataset.accent, 3000);
  out('accent key', document.documentElement.dataset.accent, /^(PWR|SPD|RNG|STA|PRC|DEV|none)$/);
  const root = getComputedStyle(document.documentElement);
  const colours = cardColours(await (await fetch(img.src)).text());
  out('accent is the card ground', root.getPropertyValue('--accent').trim(), colours.ground);
  out('text on the accent is the card ink', root.getPropertyValue('--on-accent').trim(), colours.ink);
  out('accent label contrast', contrast(colours.ground, colours.ink).toFixed(2), (x) => Number(x) >= 4.5);
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
  // Copy link: the card's data in the URL fragment, base64url
  let copiedText = null;
  Object.defineProperty(navigator.clipboard, 'writeText', { configurable: true, value: async (t) => { copiedText = t; } });
  document.getElementById('copy-link').click();
  await until('Copy link to write', () => copiedText, 5000);
  out('link', copiedText.replace(/#c=.{12,}$/, '#c=…'), `${location.origin}${location.pathname}#c=…`);
  out('link payload is base64url', /#c=[A-Za-z0-9_-]{100,900}$/.test(copiedText), true);
  out('Copy link says so', document.getElementById('sharenote').textContent, 'Link copied.');
  out('next to Copy link: what the link contains', document.getElementById('linknote').textContent.includes('The link contains your username and grades'), true);
  // opening the link: the card with no file, and no request beyond the page's own load
  const open = async (hash) => {
    const f = document.createElement('iframe');
    f.src = 'index.html' + hash;
    f.style.cssText = 'position:fixed;left:-2000px;width:1280px;height:900px';
    document.body.appendChild(f);
    await new Promise((r) => f.addEventListener('load', r, { once: true }));
    const d = () => f.contentDocument;
    await until('the linked page to boot', () => d().getElementById('status') && d().getElementById('status').textContent === '' && !d().getElementById('drop').classList.contains('loading') && d().getElementById('card').src.startsWith('blob:'), 20000);
    return f;
  };
  const fr = await open(copiedText.slice(copiedText.indexOf('#')));
  const fd = fr.contentDocument;
  await fd.getElementById('card').decode();
  const names = () => fr.contentWindow.performance.getEntriesByType('resource').map((r) => new URL(r.name).pathname);
  const atLoad = names();
  out('opened link: card label', fd.getElementById('cardlabel').textContent, 'Shared card');
  out('opened link: card', `${fd.getElementById('card').naturalWidth}x${fd.getElementById('card').naturalHeight}`, '1080x1350');
  out('opened link: same card as the file made', (await (await fetch(fd.getElementById('card').src)).text()) === (await (await fetch(img.src)).text()), true);
  out('opened link: share bar visible, why section hidden', `${!fd.getElementById('share').hidden} ${fd.getElementById('result').hidden}`, 'true true');
  await sleep(1500);
  out('opened link: requests after the page load', names().filter((n) => !atLoad.includes(n)).length, 0);
  out('opened link: only the page\'s own files', [...new Set(names())].filter((n) => !/^\/(accent\.js|app\.js|films\.json|names\.json|sample\.json|style\.css|tmdb\.svg|pkg\/.*|fonts\/.*)$/.test(n)).join(' '), '');
  out('opened link: no file was read', fd.getElementById('filebar').hidden && !fd.getElementById('drop').hidden, true);
  fr.remove();
  // a malformed fragment shows the normal page
  for (const bad of ['#c=%21%21%21', '#c=e30', '#c=', '#c=' + 'A'.repeat(5000), '#x=1']) {
    const g = await open(bad);
    const gd = g.contentDocument;
    out(`malformed ${bad.slice(0, 12)}: normal page`, `${gd.getElementById('cardlabel').textContent} ${gd.getElementById('drop').hidden} ${gd.getElementById('share').hidden}`, 'Sample false true');
    g.remove();
  }
  // theme switch: the card image changes to a new blob
  const sel = document.getElementById('theme');
  out('card theme options', [...sel.options].map((o) => o.textContent).join(' | '), 'Light card | Dark card');
  out('card theme starts as the system', sel.value, matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
  if (sel.value === 'dark') { sel.value = 'light'; sel.dispatchEvent(new Event('change')); await sleep(50); }
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
