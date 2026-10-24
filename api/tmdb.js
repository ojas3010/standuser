// Vercel function: proxies TMDB for films that are missing from the built-in lookup table.
// POST /api/tmdb  { "films": [ { "name": "Seven Pounds", "year": 2008 }, ... ] }   (at most 20 per request)
// -> { "v": 1, "countries": {...}, "people": {...}, "films": [ [key, "", tmdb, year, runtime, lang, "US|PL", "dirIds", "composerIds", null, null, null], ... ], "missing": [ ... ] }
// The TMDB key is read from the TMDB_API_KEY environment variable (v3 API key or v4 read access token). Only titles
// and years are accepted and forwarded; nothing is stored or logged.

const TMDB = 'https://api.themoviedb.org/3';
const MAX_FILMS = 20;

function filmKey(name, year) {
  // Same normalisation as crates/standuser-core/src/lookup.rs film_key and tools/build_table.py
  return name.normalize('NFKD').replace(/[^\p{L}\p{N}]/gu, '').toLowerCase() + '|' + year;
}

function norm(s) {
  return s.normalize('NFKD').replace(/[^\p{L}\p{N}]/gu, '').toLowerCase();
}

function auth() {
  const key = (process.env.TMDB_API_KEY || '').trim();
  if (!key) return null;
  // v4 read access tokens are long JWTs; v3 keys are 32 hex characters
  return key.length > 40 ? { headers: { Authorization: `Bearer ${key}`, Accept: 'application/json' }, query: '' } : { headers: { Accept: 'application/json' }, query: `api_key=${encodeURIComponent(key)}` };
}

async function tmdb(path, params, a, attempt = 0) {
  const qs = [a.query, params].filter(Boolean).join('&');
  const res = await fetch(`${TMDB}${path}?${qs}`, { headers: a.headers });
  if (res.status === 429 && attempt < 2) {
    await new Promise((r) => setTimeout(r, 400 * (attempt + 1)));
    return tmdb(path, params, a, attempt + 1);
  }
  if (!res.ok) throw new Error(`tmdb ${res.status}`);
  return res.json();
}

async function find(film, a) {
  const q = encodeURIComponent(film.name);
  for (const p of [`primary_release_year=${film.year}`, `year=${film.year}`]) {
    const r = await tmdb('/search/movie', `query=${q}&include_adult=false&${p}`, a);
    const hits = r.results || [];
    if (hits.length) {
      const want = norm(film.name);
      const exact = hits.filter((h) => norm(h.title || '') === want || norm(h.original_title || '') === want);
      return (exact.length ? exact : hits).sort((x, y) => (y.popularity || 0) - (x.popularity || 0))[0];
    }
  }
  return null;
}

async function one(film, a, out) {
  const hit = await find(film, a);
  if (!hit) return false;
  const d = await tmdb(`/movie/${hit.id}`, 'append_to_response=credits', a);
  const crew = (d.credits && d.credits.crew) || [];
  const dirs = crew.filter((c) => c.job === 'Director');
  let comps = crew.filter((c) => c.job === 'Original Music Composer');
  if (!comps.length) comps = crew.filter((c) => c.job === 'Music');
  const countries = d.production_countries || [];
  for (const c of countries) out.countries[c.iso_3166_1] = c.name;
  for (const p of [...dirs, ...comps]) out.people[p.id] = p.name;
  const uniq = (xs) => [...new Set(xs)];
  out.films.push([
    filmKey(film.name, film.year), '', d.id, film.year, d.runtime || 0, d.original_language || '',
    countries.map((c) => c.iso_3166_1).join('|'),
    uniq(dirs.map((p) => p.id)).join(','), uniq(comps.map((p) => p.id)).join(','),
    null, null, null,
  ]);
  return true;
}

module.exports = async (req, res) => {
  res.setHeader('Content-Type', 'application/json; charset=utf-8');
  res.setHeader('Cache-Control', 'no-store');
  if (req.method !== 'POST') {
    res.statusCode = 405;
    res.setHeader('Allow', 'POST');
    return res.end(JSON.stringify({ error: 'POST only' }));
  }
  const a = auth();
  if (!a) {
    res.statusCode = 500;
    return res.end(JSON.stringify({ error: 'TMDB_API_KEY is not configured on the server' }));
  }
  let body = req.body;
  if (typeof body === 'string') {
    try { body = JSON.parse(body); } catch { body = null; }
  }
  const list = body && Array.isArray(body.films) ? body.films : null;
  if (!list || list.length === 0 || list.length > MAX_FILMS) {
    res.statusCode = 400;
    return res.end(JSON.stringify({ error: `send {"films":[{"name","year"}]} with 1 to ${MAX_FILMS} films` }));
  }
  const films = [];
  for (const f of list) {
    const year = Number.parseInt(f && f.year, 10);
    if (f && typeof f.name === 'string' && f.name.length > 0 && f.name.length <= 200 && year >= 1870 && year <= 2100) films.push({ name: f.name, year });
  }
  const out = { v: 1, countries: {}, people: {}, films: [], missing: [] };
  let next = 0;
  const worker = async () => {
    while (next < films.length) {
      const f = films[next++];
      try {
        if (!(await one(f, a, out))) out.missing.push([f.name, f.year]);
      } catch (e) {
        out.missing.push([f.name, f.year]);
      }
    }
  };
  await Promise.all(Array.from({ length: Math.min(6, films.length) }, worker));
  res.statusCode = 200;
  res.end(JSON.stringify(out));
};

module.exports.filmKey = filmKey;
