// X8 SUPPLEMENTAL page C7-P2: innerHTML iframe path only, in its own observation window.
// Proposal for root review; not executed by the author. Closes its peer connections at 12000 ms (cleanup).
(function () {
  'use strict';
  var H = '__CANARY_HOST__', RUN = '__RUN__';
  var log = window.__x8 = { channel: 'C7', path: 'P2', run: RUN, realm: false, pc: false, closed: false, attempts: [] };
  var host = document.getElementById('x8host');
  var pcs = [];
  function ice(scheme, port, q) { return scheme + ':' + H + ':' + port + (q || ''); }
  function rec(id, api, target, fn) {
    var a = { id: id, api: api, target: target, error: null };
    log.attempts.push(a);
    try {
      var p = fn();
      if (p && typeof p.then === 'function') { p.then(function () {}, function (e) { a.error = String((e && e.name) || e); }); }
    } catch (e) { a.error = String((e && e.name) || e); }
  }
  function rtcFrom(frame) {
    var w = frame && frame.contentWindow;
    if (!w) { throw new Error('noRealm'); }
    log.realm = true;
    var pc = new w.RTCPeerConnection({ iceServers: [{ urls: ice('stun', 13478) }] });
    log.pc = true; pcs.push(pc);
    pc.createDataChannel('x8');
    return pc.createOffer().then(function (o) { return pc.setLocalDescription(o); });
  }
  rec('C7.innerHTML', 'iframe-innerHTML', 'udp/13478', function () {
    var d = document.createElement('div'); d.innerHTML = '<iframe></iframe>'; host.appendChild(d); return rtcFrom(d.firstChild);
  });
  setTimeout(function () { pcs.forEach(function (p) { try { p.close(); } catch (e) { /* recorded by state */ } }); log.closed = true; }, 12000);
})();
