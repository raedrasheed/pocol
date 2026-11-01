// X8 fixture page C4 (WebSocket to TCP 18443). Not executed by the author.
(function () {
  'use strict';
  var H = '__CANARY_HOST__', RUN = '__RUN__';
  var log = window.__x8 = { channel: 'C4', run: RUN, attempts: [] };
  function url(scheme, port, tail) { return scheme + '://' + H + ':' + port + tail; }
  function rec(id, api, target, fn) {
    var a = { id: id, api: api, target: target, error: null };
    log.attempts.push(a);
    try {
      var p = fn();
      if (p && typeof p.then === 'function') { p.then(function () {}, function (e) { a.error = String((e && e.name) || e); }); }
    } catch (e) { a.error = String((e && e.name) || e); }
  }
  rec('C4.websocket', 'WebSocket', 'tcp/18443', function () {
    var w = new WebSocket(url('ws', 18443, '/x8/' + RUN + '/C4/ws'));
    w.onerror = function () { log.attempts[0].error = log.attempts[0].error || 'error-event'; };
  });
  rec('X.apiProbe', 'extensionApiProbe', 'none', function () {
    log.extensionApi = { chrome: typeof window.chrome, runtime: typeof (window.chrome && window.chrome.runtime), browser: typeof window.browser };
  });
})();
