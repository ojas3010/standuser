// standuser site: export.zip in, card out, all in the browser. No network use after the page, the film table and the name candidates load.
import init, { run } from './pkg/standuser.js';
import { cardColours } from './accent.js';

const $ = (id) => document.getElementById(id);
const drop = $('drop');
const statusEl = $('status');
let table = null;
let names = '';
let last = null; // { svgs, analysis }

const setStatus = (msg, error = false) => {
  statusEl.textContent = msg;
  statusEl.classList.toggle('error', error);
};

async function boot() {
  setStatus('Loading…');
  drop.classList.add('loading');
  drop.setAttribute('aria-busy', 'true');
  try {
    // the wasm, the film table and the name candidates load in parallel; the drop zone stays inactive until all three are in
    const text = async (url) => {
      const r = await fetch(url);
      if (!r.ok) throw new Error(`${url}: ${r.status}`);
      return r.text();
    };
    const [, t, n] = await Promise.all([init(), text('films.json'), text('names.json').catch((e) => e)]);
    table = t;
    if (n instanceof Error) setStatus(`The name candidates did not load (${n.message}); names will come from your composers.`);
    else {
      names = n;
      setStatus('');
    }
  } catch (e) {
    setStatus(`Could not load the page's data: ${e.message}`, true);
  } finally {
    drop.classList.remove('loading');
    drop.removeAttribute('aria-busy');
  }
}
const ready = boot();

function chosenTheme(kind) {
  const t = $('theme').value;
  if (kind === 'png') return t === 'auto' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : t;
  return t;
}

function exec(bytes, name) {
  const out = JSON.parse(run(bytes, name, table, names, $('joined').value || '', $('username').value.trim(), new Date().toISOString().slice(0, 10)));
  if (out.error) throw new Error(out.error);
  return out;
}

async function handle(file) {
  await ready;
  if (!table) return;
  $('result').hidden = true;
  try {
    setStatus('Reading the export…');
    const bytes = new Uint8Array(await file.arrayBuffer());
    const out = exec(bytes, file.name);
    last = { svgs: out.svgs, analysis: out.analysis };
    show(out.analysis, out.svgs[resolvedTheme()]);
    reveal(out.analysis.card, out.svgs);
    setStatus('');
  } catch (e) {
    setStatus(e.message, true);
  }
}

// The one colour event. The card lands (opacity 0 and scale 0.98 to 1, 200 ms, ease-out), holds, then the page's accent shifts once
// from the ink colour to the exact ground colour of the card on show (a 250 ms colour transition in style.css), with the card's own ink
// as the colour of the text on it. With reduced motion there is no landing and no hold: the card and the accent are simply there.
const LAND_MS = 200;
const HOLD_MS = 700;
let accentTimer = 0;
const resolvedTheme = () => (($('theme').value === 'auto') ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : $('theme').value);
function setAccent(svgs, key) {
  const root = document.documentElement;
  const { ground, ink } = cardColours(svgs[resolvedTheme()]);
  root.style.setProperty('--accent', ground);
  root.style.setProperty('--on-accent', ink);
  root.dataset.accent = key || 'none';
}
function reveal(card, svgs) {
  const img = $('card');
  const still = matchMedia('(prefers-reduced-motion: reduce)').matches;
  clearTimeout(accentTimer);
  img.classList.remove('land');
  if (!still) {
    void img.offsetWidth; // restart the animation for a second file
    img.classList.add('land');
  }
  if (still) setAccent(svgs, card.accent);
  else accentTimer = setTimeout(() => setAccent(svgs, card.accent), LAND_MS + HOLD_MS);
}

function show(a, svg) {
  const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
  const img = $('card');
  const old = img.src;
  img.src = url;
  if (old.startsWith('blob:')) URL.revokeObjectURL(old);
  const tb = $('stats').tBodies[0];
  tb.replaceChildren();
  const names = ['Power', 'Speed', 'Range', 'Stamina', 'Precision', 'Potential'];
  const g = a.stats;
  [g.power, g.speed, g.range, g.stamina, g.precision, g.potential].forEach((s, i) => {
    const tr = tb.insertRow();
    const val = s.value == null ? '' : Number(s.value).toFixed(3);
    const cov = a.coverage.per_stat[i];
    [names[i], s.letter, val, `${cov.used} of ${cov.of} ${cov.unit}${cov.meets_minimum ? '' : ` (needs ${cov.minimum})`}`, s.status + (s.note ? `: ${s.note}` : '')].forEach((t) => (tr.insertCell().textContent = t));
  });
  const notes = $('notes');
  notes.replaceChildren();
  const c = a.coverage;
  const items = [
    `${c.watched_in_table} of ${c.watched} watched films are in the built-in film table; the other ${c.not_in_table} are skipped by every stat that needs film data.`,
    `${c.eligible} of ${c.rated} rated films have a Letterboxd crowd average with enough ratings.`,
    ...(c.table_built ? [`Film table built ${c.table_built}.`] : []),
    ...a.notes,
  ];
  for (const t of items) notes.appendChild(Object.assign(document.createElement('li'), { textContent: t }));
  $('result').hidden = false;
}

function download(name, blob) {
  const a = Object.assign(document.createElement('a'), { href: URL.createObjectURL(blob), download: name });
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 2000);
}

async function pngBlob(svg) {
  const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
  try {
    const img = new Image();
    img.decoding = 'async';
    img.src = url;
    await img.decode();
    const canvas = Object.assign(document.createElement('canvas'), { width: 1080, height: 1350 });
    canvas.getContext('2d').drawImage(img, 0, 0, 1080, 1350);
    return await new Promise((res, rej) => canvas.toBlob((b) => (b ? res(b) : rej(new Error('PNG export failed'))), 'image/png'));
  } finally {
    URL.revokeObjectURL(url);
  }
}

$('theme').addEventListener('change', () => {
  if (!last) return;
  show(last.analysis, last.svgs[resolvedTheme()]);
  if (document.documentElement.dataset.accent) setAccent(last.svgs, last.analysis.card.accent);
});
$('dl-svg').addEventListener('click', () => last && download('card.svg', new Blob([last.svgs.auto], { type: 'image/svg+xml' })));
$('dl-png').addEventListener('click', async () => {
  if (!last) return;
  try { download('card.png', await pngBlob(last.svgs[chosenTheme('png')])); } catch (e) { setStatus(e.message, true); }
});
$('dl-json').addEventListener('click', () => last && download('stand.json', new Blob([JSON.stringify(last.analysis, null, 2)], { type: 'application/json' })));

$('pick').addEventListener('click', (e) => { e.stopPropagation(); $('file').click(); });
drop.addEventListener('click', () => $('file').click());
drop.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); $('file').click(); } });
$('file').addEventListener('change', (e) => e.target.files[0] && handle(e.target.files[0]));
['dragenter', 'dragover'].forEach((t) => drop.addEventListener(t, (e) => { e.preventDefault(); drop.classList.add('over'); }));
['dragleave', 'drop'].forEach((t) => drop.addEventListener(t, (e) => { e.preventDefault(); drop.classList.remove('over'); }));
drop.addEventListener('drop', (e) => e.dataTransfer.files[0] && handle(e.dataTransfer.files[0]));
