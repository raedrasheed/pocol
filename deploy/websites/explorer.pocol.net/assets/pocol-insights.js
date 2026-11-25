// PoCol Explorer insights: the Holders page and the Miners map (development/explorer/MINERS-MAP-HOLDERS.md §6).
// A hand-written, dependency-free ES module beside the bundle (which has no sources, so it is edited only by anchored
// edits: tools/explorer-insights-edits.json). The bundle renders empty mount points (div#pocol-holders,
// div#pocol-miners-map, div#pocol-miners-map-compact) and exposes globalThis.PocolExplorerShared = {chainId,
// genesisHash, countries} from its own identity variables and its 240 Natural Earth country shapes; React owns the
// mount points, this module owns their children. Every answer of /api/holders.php, /api/holders-stats.php and
// /api/miner-geography.php is validated (schema, chain identity, shapes, integer strings, arithmetic) before anything
// of it is shown. Amounts are BigInt from decimal strings; percentages come from integer ratios. Nothing is
// simulated or estimated. Text is set with textContent only. The new views are English or Arabic (RTL); the rest of
// the explorer stays English.

const SCHEMA = { holders: 'pocol-holders-v1', stats: 'pocol-holders-stats-v1', geography: 'pocol-miners-geography-v2' };
const API = { holders: '/api/holders.php', stats: '/api/holders-stats.php', geography: '/api/miner-geography.php' };
const WEI = 10n ** 18n, E10 = 10n ** 10n, E12 = 10n ** 12n;
const SIZES = [25, 50, 100], ORDERS = ['balance_desc', 'balance_asc'];
const TYPES = ['all', 'wallet', 'miner', 'reward', 'contract', 'system'];
const SYSTEM_LABELS = ['MinerRegistry', 'RewardEscrow', 'ShareLedger', 'ChainCaps', 'ChunkFactory', 'PAY', 'SYSTEM'];
// Balance buckets by Total in PCOL (§5.2): [0,1), [1,10), …, [100k,1M), [1M,∞).
const BUCKETS = [0n, 1n, 10n, 100n, 1000n, 10000n, 100000n, 1000000n].map((p) => p * WEI);
const BUCKET_TICKS = ['<1', '1+', '10+', '100+', '1k+', '10k+', '100k+', '1M+'];
const LANG_KEY = 'pocol-explorer-language';
const HOLDERS_REFRESH_MS = 15000, MAP_REFRESH_MS = 30000, FETCH_TIMEOUT_MS = 12000, MAX_BODY = 4 * 1024 * 1024;
const HISTORY_MAX = 2000, QUERY = /^0x[0-9a-f]{1,40}$/;
const WORLD = Object.freeze({ x: 0, y: 0, w: 1000, h: 500 }), MIN_VIEW_W = 1000 / 16;
const CLUSTER_PX = 30, BLUE = '#1aa9e4';

// ---------------------------------------------------------------------------------------------------------------
// Texts. Every key exists in both languages (explorer_insights.test.mjs checks it). Numbers use Western digits.
const fmtInt = (n) => (typeof n === 'bigint' ? groupDigits(n.toString()) : groupDigits(String(n)));
const TEXT = {
  en: {
    language: 'Language', retry: 'Retry now', loading: 'Loading…', table: 'Table', chart: 'Chart',
    showTable: (name) => `Show ${name} as a table`, showChart: (name) => `Show ${name} as a chart`,
    dataAt: (h, t) => `Data at block #${fmtInt(h)}` + (t ? ` · updated ${t}` : ''),
    busy: (s) => `The explorer gateway is busy. Retrying in ${s} s.`,
    unreachable: 'The data service could not be reached. The page asks again automatically.',
    refused: (m) => `The request was refused: ${m}`,
    invalid: (w) => `The answer failed validation (${w}), so none of it is shown.`,
    noIdentity: 'The explorer did not expose its network identity, so no answer can be checked and nothing is shown.',
    notReady: 'The index has no published snapshot yet. The page asks again automatically.', askAgain: 'The page asks again automatically.',
    // Holders
    hTitle: 'PCOL holders', hSub: 'Every discovered account with a balance, ranked by its total (available + bonded).',
    mSupply: 'Current supply', mSupplyNote: (h) => `Minted − burned, at block #${fmtInt(h)}`,
    mHolders: 'Holders', mHoldersNew: (n) => `${fmtInt(n)} new in the last 24 h`, mHoldersNote: 'System accounts not counted',
    mCirculating: 'Circulating', mCirculatingNote: 'Available balances of non-system accounts',
    mBonded: 'Bonded', mBondedNote: (n) => `${fmtInt(n)} miner IDs with a held bond`,
    mBurned: 'Total burned', mBurnedNote: (f, p) => `Base fees ${f} + protocol ${p}`,
    mMinted: 'Minted (gross)', mMintedNote: (g, i) => `Genesis ${g} + issued ${i}`,
    mPending: 'Pending rewards (system)', mPendingNote: 'RewardEscrow balance − bonds − protocol burns',
    mTop10: 'Top 10 share', mTop10Note: 'Of current supply, system accounts not ranked',
    verifyTitle: 'Completeness check: a difference is published',
    verifyText: (h, wei, missing) => `At block #${fmtInt(h)}, ${wei} PCOL of the expected balance sum is on no discovered account${missing === null ? ' (the on-chain account count was not read at this block)' : `, and ${fmtInt(missing)} accounts on chain are not in the index`}. Such accounts received PCOL only through internal calls of contracts, which cannot be discovered over RPC. While a difference is published, the known balances are read again every minute, so one may be up to a minute old. Percentages still use the exact current supply.`,
    cTopTitle: 'Top 10 holders and the rest', cTopSub: 'Share of the current supply',
    cTop10: 'Top 10 holders', cRest: 'Everyone else, system accounts included',
    cTopN: (n) => `Top ${n}`, cGroup: 'Group', cShare: 'Share', cAmount: 'PCOL',
    cDistTitle: 'Accounts by balance', cDistSub: 'Holders per balance range in PCOL, system accounts not counted',
    cDistAxis: 'PCOL per account', cRange: 'Range (PCOL)', cAccounts: 'Accounts', cTotal: 'Total (PCOL)',
    cDistTip: (range, n) => `${range} PCOL: ${fmtInt(n)} accounts`,
    cHistTitle: 'Holders over time', cHistSub: (from) => `Hourly points since block #${fmtInt(from)}`,
    cHistSubNone: 'Hourly points from the index start',
    cHistEmptyNone: 'No history point yet. Points are recorded hourly from the index start; nothing before it is shown or estimated.',
    cHistEmptyOne: (from) => `One point so far (block #${fmtInt(from)}). The line starts with the second hourly point; nothing before the index start is shown or estimated.`,
    cBlock: 'Block', cTime: 'Time', cHolders: 'Holders', cTop10Share: 'Top 10 share',
    cHistTip: (n, h, t) => `${fmtInt(n)} holders · block #${fmtInt(h)} · ${t}`,
    search: 'Filter by address prefix (0x…)', searchAria: 'Search holders by address prefix',
    searchInvalid: 'Enter 0x followed by 1 to 40 hexadecimal digits.',
    typeAria: 'Account type',
    types: { all: 'All accounts', wallet: 'Wallets', miner: 'Miners', reward: 'Reward addresses', contract: 'Contracts', system: 'System accounts' },
    type: { wallet: 'Wallet', miner: 'Miner', reward: 'Reward address', contract: 'Contract', system: 'System' },
    order: { balance_desc: 'Largest first', balance_asc: 'Smallest first' }, orderAria: 'Change the sort order',
    colRank: 'Rank', colAddress: 'Address', colType: 'Type', colTotal: 'Total', colAvailable: 'Available', colBonded: 'Bonded',
    colShare: '% of supply', colTxs: 'Transactions', colBlocks: 'Blocks mined',
    colRewards: (from) => (from == null ? 'Mining rewards' : `Mining rewards since #${fmtInt(from)}`),
    serves: (n) => (n === 1 ? 'Reward address of 1 miner' : `Reward address of ${fmtInt(n)} miners`),
    rewardsTo: (a) => `Rewards to ${a}`,
    copy: (a) => `Copy address ${a}`, copied: 'Address copied.', copyFailed: 'The address could not be copied.',
    open: (a) => `Open account ${a}`,
    rewardsNull: 'Not exact: the index has a gap since its start', notMiner: 'Not a miner ID',
    empty: 'No holder matches', emptyHint: 'No account of this snapshot matches the search and type filter.',
    range: (a, b, t) => `${fmtInt(a)}–${fmtInt(b)} of ${fmtInt(t)}`, rows: 'Rows',
    pageOf: (p, n) => `Page ${fmtInt(p)} of ${fmtInt(n)}`,
    first: 'First page', prev: 'Previous page', next: 'Next page', last: 'Last page',
    newer: (h) => `Newer data available (block #${fmtInt(h)}).`, showNewest: 'Show newest',
    pinned: (h) => `Pages after the first stay at block #${fmtInt(h)} so they are consistent.`,
    reset: 'The holder snapshot changed. Refreshing.',
    stReady: 'Index ready', stSyncing: (a, b) => `Indexing chain history: block #${fmtInt(a)} of #${fmtInt(b)}`,
    stReconnecting: 'Reconnecting to the node; showing the last published snapshot.',
    notesTitle: 'How to read these numbers',
    notes: [
      'Holder counts, concentration and balance ranges do not count system accounts; every percentage uses the current supply as its denominator.',
      'Completeness is verified, not guaranteed: an address that only received PCOL through an internal call of a non-system contract cannot be discovered over RPC. Any difference is published above.',
      'Mining rewards are exact only from the index start; there is no exact total since genesis over RPC.',
      'No history of balances or holders exists before the index start.',
      'Transactions are top-level transactions sent or received since genesis; internal transfers are not transactions.',
    ],
    // Miners map
    mapBar: 'Country level only: no IP address, node ID or miner address is published.',
    mapTitle: 'Miners around the world', mapSub: (h) => `Registered miners by country, where an operator node reports it · block #${fmtInt(h)}`,
    layerAria: 'Map layer', layerMiners: 'Miners', layerNodes: 'Network nodes',
    mRegistered: 'Registered miners', mRegisteredNote: (a, i) => `${fmtInt(a)} active · ${fmtInt(i)} of them inactive`,
    mLocated: 'Located miners', mLocatedNote: (u) => `${fmtInt(u)} Unknown (no operator report)`,
    mConnected: 'Connected miners', mConnectedNote: 'Fresh operator report, node within 6 blocks of the index',
    mNodes: 'Network nodes', mNodesNote: (l, u) => `${fmtInt(l)} located · ${fmtInt(u)} not locatable`,
    exited: (e, p) => `${fmtInt(e)} exited · ${fmtInt(p)} punished`,
    legActive: 'Active, sharing', legInactive: (t) => `Inactive (no share for ${fmtInt(t)} blocks)`, legOther: 'Located, not active',
    legNodes: 'Network nodes', legCluster: 'Countries close together: zoom in', legScaleMiners: 'Located miners per country',
    legScaleNodes: 'Network nodes per country',
    attribution: ['Map: ', 'Natural Earth', ' · IP Geolocation by ', 'DB-IP'],
    zoomIn: 'Zoom in', zoomOut: 'Zoom out', zoomReset: 'Show the whole world',
    mapAria: 'World map. Use + and − to zoom, the arrow keys to pan and 0 to show the whole world.',
    marker: (name, n, a, i) => `${name}: ${fmtInt(n)} located miners, ${fmtInt(a)} active and sharing, ${fmtInt(i)} inactive`,
    markerNodes: (name, n) => `${name}: ${fmtInt(n)} network nodes`,
    clusterAria: (n, c) => `${fmtInt(c)} countries with ${fmtInt(n)} in total. Select to zoom in.`,
    kicker: 'COUNTRY', pickHint: 'Select a country on the map or in the table to see its miners.',
    pLocated: 'Located miners', pShare: 'Share of registered miners', pActive: 'Active, sharing', pInactive: 'Inactive',
    pConnected: 'Connected', pNodes: 'Network nodes', pBlocks: (n) => `Blocks mined (last ${fmtInt(n)} blocks)`,
    pBlocksShare: 'Share of those blocks', pRewards: 'Mining rewards since the index start',
    pNone: (name) => `No miner is located in ${name}.`, pNoShape: 'This country has no shape in the map data; it is listed in the table only.',
    clear: 'Clear selection', zoomTo: 'Zoom to this country',
    tTitle: 'Miners by country', tSub: 'Located miners per country with Unknown as its own row; percentages of all registered miners',
    tCountry: 'Country', tLocated: 'Located miners', tPct: '% of registered', tActive: 'Active', tInactive: 'Inactive',
    tConnected: 'Connected', tNodes: 'Nodes', tBlocks: (n) => `Blocks (last ${fmtInt(n)})`, tBlocksPct: '% of blocks', tRewards: 'Mining rewards',
    tUnknown: 'Unknown (no operator report)', tSelect: (name) => `Show ${name} on the map`, tEmpty: 'No country is known for any miner or node yet.',
    sTitle: 'Data sources', sObserver: 'Observer node of this explorer', sOperator: 'Operator node report',
    sFresh: 'Fresh', sStale: 'Not fresh', sNever: 'No report yet', sAge: (s) => `${fmtInt(s)} s before this answer`,
    sDb: (p, r, l) => `${p} ${r} · ${l}`, sNoDb: 'No GeoIP database is installed on the gateway, so no country can be determined.',
    sDbLabel: 'Country database', sWindow: 'Block window', sWindowValue: (a, b, n) => `#${fmtInt(a)}–#${fmtInt(b)} (${fmtInt(n)} blocks)`,
    sAnswer: 'Answer', sAnswerValue: (h, t) => `Block #${fmtInt(h)} · ${t}`, sSource: 'Source', sKind: 'Kind', sStatus: 'Status', sLast: 'Last report',
    noReporter: 'No operator node reports a miner yet, so every registered miner is Unknown. The Network nodes layer shows the nodes the explorer’s own node is connected to.',
    mapNotes: [
      'Miner countries are known only for miners whose node runs an operator reporter. Without reporters, every miner is Unknown and the map shows only the network nodes the explorer’s own node is connected to.',
      'A node’s country is the GeoIP country of its public IP address; VPN exits, NAT and hosting providers can place it elsewhere than its operator.',
      'Mining rewards are exact only from the index start; there is no exact total since genesis over RPC.',
      'Network nodes are never counted as miners. A miner reported by several servers counts once, and so does a node seen by several servers.',
    ],
    geoUnavailable: 'Miner geography is unavailable',
    compactTitle: 'Miners map', compactOpen: 'Open the Miners map',
    compactSummary: (r, l, u, n) => `${fmtInt(r)} registered miners · ${fmtInt(l)} located · ${fmtInt(u)} Unknown · ${fmtInt(n)} network nodes`,
  },
  ar: {
    language: 'اللغة', retry: 'أعد المحاولة الآن', loading: 'جارٍ التحميل…', table: 'جدول', chart: 'رسم',
    showTable: (name) => `اعرض «${name}» كجدول`, showChart: (name) => `اعرض «${name}» كرسم`,
    dataAt: (h, t) => `البيانات عند الكتلة #${fmtInt(h)}` + (t ? ` · حُدّثت ${t}` : ''),
    busy: (s) => `بوابة المستكشف مشغولة. ستُعاد المحاولة خلال ${s} ث.`,
    unreachable: 'تعذّر الوصول إلى خدمة البيانات. ستعيد الصفحة الطلب تلقائيًا.',
    refused: (m) => `رُفض الطلب: ${m}`,
    invalid: (w) => `لم يجتز الرد التحقق (${w})، لذلك لا يُعرض منه شيء.`,
    noIdentity: 'لم يُتح المستكشف هوية الشبكة، فلا يمكن التحقق من أي رد ولا يُعرض شيء.',
    notReady: 'لم ينشر الفهرس أي لقطة بعد. ستعيد الصفحة الطلب تلقائيًا.', askAgain: 'ستعيد الصفحة الطلب تلقائيًا.',
    hTitle: 'حائزو PCOL', hSub: 'كل حساب مكتشف لديه رصيد، مرتبًا حسب إجماليه (المتاح + المرهون).',
    mSupply: 'المعروض الحالي', mSupplyNote: (h) => `المسكوك − المحروق، عند الكتلة #${fmtInt(h)}`,
    mHolders: 'الحائزون', mHoldersNew: (n) => `${fmtInt(n)} جديد خلال آخر 24 ساعة`, mHoldersNote: 'لا تُحتسب حسابات النظام',
    mCirculating: 'المتداول', mCirculatingNote: 'الأرصدة المتاحة للحسابات غير النظامية',
    mBonded: 'المرهون', mBondedNote: (n) => `${fmtInt(n)} من معرّفات المعدّنين برهن محتجز`,
    mBurned: 'إجمالي المحروق', mBurnedNote: (f, p) => `رسوم أساسية ${f} + البروتوكول ${p}`,
    mMinted: 'المسكوك (الإجمالي)', mMintedNote: (g, i) => `التكوين ${g} + المُصدَر ${i}`,
    mPending: 'مكافآت معلّقة (النظام)', mPendingNote: 'رصيد RewardEscrow − الرهون − حرق البروتوكول',
    mTop10: 'حصة أكبر 10', mTop10Note: 'من المعروض الحالي، دون ترتيب حسابات النظام',
    verifyTitle: 'فحص الاكتمال: نُشر فرق',
    verifyText: (h, wei, missing) => `عند الكتلة #${fmtInt(h)}، ليس ${wei} PCOL من مجموع الأرصدة المتوقع في أي حساب مكتشف، ${missing === null ? '(لم يُقرأ عدد الحسابات على السلسلة عند هذه الكتلة)' : `و${fmtInt(missing)} من الحسابات على السلسلة غير موجودة في الفهرس`}. هذه الحسابات تلقت PCOL عبر استدعاءات داخلية لعقود فقط، ولا يمكن اكتشافها عبر RPC. وما دام الفرق منشورًا تُقرأ الأرصدة المعروفة من جديد كل دقيقة، فقد يكون عمر الرصيد المعروض حتى دقيقة. تبقى النسب محسوبة من المعروض الحالي الدقيق.`,
    cTopTitle: 'أكبر 10 حائزين والبقية', cTopSub: 'الحصة من المعروض الحالي',
    cTop10: 'أكبر 10 حائزين', cRest: 'جميع الآخرين، ومنهم حسابات النظام',
    cTopN: (n) => `أكبر ${n}`, cGroup: 'المجموعة', cShare: 'الحصة', cAmount: 'PCOL',
    cDistTitle: 'الحسابات حسب الرصيد', cDistSub: 'عدد الحائزين في كل نطاق رصيد بعملة PCOL، دون حسابات النظام',
    cDistAxis: 'PCOL لكل حساب', cRange: 'النطاق (PCOL)', cAccounts: 'الحسابات', cTotal: 'الإجمالي (PCOL)',
    cDistTip: (range, n) => `${range} PCOL: ${fmtInt(n)} حساب`,
    cHistTitle: 'عدد الحائزين عبر الزمن', cHistSub: (from) => `نقاط كل ساعة منذ الكتلة #${fmtInt(from)}`,
    cHistSubNone: 'نقاط كل ساعة منذ بدء الفهرس',
    cHistEmptyNone: 'لا توجد نقطة تاريخ بعد. تُسجَّل النقاط كل ساعة منذ بدء الفهرس؛ ولا يُعرض أو يُقدَّر شيء قبله.',
    cHistEmptyOne: (from) => `توجد نقطة واحدة حتى الآن (الكتلة #${fmtInt(from)}). يبدأ الخط مع النقطة الثانية؛ ولا يُعرض أو يُقدَّر شيء قبل بدء الفهرس.`,
    cBlock: 'الكتلة', cTime: 'الوقت', cHolders: 'الحائزون', cTop10Share: 'حصة أكبر 10',
    cHistTip: (n, h, t) => `${fmtInt(n)} حائز · الكتلة #${fmtInt(h)} · ${t}`,
    search: 'تصفية حسب بادئة العنوان (0x…)', searchAria: 'ابحث في الحائزين حسب بادئة العنوان',
    searchInvalid: 'أدخل 0x متبوعة بخانة إلى 40 خانة ست عشرية.',
    typeAria: 'نوع الحساب',
    types: { all: 'كل الحسابات', wallet: 'المحافظ', miner: 'المعدّنون', reward: 'عناوين المكافآت', contract: 'العقود', system: 'حسابات النظام' },
    type: { wallet: 'محفظة', miner: 'معدّن', reward: 'عنوان مكافآت', contract: 'عقد', system: 'نظام' },
    order: { balance_desc: 'الأكبر أولًا', balance_asc: 'الأصغر أولًا' }, orderAria: 'غيّر ترتيب الفرز',
    colRank: 'الترتيب', colAddress: 'العنوان', colType: 'النوع', colTotal: 'الإجمالي', colAvailable: 'المتاح', colBonded: 'المرهون',
    colShare: '% من المعروض', colTxs: 'المعاملات', colBlocks: 'الكتل المعدّنة',
    colRewards: (from) => (from == null ? 'مكافآت التعدين' : `مكافآت التعدين منذ #${fmtInt(from)}`),
    serves: (n) => (n === 1 ? 'عنوان مكافآت لمعدّن واحد' : `عنوان مكافآت لـ ${fmtInt(n)} معدّنين`),
    rewardsTo: (a) => `المكافآت إلى ${a}`,
    copy: (a) => `انسخ العنوان ${a}`, copied: 'نُسخ العنوان.', copyFailed: 'تعذّر نسخ العنوان.',
    open: (a) => `افتح الحساب ${a}`,
    rewardsNull: 'غير دقيق: في الفهرس فجوة منذ بدئه', notMiner: 'ليس معرّف معدّن',
    empty: 'لا يوجد حائز مطابق', emptyHint: 'لا يطابق أي حساب في هذه اللقطة البحث ونوع الحساب المختار.',
    range: (a, b, t) => `${fmtInt(a)}–${fmtInt(b)} من ${fmtInt(t)}`, rows: 'الصفوف',
    pageOf: (p, n) => `الصفحة ${fmtInt(p)} من ${fmtInt(n)}`,
    first: 'الصفحة الأولى', prev: 'الصفحة السابقة', next: 'الصفحة التالية', last: 'الصفحة الأخيرة',
    newer: (h) => `تتوفر بيانات أحدث (الكتلة #${fmtInt(h)}).`, showNewest: 'اعرض الأحدث',
    pinned: (h) => `تبقى الصفحات بعد الأولى عند الكتلة #${fmtInt(h)} لتظل متسقة.`,
    reset: 'تغيّرت لقطة الحائزين. جارٍ التحديث.',
    stReady: 'الفهرس جاهز', stSyncing: (a, b) => `فهرسة تاريخ السلسلة: الكتلة #${fmtInt(a)} من #${fmtInt(b)}`,
    stReconnecting: 'إعادة الاتصال بالعقدة؛ تُعرض آخر لقطة منشورة.',
    notesTitle: 'كيف تُقرأ هذه الأرقام',
    notes: [
      'لا تُحتسب حسابات النظام في عدد الحائزين ولا في التركّز ولا في نطاقات الأرصدة؛ وكل نسبة مقامها المعروض الحالي.',
      'الاكتمال مُتحقَّق منه وليس مضمونًا: العنوان الذي تلقى PCOL عبر استدعاء داخلي لعقد غير نظامي فقط لا يمكن اكتشافه عبر RPC. ويُنشر أي فرق أعلاه.',
      'مكافآت التعدين دقيقة فقط منذ بدء الفهرس؛ ولا يوجد إجمالي دقيق منذ التكوين عبر RPC.',
      'لا يوجد تاريخ للأرصدة أو للحائزين قبل بدء الفهرس.',
      'المعاملات هي المعاملات من المستوى الأعلى المرسلة أو المستلمة منذ التكوين؛ والتحويلات الداخلية ليست معاملات.',
    ],
    mapBar: 'على مستوى الدولة فقط: لا يُنشر أي عنوان IP أو معرّف عقدة أو عنوان معدّن.',
    mapTitle: 'المعدّنون حول العالم', mapSub: (h) => `المعدّنون المسجّلون حسب الدولة، حيث تبلّغ عنها عقدة مشغّل · الكتلة #${fmtInt(h)}`,
    layerAria: 'طبقة الخريطة', layerMiners: 'المعدّنون', layerNodes: 'عقد الشبكة',
    mRegistered: 'المعدّنون المسجّلون', mRegisteredNote: (a, i) => `${fmtInt(a)} نشط · ${fmtInt(i)} منهم غير نشط`,
    mLocated: 'معدّنون معروفو الموقع', mLocatedNote: (u) => `${fmtInt(u)} غير معروف (لا تقرير من مشغّل)`,
    mConnected: 'معدّنون متصلون', mConnectedNote: 'تقرير مشغّل حديث، والعقدة ضمن 6 كتل من الفهرس',
    mNodes: 'عقد الشبكة', mNodesNote: (l, u) => `${fmtInt(l)} معروفة الموقع · ${fmtInt(u)} لا يمكن تحديد موقعها`,
    exited: (e, p) => `${fmtInt(e)} خرج · ${fmtInt(p)} معاقَب`,
    legActive: 'نشط ويشارك', legInactive: (t) => `غير نشط (لا مشاركة منذ ${fmtInt(t)} كتلة)`, legOther: 'معروف الموقع، غير نشط في السجل',
    legNodes: 'عقد الشبكة', legCluster: 'دول متقاربة: كبّر الخريطة', legScaleMiners: 'المعدّنون معروفو الموقع لكل دولة',
    legScaleNodes: 'عقد الشبكة لكل دولة',
    attribution: ['الخريطة: ', 'Natural Earth', ' · تحديد الموقع الجغرافي عبر IP من ', 'DB-IP'],
    zoomIn: 'تكبير', zoomOut: 'تصغير', zoomReset: 'اعرض العالم كله',
    mapAria: 'خريطة العالم. استخدم + و− للتكبير والتصغير، ومفاتيح الأسهم للتحريك، و0 لعرض العالم كله.',
    marker: (name, n, a, i) => `${name}: ${fmtInt(n)} معدّن معروف الموقع، ${fmtInt(a)} نشط ويشارك، ${fmtInt(i)} غير نشط`,
    markerNodes: (name, n) => `${name}: ${fmtInt(n)} عقدة شبكة`,
    clusterAria: (n, c) => `${fmtInt(c)} دول بإجمالي ${fmtInt(n)}. اختر للتكبير.`,
    kicker: 'الدولة', pickHint: 'اختر دولة على الخريطة أو في الجدول لعرض معدّنيها.',
    pLocated: 'معدّنون معروفو الموقع', pShare: 'الحصة من المعدّنين المسجّلين', pActive: 'نشط ويشارك', pInactive: 'غير نشط',
    pConnected: 'متصل', pNodes: 'عقد الشبكة', pBlocks: (n) => `الكتل المعدّنة (آخر ${fmtInt(n)} كتلة)`,
    pBlocksShare: 'الحصة من تلك الكتل', pRewards: 'مكافآت التعدين منذ بدء الفهرس',
    pNone: (name) => `لا يوجد معدّن معروف الموقع في ${name}.`, pNoShape: 'ليس لهذه الدولة شكل في بيانات الخريطة؛ فهي مدرجة في الجدول فقط.',
    clear: 'امسح الاختيار', zoomTo: 'كبّر إلى هذه الدولة',
    tTitle: 'المعدّنون حسب الدولة', tSub: 'المعدّنون معروفو الموقع لكل دولة، وغير المعروف في صف مستقل؛ والنسب من كل المعدّنين المسجّلين',
    tCountry: 'الدولة', tLocated: 'معروفو الموقع', tPct: '% من المسجّلين', tActive: 'نشط', tInactive: 'غير نشط',
    tConnected: 'متصل', tNodes: 'العقد', tBlocks: (n) => `الكتل (آخر ${fmtInt(n)})`, tBlocksPct: '% من الكتل', tRewards: 'مكافآت التعدين',
    tUnknown: 'غير معروف (لا تقرير من مشغّل)', tSelect: (name) => `اعرض ${name} على الخريطة`, tEmpty: 'لا تُعرف بعد دولة أي معدّن أو عقدة.',
    sTitle: 'مصادر البيانات', sObserver: 'عقدة المراقبة لهذا المستكشف', sOperator: 'تقرير عقدة مشغّل',
    sFresh: 'حديث', sStale: 'غير حديث', sNever: 'لا تقرير بعد', sAge: (s) => `قبل هذا الرد بـ ${fmtInt(s)} ث`,
    sDb: (p, r, l) => `${p} ${r} · ${l}`, sNoDb: 'لا توجد قاعدة بيانات GeoIP مثبّتة على البوابة، فلا يمكن تحديد أي دولة.',
    sDbLabel: 'قاعدة بيانات الدول', sWindow: 'نافذة الكتل', sWindowValue: (a, b, n) => `#${fmtInt(a)}–#${fmtInt(b)} (${fmtInt(n)} كتلة)`,
    sAnswer: 'الرد', sAnswerValue: (h, t) => `الكتلة #${fmtInt(h)} · ${t}`, sSource: 'المصدر', sKind: 'النوع', sStatus: 'الحالة', sLast: 'آخر تقرير',
    noReporter: 'لا تبلّغ أي عقدة مشغّل عن معدّن بعد، لذا فكل المعدّنين المسجّلين غير معروفي الموقع. تعرض طبقة «عقد الشبكة» العقد المتصلة بعقدة المستكشف نفسه.',
    mapNotes: [
      'لا تُعرف دولة المعدّن إلا إذا كانت عقدته تشغّل أداة تقارير المشغّل. ومن دونها يكون كل معدّن غير معروف الموقع، ولا تعرض الخريطة إلا عقد الشبكة المتصلة بعقدة المستكشف نفسه.',
      'دولة العقدة هي دولة عنوان IP العام لها وفق GeoIP؛ وقد تضعها شبكات VPN وNAT ومزوّدو الاستضافة في غير دولة مشغّلها.',
      'مكافآت التعدين دقيقة فقط منذ بدء الفهرس؛ ولا يوجد إجمالي دقيق منذ التكوين عبر RPC.',
      'لا تُحتسب عقد الشبكة معدّنين أبدًا. والمعدّن الذي تبلّغ عنه عدة خوادم يُحتسب مرة واحدة، وكذلك العقدة التي تراها عدة خوادم.',
    ],
    geoUnavailable: 'جغرافيا المعدّنين غير متاحة',
    compactTitle: 'خريطة المعدّنين', compactOpen: 'افتح خريطة المعدّنين',
    compactSummary: (r, l, u, n) => `${fmtInt(r)} معدّن مسجّل · ${fmtInt(l)} معروف الموقع · ${fmtInt(u)} غير معروف · ${fmtInt(n)} عقدة شبكة`,
  },
};

function tr(lang, key, ...args) {
  const v = (TEXT[lang] || TEXT.en)[key];
  return typeof v === 'function' ? v(...args) : v;
}

export function pickLanguage(stored, navigatorLanguage) {
  if (stored === 'en' || stored === 'ar') return stored;
  return typeof navigatorLanguage === 'string' && /^ar\b/i.test(navigatorLanguage) ? 'ar' : 'en';
}
function readLanguage() {
  let stored = null;
  try { stored = globalThis.localStorage?.getItem(LANG_KEY) ?? null; } catch { stored = null; }
  return pickLanguage(stored, globalThis.navigator?.language);
}
function storeLanguage(lang) {
  try { globalThis.localStorage?.setItem(LANG_KEY, lang); } catch { /* storage blocked: the choice lasts this page */ }
}

// ---------------------------------------------------------------------------------------------------------------
// Numbers: BigInt amounts in wei, percentages as percent × 10^10.
export function groupDigits(s) { return s.replace(/\B(?=(\d{3})+(?!\d))/g, ','); }

/** {text, full}: text has at most `frac` fraction digits (truncated, never rounded up; a positive amount too small for
 * them reads "<0.0…1"), full is the exact value with all 18 decimals, trailing zeros trimmed. */
export function formatPcol(wei, frac = 4) {
  if (typeof wei !== 'bigint') throw TypeError('wei must be a BigInt');
  const sign = wei < 0n ? '−' : '', v = wei < 0n ? -wei : wei;
  const int = groupDigits((v / WEI).toString()), digits = (v % WEI).toString().padStart(18, '0');
  const fullFrac = digits.replace(/0+$/, ''), shown = digits.slice(0, frac).replace(/0+$/, '');
  const full = sign + int + (fullFrac ? '.' + fullFrac : '');
  if (v > 0n && v / WEI === 0n && !shown) return { text: '<0.' + '0'.repeat(Math.max(0, frac - 1)) + '1', full };
  return { text: sign + int + (shown ? '.' + shown : ''), full };
}
/** {text, full} of a percentage given as percent × 10^10 (BigInt). */
export function formatPercentE10(e10, frac = 2) {
  if (typeof e10 !== 'bigint' || e10 < 0n) throw TypeError('a non-negative BigInt');
  const int = (e10 / E10).toString(), digits = (e10 % E10).toString().padStart(10, '0');
  const full = int + '.' + digits + '%';
  const shown = digits.slice(0, frac);
  if (e10 > 0n && e10 / E10 === 0n && /^0*$/.test(shown)) return { text: '<0.' + '0'.repeat(Math.max(0, frac - 1)) + '1%', full };
  return { text: groupDigits(int) + (frac ? '.' + shown : '') + '%', full };
}
/** floor(part × 10^12 / whole): the percentage × 10^10 of two integers (0 when whole is 0). */
export function shareE10(part, whole) {
  const p = BigInt(part), w = BigInt(whole);
  return w > 0n ? (p * E12) / w : 0n;
}
export function pageCount(total, size) { return Math.max(1, Math.ceil(total / size)); }

// ---------------------------------------------------------------------------------------------------------------
// Validation. Each validator returns the answer with BigInt amounts, or throws Invalid naming what failed.
export class Invalid extends Error {}
const fail = (what) => { throw new Invalid(what); };
const isObj = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const nat = (v, what, min = 0) => (Number.isSafeInteger(v) && v >= min ? v : fail(what));
const natOrNull = (v, what) => (v === null ? null : nat(v, what));
const dec = (v, what) => (typeof v === 'string' && /^(?:0|[1-9][0-9]{0,77})$/.test(v) ? BigInt(v) : fail(what));
const decOrNull = (v, what) => (v === null ? null : dec(v, what));
const signedDec = (v, what) => (typeof v === 'string' && /^-?(?:0|[1-9][0-9]{0,77})$/.test(v) && v !== '-0' ? BigInt(v) : fail(what));
const hash32 = (v, what) => (typeof v === 'string' && /^0x[0-9a-f]{64}$/.test(v) ? v : fail(what));
const address = (v, what) => (typeof v === 'string' && /^0x[0-9a-f]{40}$/.test(v) ? v : fail(what));
const percentE10 = (v, what) => { const x = dec(v, what); return x <= 100n * E10 ? x : fail(what); };

/** The network identity the bundle exposes (its own identity variables and country shapes), checked. */
export function readShared(value = globalThis.PocolExplorerShared) {
  if (!isObj(value) || !Number.isSafeInteger(value.chainId) || value.chainId <= 0) fail('network identity');
  if (typeof value.genesisHash !== 'string' || !/^0x[0-9a-f]{64}$/i.test(value.genesisHash)) fail('network identity');
  const countries = Array.isArray(value.countries) ? value.countries : [];
  const shapes = new Map();
  for (const c of countries) {
    if (isObj(c) && typeof c.code === 'string' && typeof c.name === 'string' && typeof c.path === 'string'
        && Number.isFinite(c.longitude) && Number.isFinite(c.latitude) && /^[ML][0-9.,LMZ -]+$/.test(c.path)) shapes.set(c.code, c);
  }
  return { chainId: value.chainId, genesisHash: value.genesisHash.toLowerCase(), countries: [...shapes.values()], shapes };
}
function identity(d, schema, shared) {
  if (!isObj(d) || d.schema !== schema) fail('schema');
  if (d.chainId !== shared.chainId) fail('chain ID');
  if (typeof d.genesisHash !== 'string' || d.genesisHash.toLowerCase() !== shared.genesisHash) fail('genesis hash');
}
function snapshotOf(s) {
  if (!isObj(s)) fail('snapshot');
  return { height: nat(s.height, 'snapshot height'), hash: hash32(s.hash, 'snapshot hash'), timestamp: nat(s.timestamp, 'snapshot time') };
}
function indexOf(i) {
  if (!isObj(i) || !['ready', 'syncing', 'reconnecting'].includes(i.state)) fail('index state');
  return { state: i.state, head: nat(i.head, 'index head', -1), historyHeight: nat(i.historyHeight, 'index height', -1), updatedAt: nat(i.updatedAt, 'index time') };
}

/** A /api/holders.php answer for `req` ({page, size, order, type, q, pin: null|{height, hash}}). */
export function validateHoldersPage(d, req, shared) {
  identity(d, SCHEMA.holders, shared);
  if (d.page !== req.page || d.size !== req.size || d.order !== req.order || d.type !== req.type || d.q !== req.q) fail('query echo');
  const total = nat(d.total, 'total');
  const snapshot = snapshotOf(d.snapshot);
  if (req.pin && (snapshot.height !== req.pin.height || snapshot.hash !== req.pin.hash)) fail('pinned snapshot');
  const index = indexOf(d.index);
  if (!isObj(d.supply)) fail('supply');
  const supply = dec(d.supply.currentSupplyWei, 'current supply');
  if (supply <= 0n) fail('current supply');
  if (!isObj(d.verification) || typeof d.verification.complete !== 'boolean') fail('verification');
  if (!Array.isArray(d.rows)) fail('rows');
  const offset = (req.page - 1) * req.size;
  if (d.rows.length !== Math.max(0, Math.min(req.size, total - offset))) fail('row count');
  const ranks = new Set(), addresses = new Set();
  let prev = null;
  const rows = d.rows.map((r, i) => {
    if (!isObj(r)) fail('row');
    const rank = nat(r.rank, 'rank', 1), addr = address(r.address, 'address');
    if (ranks.has(rank) || addresses.has(addr)) fail('duplicate row');
    ranks.add(rank); addresses.add(addr);
    if (req.type === 'all' && !req.q && rank !== (req.order === 'balance_desc' ? offset + i + 1 : total - offset - i)) fail('rank');
    if (!TYPES.includes(r.type) || r.type === 'all' || (req.type !== 'all' && r.type !== req.type)) fail('type');
    if (req.q && !addr.startsWith(req.q)) fail('search match');
    if (r.type === 'system' ? !SYSTEM_LABELS.includes(r.label) : r.label !== null) fail('label');
    if (!Array.isArray(r.minerIds) || r.minerIds.length > 4096) fail('miner IDs');
    const minerIds = r.minerIds.map((m) => address(m, 'miner ID'));
    if (new Set(minerIds).size !== minerIds.length || (r.type === 'reward' && !minerIds.length)) fail('miner IDs');
    const rewardAddress = r.rewardAddress === null ? null : address(r.rewardAddress, 'reward address');
    if (rewardAddress !== null && r.type !== 'miner') fail('reward address');
    const totalWei = dec(r.totalWei, 'total'), availableWei = dec(r.availableWei, 'available'), bondedWei = dec(r.bondedWei, 'bonded');
    if (totalWei <= 0n) fail('total');
    if (r.type === 'system' ? bondedWei !== 0n : totalWei !== availableWei + bondedWei) fail('total = available + bonded');
    if (bondedWei > 0n && r.type !== 'miner') fail('bonded');
    const share = percentE10(r.sharePercentE10, 'share');
    if (share !== shareE10(totalWei, supply)) fail('share of supply');
    const row = { rank, address: addr, type: r.type, label: r.label, minerIds, rewardAddress, totalWei, availableWei, bondedWei, share,
      transactions: nat(r.transactions, 'transactions'), blocksMined: natOrNull(r.blocksMined, 'blocks mined'), rewardsWei: decOrNull(r.rewardsWei, 'rewards') };
    if (prev && (req.order === 'balance_desc' ? (row.totalWei > prev.totalWei || row.rank < prev.rank) : (row.totalWei < prev.totalWei || row.rank > prev.rank))) fail('order');
    prev = row;
    return row;
  });
  return { page: d.page, size: d.size, order: d.order, type: d.type, q: d.q, total, snapshot, index, supply, complete: d.verification.complete, rows };
}

/** A /api/holders-stats.php answer. */
export function validateStats(d, shared) {
  identity(d, SCHEMA.stats, shared);
  const snapshot = snapshotOf(d.snapshot), index = indexOf(d.index);
  if (!isObj(d.supply)) fail('supply');
  const s = {};
  for (const k of ['genesisWei', 'issuedWei', 'mintedWei', 'baseFeeBurnedWei', 'protocolBurnedWei', 'totalBurnedWei', 'currentSupplyWei',
    'circulatingWei', 'availableWei', 'bondedWei', 'systemWei', 'pendingRewardsWei']) s[k] = dec(d.supply[k], 'supply ' + k);
  if (s.mintedWei !== s.genesisWei + s.issuedWei) fail('minted = genesis + issued');
  if (s.totalBurnedWei !== s.baseFeeBurnedWei + s.protocolBurnedWei) fail('burned = base fees + protocol');
  if (s.currentSupplyWei !== s.mintedWei - s.totalBurnedWei || s.currentSupplyWei <= 0n) fail('supply = minted − burned');
  const v = d.verification;
  if (!isObj(v) || typeof v.complete !== 'boolean') fail('verification');
  const verification = { complete: v.complete, balancesSumWei: dec(v.balancesSumWei, 'balances sum'), expectedSumWei: dec(v.expectedSumWei, 'expected sum'),
    unlocatedWei: signedDec(v.unlocatedWei, 'unlocated'), accountsIndexed: nat(v.accountsIndexed, 'accounts indexed'),
    accountsOnChain: natOrNull(v.accountsOnChain, 'accounts on chain'), accountsMissing: natOrNull(v.accountsMissing, 'accounts missing') };
  if ((verification.accountsOnChain === null) !== (verification.accountsMissing === null)) fail('verification');
  if (verification.unlocatedWei !== verification.expectedSumWei - verification.balancesSumWei) fail('verification');
  if (verification.complete && (verification.unlocatedWei !== 0n || (verification.accountsMissing !== null && verification.accountsMissing !== 0))) fail('verification');
  const ho = d.holders;
  if (!isObj(ho)) fail('holders');
  const holders = { count: nat(ho.count, 'holders'), withAvailable: nat(ho.withAvailable, 'holders'), withBonded: nat(ho.withBonded, 'holders'),
    new24h: natOrNull(ho.new24h, 'new holders'), new24hFrom: natOrNull(ho.new24hFrom, 'new holders') };
  if ((holders.new24h === null) !== (holders.new24hFrom === null) || holders.withAvailable > holders.count || holders.withBonded > holders.count
      || (holders.new24h !== null && holders.new24h > holders.count)) fail('holders');
  const concentration = {};
  let last = 0n;
  for (const n of [1, 10, 50, 100]) {
    const c = isObj(d.concentration) ? d.concentration['top' + n] : null;
    if (!isObj(c)) fail('concentration');
    const totalWei = dec(c.totalWei, 'concentration'), share = percentE10(c.sharePercentE10, 'concentration');
    if (totalWei < last || totalWei > s.currentSupplyWei || share !== shareE10(totalWei, s.currentSupplyWei)) fail('concentration');
    concentration[n] = { totalWei, share }; last = totalWei;
  }
  if (!Array.isArray(d.distribution) || d.distribution.length !== BUCKETS.length) fail('distribution');
  let accounts = 0;
  const distribution = d.distribution.map((b, i) => {
    if (!isObj(b) || dec(b.minWei, 'distribution') !== BUCKETS[i]) fail('distribution');
    const max = i + 1 < BUCKETS.length ? dec(b.maxWei, 'distribution') : b.maxWei;
    if (i + 1 < BUCKETS.length ? max !== BUCKETS[i + 1] : max !== null) fail('distribution');
    const n = nat(b.accounts, 'distribution'), totalWei = dec(b.totalWei, 'distribution');
    if ((n === 0) !== (totalWei === 0n) || (n > 0 && (totalWei < BigInt(n) * BUCKETS[i] || (max !== null && totalWei >= BigInt(n) * max)))) fail('distribution');
    accounts += n;
    return { min: BUCKETS[i], max, accounts: n, totalWei };
  });
  if (accounts !== holders.count) fail('distribution count');
  const hi = d.history;
  if (!isObj(hi) || !Array.isArray(hi.points) || hi.points.length > HISTORY_MAX) fail('history');
  let previous = -1;
  const points = hi.points.map((p) => {
    if (!isObj(p)) fail('history');
    const point = { height: nat(p.height, 'history'), timestamp: nat(p.timestamp, 'history'), holders: nat(p.holders, 'history'),
      currentSupplyWei: dec(p.currentSupplyWei, 'history'), top10: percentE10(p.top10SharePercentE10, 'history') };
    if (point.height <= previous || point.height > snapshot.height) fail('history order');
    previous = point.height;
    return point;
  });
  const from = natOrNull(hi.from, 'history start');
  if (points.length ? from !== points[0].height : from !== null) fail('history start');
  let rewards = null;
  if (d.rewards !== undefined) {
    if (!isObj(d.rewards)) fail('rewards');
    rewards = { from: natOrNull(d.rewards.from, 'rewards'), to: natOrNull(d.rewards.to, 'rewards') };
  }
  return { snapshot, index, supply: s, verification, holders, concentration, distribution, history: { from, points }, rewards };
}

/** A /api/miner-geography.php answer. */
export function validateGeography(d, shared) {
  identity(d, SCHEMA.geography, shared);
  const generatedAt = nat(d.generatedAt, 'answer time'), height = nat(d.height, 'height');
  hash32(d.blockHash, 'block hash');
  let database = null;
  if (d.database !== null) {
    if (!isObj(d.database) || d.database.provider !== 'DB-IP Lite' || d.database.license !== 'CC BY 4.0'
        || typeof d.database.release !== 'string' || !/^\d{4}-(0[1-9]|1[0-2])$/.test(d.database.release)) fail('database');
    database = { provider: d.database.provider, release: d.database.release, license: d.database.license };
  }
  const m = d.miners;
  if (!isObj(m)) fail('miners');
  const miners = {};
  for (const k of ['registered', 'active', 'inactive', 'exited', 'punished', 'located', 'unknown', 'connected', 'thetaInact']) miners[k] = nat(m[k], 'miners ' + k);
  if (miners.located + miners.unknown !== miners.registered || miners.inactive > miners.active || miners.active > miners.registered
      || miners.connected > miners.registered || miners.exited > miners.registered || miners.punished > miners.registered) fail('miner counts');
  const no = d.nodes;
  if (!isObj(no)) fail('nodes');
  const nodes = { observed: nat(no.observed, 'nodes'), located: nat(no.located, 'nodes'), unlocated: nat(no.unlocated, 'nodes') };
  if (nodes.located + nodes.unlocated !== nodes.observed) fail('node counts');
  if (!Array.isArray(d.sources) || d.sources.length > 64) fail('sources');
  const names = new Set();
  const sources = d.sources.map((x) => {
    if (!isObj(x) || !['observer', 'operator'].includes(x.kind) || typeof x.name !== 'string' || !/^[A-Za-z0-9._-]{1,64}$/.test(x.name)
        || typeof x.fresh !== 'boolean' || names.has(x.kind + ':' + x.name)) fail('source');
    names.add(x.kind + ':' + x.name);
    const reportedAt = natOrNull(x.reportedAt, 'source time');
    if (x.fresh && reportedAt === null) fail('source time');
    return { kind: x.kind, name: x.name, fresh: x.fresh, reportedAt };
  });
  const w = d.blocksWindow;
  if (!isObj(w)) fail('block window');
  const window = { from: nat(w.from, 'block window'), to: nat(w.to, 'block window'), blocks: nat(w.blocks, 'block window') };
  if (window.from > window.to || window.blocks > window.to - window.from + 1) fail('block window');
  if (!Array.isArray(d.countries) || d.countries.length > 512) fail('countries');
  const codes = new Set(), sum = { located: 0, active: 0, inactive: 0, connected: 0, nodes: 0, blocks: 0 };
  const countries = d.countries.map((c) => {
    if (!isObj(c) || typeof c.code !== 'string' || !/^[A-Z]{2}$/.test(c.code) || codes.has(c.code) || !isObj(c.miners)) fail('country');
    codes.add(c.code);
    const cm = { located: nat(c.miners.located, 'country miners'), active: nat(c.miners.active, 'country miners'),
      inactive: nat(c.miners.inactive, 'country miners'), connected: nat(c.miners.connected, 'country miners') };
    if (cm.inactive > cm.active || cm.active > cm.located || cm.connected > cm.located) fail('country miners');
    const row = { code: c.code, miners: cm, nodes: nat(c.nodes, 'country nodes'), blocks: nat(c.blocks, 'country blocks'),
      blocksShare: percentE10(c.blocksSharePercentE10, 'country block share'), rewardsWei: decOrNull(c.rewardsWei, 'country rewards') };
    if (row.blocksShare !== shareE10(row.blocks, window.blocks) || (cm.located === 0 && (row.blocks > 0 || (row.rewardsWei ?? 0n) > 0n))) fail('country blocks');
    for (const k of ['located', 'active', 'inactive', 'connected']) sum[k] += cm[k];
    sum.nodes += row.nodes; sum.blocks += row.blocks;
    return row;
  });
  const u = d.unknown;
  if (!isObj(u)) fail('unknown');
  const unknown = { miners: nat(u.miners, 'unknown miners'), blocks: nat(u.blocks, 'unknown blocks') };
  if (sum.located !== miners.located || sum.active > miners.active || sum.inactive > miners.inactive || sum.connected > miners.connected
      || unknown.miners !== miners.unknown || sum.nodes !== nodes.located || sum.blocks + unknown.blocks !== window.blocks) fail('country totals');
  const rewardsKnown = countries.every((c) => c.rewardsWei !== null) || countries.every((c) => c.rewardsWei === null);
  if (!rewardsKnown) fail('country rewards');
  return { generatedAt, height, blockHash: d.blockHash, database, miners, nodes, sources, window, countries, unknown,
    unknownActive: miners.active - sum.active, unknownInactive: miners.inactive - sum.inactive, unknownConnected: miners.connected - sum.connected };
}

// ---------------------------------------------------------------------------------------------------------------
// Map geometry: the bundle's shapes are pre-projected equirectangular in a 1000 × 500 space.
export const project = (lon, lat) => [((lon + 180) * 1000) / 360, ((90 - lat) * 1000) / 360];
export function clampView(v) {
  const w = Math.min(WORLD.w, Math.max(MIN_VIEW_W, v.w)), h = w / 2;
  return { x: Math.min(WORLD.w - w, Math.max(0, v.x)), y: Math.min(WORLD.h - h, Math.max(0, v.y)), w, h };
}
/** The view zoomed by `factor` (> 1 zooms in) keeping the map point (px, py) where it is on screen. */
export function zoomView(v, factor, px = v.x + v.w / 2, py = v.y + v.h / 2) {
  const w = Math.min(WORLD.w, Math.max(MIN_VIEW_W, v.w / factor)), k = w / v.w;
  return clampView({ x: px - (px - v.x) * k, y: py - (py - v.y) * k, w });
}
/** Distance clustering at the current zoom: points ({key, x, y, weight}) within radiusPx screen pixels of a cluster's
 * seed (the heaviest point first) join it. Returns [{x, y, members}] with the seed's position. */
export function cluster(points, unitsPerPx, radiusPx = CLUSTER_PX) {
  const r = radiusPx * unitsPerPx, out = [];
  const sorted = [...points].sort((a, b) => b.weight - a.weight || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
  for (const p of sorted) {
    let best = null, distance = Infinity;
    for (const c of out) { const dd = Math.hypot(c.x - p.x, c.y - p.y); if (dd <= r && dd < distance) { best = c; distance = dd; } }
    if (best) best.members.push(p); else out.push({ x: p.x, y: p.y, members: [p] });
  }
  return out;
}
/** Ring segments [{kind, from, length}] in fractions of the circle for active-and-sharing, inactive and other located
 * miners (located − active). */
/** The marker core's class from a country's miner counts (the legend's keys): blue (no class) while any located miner is
 *  active and sharing, the inactive grey when the active ones are all inactive, the "located, not active" key when none
 *  is active; the nodes layer (counts null) is blue. */
export function markerCore(counts) {
  if (!counts || counts.active - counts.inactive > 0) return '';
  return counts.inactive > 0 ? 'is-inactive' : 'is-other';
}
export function ringSegments(located, active, inactive) {
  if (located <= 0) return [];
  const parts = [['active', active - inactive], ['inactive', inactive], ['other', located - active]].filter(([, n]) => n > 0);
  let at = 0;
  return parts.map(([kind, n]) => { const seg = { kind, from: at, length: n / located }; at += n / located; return seg; });
}
/** Choropleth classes: up to four equal ranges of 1..max; [{level, lo, hi}] with empty ranges dropped. */
export function choroplethLevels(max) {
  const out = [];
  for (let k = 1; k <= 4; k++) {
    const lo = Math.floor((max * (k - 1)) / 4) + 1, hi = Math.floor((max * k) / 4);
    if (lo <= hi) out.push({ level: out.length + 1, lo, hi });
  }
  return out;
}
const levelOf = (levels, n) => (n > 0 ? (levels.find((l) => n <= l.hi) || levels[levels.length - 1]).level : 0);
/** Clean axis ticks from 0 to at least max: [0, step, …]. */
export function niceTicks(max, count = 4) {
  if (max <= 0) return [0, 1];
  const raw = max / count, mag = 10 ** Math.floor(Math.log10(raw)), step = [1, 2, 5, 10].map((m) => m * mag).find((s) => s >= raw);
  const ticks = [];
  for (let t = 0; t < max + step; t += step) ticks.push(Math.round(t));
  return ticks;
}

// ---------------------------------------------------------------------------------------------------------------
// Fetching.
class ApiError extends Error {
  constructor(kind, message, extra = {}) { super(message || kind); this.kind = kind; Object.assign(this, extra); }
}
const retryAfter = (value) => { const n = Number(value); return Number.isFinite(n) && n > 0 ? Math.min(30, Math.ceil(n)) : 5; };
async function getData(path, params, signal) {
  const url = path + (params ? '?' + new URLSearchParams(params).toString() : '');
  const ctrl = new AbortController(), timer = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
  const abort = () => ctrl.abort();
  signal?.addEventListener('abort', abort);
  try {
    let res;
    try { res = await fetch(url, { cache: 'no-store', credentials: 'same-origin', headers: { Accept: 'application/json' }, signal: ctrl.signal }); } catch (e) {
      throw new ApiError(signal?.aborted ? 'aborted' : 'network');
    }
    const text = await res.text();
    let body = null;
    if (text.length <= MAX_BODY) { try { body = JSON.parse(text); } catch { body = null; } }
    const message = isObj(body) && typeof body.error === 'string' ? body.error.slice(0, 300) : '';
    if (res.status === 200) {
      if (!isObj(body) || !('data' in body)) throw new ApiError('invalid', 'unreadable answer');
      return body.data;
    }
    if (res.status === 429) throw new ApiError('busy', message, { after: retryAfter(res.headers.get('Retry-After')) });
    if (res.status === 409 && isObj(body) && body.reset === true) throw new ApiError('reset', message);
    if (res.status === 503) throw new ApiError('unavailable', message);
    if (res.status >= 400 && res.status < 500) throw new ApiError('refused', message || `HTTP ${res.status}`);
    throw new ApiError('network', message);
  } finally { clearTimeout(timer); signal?.removeEventListener('abort', abort); }
}

// ---------------------------------------------------------------------------------------------------------------
// DOM helpers (text through textContent only; no HTML string is ever parsed).
const SVG_NS = 'http://www.w3.org/2000/svg';
function setProps(el, props) {
  for (const [k, v] of Object.entries(props || {})) {
    if (v === null || v === undefined || v === false) continue;
    if (k === 'class') el.setAttribute('class', v);
    else if (k === 'text') el.textContent = String(v);
    else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v);
    else el.setAttribute(k, v === true ? '' : String(v));
  }
}
function add(el, kids) {
  for (const kid of kids.flat(Infinity)) if (kid !== null && kid !== undefined && kid !== false) el.append(kid instanceof Node ? kid : document.createTextNode(String(kid)));
  return el;
}
function h(tag, props, ...kids) { const el = document.createElement(tag); setProps(el, props); return add(el, kids); }
function s(tag, props, ...kids) { const el = document.createElementNS(SVG_NS, tag); setProps(el, props); return add(el, kids); }
const ICONS = {
  wallet: ['M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1', 'M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4'],
  globe: ['M12 2a10 10 0 1 0 0 20 10 10 0 1 0 0-20', 'M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20', 'M2 12h20'],
  search: ['M11 3a8 8 0 1 0 0 16 8 8 0 1 0 0-16', 'm21 21-4.3-4.3'],
  copy: ['M10 8h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H10a2 2 0 0 1-2-2V10a2 2 0 0 1 2-2', 'M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2'],
  plus: ['M5 12h14', 'M12 5v14'], minus: ['M5 12h14'],
  locate: ['M2 12h3', 'M19 12h3', 'M12 2v3', 'M12 19v3', 'M12 5a7 7 0 1 0 0 14 7 7 0 1 0 0-14'],
  info: ['M12 2a10 10 0 1 0 0 20 10 10 0 1 0 0-20', 'M12 16v-4', 'M12 8h.01'],
  alert: ['m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3', 'M12 9v4', 'M12 17h.01'],
  refresh: ['M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8', 'M21 3v5h-5', 'M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16', 'M8 16H3v5'],
  sort: ['m21 16-4 4-4-4', 'M17 20V4', 'm3 8 4-4 4 4', 'M7 4v16'],
  first: ['m11 17-5-5 5-5', 'm18 17-5-5 5-5'], prev: ['m15 18-6-6 6-6'], next: ['m9 18 6-6-6-6'], last: ['m6 17 5-5-5-5', 'm13 17 5-5-5-5'],
  open: ['M15 3h6v6', 'M10 14 21 3', 'M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6'],
  table: ['M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2', 'M3 9h18', 'M3 15h18', 'M12 3v18'],
  chart: ['M3 3v16a2 2 0 0 0 2 2h16', 'M18 17V9', 'M13 17V5', 'M8 17v-3'],
  pin: ['M20 10c0 4.993-5.539 10.193-7.399 11.799a1 1 0 0 1-1.202 0C9.539 20.193 4 14.993 4 10a8 8 0 0 1 16 0', 'M12 7a3 3 0 1 0 0 6 3 3 0 1 0 0-6'],
  users: ['M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2', 'M9 3a4 4 0 1 0 0 8 4 4 0 1 0 0-8', 'M22 21v-2a4 4 0 0 0-3-3.87', 'M16 3.13a4 4 0 0 1 0 7.75'],
  server: ['M4 2h16a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2', 'M4 14h16a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-4a2 2 0 0 1 2-2', 'M6 6h.01', 'M6 18h.01'],
  link: ['M9 17H7A5 5 0 0 1 7 7h2', 'M15 7h2a5 5 0 1 1 0 10h-2', 'M8 12h8'],
};
function icon(name, size = 16, extra = '') {
  return s('svg', { class: ('pocol-icon ' + extra).trim(), width: size, height: size, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor',
    'stroke-width': 2, 'stroke-linecap': 'round', 'stroke-linejoin': 'round', 'aria-hidden': 'true', focusable: 'false' },
  ICONS[name].map((d) => s('path', { d })));
}
const bdi = (text, cls) => h('bdi', { class: cls || null, dir: 'ltr', text });
function amount(wei, frac = 4, unit = 'PCOL') {
  const f = formatPcol(wei, frac);
  return h('span', { class: 'amount', title: `${f.full} ${unit}` }, bdi(f.text), unit ? h('small', { text: unit }) : null);
}
function percent(e10, frac = 2) { const f = formatPercentE10(e10, frac); return h('span', { title: f.full }, bdi(f.text)); }
const shortAddress = (a) => a.slice(0, 8) + '…' + a.slice(-6);
function timeText(lang, unix, withDate = false) {
  if (!unix) return '';
  const opts = withDate ? { dateStyle: 'medium', timeStyle: 'medium' } : { timeStyle: 'medium' };
  try { return new Intl.DateTimeFormat(lang === 'ar' ? 'ar-u-nu-latn' : 'en-GB', opts).format(new Date(unix * 1000)); } catch { return new Date(unix * 1000).toISOString(); }
}
const countryNames = {};
function countryName(lang, code, shared) {
  const shape = shared?.shapes.get(code)?.name, fallback = shape || code;
  if (lang === 'en' && shape) return shape;
  try {
    countryNames[lang] ||= new Intl.DisplayNames([lang], { type: 'region' });
    const n = /^[A-Z]{2}$/.test(code) ? countryNames[lang].of(code) : null;
    return n && n !== code ? n : fallback;
  } catch { return fallback; }
}
function metric(label, valueNode, note, accent = false, iconName = null) {
  return h('div', { class: 'metric' + (accent ? ' accent' : '') },
    h('div', { class: 'metric-label' }, h('span', { text: label }), iconName ? icon(iconName, 17) : null),
    h('div', { class: 'metric-value' }, valueNode),
    h('div', { class: 'metric-note', text: note }));
}
function bigValue(wei, frac = 2) {
  const f = formatPcol(wei, frac);
  return h('span', { class: 'pocol-big', title: `${f.full} PCOL` }, bdi(f.text), h('small', { text: 'PCOL' }));
}
function emptyState(iconName, title, text, action) {
  return h('div', { class: 'empty' }, h('div', { class: 'empty-icon' }, icon(iconName, 20)), h('h3', { text: title }), text ? h('p', { text }) : null, action || null);
}

// ---------------------------------------------------------------------------------------------------------------
// The language switch and the view root shared by every view.
const views = new Set();
let language = 'en';
function setLanguage(lang) {
  if (lang === language) return;
  language = lang; storeLanguage(lang);
  for (const v of views) v.relabel();
}
function languageSwitch() {
  const group = h('div', { class: 'pocol-lang', role: 'group', 'aria-label': tr(language, 'language') });
  for (const [code, label] of [['en', 'EN'], ['ar', 'العربية']]) {
    group.append(h('button', { type: 'button', lang: code, 'aria-pressed': String(language === code), 'data-lang': code, text: label,
      onclick: () => { setLanguage(code); document.querySelector(`.pocol-lang [data-lang="${code}"]`)?.focus(); } }));
  }
  return group;
}
function errorText(lang, e) {
  if (e instanceof Invalid) return tr(lang, 'invalid', e.message);
  if (e?.kind === 'busy') return tr(lang, 'busy', e.after);
  if (e?.kind === 'refused') return tr(lang, 'refused', e.message);
  if (e?.kind === 'unavailable') return e.message ? `${e.message} ${tr(lang, 'askAgain')}` : tr(lang, 'notReady');
  if (e?.kind === 'invalid') return tr(lang, 'invalid', e.message);
  return tr(lang, 'unreachable');
}

/** Gives the focus back to the control that has `key` as its data-focus, after a re-render replaced it; a control now
 *  disabled (a pager's Next on the last page) passes the focus to the nearest enabled control of its data-focus-group. */
function refocus(root, key) {
  const all = [...root.querySelectorAll('[data-focus]')];
  let target = all.find((e) => e.getAttribute('data-focus') === key) || null;
  if (target && target.disabled) {
    const group = target.closest('[data-focus-group]');
    const peers = group ? [...group.querySelectorAll('[data-focus]')] : [];
    const at = peers.indexOf(target);
    target = peers.filter((e) => !e.disabled).sort((a, b) => Math.abs(peers.indexOf(a) - at) - Math.abs(peers.indexOf(b) - at))[0] || null;
  }
  if (!target && key.startsWith('marker:')) {
    // A marker whose cluster changed: the marker now holding one of its countries, else the map itself.
    const codes = key.slice(7).split(' '), markers = all.filter((e) => e.hasAttribute('data-codes'));
    target = codes.map((c) => markers.find((m) => m.getAttribute('data-codes').split(' ').includes(c))).find(Boolean)
      || all.find((e) => e.getAttribute('data-focus') === 'map') || null;
  }
  target?.focus({ preventScroll: true });
  return target;
}
class View {
  constructor(mount) {
    this.mount = mount; this.timers = new Set(); this.abort = new AbortController();
    this.root = h('div', { class: 'pocol-insights' });
    mount.replaceChildren(this.root);
    views.add(this);
    try { this.shared = readShared(); } catch { this.shared = null; }
  }
  get lang() { return language; }
  t(key, ...args) { return tr(language, key, ...args); }
  later(fn, ms) { const id = setTimeout(() => { this.timers.delete(id); fn(); }, ms); this.timers.add(id); return id; }
  every(fn, ms) { const id = setInterval(fn, ms); this.timers.add(id); return id; }
  frame() { this.root.setAttribute('lang', language); this.root.setAttribute('dir', language === 'ar' ? 'rtl' : 'ltr'); }
  destroy() { this.abort.abort(); for (const id of this.timers) { clearTimeout(id); clearInterval(id); } this.timers.clear(); views.delete(this); }
  relabel() { this.render(); }
  /** Runs `fn`, which may rebuild the focused control, and keeps the keyboard focus on the control with the same
   *  data-focus key (an auto-refresh or a keyboard action never throws the focus back to the page). */
  keep(fn) {
    const a = document.activeElement, key = a && this.root.contains(a) ? a.closest('[data-focus]')?.getAttribute('data-focus') : null;
    const out = fn();
    if (key && document.activeElement !== a) refocus(this.root, key);
    else if (key && !a.isConnected) refocus(this.root, key);
    return out;
  }
}

// ---------------------------------------------------------------------------------------------------------------
// The Holders page.
class HoldersView extends View {
  constructor(mount) {
    super(mount);
    this.q = { page: 1, size: 25, order: 'balance_desc', type: 'all', q: '', pin: null };
    this.page = null; this.pageError = null; this.stats = null; this.statsError = null; this.loadingPage = false;
    this.newer = null; this.notice = null; this.searchText = ''; this.searchInvalid = false; this.chartTables = {};
    this.seq = 0; this.retryTimer = null;
    this.render();
    if (this.shared) { this.load(); this.loadStats(); }
    this.every(() => this.tick(), HOLDERS_REFRESH_MS);
    this.onVisible = () => { if (!document.hidden) this.tick(); };
    document.addEventListener('visibilitychange', this.onVisible);
  }
  destroy() { document.removeEventListener('visibilitychange', this.onVisible); super.destroy(); }
  tick() {
    if (document.hidden || !this.shared) return;
    this.loadStats();
    if (this.q.page === 1 && !this.q.pin) this.load({ quiet: true });
  }
  request() { return { page: this.q.page, size: this.q.size, order: this.q.order, type: this.q.type, q: this.q.q, pin: this.q.pin }; }
  async load({ quiet = false } = {}) {
    const req = this.request(), seq = ++this.seq;
    if (!quiet) { this.loadingPage = true; this.renderTable(); }
    const params = { page: req.page, size: req.size, order: req.order, type: req.type, q: req.q };
    if (req.pin) Object.assign(params, { height: req.pin.height, hash: req.pin.hash });
    let unchanged = false;
    try {
      const data = validateHoldersPage(await getData(API.holders, params, this.abort.signal), req, this.shared);
      if (seq !== this.seq) return;
      // The same snapshot again (an auto-refresh with nothing new): the table and its pager are not rebuilt.
      unchanged = quiet && !!this.page && !this.pageError && data.snapshot.hash === this.page.snapshot.hash && data.snapshot.height === this.page.snapshot.height;
      this.page = data; this.pageError = null;
      if (this.q.pin === null && this.q.page > 1) this.q.pin = { height: data.snapshot.height, hash: data.snapshot.hash };
    } catch (e) {
      if (seq !== this.seq || e?.kind === 'aborted') return;
      if (e?.kind === 'reset') {
        this.notice = this.t('reset'); this.q.page = 1; this.q.pin = null; this.newer = null;
        this.loadingPage = false; this.renderTable(); this.later(() => { this.notice = null; this.renderTable(); }, 6000);
        return this.load();
      }
      this.pageError = e;
      if (e instanceof Invalid) this.page = null;
      if (e?.kind === 'busy') this.later(() => { if (this.pageError === e) this.load(); }, e.after * 1000);
    } finally {
      if (seq === this.seq) { this.loadingPage = false; if (unchanged) { this.renderTableHead(); this.renderStatus(); } else this.renderTable(); }
    }
  }
  async loadStats() {
    const before = this.statsError ? null : this.stats;
    try {
      const data = validateStats(await getData(API.stats, null, this.abort.signal), this.shared);
      this.stats = data; this.statsError = null;
      const pin = this.q.pin;
      this.newer = pin && data.snapshot.height > pin.height ? data.snapshot.height : null;
    } catch (e) {
      if (e?.kind === 'aborted') return;
      this.statsError = e;
      if (e instanceof Invalid) this.stats = null;
      if (e?.kind === 'busy') this.later(() => { if (this.statsError === e) this.loadStats(); }, e.after * 1000);
    }
    // The same snapshot as before: only the figures are redrawn (no chart, table or pager is rebuilt). Otherwise the
    // table too: its rewards column names the index start the statistics give.
    const same = !!before && !this.statsError && this.stats.snapshot.hash === before.snapshot.hash && this.stats.snapshot.height === before.snapshot.height;
    this.renderStats();
    if (same) this.renderStatus(); else { this.renderCharts(); this.renderTable(); }
  }
  go(page) {
    const pages = this.page ? pageCount(this.page.total, this.page.size) : 1;
    page = Math.max(1, Math.min(pages, page));
    if (page === this.q.page) return;
    if (page === 1) this.q.pin = null;
    else if (!this.q.pin && this.page) this.q.pin = { height: this.page.snapshot.height, hash: this.page.snapshot.hash };
    this.q.page = page; this.newer = null;
    this.load();
  }
  restart(changes) { Object.assign(this.q, changes, { page: 1, pin: null }); this.newer = null; this.load(); }
  showNewest() { this.q.pin = null; this.newer = null; this.load(); }

  render() {
    this.frame();
    const shared = this.shared;
    this.root.replaceChildren();
    this.live = h('div', { class: 'sr-only', 'aria-live': 'polite' });
    this.root.append(h('div', { class: 'pocol-bar' }, h('p', { class: 'pocol-bar-text', text: this.t('hSub') }), languageSwitch()), this.live);
    if (!shared) { this.root.append(h('section', { class: 'panel' }, emptyState('alert', this.t('hTitle'), this.t('noIdentity')))); return; }
    this.verifyBox = h('div'); this.statsBox = h('div'); this.chartsBox = h('div', { class: 'pocol-charts' });
    this.tableHeadBox = h('div'); this.statusBox = h('div'); this.tableBox = h('div', { class: 'pocol-table-box' }); this.footBox = h('div');
    this.searchInput = h('input', { type: 'search', placeholder: this.t('search'), 'aria-label': this.t('searchAria'), spellcheck: 'false', autocomplete: 'off',
      maxlength: 42, value: this.searchText, dir: 'ltr', oninput: (e) => this.onSearch(e.target.value) });
    this.searchError = h('p', { class: 'pocol-field-error', id: 'pocol-holders-search-error', role: 'alert' });
    const typeSelect = h('select', { 'aria-label': this.t('typeAria'), onchange: (e) => this.restart({ type: e.target.value }) },
      TYPES.map((k) => h('option', { value: k, text: this.t('types')[k], selected: this.q.type === k })));
    this.orderButton = h('button', { type: 'button', class: 'history-sort pocol-order', 'aria-label': this.t('orderAria'),
      onclick: () => this.restart({ order: this.q.order === 'balance_desc' ? 'balance_asc' : 'balance_desc' }) });
    const panel = h('section', { class: 'panel transaction-history pocol-holders-panel', 'aria-labelledby': 'pocol-holders-title' },
      this.tableHeadBox,
      h('div', { class: 'history-controls' },
        h('label', { class: 'history-search' }, icon('search', 16), this.searchInput),
        h('div', { class: 'pocol-controls-end' }, h('label', { class: 'history-sort' }, icon('users', 15), typeSelect), this.orderButton)),
      this.searchError, this.statusBox, this.tableBox, this.footBox);
    this.root.append(this.verifyBox, this.statsBox, this.chartsBox, panel, this.notesBlock());
    this.renderStats(); this.renderCharts(); this.renderTableHead(); this.renderStatus(); this.renderTable(); this.showSearchError();
  }
  notesBlock() {
    return h('section', { class: 'pocol-notes', 'aria-label': this.t('notesTitle') }, icon('info', 15),
      h('div', {}, h('strong', { text: this.t('notesTitle') }), h('ul', {}, this.t('notes').map((n) => h('li', { text: n })))));
  }
  onSearch(value) {
    this.searchText = value;
    clearTimeout(this.searchTimer);
    this.searchTimer = setTimeout(() => {
      const q = value.trim().toLowerCase();
      this.searchInvalid = q !== '' && !QUERY.test(q);
      this.showSearchError();
      if (!this.searchInvalid && q !== this.q.q) this.restart({ q });
    }, 350);
  }
  showSearchError() {
    if (!this.searchError) return;
    this.searchError.textContent = this.searchInvalid ? this.t('searchInvalid') : '';
    this.searchError.hidden = !this.searchInvalid;
    if (this.searchInvalid) this.searchInput.setAttribute('aria-describedby', this.searchError.id); else this.searchInput.removeAttribute('aria-describedby');
    this.searchInput.setAttribute('aria-invalid', String(this.searchInvalid));
  }
  renderStats() { this.keep(() => this._renderStats()); }
  _renderStats() {
    if (!this.statsBox) return;
    const st = this.stats, err = this.statsError;
    this.verifyBox.replaceChildren();
    if (st && !st.verification.complete) {
      const v = st.verification;
      this.verifyBox.append(h('div', { class: 'warning pocol-verify', role: 'status' }, icon('alert', 17),
        h('div', {}, h('strong', { text: this.t('verifyTitle') }), h('p', { text: this.t('verifyText', st.snapshot.height, formatPcol(v.unlocatedWei, 18).text, v.accountsMissing) }))));
    }
    if (!st) {
      this.statsBox.replaceChildren(err ? this.errorPanel(err, () => this.loadStats()) : h('div', { class: 'metrics pocol-loading', 'aria-busy': 'true' },
        [0, 1, 2, 3].map(() => h('div', { class: 'metric' }, h('div', { class: 'metric-label', text: this.t('loading') }), h('div', { class: 'metric-value' })))));
      return;
    }
    const S = st.supply, short = (w) => formatPcol(w, 2).text;
    const metrics = h('div', { class: 'metrics pocol-metrics is-amounts' + (err ? ' pocol-stale' : '') },
      metric(this.t('mSupply'), bigValue(S.currentSupplyWei), this.t('mSupplyNote', st.snapshot.height), true, 'wallet'),
      metric(this.t('mHolders'), h('span', { class: 'pocol-big' }, bdi(fmtInt(st.holders.count))),
        st.holders.new24h === null ? this.t('mHoldersNote') : this.t('mHoldersNew', st.holders.new24h), false, 'users'),
      metric(this.t('mCirculating'), bigValue(S.circulatingWei), this.t('mCirculatingNote')),
      metric(this.t('mTop10'), h('span', { class: 'pocol-big' }, percent(st.concentration[10].share)), this.t('mTop10Note')),
      metric(this.t('mBonded'), bigValue(S.bondedWei), this.t('mBondedNote', st.holders.withBonded)),
      metric(this.t('mPending'), bigValue(S.pendingRewardsWei), this.t('mPendingNote')),
      metric(this.t('mBurned'), bigValue(S.totalBurnedWei), this.t('mBurnedNote', short(S.baseFeeBurnedWei), short(S.protocolBurnedWei))),
      metric(this.t('mMinted'), bigValue(S.mintedWei), this.t('mMintedNote', short(S.genesisWei), short(S.issuedWei))));
    this.statsBox.replaceChildren(err ? this.inlineError(err, () => this.loadStats()) : '', metrics);
  }
  errorPanel(err, retry) {
    return h('section', { class: 'panel pocol-error', role: 'alert' }, emptyState('alert', err?.kind === 'busy' ? this.t('loading') : this.t('hTitle'), errorText(this.lang, err),
      h('button', { type: 'button', class: 'text-button', 'data-focus': 'retry:panel', onclick: retry }, icon('refresh', 13), this.t('retry'))));
  }
  inlineError(err, retry) {
    return h('div', { class: 'warning', role: 'status' }, icon('alert', 16), h('span', { text: errorText(this.lang, err) }),
      h('button', { type: 'button', 'data-focus': 'retry:inline', onclick: retry, text: this.t('retry') }));
  }
  chartPanel(key, title, sub, body, tableFn, legend) {
    const showing = !!this.chartTables[key];
    const toggle = h('button', { type: 'button', class: 'text-button pocol-view-toggle', 'aria-pressed': String(showing), 'data-focus': 'toggle:' + key,
      'aria-label': showing ? this.t('showChart', title) : this.t('showTable', title),
      onclick: () => { this.chartTables[key] = !showing; this.renderCharts(); this.chartsBox.querySelector(`[data-chart="${key}"] .pocol-view-toggle`)?.focus(); } },
    icon(showing ? 'chart' : 'table', 13), showing ? this.t('chart') : this.t('table'));
    return h('section', { class: 'panel chart-panel pocol-chart', 'data-chart': key, 'aria-label': title },
      h('div', { class: 'panel-heading' }, h('div', {}, h('h2', { text: title }), h('p', { text: sub })), toggle),
      showing ? h('div', { class: 'table-wrap pocol-chart-table' }, tableFn()) : body, showing ? null : legend || null);
  }
  renderCharts() { this.keep(() => this._renderCharts()); }
  _renderCharts() {
    if (!this.chartsBox) return;
    const st = this.stats;
    if (!st) { this.chartsBox.replaceChildren(); return; }
    const supply = st.supply.currentSupplyWei, top = st.concentration[10], rest = supply - top.totalWei, restShare = shareE10(rest, supply);
    // Top 10 against the rest: a tall segmented bar (two parts of one whole) with its shares printed inside.
    const parts = [{ key: 'top', label: this.t('cTop10'), wei: top.totalWei, share: top.share }, { key: 'rest', label: this.t('cRest'), wei: rest, share: restShare }];
    const bar = h('div', { class: 'pocol-split', role: 'img', 'aria-label': parts.map((p) => `${p.label}: ${formatPercentE10(p.share).text}`).join('; ') },
      parts.filter((p) => p.wei > 0n).map((p) => h('div', { class: 'pocol-split-part is-' + p.key, style: `flex-grow:${Number(p.share) / 1e10}`,
        title: `${p.label}: ${formatPercentE10(p.share).text} · ${formatPcol(p.wei, 2).text} PCOL`, tabindex: 0, 'data-focus': 'split:' + p.key },
      // The share is printed inside a part only where it fits (a fifth of the bar); the legend always gives it.
      p.share >= 20n * E10 ? h('span', { class: 'pocol-split-label' }, bdi(formatPercentE10(p.share, 1).text)) : null)));
    const splitLegend = h('ul', { class: 'pocol-legend pocol-legend-rows' }, parts.map((p) => h('li', {}, h('i', { class: 'pocol-swatch is-' + p.key, 'aria-hidden': 'true' }),
      h('span', { class: 'pocol-legend-name', text: p.label }), h('span', { class: 'pocol-legend-value' }, percent(p.share), ' · ', amount(p.wei, 2)))));
    const others = h('dl', { class: 'pocol-topn' }, [1, 50, 100].map((n) => h('div', {}, h('dt', { text: this.t('cTopN', n) }), h('dd', {}, percent(st.concentration[n].share)))));
    const topTable = () => h('table', {}, h('caption', { class: 'sr-only', text: this.t('cTopTitle') }),
      h('thead', {}, h('tr', {}, [this.t('cGroup'), this.t('cShare'), this.t('cAmount')].map((c, i) => h('th', { scope: 'col', class: i ? 'right' : null, text: c })))),
      h('tbody', {}, [...parts, ...[1, 50, 100].map((n) => ({ label: this.t('cTopN', n), wei: st.concentration[n].totalWei, share: st.concentration[n].share }))]
        .map((p) => h('tr', {}, h('td', { text: p.label }), h('td', { class: 'right' }, percent(p.share, 4)), h('td', { class: 'right' }, amount(p.wei, 4))))));
    const topBody = h('div', { class: 'pocol-chart-body' }, bar, splitLegend, others);

    const distTable = () => h('table', {}, h('caption', { class: 'sr-only', text: this.t('cDistTitle') }),
      h('thead', {}, h('tr', {}, [this.t('cRange'), this.t('cAccounts'), this.t('cTotal')].map((c, i) => h('th', { scope: 'col', class: i ? 'right' : null, text: c })))),
      h('tbody', {}, st.distribution.map((b) => h('tr', {}, h('td', {}, bdi(rangeLabel(b))), h('td', { class: 'right' }, bdi(fmtInt(b.accounts))), h('td', { class: 'right' }, amount(b.totalWei, 2, ''))))));
    const distBody = h('div', { class: 'pocol-chart-body pocol-plot', 'data-plot': 'distribution' });

    const pts = st.history.points;
    const histTable = () => pts.length ? h('table', {}, h('caption', { class: 'sr-only', text: this.t('cHistTitle') }),
      h('thead', {}, h('tr', {}, [this.t('cBlock'), this.t('cTime'), this.t('cHolders'), this.t('cTop10Share')].map((c, i) => h('th', { scope: 'col', class: i > 1 ? 'right' : null, text: c })))),
      h('tbody', {}, [...pts].reverse().map((p) => h('tr', {}, h('td', {}, bdi('#' + fmtInt(p.height))), h('td', { text: timeText(this.lang, p.timestamp, true) }),
        h('td', { class: 'right' }, bdi(fmtInt(p.holders))), h('td', { class: 'right' }, percent(p.top10, 2)))))) : h('p', { class: 'small-empty', text: this.t('cHistEmptyNone') });
    const histBody = pts.length >= 2 ? h('div', { class: 'pocol-chart-body pocol-plot', 'data-plot': 'history' })
      : h('div', { class: 'pocol-chart-body pocol-chart-empty' }, emptyState('chart', this.t('cHistTitle'), pts.length ? this.t('cHistEmptyOne', st.history.from) : this.t('cHistEmptyNone')));
    this.chartsBox.replaceChildren(
      this.chartPanel('top', this.t('cTopTitle'), this.t('cTopSub'), topBody, topTable),
      this.chartPanel('distribution', this.t('cDistTitle'), this.t('cDistSub'), distBody, distTable),
      this.chartPanel('history', this.t('cHistTitle'), st.history.from === null ? this.t('cHistSubNone') : this.t('cHistSub', st.history.from), histBody, histTable));
    this.watchPlots();
  }
  watchPlots() {
    this.plotObserver?.disconnect();
    const plots = [...this.chartsBox.querySelectorAll('.pocol-plot')];
    if (!plots.length) return;
    const draw = (el) => (el.dataset.plot === 'distribution' ? this.drawDistribution(el) : this.drawHistory(el));
    if (typeof ResizeObserver === 'function') {
      this.plotObserver = new ResizeObserver((entries) => { for (const e of entries) if (Math.abs((e.target._w || 0) - e.contentRect.width) > 1) { e.target._w = e.contentRect.width; draw(e.target); } });
      for (const el of plots) this.plotObserver.observe(el);
    }
    for (const el of plots) { el._w = el.clientWidth; draw(el); }
  }
  drawDistribution(el) {
    const st = this.stats, data = st.distribution, width = Math.max(240, el.clientWidth || 300), height = 196;
    const pad = { l: 40, r: 8, t: 18, b: 40 }, plotW = width - pad.l - pad.r, plotH = height - pad.t - pad.b;
    const ticks = niceTicks(Math.max(...data.map((b) => b.accounts))), top = ticks[ticks.length - 1], band = plotW / data.length, barW = Math.min(24, band - 6);
    const y = (n) => pad.t + plotH - (n / top) * plotH, rtl = this.lang === 'ar';
    const xAt = (i) => pad.l + band * (rtl ? data.length - 1 - i : i) + band / 2;
    const tip = h('div', { class: 'pocol-tip', role: 'status', hidden: true });
    const svg = s('svg', { class: 'pocol-svg', width, height, viewBox: `0 0 ${width} ${height}`, role: 'img', 'aria-label': this.t('cDistTitle') },
      ticks.map((t) => s('g', { class: 'pocol-grid' }, s('line', { x1: pad.l, x2: width - pad.r, y1: y(t), y2: y(t) }),
        s('text', { x: pad.l - 8, y: y(t) + 3, 'text-anchor': 'end', text: fmtInt(t) }))),
      s('line', { class: 'pocol-baseline', x1: pad.l, x2: width - pad.r, y1: y(0), y2: y(0) }));
    const maxIndex = data.reduce((m, b, i) => (b.accounts > data[m].accounts ? i : m), 0);
    data.forEach((b, i) => {
      const x = xAt(i), top0 = y(b.accounts), hBar = y(0) - top0, r = Math.min(4, hBar, barW / 2);
      const g = s('g', { class: 'pocol-bar', tabindex: 0, role: 'img', 'aria-label': this.t('cDistTip', rangeLabel(b), b.accounts), 'data-focus': 'bucket:' + i });
      g.append(s('rect', { class: 'pocol-hit', x: x - band / 2, y: pad.t, width: band, height: plotH }));
      if (b.accounts > 0) g.append(s('path', { class: 'pocol-column', d: `M${x - barW / 2},${y(0)}V${top0 + r}Q${x - barW / 2},${top0} ${x - barW / 2 + r},${top0}H${x + barW / 2 - r}Q${x + barW / 2},${top0} ${x + barW / 2},${top0 + r}V${y(0)}Z` }));
      if (i === maxIndex && b.accounts > 0) g.append(s('text', { class: 'pocol-value', x, y: top0 - 6, 'text-anchor': 'middle', text: fmtInt(b.accounts) }));
      g.append(s('text', { class: 'pocol-tick', x, y: y(0) + 15, 'text-anchor': 'middle', text: BUCKET_TICKS[i] }));
      const show = () => { tip.hidden = false; tip.textContent = this.t('cDistTip', rangeLabel(b), b.accounts); tip.style.left = `${Math.min(width - 150, Math.max(0, x - 70))}px`; tip.style.top = '0px'; g.classList.add('is-active'); };
      const hide = () => { tip.hidden = true; g.classList.remove('is-active'); };
      g.addEventListener('pointerenter', show); g.addEventListener('pointerleave', hide); g.addEventListener('focus', show); g.addEventListener('blur', hide);
      svg.append(g);
    });
    svg.append(s('text', { class: 'pocol-axis-title', x: pad.l + plotW / 2, y: height - 6, 'text-anchor': 'middle', text: this.t('cDistAxis') }));
    el.replaceChildren(svg, tip);
  }
  drawHistory(el) {
    const pts = this.stats.history.points, width = Math.max(240, el.clientWidth || 300), height = 196, rtl = this.lang === 'ar';
    const pad = { l: 40, r: 14, t: 16, b: 34 }, plotW = width - pad.l - pad.r, plotH = height - pad.t - pad.b;
    const lo = pts[0].height, hi = pts[pts.length - 1].height;
    const ticks = niceTicks(Math.max(...pts.map((p) => p.holders))), top = ticks[ticks.length - 1];
    const x = (p) => { const f = hi > lo ? (p.height - lo) / (hi - lo) : 0.5; return pad.l + (rtl ? 1 - f : f) * plotW; };
    const y = (n) => pad.t + plotH - (n / top) * plotH;
    const line = pts.map((p, i) => `${i ? 'L' : 'M'}${x(p).toFixed(1)},${y(p.holders).toFixed(1)}`).join('');
    const area = `${line}L${x(pts[pts.length - 1]).toFixed(1)},${y(0)}L${x(pts[0]).toFixed(1)},${y(0)}Z`;
    const cross = s('line', { class: 'pocol-cross', y1: pad.t, y2: y(0), visibility: 'hidden' });
    const dot = s('circle', { class: 'pocol-dot', r: 4, visibility: 'hidden' });
    const end = pts[pts.length - 1];
    const svg = s('svg', { class: 'pocol-svg', width, height, viewBox: `0 0 ${width} ${height}`, tabindex: 0, role: 'img', 'data-focus': 'history-plot',
      'aria-label': `${this.t('cHistTitle')}: ${this.t('cHistTip', end.holders, end.height, timeText(this.lang, end.timestamp, true))}` },
    ticks.map((t) => s('g', { class: 'pocol-grid' }, s('line', { x1: pad.l, x2: width - pad.r, y1: y(t), y2: y(t) }),
      s('text', { x: pad.l - 8, y: y(t) + 3, 'text-anchor': 'end', text: fmtInt(t) }))),
    s('path', { class: 'pocol-area', d: area }), s('path', { class: 'pocol-line', d: line }),
    s('circle', { class: 'pocol-end', cx: x(end), cy: y(end.holders), r: 4 }),
    s('text', { class: 'pocol-tick', x: x(pts[0]), y: height - 12, 'text-anchor': rtl ? 'end' : 'start', text: '#' + fmtInt(lo) }),
    s('text', { class: 'pocol-tick', x: x(end), y: height - 12, 'text-anchor': rtl ? 'start' : 'end', text: '#' + fmtInt(hi) }),
    cross, dot);
    const tip = h('div', { class: 'pocol-tip', role: 'status', hidden: true });
    let active = -1;
    const show = (i) => {
      active = Math.max(0, Math.min(pts.length - 1, i));
      const p = pts[active], px = x(p);
      for (const [a, v] of [['x1', px], ['x2', px]]) cross.setAttribute(a, v);
      cross.setAttribute('visibility', 'visible'); dot.setAttribute('cx', px); dot.setAttribute('cy', y(p.holders)); dot.setAttribute('visibility', 'visible');
      tip.hidden = false; tip.textContent = this.t('cHistTip', p.holders, p.height, timeText(this.lang, p.timestamp, true));
      tip.style.left = `${Math.min(width - 190, Math.max(0, px - 95))}px`; tip.style.top = '0px';
    };
    const hide = () => { cross.setAttribute('visibility', 'hidden'); dot.setAttribute('visibility', 'hidden'); tip.hidden = true; };
    svg.addEventListener('pointermove', (e) => {
      const r = svg.getBoundingClientRect(), px = e.clientX - r.left;
      let best = 0; pts.forEach((p, i) => { if (Math.abs(x(p) - px) < Math.abs(x(pts[best]) - px)) best = i; });
      show(best);
    });
    svg.addEventListener('pointerleave', hide); svg.addEventListener('blur', hide);
    svg.addEventListener('focus', () => show(active < 0 ? pts.length - 1 : active));
    svg.addEventListener('keydown', (e) => {
      const step = { ArrowLeft: rtl ? 1 : -1, ArrowRight: rtl ? -1 : 1, Home: -Infinity, End: Infinity }[e.key];
      if (step === undefined) return;
      e.preventDefault(); show(step === -Infinity ? 0 : step === Infinity ? pts.length - 1 : active + step);
    });
    el.replaceChildren(svg, tip);
  }
  renderTableHead() { this.keep(() => this._renderTableHead()); }
  _renderTableHead() {
    if (!this.tableHeadBox) return;
    const data = this.page;
    const line = data ? this.t('dataAt', data.snapshot.height, timeText(this.lang, data.index.updatedAt)) : this.t('loading');
    this.tableHeadBox.replaceChildren(h('div', { class: 'panel-heading' },
      h('div', { class: 'history-title' }, h('span', { class: 'history-emblem' }, icon('wallet', 19)),
        h('div', {}, h('h2', { id: 'pocol-holders-title', text: this.t('hTitle') }), h('p', { class: 'pocol-data-at', text: line })))));
    if (this.orderButton) this.orderButton.replaceChildren(icon('sort', 14), h('span', { text: this.t('order')[this.q.order] }));
  }
  renderStatus() { this.keep(() => this._renderStatus()); }
  _renderStatus() {
    if (!this.statusBox) return;
    const data = this.page, out = [];
    if (data) {
      const i = data.index;
      const text = i.state === 'ready' ? this.t('stReady') : i.state === 'syncing' ? this.t('stSyncing', Math.max(0, i.historyHeight), Math.max(0, i.head)) : this.t('stReconnecting');
      out.push(h('div', { class: 'history-index' + (i.state === 'ready' ? ' is-ready' : '') }, h('div', {}, icon(i.state === 'ready' ? 'info' : 'refresh', 14), h('span', { text: text }),
        this.q.pin ? h('small', { text: this.t('pinned', this.q.pin.height) }) : null),
      i.state === 'syncing' && i.head > 0 ? h('progress', { max: i.head, value: Math.max(0, i.historyHeight), 'aria-label': text }) : null));
    }
    if (this.newer) out.push(h('div', { class: 'history-notice', role: 'status' }, h('span', { text: this.t('newer', this.newer) }),
      h('button', { type: 'button', class: 'text-button', 'data-focus': 'newest', onclick: () => this.showNewest() }, icon('refresh', 13), this.t('showNewest'))));
    if (this.notice) out.push(h('div', { class: 'history-notice', role: 'status' }, h('span', { text: this.notice })));
    this.statusBox.replaceChildren(...out);
  }
  renderTable() { this.keep(() => this._renderTable()); }
  _renderTable() {
    if (!this.tableBox) return;
    this.renderTableHead(); this.renderStatus();
    const data = this.page, err = this.pageError;
    const box = [];
    if (err) box.push(h('div', { class: 'warning pocol-table-error', role: 'alert' }, icon('alert', 16), h('span', { text: errorText(this.lang, err) }),
      h('button', { type: 'button', 'data-focus': 'retry:table', onclick: () => this.load(), text: this.t('retry') })));
    if (!data) {
      if (!err) box.push(emptyState('refresh', this.t('loading'), null));
      this.tableBox.replaceChildren(...box); this.footBox.replaceChildren(); this.tableBox.setAttribute('aria-busy', String(this.loadingPage));
      return;
    }
    if (!data.rows.length) box.push(emptyState('search', this.t('empty'), this.t('emptyHint')));
    else box.push(this.table(data));
    this.tableBox.replaceChildren(...box);
    this.tableBox.classList.toggle('is-loading', this.loadingPage);
    this.tableBox.setAttribute('aria-busy', String(this.loadingPage));
    this.footer(data);
  }
  table(data) {
    const from = this.stats?.rewards?.from ?? null;
    const cols = [['colRank', 'right'], ['colAddress'], ['colType'], ['colTotal', 'right'], ['colAvailable', 'right'], ['colBonded', 'right'],
      ['colShare', 'right'], ['colTxs', 'right'], ['colBlocks', 'right'], ['colRewards', 'right']];
    return h('div', { class: 'table-wrap' }, h('table', { class: 'history-table pocol-holders-table' },
      h('caption', { class: 'sr-only', text: `${this.t('hTitle')} · ${this.t('dataAt', data.snapshot.height)}` }),
      h('thead', {}, h('tr', {}, cols.map(([k, c]) => h('th', { scope: 'col', class: c || null, text: k === 'colRewards' ? this.t(k, from) : this.t(k) })), h('th', { scope: 'col' }, h('span', { class: 'sr-only', text: this.t('copy', '') })))),
      h('tbody', {}, data.rows.map((r) => this.row(r)))));
  }
  row(r) {
    const sub = r.type === 'reward' ? this.t('serves', r.minerIds.length) : r.rewardAddress ? this.t('rewardsTo', shortAddress(r.rewardAddress)) : null;
    const open = h('button', { type: 'button', class: 'hash-link pocol-address', title: r.address, 'data-focus': 'open:' + r.address, 'aria-label': this.t('open', r.address), onclick: () => openAccount(r.address) }, bdi(shortAddress(r.address)));
    const copy = h('button', { type: 'button', class: 'icon-button pocol-copy', title: this.t('copy', r.address), 'data-focus': 'copy:' + r.address, 'aria-label': this.t('copy', r.address), onclick: () => this.copy(r.address) }, icon('copy', 14));
    return h('tr', {},
      h('td', { class: 'right pocol-rank' }, bdi(fmtInt(r.rank))),
      h('td', {}, h('div', { class: 'pocol-address-cell' }, open, r.label ? h('span', { class: 'soft-tag', text: r.label }) : null), sub ? h('span', { class: 'pocol-sub', text: sub }) : null),
      h('td', {}, h('span', { class: 'badge neutral pocol-type-' + r.type, text: this.t('type')[r.type] })),
      h('td', { class: 'right' }, amount(r.totalWei)), h('td', { class: 'right' }, amount(r.availableWei)), h('td', { class: 'right' }, amount(r.bondedWei)),
      h('td', { class: 'right' }, percent(r.share, 4)), h('td', { class: 'right' }, bdi(fmtInt(r.transactions))),
      h('td', { class: 'right' }, r.blocksMined === null ? h('span', { class: 'muted', title: this.t('notMiner'), text: '—' }) : bdi(fmtInt(r.blocksMined))),
      h('td', { class: 'right' }, r.rewardsWei === null ? h('span', { class: 'muted', title: this.t('rewardsNull'), text: '—' }) : amount(r.rewardsWei)),
      h('td', {}, copy));
  }
  footer(data) {
    const pages = pageCount(data.total, data.size), p = data.page, a = data.total ? (p - 1) * data.size + 1 : 0, b = (p - 1) * data.size + data.rows.length;
    const nav = (name, target, disabled) => h('button', { type: 'button', 'aria-label': this.t(name), title: this.t(name), disabled, 'data-focus': 'page:' + name, onclick: () => this.go(target) }, icon(name, 14, 'pocol-flip'));
    this.footBox.replaceChildren(h('div', { class: 'history-footer' },
      h('div', {}, h('span', {}, bdi(this.t('range', a, b, data.total))),
        h('label', {}, h('span', { text: this.t('rows') }), h('select', { 'data-focus': 'size', onchange: (e) => this.restart({ size: Number(e.target.value) }) },
          SIZES.map((n) => h('option', { value: n, text: n, selected: n === data.size }))))),
      // Not a <nav>: the explorer styles every `nav button` as its sidebar's.
      h('div', { class: 'history-pages', role: 'navigation', 'aria-label': this.t('pageOf', p, pages), 'data-focus-group': 'pages' },
        nav('first', 1, p <= 1), nav('prev', p - 1, p <= 1), h('span', { 'aria-current': 'page', text: this.t('pageOf', p, pages) }),
        nav('next', p + 1, p >= pages), nav('last', pages, p >= pages))));
  }
  async copy(text) {
    let ok = false;
    try { await navigator.clipboard.writeText(text); ok = true; } catch { ok = false; }
    this.live.textContent = ok ? this.t('copied') : this.t('copyFailed');
    toast(ok ? this.t('copied') : this.t('copyFailed'));
  }
}
function rangeLabel(b) {
  const n = (w) => groupDigits((w / WEI).toString());
  return b.max === null ? `≥ ${n(b.min)}` : `${n(b.min)} – ${n(b.max)}`;
}
function toast(text) {
  let el = document.querySelector('.pocol-toast');
  if (!el) { el = h('div', { class: 'toast pocol-toast', role: 'status' }); document.body.append(el); }
  el.textContent = text; el.hidden = false;
  clearTimeout(toast.timer); toast.timer = setTimeout(() => { el.hidden = true; }, 2200);
}
/** Opens an account in the explorer's own account dialog: its hashchange handler reads #page=…&q=<address>. */
function openAccount(addr) {
  const page = location.hash.includes('page=Miner') ? 'Miner%20map' : 'Holders';
  history.replaceState(null, '', `#page=${page}`);
  location.hash = `#page=${page}&q=${addr}`;
}
/** Switches the explorer to its Miner map tab through its own navigation button (or its hash route). */
function openMapPage() {
  const button = [...document.querySelectorAll('.sidebar nav button')].find((b) => b.textContent.trim() === 'Miner map');
  if (button) button.click(); else location.hash = '#page=Miner%20map';
}

// ---------------------------------------------------------------------------------------------------------------
// Geography: one poll for every mounted map view (30 s, only while visible).
const geo = {
  subscribers: new Set(), data: null, error: null, timer: null, inflight: null, loadedAt: 0,
  subscribe(view) {
    this.subscribers.add(view);
    if (!this.timer) this.timer = setInterval(() => { if (!document.hidden) this.load(); }, MAP_REFRESH_MS);
    if (this.data || this.error) view.update();
    if (Date.now() - this.loadedAt >= MAP_REFRESH_MS) this.load();
  },
  unsubscribe(view) { this.subscribers.delete(view); if (!this.subscribers.size) { clearInterval(this.timer); this.timer = null; } },
  async load() {
    if (this.inflight) return this.inflight;
    const shared = [...this.subscribers][0]?.shared;
    if (!shared) return null;
    this.inflight = (async () => {
      try { this.data = validateGeography(await getData(API.geography, null), shared); this.error = null; } catch (e) {
        this.error = e;
        if (e instanceof Invalid) this.data = null;
        if (e?.kind === 'busy') setTimeout(() => this.load(), e.after * 1000);
      } finally { this.inflight = null; this.loadedAt = Date.now(); }
      for (const v of this.subscribers) v.update();
    })();
    return this.inflight;
  },
};

/** The SVG world map: country shapes, choropleth, clustered glowing markers, zoom and pan (not in compact mode). */
class WorldMap {
  constructor(view, { compact = false } = {}) {
    this.view = view; this.compact = compact; this.box = { ...WORLD }; this.model = null; this.selected = null; this.pointers = new Map();
    const shared = view.shared;
    this.el = h('div', { class: 'world-map pocol-world' });
    this.svg = s('svg', { class: 'world-map-svg', viewBox: '0 0 1000 500', role: compact ? 'img' : 'application', tabindex: compact ? null : 0, preserveAspectRatio: 'xMidYMid meet',
      'data-focus': compact ? null : 'map' });
    const grat = [];
    for (let lon = -150; lon <= 150; lon += 30) grat.push(`M${project(lon, 0)[0]},0V500`);
    for (let lat = -60; lat <= 60; lat += 30) grat.push(`M0,${project(0, lat)[1]}H1000`);
    this.countryLayer = s('g', { class: 'pocol-countries' });
    this.paths = new Map();
    for (const c of shared.countries) {
      const p = s('path', { class: 'map-country', d: c.path, 'data-country': c.code });
      this.paths.set(c.code, p); this.countryLayer.append(p);
    }
    this.markerLayer = s('g', { class: 'pocol-markers' });
    this.svg.append(s('rect', { class: 'map-ocean', x: 0, y: 0, width: 1000, height: 500 }), s('path', { class: 'map-graticule', d: grat.join('') }), this.countryLayer, this.markerLayer);
    this.hoverName = h('div', { class: 'map-hover-name', 'aria-hidden': 'true' });
    this.tip = h('div', { class: 'pocol-tip pocol-map-tip', hidden: true });
    this.el.append(this.svg, this.hoverName, this.tip);
    if (!compact) {
      this.zoom = h('div', { class: 'map-zoom' },
        h('button', { type: 'button', 'data-zoom': 'in', 'data-focus': 'zoom:in', onclick: () => this.zoomBy(1.6) }, icon('plus', 16)),
        h('button', { type: 'button', 'data-zoom': 'out', 'data-focus': 'zoom:out', onclick: () => this.zoomBy(1 / 1.6) }, icon('minus', 16)),
        h('button', { type: 'button', 'data-zoom': 'reset', 'data-focus': 'zoom:reset', onclick: () => this.setBox(WORLD) }, icon('locate', 16)));
      this.el.append(this.zoom);
      this.bindInteraction();
    } else {
      this.svg.addEventListener('click', () => openMapPage());
    }
    this.svg.addEventListener('pointermove', (e) => { if (!this.dragging) this.hover(e); });
    this.svg.addEventListener('pointerleave', () => this.hideTip());
    if (typeof ResizeObserver === 'function') { this.resize = new ResizeObserver(() => this.drawMarkers()); this.resize.observe(this.svg); }
    this.relabel();
  }
  relabel() {
    const t = (k, ...a) => this.view.t(k, ...a);
    this.svg.setAttribute('aria-label', this.compact ? t('compactTitle') : t('mapAria'));
    if (this.zoom) for (const [b, k] of [['in', 'zoomIn'], ['out', 'zoomOut'], ['reset', 'zoomReset']]) {
      const el = this.zoom.querySelector(`[data-zoom="${b}"]`); el.setAttribute('aria-label', t(k)); el.setAttribute('title', t(k));
    }
  }
  destroy() { this.resize?.disconnect(); }
  unitsPerPx() { const w = this.svg.clientWidth || this.svg.getBoundingClientRect().width || 1000; return this.box.w / Math.max(1, w); }
  setBox(b) {
    this.box = clampView(b);
    this.svg.setAttribute('viewBox', `${this.box.x.toFixed(2)} ${this.box.y.toFixed(2)} ${this.box.w.toFixed(2)} ${this.box.h.toFixed(2)}`);
    this.drawMarkers();
  }
  zoomBy(f, px, py) { this.setBox(zoomView(this.box, f, px, py)); }
  toMap(clientX, clientY) {
    const r = this.svg.getBoundingClientRect();
    // preserveAspectRatio meet: the 2:1 view box fits the element's width (its aspect ratio is 2).
    const k = this.box.w / Math.max(1, r.width);
    return [this.box.x + (clientX - r.left) * k, this.box.y + (clientY - r.top - (r.height - r.width / 2) / 2) * k];
  }
  fitCountry(code) {
    const p = this.paths.get(code);
    if (!p) return;
    let b;
    try { b = p.getBBox(); } catch { return; }
    const w = Math.max(MIN_VIEW_W * 1.5, Math.max(b.width * 1.6, b.height * 3.2));
    this.setBox({ x: b.x + b.width / 2 - w / 2, y: b.y + b.height / 2 - w / 4, w });
  }
  bindInteraction() {
    const svg = this.svg;
    svg.addEventListener('wheel', (e) => { e.preventDefault(); const [px, py] = this.toMap(e.clientX, e.clientY); this.zoomBy(e.deltaY < 0 ? 1.25 : 0.8, px, py); }, { passive: false });
    svg.addEventListener('pointerdown', (e) => {
      if (e.button !== 0 && e.pointerType === 'mouse') return;
      this.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      this.start = { box: { ...this.box }, x: e.clientX, y: e.clientY, pinch: this.pinchDistance() };
      this.moved = false;
      try { svg.setPointerCapture(e.pointerId); } catch { /* not capturable */ }
    });
    svg.addEventListener('pointermove', (e) => {
      if (!this.pointers.has(e.pointerId)) return;
      this.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      const k = this.box.w / Math.max(1, svg.getBoundingClientRect().width);
      if (this.pointers.size >= 2) {
        const d = this.pinchDistance(), [a, b] = [...this.pointers.values()];
        if (this.start.pinch && d) { this.box = this.start.box; const [px, py] = this.toMap((a.x + b.x) / 2, (a.y + b.y) / 2); this.zoomBy(d / this.start.pinch, px, py); this.moved = true; }
        return;
      }
      const dx = e.clientX - this.start.x, dy = e.clientY - this.start.y;
      if (!this.moved && Math.hypot(dx, dy) < 4) return;
      this.moved = true; this.dragging = true; this.hideTip();
      this.setBox({ ...this.start.box, x: this.start.box.x - dx * k, y: this.start.box.y - dy * k });
    });
    const end = (e) => {
      if (!this.pointers.has(e.pointerId)) return;
      this.pointers.delete(e.pointerId);
      if (this.pointers.size === 1) { const [p] = this.pointers.values(); this.start = { box: { ...this.box }, x: p.x, y: p.y, pinch: 0 }; return; }
      const wasMoved = this.moved; this.dragging = false;
      if (!wasMoved && e.type === 'pointerup') this.click(e);
    };
    svg.addEventListener('pointerup', end); svg.addEventListener('pointercancel', end);
    svg.addEventListener('keydown', (e) => {
      if (e.target !== svg) return;
      const pan = this.box.w * 0.12;
      const act = { '+': () => this.zoomBy(1.6), '=': () => this.zoomBy(1.6), '-': () => this.zoomBy(1 / 1.6), _: () => this.zoomBy(1 / 1.6),
        0: () => this.setBox(WORLD), Home: () => this.setBox(WORLD), Escape: () => this.view.select(null),
        ArrowLeft: () => this.setBox({ ...this.box, x: this.box.x - pan }), ArrowRight: () => this.setBox({ ...this.box, x: this.box.x + pan }),
        ArrowUp: () => this.setBox({ ...this.box, y: this.box.y - pan / 2 }), ArrowDown: () => this.setBox({ ...this.box, y: this.box.y + pan / 2 }) }[e.key];
      if (act) { e.preventDefault(); act(); }
    });
  }
  pinchDistance() { if (this.pointers.size < 2) return 0; const [a, b] = [...this.pointers.values()]; return Math.hypot(a.x - b.x, a.y - b.y); }
  click(e) {
    const target = document.elementFromPoint(e.clientX, e.clientY);
    const marker = target?.closest?.('.pocol-marker');
    if (marker) return this.activateMarker(marker);
    const code = target?.closest?.('.map-country')?.getAttribute('data-country');
    this.view.select(code && code !== this.selected ? code : null);
  }
  activateMarker(marker) {
    const codes = marker.getAttribute('data-codes').split(' ');
    if (this.compact) return openMapPage();
    if (codes.length > 1) {
      const pts = codes.map((c) => this.view.shared.shapes.get(c)).map((c) => project(c.longitude, c.latitude));
      const xs = pts.map((p) => p[0]), ys = pts.map((p) => p[1]);
      const w = Math.max(MIN_VIEW_W, (Math.max(...xs) - Math.min(...xs)) * 2.2, (Math.max(...ys) - Math.min(...ys)) * 4.4, this.box.w / 2.5);
      this.setBox({ x: (Math.max(...xs) + Math.min(...xs)) / 2 - w / 2, y: (Math.max(...ys) + Math.min(...ys)) / 2 - w / 4, w });
      return;
    }
    this.view.select(codes[0]);
  }
  hover(e) {
    const target = e.target, marker = target.closest?.('.pocol-marker');
    const code = marker ? null : target.closest?.('.map-country')?.getAttribute('data-country');
    if (marker) this.showTip(marker.getAttribute('aria-label'), e.clientX, e.clientY);
    else if (code) this.showTip(this.view.countryLine(code), e.clientX, e.clientY);
    else this.hideTip();
    this.hoverName.textContent = code ? countryName(this.view.lang, code, this.view.shared) : '';
  }
  showTip(text, cx, cy) {
    const r = this.el.getBoundingClientRect();
    this.tip.hidden = false; this.tip.textContent = text;
    const w = this.tip.offsetWidth || 180;
    this.tip.style.left = `${Math.max(6, Math.min(r.width - w - 6, cx - r.left + 12))}px`;
    this.tip.style.top = `${Math.max(6, cy - r.top - 44)}px`;
  }
  hideTip() { this.tip.hidden = true; }
  setModel(model) { this.model = model; this.paint(); this.drawMarkers(); }
  paint() {
    const m = this.model;
    if (!m) return;
    for (const [code, p] of this.paths) {
      const n = m.values.get(code) || 0, level = levelOf(m.levels, n);
      p.setAttribute('class', 'map-country' + (level ? ` has-miners pocol-lv${level}` : '') + (this.selected === code ? ' selected' : ''));
    }
  }
  select(code) { this.selected = code; this.paint(); this.drawMarkers(); }
  drawMarkers() {
    const m = this.model;
    if (!m) return;
    const k = this.unitsPerPx(), shapes = this.view.shared.shapes;
    const points = [];
    for (const [code, n] of m.values) {
      const shape = shapes.get(code);
      if (!shape || n <= 0) continue;
      const [x, y] = project(shape.longitude, shape.latitude);
      points.push({ key: code, x, y, weight: n });
    }
    const groups = cluster(points, k, this.compact ? CLUSTER_PX * 0.8 : CLUSTER_PX);
    const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
    // A focused marker keeps the focus through the redraw: the marker of the same countries, else the one that now
    // holds the selected country or the first of them (a cluster split by a zoom).
    const focused = document.activeElement?.closest?.('.pocol-marker');
    const had = focused && this.markerLayer.contains(focused) ? focused.getAttribute('data-codes').split(' ') : null;
    this.markerLayer.replaceChildren();
    for (const g of groups) {
      const codes = g.members.map((p) => p.key), rows = codes.map((c) => m.byCode.get(c));
      const total = g.members.reduce((a, p) => a + p.weight, 0);
      const counts = m.layer === 'miners' ? rows.reduce((a, r) => ({ located: a.located + r.miners.located, active: a.active + r.miners.active, inactive: a.inactive + r.miners.inactive }), { located: 0, active: 0, inactive: 0 }) : null;
      const r = Math.min(15, 6.5 + 2.4 * Math.log2(total));
      const label = codes.length > 1 ? this.view.t('clusterAria', total, codes.length)
        : m.layer === 'miners' ? this.view.t('marker', countryName(this.view.lang, codes[0], this.view.shared), counts.located, counts.active - counts.inactive, counts.inactive)
          : this.view.t('markerNodes', countryName(this.view.lang, codes[0], this.view.shared), total);
      const sel = codes.includes(this.selected);
      const marker = s('g', { class: 'pocol-marker' + (sel ? ' is-selected' : '') + (codes.length > 1 ? ' is-cluster' : ''), transform: `translate(${g.x.toFixed(2)} ${g.y.toFixed(2)}) scale(${k.toFixed(5)})`,
        tabindex: this.compact ? null : 0, role: this.compact ? null : 'button', 'aria-label': label, 'data-codes': codes.join(' '),
        'data-focus': this.compact ? null : 'marker:' + codes.join(' ') });
      marker.append(s('circle', { class: 'pocol-marker-hit', r: Math.max(14, r + 8) }), s('circle', { class: 'pocol-marker-glow' + (reduced ? '' : ' is-animated'), r: r + 7 }));
      const core = markerCore(counts);
      marker.append(s('circle', { class: 'pocol-marker-core' + (core ? ' ' + core : ''), r }));
      if (counts) {
        const ring = r + 4.5, C = 2 * Math.PI * ring, segs = ringSegments(counts.located, counts.active, counts.inactive), gap = segs.length > 1 ? 2 : 0;
        for (const seg of segs) {
          const len = Math.max(0.5, seg.length * C - gap);
          marker.append(s('circle', { class: 'pocol-ring is-' + seg.kind, r: ring, 'stroke-dasharray': `${len.toFixed(2)} ${(C - len).toFixed(2)}`,
            'stroke-dashoffset': (-(seg.from * C) + C / 4).toFixed(2) }));
        }
      }
      if (total > 1 || codes.length > 1) marker.append(s('text', { class: 'pocol-marker-count', 'text-anchor': 'middle', dy: '0.35em', 'font-size': total >= 100 ? 8.5 : 10, text: fmtInt(total) }));
      if (!this.compact) {
        marker.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); this.activateMarker(marker); } });
        marker.addEventListener('focus', () => { const rect = marker.getBoundingClientRect(); this.showTip(label, rect.left + rect.width / 2, rect.top); });
        marker.addEventListener('blur', () => this.hideTip());
      }
      this.markerLayer.append(marker);
    }
    if (had) {
      const markers = [...this.markerLayer.querySelectorAll('.pocol-marker')], codesOf = (m) => m.getAttribute('data-codes').split(' ');
      const prefer = had.includes(this.selected) ? [this.selected, ...had] : had;
      const next = markers.find((m) => m.getAttribute('data-codes') === had.join(' '))
        || prefer.map((c) => markers.find((m) => codesOf(m).includes(c))).find(Boolean);
      (next || this.svg).focus({ preventScroll: true });
    }
  }
}

class MapView extends View {
  constructor(mount, compact) {
    super(mount);
    this.compact = compact; this.layer = null; this.selectedCode = null;
    this.render();
    if (this.shared) geo.subscribe(this);
  }
  destroy() { geo.unsubscribe(this); this.map?.destroy(); super.destroy(); }
  render() {
    this.frame();
    this.root.replaceChildren();
    this.map?.destroy(); this.map = null;
    if (!this.shared) { this.root.append(h('section', { class: 'panel' }, emptyState('alert', this.t('geoUnavailable'), this.t('noIdentity')))); return; }
    if (this.compact) {
      this.body = h('div', { class: 'pocol-compact-body' });
      this.root.append(h('section', { class: 'panel pocol-compact', 'aria-labelledby': 'pocol-compact-title' },
        h('div', { class: 'panel-heading' }, h('div', {}, h('h2', { id: 'pocol-compact-title' }, icon('globe', 18), this.t('compactTitle')), h('p', { class: 'pocol-compact-sub' })),
          h('div', { class: 'pocol-heading-end' }, languageSwitch(), h('button', { type: 'button', class: 'secondary', onclick: () => openMapPage() }, icon('open', 13), this.t('compactOpen')))),
        this.body));
    } else {
      this.body = h('div', { class: 'pocol-map-body' });
      this.root.append(h('div', { class: 'pocol-bar' }, h('p', { class: 'pocol-bar-text', text: this.t('mapBar') }), languageSwitch()), this.body);
    }
    this.update();
  }
  update() { this.keep(() => this._update()); }
  _update() {
    if (!this.body) return;
    const d = geo.data, err = geo.error;
    if (!d) {
      if (this.map) { this.map.destroy(); this.map = null; }
      this.body.replaceChildren(err ? h('section', { class: this.compact ? 'pocol-compact-error' : 'panel pocol-error', role: 'alert' },
        emptyState('alert', this.t('geoUnavailable'), errorText(this.lang, err), h('button', { type: 'button', class: 'text-button', onclick: () => geo.load() }, icon('refresh', 13), this.t('retry'))))
        : emptyState('refresh', this.t('loading'), null));
      return;
    }
    if (this.layer === null) this.layer = d.miners.located > 0 ? 'miners' : 'nodes';
    if (this.compact) return this.updateCompact(d, err);
    this.updateFull(d, err);
  }
  model(d) {
    const layer = this.layer, values = new Map(), byCode = new Map();
    for (const c of d.countries) { byCode.set(c.code, c); const n = layer === 'miners' ? c.miners.located : c.nodes; if (n > 0) values.set(c.code, n); }
    const max = Math.max(0, ...values.values());
    return { layer, values, byCode, levels: choroplethLevels(max), max };
  }
  ensureMap() {
    if (!this.map) this.map = new WorldMap(this, { compact: this.compact });
    return this.map;
  }
  countryLine(code) {
    const d = geo.data, c = d?.countries.find((x) => x.code === code), name = countryName(this.lang, code, this.shared);
    if (!c) return this.layer === 'miners' ? this.t('pNone', name) : this.t('markerNodes', name, 0);
    return this.layer === 'miners' ? this.t('marker', name, c.miners.located, c.miners.active - c.miners.inactive, c.miners.inactive) : this.t('markerNodes', name, c.nodes);
  }
  select(code) {
    this.selectedCode = code;
    this.keep(() => {
      this.map?.select(code);
      this.renderPanel();
      if (code && this.map && this.map.paths.has(code)) this.map.fitCountry(code);
    });
    // The country table's highlighted row follows the selection at once.
    for (const row of this.body?.querySelectorAll('.pocol-country-table tr[data-country]') || []) row.classList.toggle('is-selected', row.dataset.country === code);
  }
  updateCompact(d, err) {
    const map = this.ensureMap();
    map.setModel(this.model(d));
    this.root.querySelector('.pocol-compact-sub').textContent = this.t('compactSummary', d.miners.registered, d.miners.located, d.miners.unknown, d.nodes.observed);
    const note = d.miners.located === 0 ? h('p', { class: 'pocol-compact-note' }, icon('info', 13), h('span', { text: this.t('noReporter') })) : null;
    this.body.replaceChildren(err ? this.staleLine(err) : '', map.el, note || '', this.legend(d, true));
  }
  staleLine(err) { return h('div', { class: 'map-alert', role: 'status' }, icon('alert', 15), h('span', { text: errorText(this.lang, err) })); }
  updateFull(d, err) {
    const map = this.ensureMap(), model = this.model(d);
    map.setModel(model);
    map.select(this.selectedCode);
    const M = d.miners;
    const metrics = h('div', { class: 'metrics pocol-metrics' },
      metric(this.t('mRegistered'), h('span', { class: 'pocol-big' }, bdi(fmtInt(M.registered))), `${this.t('mRegisteredNote', M.active, M.inactive)} · ${this.t('exited', M.exited, M.punished)}`, true, 'users'),
      metric(this.t('mLocated'), h('span', { class: 'pocol-big' }, bdi(fmtInt(M.located))), this.t('mLocatedNote', M.unknown), false, 'pin'),
      metric(this.t('mConnected'), h('span', { class: 'pocol-big' }, bdi(fmtInt(M.connected))), this.t('mConnectedNote'), false, 'link'),
      metric(this.t('mNodes'), h('span', { class: 'pocol-big' }, bdi(fmtInt(d.nodes.observed))), this.t('mNodesNote', d.nodes.located, d.nodes.unlocated), false, 'server'));
    const layerSwitch = h('div', { class: 'pocol-segmented', role: 'group', 'aria-label': this.t('layerAria') },
      [['miners', 'layerMiners', 'users'], ['nodes', 'layerNodes', 'server']].map(([k, label, ic]) => h('button', { type: 'button', 'aria-pressed': String(this.layer === k), 'data-layer': k,
        'data-focus': 'layer:' + k, onclick: () => { if (this.layer !== k) { this.layer = k; this.update(); this.root.querySelector(`[data-layer="${k}"]`)?.focus(); } } }, icon(ic, 14), this.t(label))));
    this.panelBox = h('aside', { class: 'map-detail-list pocol-country-panel', 'aria-live': 'polite' });
    const mapPanel = h('section', { class: 'panel miner-map-panel pocol-map-panel', 'aria-labelledby': 'pocol-map-title' },
      h('div', { class: 'panel-heading' }, h('div', {}, h('h2', { id: 'pocol-map-title' }, icon('globe', 18), this.t('mapTitle')), h('p', { text: this.t('mapSub', d.height) })), layerSwitch),
      err ? this.staleLine(err) : null,
      M.located === 0 ? h('div', { class: 'map-alert pocol-info', role: 'note' }, icon('info', 15), h('span', { text: this.t('noReporter') })) : null,
      h('div', { class: 'map-with-sidebar' }, h('div', { class: 'map-layout' }, map.el), this.panelBox),
      this.legend(d, false),
      h('div', { class: 'map-note' }, icon('info', 14), h('div', {}, this.t('mapNotes').map((n) => h('p', { text: n })))));
    this.body.replaceChildren(metrics, mapPanel, this.countryTable(d), this.sourcesPanel(d));
    this.renderPanel();
    map.drawMarkers();
  }
  legend(d, compact) {
    const m = this.model(d), items = [];
    if (this.layer === 'miners') {
      items.push(h('span', {}, h('i', { class: 'pocol-key is-active', 'aria-hidden': 'true' }), this.t('legActive')),
        h('span', {}, h('i', { class: 'pocol-key is-inactive', 'aria-hidden': 'true' }), this.t('legInactive', d.miners.thetaInact)));
      const notActive = d.countries.some((c) => c.miners.located > c.miners.active);
      if (!compact || notActive) items.push(h('span', {}, h('i', { class: 'pocol-key is-other', 'aria-hidden': 'true' }), this.t('legOther')));
    } else items.push(h('span', {}, h('i', { class: 'pocol-key is-active', 'aria-hidden': 'true' }), this.t('legNodes')));
    if (!compact) items.push(h('span', {}, h('b', { 'aria-hidden': 'true', text: '2' }), this.t('legCluster')));
    if (m.levels.length && !compact) items.push(h('span', { class: 'pocol-scale' }, h('span', { text: this.t(this.layer === 'miners' ? 'legScaleMiners' : 'legScaleNodes') }),
      m.levels.map((l) => h('span', { class: 'pocol-scale-step' }, h('i', { class: 'pocol-lv' + l.level, 'aria-hidden': 'true' }), bdi(l.lo === l.hi ? fmtInt(l.lo) : `${fmtInt(l.lo)}–${fmtInt(l.hi)}`)))));
    const [a, ne, b, db] = this.t('attribution');
    items.push(h('span', { class: 'map-attribution' }, a, h('a', { href: 'https://www.naturalearthdata.com/', target: '_blank', rel: 'noreferrer', text: ne }),
      d.database ? [b, h('a', { href: 'https://db-ip.com', target: '_blank', rel: 'noreferrer', text: db })] : null));
    return h('div', { class: 'map-legend' }, items);
  }
  renderPanel() {
    const box = this.panelBox, d = geo.data;
    if (!box || !d) return;
    const code = this.selectedCode;
    if (!code) {
      box.replaceChildren(h('div', { class: 'map-detail-heading' }, h('span', { text: this.t('kicker') }), h('p', { text: this.t('pickHint') })));
      return;
    }
    const name = countryName(this.lang, code, this.shared), c = d.countries.find((x) => x.code === code);
    const rows = [];
    if (c) {
      rows.push([this.t('pLocated'), bdi(fmtInt(c.miners.located))], [this.t('pShare'), percent(shareE10(c.miners.located, d.miners.registered))],
        [this.t('pActive'), bdi(fmtInt(c.miners.active - c.miners.inactive))], [this.t('pInactive'), bdi(fmtInt(c.miners.inactive))],
        [this.t('pConnected'), bdi(fmtInt(c.miners.connected))], [this.t('pNodes'), bdi(fmtInt(c.nodes))],
        [this.t('pBlocks', d.window.blocks), bdi(fmtInt(c.blocks))], [this.t('pBlocksShare'), percent(c.blocksShare)],
        [this.t('pRewards'), c.rewardsWei === null ? h('span', { class: 'muted', title: this.t('rewardsNull'), text: '—' }) : amount(c.rewardsWei, 4)]);
    }
    box.replaceChildren(h('div', { class: 'map-detail-heading' }, h('span', { text: this.t('kicker') }), h('h3', { text: name }),
      !c || c.miners.located === 0 ? h('p', { text: this.t('pNone', name) }) : null, this.shared.shapes.has(code) ? null : h('p', { text: this.t('pNoShape') })),
    rows.length ? h('dl', { class: 'pocol-country-stats' }, rows.map(([k, v]) => h('div', {}, h('dt', { text: k }), h('dd', {}, v)))) : null,
    h('div', { class: 'pocol-panel-actions' }, this.shared.shapes.has(code) ? h('button', { type: 'button', class: 'text-button', 'data-focus': 'panel:zoom', onclick: () => this.map?.fitCountry(code) }, icon('locate', 13), this.t('zoomTo')) : null,
      h('button', { type: 'button', class: 'text-button', 'data-focus': 'panel:clear', onclick: () => this.select(null) }, this.t('clear'))));
  }
  countryTable(d) {
    const reg = d.miners.registered, sel = (code) => () => { this.select(code); this.map?.el.scrollIntoView({ block: 'nearest', behavior: 'smooth' }); };
    const rows = d.countries.map((c) => h('tr', { class: this.selectedCode === c.code ? 'is-selected' : null, 'data-country': c.code },
      h('td', {}, h('button', { type: 'button', class: 'hash-link pocol-country-link', 'data-focus': 'country:' + c.code, 'aria-label': this.t('tSelect', countryName(this.lang, c.code, this.shared)), onclick: sel(c.code) },
        h('span', { class: 'pocol-code', text: c.code }), countryName(this.lang, c.code, this.shared))),
      h('td', { class: 'right' }, bdi(fmtInt(c.miners.located))), h('td', { class: 'right' }, percent(shareE10(c.miners.located, reg))),
      h('td', { class: 'right' }, bdi(fmtInt(c.miners.active - c.miners.inactive))), h('td', { class: 'right' }, bdi(fmtInt(c.miners.inactive))),
      h('td', { class: 'right' }, bdi(fmtInt(c.miners.connected))), h('td', { class: 'right' }, bdi(fmtInt(c.nodes))),
      h('td', { class: 'right' }, bdi(fmtInt(c.blocks))), h('td', { class: 'right' }, percent(c.blocksShare)),
      h('td', { class: 'right' }, c.rewardsWei === null ? h('span', { class: 'muted', title: this.t('rewardsNull'), text: '—' }) : amount(c.rewardsWei, 2))));
    rows.push(h('tr', { class: 'pocol-unknown-row' }, h('td', { text: this.t('tUnknown') }),
      h('td', { class: 'right' }, bdi(fmtInt(d.unknown.miners))), h('td', { class: 'right' }, percent(shareE10(d.unknown.miners, reg))),
      h('td', { class: 'right' }, bdi(fmtInt(d.unknownActive - d.unknownInactive))), h('td', { class: 'right' }, bdi(fmtInt(d.unknownInactive))),
      h('td', { class: 'right' }, bdi(fmtInt(d.unknownConnected))), h('td', { class: 'right' }, bdi(fmtInt(d.nodes.unlocated))),
      h('td', { class: 'right' }, bdi(fmtInt(d.unknown.blocks))), h('td', { class: 'right' }, percent(shareE10(d.unknown.blocks, d.window.blocks))),
      h('td', { class: 'right muted', text: '—' })));
    const cols = [['tCountry'], ['tLocated', 1], ['tPct', 1], ['tActive', 1], ['tInactive', 1], ['tConnected', 1], ['tNodes', 1], ['tBlocks', 1], ['tBlocksPct', 1], ['tRewards', 1]];
    return h('section', { class: 'panel pocol-country-table', 'aria-labelledby': 'pocol-country-title' },
      h('div', { class: 'panel-heading' }, h('div', {}, h('h2', { id: 'pocol-country-title' }, icon('pin', 18), this.t('tTitle')), h('p', { text: this.t('tSub') }))),
      h('div', { class: 'table-wrap' }, h('table', {}, h('caption', { class: 'sr-only', text: this.t('tTitle') }),
        h('thead', {}, h('tr', {}, cols.map(([k, r]) => h('th', { scope: 'col', class: r ? 'right' : null, text: k === 'tBlocks' ? this.t(k, d.window.blocks) : this.t(k) })))),
        h('tbody', {}, rows))),
      d.countries.length ? null : h('p', { class: 'table-foot', text: this.t('tEmpty') }));
  }
  sourcesPanel(d) {
    const rows = d.sources.map((x) => h('tr', {}, h('td', {}, bdi(x.name)), h('td', { text: this.t(x.kind === 'observer' ? 'sObserver' : 'sOperator') }),
      h('td', {}, h('span', { class: 'badge ' + (x.fresh ? 'success' : 'neutral'), text: x.reportedAt === null ? this.t('sNever') : x.fresh ? this.t('sFresh') : this.t('sStale') })),
      h('td', { text: x.reportedAt === null ? '—' : `${timeText(this.lang, x.reportedAt)} · ${this.t('sAge', Math.max(0, d.generatedAt - x.reportedAt))}` })));
    const facts = [[this.t('sAnswer'), this.t('sAnswerValue', d.height, timeText(this.lang, d.generatedAt, true))],
      [this.t('sDbLabel'), d.database ? this.t('sDb', d.database.provider, d.database.release, d.database.license) : this.t('sNoDb')],
      [this.t('sWindow'), this.t('sWindowValue', d.window.from, d.window.to, d.window.blocks)]];
    return h('section', { class: 'panel pocol-sources', 'aria-labelledby': 'pocol-sources-title' },
      h('div', { class: 'panel-heading' }, h('div', {}, h('h2', { id: 'pocol-sources-title' }, icon('server', 18), this.t('sTitle')))),
      h('dl', { class: 'pocol-facts' }, facts.map(([k, v]) => h('div', {}, h('dt', { text: k }), h('dd', { text: v })))),
      h('div', { class: 'table-wrap' }, h('table', {}, h('caption', { class: 'sr-only', text: this.t('sTitle') }),
        h('thead', {}, h('tr', {}, [this.t('sSource'), this.t('sKind'), this.t('sStatus'), this.t('sLast')].map((c) => h('th', { scope: 'col', text: c })))),
        h('tbody', {}, rows))));
  }
}

// ---------------------------------------------------------------------------------------------------------------
// Mounting: the bundle's mount points come and go with its tabs.
const MOUNTS = { 'pocol-holders': (el) => new HoldersView(el), 'pocol-miners-map': (el) => new MapView(el, false), 'pocol-miners-map-compact': (el) => new MapView(el, true) };
const mounted = new Map();
function scan() {
  for (const [el, view] of mounted) if (!el.isConnected) { view.destroy(); mounted.delete(el); }
  for (const [id, make] of Object.entries(MOUNTS)) {
    const el = document.getElementById(id);
    if (el && !mounted.has(el)) mounted.set(el, make(el));
  }
}
function start() {
  language = readLanguage();
  scan();
  new MutationObserver(scan).observe(document.body, { childList: true, subtree: true });
}
if (typeof document !== 'undefined' && typeof MutationObserver === 'function') {
  if (document.body) start(); else document.addEventListener('DOMContentLoaded', start, { once: true });
}

export const TEXTS = TEXT;
