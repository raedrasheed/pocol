// X8 fixture page C7: a new realm is created by five paths, then RTCPeerConnection is used FROM that realm
// (STUN on UDP 13478, the C5 configuration: proposal PB2). Not executed by the author.
(function () {
  'use strict';
  var H = '__CANARY_HOST__', RUN = '__RUN__';
  var log = window.__x8 = { channel: 'C7', run: RUN, attempts: [] };
  var host = document.getElementById('x8host');
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
    var pc = new w.RTCPeerConnection({ iceServers: [{ urls: ice('stun', 13478) }] });
    pc.createDataChannel('x8');
    return pc.createOffer().then(function (o) { return pc.setLocalDescription(o); });
  }
  rec('C7.createElement', 'iframe-createElement', 'udp/13478', function () {
    var f = document.createElement('iframe'); host.appendChild(f); return rtcFrom(f);
  });
  rec('C7.innerHTML', 'iframe-innerHTML', 'udp/13478', function () {
    var d = document.createElement('div'); d.innerHTML = '<iframe></iframe>'; host.appendChild(d); return rtcFrom(d.firstChild);
  });
  rec('C7.createElementNS', 'iframe-createElementNS', 'udp/13478', function () {
    var f = document.createElementNS('http://www.w3.org/1999/xhtml', 'iframe'); host.appendChild(f); return rtcFrom(f);
  });
  rec('C7.templateImportNode', 'iframe-template-importNode', 'udp/13478', function () {
    var t = document.createElement('template'); t.innerHTML = '<iframe></iframe>';
    var n = document.importNode(t.content, true); var f = n.firstChild; host.appendChild(n); return rtcFrom(f);
  });
  rec('C7.documentWrite', 'iframe-document-write', 'udp/13478', function () {
    document.write('<iframe id="x8c7w"></iframe>'); return rtcFrom(document.getElementById('x8c7w'));
  });
  rec('X.apiProbe', 'extensionApiProbe', 'none', function () {
    log.extensionApi = { chrome: typeof window.chrome, runtime: typeof (window.chrome && window.chrome.runtime), browser: typeof window.browser };
  });
})();
