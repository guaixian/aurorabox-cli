/**
 * Tauri v2 API Compatibility Shim for AuroraBox Web
 *
 * Handles ALL Tauri plugin: commands the app uses.
 * Maps them to REST API calls or provides browser-compatible stubs.
 */
(function() {
  'use strict';

  // ============================================================
  // Callback system
  // ============================================================
  let nextCbId = 1;
  const cbMap = new Map();
  function transformCallback(fn, once) {
    const id = nextCbId++;
    cbMap.set(id, { fn, once: !!once });
    return id;
  }
  function unregisterCallback(id) { cbMap.delete(id); }
  function invokeCb(id, data) {
    const cb = cbMap.get(id);
    if (cb) { try { cb.fn(data); } catch(e) {} if (cb.once) cbMap.delete(id); }
  }

  // ============================================================
  // Event system
  // ============================================================
  const evtListeners = new Map();
  let nextEvtId = 1;
  const pollIntervals = {};
  let lastStateStr = null;

  // ============================================================
  // In-memory stores (for SQL and Store plugins)
  // ============================================================
  const sqlData = new Map(); // dbPath -> array of rows
  const kvStores = new Map(); // storePath -> Map of key-values
  const fsFiles = new Map(); // filePath -> content string

  function getStore(path) {
    if (!kvStores.has(path)) kvStores.set(path, new Map());
    return kvStores.get(path);
  }

  // Pre-populate settings store with defaults
  const defaultSettings = getStore('settings.json');
  if (!defaultSettings.has('proxy_port_key')) defaultSettings.set('proxy_port_key', 6789);
  if (!defaultSettings.has('theme_pref_key')) defaultSettings.set('theme_pref_key', 'system');
  if (!defaultSettings.has('language')) defaultSettings.set('language', 'en');
  if (!defaultSettings.has('rule_mode_key')) defaultSettings.set('rule_mode_key', 'rule');
  if (!defaultSettings.has('allow_lan_key')) defaultSettings.set('allow_lan_key', false);
  if (!defaultSettings.has('enable_tun_key')) defaultSettings.set('enable_tun_key', false);
  if (!defaultSettings.has('selected_subscription_identifier')) defaultSettings.set('selected_subscription_identifier', '');
  if (!defaultSettings.has('skip_system_proxy_key')) defaultSettings.set('skip_system_proxy_key', false);

  // ============================================================
  // Debug logging
  // ============================================================
  const debugLog = [];
  const MAX_LOG = 200;
  function addLog(entry) {
    debugLog.push(entry);
    if (debugLog.length > MAX_LOG) debugLog.shift();
    updateDebugPanel();
  }
  function updateDebugPanel() {
    var panel = document.getElementById('__aurora_debug');
    if (!panel) return;
    var recent = debugLog.slice(-50);
    panel.innerHTML = '<div style="position:fixed;bottom:0;left:0;right:0;max-height:300px;overflow-y:auto;background:#0a0a0a;border-top:2px solid #333;font-family:monospace;font-size:11px;z-index:99999;padding:8px;color:#aaa;">' +
      '<div style="color:#6c5ce7;margin-bottom:4px;display:flex;justify-content:space-between;"><b>🔍 Tauri→REST Debug</b> <span>' + debugLog.length + ' calls</span></div>' +
      recent.map(function(e) { return '<div style="border-bottom:1px solid #1a1a1a;padding:2px 0;"><span style="color:' + (e.err ? '#f44' : '#4a4') + ';">' + (e.err ? '✗' : '✓') + '</span> <span style="color:#888;">' + e.ts + '</span> <span style="color:#6c5ce7;">' + e.cmd + '</span>' + (e.err ? ' <span style="color:#f44;">' + e.err + '</span>' : ' <span style="color:#4a4;">OK</span>') + '</div>'; }).join('') +
      '</div>';
  }
  function createDebugPanel() {
    if (document.getElementById('__aurora_debug')) return;
    var div = document.createElement('div');
    div.id = '__aurora_debug';
    document.body.appendChild(div);
    updateDebugPanel();
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', createDebugPanel);
  } else {
    createDebugPanel();
  }

  // ============================================================
  // Core invoke handler
  // ============================================================
  async function shimInvoke(cmd, args, options) {
    args = args || {};
    var startTs = Date.now();
    var ts = new Date().toISOString().slice(11,23);

    // ---- Plugin: event ----
    if (cmd === 'plugin:event|listen') {
      const id = nextEvtId++;
      if (!evtListeners.has(args.event)) evtListeners.set(args.event, []);
      evtListeners.get(args.event).push({ id, handlerId: args.handler });
      if (args.event === 'engine-state' || args.event === 'status-changed') startPoll(args.event, '/api/status', 2000);
      if (args.event === 'deep_link_pending') { checkDL(); setInterval(checkDL, 5000); }
      return id;
    }
    if (cmd === 'plugin:event|unlisten') {
      if (evtListeners.has(args.event)) {
        const list = evtListeners.get(args.event);
        const idx = list.findIndex(l => l.id === args.eventId);
        if (idx >= 0) list.splice(idx, 1);
      }
      return;
    }
    if (cmd === 'plugin:event|emit' || cmd === 'plugin:event|emit_to') {
      if (evtListeners.has(args.event)) {
        evtListeners.get(args.event).forEach(l => {
          invokeCb(l.handlerId, { event: args.event, payload: args.payload, id: l.id });
        });
      }
      return;
    }

    // ---- Plugin: sql ----
    // Database.load(uri) returns the uri string as this.path
    if (cmd === 'plugin:sql|load') {
      return args.db || 'sqlite:data.db';
    }
    // execute returns [rowsAffected, lastInsertId] as ARRAY
    if (cmd === 'plugin:sql|execute') {
      try {
        const r = await fetch('/api/db/execute', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ sql: args.query, bindings: args.values || [] })
        });
        const data = await r.json();
        return [data.rowsAffected || 0, data.lastInsertId || 0];
      } catch(e) {
        console.warn('[shim] SQL execute error:', e);
        return [0, 0];
      }
    }
    if (cmd === 'plugin:sql|select') {
      try {
        const r = await fetch('/api/db/select', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ sql: args.query, bindings: args.values || [] })
        });
        return await r.json();
      } catch(e) {
        console.warn('[shim] SQL select error:', e);
        return [];
      }
    }
    if (cmd === 'plugin:sql|close') return;

    // ---- Plugin: store ----
    // Store.load returns value that becomes Store.rid
    if (cmd === 'plugin:store|load') {
      return args.path || 'settings.json';
    }
    // Store.get (static) - get or create store by path
    if (cmd === 'plugin:store|get_store') {
      return args.path || 'settings.json';
    }
    // Store.get returns [value] as array (destructured by caller)
    if (cmd === 'plugin:store|get') {
      const store = getStore(args.rid || 'settings.json');
      const val = store.has(args.key) ? store.get(args.key) : null;
      return [val];
    }
    if (cmd === 'plugin:store|set') {
      const store = getStore(args.rid || 'settings.json');
      store.set(args.key, args.value);
      try { localStorage.setItem('s_' + args.key, JSON.stringify(args.value)); } catch(e) {}
      return;
    }
    if (cmd === 'plugin:store|has') {
      return getStore(args.rid || 'settings.json').has(args.key);
    }
    if (cmd === 'plugin:store|delete') {
      getStore(args.rid || 'settings.json').delete(args.key);
      return;
    }
    if (cmd === 'plugin:store|keys') {
      return Array.from(getStore(args.rid || 'settings.json').keys());
    }
    if (cmd === 'plugin:store|values') {
      return Array.from(getStore(args.rid || 'settings.json').values());
    }
    if (cmd === 'plugin:store|entries') {
      return Array.from(getStore(args.rid || 'settings.json').entries());
    }
    if (cmd === 'plugin:store|length') {
      return getStore(args.rid || 'settings.json').size;
    }
    if (cmd === 'plugin:store|clear' || cmd === 'plugin:store|reset') {
      getStore(args.rid || 'settings.json').clear();
      return;
    }
    if (cmd === 'plugin:store|save' || cmd === 'plugin:store|reload') return;

    // ---- Plugin: fs ----
    if (cmd === 'plugin:fs|read_text_file') {
      const p = args.path || '';
      if (fsFiles.has(p)) return fsFiles.get(p);
      // Try REST API for config
      try {
        const r = await fetch('/api/config/get?path=' + encodeURIComponent(p));
        if (r.ok) return await r.text();
      } catch(e) {}
      return '';
    }
    if (cmd === 'plugin:fs|write_file') {
      const p = args.path || '';
      if (typeof args.contents === 'string') {
        fsFiles.set(p, args.contents);
      } else if (args.contents && args.contents.length !== undefined) {
        // Uint8Array or Array
        fsFiles.set(p, String.fromCharCode.apply(null, args.contents));
      }
      return;
    }
    if (cmd === 'plugin:fs|create') { return args.path || null; }
    if (cmd === 'plugin:fs|exists') { return fsFiles.has(args.path || ''); }
    if (cmd === 'plugin:fs|read' || cmd === 'plugin:fs|open' || cmd === 'plugin:fs|fstat') {
      return { size: 0 };
    }
    if (cmd === 'plugin:fs|seek') return 0;
    if (cmd === 'plugin:fs|write' || cmd === 'plugin:fs|ftruncate') return;
    if (cmd === 'plugin:fs|close') return;

    // ---- Plugin: path ----
    if (cmd === 'plugin:path|resolve_directory') {
      const dirs = {
        AppConfig: '/config',
        AppData: '/data',
        AppCache: '/cache',
        AppLog: '/logs',
        Home: '/home',
        Temp: '/tmp',
        Desktop: '/desktop',
        Document: '/docs',
        Download: '/downloads',
      };
      return (dirs[args.directory] || '/data') + (args.path || '');
    }
    if (cmd === 'plugin:path|join') {
      return (args.paths || []).join('/');
    }

    // ---- Plugin: window (stubs for browser) ----
    if (cmd === 'plugin:window|get_all_windows') {
      return [{ label: 'main', title: 'AuroraBox' }];
    }
    if (cmd === 'plugin:window|create') {
      if (args.windowTag === 'sing-box-log') window.open('/logs', '_blank');
      return { label: args.label || 'window', title: args.title || '' };
    }
    if (cmd === 'plugin:window|scale_factor') return window.devicePixelRatio || 1;
    if (cmd === 'plugin:window|inner_size') return { width: window.innerWidth, height: window.innerHeight };
    if (cmd === 'plugin:window|outer_size') return { width: window.outerWidth, height: window.outerHeight };
    if (cmd === 'plugin:window|inner_position') return { x: 0, y: 0 };
    if (cmd === 'plugin:window|is_focused') return document.hasFocus();
    if (cmd === 'plugin:window|is_fullscreen') return !!document.fullscreenElement;
    if (cmd === 'plugin:window|is_maximized') return true;
    if (cmd === 'plugin:window|is_visible') return !document.hidden;
    if (cmd === 'plugin:window|is_minimized') return false;
    if (cmd === 'plugin:window|is_closable') return true;
    if (cmd === 'plugin:window|is_decorated') return true;
    if (cmd === 'plugin:window|is_resizable') return true;
    if (cmd === 'plugin:window|is_maximizable') return true;
    if (cmd === 'plugin:window|is_minimizable') return true;
    if (cmd === 'plugin:window|is_enabled') return true;
    if (cmd === 'plugin:window|is_always_on_top') return false;
    if (cmd === 'plugin:window|title') return 'AuroraBox';
    if (cmd === 'plugin:window|theme') return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    if (cmd.startsWith('plugin:window|set_')) return;  // All set_* are no-ops
    if (cmd.startsWith('plugin:window|')) return;       // Other window ops are no-ops

    // ---- Plugin: app ----
    if (cmd === 'plugin:app|default_window_icon') return null;

    // ---- Plugin: os ----
    if (cmd === 'plugin:os|locale') return navigator.language || 'en-US';

    // ---- Plugin: http (pass through to fetch) ----
    if (cmd === 'plugin:http|fetch') {
      try {
        const resp = await fetch(args.url, {
          method: args.method || 'GET',
          headers: args.headers || {},
          body: args.body || null,
        });
        const rid = nextCbId++;
        const body = await resp.arrayBuffer();
        const headers = {};
        resp.headers.forEach((v, k) => headers[k] = v);
        const result = {
          rid, status: resp.status, statusText: resp.statusText,
          headers, body: new Uint8Array(body),
          ok: resp.ok, redirected: resp.redirected, url: resp.url,
        };
        return result;
      } catch(e) {
        throw new Error('HTTP fetch failed: ' + e.message);
      }
    }
    if (cmd === 'plugin:http|fetch_read_body') return new Uint8Array(0);
    if (cmd === 'plugin:http|fetch_send' || cmd === 'plugin:http|fetch_cancel' || cmd === 'plugin:http|fetch_cancel_body') return;

    // ---- Plugin: menu / tray (stubs) ----
    if (cmd.startsWith('plugin:menu|')) return cmd.includes('items') ? [] : (cmd.includes('new') || cmd.includes('create') ? { rid: 1 } : undefined);
    if (cmd.startsWith('plugin:tray|')) return cmd === 'plugin:tray|new' ? { rid: 1 } : undefined;

    // ---- Plugin: dialog ----
    if (cmd === 'plugin:dialog|message') { window.alert(args.message || ''); return; }

    // ---- Plugin: updater ----
    if (cmd === 'plugin:updater|check') return null;

    // ---- Plugin: image ----
    if (cmd.startsWith('plugin:image|')) return cmd.includes('size') ? { width: 16, height: 16 } : (cmd.includes('new') ? { rid: 1 } : null);

    // ---- Plugin: resources ----
    if (cmd === 'plugin:resources|close') { addLog({cmd:cmd,ts:ts,err:null}); return; }

    // ---- App-specific commands → REST API ----
    try {
      var result = await handleAppCommand(cmd, args);
      addLog({cmd:cmd,ts:ts,err:null});
      return result;
    } catch(e) {
      addLog({cmd:cmd,ts:ts,err:e.message});
      throw e;
    }
  }

  // ============================================================
  // App-specific commands mapped to REST API
  // ============================================================
  async function handleAppCommand(cmd, args) {
    const map = {
      // start: app sends {app, path, mode} where mode is TunProxy|SystemProxy|ManualProxy
      'start':                    { method: 'POST', path: '/api/start',
                                    body: { mode: (args.mode||'').replace('Proxy','').toLowerCase(),
                                            path: args.path, _app: !!args.app } },
      'stop':                     { method: 'POST', path: '/api/stop' },
      'reload_config':            { method: 'POST', path: '/api/reload' },
      'is_running':               { method: 'GET',  path: '/api/status', map: d => d.state === 'running' || d.type === 'Running' },
      'get_engine_state':         { method: 'GET',  path: '/api/status', map: d => ({ type: d.state||d.type||'Idle', epoch: d.epoch||0, mode: d.mode||null }) },
      'clear_engine_error':       { method: 'POST', path: '/api/clear-error' },
      'version':                  { method: 'GET',  path: '/api/version', map: d => d.version || 'unknown' },
      'get_app_version':          { method: 'GET',  path: '/api/version', map: d => d.cli_version || '0.1.0' },
      'read_logs':                { method: 'GET',  path: '/api/logs', map: d => typeof d === 'string' ? d : JSON.stringify(d) },
      'get_app_paths':            { method: 'GET',  path: '/api/paths' },
      'get_lan_ip':               { method: 'GET',  path: '/api/network/lan-ip', map: d => d.ip || '127.0.0.1' },
      'ping_google':              { method: 'GET',  path: '/api/network/ping', map: d => d.ok || false },
      'check_captive_portal_status': { method: 'GET', path: '/api/network/captive', map: d => d.status || 0 },
      'get_captive_redirect_url': { method: 'GET',  path: '/api/network/captive-url', map: d => d.url || '' },
      'open_browser':             { method: 'POST', path: '/api/network/open-url', body: { url: args.url } },
      'run_singbox_tests':        { method: 'POST', path: '/api/proxies/test', body: { outbounds: args.outbounds }, map: d => d.results || d },
      'get_traffic':              { method: 'GET',  path: '/api/traffic', map: d => ({ up: d.up||0, down: d.down||0 }) },
      'prestart_check':           { method: 'GET',  path: '/api/proxy/port-check', map: d => ({ port_occupied: !d.available, orphan_pids: d.pids||[] }) },
      'kill_orphans':             { method: 'POST', path: '/api/proxy/kill-orphans', map: d => ({ success: true, port_released: true }) },
      'get_optimal_local_dns_server': { method: 'GET', path: '/api/dns/optimal', map: d => d.server || '119.29.29.29' },
      'start_chain':              { method: 'POST', path: '/api/chain/start', map: d => d.port || 0 },
      'stop_chain':               { method: 'POST', path: '/api/chain/stop' },
      'engine_probe':             { method: 'GET',  path: '/api/engine/probe', map: d => d.status || 'ok' },
      'engine_ensure_installed':  { method: 'POST', path: '/api/engine/install' },
      'set_native_window_theme':  { method: 'POST', path: '/api/theme' },
      'get_pending_deep_link':    { method: 'GET',  path: '/api/deep-link/pending', map: d => d || null },
      'verify_deep_link_url':     { method: 'POST', path: '/api/deep-link/verify', body: { url: args.url }, map: d => d.valid || false },
      'fetch_config_with_optimal_dns': { method: 'POST', path: '/api/subscriptions/fetch', body: { url: args.url, user_agent: args.userAgent || args.user_agent } },
      'open_devtools':            { method: 'GET',  path: '/api/health', map: () => undefined },
      'create_window':            { method: 'GET',  path: '/api/health', map: () => undefined },
      'open_directory':           { method: 'GET',  path: '/api/health', map: () => undefined },
      'get_tray_icon':            { method: 'GET',  path: '/api/health', map: () => new ArrayBuffer(0) },
    };

    const m = map[cmd];
    if (!m) {
      console.warn('[shim] unknown invoke:', cmd, args);
      return null;
    }

    try {
      const opts = { method: m.method || 'POST', headers: { 'Content-Type': 'application/json' } };
      if (m.body && m.method !== 'GET') opts.body = JSON.stringify(m.body);
      const resp = await fetch(m.path, opts);
      if (!resp.ok) throw new Error(await resp.text().catch(() => 'Error ' + resp.status));
      const data = await resp.json();
      return m.map ? m.map(data) : data;
    } catch(e) {
      console.error('[shim] invoke(' + cmd + ') failed:', e.message);
      throw e;
    }
  }

  // ============================================================
  // Event polling
  // ============================================================
  function startPoll(event, path, ms) {
    if (pollIntervals[event]) return;
    pollIntervals[event] = setInterval(async () => {
      try {
        const resp = await fetch(path);
        const data = await resp.json();
        const str = JSON.stringify(data);
        if (str !== lastStateStr) {
          lastStateStr = str;
          [event, 'status-changed'].forEach(evt => {
            if (evtListeners.has(evt)) {
              evtListeners.get(evt).forEach(l => {
                invokeCb(l.handlerId, { event: evt, payload: data });
              });
            }
          });
        }
      } catch(e) {}
    }, ms);
  }

  let lastDLKey = null;
  async function checkDL() {
    try {
      const resp = await fetch('/api/deep-link/pending');
      const data = await resp.json();
      if (data && data.data) {
        const key = data.data + '|' + (data.apply ? '1' : '0');
        if (key !== lastDLKey) {
          lastDLKey = key;
          if (evtListeners.has('deep_link_pending')) {
            evtListeners.get('deep_link_pending').forEach(l => {
              invokeCb(l.handlerId, { event: 'deep_link_pending', payload: data });
            });
          }
        }
      }
    } catch(e) {}
  }

  // ============================================================
  // Set up global Tauri internals
  // ============================================================
  const osPlatform = (() => {
    const p = navigator.platform || '';
    if (p.includes('Win')) return 'windows';
    if (p.includes('Mac')) return 'macos';
    return 'linux';
  })();

  window.__TAURI_INTERNALS__ = {
    invoke: shimInvoke,
    transformCallback,
    unregisterCallback,
    metadata: { currentWindow: { label: 'main' } },
  };

  window.__TAURI_OS_PLUGIN_INTERNALS__ = {
    platform: osPlatform,
    version: '1.0.0',
    os_type: osPlatform,
    arch: navigator.userAgent.includes('aarch64') ? 'aarch64' : 'x86_64',
  };

  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener(event, handlerId) {
      if (evtListeners.has(event)) {
        const list = evtListeners.get(event);
        const idx = list.findIndex(l => l.handlerId === handlerId);
        if (idx >= 0) list.splice(idx, 1);
      }
    },
  };

  window.__TAURI_TO_IPC_KEY__ = '__TAURI_TO_IPC_KEY__';

  // Restore persisted store values from localStorage
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (key && key.startsWith('s_')) {
      try {
        const val = JSON.parse(localStorage.getItem(key));
        getStore('settings.json').set(key.slice(2), val);
      } catch(e) {}
    }
  }

  console.log('[tauri-shim] v2 loaded — ' + osPlatform);
})();

// ---- Error reporter (catches and displays startup errors) ----
window.addEventListener('error', function(e) {
  var root = document.getElementById('root');
  if (root && root.children.length === 0) {
    root.innerHTML = '<div style="padding:20px;color:red;font-family:monospace;background:#1a0000;border:2px solid red;border-radius:8px;margin:20px;"><h2>⚠️ Application Error</h2><pre style="white-space:pre-wrap;word-break:break-all;">' + e.message + '\n\n' + (e.error ? e.error.stack : '') + '</pre><p>Source: ' + (e.filename || 'unknown') + ':' + (e.lineno || '?') + '</p></div>';
  }
});

// Report unhandled promise rejections
window.addEventListener('unhandledrejection', function(e) {
  var root = document.getElementById('root');
  if (root && root.children.length === 0) {
    root.innerHTML = '<div style="padding:20px;color:orange;font-family:monospace;background:#1a1000;border:2px solid orange;border-radius:8px;margin:20px;"><h2>⚠️ Unhandled Promise Rejection</h2><pre style="white-space:pre-wrap;word-break:break-all;">' + (e.reason ? (e.reason.message || String(e.reason)) : 'Unknown') + '</pre></div>';
  }
});

console.log('[tauri-shim] Debug mode: errors will be displayed on page');
