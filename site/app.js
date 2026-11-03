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
// "How the card was built" is there only once a card exists (a file's or an opened link's).
const setDone = (on) => { $('stage').classList.toggle('done', on); $('method').hidden = !on; };

// The page is in one of three modes, and this is the one place that sets what a mode is made of, so that no path leaves a flag of another mode
// behind (a link opened over a file's card, the bare URL after a link, a failed file):
//   sample: the sample card and the drop zone; nothing of the user's
//   link:   an opened #c= card: the card and the share bar; no file was read, so no file line, no tables, and the drop zone is there
//   own:    a file's card: the file line, the share bar, the tables; the drop zone is gone
// `shown` is { svgs, card, analysis } of the card on show (analysis is null for a link); it is `last`, and null on the sample. The accent's
// colours and the field's are set by reveal() (setAccent) and showSample() (setField), which the calm fade needs to time itself.
function setMode(mode, shown = null) {
  const sample = mode === 'sample', own = mode === 'own';
  last = sample ? null : shown;
  setDone(!sample); // #stage.done and #method
  drop.hidden = own;
  $('filebar').hidden = !own;
  $('share').hidden = sample;
  for (const id of ['result', 'adv-result', 'downloads']) $(id).hidden = !own;
  root.classList.toggle('link', mode === 'link');
  if (sample) {
    root.style.removeProperty('--accent');
    root.style.removeProperty('--on-accent');
    delete root.dataset.accent;
  } else root.dataset.accent = last.card.accent || 'none';
}

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
    // there is no table: the zone stays inactive (the finally below ends loading, not this) and nothing in it can be reached, by pointer or key
    drop.classList.add('failed');
    drop.inert = true;
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
  setMode('link', { svgs: r.svgs, card: r.card, analysis: null });
  showCard(r.svgs[resolvedTheme()], 'Shared card', `Shared Stand card for ${r.card.user}`);
  buildHots(null);
  toast('');
  reveal(r.card, r.svgs);
  return true;
}
// The bare URL again (Back from a link): the link's card goes, and the sample is back. A file's card stays: its data is not in the URL.
addEventListener('hashchange', () => ready.then(() => {
  if (openLink() || (last && last.analysis)) return;
  endReveal();
  buildHots(null);
  setMode('sample');
  showSample();
}));

function exec(bytes, name) {
  const out = JSON.parse(run(bytes, name, table, names, $('joined').value || '', $('username').value.trim(), new Date().toISOString().slice(0, 10)));
  if (out.error) throw new Error(out.error);
  return out;
}

let busy = false; // a handle() call is running: a new file is ignored, and Change file is inactive
async function handle(file) {
  await ready;
  if (!table || busy) return;
  busy = true;
  $('change').disabled = true;
  endReveal();
  $('result').hidden = true;
  $('adv-result').hidden = true;
  $('downloads').hidden = true;
  $('share').hidden = true;
  let hold = null;
  let arrowFrom = 0;
  try {
    setStatus('Reading the export…');
    // From the drop a line travels once along the zone's border (TRACE_MS), then the zone shows the To Be Continued arrow, sliding in from the
    // left with a short overshoot and spelling its words out letter by letter, with the page behind it held in grey halftone (style.css), for at
    // least ARROW_MS, however fast the file is read; the reveal starts when it ends. A click or a key press ends all of it and skips the
    // reveal. Reduced motion: no line, no arrow, no freeze (the calm reveal in reveal()).
    if (!reducedMotion()) {
      hold = skipWatch();
      // a second file: the zone was hidden after the first, and the trace, the arrow and the freeze are inside it. It comes back under the card
      // (one column), so it is scrolled to; the card is scrolled to again when it is shown.
      drop.hidden = false;
      drop.scrollIntoView({ block: 'center' });
      startTrace();
      drop.classList.add('tracing');
      const skippedTrace = await hold.wait(TRACE_MS);
      drop.classList.remove('tracing');
      if (!skippedTrace) {
        arrowFrom = performance.now();
        drop.classList.add('reading');
        await new Promise((r) => setTimeout(r, 40)); // a few frames to paint it before the wasm runs
      }
    }
    const bytes = new Uint8Array(await file.arrayBuffer());
    const out = exec(bytes, file.name);
    const skipped = hold ? hold.skipped() || (await hold.wait(arrowFrom + ARROW_MS - performance.now())) : false;
    if (location.hash) history.replaceState(null, '', location.pathname + location.search);
    // the drop zone gives way to one line: the films read, and a way to pick another (the file's name is not shown)
    setMode('own', { svgs: out.svgs, card: out.analysis.card, analysis: out.analysis });
    show(out.analysis, out.svgs[resolvedTheme()]);
    $('fileinfo').textContent = `${out.analysis.coverage.watched.toLocaleString('en-US')} films read`;
    toast('');
    reveal(out.analysis.card, out.svgs, skipped);
    $('stage').scrollIntoView({ block: 'start' });
    setStatus('');
  } catch (e) {
    // back to the first state: the sample card, the ink accent, the drop zone
    buildHots(null);
    if (location.hash.startsWith('#c=')) history.replaceState(null, '', location.pathname + location.search);
    endReveal();
    setMode('sample');
    showSample();
    setStatus(e.message, true);
  } finally {
    busy = false;
    $('change').disabled = false;
    if (hold) hold.cancel();
    drop.classList.remove('tracing', 'reading');
  }
}

// The page's field (the band behind the header and the stage) is the card's ground colour, with the card's own ink on it (accent.js); before
// a drop it is the sample card's. The page's accent (the filled rules and the headings' rules) is the same ground colour after a drop.
// The card starts in the theme of the system and follows it until the user picks one.
// The theme is the Light / Dark toggle in the share bar (#theme, data-value and aria-pressed on its two buttons).
const resolvedTheme = () => $('theme').dataset.value;
const systemDark = matchMedia('(prefers-color-scheme: dark)');
let picked = false;
const setTheme = (v) => {
  $('theme').dataset.value = v;
  for (const b of $('theme').querySelectorAll('button')) b.setAttribute('aria-pressed', String(b.dataset.v === v));
};
setTheme(systemDark.matches ? 'dark' : 'light');
for (const b of $('theme').querySelectorAll('button')) b.addEventListener('click', () => { picked = true; setTheme(b.dataset.v); themeChanged(); });
systemDark.addEventListener('change', (e) => {
  if (picked) return;
  setTheme(e.matches ? 'dark' : 'light');
  themeChanged();
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
document.fonts.ready.then(fieldHeight); // the headings change height when the web fonts swap in

// The percentile numbers in Why these grades count up once, over at most 0.6 s, when the reveal has ended. A click or a key press that skips the
// reveal, a new card, a theme change and prefers-reduced-motion all show the final numbers at once.
const COUNT_MS = 550;
let counting = null;
const pcts = () => [...document.querySelectorAll('#stats .pct')];
function settleNumbers() {
  if (counting) cancelAnimationFrame(counting);
  counting = null;
  for (const e of pcts()) e.textContent = e.dataset.n;
}
function countUp() {
  const els = pcts();
  if (!els.length || reducedMotion()) return settleNumbers();
  const t0 = performance.now();
  const tick = (now) => {
    const k = Math.min(1, (now - t0) / COUNT_MS), e = 1 - (1 - k) ** 3; // ease-out
    for (const el of els) el.textContent = String(Math.round(Number(el.dataset.n) * e));
    counting = k < 1 ? requestAnimationFrame(tick) : null;
  };
  for (const el of els) el.textContent = '0';
  counting = requestAnimationFrame(tick);
}

// The reveal, at most 2.2 s, after the To Be Continued arrow (docs/DESIGN.md rule 8). At the drop the field flashes to the card's ground colour and the accent shifts with it.
// The card's own parts (data-part in its SVG) are played from an inline copy laid over the card (#cardlive; the PNG and the SVG that are
// downloaded are the card, static): the ゴ one at a time down the cascade, the name dropping in with a short shake, the radar fill growing
// from the centre, then the six grades popping in clockwise from the top, as in the anime's eyecatch. The delays and durations are in
// style.css. A click or a key press ends it at the card's end state. With reduced motion (docs/DESIGN.md rule 4) the reveal is calm: the
// field fades to the card's colour (300 ms), the card fades in (400 ms), then all six grades fade in together; no movement, no flash, no
// trace, no arrow, no count-up.
const REVEAL_MS = 2150;
const CALM_MS = 700; // the calm reveal: the card 400 ms, then the grades 300 ms
const ARROW_MS = 1800;
const TRACE_MS = 700;
const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
// A click or a key press ends the trace and the arrow at once and skips the reveal. `wait(ms)` resolves after ms, or at once when skipped; true when skipped.
function skipWatch() {
  const keys = ['pointerdown', 'keydown'];
  let skipped = false, wake = null;
  const skip = () => { skipped = true; if (wake) wake(); };
  keys.forEach((t) => addEventListener(t, skip, true));
  return {
    skipped: () => skipped,
    wait: (ms) => (skipped ? Promise.resolve(true) : new Promise((res) => {
      const t = setTimeout(() => { wake = null; res(false); }, Math.max(0, ms));
      wake = () => { clearTimeout(t); wake = null; res(true); };
    })),
    cancel: () => keys.forEach((t) => removeEventListener(t, skip, true)),
  };
}
// The line that travels along the drop zone's border while the file is taken in. The technique is that of the file-upload component of BoardUI
// (MIT, github.com/BoardUI/boardui, components/base/file-upload/file-upload.tsx): an SVG whose viewBox is the measured box of the zone, a rect
// on the border line with pathLength=100, and stroke-dasharray / stroke-dashoffset to draw and move a part of that path. Here a dash of 22 of
// the 100 units laps the border once (style.css, @keyframes traceRun), in plain CSS and JavaScript.
function startTrace() {
  const w = drop.offsetWidth, h = drop.offsetHeight, svg = $('trace'), r = svg.querySelector('rect');
  svg.setAttribute('viewBox', `0 0 ${w} ${h}`);
  for (const [k, v] of Object.entries({ x: 1, y: 1, width: w - 2, height: h - 2 })) r.setAttribute(k, String(v));
}
let live = null; // { svg, timer, off } while a reveal plays
function endReveal() {
  if (!live) return;
  clearTimeout(live.timer);
  live.off();
  live.svg.remove();
  $('card').classList.remove('playing');
  $('field').classList.remove('flash', 'fading');
  live = null;
  $('hots').hidden = false;
  settleNumbers();
}
function reveal(card, svgs, skipped = false) {
  endReveal();
  const calm = reducedMotion();
  // calm reveal (prefers-reduced-motion): the field fades to the card's colour, then the card and its grades fade in; the CSS holds the times
  if (calm && !skipped) { $('field').classList.add('fading'); void $('field').offsetWidth; }
  setAccent(svgs, card.accent);
  if (skipped) return;
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
  closePop();
  $('hots').hidden = true;
  img.classList.add('playing');
  img.after(svg);
  if (!calm) {
    for (const e of pcts()) e.textContent = '0'; // they count up when the reveal ends
    const field = $('field');
    field.classList.remove('flash');
    void field.offsetWidth; // restart the flash for a second file
    field.classList.add('flash');
  }
  const off = () => ['pointerdown', 'keydown'].forEach((t) => removeEventListener(t, endReveal, true));
  ['pointerdown', 'keydown'].forEach((t) => addEventListener(t, endReveal, true));
  live = { svg, off, timer: setTimeout(() => { endReveal(); countUp(); }, calm ? CALM_MS : REVEAL_MS) };
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

// The lines under the table are one plain sentence each, with no [provisional] tag (the grade's asterisk says it).
const noTag = (t) => t.replace(/\s*\[provisional\]/g, '').trim();
const cap = (t) => (t ? t[0].toUpperCase() + t.slice(1) : t);
const dot = (t) => (/[.!?]$/.test(t) ? t : `${t}.`);
// "slow hands (Speed D). 1.5 films per active month, so ..." becomes "Slow hands: 1.5 films per active month, so ...": the parenthesis with the
// stat's name goes and the two sentences are joined
function weaknessLine(raw) {
  let t = noTag(raw).replace(/^(Weakness|Soft spot):\s*/, '');
  if (!t.startsWith('No weakness')) {
    t = t.replace(/ \([^)]*\)(?=\.)/, '');
    t = t.replace(/\. (\S)(\S?)/, (m, c, d) => `: ${/\p{Lu}/u.test(d) ? c : c.toLowerCase()}${d}`);
  }
  return dot(cap(t));
}
// "Track N of COMPOSER's FILM score, from a R★ film of yours." from the name's own why line (track N of ...'s FILM score (year) ... (R★));
// a name that did not come from a track title (a composer, a Tarot card) keeps its why line, as one sentence
function nameLine(raw) {
  const why = noTag(raw);
  const m = /track (\d+) of the (.+?) (score|soundtrack) \(\d{4}\)/.exec(why) || /track (\d+) of (.+?['\u2019]s?) (.+?) (score|soundtrack) \(\d{4}\)/.exec(why);
  if (!m) return dot(cap(why.replace(/^Why \u300c[^\u300d]*\u300d:\s*/, '').replace(/\u300c/g, '\u201c').replace(/\u300d/g, '\u201d')) || 'No name line.');
  const rating = /\((\d(?:\.\d)?)\u2605\)/.exec(why.slice(m.index + m[0].length));
  const [, n] = m;
  const place = m.length === 4 ? `the ${m[2]} ${m[3]}` : `${m[2]} ${m[3]} ${m[4]}`;
  return `Track ${n} of ${place}, from a ${rating ? `${rating[1]}\u2605 ` : ''}film of yours.`;
}
// "Signature film: Cabaret (1972), 0.5★ against the crowd's 4.18★ (-3.68), 200,558 ratings." as one sentence
function signatureLine(raw) {
  const sig = noTag(raw).replace(/^Signature film:\s*/, '');
  const m = /^(.+?) \((\d{4})\), ([\d.]+)\u2605 against the crowd's ([\d.]+)\u2605 \([^)]*\), [\d,]+ ratings\.?$/.exec(sig);
  if (m) return `${q(m[1])} (${m[2]}) is where you and the crowd disagree most: you gave it ${m[3]}\u2605, its average is ${m[4]}\u2605.`;
  return dot(cap(sig));
}

// One plain sentence per stat. It names the film the card's evidence line names for that stat (`film`; Range's reads "Film (Country)") and says
// what the film shows; with no film it says what the stat is based on. A stat that is not graded yet (its metric starts with ? or ∅) says so.
const q = (f) => `\u201c${f}\u201d`;
function statSentence(i, metric, film) {
  const f = film ? q(film) : '';
  const own = /^(.*) \((.+)\)$/.exec(film || '');
  const dir = metric.startsWith('\u2212') ? 'below' : metric.startsWith('+') ? 'above' : '';
  const s = [
    dir ? `You rate ${dir} the crowd on average${f ? `, most of all on ${f}` : ''}.` : f ? `${f} is where you differ most from the crowd.` : 'Based on how your ratings compare with the crowd\u2019s.',
    f ? `Your busiest month of watching included ${f}.` : 'Based on how many films you log in the months you watch.',
    own ? `Your rarest country is ${own[2]}, through ${q(own[1])}.` : f ? `Your rarest country is shown by ${f}.` : 'Based on how many different countries your films come from.',
    f ? `${f} is from the last month of your longest run of watching.` : 'Based on how many months you watched something.',
    f ? `You rate close to the crowd on well-known films, such as ${f}.` : 'Based on how closely your ratings follow the crowd\u2019s.',
    f ? `${f} is by a director you have seen only once.` : 'Based on how many directors you have seen only once.',
  ][i];
  return /^[?\u2205]/.test(metric) ? `${s} Not enough data to grade this yet.` : s;
}

function show(a, svg) {
  showCard(svg, 'Your card', 'Your Stand card');
  // Why these grades: for each stat its grade and one plain sentence that names your film. The raw value, the counts and the status words
  // are in Advanced.
  const tb = $('stats').tBodies[0];
  tb.replaceChildren();
  const dt = $('detail').tBodies[0];
  dt.replaceChildren();
  const names = ['Power', 'Speed', 'Range', 'Stamina', 'Precision', 'Potential'];
  const g = a.stats;
  const card = a.card;
  const list = [];
  [g.power, g.speed, g.range, g.stamina, g.precision, g.potential].forEach((s, i) => {
    const [metric, film] = card.evidence[i];
    const tr = tb.insertRow();
    const sentence = statSentence(i, metric, film);
    list.push({ name: names[i], letter: s.letter + (card.prov[i] ? '*' : ''), sentence, pct: topPct(s.p) });
    [names[i], s.letter + (card.prov[i] ? '*' : ''), sentence].forEach((t) => (tr.insertCell().textContent = t));
    // the percentile beside the grade ("B · top 36%"); its number is counted up once after the reveal (countUp)
    const pct = topPct(s.p);
    if (pct != null) tr.cells[1].append(' \u00b7 top ', Object.assign(document.createElement('span'), { className: 'pct', textContent: String(pct) }), '%');
    if (pct != null) tr.cells[1].querySelector('.pct').dataset.n = String(pct);
    // Advanced: the value, how many films or watches it was graded on, then the status and what the note adds. A count the first line
    // already gives is not said again: the note's leading count is dropped, and the minimum is left to the note when it says what is needed.
    const dr = dt.insertRow();
    const val = s.value == null ? '' : Number(s.value).toFixed(3);
    const cov = a.coverage.per_stat[i];
    const note = (s.note || '').replace(new RegExp(`^${cov.used} `), '');
    const needs = cov.meets_minimum || note.startsWith('needs') ? '' : ` (needs ${cov.minimum})`;
    [names[i], val, `${cov.used} of ${cov.of} ${cov.unit}${needs}`, ''].forEach((t) => (dr.insertCell().textContent = t));
    if (metric) dr.cells[1].append(Object.assign(document.createElement('small'), { textContent: metric }));
    dr.cells[3].textContent = s.status + (note ? ` · ${note}` : '');
  });
  buildHots(list);
  // The ability is on the card, so the page does not repeat it. Weakness, the name and the signature film are one plain sentence each.
  $('why-weakness').textContent = weaknessLine(card.weakness);
  $('why-name').textContent = nameLine(a.name.why || card.why || '');
  $('why-film').textContent = signatureLine(card.signature);
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
  $('adv-result').hidden = false;
  $('downloads').hidden = false;
}

// Tap a grade. Over the card on the page (never in the card's own SVG or PNG) each stat has one focusable target, a button laid over its grade
// letter and its axis label (the union of the box of the card's own data-part=grade group and of the label path that follows it). A tap, a hover or Enter opens one small popover
// with that stat's sentence from Why these grades and its percentile; Escape or a tap outside closes it. One popover at a time.
let info = null; // [{ name, letter, sentence, pct }] of the card on show, or null (the sample, an opened link)
let popFor = -1;
let pinned = false; // opened by a tap or Enter (stays) and not by a hover (goes with the pointer)
const topPct = (p) => (p == null ? null : Math.max(1, Math.round((1 - p) * 100)));
function gradeBoxes(svg) {
  const host = document.createElement('div');
  host.style.cssText = 'position:absolute;left:-9999px;top:0;width:1080px;height:1350px;visibility:hidden;pointer-events:none';
  host.innerHTML = svg; // measured in place, then dropped
  document.body.appendChild(host);
  // each grade group is followed in the SVG by its axis label (a path): the target is the box of both
  const boxes = [...host.querySelectorAll('[data-part=grade]')].sort((a, b) => a.dataset.i - b.dataset.i).map((e) => {
    const a = e.getBBox(), l = e.nextElementSibling && e.nextElementSibling.tagName === 'path' ? e.nextElementSibling.getBBox() : a;
    const x = Math.min(a.x, l.x), y = Math.min(a.y, l.y);
    return [x, y, Math.max(a.x + a.width, l.x + l.width) - x, Math.max(a.y + a.height, l.y + l.height) - y];
  });
  host.remove();
  return boxes;
}
const hotButtons = () => [...$('hots').children];
function closePop() {
  if (popFor >= 0 && hotButtons()[popFor]) hotButtons()[popFor].setAttribute('aria-expanded', 'false');
  $('statpop').hidden = true;
  popFor = -1;
  pinned = false;
}
function openPop(i, pin) {
  closePop(); // only one at a time
  const s = info && info[i];
  const btn = hotButtons()[i];
  if (!s || !btn) return;
  const pop = $('statpop');
  pop.querySelector('.pop-head').textContent = `${s.name} ${s.letter}`;
  pop.querySelector('.pop-why').textContent = s.sentence;
  pop.querySelector('.pop-top').textContent = s.pct == null ? '' : `Top ${s.pct}%`;
  pop.querySelector('.pop-top').hidden = s.pct == null;
  pop.hidden = false;
  btn.setAttribute('aria-expanded', 'true');
  popFor = i;
  pinned = pin;
  // under the target where there is room, else over it, and inside the card's width
  const w = $('cardwrap').getBoundingClientRect(), b = btn.getBoundingClientRect(), pw = pop.offsetWidth, ph = pop.offsetHeight;
  const below = b.bottom - w.top + 6;
  pop.style.left = `${Math.max(0, Math.min(w.width - pw, b.left - w.left + b.width / 2 - pw / 2))}px`;
  pop.style.top = `${below + ph <= w.height ? below : Math.max(0, b.top - w.top - ph - 6)}px`;
}
function buildHots(list) {
  closePop();
  info = list;
  $('hots').replaceChildren();
  if (!list || !last || !last.svgs) return;
  last.boxes = last.boxes || gradeBoxes(last.svgs.light);
  list.forEach((s, i) => {
    const [x, y, w, h] = last.boxes[i];
    const b = document.createElement('button');
    b.type = 'button';
    b.className = 'hot';
    b.setAttribute('aria-haspopup', 'dialog');
    b.setAttribute('aria-expanded', 'false');
    b.setAttribute('aria-controls', 'statpop');
    b.setAttribute('aria-label', `${s.name}, grade ${s.letter}: why`);
    // centred on the group's box, at least 44 px each way; the box is in card units (1080 x 1350)
    b.style.cssText = `left:${((x + w / 2) / 10.8).toFixed(2)}%;top:${((y + h / 2) / 13.5).toFixed(2)}%;width:max(44px,${((w + 12) / 10.8).toFixed(2)}%);height:max(44px,${((h + 12) / 13.5).toFixed(2)}%)`;
    b.addEventListener('click', () => (popFor === i && pinned ? closePop() : openPop(i, true)));
    b.addEventListener('pointerenter', (e) => { if (e.pointerType === 'mouse' && !pinned) openPop(i, false); });
    b.addEventListener('pointerleave', (e) => { if (e.pointerType === 'mouse' && !pinned && popFor === i) closePop(); });
    $('hots').appendChild(b);
  });
  $('hots').hidden = !!live; // they come back when the reveal ends
}
addEventListener('keydown', (e) => { if (e.key === 'Escape' && popFor >= 0) closePop(); });
addEventListener('pointerdown', (e) => { if (popFor >= 0 && !e.target.closest('#statpop, #hots')) closePop(); }, true);

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

function themeChanged() {
  endReveal();
  if (!last) return showSample();
  if (last.analysis) show(last.analysis, last.svgs[resolvedTheme()]);
  else showCard(last.svgs[resolvedTheme()], 'Shared card', `Shared Stand card for ${last.card.user}`);
  if (document.documentElement.dataset.accent) setAccent(last.svgs, last.card.accent);
}
const png = () => pngBlob(last.svgs[resolvedTheme()]);
$('dl-png').addEventListener('click', async () => {
  if (!last) return;
  try { download('card.png', await png()); } catch (e) { setStatus(e.message, true); }
});

const SHARE_TEXT = 'My JoJo Stand, from my Letterboxd. Find yours: https://standuser.pages.dev';
// Copy image and Share need the clipboard and the Web Share API; each button stays hidden where its API is missing
// A toast (#toast, an aria-live status region) says what a button did, for 2 s: "Image copied", "Link copied", or the error.
const TOAST_MS = 2000;
let toastTimer = null;
function toast(t) {
  const el = $('toast');
  clearTimeout(toastTimer);
  el.classList.toggle('show', !!t);
  el.textContent = t;
  if (t) toastTimer = setTimeout(() => { el.classList.remove('show'); el.textContent = ''; }, TOAST_MS);
}
const note = toast;
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
    note('Image copied');
  } catch (e) { note(`Could not copy the image: ${e.message}`); }
});
// Copy link writes the card's data in the fragment, which the browser never sends anywhere
const shareLink = () => `${location.origin}${location.pathname}#c=${share_encode(JSON.stringify(last.card))}`;
$('copy-link').addEventListener('click', async () => {
  if (!last) return;
  const url = shareLink();
  try {
    await navigator.clipboard.writeText(url);
    note('Link copied');
  } catch { note('Could not copy the link'); }
});
$('share-btn').addEventListener('click', async () => {
  if (!last) return;
  try {
    const file = new File([await png()], 'stand-card.png', { type: 'image/png' });
    // the text carries the site's fixed address (not location.origin, and not the #c= card link), in text, not url: many share targets drop url when files are present
    const data = { title: 'My Stand card', text: SHARE_TEXT };
    // where the API refuses a file together with text, the text goes without the file
    await navigator.share(navigator.canShare({ ...data, files: [file] }) ? { ...data, files: [file] } : data);
  } catch (e) { if (e.name !== 'AbortError') note(`Could not share: ${e.message}`); }
});

$('change').addEventListener('click', () => $('file').click());
$('pick').addEventListener('click', (e) => { e.stopPropagation(); $('file').click(); });
drop.addEventListener('click', () => $('file').click());
drop.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); $('file').click(); } });
$('file').addEventListener('change', (e) => {
  const f = e.target.files[0];
  e.target.value = ''; // Chrome fires no change event for the same file while the input holds it; the File itself stays readable
  if (f) handle(f);
});
['dragenter', 'dragover'].forEach((t) => drop.addEventListener(t, (e) => { e.preventDefault(); drop.classList.add('over'); }));
['dragleave', 'drop'].forEach((t) => drop.addEventListener(t, (e) => { e.preventDefault(); drop.classList.remove('over'); }));
drop.addEventListener('drop', (e) => e.dataTransfer.files[0] && handle(e.dataTransfer.files[0]));
