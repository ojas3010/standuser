// Time to ready of the real page under Fast 4G throttling, in headless Chrome driven over the DevTools protocol.
//   node ready_time.js <chrome.exe> <url> [runs]
// Fast 4G here: 9 Mbit/s down, 1.5 Mbit/s up, 170 ms round trip (WebPageTest's 4G figures). The cache is disabled, so every run
// is a cold load. "Ready" is the moment the page's boot has finished: films.json, the wasm (and names.json, when the page loads
// it) have all been received and the status line is empty again, i.e. the drop zone is active. Prints one number per run in
// milliseconds from the start of the navigation, then the median.
const { spawn } = require('node:child_process');
const [chrome, url, runsArg] = process.argv.slice(2);
const runs = Number(runsArg || 3);
const port = 9300 + Math.floor(Math.random() * 500);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function once(i) {
  const dir = `C:\\Users\\Public\\standuser_ready_${port}_${i}`;
  const p = spawn(chrome, ['--headless=new', '--disable-gpu', '--no-first-run', `--remote-debugging-port=${port + i}`, `--user-data-dir=${dir}`, 'about:blank'], { stdio: 'ignore' });
  try {
    let target;
    for (let k = 0; k < 100 && !target; k++) {
      await sleep(100);
      try { target = (await (await fetch(`http://127.0.0.1:${port + i}/json`)).json()).find((t) => t.type === 'page'); } catch {}
    }
    if (!target) throw new Error('Chrome did not start');
    const ws = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((r, e) => { ws.onopen = r; ws.onerror = e; });
    let id = 0;
    const pending = new Map();
    ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && pending.has(d.id)) pending.get(d.id)(d); };
    const send = (method, params = {}) => new Promise((r) => { const n = ++id; pending.set(n, r); ws.send(JSON.stringify({ id: n, method, params })); });
    await send('Network.enable');
    await send('Network.setCacheDisabled', { cacheDisabled: true });
    await send('Network.emulateNetworkConditions', { offline: false, latency: 170, downloadThroughput: 9e6 / 8, uploadThroughput: 1.5e6 / 8 });
    const t0 = Date.now();
    await send('Page.navigate', { url });
    const expr = `(() => { const n = performance.getEntriesByType('resource').map(r => new URL(r.name).pathname);
      const want = ['/films.json', '/pkg/standuser_bg.wasm'].concat(window.__wantNames ? ['/names.json'] : []);
      return want.every(w => n.includes(w)) && document.getElementById('status') && document.getElementById('status').textContent === '' && !!document.getElementById('file'); })()`;
    const wantNames = await (async () => true)();
    for (;;) {
      if (Date.now() - t0 > 120000) throw new Error('timed out');
      const r = await send('Runtime.evaluate', { expression: expr.replace('window.__wantNames', process.env.WANT_NAMES === '1' ? 'true' : 'false'), returnByValue: true });
      if (r.result && r.result.result && r.result.result.value === true) break;
      await sleep(25);
    }
    const ms = Date.now() - t0;
    ws.close();
    return ms;
  } finally {
    p.kill();
  }
}
(async () => {
  const xs = [];
  for (let i = 0; i < runs; i++) xs.push(await once(i));
  xs.sort((a, b) => a - b);
  console.log(JSON.stringify({ runs: xs, median_ms: xs[Math.floor(xs.length / 2)] }));
})();
