// X8 fixture page C2 (img, link stylesheet, FontFace, CSS @import), created at run time. Not executed by the author.
(function () {
  'use strict';
  var H = '__CANARY_HOST__', RUN = '__RUN__';
  var log = window.__x8 = { channel: 'C2', run: RUN, attempts: [] };
  function url(scheme, port, tail) { return scheme + '://' + H + ':' + port + tail; }
  function rec(id, api, target, fn) {
    var a = { id: id, api: api, target: target, error: null };
    log.attempts.push(a);
    try {
      var p = fn();
      if (p && typeof p.then === 'function') { p.then(function () {}, function (e) { a.error = String((e && e.name) || e); }); }
    } catch (e) { a.error = String((e && e.name) || e); }
  }
  rec('C2.img', 'img', 'tcp/18080', function () {
    var i = new Image(); i.src = url('http', 18080, '/x8/' + RUN + '/C2/img.png'); document.body.appendChild(i);
  });
  rec('C2.link', 'link-stylesheet', 'tcp/18080', function () {
    var l = document.createElement('link'); l.rel = 'stylesheet'; l.href = url('http', 18080, '/x8/' + RUN + '/C2/link.css'); document.head.appendChild(l);
  });
  rec('C2.font', 'FontFace', 'tcp/18080', function () {
    var f = new FontFace('x8c2', 'url(' + url('http', 18080, '/x8/' + RUN + '/C2/font.woff2') + ')'); return f.load();
  });
  rec('C2.import', 'css-import', 'tcp/18080', function () {
    var s = document.createElement('style'); s.textContent = '@import url(' + url('http', 18080, '/x8/' + RUN + '/C2/import.css') + ');'; document.head.appendChild(s);
  });
  rec('X.apiProbe', 'extensionApiProbe', 'none', function () {
    log.extensionApi = { chrome: typeof window.chrome, runtime: typeof (window.chrome && window.chrome.runtime), browser: typeof window.browser };
  });
})();
