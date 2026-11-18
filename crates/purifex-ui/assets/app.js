/* Purifex page. It only draws what the program decided (the domain builds every
   figure and every verdict); it formats numbers and escapes every text that comes
   from the system, because names of devices and processes are not trusted. */
(function () {
  'use strict';

  // ---- Access: the token of this run arrives once in the address ------------
  var params = new URLSearchParams(location.search);
  if (params.get('t')) { sessionStorage.setItem('pt', params.get('t')); }
  var TOKEN = sessionStorage.getItem('pt') || '';
  var START = location.hash.replace('#', '');
  history.replaceState(null, '', '/' + location.hash);

  function api(path) {
    return fetch(path, { headers: { 'X-Purifex-Token': TOKEN }, cache: 'no-store' }).then(function (r) {
      return r.json().then(function (body) {
        if (!r.ok) { throw new Error(body && body.error ? body.error : 'error ' + r.status); }
        return body;
      });
    });
  }

  // ---- Helpers ---------------------------------------------------------------
  var ESC = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };
  function esc(v) { return String(v == null ? '' : v).replace(/[&<>"']/g, function (c) { return ESC[c]; }); }
  function ic(name, size) { return window.icon(name, size); }
  function n1(v) { return (Math.round(v * 10) / 10).toFixed(1); }
  function n0(v) { return String(Math.round(v)); }

  function ageText(days) {
    if (days == null) { return '–'; }
    if (days < 0) { return '–'; }
    if (days >= 365) { return (days / 365.25).toFixed(1) + ' años'; }
    if (days >= 30) { return Math.floor(days / 30) + ' meses'; }
    return days + ' d';
  }

  function durationText(s) {
    if (s < 60) { return s + ' s'; }
    if (s < 3600) { return Math.floor(s / 60) + ' min ' + ('0' + (s % 60)).slice(-2) + ' s'; }
    return Math.floor(s / 3600) + ' h ' + ('0' + Math.floor((s % 3600) / 60)).slice(-2) + ' min';
  }

  function clock() {
    var d = new Date();
    function p(n, l) { return ('000' + n).slice(-l); }
    return p(d.getHours(), 2) + ':' + p(d.getMinutes(), 2) + ':' + p(d.getSeconds(), 2) + '.' + p(d.getMilliseconds(), 3);
  }

  var CATEGORY_ICON = {
    gpu: 'microchip', audio: 'audio-lines', network: 'router', input: 'mouse', bluetooth: 'bluetooth',
    storage: 'hard-drive', monitor: 'monitor', camera: 'video', system: 'cpu', other: 'package'
  };

  // ---- State -----------------------------------------------------------------
  var state = {
    page: ['drivers', 'sessions'].indexOf(START) !== -1 ? START : 'overview', overview: null, drivers: null, sessions: null, analysis: null,
    selected: null, filter: 'all', query: '', error: null, busy: false
  };
  var app = document.getElementById('app');

  // ---- Pieces ----------------------------------------------------------------
  function topbar() {
    var o = state.overview;
    return '<header class="topbar">' +
      '<div class="brand"><span class="brand-mark">' + ic('zap', 16) + '</span>PURIFEX <small>// DIAGNÓSTICO</small></div>' +
      '<span class="chip ok"><span class="dot green"></span>Solo lectura</span>' +
      '<span class="chip">' + ic('shield-check', 14) + 'Sin telemetría</span>' +
      (o ? '<span class="chip">' + ic('activity', 14) + 'Escaneo <b>' + esc(o.scan_ms) + ' ms</b></span>' : '') +
      '<span class="spacer"></span>' +
      '<span class="chip">' + ic('folder-open', 14) + '%LOCALAPPDATA%\\Purifex</span>' +
      '</header>';
  }

  function navItem(id, icon, label, badge, off) {
    var cls = 'nav-item' + (state.page === id ? ' active' : '') + (off ? ' off' : '');
    var b = badge ? '<span class="badge ' + (badge.cls || '') + '">' + esc(badge.text) + '</span>' : '';
    return '<button class="' + cls + '" data-page="' + id + '"' + (off ? ' disabled' : '') + '>' + ic(icon, 18) + '<span>' + esc(label) + '</span>' + b + '</button>';
  }

  function sidebar() {
    var o = state.overview;
    var attention = o ? o.devices.faults + o.devices.missing_driver : 0;
    var healthy = o && o.devices.hardware ? (o.devices.healthy * 100 / o.devices.hardware) : 0;
    return '<aside class="side">' +
      '<div class="engine"><div class="engine-head"><span class="kpi-icon">' + ic('layout-dashboard', 18) + '</span>Purifex<span class="badge accent">activo</span></div>' +
      '<dl><dt>Dispositivos</dt><dd>' + (o ? esc(o.devices.hardware) : '–') + '</dd>' +
      '<dt>Grabaciones</dt><dd>' + (state.sessions ? esc(state.sessions.length) : '–') + '</dd></dl></div>' +
      '<div><div class="eyebrow">Navegación</div><nav class="nav">' +
      navItem('overview', 'gauge', 'Estado general') +
      navItem('drivers', 'layers', 'Drivers', attention ? { text: String(attention), cls: 'red' } : null) +
      navItem('sessions', 'gamepad-2', 'Partidas', state.sessions ? { text: String(state.sessions.length) } : null) +
      navItem('fixes', 'sliders-horizontal', 'Arreglos', { text: 'pronto' }, true) +
      navItem('history', 'history', 'Historial', { text: 'pronto' }, true) +
      '</nav></div>' +
      '<div class="side-foot"><div class="row"><span><span class="dot green"></span> Modo</span><b>Solo lectura</b></div>' +
      '<div class="row"><span>Salud de drivers</span><b>' + (o ? n1(healthy) + ' %' : '–') + '</b></div>' +
      '<div class="bar"><i style="width:' + n0(healthy) + '%"></i></div></div>' +
      '</aside>';
  }

  function head(tag, title, lead, button) {
    return '<div class="page-head"><div><div class="tag-line"><span class="pill"><span class="dot green"></span>' + esc(tag.pill) + '</span><span>' + esc(tag.text) + '</span></div>' +
      '<h1>' + esc(title) + '</h1><p class="lead">' + esc(lead) + '</p></div>' +
      '<div class="actions">' + (button || '') + '</div></div>';
  }

  function rescanButton(primary) {
    return '<button class="btn' + (primary ? ' primary' : '') + '" id="rescan"' + (state.busy ? ' disabled' : '') + '>' +
      '<span class="' + (state.busy ? 'spin' : '') + '">' + ic('refresh-cw', 16) + '</span>' + (state.busy ? 'Leyendo…' : 'Reescanear') + '</button>';
  }

  function errorBox() {
    return state.error ? '<div class="error-box">' + ic('circle-alert', 18) + '<div><b>No se pudo leer</b><br>' + esc(state.error) + '</div></div>' : '';
  }

  // ---- Overview --------------------------------------------------------------
  function kpiDpc(o) {
    var s = o.session;
    var value = s ? n1(s.dpc_p95) : '–';
    var chip = !s ? '<span class="mini gray">sin datos</span>' : (s.dpc_p95 >= 5 ? '<span class="mini amber">Alto</span>' : '<span class="mini green">Normal</span>');
    return '<div class="kpi"><div class="kpi-head"><span class="eyebrow">DPC / interrupciones · p95</span><span class="kpi-icon">' + ic('activity', 18) + '</span></div>' +
      '<div class="kpi-value">' + value + '<small>%</small>' + chip + '</div>' +
      '<div class="kpi-note">' + (s ? 'Tiempo del CPU en interrupciones. Alto = un driver acapara el procesador y da tirones. Partida de ' + esc(durationText(s.seconds)) + '.' : 'Graba una partida con <b>purifex watch</b>.') + '</div></div>';
  }

  function kpiDevices(o) {
    var d = o.devices;
    return '<div class="kpi"><div class="kpi-head"><span class="eyebrow">Integridad de drivers</span><span class="kpi-icon blue">' + ic('microchip', 18) + '</span></div>' +
      '<div class="kpi-value">' + esc(d.hardware) + '<small>dispositivos</small></div>' +
      '<div class="kpi-foot"><span><span class="dot green"></span><b>' + esc(d.healthy) + '</b> sin problemas</span>' +
      '<span><span class="dot blue"></span><b>' + esc(d.missing_driver) + '</b> sin driver</span>' +
      '<span><span class="dot red"></span><b>' + esc(d.faults) + '</b> con fallo</span></div></div>';
  }

  function kpiLimiter(o) {
    var l = o.limiter;
    var good = l.title === 'Nada destacable';
    return '<div class="kpi"><div class="kpi-head"><span class="eyebrow">Lo que más te limita</span><span class="kpi-icon amber">' + ic('scan-search', 18) + '</span></div>' +
      '<div class="kpi-title">' + ic(good ? 'circle-check' : 'triangle-alert', 18) + '<span>' + esc(l.title) + '</span></div>' +
      '<div class="kpi-note">' + esc(l.detail) + '</div></div>';
  }

  function kpiGpu(o) {
    var g = o.gpu;
    if (!g) { return '<div class="kpi"><div class="kpi-head"><span class="eyebrow">Tarjeta gráfica</span><span class="kpi-icon">' + ic('monitor', 18) + '</span></div><div class="kpi-note">No se encontró ninguna.</div></div>'; }
    return '<div class="kpi"><div class="kpi-head"><span class="eyebrow">Tarjeta gráfica</span><span class="kpi-icon">' + ic('monitor', 18) + '</span></div>' +
      '<div class="kpi-title"><span>' + esc(g.name) + '</span></div>' +
      '<div class="kpi-foot"><span>Driver <b>' + esc(g.version || '–') + '</b></span><span>' + esc(g.date || '–') + '</span>' +
      (g.age_days != null ? '<span class="mini blue">' + esc(ageText(g.age_days)) + '</span>' : '') + '</div></div>';
  }

  function leadsPanel(o) {
    var f = o.session ? o.session.findings : [];
    var body;
    if (!o.session) { body = '<div class="empty">Sin partidas grabadas: aún no hay pistas.</div>'; }
    else if (!f.length) { body = '<div class="empty">' + ic('circle-check', 16) + ' Nada destacable en la última partida.</div>'; }
    else {
      body = '<ul class="leads">' + f.map(function (x) {
        return '<li>' + ic('triangle-alert', 18) + '<div><b>' + esc(x.title) + '</b><span>' + esc(x.evidence) + '</span></div></li>';
      }).join('') + '</ul>';
    }
    return '<section class="panel"><div class="panel-head">' + ic('list-checks', 16) + 'Pistas de la última partida' +
      (o.session ? '<span class="right">' + esc(o.session.id) + '</span>' : '') + '</div><div class="panel-body">' + body + '</div></section>';
  }

  function consolePanel(o) {
    var t = clock();
    var lines = o.console.map(function (l) {
      return '<div class="ln"><span class="t">[' + t + ']</span><span class="tag ' + esc(l.tag) + '">[' + esc(l.tag) + ']</span><span>' + esc(l.text) + '</span></div>';
    }).join('');
    return '<section class="panel"><div class="panel-head">' + ic('square-terminal', 16) + 'Purifex // consola de eventos<span class="right"><span class="dot green"></span> lectura local</span></div>' +
      '<div class="panel-body"><div class="console">' + lines + '</div></div></section>';
  }

  function principlesPanel() {
    function item(title, small) { return '<li><span class="box">' + ic('circle-check', 12) + '</span><div>' + esc(title) + '<small>' + esc(small) + '</small></div></li>'; }
    return '<section class="panel"><div class="panel-head">Principios<span class="right"><span class="mini green">siempre</span></span></div>' +
      '<div class="panel-body"><ul class="checks">' +
      item('Solo lectura', 'Esta versión no cambia nada de tu sistema.') +
      item('Sin telemetría', 'Nada sale de tu PC: la página solo habla con este programa.') +
      item('Datos locales', 'Las grabaciones están en %LOCALAPPDATA%\\Purifex.') +
      '</ul></div><div class="panel-foot"><span>Servidor local:</span><b>127.0.0.1</b></div></section>';
  }

  function overviewPage() {
    var o = state.overview;
    var h = head({ pill: 'Estado general', text: 'Escaneo de dispositivos · grabación de partidas' }, 'Estado de tu PC',
      'Lo que hay instalado y lo que limitó tu última partida. Cada cifra sale de una medición real hecha en este equipo.', rescanButton(true));
    if (!o) { return h + errorBox() + (state.error ? '' : '<div class="loading"><span class="spin">' + ic('refresh-cw', 18) + '</span>Leyendo el equipo…</div>'); }
    return h + errorBox() +
      '<div class="kpis">' + kpiDpc(o) + kpiDevices(o) + kpiLimiter(o) + kpiGpu(o) + '</div>' +
      leadsPanel(o) + '<div class="two">' + consolePanel(o) + principlesPanel() + '</div>';
  }

  // ---- Drivers ---------------------------------------------------------------
  function healthTag(r) {
    var h = r.health;
    if (h === 'working' && !r.has_driver) { return '<span class="tag gray">Sin datos de driver</span>'; }
    if (h === 'working') { return r.inbox ? '<span class="tag gray">' + ic('circle-check') + 'Windows</span>' : '<span class="tag green">' + ic('circle-check') + 'Correcto</span>'; }
    if (h === 'missing_driver') { return '<span class="tag red">' + ic('circle-alert') + 'Sin driver</span>'; }
    if (h === 'disabled') { return '<span class="tag gray">Desactivado</span>'; }
    if (h && h.fault) { return '<span class="tag red">' + ic('triangle-alert') + 'Fallo · código ' + esc(h.fault.code) + '</span>'; }
    return '';
  }

  function rowMatches(r, q) {
    if (!q) { return true; }
    return [r.name, r.instance_id, r.provider, r.class, r.version].join(' ').toLowerCase().indexOf(q) !== -1;
  }

  function rowVisible(r) {
    var bad = r.health === 'missing_driver' || (r.health && r.health.fault);
    if (state.filter === 'attention' && !bad) { return false; }
    if (state.filter !== 'all' && state.filter !== 'attention' && r.category !== state.filter) { return false; }
    return rowMatches(r, state.query.trim().toLowerCase());
  }

  function driverRows() {
    var rows = state.drivers.rows.filter(rowVisible);
    if (!rows.length) { return '<tr><td colspan="4"><div class="empty">Ningún dispositivo coincide.</div></td></tr>'; }
    return rows.map(function (r) {
      var bad = r.health === 'missing_driver' || (r.health && r.health.fault);
      var iconCls = bad ? 'red' : (r.inbox ? 'gray' : '');
      return '<tr><td><div class="dev"><span class="dev-icon ' + iconCls + '">' + ic(CATEGORY_ICON[r.category] || 'package', 20) + '</span><div>' +
        '<div class="dev-name">' + esc(r.name) + (r.count > 1 ? '<span class="x">×' + esc(r.count) + '</span>' : '') + '</div>' +
        '<div class="dev-id" title="' + esc(r.instance_id) + '">' + esc(r.instance_id) + '</div></div></div></td>' +
        '<td><div class="cut">' + esc(r.class || '–') + '</div><div class="muted cut" title="' + esc(r.provider || '') + '">' + esc(r.provider || '–') + '</div></td>' +
        '<td class="nowrap"><div class="ver">' + esc(r.version || '–') + '</div><div class="muted">' + esc(r.date || '–') + (r.age_days != null ? ' · ' + esc(ageText(r.age_days)) : '') + '</div></td>' +
        '<td class="nowrap">' + healthTag(r) + '</td></tr>';
    }).join('');
  }

  function filterChips() {
    var d = state.drivers;
    function chip(id, label, count, cls) {
      return '<button class="filter ' + (state.filter === id ? 'active ' : '') + (cls || '') + '" data-filter="' + esc(id) + '">' +
        (cls === 'red' ? '<span class="dot red"></span>' : '') + esc(label) + ' <span class="n">' + esc(count) + '</span></button>';
    }
    var chips = chip('all', 'Todos los dispositivos', d.total);
    if (d.attention) { chips += chip('attention', 'Con problema', d.attention, 'red'); }
    d.categories.forEach(function (c) { chips += chip(c.category, c.label, c.count); });
    return chips;
  }

  function driversPage() {
    var h = head({ pill: 'Escaneo de hardware', text: 'SetupAPI · lo mismo que lee el Administrador de dispositivos' }, 'Drivers y dispositivos',
      'Cada dispositivo con el driver que usa hoy. Un driver viejo no es un problema por sí solo: aquí solo se muestran hechos.', rescanButton(true));
    if (!state.drivers) { return h + errorBox() + (state.error ? '' : '<div class="loading"><span class="spin">' + ic('refresh-cw', 18) + '</span>Leyendo dispositivos…</div>'); }
    return h + errorBox() +
      '<section class="panel"><div class="tools"><label class="search">' + ic('search', 18) +
      '<input id="q" type="search" placeholder="Filtrar por nombre, VEN_/DEV_, proveedor…" value="' + esc(state.query) + '" autocomplete="off" spellcheck="false"><kbd>Ctrl K</kbd></label></div></section>' +
      '<div class="filters" id="filters">' + filterChips() + '</div>' +
      '<section class="panel"><div class="table-wrap"><table><thead><tr><th>Dispositivo y ubicación</th><th>Clase / proveedor</th><th>Versión del driver</th><th>Estado</th></tr></thead>' +
      '<tbody id="drv-body">' + driverRows() + '</tbody></table></div></section>';
  }

  // ---- Sessions --------------------------------------------------------------
  function sessionList() {
    var rows = state.sessions || [];
    if (!rows.length) { return '<div class="empty" style="padding:16px">Todavía no hay grabaciones. Graba una partida con <b>purifex watch</b>.</div>'; }
    return rows.map(function (s) {
      var bad = !!s.problem;
      return '<button class="sess' + (state.selected === s.id ? ' active' : '') + '" data-session="' + esc(s.id) + '">' +
        '<span class="kpi-icon ' + (bad ? 'amber' : '') + '">' + ic(bad ? 'triangle-alert' : 'gamepad-2', 16) + '</span>' +
        '<div><b>' + esc(s.started || s.id) + '</b><span>' + esc(bad ? 'no se puede leer' : s.samples + ' muestras') + '</span></div>' +
        '<span class="len">' + esc(bad ? '–' : durationText(s.seconds)) + '</span></button>';
    }).join('');
  }

  function statRow(label, s, unit, dec) {
    var f = dec === 0 ? n0 : n1;
    return '<tr><td>' + esc(label) + '</td><td class="num">' + f(s.avg) + unit + '</td><td class="num">' + f(s.p95) + unit + '</td><td class="num">' + f(s.max) + unit + '</td></tr>';
  }

  function analysisPanel() {
    var a = state.analysis;
    if (!a) { return '<section class="panel"><div class="panel-body"><div class="empty">Elige una grabación.</div></div></section>'; }
    var s = a.summary;
    var rows = statRow('CPU total', s.cpu_total, ' %') + statRow('Núcleo más cargado', s.cpu_max_core, ' %') +
      statRow('Interrupciones / DPC', s.dpc_interrupt, ' %') + (s.cpu_clock_pct.avg > 0 ? statRow('Frecuencia (de la base)', s.cpu_clock_pct, ' %', 0) : '') +
      statRow('RAM en uso', s.ram_used, ' %', 0);
    if (s.game_cpu.max > 0) { rows += statRow('El juego · CPU', s.game_cpu, ' %') + statRow('El juego · memoria', s.game_memory_mb, ' MB', 0); }
    s.groups.forEach(function (g) { rows += statRow(groupLabel(g.group), g.cpu, ' %'); });
    if (s.gpu) { rows += statRow('GPU · uso', s.gpu.util, ' %', 0) + statRow('GPU · temperatura', s.gpu.temp_c, ' °C', 0) + statRow('GPU · consumo', s.gpu.power_w, ' W'); }
    var leads = a.findings.length ? '<ul class="leads">' + a.findings.map(function (x) {
      return '<li>' + ic('triangle-alert', 18) + '<div><b>' + esc(x.title) + '</b><span>' + esc(x.evidence) + '</span></div></li>';
    }).join('') + '</ul>' : '<div class="empty">' + ic('circle-check', 16) + ' Nada destacable en esta partida.</div>';
    var who = a.header ? esc(a.header.game) + ' · ' + esc(a.header.cores) + ' núcleos' : 'sin cabecera (grabación antigua)';
    return '<section class="panel"><div class="panel-head">' + ic('chart-no-axes-combined', 16) + esc(a.id) + '<span class="right">' + who + ' · ' + esc(durationText(s.seconds)) + '</span></div>' +
      '<div class="panel-body">' + leads + '</div></section>' +
      '<section class="panel" style="margin-top:16px"><div class="table-wrap"><table class="stats"><thead><tr><th>Medida</th><th class="num" style="text-align:right">Media</th><th style="text-align:right">p95</th><th style="text-align:right">Máx.</th></tr></thead><tbody>' + rows + '</tbody></table></div>' +
      '<div class="panel-foot"><span>Medir costó</span><b>' + n1(s.sample_cost_us.avg / 1000) + ' ms por muestra</b></div></section>';
  }

  function groupLabel(key) {
    return { league_client: 'Cliente de League · CPU', riot_client: 'Riot Client · CPU', anti_cheat: 'Vanguard · CPU', browser: 'Navegadores · CPU', system: 'Windows y antivirus · CPU' }[key] || key;
  }

  function sessionsPage() {
    var h = head({ pill: 'Partidas grabadas', text: 'purifex watch · una muestra por segundo' }, 'Partidas',
      'Elige una grabación para ver qué limitó al equipo mientras jugabas.', rescanButton(false));
    return h + errorBox() + '<div class="sessions"><section class="panel"><div class="sess-list">' + sessionList() + '</div></section><div>' + analysisPanel() + '</div></div>';
  }

  // ---- Render and events ---------------------------------------------------
  function render() {
    var page = state.page === 'drivers' ? driversPage() : (state.page === 'sessions' ? sessionsPage() : overviewPage());
    var keep = document.querySelector('.main');
    var scroll = keep ? keep.scrollTop : 0;
    app.innerHTML = '<div class="shell">' + topbar() + sidebar() + '<main class="main">' + page + '</main></div>';
    var main = document.querySelector('.main');
    if (main) { main.scrollTop = scroll; }
    bind();
  }

  function bind() {
    Array.prototype.forEach.call(document.querySelectorAll('[data-page]'), function (b) {
      b.addEventListener('click', function () { go(b.getAttribute('data-page')); });
    });
    var rescan = document.getElementById('rescan');
    if (rescan) { rescan.addEventListener('click', reload); }
    Array.prototype.forEach.call(document.querySelectorAll('[data-filter]'), function (b) {
      b.addEventListener('click', function () { state.filter = b.getAttribute('data-filter'); render(); });
    });
    Array.prototype.forEach.call(document.querySelectorAll('[data-session]'), function (b) {
      b.addEventListener('click', function () { selectSession(b.getAttribute('data-session')); });
    });
    var q = document.getElementById('q');
    if (q) {
      q.addEventListener('input', function () {
        state.query = q.value;
        document.getElementById('drv-body').innerHTML = driverRows();
      });
    }
  }

  function go(page) { state.page = page; state.error = null; history.replaceState(null, '', '/#' + page); render(); load(); }

  function selectSession(id) {
    state.selected = id;
    api('/api/analysis?id=' + encodeURIComponent(id)).then(function (a) { state.analysis = a; state.error = null; })
      .catch(function (e) { state.analysis = null; state.error = e.message; }).then(render);
  }

  function load() {
    var jobs = [];
    if (state.page === 'overview' || !state.overview) { jobs.push(api('/api/overview').then(function (v) { state.overview = v; })); }
    if (state.page === 'drivers') { jobs.push(api('/api/drivers').then(function (v) { state.drivers = v; })); }
    if (state.page === 'sessions' || !state.sessions) {
      jobs.push(api('/api/sessions').then(function (v) { state.sessions = v.sessions; }));
    }
    state.busy = true;
    Promise.all(jobs).then(function () {
      state.error = null;
      if (state.page === 'sessions' && !state.selected && state.sessions && state.sessions.length) {
        var first = state.sessions.filter(function (s) { return !s.problem; })[0];
        if (first) { return selectSession(first.id); }
      }
    }).catch(function (e) { state.error = e.message; }).then(function () { state.busy = false; render(); });
  }

  function reload() {
    state.overview = state.drivers = state.sessions = null;
    state.busy = true;
    render();
    load();
  }

  document.addEventListener('keydown', function (e) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      var q = document.getElementById('q');
      if (!q) { go('drivers'); return; }
      e.preventDefault();
      q.focus();
    }
  });

  // The program closes itself a few seconds after the page stops answering.
  setInterval(function () { api('/api/ping').catch(function () {}); }, 3000);

  render();
  load();
})();
