(() => {
  'use strict';

  const SCHEMA = 'pocol-network-summary-v1';
  const CHAIN_ID = 1599846715;
  const GENESIS = '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca';
  const SOURCE = 'https://explorer.pocol.net';
  const POLL_MS = 15000;
  const TIMEOUT_MS = 26000;
  const FRESH_MS = 45000;
  const RETAIN_MS = 300000;
  const metricNames = ['blockHeight', 'eligibleMiners', 'averageBlockTime', 'pendingTransactions'];

  function validate(value, now) {
    if (!value || typeof value !== 'object' || Array.isArray(value)
        || value.schema !== SCHEMA || value.chainId !== CHAIN_ID
        || value.genesisHash !== GENESIS || value.source !== SOURCE
        || typeof value.stale !== 'boolean' || typeof value.fetchedAt !== 'string'
        || !Number.isSafeInteger(value.headTimestamp) || value.headTimestamp < 0) {
      throw new Error('Invalid network summary');
    }
    const fetchedAt = Date.parse(value.fetchedAt);
    if (!Number.isFinite(fetchedAt) || fetchedAt > now + 300000 || now - fetchedAt > RETAIN_MS) {
      throw new Error('Invalid network timestamp');
    }
    const metrics = value.metrics;
    if (!metrics || typeof metrics !== 'object' || Array.isArray(metrics)) throw new Error('Missing network metrics');
    for (const name of metricNames) {
      const number = metrics[name];
      if (name === 'averageBlockTime') {
        if (number !== null && (typeof number !== 'number' || !Number.isFinite(number) || number < 0)) {
          throw new Error('Invalid block interval');
        }
      } else if (!Number.isSafeInteger(number) || number < 0) {
        throw new Error('Invalid network count');
      }
    }
    // Retain only the verified fields used by the page.
    return {
      fetchedAt: new Date(fetchedAt).toISOString(), fetchedAtMs: fetchedAt,
      stale: value.stale, headTimestamp: value.headTimestamp,
      metrics: Object.fromEntries(metricNames.map(name => [name, metrics[name]]))
    };
  }

  window.createPoColNetwork = ({ getLanguage, getStrings }) => {
    const section = document.querySelector('#network');
    const status = document.querySelector('#network-status');
    const note = document.querySelector('#network-note');
    const updated = document.querySelector('#network-updated');
    const fields = Object.fromEntries(metricNames.map(name => [name, document.querySelector(`[data-network-metric="${name}"]`)]));
    if (!section || !status || !note || !updated || metricNames.some(name => !fields[name])) {
      return { refreshLanguage() {} };
    }

    let snapshot = null;
    let inFlight = false;
    let lastError = false;
    let attempted = false;

    function formatter(kind, options) {
      try { return new Intl[kind](getLanguage(), options); }
      catch { return new Intl[kind]('en', options); }
    }

    function render() {
      const strings = getStrings();
      const age = snapshot ? Math.max(0, Date.now() - snapshot.fetchedAtMs) : Infinity;
      const retained = snapshot && age <= RETAIN_MS;
      const delayed = retained && (snapshot.stale || lastError || age > FRESH_MS);
      const state = retained ? (delayed ? 'stale' : 'live') : (attempted && !lastError && inFlight ? 'loading' : 'offline');
      section.dataset.state = state;
      section.setAttribute('aria-busy', String(inFlight));
      const statusText = state === 'live' ? strings.networkLive
        : state === 'stale' ? strings.networkStale
        : state === 'loading' ? strings.t18 : strings.networkOffline;
      // Avoid repeating an identical aria-live announcement on every age check.
      if (status.textContent !== statusText) status.textContent = statusText;
      note.textContent = retained && !delayed ? strings.t23 : (state === 'loading' ? strings.t23 : strings.networkUnavailable);

      const integer = formatter('NumberFormat', { maximumFractionDigits: 0 });
      const decimal = formatter('NumberFormat', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
      for (const name of metricNames) {
        const value = retained ? snapshot.metrics[name] : null;
        fields[name].textContent = value === null ? '—' : name === 'averageBlockTime'
          ? strings.networkSeconds.replace('{value}', decimal.format(value)) : integer.format(value);
      }
      updated.hidden = !retained;
      if (retained) {
        updated.dateTime = snapshot.fetchedAt;
        const time = formatter('DateTimeFormat', { hour: '2-digit', minute: '2-digit', second: '2-digit' }).format(snapshot.fetchedAtMs);
        updated.textContent = strings.networkUpdated.replace('{time}', time);
      } else {
        updated.textContent = '';
        updated.removeAttribute('datetime');
      }
    }

    async function refresh() {
      if (document.hidden || inFlight) return;
      inFlight = true;
      attempted = true;
      render();
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), TIMEOUT_MS);
      try {
        const response = await fetch('/api/network.php', {
          method: 'GET', headers: { Accept: 'application/json' },
          cache: 'no-store', credentials: 'same-origin', signal: controller.signal
        });
        if (!response.ok) throw new Error('Network summary unavailable');
        const body = await response.text();
        if (body.length > 16384) throw new Error('Network summary too large');
        const next = validate(JSON.parse(body), Date.now());
        if (snapshot && next.fetchedAtMs < snapshot.fetchedAtMs) throw new Error('Older network summary');
        snapshot = next;
        lastError = false;
      } catch {
        lastError = true;
      } finally {
        clearTimeout(timeout);
        inFlight = false;
        render();
      }
    }

    setInterval(refresh, POLL_MS);
    setInterval(() => { if (!document.hidden) render(); }, 5000);
    document.addEventListener('visibilitychange', () => {
      if (!document.hidden) {
        render();
        void refresh();
      }
    });
    void refresh();
    return { refreshLanguage: render };
  };
})();
