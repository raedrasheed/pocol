// X8 fixture page C1 (fetch, XHR). Specification fixture; not executed by the author.
(function () {
  'use strict';
  var H = '__CANARY_HOST__', RUN = '__RUN__';
  var log = window.__x8 = { channel: 'C1', run: RUN, attempts: [] };
  function url(scheme, port, tail) { return scheme + '://' + H + ':' + port + tail; }
  function rec(id, api, target, fn) {
    var a = { id: id, api: api, target: target, error: null };
    log.attempts.push(a);
    try {
      var p = fn();
      if (p && typeof p.then === 'function') { p.then(function () {}, function (e) { a.error = String((e && e.name) || e); }); }
    } catch (e) { a.error = String((e && e.name) || e); }
  }
  rec('C1.fetch', 'fetch', 'tcp/18080', function () { return fetch(url('http', 18080, '/x8/' + RUN + '/C1/fetch'), { mode: 'no-cors' }); });
  rec('C1.xhr', 'XMLHttpRequest', 'tcp/18080', function () {
    var x = new XMLHttpRequest(); x.open('GET', url('http', 18080, '/x8/' + RUN + '/C1/xhr')); x.send();
  });
  rec('X.apiProbe', 'extensionApiProbe', 'none', function () {
    log.extensionApi = { chrome: typeof window.chrome, runtime: typeof (window.chrome && window.chrome.runtime), browser: typeof window.browser };
  });
})();
