// X8 fixture page C3 (link rel=prefetch, rel=preconnect), created at run time. Not executed by the author.
(function () {
  'use strict';
  var H = '__CANARY_HOST__', RUN = '__RUN__';
  var log = window.__x8 = { channel: 'C3', run: RUN, attempts: [] };
  function url(scheme, port, tail) { return scheme + '://' + H + ':' + port + tail; }
  function rec(id, api, target, fn) {
    var a = { id: id, api: api, target: target, error: null };
    log.attempts.push(a);
    try {
      var p = fn();
      if (p && typeof p.then === 'function') { p.then(function () {}, function (e) { a.error = String((e && e.name) || e); }); }
    } catch (e) { a.error = String((e && e.name) || e); }
  }
  rec('C3.prefetch', 'link-prefetch', 'tcp/18080', function () {
    var l = document.createElement('link'); l.rel = 'prefetch'; l.href = url('http', 18080, '/x8/' + RUN + '/C3/prefetch'); document.head.appendChild(l);
  });
  rec('C3.preconnect', 'link-preconnect', 'tcp/18080', function () {
    var l = document.createElement('link'); l.rel = 'preconnect'; l.href = url('http', 18080, '/'); document.head.appendChild(l);
  });
  rec('X.apiProbe', 'extensionApiProbe', 'none', function () {
    log.extensionApi = { chrome: typeof window.chrome, runtime: typeof (window.chrome && window.chrome.runtime), browser: typeof window.browser };
  });
})();
