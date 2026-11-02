(() => {
  'use strict';
  const guide = window.POCOL_GUIDE;
  const id = document.body.dataset.topic;
  const topic = guide.topics.find(item => item.id === id);
  if (!topic) return;
  const index = guide.topics.indexOf(topic);
  const article = document.querySelector('#docs-article');
  const layout = document.querySelector('.docs-layout');
  const search = document.querySelector('#docs-search');
  const sidebar = document.querySelector('.docs-sidebar');
  const mobileToggle = document.querySelector('#docs-mobile-toggle');
  const symbols = ['◇','◎','▤','⌘','</>','◉','↗','✓'];
  const wording = {
    en: {home:'Home',about:'About PoCol',browse:'Browse the guide',search:'Search topics',find:'Find a topic…',empty:'No matching topics. Try a different term.',repo:'View on GitHub ↗',read:'Read in',status:'Experimental implementation',statusLink:'Project status ↗',onPage:'On this page',top:'Back to top ↑',sources:'Sources & further reading',snapshot:'References checked · 30 October 2026 · repository links pin their source revision.',previous:'← Previous',next:'Next →',explore:'Choose your next chapter',labTitle:'One template. Separate search ranges.',labIntro:'Illustration only · a 1,600-value nonce space. Change the number of miners to see how the assignment changes.',miners:'Miners',nonce:'Nonce range',overlap:'Overlapping values',covered:'Assigned values',labSteps:['1. Common template','2. Assign ranges','3. Search & share','4. Verify the block'],labText:['All participants begin with the same candidate block template. The transactions, timestamp and target define the shared work.','A deterministic ordering gives each eligible miner one contiguous range. The ranges cover the nonce space without overlap.','Each miner hashes within its own range. A share meets an easier target; a winning solution must meet the block target.','Nodes verify the winning proof, range ownership, timing, signatures and execution. Accepted work becomes part of the chain.'],miner:'Miner',related:'Continue exploring',proofNote:'Shares sample work; they do not prove that a miner searched every value. This illustration does not model real mining speed or rewards.'},
    ar: {home:'الرئيسية',about:'عن PoCol',browse:'تصفّح الدليل',search:'البحث في الموضوعات',find:'ابحث عن موضوع…',empty:'لا توجد موضوعات مطابقة. جرّب كلمة أخرى.',repo:'المصدر على GitHub ↗',read:'لغة القراءة',status:'تنفيذ تجريبي',statusLink:'حالة المشروع ↗',onPage:'في هذه الصفحة',top:'العودة للأعلى ↑',sources:'المصادر وللمزيد من القراءة',snapshot:'مراجع راجعتها الصفحة في 30 أكتوبر 2026؛ ترتبط ملفات المستودع بإصداراتها المحددة.',previous:'→ السابق',next:'التالي ←',explore:'اختر موضوعك التالي',labTitle:'قالب واحد. نطاقات بحث منفصلة.',labIntro:'مثال توضيحي فقط · مساحة بحث من 1,600 قيمة nonce. غيّر عدد المعدّنين لترى كيف يتغيّر توزيع النطاقات.',miners:'عدد المعدّنين',nonce:'نطاق nonce',overlap:'القيم المتداخلة',covered:'القيم الموزّعة',labSteps:['1. قالب موحّد','2. توزيع النطاقات','3. البحث والمساهمة','4. التحقق من الكتلة'],labText:['يبدأ المشاركون بقالب كتلة مرشحة واحد؛ تحدد المعاملات والتوقيت والهدف العمل المشترك.','يعطي ترتيب حتمي لكل معدّن مؤهل نطاقًا متجاورًا. تغطي النطاقات مساحة البحث دون تداخل.','يحسب كل معدّن التجزئات داخل نطاقه. تحقق المساهمة هدفًا أسهل، بينما يجب أن يحقق الحل الفائز هدف الكتلة.','تتحقق العقد من إثبات العمل وملكية النطاق والتوقيت والتوقيعات والتنفيذ، ثم يُضاف العمل المقبول إلى السلسلة.'],miner:'المعدّن',related:'واصل الاستكشاف',proofNote:'المساهمات عينات من العمل، وليست دليلًا على فحص كل قيمة. لا يحاكي هذا المثال سرعة التعدين الفعلية أو المكافآت.'}
  };
  const escape = text => String(text).replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
  function safeLink(value) {
    if (/^https:\/\//i.test(value)) {
      const url = new URL(value);
      if (!url.username && !url.password) return value;
    } else if (value && !/^[a-z][a-z0-9+.-]*:/i.test(value) && !/^[\/\\]/.test(value) && !value.includes('\\')) return value;
    throw new Error('Unsupported guide link');
  }
  function sourceLink(source) {
    if (source.url) return safeLink(source.url);
    const ref = source.ref || guide.sha;
    if (!/^[0-9a-f]{40}$/.test(ref)) throw new Error('Source references must pin a full commit SHA');
    return `https://github.com/palcertcom/pocol/blob/${ref}/${source.path}${source.anchor ? '#'+source.anchor : ''}`;
  }
  let language = 'en';
  let observer;
  function link(item) { return guide.files[item.id]; }
  function render() {
    const selected = document.documentElement.lang;
    language = selected === 'ar' ? 'ar' : 'en';
    const w = wording[language];
    const text = value => escape(value[language]);
    layout.lang = language;
    layout.dir = language === 'ar' ? 'rtl' : 'ltr';
    document.title = `${topic.title[language]} · PoCol`;
    document.querySelector('meta[name="description"]').content = topic.intro[language];
    document.querySelector('.docs-home').textContent = w.about;
    mobileToggle.innerHTML = `${w.browse}<span aria-hidden="true">⌄</span>`;
    document.querySelector('.docs-search > span').textContent = w.search;
    search.placeholder = w.find;
    search.value = '';
    document.querySelector('.docs-empty').textContent = w.empty;
    document.querySelector('.docs-repo').textContent = w.repo;
    document.querySelector('.docs-topics').setAttribute('aria-label',w.about);
    sidebar.querySelectorAll('a[data-topic]').forEach(anchor => {
      anchor.querySelector('.topic-name').textContent = window.POCOL_LOCALES[language][guide.keys[anchor.dataset.topic]];
      anchor.hidden = false;
    });
    document.querySelector('.docs-empty').hidden = true;
    const fallback = !['ar','en'].includes(selected) ? `<span class="reading-availability" lang="${escape(selected)}" dir="auto">${escape(window.POCOL_LOCALES[selected].aboutReadingNotice)}</span>` : '';
    document.querySelector('.docs-language-note').innerHTML = `${fallback}<span>${w.read}</span><a href="?lang=en" lang="en" ${language === 'en' ? 'aria-current="true"' : ''}>English</a><a href="?lang=ar" lang="ar" ${language === 'ar' ? 'aria-current="true"' : ''}>العربية</a>`;
    document.querySelector('.docs-breadcrumb').innerHTML = `<a href="index.html">${w.home}</a><span>/</span><a href="about.html">${w.about}</a><span>/</span><span>${text(topic.title)}</span>`;
    document.querySelector('.docs-status').innerHTML = `<span>${w.status}</span><a href="pocol-status.html">${w.statusLink}</a>`;
    const sections = topic.sections.map(section => `<section class="doc-section" id="${escape(section.id)}"><h2>${text(section.title)}</h2>${(section.paragraphs || []).map(p=>`<p>${text(p)}</p>`).join('')}${section.bullets?.length ? `<ul>${section.bullets.map(b=>`<li>${text(b)}</li>`).join('')}</ul>` : ''}${section.links?.length ? `<div class="actions">${section.links.map(l=>`<a class="button outline" href="${escape(safeLink(l.url))}">${text(l.label)}</a>`).join('')}</div>` : ''}</section>`).join('');
    article.innerHTML = `<div class="doc-intro"><span class="doc-number">${String(index+1).padStart(2,'0')} / 08</span><span class="eyebrow">${text(topic.eyebrow)}</span><h1 tabindex="-1">${text(topic.title)}</h1><p>${text(topic.intro)}</p></div>${id==='mining' ? miningMarkup(w) : ''}${sections}${id==='overview' ? `<section class="doc-section" id="chapters"><h2>${w.explore}</h2><div class="topic-grid">${guide.topics.slice(1).map((t,i)=>`<a class="topic-card" href="${link(t)}"><span class="topic-num">${String(i+2).padStart(2,'0')}</span><span class="topic-symbol" aria-hidden="true">${escape(symbols[i+1])}</span><h3>${text(t.title)}</h3><p>${text(t.eyebrow)}</p><span class="topic-arrow" aria-hidden="true">↗</span></a>`).join('')}</div></section>` : ''}`;
    const sourceDetails = document.querySelector('.doc-sources');
    sourceDetails.innerHTML = `<summary>${w.sources}</summary><p>${w.snapshot}</p><ol>${topic.sources.map(s => `<li><a href="${escape(sourceLink(s))}">${text(s.label)} <span aria-hidden="true">↗</span></a>${s.url ? '' : ` · <code>${escape((s.ref || guide.sha).slice(0,7))}</code>`}</li>`).join('')}</ol>`;
    const pagination = document.querySelector('.doc-pagination');
    pagination.setAttribute('aria-label',w.related);
    pagination.innerHTML = [[index-1,w.previous],[index+1,w.next]].filter(([i])=>i>=0&&i<guide.topics.length).map(([i,label])=>`<a href="${link(guide.topics[i])}"><small>${label}</small><strong>${text(guide.topics[i].title)}</strong></a>`).join('');
    document.querySelector('.docs-toc > span').textContent = w.onPage;
    document.querySelector('.back-top').textContent = w.top;
    const toc = document.querySelector('.toc');
    toc.setAttribute('aria-label',w.onPage);
    toc.innerHTML = `${id==='mining' ? `<a href="#mining-lab">${w.labTitle}</a>` : ''}${topic.sections.map(s=>`<a href="#${escape(s.id)}">${text(s.title)}</a>`).join('')}${id==='overview' ? `<a href="#chapters">${w.explore}</a>` : ''}`;
    observer?.disconnect();
    if ('IntersectionObserver' in window) {
      observer = new IntersectionObserver(entries => {
        const active = entries.find(entry=>entry.isIntersecting);
        if (!active) return;
        toc.querySelectorAll('a').forEach(anchor=>anchor.setAttribute('aria-current', String(anchor.hash === '#'+active.target.id)));
      }, {rootMargin:'-10% 0px -60% 0px'});
      article.querySelectorAll('.doc-section, #mining-lab').forEach(section=>observer.observe(section));
    }
    if (id === 'mining') initMining(w);
    updateProgress();
  }
  function miningMarkup(w) {
    return `<section class="mining-lab" id="mining-lab" aria-labelledby="lab-title"><div class="lab-heading"><span class="eyebrow">POCOL / NONCE</span><h2 id="lab-title">${w.labTitle}</h2><p>${w.labIntro}</p></div><div class="lab-controls"><label for="miner-count">${w.miners}</label><input type="range" id="miner-count" min="2" max="8" value="4"><output for="miner-count" id="miner-output">4</output></div><div class="nonce-track" dir="ltr" aria-label="${w.nonce}"></div><div class="lab-stats"><span>${w.covered} <strong dir="ltr">1,600 / 1,600</strong></span><span>${w.overlap} <strong>0</strong></span></div><div class="lab-steps" role="group" aria-label="${w.labTitle}">${w.labSteps.map((step,i)=>`<button type="button" data-step="${i}" aria-pressed="${i===0}">${step}</button>`).join('')}</div><p class="lab-explanation" role="status" aria-live="polite">${w.labText[0]}</p><p class="lab-footnote">${w.proofNote}</p></section>`;
  }
  function initMining(w) {
    const slider = document.querySelector('#miner-count');
    const colors = ['#20abe2','#71cbbf','#a5b4fc','#f4bf76','#ec9cbd','#80bde8','#bdc789','#c3a3e7'];
    function distribute() {
      const n = Number(slider.value);
      document.querySelector('#miner-output').value = n;
      slider.setAttribute('aria-valuetext',`${n} ${w.miners}`);
      document.querySelector('.nonce-track').innerHTML = Array.from({length:n},(_,i)=>{
        const start=Math.floor(i*1600/n), end=Math.floor((i+1)*1600/n)-1;
        return `<div class="nonce-range" style="--range-color:${colors[i]};flex-grow:${end-start+1}" title="${w.miner} ${i+1}: ${start}–${end}"><b>${i+1}</b><span>${start}<br>–${end}</span></div>`;
      }).join('');
    }
    slider.addEventListener('input',distribute);
    distribute();
    document.querySelectorAll('[data-step]').forEach(button=>button.addEventListener('click',()=>{
      document.querySelectorAll('[data-step]').forEach(b=>b.setAttribute('aria-pressed',String(b===button)));
      document.querySelector('.lab-explanation').textContent=w.labText[Number(button.dataset.step)];
    }));
  }
  search.addEventListener('input',()=>{
    const query=search.value.trim().toLocaleLowerCase();
    let visible=0;
    sidebar.querySelectorAll('a[data-topic]').forEach(anchor=>{
      const t=guide.topics.find(item=>item.id===anchor.dataset.topic);
      const haystack=[...Object.values(t.title),...Object.values(t.intro), t.id].join(' ').toLocaleLowerCase();
      anchor.hidden = !haystack.includes(query);
      if(!anchor.hidden) visible++;
    });
    document.querySelector('.docs-empty').hidden=visible!==0;
  });
  mobileToggle.addEventListener('click',()=>{
    const open=sidebar.classList.toggle('is-open');
    mobileToggle.setAttribute('aria-expanded',String(open));
  });
  sidebar.addEventListener('keydown',event=>{
    if(event.key==='Escape'&&sidebar.classList.contains('is-open')) {
      sidebar.classList.remove('is-open');mobileToggle.setAttribute('aria-expanded','false');mobileToggle.focus();
    }
  });
  function updateProgress() {
    const scrollable=document.documentElement.scrollHeight-innerHeight;
    document.querySelector('.reading-progress').style.width=`${scrollable>0 ? Math.min(100,scrollY/scrollable*100) : 100}%`;
  }
  window.addEventListener('scroll',updateProgress,{passive:true});
  window.addEventListener('pocol:language',render);
  render();
  // Restore deep links after the language-specific content has been rendered.
  if(location.hash) requestAnimationFrame(()=>document.getElementById(decodeURIComponent(location.hash.slice(1)))?.scrollIntoView());
})();
