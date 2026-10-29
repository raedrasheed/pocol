// X8 fixture page C5 (RTCPeerConnection with a STUN server on UDP 13478). Not executed by the author.
(function () {
  'use strict';
  var H = '__CANARY_HOST__', RUN = '__RUN__';
  var log = window.__x8 = { channel: 'C5', run: RUN, attempts: [] };
  function ice(scheme, port, q) { return scheme + ':' + H + ':' + port + (q || ''); }
  function rec(id, api, target, fn) {
    var a = { id: id, api: api, target: target, error: null };
    log.attempts.push(a);
    try {
      var p = fn();
      if (p && typeof p.then === 'function') { p.then(function () {}, function (e) { a.error = String((e && e.name) || e); }); }
    } catch (e) { a.error = String((e && e.name) || e); }
  }
  rec('C5.stun', 'RTCPeerConnection', 'udp/13478', function () {
    var pc = new RTCPeerConnection({ iceServers: [{ urls: ice('stun', 13478) }] });
    pc.createDataChannel('x8');
    return pc.createOffer().then(function (o) { return pc.setLocalDescription(o); });
  });
  rec('X.apiProbe', 'extensionApiProbe', 'none', function () {
    log.extensionApi = { chrome: typeof window.chrome, runtime: typeof (window.chrome && window.chrome.runtime), browser: typeof window.browser };
  });
})();
