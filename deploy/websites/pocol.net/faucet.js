(() => {
  'use strict';
  // The browser calls the fee faucet on the explorer host itself, so the faucet's per-client limit counts each visitor's
  // IP address, not this site's server. The faucet pins its own network; this page checks no chain ID.
  const EXPLORER = 'https://explorer.pocol.net';
  const STATUS_URL = 'https://explorer.pocol.net/faucet/status';
  const CLAIM_URL = 'https://explorer.pocol.net/faucet/claim';
  const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
  const HASH = /^0x[0-9a-fA-F]{64}$/;
  const UNIT = 10n ** 18n;
  const MAX_UINT = (1n << 256n) - 1n;
  const STATUS_TIMEOUT_MS = 15000;
  const CLAIM_TIMEOUT_MS = 60000;
  const MAX_BODY = 16384;
  const CODES = ['invalid_address', 'reserved_address', 'contract_account', 'bad_request', 'too_large',
    'unsupported_media', 'not_found', 'deadline', 'already_claimed', 'not_new_account', 'already_funded', 'busy',
    'daily_limit', 'client_limit', 'signer_failed', 'node_refused', 'node_unavailable', 'unknown_outcome',
    'wrong_network', 'over_float', 'empty'];
  // Outcomes after which a transfer may exist: the page points to the explorer before any retry.
  const CHECK_FIRST = new Set(['unknown_outcome', 'already_claimed', 'unreachableClaim', 'fallbackUnavailable']);
  const translations = {
    "en": {
      "title": "Get your first fee · PoCol",
      "description": "The PoCol fee faucet sends 0.002 PCOL, once, to a new address so that its first registration can pay its transaction fee.",
      "eyebrow": "FEE FAUCET",
      "heading": "Get your first fee",
      "lead": "A new miner can register with a 0 PCOL bond, but that first registration is still a transaction and pays a fee. The faucet sends a new address enough PCOL for that fee, and nothing more.",
      "amountLabel": "Sent to each new address",
      "remainingLabel": "Transfers left today (UTC)",
      "statusLoading": "Reading the faucet status…",
      "statusReady": "As reported by the faucet.",
      "statusUnreachable": "The faucet could not be reached. You can still try a claim, or come back later.",
      "statusUnavailable": "The faucet status is unavailable right now. You can still try a claim, or come back later.",
      "statusSpent": "Today's transfers have all been sent. Come back after midnight UTC.",
      "addressLabel": "Your PoCol address",
      "addressHint": "Paste the address of your new account: 0x followed by 40 hexadecimal characters. Never enter a private key or a recovery phrase.",
      "button": "Send me the fee",
      "buttonPending": "Sending…",
      "pending": "Your claim is on its way to the faucet. This can take up to a minute; keep this page open.",
      "successTitle": "The faucet sent your fee.",
      "successBody": "The transfer is in the network's queue. It appears in the explorer once a block includes it.",
      "hashLabel": "Transaction hash",
      "toLabel": "Address",
      "copy": "Copy",
      "copied": "Copied",
      "copyFailed": "Copying is unavailable; select the text instead",
      "viewTransaction": "View the transaction in the explorer ↗",
      "viewAddress": "Check this address in the explorer ↗",
      "rulesHeading": "How the faucet works",
      "ruleAmount": "0.002 PCOL, once per address, to pay the fee of the first registration.",
      "ruleNew": "New accounts only: the address must not have sent a transaction, must not be a contract and must not already hold that much PCOL.",
      "ruleLimits": "Daily limits apply: a total number of transfers each UTC day for everyone, and a smaller number per network address (IP), shared by everyone who uses that address.",
      "ruleOthers": "The faucet is not the only way: any PCOL holder can send you the fee with an ordinary transfer.",
      "rulePrivacy": "This page sends only your address to explorer.pocol.net, which runs the faucet.",
      "fallbackAddress": "The faucet refused this address. Check the address and try again.",
      "fallbackLimit": "The faucet is busy or a limit has been reached. Try again later.",
      "fallbackUnavailable": "The faucet is unavailable right now. Check the address in the explorer before trying again later.",
      "fallbackRefused": "The faucet refused the request. Refresh the page and try again.",
      "unreachableClaim": "The faucet could not be reached or did not answer in time. Check the address in the explorer before trying again.",
      "errors": {
        "invalid_address": "Enter the full address: 0x followed by 40 hexadecimal characters.",
        "reserved_address": "This address cannot receive faucet transfers.",
        "contract_account": "The faucet funds only ordinary accounts, not contracts.",
        "bad_request": "The faucet could not read this request. Refresh the page and try again.",
        "too_large": "The request was too large. Refresh the page and try again.",
        "unsupported_media": "The faucet did not accept the request format. Refresh the page and try again.",
        "not_found": "The faucet service was not found on the server. Try again later.",
        "deadline": "The request took too long to arrive and no claim was processed. Try again.",
        "already_claimed": "The faucet has already handled a claim for this address. Check the address in the explorer.",
        "not_new_account": "The faucet funds only new accounts, and this address has already sent a transaction.",
        "already_funded": "This address already holds enough PCOL for its first fee.",
        "busy": "The faucet is handling another claim. Wait a few seconds and try again.",
        "daily_limit": "Today's faucet limit has been reached. Try again after midnight UTC.",
        "client_limit": "Your network address (IP) has reached its daily faucet limit, which everyone using that address shares. Try again after midnight UTC.",
        "signer_failed": "The faucet could not sign the transfer. Try again later.",
        "node_refused": "The network node refused the request, and nothing was sent. Try again later.",
        "node_unavailable": "The faucet could not reach its network node, and nothing was sent. Try again later.",
        "unknown_outcome": "A transfer may already have been sent. Check the address in the explorer before trying again.",
        "wrong_network": "The faucet's node reports a different network, so the faucet is not sending transfers. Try again later.",
        "over_float": "The faucet is stopped until its operator checks its account. Try again later.",
        "empty": "The faucet has run out of PCOL. Try again later, or ask any PCOL holder to send you the fee."
      }
    },
    "ar": {
      "title": "احصل على رسومك الأولى · PoCol",
      "description": "يرسل صنبور رسوم PoCol مبلغ 0.002 PCOL مرة واحدة إلى كل عنوان جديد، ليتمكّن تسجيله الأول من دفع رسوم المعاملة.",
      "eyebrow": "صنبور الرسوم",
      "heading": "احصل على رسومك الأولى",
      "lead": "يستطيع المعدّن الجديد التسجيل بضمان قدره 0 PCOL، لكن تسجيله الأول يبقى معاملة تُدفع عنها رسوم. يرسل الصنبور إلى العنوان الجديد ما يكفي من PCOL لتلك الرسوم فقط، لا أكثر.",
      "amountLabel": "المبلغ المرسل إلى كل عنوان جديد",
      "remainingLabel": "التحويلات المتبقية اليوم (UTC)",
      "statusLoading": "جارٍ قراءة حالة الصنبور…",
      "statusReady": "وفق ما يُبلغ عنه الصنبور.",
      "statusUnreachable": "تعذّر الوصول إلى الصنبور. يمكنك تجربة الطلب رغم ذلك، أو العودة لاحقًا.",
      "statusUnavailable": "حالة الصنبور غير متاحة حاليًا. يمكنك تجربة الطلب رغم ذلك، أو العودة لاحقًا.",
      "statusSpent": "أُرسلت جميع تحويلات اليوم. عُد بعد منتصف الليل بالتوقيت العالمي المنسَّق (UTC).",
      "addressLabel": "عنوانك في PoCol",
      "addressHint": "الصق عنوان حسابك الجديد: ‎0x‎ متبوعًا بـ 40 خانة ست عشرية. لا تُدخل أبدًا مفتاحًا خاصًا أو عبارة استرداد.",
      "button": "أرسل إليّ الرسوم",
      "buttonPending": "جارٍ الإرسال…",
      "pending": "طلبك في طريقه إلى الصنبور. قد يستغرق ذلك ما يصل إلى دقيقة؛ أبقِ هذه الصفحة مفتوحة.",
      "successTitle": "أرسل الصنبور رسومك.",
      "successBody": "التحويل في قائمة انتظار الشبكة، ويظهر في المستكشف بعد إدراجه في كتلة.",
      "hashLabel": "معرّف المعاملة (Hash)",
      "toLabel": "العنوان",
      "copy": "نسخ",
      "copied": "نُسخ",
      "copyFailed": "النسخ غير متاح؛ حدّد النص بدلًا من ذلك",
      "viewTransaction": "اعرض المعاملة في المستكشف ↗",
      "viewAddress": "تحقّق من هذا العنوان في المستكشف ↗",
      "rulesHeading": "كيف يعمل الصنبور",
      "ruleAmount": "‏0.002 PCOL مرة واحدة لكل عنوان، لدفع رسوم التسجيل الأول.",
      "ruleNew": "للحسابات الجديدة فقط: يجب ألّا يكون العنوان قد أرسل أي معاملة، وألّا يكون عقدًا ذكيًا، وألّا يملك هذا المبلغ من PCOL مسبقًا.",
      "ruleLimits": "تنطبق حدود يومية: عدد إجمالي من التحويلات لكل يوم بالتوقيت العالمي للجميع، وعدد أقل لكل عنوان شبكة (IP) يتقاسمه كل من يستخدم هذا العنوان.",
      "ruleOthers": "الصنبور ليس الطريقة الوحيدة: يستطيع أي حامل لـ PCOL أن يرسل إليك الرسوم بتحويل عادي.",
      "rulePrivacy": "لا ترسل هذه الصفحة سوى عنوانك إلى explorer.pocol.net الذي يشغّل الصنبور.",
      "fallbackAddress": "رفض الصنبور هذا العنوان. تحقّق من العنوان وحاول مرة أخرى.",
      "fallbackLimit": "الصنبور مشغول أو بلغ أحد حدوده. حاول لاحقًا.",
      "fallbackUnavailable": "الصنبور غير متاح حاليًا. تحقّق من العنوان في المستكشف قبل أن تحاول مجددًا لاحقًا.",
      "fallbackRefused": "رفض الصنبور الطلب. حدّث الصفحة وحاول مرة أخرى.",
      "unreachableClaim": "تعذّر الوصول إلى الصنبور أو لم يصل منه رد في الوقت المحدد. تحقّق من العنوان في المستكشف قبل إعادة المحاولة.",
      "errors": {
        "invalid_address": "أدخل العنوان كاملًا: ‎0x‎ متبوعًا بـ 40 خانة ست عشرية.",
        "reserved_address": "لا يمكن لهذا العنوان استلام تحويلات من الصنبور.",
        "contract_account": "يموّل الصنبور الحسابات العادية فقط، لا العقود الذكية.",
        "bad_request": "تعذّر على الصنبور قراءة هذا الطلب. حدّث الصفحة وحاول مرة أخرى.",
        "too_large": "حجم الطلب أكبر من المسموح. حدّث الصفحة وحاول مرة أخرى.",
        "unsupported_media": "لم يقبل الصنبور صيغة الطلب. حدّث الصفحة وحاول مرة أخرى.",
        "not_found": "لم يُعثر على خدمة الصنبور في الخادم. حاول لاحقًا.",
        "deadline": "استغرق وصول الطلب وقتًا طويلًا ولم يُعالَج أي طلب. حاول مرة أخرى.",
        "already_claimed": "سبق أن عالج الصنبور طلبًا لهذا العنوان. تحقّق من العنوان في المستكشف.",
        "not_new_account": "يموّل الصنبور الحسابات الجديدة فقط، وقد أرسل هذا العنوان معاملة من قبل.",
        "already_funded": "يملك هذا العنوان بالفعل ما يكفي من PCOL لرسومه الأولى.",
        "busy": "يعالج الصنبور طلبًا آخر. انتظر بضع ثوانٍ ثم حاول مرة أخرى.",
        "daily_limit": "بلغ الصنبور حدّه اليومي. حاول مجددًا بعد منتصف الليل بالتوقيت العالمي المنسَّق (UTC).",
        "client_limit": "بلغ عنوان الشبكة (IP) الذي تتصل منه الحدّ اليومي للصنبور، ويتقاسم هذا الحدّ كل من يستخدم العنوان نفسه. حاول مجددًا بعد منتصف الليل بالتوقيت العالمي المنسَّق (UTC).",
        "signer_failed": "تعذّر على الصنبور توقيع التحويل. حاول لاحقًا.",
        "node_refused": "رفضت عقدة الشبكة الطلب، ولم يُرسَل أي تحويل. حاول لاحقًا.",
        "node_unavailable": "تعذّر على الصنبور الوصول إلى عقدة الشبكة، ولم يُرسَل أي تحويل. حاول لاحقًا.",
        "unknown_outcome": "ربما أُرسل التحويل بالفعل؛ تحقّق من العنوان في المستكشف قبل إعادة المحاولة.",
        "wrong_network": "تُبلغ عقدة الصنبور عن شبكة مختلفة، لذا لا يرسل الصنبور أي تحويلات حاليًا. حاول لاحقًا.",
        "over_float": "الصنبور متوقف إلى أن يراجع مشغّله حسابه. حاول لاحقًا.",
        "empty": "نفد رصيد الصنبور من PCOL. حاول لاحقًا، أو اطلب من أي حامل لـ PCOL أن يرسل إليك الرسوم."
      }
    }
  };

  const ids = ['eyebrow', 'heading', 'lead', 'amount-label', 'remaining-label', 'amount', 'remaining', 'status',
    'form', 'address-label', 'address', 'address-hint', 'submit', 'message', 'result', 'result-title', 'result-body',
    'hash-label', 'hash', 'hash-copy', 'to-label', 'to', 'to-copy', 'link', 'rules-heading', 'rule-amount', 'rule-new',
    'rule-limits', 'rule-others', 'rule-privacy'];
  const fields = Object.fromEntries(ids.map(id => [id, document.getElementById(`faucet-${id}`)]));
  if (ids.some(id => !fields[id])) return;
  const description = document.querySelector('meta[name="description"]');
  const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
  // status: loading | ready | unreachable | unavailable; outcome: null | {key, address} | {sent, hash, address}
  let status = 'loading', amount = null, remaining = null, pending = false, outcome = null, copied = null;
  let statusRequest = 0;

  const language = () => (document.documentElement.lang === 'ar' ? 'ar' : 'en');
  function formatter(options) {
    try { return new Intl.NumberFormat(language(), options); }
    catch { return new Intl.NumberFormat('en', options); }
  }
  function coins(value) {
    const whole = formatter({ maximumFractionDigits: 0 }).format(value / UNIT);
    const fraction = (value % UNIT).toString().padStart(18, '0').replace(/0+$/, '');
    if (!fraction) return whole;
    const decimal = formatter({ useGrouping: false }).formatToParts(1.1).find(part => part.type === 'decimal').value;
    const digits = formatter({ useGrouping: false, maximumFractionDigits: 0 });
    return whole + decimal + [...fraction].map(digit => digits.format(Number(digit))).join('');
  }
  const explorerLink = value => `${EXPLORER}/#page=Transactions&q=${encodeURIComponent(value)}`;

  function render() {
    const text = translations[language()];
    document.title = text.title;
    if (description) description.content = text.description;
    const labels = { eyebrow: 'eyebrow', heading: 'heading', lead: 'lead', 'amount-label': 'amountLabel',
      'remaining-label': 'remainingLabel', 'address-label': 'addressLabel', 'address-hint': 'addressHint',
      'hash-label': 'hashLabel', 'to-label': 'toLabel', 'rules-heading': 'rulesHeading', 'rule-amount': 'ruleAmount',
      'rule-new': 'ruleNew', 'rule-limits': 'ruleLimits', 'rule-others': 'ruleOthers', 'rule-privacy': 'rulePrivacy' };
    for (const [id, key] of Object.entries(labels)) fields[id].textContent = text[key];
    fields.amount.textContent = amount === null ? '—' : `${coins(amount)} PCOL`;
    fields.remaining.textContent = remaining === null ? '—' : formatter({ maximumFractionDigits: 0 }).format(remaining);
    const spent = status === 'ready' && remaining === 0;
    fields.status.dataset.state = spent ? 'spent' : status;
    fields.status.textContent = spent ? text.statusSpent : text[`status${status[0].toUpperCase()}${status.slice(1)}`];
    fields.submit.disabled = pending || spent;
    fields.submit.textContent = pending ? text.buttonPending : text.button;
    fields.form.setAttribute('aria-busy', String(pending));
    fields.address.readOnly = pending;

    const sent = Boolean(outcome?.sent);
    fields.message.hidden = !pending && (!outcome || sent);
    fields.message.dataset.tone = pending ? 'pending' : outcome && CHECK_FIRST.has(outcome.key) ? 'warning' : 'error';
    if (pending) fields.message.textContent = text.pending;
    else if (outcome && !sent) fields.message.textContent = text.errors[outcome.key] ?? text[outcome.key];
    else fields.message.textContent = '';
    fields.result.hidden = !sent;
    if (sent) {
      fields['result-title'].textContent = text.successTitle;
      fields['result-body'].textContent = text.successBody;
      fields.hash.textContent = outcome.hash;
      fields.to.textContent = outcome.address;
    }
    for (const name of ['hash', 'to']) {
      const button = fields[`${name}-copy`];
      button.textContent = copied?.field === name ? (copied.ok ? text.copied : text.copyFailed) : text.copy;
    }
    // The explorer link: the transaction after a transfer, the address when a transfer may exist.
    const linkTarget = pending || !outcome ? null : sent ? outcome.hash
      : CHECK_FIRST.has(outcome.key) && outcome.address ? outcome.address : null;
    fields.link.hidden = !linkTarget;
    if (linkTarget) {
      fields.link.href = explorerLink(linkTarget);
      fields.link.textContent = sent ? text.viewTransaction : text.viewAddress;
    } else fields.link.removeAttribute('href');
  }

  async function request(url, options, timeoutMs) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), timeoutMs);
    try {
      const response = await fetch(url, { ...options, cache: 'no-store', credentials: 'omit', redirect: 'error',
        signal: controller.signal });
      const body = await readBody(response);
      let value = null;
      if (body !== null) {
        try { value = JSON.parse(body); } catch {}
      }
      return { status: response.status, ok: response.ok, value: object(value) ? value : null };
    } finally { clearTimeout(timeout); }
  }

  // The answer's text, or null once it is over MAX_BODY bytes: the rest is not downloaded, and the answer counts as
  // unreadable (an ok claim then reads as unknown_outcome, an error falls back on its status).
  async function readBody(response) {
    if (!response.body) return '';
    if (Number(response.headers.get('Content-Length')) > MAX_BODY) {
      await response.body.cancel().catch(() => {});
      return null;
    }
    const reader = response.body.getReader(), chunks = [];
    let size = 0;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > MAX_BODY) {
        await reader.cancel().catch(() => {});
        return null;
      }
      chunks.push(value);
    }
    const bytes = new Uint8Array(size);
    let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
    return new TextDecoder().decode(bytes);
  }

  async function loadStatus() {
    const id = ++statusRequest;
    let next = 'unavailable', nextAmount = null, nextRemaining = null;
    try {
      const answer = await request(STATUS_URL, { method: 'GET', headers: { Accept: 'application/json' } }, STATUS_TIMEOUT_MS);
      const value = answer.value;
      if (answer.ok && value && value.type === 'status' && typeof value.amountWei === 'string'
          && /^(?:0|[1-9]\d{0,77})$/.test(value.amountWei) && BigInt(value.amountWei) <= MAX_UINT
          && Number.isSafeInteger(value.remainingToday) && value.remainingToday >= 0) {
        next = 'ready'; nextAmount = BigInt(value.amountWei); nextRemaining = value.remainingToday;
      }
    } catch { next = 'unreachable'; }
    if (id !== statusRequest) return;
    status = next; amount = nextAmount; remaining = nextRemaining;
    render();
  }

  function fallback(code) {
    if (code === 400 || code === 409) return 'fallbackAddress';
    if (code === 429) return 'fallbackLimit';
    if (code >= 500) return 'fallbackUnavailable';
    return 'fallbackRefused';
  }

  async function claim(address) {
    pending = true; outcome = null; copied = null;
    render();
    try {
      const answer = await request(CLAIM_URL, { method: 'POST', headers: { 'Content-Type': 'application/json',
        Accept: 'application/json' }, body: JSON.stringify({ address }) }, CLAIM_TIMEOUT_MS);
      const value = answer.value;
      if (answer.ok) {
        // An answer of success that cannot be read may still mean a transfer: never report it as failed.
        const valid = value && value.type === 'sent' && typeof value.hash === 'string' && HASH.test(value.hash)
          && typeof value.to === 'string' && value.to.toLowerCase() === address.toLowerCase();
        outcome = valid ? { sent: true, hash: value.hash, address } : { key: 'unknown_outcome', address };
      } else {
        const code = value && typeof value.code === 'string' && CODES.includes(value.code) ? value.code : null;
        outcome = { key: code ?? fallback(answer.status), address };
      }
    } catch { outcome = { key: 'unreachableClaim', address }; }
    finally { pending = false; render(); }
    void loadStatus();
  }

  fields.form.addEventListener('submit', event => {
    event.preventDefault();
    if (pending || fields.submit.disabled) return;
    const address = fields.address.value.trim();
    fields.address.value = address;
    if (!ADDRESS.test(address)) {
      fields.address.setAttribute('aria-invalid', 'true');
      outcome = { key: 'invalid_address', address: null }; copied = null;
      render();
      fields.address.focus();
      return;
    }
    fields.address.removeAttribute('aria-invalid');
    void claim(address);
  });
  fields.address.addEventListener('input', () => fields.address.removeAttribute('aria-invalid'));
  for (const name of ['hash', 'to']) {
    fields[`${name}-copy`].addEventListener('click', async () => {
      const value = fields[name].textContent;
      let ok = false;
      try { await navigator.clipboard.writeText(value); ok = true; } catch {}
      copied = { field: name, ok };
      render();
    });
  }
  window.addEventListener('pocol:language', render);
  render();
  void loadStatus();
})();
