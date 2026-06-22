/**
 * Tauri API Compatibility Shim for AuroraBox Web
 *
 * Translates Tauri @tauri-apps/api calls to REST API calls (axum backend)
 * and provides browser-compatible polyfills for Tauri plugins.
 *
 * Load this BEFORE the main application bundle.
 */
(function() {
  'use strict';

  const API_BASE = '';

  // Track event listeners for later emit
  const listeners = {};
  const pendingDeepLinks = [];

  // ============================================================
  // invoke() → fetch() command mapping
  // ============================================================
  const COMMAND_MAP = {
    // Core engine
    'start':                  { method: 'POST', path: '/api/start',                  mapArgs: a => ({ mode: a.mode, subscription: a.subscription ? String(a.subscription) : undefined }) },
    'stop':                   { method: 'POST', path: '/api/stop' },
    'reload_config':          { method: 'POST', path: '/api/reload' },
    'is_running':             { method: 'GET',  path: '/api/status',                 mapResult: r => r.state === 'running' },
    'get_engine_state':       { method: 'GET',  path: '/api/status' },
    'clear_engine_error':     { method: 'POST', path: '/api/clear-error' },

    // Config generation
    'get_config':             { method: 'POST', path: '/api/config/generate' },

    // Shell / info
    'version':                { method: 'GET',  path: '/api/version',                mapResult: r => r.version || 'sing-box unknown' },
    'get_app_version':        { method: 'GET',  path: '/api/version',                mapResult: r => r.cli_version || '0.1.0' },
    'read_logs':              { method: 'GET',  path: '/api/logs' },
    'get_app_paths':          { method: 'GET',  path: '/api/paths' },

    // Proxy testing
    'run_singbox_tests':      { method: 'POST', path: '/api/proxies/test',           mapArgs: a => ({ outbounds: a.outbounds }) },

    // Traffic
    'get_traffic':            { method: 'GET',  path: '/api/traffic' },

    // Network
    'get_lan_ip':             { method: 'GET',  path: '/api/network/lan-ip',         mapResult: r => r.ip || '127.0.0.1' },
    'ping_google':            { method: 'GET',  path: '/api/network/ping',           mapResult: r => r.ok || false },
    'check_captive_portal_status': { method: 'GET', path: '/api/network/captive',    mapResult: r => r.status || 0 },
    'open_browser':           { method: 'POST', path: '/api/network/open-url' },
    'get_captive_redirect_url': { method: 'GET', path: '/api/network/captive-url',   mapResult: r => r.url || '' },

    // Prestart
    'prestart_check':         { method: 'GET',  path: '/api/proxy/port-check',       mapResult: r => ({ port_occupied: !r.available, orphan_pids: r.pids || [] }) },
    'kill_orphans':           { method: 'POST', path: '/api/proxy/kill-orphans',     mapResult: r => ({ success: r.success, port_released: r.success }) },

    // DNS
    'get_optimal_local_dns_server': { method: 'GET', path: '/api/dns/optimal',       mapResult: r => r.server || '119.29.29.29' },

    // Chain proxy
    'start_chain':            { method: 'POST', path: '/api/chain/start',            mapResult: r => r.port || 0 },
    'stop_chain':             { method: 'POST', path: '/api/chain/stop' },

    // Subscriptions
    'get_subscriptions':      { method: 'GET',  path: '/api/subscriptions' },
    'add_subscription':       { method: 'POST', path: '/api/subscriptions' },
    'delete_subscription':    { method: 'DELETE', path: '/api/subscriptions' },
    'fetch_config_with_optimal_dns': { method: 'POST', path: '/api/subscriptions/fetch' },
    'verify_deep_link_url':   { method: 'POST', path: '/api/deep-link/verify',       mapResult: r => r.valid || false },

    // Deep link
    'get_pending_deep_link':  { method: 'GET',  path: '/api/deep-link/pending',      mapResult: r => r || null },

    // Engine install/probe
    'engine_probe':           { method: 'GET',  path: '/api/engine/probe',           mapResult: r => r.status || 'ok' },
    'engine_ensure_installed': { method: 'POST', path: '/api/engine/install' },

    // Theme (no-op in browser)
    'set_native_window_theme': { method: 'POST', path: '/api/theme' },
  };

  // ============================================================
  // Core invoke()
  // ============================================================
  async function shimInvoke(cmd, args) {
    args = args || {};
    const mapping = COMMAND_MAP[cmd];

    if (mapping) {
      try {
        let body = null;
        let path = mapping.path;

        if (mapping.mapArgs) {
          body = JSON.stringify(mapping.mapArgs(args));
        } else if (Object.keys(args).length > 0) {
          // Default: send args as JSON body for POST, query params for GET
          if (mapping.method === 'GET' || mapping.method === 'DELETE') {
            const params = new URLSearchParams();
            for (const [k, v] of Object.entries(args)) {
              if (v !== undefined && v !== null) params.append(k, String(v));
            }
            const qs = params.toString();
            if (qs) path += '?' + qs;
          } else {
            body = JSON.stringify(args);
          }
        }

        const opts = {
          method: mapping.method || 'POST',
          headers: { 'Content-Type': 'application/json' },
        };
        if (body && mapping.method !== 'GET') opts.body = body;

        const resp = await fetch(API_BASE + path, opts);
        if (!resp.ok) {
          const err = await resp.json().catch(() => ({ error: resp.statusText }));
          throw new Error(err.error || resp.statusText);
        }
        const data = await resp.json();

        if (mapping.mapResult) return mapping.mapResult(data);
        return data;
      } catch (e) {
        console.error('[tauri-shim] invoke(' + cmd + ') failed:', e);
        throw e;
      }
    }

    // Unmapped commands: warn and return sensible defaults
    console.warn('[tauri-shim] unmapped invoke:', cmd, args);
    if (cmd === 'open_devtools') return; // no-op
    if (cmd === 'create_window') return; // no-op: opens in same tab
    if (cmd === 'open_directory') return; // no-op
    if (cmd === 'get_tray_icon') return new ArrayBuffer(0);
    return null;
  }

  // ============================================================
  // Event system
  // ============================================================
  function shimListen(event, handler) {
    if (!listeners[event]) listeners[event] = [];
    listeners[event].push(handler);
    console.log('[tauri-shim] listening for event:', event);

    // For engine-state, poll /api/status
    if (event === 'engine-state') {
      startPolling('engine-state', '/api/status', 2000);
    }
    if (event === 'status-changed') {
      startPolling('status-changed', '/api/status', 2000);
    }
    if (event === 'deep_link_pending') {
      // Check immediately
      checkDeepLink();
      setInterval(checkDeepLink, 3000);
    }

    return () => {
      listeners[event] = listeners[event].filter(h => h !== handler);
    };
  }

  function shimEmit(event, payload) {
    console.log('[tauri-shim] emit:', event, payload);
  }

  const pollingIntervals = {};
  let lastState = null;
  function startPolling(event, path, intervalMs) {
    if (pollingIntervals[event]) return;
    pollingIntervals[event] = setInterval(async () => {
      try {
        const resp = await fetch(API_BASE + path);
        const data = await resp.json();
        const stateStr = JSON.stringify(data);
        if (stateStr !== lastState) {
          lastState = stateStr;
          if (listeners[event]) {
            listeners[event].forEach(h => {
              try { h(data); } catch(e) { console.error('[tauri-shim] event handler error:', e); }
            });
          }
        }
      } catch(e) { /* ignore poll errors */ }
    }, intervalMs);
  }

  let lastDeepLinkCheck = null;
  async function checkDeepLink() {
    try {
      const resp = await fetch(API_BASE + '/api/deep-link/pending');
      const data = await resp.json();
      if (data && data.data) {
        const key = data.data + '|' + (data.apply ? '1' : '0');
        if (key !== lastDeepLinkCheck) {
          lastDeepLinkCheck = key;
          if (listeners['deep_link_pending']) {
            listeners['deep_link_pending'].forEach(h => {
              try { h(data); } catch(e) {}
            });
          }
        }
      }
    } catch(e) {}
  }

  // ============================================================
  // SQL plugin shim
  // ============================================================
  class ShimDatabase {
    constructor() {
      this._db = null;
    }
    static async load(url) {
      const db = new ShimDatabase();
      return db;
    }
    async execute(sql, bindings) {
      const resp = await fetch(API_BASE + '/api/db/execute', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ sql, bindings: bindings || [] }),
      });
      if (!resp.ok) throw new Error('SQL error');
      return await resp.json();
    }
    async select(sql, bindings) {
      const resp = await fetch(API_BASE + '/api/db/select', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ sql, bindings: bindings || [] }),
      });
      if (!resp.ok) throw new Error('SQL error');
      return await resp.json();
    }
    async close() {}
  }

  // ============================================================
  // Store plugin shim (localStorage-based)
  // ============================================================
  class ShimStore {
    constructor(name) {
      this._prefix = 'aurorabox_store_' + name + '_';
      // Load existing
      this._cache = {};
    }
    async get(key) {
      const val = localStorage.getItem(this._prefix + key);
      if (val === null) return null;
      try { return JSON.parse(val); } catch { return val; }
    }
    async set(key, value) {
      localStorage.setItem(this._prefix + key, JSON.stringify(value));
      this._cache[key] = value;
    }
    async delete(key) {
      localStorage.removeItem(this._prefix + key);
      delete this._cache[key];
    }
    async clear() {
      const keys = Object.keys(localStorage).filter(k => k.startsWith(this._prefix));
      keys.forEach(k => localStorage.removeItem(k));
      this._cache = {};
    }
    async keys() {
      return Object.keys(localStorage)
        .filter(k => k.startsWith(this._prefix))
        .map(k => k.slice(this._prefix.length));
    }
    async values() {
      const ks = await this.keys();
      const vals = [];
      for (const k of ks) vals.push(await this.get(k));
      return vals;
    }
    async entries() {
      const ks = await this.keys();
      const entries = [];
      for (const k of ks) entries.push([k, await this.get(k)]);
      return entries;
    }
    async length() { return (await this.keys()).length; }
    async load() {}
    async save() {}
    async reload() {}
    onKeyChange(key, handler) {
      window.addEventListener('storage', (e) => {
        if (e.key === this._prefix + key) {
          handler(e.newValue ? JSON.parse(e.newValue) : null);
        }
      });
      return () => {};
    }
  }

  class ShimLazyStore {
    constructor(name) {
      this._store = new ShimStore(name);
    }
    async get(key) { return this._store.get(key); }
    async set(key, value) { return this._store.set(key, value); }
    async save() {}
  }

  // ============================================================
  // Window / app shims
  // ============================================================
  function getCurrentWindow() { return { label: 'main', onCloseRequested: fn => {} }; }
  const appWindow = { label: 'main', onCloseRequested: fn => {} };
  function getAllWindows() { return [appWindow]; }
  const defaultWindowIcon = null;
  function relaunch() { location.reload(); }

  // ============================================================
  // Path shims
  // ============================================================
  const path = {
    appConfigDir: async () => '/config',
    appDataDir: async () => '/data',
    appCacheDir: async () => '/cache',
    appLogDir: async () => '/logs',
    deserialize: async (p) => p,
    join: (...parts) => parts.join('/'),
    basename: (p) => p.split('/').pop(),
    dirname: (p) => p.split('/').slice(0, -1).join('/'),
    extname: (p) => { const parts = p.split('.'); return parts.length > 1 ? '.' + parts.pop() : ''; },
  };

  // ============================================================
  // FS shims
  // ============================================================
  const BaseDirectory = { AppConfig: 1, AppData: 2 };
  const fs = {
    writeFile: async (path, contents) => { console.log('[shim] writeFile:', path); return true; },
    readTextFile: async (path) => { console.log('[shim] readTextFile:', path); return ''; },
    create: async (path) => { console.log('[shim] create:', path); return true; },
    exists: async (path) => { return false; },
    BaseDirectory,
  };

  // ============================================================
  // OS shims
  // ============================================================
  function type() { return navigator.platform.includes('Win') ? 'windows' : (navigator.platform.includes('Mac') ? 'macos' : 'linux'); }
  function platform() { return type(); }
  function arch() { return 'x86_64'; }
  function version() { return '1.0.0'; }
  function locale() { return navigator.language || 'en-US'; }
  const OsType = { Windows: 'windows', MacOS: 'macos', Linux: 'linux' };
  const Arch = { X8664: 'x86_64', Aarch64: 'aarch64' };

  // ============================================================
  // Shell / process shims
  // ============================================================
  const shell = {
    open: async (url) => { window.open(url, '_blank'); },
    execute: async (cmd, args) => { console.log('[shim] shell execute:', cmd, args); return { code: 0, stdout: '', stderr: '' }; },
    sidecar: (name) => ({ spawn: () => ({ on: () => {}, kill: async () => {} }) }),
  };
  const Command = { create: (cmd, args) => ({ execute: async () => ({ code: 0, stdout: '', stderr: '' }) }) };

  // ============================================================
  // Menu / Tray shims (no-op in browser)
  // ============================================================
  class ShimMenu {
    static async default() { return new ShimMenu(); }
    static async new(opts) { return new ShimMenu(); }
    async setAsAppMenu() {}
    async setAsWindowMenu() {}
    async popup() {}
    async close() {}
    async items() { return []; }
    async getItem(id) { return null; }
    async setText(id, text) {}
  }
  class ShimTrayIcon {
    static async new(opts) { return new ShimTrayIcon(); }
    async setMenu(menu) {}
    async setTooltip(text) {}
    async close() {}
    onEvent(handler) {}
  }
  const TrayIconEvent = { Click: 'click', DoubleClick: 'doubleClick' };

  // ============================================================
  // Dialog shims
  // ============================================================
  async function confirm(msg, opts) { return window.confirm(msg); }
  async function message(msg, opts) { window.alert(msg); }

  // ============================================================
  // Other plugin shims
  // ============================================================
  async function checkUpdate() { return null; }
  async function installUpdate() {}
  const Update = {};
  async function isEnabled() { return false; }
  async function enable() {}
  async function disable() {}
  async function writeText(text) { navigator.clipboard?.writeText(text); }
  async function openUrl(url) { window.open(url, '_blank'); }
  async function fetchUrl(url, opts) { return fetch(url, opts); }
  async function relaunchApp() { location.reload(); }

  // ============================================================
  // Window state plugin
  // ============================================================
  const windowState = {
    restore: async () => {},
    save: async () => {},
  };

  // ============================================================
  // Build the window.__TAURI__ object
  // ============================================================
  window.__TAURI__ = {
    core: { invoke: shimInvoke },
    event: { listen: shimListen, emit: shimEmit, TauriEvent: {} },
    window: {
      getCurrentWindow,
      getAllWindows,
      appWindow,
      currentMonitor: () => null,
      primaryMonitor: () => null,
      availableMonitors: () => [],
      PhysicalSize: class {},
      LogicalSize: class {},
    },
    app: { defaultWindowIcon, getName: () => 'AuroraBox', getVersion: () => '0.1.0' },
    path,
    fs,
    os: { type, platform, arch, version, locale, OsType, Arch },
    shell,
    menu: { Menu: ShimMenu, MenuItem: class {}, Submenu: class {}, PredefinedMenuItem: class {} },
    tray: { TrayIcon: ShimTrayIcon, TrayIconEvent },
    dialog: { confirm, message, open: async () => null, save: async () => null },
    updater: { check: checkUpdate, install: installUpdate, Update },
    autostart: { isEnabled, enable, disable },
    clipboard: { writeText, readText: async () => '' },
    opener: { openUrl, openPath: async () => {} },
    process: { relaunch: relaunchApp, exit: () => {} },
    http: { fetch: fetchUrl },
    notification: { sendNotification: async () => {}, isPermissionGranted: async () => true, requestPermission: async () => 'granted' },
    globalShortcut: { register: async () => {}, unregister: async () => {} },
    deepLink: { onOpenUrl: fn => {} },
    log: { info: console.log, warn: console.warn, error: console.error, debug: console.debug },
    store: { Store: ShimStore, LazyStore: ShimLazyStore },
    sql: { default: ShimDatabase, Database: ShimDatabase },
    fs2: fs,
    process2: { relaunch: relaunchApp },
    singleInstance: { on: () => {} },
    windowState,
  };

  // ============================================================
  // Also patch the module-level API for direct ES imports
  // The frontend imports like: import { invoke } from '@tauri-apps/api/core'
  // Vite bundles these, so we need to patch at the module level too.
  // This is done by aliasing in the build config, but since we're serving
  // pre-built files, we patch the global and rely on the fact that Vite
  // tree-shakes to the bundled code. The actual Tauri calls go through
  // __TAURI_INTERNALS__ or similar internal mechanisms.
  // ============================================================

  // Expose as ES module-compatible global
  window.__TAURI_INTERNALS__ = window.__TAURI__;

  // Override fetch for tauri-specific URL schemes
  const origFetch = window.fetch;
  window.fetch = function(url, opts) {
    if (typeof url === 'string' && (url.startsWith('tauri://') || url.startsWith('ipc://'))) {
      console.log('[tauri-shim] blocked fetch to:', url);
      return Promise.resolve(new Response('{}', { status: 200, headers: { 'Content-Type': 'application/json' } }));
    }
    return origFetch.apply(this, arguments);
  };

  console.log('[tauri-shim] AuroraBox Web compatibility layer loaded');
  console.log('[tauri-shim] API base:', API_BASE || '(same origin)');
})();
