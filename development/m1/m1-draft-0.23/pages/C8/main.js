// X8 fixture page C8 (navigator.sendBeacon, then a form submission 1000 ms later). Not executed by the author.
(function () {
  'use strict';
  var H = '__CANARY_HOST__', RUN = '__RUN__';
  var log = window.__x8 = { channel: 'C8', run: RUN, attempts: [] };
  function url(scheme, port, tail) { return scheme + '://' + H + ':' + port + tail; }
  function rec(id, api, target, fn) {
    var a = { id: id, api: api, target: target, error: null };
    log.attempts.push(a);
    try {
      var p = fn();
      if (p && typeof p.then === 'function') { p.then(function () {}, function (e) { a.error = String((e && e.name) || e); }); }
    } catch (e) { a.error = String((e && e.name) || e); }
  }
  rec('C8.beacon', 'sendBeacon', 'tcp/18080', function () {
    log.beaconQueued = navigator.sendBeacon(url('http', 18080, '/x8/' + RUN + '/C8/beacon'), 'x8');
  });
  rec('X.apiProbe', 'extensionApiProbe', 'none', function () {
    log.extensionApi = { chrome: typeof window.chrome, runtime: typeof (window.chrome && window.chrome.runtime), browser: typeof window.browser };
  });
  rec('C8.form', 'form-submit', 'tcp/18080', function () {
    var f = document.createElement('form'); f.method = 'post'; f.action = url('http', 18080, '/x8/' + RUN + '/C8/form');
    var i = document.createElement('input'); i.type = 'hidden'; i.name = 'x8'; i.value = RUN; f.appendChild(i); document.body.appendChild(f);
    setTimeout(function () { f.submit(); }, 1000);
  });
})();
