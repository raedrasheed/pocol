(() => {
  'use strict';
  const root = document.documentElement;
  const dictionaries = window.POCOL_LOCALES;
  const language = document.querySelector('#language');
  const theme = document.querySelector('#theme');
  const menu = document.querySelector('#menu');
  const nav = document.querySelector('#nav');
  const dialog = document.querySelector('#dialog');
  const system = matchMedia('(prefers-color-scheme: dark)');
  const read = (key, fallback) => { try { return localStorage.getItem(key) || fallback; } catch { return fallback; } };
  const save = (key, value) => { try { localStorage.setItem(key, value); } catch {} };
  const supported = value => Object.prototype.hasOwnProperty.call(dictionaries, value);
  const requested = new URL(location.href).searchParams.get('lang');
  let lang = requested || read('pocol-language', 'en');
  if (!supported(lang)) lang = 'en';
  let activeModal = null;
  let network = null;

  const preference = read('pocol-theme', 'dark');
  theme.value = ['dark', 'light', 'system'].includes(preference) ? preference : 'dark';
  function applyTheme() {
    root.dataset.theme = theme.value === 'system' ? (system.matches ? 'dark' : 'light') : theme.value;
    document.querySelector('meta[name="theme-color"]').content = root.dataset.theme === 'dark' ? '#060e18' : '#f9fcff';
    save('pocol-theme', theme.value);
  }
  theme.addEventListener('change', applyTheme);
  system.addEventListener('change', applyTheme);
  applyTheme();

  function renderModal() {
    if (!activeModal) return;
    document.querySelector('#dialog-title').textContent = dictionaries[lang][`${activeModal}Title`];
    document.querySelector('#dialog-body').textContent = dictionaries[lang][`${activeModal}Body`];
  }
  function renderDownloads() {
    const text = dictionaries[lang];
    const phrase = key => text[key] ?? dictionaries.en[key];
    document.querySelectorAll('[data-wallet-platform]').forEach(link => {
      const configured = window.POCOL_DOWNLOADS?.[link.dataset.walletPlatform];
      const release = typeof configured === 'string' ? { url: configured } : configured;
      let url = null;
      try {
        const candidate = new URL(release?.url);
        if (candidate.protocol === 'https:') url = candidate.href;
      } catch {}
      const label = link.querySelector('[data-i18n]');
      label.dataset.i18n = url ? (link.dataset.downloadLabel || 'walletDownload') : 'appsUnavailable';
      label.textContent = phrase(label.dataset.i18n);
      link.setAttribute('aria-label', `${label.textContent} — ${phrase(link.dataset.platformLabel)}`);
      const metadata = document.querySelector(`[data-download-meta="${link.dataset.walletPlatform}"]`);
      if (metadata) metadata.textContent = url ? (release?.size || '') : '';
      if (url) {
        link.href = url;
        link.removeAttribute('aria-disabled');
        link.removeAttribute('tabindex');
      } else {
        link.removeAttribute('href');
        link.setAttribute('aria-disabled', 'true');
        link.setAttribute('tabindex', '-1');
      }
    });
  }
  const languagePicker = window.createPoColLanguagePicker?.({
    language,
    getLanguage: () => lang,
    getStrings: () => dictionaries[lang],
    beforeOpen: closeMenu
  }) || { refresh() {} };
  function translate() {
    const text = dictionaries[lang];
    root.lang = lang;
    root.dir = ['ar', 'ug', 'he'].includes(lang) ? 'rtl' : 'ltr';
    language.value = lang;
    const studio = document.querySelector('#studio');
    if (studio) { studio.lang = lang === 'ar' ? 'ar' : 'en'; studio.dir = lang === 'ar' ? 'rtl' : 'ltr'; }
    const ide = document.querySelector('#ide');
    if (ide) { ide.lang = lang === 'ar' ? 'ar' : 'en'; ide.dir = lang === 'ar' ? 'rtl' : 'ltr'; }
    const walletSection = document.querySelector('#wallet-downloads');
    if (walletSection) { walletSection.lang = lang === 'ar' ? 'ar' : 'en'; walletSection.dir = lang === 'ar' ? 'rtl' : 'ltr'; }
    document.querySelectorAll('[data-i18n]').forEach(element => {
      // Local dictionaries contain only text and the existing br/em markup.
      const translated = text[element.dataset.i18n] ?? dictionaries.en[element.dataset.i18n];
      if (translated !== undefined) element.innerHTML = translated;
    });
    document.querySelectorAll('[data-i18n-label]').forEach(element => {
      element.label = text[element.dataset.i18nLabel];
    });
    languagePicker.refresh();
    network?.refreshLanguage();
    menu.setAttribute('aria-label', text.menu);
    nav.setAttribute('aria-label', text.navigation);
    document.querySelector('#close').setAttribute('aria-label', text.close);
    document.title = text.pageTitle;
    document.querySelector('meta[name="description"]').content = text.metaDescription;
    save('pocol-language', lang);
    renderDownloads();
    renderModal();
    window.dispatchEvent(new CustomEvent('pocol:language', {detail:{language:lang}}));
  }
  language.addEventListener('change', () => {
    if (!supported(language.value)) return;
    lang = language.value;
    translate();
    try {
      const url = new URL(location.href);
      url.searchParams.set('lang', lang);
      history.replaceState(null, '', url);
    } catch {}
  });
  translate();
  network = window.createPoColNetwork?.({ getLanguage: () => lang, getStrings: () => dictionaries[lang] });
  document.querySelector('#year').textContent = new Date().getFullYear();

  function closeMenu() {
    nav.classList.remove('open');
    menu.setAttribute('aria-expanded', 'false');
  }
  menu.addEventListener('click', () => {
    menu.setAttribute('aria-expanded', String(nav.classList.toggle('open')));
  });
  nav.querySelectorAll('a').forEach(link => link.addEventListener('click', closeMenu));
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape' && nav.classList.contains('open')) {
      closeMenu();
      menu.focus();
    }
  });
  document.querySelectorAll('[data-modal]').forEach(button => button.addEventListener('click', () => {
    activeModal = button.dataset.modal;
    renderModal();
    dialog.showModal();
  }));
  dialog.setAttribute('aria-labelledby', 'dialog-title');
  dialog.setAttribute('aria-describedby', 'dialog-body');
  ['close', 'done'].forEach(id => document.getElementById(id).addEventListener('click', () => dialog.close()));
  dialog.addEventListener('close', () => { activeModal = null; });
  dialog.addEventListener('click', event => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) dialog.close();
  });
})();
