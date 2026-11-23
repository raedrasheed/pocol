(() => {
  'use strict';
  const CHAIN_ID = 2656089850;
  const GENESIS = '0x664dc53a2fa5e9e7c94ae3d735b1eefabb8630fcbabf5e8b0d34f5db0b8cc64e';
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
      rewardAllocated: 'Total allocated to this block · before settlement', rewardDistinct: 'Distinct miners in the projected settlement',
      rewardLoading: 'Fetching block snapshot…', rewardPending: 'Pending settlement', rewardOffline: 'Unavailable',
      rewardOnce: 'Fetched once when this page loads. Reload the page for a new snapshot.',
      rewardSettlement: 'Expected settlement at block #{number}. Payments may change before settlement.',
      rewardPayout: 'Projected to miners: {paid} PCOL · Projected burn: {burned} PCOL · Carried to the next pot: {carry} PCOL.',
      rewardBond: 'Of the amount projected to miners, {bond} PCOL fills miners’ bonds.',
      rewardPot: 'This block’s pot: {issued} PCOL issued + {tips} PCOL fees − {share} PCOL proposer fee share + {carryIn} PCOL carried in.',
      rewardClosing: 'The network is closing: this pot may be settled by the final block, which burns its base burn.',
      rewardFinal: 'Final block of the network: it opens no new reward pot.', rewardFinalStatus: 'Final block',
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
      rewardAllocated: 'إجمالي المبلغ المخصص للبلوك · قبل التسوية', rewardDistinct: 'المعدّنون المختلفون في التسوية المتوقعة',
      rewardLoading: 'جارٍ جلب بيانات البلوك…', rewardPending: 'قيد التسوية', rewardOffline: 'غير متاح',
      rewardOnce: 'تُجلب مرة واحدة عند تحميل الصفحة. أعد تحميل الصفحة لجلب لقطة جديدة.',
      rewardSettlement: 'التسوية المتوقعة عند البلوك #{number}. قد تتغير المدفوعات قبل التسوية.',
      rewardPayout: 'المتوقع للمعدّنين: {paid} PCOL · الحرق المتوقع: {burned} PCOL · المُرحَّل إلى المكافأة التالية: {carry} PCOL.',
      rewardBond: 'من المبلغ المتوقع للمعدّنين يذهب {bond} PCOL إلى استكمال ضمانات المعدّنين.',
      rewardPot: 'مكافأة هذا البلوك: {issued} PCOL إصدار + {tips} PCOL رسوم − {share} PCOL حصة المقترح من الرسوم + {carryIn} PCOL مُرحَّل إليها.',
      rewardClosing: 'الشبكة في مرحلة الإغلاق: قد يسوّي البلوك الأخير هذه المكافأة، ويحرق حينها الجزء المخصص للحرق.',
      rewardFinal: 'البلوك الأخير في الشبكة: لا يفتح مكافأة جديدة.', rewardFinalStatus: 'البلوك الأخير',
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
  // The summary api/reward.php passes on from the gateway's pocol-block-reward-v3 answer (F-74), checked again here:
  // the pot is issuance + fees - the proposer's fee share + the carry in, and splits exactly into paid (bond fills
  // included), burned and carried; a final block opens no pot.
  function validate(value) {
    const flag = v => typeof v === 'boolean';
    if (!object(value) || value.schema !== 'pocol-block-reward-v3' || value.chainId !== CHAIN_ID
        || value.genesisHash !== GENESIS || value.specVersion !== 2 || ('source' in value && value.source !== SOURCE)
        || value.status !== 'pending' || !flag(value.final) || !flag(value.winnerAccruing)
        || !count(value.blockHeight) || value.blockHeight === 0
        || (value.final ? value.settlementHeight !== null
          : !count(value.settlementHeight) || value.settlementHeight <= value.blockHeight)
        || !count(value.shareCount) || value.shareCount > 256
        || !count(value.beneficiaryCount) || value.beneficiaryCount > value.shareCount + 2
        || typeof value.blockHash !== 'string' || !/^0x[\da-f]{64}$/i.test(value.blockHash)
        || typeof value.fetchedAt !== 'string' || !Array.isArray(value.beneficiaries)
        || value.beneficiaries.length !== value.beneficiaryCount || !('closing' in value)) return invalid();
    const fetchedAtMs = Date.parse(value.fetchedAt);
    if (!Number.isFinite(fetchedAtMs) || fetchedAtMs > Date.now() + 300000 || Date.now() - fetchedAtMs > 300000) return invalid();
    const [issued, tips, share, carryIn, gross, paid, burned, bond, carry] = ['issuedWei', 'tipsWei', 'tipShareWei',
      'carryInWei', 'grossRewardWei', 'projectedPaidWei', 'projectedBurnedWei', 'projectedBondWei', 'projectedCarryWei']
      .map(key => amount(value[key]));
    if (share > tips || gross !== paid + burned + carry || bond > paid) return invalid();
    if (value.final ? gross !== 0n || issued !== 0n || share !== 0n || value.beneficiaryCount !== 0
      : gross !== issued + tips - share + carryIn) return invalid();
    let closing = null;
    if (value.closing !== null) {
      const c = value.closing;
      if (!object(c) || !count(c.closeHeight) || !flag(c.cursorDone)) return invalid();
      if ('settlementMayBeFinal' in c && (!flag(c.settlementMayBeFinal) || !count(c.earliestFinalHeight))) return invalid();
      closing = { mayBeFinal: c.settlementMayBeFinal === true };
    }
    let sum = 0n, filled = 0n;
    const seen = new Set(), miners = new Set();
    for (const beneficiary of value.beneficiaries) {
      if (!object(beneficiary) || !address(beneficiary.minerId) || !address(beneficiary.rewardAddress)) return invalid();
      const id = beneficiary.minerId.toLowerCase(), pair = id + beneficiary.rewardAddress.toLowerCase();
      const payment = amount(beneficiary.amountWei);
      const fill = 'bondAccruedWei' in beneficiary ? amount(beneficiary.bondAccruedWei) : 0n;
      if (seen.has(pair) || payment + fill === 0n || ('bondAccruedWei' in beneficiary && fill === 0n)) return invalid();
      seen.add(pair); miners.add(id); sum += payment + fill; filled += fill;
    }
    if (sum !== paid || filled !== bond) return invalid();
    return { blockHeight: value.blockHeight, blockHash: value.blockHash, settlementHeight: value.settlementHeight,
      final: value.final, closing, miners: miners.size, fetchedAtMs, fetchedAt: new Date(fetchedAtMs).toISOString(),
      issued, tips, share, carryIn, gross, paid, burned, bond, carry };
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
      fields.status.textContent = state === 'loading' ? strings.rewardLoading
        : snapshot ? (snapshot.final ? strings.rewardFinalStatus : strings.rewardPending) : strings.rewardOffline;
      fields.amount.textContent = snapshot ? coins(snapshot.gross) : '—';
      fields.amount.dataset.long = String(fields.amount.textContent.length > 10);
      if (state !== 'ready') fields.breakdown.open = true;
      else if (fields.breakdown.dataset.loaded !== 'true') {
        fields.breakdown.open = false;
        fields.breakdown.dataset.loaded = 'true';
      }
      fields.miners.textContent = snapshot ? formatter('NumberFormat', { maximumFractionDigits: 0 }).format(snapshot.miners) : '—';
      fields.block.hidden = !snapshot;
      fields.captured.hidden = !snapshot;
      fields.settlement.hidden = !snapshot;
      if (snapshot) {
        const integer = formatter('NumberFormat', { maximumFractionDigits: 0 });
        fields.block.textContent = strings.rewardBlock.replace('{number}', integer.format(snapshot.blockHeight));
        fields.block.href = `${SOURCE}/#page=Blocks&q=${snapshot.blockHash}`;
        // Text only: every figure is formatted here and inserted with textContent.
        fields.payout.textContent = snapshot.final ? strings.rewardFinal : [
          strings.rewardPot.replace('{issued}', coins(snapshot.issued)).replace('{tips}', coins(snapshot.tips))
            .replace('{share}', coins(snapshot.share)).replace('{carryIn}', coins(snapshot.carryIn)),
          strings.rewardPayout.replace('{paid}', coins(snapshot.paid)).replace('{burned}', coins(snapshot.burned))
            .replace('{carry}', coins(snapshot.carry)),
          ...(snapshot.bond > 0n ? [strings.rewardBond.replace('{bond}', coins(snapshot.bond))] : [])].join(' ');
        fields.settlement.hidden = snapshot.final;
        fields.settlement.textContent = snapshot.final ? '' : strings.rewardSettlement.replace('{number}',
          integer.format(snapshot.settlementHeight)) + (snapshot.closing?.mayBeFinal ? ' ' + strings.rewardClosing : '');
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
