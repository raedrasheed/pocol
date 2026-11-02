(() => {
  'use strict';
  const CHAIN_ID = 1599846715;
  const GENESIS = '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca';
  const SOURCE = 'https://explorer.pocol.net';
  const UNIT = 10n ** 18n;
  const MAX_UINT = (1n << 256n) - 1n;
  const TIMEOUT_MS = 35000;
  const translations = {
    en: {
      rewardTitle: 'Collaboration. Rewarded.', rewardSubtitle: 'A closer look at the latest block and the miners who benefit.',
      rewardDetailsLabel: 'Reward breakdown & settlement', rewardSourceLabel: 'PoCol Explorer data',
      rewardContextNote: 'A snapshot of the latest block, captured when you open this page.',
      rewardCtaLabel: 'Explore collaborative mining', rewardHeading: 'Latest block reward', rewardBlock: 'Block #{number} ↗',
      rewardAmount: 'Block reward', rewardMiners: 'Beneficiary miners',
      rewardAllocated: 'Total allocated to this block · before settlement', rewardDistinct: 'Distinct miners expected to receive a payment',
      rewardLoading: 'Fetching block snapshot…', rewardPending: 'Pending settlement', rewardOffline: 'Unavailable',
      rewardOnce: 'Fetched once when this page loads. Reload the page for a new snapshot.',
      rewardSettlement: 'Expected settlement at block #{number}. Payments may change before settlement.',
      rewardPayout: 'Expected to miners: {paid} PCOL · Expected burn: {burned} PCOL.',
      rewardUnavailable: 'Reward data is temporarily unavailable. Reload the page to try again.',
      rewardUpdate: 'The reward service is awaiting an update. Reload the page after it is ready.',
      rewardCaptured: 'Snapshot captured {time}'
    },
    ar: {
      rewardTitle: 'التعاون يصنع المكافأة.', rewardSubtitle: 'نظرة على مكافأة أحدث بلوك والمعدّنين المستفيدين منها.',
      rewardDetailsLabel: 'تفاصيل التوزيع والتسوية', rewardSourceLabel: 'بيانات مستكشف PoCol',
      rewardContextNote: 'لقطة لأحدث بلوك، تُجلب عند فتح هذه الصفحة.',
      rewardCtaLabel: 'اكتشف التعدين التعاوني', rewardHeading: 'مكافأة أحدث بلوك', rewardBlock: 'البلوك #{number} ↗',
      rewardAmount: 'مكافأة البلوك', rewardMiners: 'المعدّنون المستفيدون',
      rewardAllocated: 'إجمالي المبلغ المخصص للبلوك · قبل التسوية', rewardDistinct: 'المعدّنون المتوقع حصولهم على مكافأة',
      rewardLoading: 'جارٍ جلب بيانات البلوك…', rewardPending: 'قيد التسوية', rewardOffline: 'غير متاح',
      rewardOnce: 'تُجلب مرة واحدة عند تحميل الصفحة. أعد تحميل الصفحة لجلب لقطة جديدة.',
      rewardSettlement: 'التسوية المتوقعة عند البلوك #{number}. قد تتغير المدفوعات قبل التسوية.',
      rewardPayout: 'المتوقع للمعدّنين: {paid} PCOL · الحرق المتوقع: {burned} PCOL.',
      rewardUnavailable: 'بيانات المكافأة غير متاحة مؤقتًا. أعد تحميل الصفحة للمحاولة مجددًا.',
      rewardUpdate: 'خدمة المكافآت بانتظار التحديث. أعد تحميل الصفحة بعد تفعيلها.',
      rewardCaptured: 'وقت جلب اللقطة {time}'
    }
  };
  const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
  const count = value => Number.isSafeInteger(value) && value >= 0;
  const address = value => typeof value === 'string' && /^0x[\da-f]{40}$/i.test(value) && !/^0x0{40}$/i.test(value);
  const invalid = () => { throw new Error('Invalid reward snapshot'); };
  function amount(value) {
    if (typeof value !== 'string' || !/^(?:0|[1-9]\d{0,77})$/.test(value)) return invalid();
    const number = BigInt(value);
    if (number > MAX_UINT) return invalid();
    return number;
  }
  function validate(value) {
    if (!object(value) || value.schema !== 'pocol-block-reward-v1' || value.chainId !== CHAIN_ID
        || value.genesisHash !== GENESIS || value.source !== SOURCE || value.status !== 'pending'
        || !count(value.blockHeight) || value.blockHeight === 0 || !count(value.settlementHeight)
        || value.settlementHeight <= value.blockHeight || !count(value.shareCount)
        || !count(value.beneficiaryCount) || value.beneficiaryCount > value.shareCount + 2
        || typeof value.blockHash !== 'string' || !/^0x[\da-f]{64}$/i.test(value.blockHash)
        || typeof value.fetchedAt !== 'string' || !Array.isArray(value.beneficiaries)
        || value.beneficiaries.length !== value.beneficiaryCount || value.beneficiaries.length > 10000) return invalid();
    const fetchedAtMs = Date.parse(value.fetchedAt);
    if (!Number.isFinite(fetchedAtMs) || fetchedAtMs > Date.now() + 300000 || Date.now() - fetchedAtMs > 300000) return invalid();
    const gross = amount(value.grossRewardWei), subsidy = amount(value.subsidyWei);
    const paid = amount(value.projectedPaidWei), burned = amount(value.projectedBurnedWei);
    if (gross !== paid + burned || subsidy > gross) return invalid();
    let sum = 0n;
    const seen = new Set();
    for (const beneficiary of value.beneficiaries) {
      if (!object(beneficiary) || !address(beneficiary.minerId) || !address(beneficiary.rewardAddress)) return invalid();
      const id = beneficiary.minerId.toLowerCase(), payment = amount(beneficiary.amountWei);
      if (seen.has(id) || payment === 0n) return invalid();
      seen.add(id); sum += payment;
    }
    if (sum !== paid) return invalid();
    return { blockHeight: value.blockHeight, blockHash: value.blockHash, settlementHeight: value.settlementHeight,
      beneficiaryCount: value.beneficiaryCount, fetchedAtMs, fetchedAt: new Date(fetchedAtMs).toISOString(), gross, paid, burned };
  }

  window.createPoColReward = ({ getLanguage = () => document.documentElement.lang,
    getStrings = () => window.POCOL_LOCALES?.[getLanguage()] || {} } = {}) => {
    const section = document.querySelector('#block-reward');
    const ids = ['heading', 'block', 'status', 'amount-label', 'miners-label', 'allocated', 'distinct',
      'amount', 'miners', 'payout', 'settlement', 'once', 'captured', 'title', 'subtitle',
      'details-label', 'source-label', 'cta-label', 'context-note', 'breakdown'];
    const fields = Object.fromEntries(ids.map(id => [id, document.querySelector(`#reward-${id}`)]));
    if (!section || ids.some(id => !fields[id])) return { refreshLanguage() {} };
    let snapshot = null, state = 'loading';
    function formatter(kind, options) {
      try { return new Intl[kind](getLanguage(), options); }
      catch { return new Intl[kind]('en', options); }
    }
    function coins(value) {
      const whole = formatter('NumberFormat', { maximumFractionDigits: 0 }).format(value / UNIT);
      const fraction = (value % UNIT).toString().padStart(18, '0').replace(/0+$/, '');
      if (!fraction) return whole;
      const decimal = formatter('NumberFormat', { useGrouping: false }).formatToParts(1.1).find(part => part.type === 'decimal').value;
      const digits = formatter('NumberFormat', { useGrouping: false, maximumFractionDigits: 0 });
      return whole + decimal + [...fraction].map(digit => digits.format(Number(digit))).join('');
    }
    function render() {
      const language = getLanguage();
      const strings = { ...translations.en, ...(language === 'ar' ? translations.ar : {}), ...getStrings() };
      section.dataset.state = state;
      section.setAttribute('aria-busy', String(state === 'loading'));
      const labels = { heading: 'rewardHeading', 'amount-label': 'rewardAmount', 'miners-label': 'rewardMiners',
        allocated: 'rewardAllocated', distinct: 'rewardDistinct', once: 'rewardOnce',
        title: 'rewardTitle', subtitle: 'rewardSubtitle', 'details-label': 'rewardDetailsLabel',
        'source-label': 'rewardSourceLabel', 'cta-label': 'rewardCtaLabel', 'context-note': 'rewardContextNote' };
      for (const [id, key] of Object.entries(labels)) fields[id].textContent = strings[key];
      fields.status.textContent = state === 'loading' ? strings.rewardLoading : snapshot ? strings.rewardPending : strings.rewardOffline;
      fields.amount.textContent = snapshot ? coins(snapshot.gross) : '—';
      fields.amount.dataset.long = String(fields.amount.textContent.length > 10);
      if (state !== 'ready') fields.breakdown.open = true;
      else if (fields.breakdown.dataset.loaded !== 'true') {
        fields.breakdown.open = false;
        fields.breakdown.dataset.loaded = 'true';
      }
      fields.miners.textContent = snapshot ? formatter('NumberFormat', { maximumFractionDigits: 0 }).format(snapshot.beneficiaryCount) : '—';
      fields.block.hidden = !snapshot;
      fields.captured.hidden = !snapshot;
      fields.settlement.hidden = !snapshot;
      if (snapshot) {
        const integer = formatter('NumberFormat', { maximumFractionDigits: 0 });
        fields.block.textContent = strings.rewardBlock.replace('{number}', integer.format(snapshot.blockHeight));
        fields.block.href = `${SOURCE}/#page=Blocks&q=${snapshot.blockHash}`;
        fields.payout.textContent = strings.rewardPayout.replace('{paid}', coins(snapshot.paid)).replace('{burned}', coins(snapshot.burned));
        fields.settlement.textContent = strings.rewardSettlement.replace('{number}', integer.format(snapshot.settlementHeight));
        fields.captured.dateTime = snapshot.fetchedAt;
        const time = formatter('DateTimeFormat', { dateStyle: 'medium', timeStyle: 'medium' }).format(snapshot.fetchedAtMs);
        fields.captured.textContent = strings.rewardCaptured.replace('{time}', time);
      } else {
        fields.block.removeAttribute('href');
        fields.captured.removeAttribute('datetime');
        fields.payout.textContent = state === 'loading' ? strings.rewardLoading
          : state === 'gateway_update_required' ? strings.rewardUpdate : strings.rewardUnavailable;
      }
    }
    async function loadOnce() {
      render();
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), TIMEOUT_MS);
      try {
        const response = await fetch('/api/reward.php', { method: 'GET', headers: { Accept: 'application/json' },
          cache: 'no-store', credentials: 'same-origin', signal: controller.signal });
        const body = await response.text();
        if (body.length > 1024 * 1024) throw new Error('Reward response too large');
        const value = JSON.parse(body);
        if (!response.ok) {
          state = response.status === 503 && value.code === 'gateway_update_required' ? 'gateway_update_required' : 'offline';
          return;
        }
        snapshot = validate(value);
        state = 'ready';
      } catch { state = 'offline'; }
      finally { clearTimeout(timeout); render(); }
    }
    // A language change only reformats this captured block; it never fetches another one.
    window.addEventListener('pocol:language', render);
    void loadOnce();
    return { refreshLanguage: render };
  };
  window.createPoColReward();
})();
