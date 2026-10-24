// standuser site: export.zip in, card out, all in the browser. Only films missing from the built-in table are looked up
// (title and year only) through /api/tmdb, and only if the box is ticked.
import init, { run } from './pkg/standuser.js';

const $ = (id) => document.getElementById(id);
const statusEl = $('status');
let table = null;
let last = null; // { svgs, analysis, name }

const setStatus = (msg, error = false) => {
  statusEl.textContent = msg;
  statusEl.classList.toggle('error', error);
};

async function boot() {
  setStatus('Loading…');
  try {
    await init();
    table = await (await fetch('films.json')).text();
    setStatus('');
  } catch (e) {
    setStatus(`Could not load the page's data: ${e.message}`, true);
  }
}
const ready = boot();

function chosenTheme(kind) {
  const t = $('theme').value;
  if (kind === 'png') return t === 'auto' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : t;
  return t;
}

function exec(bytes, name, extra) {
  const out = JSON.parse(run(bytes, name, table, extra, $('joined').value || '', $('username').value.trim(), new Date().toISOString().slice(0, 10)));
  if (out.error) throw new Error(out.error);
  return out;
}

async function lookupMissing(films) {
  // up to 600 films, 15 per request, 3 requests at a time; rows are merged into one table JSON
  const todo = films.slice(0, 600);
  const merged = { v: 1, countries: {}, people: {}, films: [] };
  const batches = [];
  for (let i = 0; i < todo.length; i += 15) batches.push(todo.slice(i, i + 15));
  let done = 0, failed = 0;
  const work = async () => {
    while (batches.length) {
      const b = batches.shift();
      try {
        const r = await fetch('/api/tmdb', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ films: b.map(([name, year]) => ({ name, year })) }) });
        if (!r.ok) throw new Error(String(r.status));
        const j = await r.json();
        Object.assign(merged.countries, j.countries);
        Object.assign(merged.people, j.people);
        merged.films.push(...j.films);
      } catch {
        failed += b.length;
      }
      done += b.length;
      setStatus(`Looking up films missing from the table… ${Math.min(done, todo.length)} of ${todo.length}`);
    }
  };
  await Promise.all([work(), work(), work()]);
  return { merged, failed };
}

async function handle(file) {
  await ready;
  if (!table) return;
  $('result').hidden = true;
  try {
    setStatus('Reading the export…');
    const bytes = new Uint8Array(await file.arrayBuffer());
    let out = exec(bytes, file.name, '');
    const unknown = out.analysis.coverage.unknown_films;
    let looked = '';
    if ($('lookup').checked && unknown.length) {
      const { merged, failed } = await lookupMissing(unknown);
      if (merged.films.length) {
        setStatus('Building the card…');
        out = exec(bytes, file.name, JSON.stringify(merged));
      }
      looked = `${merged.films.length} of ${unknown.length} missing films found on TMDB${failed ? `, ${failed} lookups failed` : ''}.`;
    }
    last = { svgs: out.svgs, analysis: out.analysis };
    show(out.analysis, out.svgs[$('theme').value]);
    setStatus(looked);
  } catch (e) {
    setStatus(e.message, true);
  }
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
    [names[i], s.letter, val, s.status + (s.note ? `: ${s.note}` : '')].forEach((t) => (tr.insertCell().textContent = t));
  });
  const notes = $('notes');
  notes.replaceChildren();
  const c = a.coverage;
  const items = [
    `${c.watched_in_table} of ${c.watched} watched films are in the lookup (${c.unknown_films.length} are not).`,
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

$('theme').addEventListener('change', () => last && show(last.analysis, last.svgs[$('theme').value]));
$('dl-svg').addEventListener('click', () => last && download('card.svg', new Blob([last.svgs.auto], { type: 'image/svg+xml' })));
$('dl-png').addEventListener('click', async () => {
  if (!last) return;
  try { download('card.png', await pngBlob(last.svgs[chosenTheme('png')])); } catch (e) { setStatus(e.message, true); }
});
$('dl-json').addEventListener('click', () => last && download('stand.json', new Blob([JSON.stringify(last.analysis, null, 2)], { type: 'application/json' })));

const drop = $('drop');
$('pick').addEventListener('click', (e) => { e.stopPropagation(); $('file').click(); });
drop.addEventListener('click', () => $('file').click());
drop.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); $('file').click(); } });
$('file').addEventListener('change', (e) => e.target.files[0] && handle(e.target.files[0]));
['dragenter', 'dragover'].forEach((t) => drop.addEventListener(t, (e) => { e.preventDefault(); drop.classList.add('over'); }));
['dragleave', 'drop'].forEach((t) => drop.addEventListener(t, (e) => { e.preventDefault(); drop.classList.remove('over'); }));
drop.addEventListener('drop', (e) => e.dataTransfer.files[0] && handle(e.dataTransfer.files[0]));
