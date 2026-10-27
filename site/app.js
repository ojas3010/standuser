// standuser site: export.zip in, card out, all in the browser. No network use after the page, the film table and the name candidates load.
import init, { run, render_card, share_encode, share_decode } from './pkg/standuser.js';
import { cardColours, fieldColours } from './accent.js';

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
    paintGlyphs(r.svgs.light);
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
  if (!sample) return;
  showCard(sample.svgs[resolvedTheme()], 'Sample', 'Sample Stand card, made from synthetic data');
  setField(sample.svgs); // before a drop the field is the sample card's colour
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
  endReveal();
  $('result').hidden = true;
  $('downloads').hidden = true;
  $('share').hidden = true;
  let arrow = null;
  try {
    setStatus('Reading the export…');
    // From the drop the zone shows the To Be Continued arrow, sliding in from the left (style.css), for at least ARROW_MS, however fast the
    // file is read; the reveal starts when it ends. A click or a key press ends the arrow and skips the reveal. Reduced motion: no arrow.
    if (!reducedMotion()) {
      arrow = holdArrow(performance.now());
      drop.classList.add('reading');
      await new Promise((r) => setTimeout(r, 40)); // a few frames to paint it before the wasm runs
    }
    const bytes = new Uint8Array(await file.arrayBuffer());
    const out = exec(bytes, file.name);
    last = { svgs: out.svgs, card: out.analysis.card, analysis: out.analysis };
    const skipped = arrow ? await arrow.ended : false;
    if (location.hash) history.replaceState(null, '', location.pathname + location.search);
    show(out.analysis, out.svgs[resolvedTheme()]);
    // the drop zone gives way to one line: the file, the films read, and a way to pick another
    $('fileinfo').textContent = `${file.name} · ${out.analysis.coverage.watched.toLocaleString('en-US')} films read`;
    drop.hidden = true;
    $('filebar').hidden = false;
    $('share').hidden = false;
    $('sharenote').textContent = '';
    setDone(true);
    reveal(out.analysis.card, out.svgs, skipped);
    $('stage').scrollIntoView({ block: 'start' });
    setStatus('');
  } catch (e) {
    // back to the first state: the sample card, the ink accent, the drop zone
    last = null;
    if (location.hash.startsWith('#c=')) history.replaceState(null, '', location.pathname + location.search);
    endReveal();
    root.style.removeProperty('--accent');
    root.style.removeProperty('--on-accent');
    delete root.dataset.accent;
    showSample();
    drop.hidden = false;
    setDone(false);
    $('filebar').hidden = true;
    $('share').hidden = true;
    setStatus(e.message, true);
  } finally {
    if (arrow) arrow.cancel();
    drop.classList.remove('reading');
  }
}

// The page's field (the band behind the header and the stage) is the card's ground colour, with the card's own ink on it (accent.js); before
// a drop it is the sample card's. The page's accent (the filled rules and the headings' rules) is the same ground colour after a drop.
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
const root = document.documentElement;
function setField(svgs) {
  const f = fieldColours(svgs[resolvedTheme()]);
  for (const [k, v] of Object.entries({ field: f.ground, 'field-ink': f.ink, 'field-mute': f.mute, dot: f.dot, ghost: f.ghost, flash: f.flash })) root.style.setProperty('--' + k, v);
}
function setAccent(svgs, key) {
  const { ground, ink } = cardColours(svgs[resolvedTheme()]);
  root.style.setProperty('--accent', ground);
  root.style.setProperty('--on-accent', ink);
  root.dataset.accent = key || 'none';
  setField(svgs);
}

// The ゴ of the card is the page's too: its outline path is read from the card's own SVG (no font, no request) and drawn big and faint
// in the field, and as the mark beside the drop zone while a file is dragged over it.
function glyphOf(svg) {
  const d = (svg.match(/data-part="go"><g transform="rotate\([^)]*\)"><path d="([^"]+)"/) || [])[1];
  if (!d) return null;
  const n = d.match(/-?\d*\.?\d+/g).map(Number);
  const xs = n.filter((_, i) => i % 2 === 0), ys = n.filter((_, i) => i % 2 === 1);
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  return { d, cx: (x0 + x1) / 2, cy: (y0 + y1) / 2, h: y1 - y0 };
}
// glyphs: [centre x, centre y, height], each leaning like the card's
const goPaths = (g, glyphs) => glyphs.map(([x, y, h]) => `<path d="${g.d}" transform="translate(${x} ${y}) rotate(-8) scale(${(h / g.h).toFixed(4)}) translate(${-g.cx} ${-g.cy})"/>`).join('');
function paintGlyphs(svg) {
  const g = glyphOf(svg);
  if (!g) return;
  const wide = (cls, box, glyphs) => `<svg class="${cls}" viewBox="${box}" preserveAspectRatio="xMinYMin slice" aria-hidden="true" focusable="false">${goPaths(g, glyphs)}</svg>`;
  $('field').innerHTML = wide('ghost', '0 0 1280 800', [[150, 200, 250], [235, 440, 205], [315, 640, 165], [1120, 250, 225], [1035, 470, 185], [1130, 650, 150]]);
  const [col, row] = $('dropgo').children;
  col.innerHTML = goPaths(g, [[44, 52, 84], [74, 138, 64], [100, 208, 48]]);
  row.innerHTML = goPaths(g, [[26, 24, 46], [76, 24, 38], [122, 24, 30]]);
}
// The band is as tall as the header and the stage: it follows them as the stage changes between one column and two.
const fieldHeight = () => root.style.setProperty('--field-h', `${Math.round($('stage').getBoundingClientRect().bottom + scrollY + 24)}px`);
new ResizeObserver(fieldHeight).observe(document.querySelector('main'));
addEventListener('load', fieldHeight);

// The reveal, at most 2.2 s, after the To Be Continued arrow (docs/DESIGN.md rule 8). At the drop the field flashes to the card's ground colour and the accent shifts with it.
// The card's own parts (data-part in its SVG) are played from an inline copy laid over the card (#cardlive; the PNG and the SVG that are
// downloaded are the card, static): the ゴ one at a time down the cascade, the name dropping in with a short shake, the radar fill growing
// from the centre, then the six grades popping in clockwise from the top, as in the anime's eyecatch. The delays and durations are in
// style.css. A click or a key press ends it at the card's end state; with reduced motion there is no copy and no flash, the end state is
// simply there.
const REVEAL_MS = 2150;
const ARROW_MS = 1000;
const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
// The wait for the arrow: `ended` resolves false ARROW_MS after `from`, or true as soon as a click or a key press skips it.
function holdArrow(from) {
  const keys = ['pointerdown', 'keydown'];
  let done;
  const ended = new Promise((res) => { done = res; });
  const finish = (skipped) => { clearTimeout(timer); keys.forEach((t) => removeEventListener(t, skip, true)); done(skipped); };
  const skip = () => finish(true);
  const timer = setTimeout(() => finish(false), Math.max(0, from + ARROW_MS - performance.now()));
  keys.forEach((t) => addEventListener(t, skip, true));
  return { ended, cancel: () => finish(false) };
}
let live = null; // { svg, timer, off } while a reveal plays
function endReveal() {
  if (!live) return;
  clearTimeout(live.timer);
  live.off();
  live.svg.remove();
  $('card').classList.remove('playing');
  $('field').classList.remove('flash');
  live = null;
}
function reveal(card, svgs, skipped = false) {
  endReveal();
  setAccent(svgs, card.accent);
  if (skipped || reducedMotion()) return;
  const tpl = document.createElement('template');
  tpl.innerHTML = svgs[resolvedTheme()];
  const svg = tpl.content.firstElementChild;
  for (const e of svg.querySelectorAll('title, desc')) e.remove();
  for (const a of ['width', 'height', 'role', 'aria-labelledby']) svg.removeAttribute(a);
  svg.id = 'cardlive';
  svg.setAttribute('aria-hidden', 'true'); // the card image stays in the page for screen readers
  svg.querySelectorAll('[data-part=go]').forEach((e, i) => e.style.setProperty('--i', i));
  svg.querySelectorAll('[data-part=grade]').forEach((e) => e.style.setProperty('--i', e.dataset.i));
  const img = $('card');
  img.classList.add('playing');
  img.after(svg);
  const field = $('field');
  field.classList.remove('flash');
  void field.offsetWidth; // restart the flash for a second file
  field.classList.add('flash');
  const off = () => ['pointerdown', 'keydown'].forEach((t) => removeEventListener(t, endReveal, true));
  ['pointerdown', 'keydown'].forEach((t) => addEventListener(t, endReveal, true));
  live = { svg, off, timer: setTimeout(endReveal, REVEAL_MS) };
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
  endReveal();
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
