// standuser site: export.zip in, card out, all in the browser. No network use after the page, the film table and the name candidates load.
import init, { run, render_card, share_encode, share_decode } from './pkg/standuser.js';
import { cardColours } from './accent.js';

const $ = (id) => document.getElementById(id);
const drop = $('drop');
const statusEl = $('status');
let table = null;
let names = '';
let last = null; // { svgs, card, analysis } of the card on show (a file's); null on the sample
let sample = null; // { svgs, card } of the sample card

// Before a card of the user's exists the stage has two columns (the drop zone, the sample); after a file or on an opened link it is one
// column: the file line, the card, the share bar.
const setDone = (on) => $('stage').classList.toggle('done', on);

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
    const [, t, n, smp] = await Promise.all([init(), text('films.json'), text('names.json').catch((e) => e), text('sample.json')]);
    table = t;
    const r = JSON.parse(render_card(smp));
    if (r.error) throw new Error(r.error);
    sample = { svgs: r.svgs, card: r.card };
    if (!openLink()) showSample();
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

function showSample() {
  if (sample) showCard(sample.svgs[resolvedTheme()], 'Sample', 'Sample Stand card, made from synthetic data');
}

// A share link (#c=...) holds the data the card needs: it renders the card with no file and no request. A fragment that is not a
// well-formed card link leaves the page as it is, in its normal state.
function openLink() {
  if (!location.hash.startsWith('#c=')) return false;
  let r;
  try { r = JSON.parse(share_decode(location.hash)); } catch { return false; }
  if (r.error) return false;
  last = { svgs: r.svgs, card: r.card, analysis: null };
  showCard(r.svgs[resolvedTheme()], 'Shared card', `Shared Stand card for ${r.card.user}`);
  $('result').hidden = true;
  $('downloads').hidden = true;
  $('share').hidden = false;
  $('sharenote').textContent = '';
  setDone(true);
  reveal(r.card, r.svgs);
  return true;
}
addEventListener('hashchange', () => ready.then(() => { if (!openLink() && !last) showSample(); }));

function exec(bytes, name) {
  const out = JSON.parse(run(bytes, name, table, names, $('joined').value || '', $('username').value.trim(), new Date().toISOString().slice(0, 10)));
  if (out.error) throw new Error(out.error);
  return out;
}

async function handle(file) {
  await ready;
  if (!table) return;
  $('result').hidden = true;
  $('downloads').hidden = true;
  $('share').hidden = true;
  try {
    setStatus('Reading the export…');
    const bytes = new Uint8Array(await file.arrayBuffer());
    const out = exec(bytes, file.name);
    last = { svgs: out.svgs, card: out.analysis.card, analysis: out.analysis };
    if (location.hash) history.replaceState(null, '', location.pathname + location.search);
    show(out.analysis, out.svgs[resolvedTheme()]);
    // the drop zone gives way to one line: the file, the films read, and a way to pick another
    $('fileinfo').textContent = `${file.name} · ${out.analysis.coverage.watched.toLocaleString('en-US')} films read`;
    drop.hidden = true;
    $('filebar').hidden = false;
    $('share').hidden = false;
    $('sharenote').textContent = '';
    setDone(true);
    reveal(out.analysis.card, out.svgs);
    $('stage').scrollIntoView({ block: 'start' });
    setStatus('');
  } catch (e) {
    // back to the first state: the sample card, the ink accent, the drop zone
    last = null;
    if (location.hash.startsWith('#c=')) history.replaceState(null, '', location.pathname + location.search);
    clearTimeout(accentTimer);
    const root = document.documentElement;
    root.style.removeProperty('--accent');
    root.style.removeProperty('--on-accent');
    delete root.dataset.accent;
    showSample();
    drop.hidden = false;
    setDone(false);
    $('filebar').hidden = true;
    $('share').hidden = true;
    setStatus(e.message, true);
  }
}

// The one colour event. The card lands (opacity 0 and scale 0.98 to 1, 200 ms, ease-out), holds, then the page's accent shifts once
// from the ink colour to the exact ground colour of the card on show (a 250 ms colour transition in style.css), with the card's own ink
// as the colour of the text on it. With reduced motion there is no landing and no hold: the card and the accent are simply there.
const LAND_MS = 200;
const HOLD_MS = 700;
let accentTimer = 0;
// The card starts in the theme of the system and follows it until the user picks one.
const resolvedTheme = () => $('theme').value;
const systemDark = matchMedia('(prefers-color-scheme: dark)');
let picked = false;
$('theme').value = systemDark.matches ? 'dark' : 'light';
$('theme').addEventListener('change', () => { picked = true; });
systemDark.addEventListener('change', (e) => {
  if (picked) return;
  $('theme').value = e.matches ? 'dark' : 'light';
  $('theme').dispatchEvent(new Event('change'));
  picked = false;
});
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

function showCard(svg, label, alt) {
  const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
  const img = $('card');
  const old = img.src;
  img.src = url;
  img.alt = alt;
  $('cardlabel').textContent = label;
  if (old.startsWith('blob:')) URL.revokeObjectURL(old);
}

function show(a, svg) {
  showCard(svg, 'Your card', 'Your Stand card');
  // Why these grades: for each stat its value, your film and what it is based on
  const tb = $('stats').tBodies[0];
  tb.replaceChildren();
  const names = ['Power', 'Speed', 'Range', 'Stamina', 'Precision', 'Potential'];
  const g = a.stats;
  const card = a.card;
  [g.power, g.speed, g.range, g.stamina, g.precision, g.potential].forEach((s, i) => {
    const tr = tb.insertRow();
    const val = s.value == null ? '' : Number(s.value).toFixed(3);
    const cov = a.coverage.per_stat[i];
    const [metric, film] = card.evidence[i];
    // Based on: how many films or watches it was graded on, then the status and what the note adds. A count the first line already
    // gives is not said again: the note's leading count is dropped, and the minimum is left to the note when it says what is needed.
    const note = (s.note || '').replace(new RegExp(`^${cov.used} `), '');
    const needs = cov.meets_minimum || note.startsWith('needs') ? '' : ` (needs ${cov.minimum})`;
    const cells = [names[i], s.letter + (card.prov[i] ? '*' : ''), val, film || '–', `${cov.used} of ${cov.of} ${cov.unit}${needs}`];
    cells.forEach((t) => (tr.insertCell().textContent = t));
    // the line the stat was shown with on the card before: what the value means
    if (metric) tr.cells[2].append(Object.assign(document.createElement('small'), { textContent: metric }));
    tr.cells[4].append(Object.assign(document.createElement('small'), { textContent: s.status + (note ? ` · ${note}` : '') }));
  });
  $('why-ability').textContent = card.ability;
  // the section is headed Weakness, so the line drops its label
  const weak = card.weakness.replace(/^(Weakness|Soft spot):\s*/, '');
  $('why-weakness').textContent = weak[0].toUpperCase() + weak.slice(1);
  // the card's corner brackets are card lettering; the page quotes the name in plain quotes
  // the section is headed The name, so the leading Why 「NAME」: goes
  const why = (a.name.why || card.why || 'No name line.').replace(/^Why 「[^」]*」:\s*/, '').replace(/「/g, '“').replace(/」/g, '”');
  $('why-name').textContent = why[0].toUpperCase() + why.slice(1);
  // the section's own heading says it is the signature film, so the line drops its label
  const sig = card.signature.replace(/^Signature film:\s*/, '');
  $('why-film').textContent = sig.startsWith('not enough') ? sig[0].toUpperCase() + sig.slice(1) : sig;
  // a sentence: the type, where it comes from, how often it matches canon (the figure is read from the note, not repeated here)
  const agree = (card.type_note.match(/about (\d+)%/) || [])[1];
  $('why-type').textContent = `${card.type_en}. The type comes from your stats; it matches the canon Stand type about ${agree || 60}% of the time.` +
    (card.type_note.includes('low confidence') ? ' Confidence is low for this type.' : '');
  const notes = $('notes');
  notes.replaceChildren();
  const c = a.coverage;
  const items = [
    `${c.eligible} of ${c.rated} rated films have a Letterboxd crowd average with enough ratings.`,
    ...(c.table_built ? [`Film table built ${c.table_built}.`] : []),
    ...a.notes,
  ];
  for (const t of items) notes.appendChild(Object.assign(document.createElement('li'), { textContent: t }));
  setLink($('dl-svg'), new Blob([last.svgs.auto], { type: 'image/svg+xml' }));
  setLink($('dl-json'), new Blob([JSON.stringify(a, null, 2)], { type: 'application/json' }));
  $('result').hidden = false;
  $('downloads').hidden = false;
}

// a download link that points at a blob made in this tab
function setLink(a, blob) {
  if (a.href.startsWith('blob:')) URL.revokeObjectURL(a.href);
  a.href = URL.createObjectURL(blob);
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
  if (!last) return showSample();
  if (last.analysis) show(last.analysis, last.svgs[resolvedTheme()]);
  else showCard(last.svgs[resolvedTheme()], 'Shared card', `Shared Stand card for ${last.card.user}`);
  if (document.documentElement.dataset.accent) setAccent(last.svgs, last.card.accent);
});
const png = () => pngBlob(last.svgs[resolvedTheme()]);
$('dl-png').addEventListener('click', async () => {
  if (!last) return;
  try { download('card.png', await png()); } catch (e) { setStatus(e.message, true); }
});

// Copy image and Share need the clipboard and the Web Share API; each button stays hidden where its API is missing
const note = (t) => { $('sharenote').textContent = t; };
if (window.ClipboardItem && navigator.clipboard && navigator.clipboard.write) $('copy-img').hidden = false;
const canShareFile = () => {
  try { return !!(navigator.share && navigator.canShare && navigator.canShare({ files: [new File([new Uint8Array(1)], 'card.png', { type: 'image/png' })] })); } catch { return false; }
};
if (canShareFile()) $('share-btn').hidden = false;
$('copy-img').addEventListener('click', async () => {
  if (!last) return;
  try {
    // the PNG is handed over as a promise so that Safari keeps the click's permission while it is made
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': png() })]);
    note('Image copied.');
  } catch (e) { note(`Could not copy the image: ${e.message}`); }
});
// Copy link: the card's data in the fragment, which the browser never sends anywhere
$('copy-link').addEventListener('click', async () => {
  if (!last) return;
  const url = `${location.origin}${location.pathname}#c=${share_encode(JSON.stringify(last.card))}`;
  try {
    await navigator.clipboard.writeText(url);
    note('Link copied.');
  } catch { note(url); }
});
$('share-btn').addEventListener('click', async () => {
  if (!last) return;
  try {
    const file = new File([await png()], 'stand-card.png', { type: 'image/png' });
    await navigator.share({ files: [file], title: 'My Stand card' });
  } catch (e) { if (e.name !== 'AbortError') note(`Could not share: ${e.message}`); }
});

$('change').addEventListener('click', () => $('file').click());
$('pick').addEventListener('click', (e) => { e.stopPropagation(); $('file').click(); });
drop.addEventListener('click', () => $('file').click());
drop.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); $('file').click(); } });
$('file').addEventListener('change', (e) => e.target.files[0] && handle(e.target.files[0]));
['dragenter', 'dragover'].forEach((t) => drop.addEventListener(t, (e) => { e.preventDefault(); drop.classList.add('over'); }));
['dragleave', 'drop'].forEach((t) => drop.addEventListener(t, (e) => { e.preventDefault(); drop.classList.remove('over'); }));
drop.addEventListener('drop', (e) => e.dataTransfer.files[0] && handle(e.dataTransfer.files[0]));
