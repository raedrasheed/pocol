(() => {
  'use strict';

  // Enhance the native selector only when a complete modal chooser is available.
  window.createPoColLanguagePicker = ({ language, getLanguage, getStrings, beforeOpen }) => {
    const trigger = document.querySelector('#language-trigger');
    const current = document.querySelector('#language-current');
    const dialog = document.querySelector('#language-dialog');
    const title = document.querySelector('#language-dialog-title');
    const close = document.querySelector('#language-close');
    const search = document.querySelector('#language-search');
    const results = document.querySelector('#language-results');
    const empty = document.querySelector('#language-empty');
    const fallback = language?.closest('.language-label');
    if (!language || !trigger || !current || !dialog || !title || !close || !search || !results || !empty || !fallback || typeof dialog.showModal !== 'function') {
      return { refresh() {} };
    }

    const normalize = value => String(value).normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLocaleLowerCase().trim();
    const makeDisplayNames = locale => {
      try { return new Intl.DisplayNames([locale], { type: 'language' }); } catch { return null; }
    };
    const displayName = (names, code) => {
      try { return names?.of(code) || code; } catch { return code; }
    };
    const englishNames = makeDisplayNames('en');
    const groups = [];
    let pointerStartedOutside = false;

    Array.from(language.querySelectorAll('optgroup')).forEach((source, index) => {
      const group = document.createElement('section');
      group.className = 'language-group';
      const heading = document.createElement('h3');
      heading.className = 'language-group-title';
      heading.id = `language-group-${index}`;
      const options = document.createElement('div');
      options.className = 'language-options';
      options.setAttribute('role', 'group');
      options.setAttribute('aria-labelledby', heading.id);
      const entries = Array.from(source.querySelectorAll('option')).map(option => {
        const code = option.value;
        const nativeName = option.textContent.trim();
        const englishName = displayName(englishNames, code);
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'language-option';
        button.dataset.language = code;
        const names = document.createElement('span');
        names.className = 'language-name';
        const native = document.createElement('span');
        native.className = 'language-native';
        native.textContent = nativeName;
        native.lang = option.lang || code;
        native.dir = option.dir || 'auto';
        const english = document.createElement('span');
        english.className = 'language-english';
        english.textContent = normalize(englishName) === normalize(nativeName) ? code.toUpperCase() : englishName;
        english.lang = 'en';
        english.dir = 'ltr';
        const check = document.createElement('span');
        check.className = 'language-check';
        check.textContent = '✓';
        check.setAttribute('aria-hidden', 'true');
        names.append(native, english);
        button.append(names, check);
        button.addEventListener('click', () => {
          language.value = code;
          language.dispatchEvent(new Event('change', { bubbles: true }));
          dialog.close();
        });
        options.append(button);
        return { code, nativeName, englishName, button, check, searchText: '' };
      });
      group.append(heading, options);
      results.append(group);
      groups.push({ source, group, heading, entries });
    });

    function filter() {
      const query = normalize(search.value);
      let count = 0;
      groups.forEach(({ group, entries }) => {
        let matches = 0;
        entries.forEach(entry => {
          entry.button.hidden = !entry.searchText.includes(query);
          if (!entry.button.hidden) matches += 1;
        });
        group.hidden = matches === 0;
        count += matches;
      });
      empty.hidden = count !== 0;
      results.scrollTop = 0;
    }

    function refresh() {
      const code = getLanguage();
      const strings = getStrings();
      const localizedNames = makeDisplayNames(code);
      const selected = Array.from(language.options).find(option => option.value === code);
      current.textContent = selected?.textContent.trim() || code;
      current.lang = code;
      current.dir = selected?.dir || 'auto';
      trigger.setAttribute('aria-label', `${strings.language}: ${current.textContent}`);
      title.textContent = strings.language;
      close.setAttribute('aria-label', strings.close);
      search.placeholder = strings.languageSearch;
      search.setAttribute('aria-label', strings.languageSearch);
      empty.textContent = strings.languageNoResults;
      groups.forEach(({ source, heading, entries }) => {
        heading.textContent = source.label;
        entries.forEach(entry => {
          const selected = entry.code === code;
          entry.button.setAttribute('aria-pressed', String(selected));
          entry.button.classList.toggle('is-selected', selected);
          entry.check.hidden = !selected;
          entry.searchText = normalize([entry.nativeName, entry.englishName, displayName(localizedNames, entry.code), entry.code].join(' '));
        });
      });
      filter();
    }

    function visibleButtons() {
      return groups.flatMap(group => group.entries).filter(entry => !entry.button.hidden).map(entry => entry.button);
    }

    trigger.type = 'button';
    trigger.setAttribute('aria-haspopup', 'dialog');
    trigger.setAttribute('aria-controls', dialog.id);
    trigger.setAttribute('aria-expanded', 'false');
    dialog.setAttribute('aria-labelledby', title.id);
    trigger.addEventListener('click', () => {
      if (dialog.open) return;
      beforeOpen?.();
      search.value = '';
      refresh();
      dialog.showModal();
      document.documentElement.classList.add('has-language-dialog');
      trigger.setAttribute('aria-expanded', 'true');
      search.focus({ preventScroll: true });
    });
    close.addEventListener('click', () => dialog.close());
    dialog.addEventListener('keydown', event => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      dialog.close();
    });
    dialog.addEventListener('close', () => {
      document.documentElement.classList.remove('has-language-dialog');
      trigger.setAttribute('aria-expanded', 'false');
      pointerStartedOutside = false;
      trigger.focus({ preventScroll: true });
    });

    const outside = event => {
      const bounds = dialog.getBoundingClientRect();
      return event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom;
    };
    dialog.addEventListener('pointerdown', event => {
      pointerStartedOutside = event.target === dialog && outside(event);
    });
    dialog.addEventListener('click', event => {
      if (pointerStartedOutside && event.target === dialog && outside(event)) dialog.close();
      pointerStartedOutside = false;
    });
    search.addEventListener('input', filter);
    search.addEventListener('keydown', event => {
      if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
      const buttons = visibleButtons();
      if (!buttons.length) return;
      event.preventDefault();
      buttons[event.key === 'ArrowDown' ? 0 : buttons.length - 1].focus();
    });
    results.addEventListener('keydown', event => {
      if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
      const button = event.target.closest('.language-option');
      if (!button) return;
      const buttons = visibleButtons();
      const index = buttons.indexOf(button);
      if (index < 0) return;
      event.preventDefault();
      const next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length;
      buttons[next].focus();
    });

    refresh();
    fallback.hidden = true;
    trigger.hidden = false;
    return { refresh };
  };
})();
