// The index's source (#292): Discord's own in-page stores, found READ-ONLY and kept on window.__xsdStores once the
// main ones are found. First route: webpack. Several webpack runtimes share the chunk global webpackChunkdiscord_app
// and each wraps its push, so one probe chunk (a fresh id each time, popped after) is answered once per runtime with
// that runtime's require: every one is kept, and every module cache (require.c, loaded modules only: no factory is
// ever run) is searched, the largest first. Candidates are each module's exports and every own property of them
// (minified keys), each access guarded. Second route, only if the first finds no guild store: React's fiber tree
// from #app-mount (capped at 50k nodes), its props, state, hooks and context values. Flux stores are class instances:
// found first by their own name (getName() or constructor.displayName), then by the methods on their prototype;
// Flux's registry of every store (getAll() on a store class or its parents) is added if reachable. Only the methods
// named below are ever called, and only to read, except selectVoiceChannel by store_join.js. Nothing credential-like
// is read: a unit test asserts this file and the scripts that use it hold none of the words for one.
// Defines xsdRequires() and xsdStores().
const xsdRequires = () => {
  const chunk = window.webpackChunkdiscord_app;
  if (!chunk || typeof chunk.push !== "function") return [];
  const all = [];
  window.__xsdProbe = (window.__xsdProbe || 0) + 1;
  try {
    chunk.push([[Symbol("xsd-probe-" + window.__xsdProbe)], {}, (r) => { if (r && !all.includes(r)) all.push(r); }]);
    chunk.pop();
  } catch (e) {}
  const size = (r) => { try { return r.c ? Object.keys(r.c).length : 0; } catch (e) { return 0; } };
  return all.sort((a, b) => size(b) - size(a));
};
const xsdStores = () => {
  if (window.__xsdStores) return window.__xsdStores;
  const out = { found: {}, why: "", diag: {} };
  const reqs = xsdRequires();
  const get = (o, k) => { try { return o[k]; } catch (e) { return undefined; } };
  const isObj = (v) => !!v && (typeof v === "object" || typeof v === "function");
  const isDom = (o) => { try { return o === window || o === document || (typeof Node !== "undefined" && o instanceof Node); } catch (e) { return true; } };
  const fn = (o, n) => { try { return typeof o[n] === "function"; } catch (x) { return false; } };
  const nameOf = (o) => {
    try { if (fn(o, "getName")) { const n = o.getName(); if (typeof n === "string") return n; } } catch (x) {}
    try { const c = o.constructor; if (c && typeof c.displayName === "string") return c.displayName; } catch (x) {}
    return "";
  };
  const candidates = new Set();
  const byName = new Map();
  const add = (o) => {
    if (!isObj(o) || isDom(o) || candidates.has(o)) return;
    candidates.add(o);
    if (typeof o !== "object") return;
    const n = nameOf(o);
    if (n && !byName.has(n)) byName.set(n, o);
  };
  // Route 1: every webpack runtime's loaded modules, the largest cache first.
  let objects = 0;
  let modules = 0;
  const runtimes = [];
  for (const req of reqs) {
    const c = get(req, "c");
    const m = get(req, "m");
    let ids = [];
    try { ids = c ? Object.keys(c) : []; } catch (x) {}
    let factories = -1;
    try { factories = m ? Object.keys(m).length : -1; } catch (x) {}
    runtimes.push({ cache: ids.length, factories });
    modules += ids.length;
    for (const id of ids) {
      const e = get(get(c, id), "exports");
      if (!isObj(e) || isDom(e)) continue;
      objects++;
      add(e);
      let keys = [];
      try { keys = Object.getOwnPropertyNames(e); } catch (x) {}
      for (const k of keys) add(get(e, k));
    }
  }
  const viaWebpack = byName.size;
  // Flux's own registry: getAll() on a store's class or its parents; also whether a dispatcher was reached.
  let registry = -1;
  let dispatcher = false;
  const reachRegistry = () => {
    for (const s of [...byName.values()]) {
      if (isObj(get(s, "_dispatcher"))) dispatcher = true;
      let k = get(s, "constructor");
      for (let i = 0; i < 4 && k && registry < 0; i++, k = Object.getPrototypeOf(k)) {
        if (!fn(k, "getAll")) continue;
        try {
          const list = k.getAll();
          if (Array.isArray(list)) { registry = list.length; for (const x of list) add(x); }
        } catch (x) {}
      }
      if (registry >= 0) return;
    }
  };
  reachRegistry();
  const hasGuilds = () => byName.has("GuildStore") || [...candidates].some((o) => typeof o === "object" && fn(o, "getGuilds") && fn(o, "getGuild"));
  // Route 2, only if needed: React's fiber tree.
  let fiberNodes = -1;
  let fiberRoot = "";
  if (!hasGuilds()) {
    fiberNodes = 0;
    const mount = document.querySelector("#app-mount");
    let root = null;
    if (mount) {
      for (const k of Object.keys(mount)) {
        if (k.startsWith("__reactContainer$") || k.startsWith("__reactFiber$")) { root = get(mount, k); fiberRoot = k.split("$")[0]; break; }
      }
      if (!root) {
        const legacy = get(mount, "_reactRootContainer");
        const r = get(get(legacy, "_internalRoot"), "current");
        if (r) { root = r; fiberRoot = "_reactRootContainer"; }
      }
    }
    if (!root) {
      for (const el of document.querySelectorAll("#app-mount *")) {
        const k = Object.keys(el).find((x) => x.startsWith("__reactFiber$"));
        if (k) { root = get(el, k); fiberRoot = "first element's __reactFiber"; break; }
      }
    }
    if (root) {
      while (get(root, "return")) root = get(root, "return");
      const look = (v, depth) => {
        if (!isObj(v) || isDom(v)) return;
        add(v);
        if (depth <= 0 || typeof v !== "object") return;
        let keys = [];
        try { keys = Array.isArray(v) ? [] : Object.keys(v).slice(0, 50); } catch (x) {}
        for (const k of keys) { const w = get(v, k); if (isObj(w) && k !== "children" && k !== "_owner") look(w, depth - 1); }
      };
      const stack = [root];
      fiberNodes = 0;
      while (stack.length && fiberNodes < 50000) {
        const f = stack.pop();
        fiberNodes++;
        look(get(f, "memoizedProps"), 1);
        let h = get(f, "memoizedState");
        for (let i = 0; i < 30 && isObj(h); i++) {
          look(get(h, "memoizedState"), 1);
          look(get(get(h, "queue"), "lastRenderedState"), 1);
          h = get(h, "next");
        }
        for (let d = get(get(f, "dependencies"), "firstContext"), i = 0; d && i < 30; d = get(d, "next"), i++) look(get(d, "memoizedValue"), 1);
        const sib = get(f, "sibling");
        const child = get(f, "child");
        if (sib) stack.push(sib);
        if (child) stack.push(child);
      }
      if (registry < 0) reachRegistry();
    }
  }
  const shape = (all) => [...byName.values(), ...candidates].find((o) => typeof o === "object" && all.every((n) => fn(o, n))) || null;
  const pick = (names, methods) => {
    for (const n of names) if (byName.has(n)) return byName.get(n);
    return shape(methods);
  };
  const S = {
    guilds: pick(["GuildStore"], ["getGuilds", "getGuild"]),
    channels: pick(["ChannelStore"], ["getChannel", "hasChannel"]),
    guildChannels: pick(["GuildChannelStore"], ["getChannels", "getDefaultChannel"]),
    privateSort: pick(["PrivateChannelSortStore", "ChannelListStore"], ["getPrivateChannelIds"]) || shape(["getSortedPrivateChannels"]),
    voice: pick(["VoiceStateStore"], ["getVoiceStatesForChannel"]),
    users: pick(["UserStore"], ["getUser", "getCurrentUser"]),
    speaking: pick(["SpeakingStore"], ["isSpeaking"]),
    media: pick(["MediaEngineStore"], ["isSelfMute", "isSelfDeaf"]),
    voiceActions: shape(["selectVoiceChannel"]),
  };
  out.S = S;
  const names = {
    guilds: ["getGuilds", "getGuild"],
    channels: ["getChannel", "hasChannel", "getMutableGuildChannelsForGuild", "getSortedPrivateChannels"],
    guildChannels: ["getChannels", "getDefaultChannel"],
    privateSort: ["getPrivateChannelIds", "getSortedPrivateChannels"],
    voice: ["getVoiceStatesForChannel", "getAllVoiceStates"],
    users: ["getUser", "getCurrentUser"],
    speaking: ["isSpeaking", "isAnyoneElseSpeaking"],
    media: ["isSelfMute", "isSelfDeaf"],
    voiceActions: ["selectVoiceChannel"],
  };
  for (const [k, list] of Object.entries(names)) {
    out.found[k] = S[k] ? [nameOf(S[k]) || "(by shape)", ...list.filter((n) => fn(S[k], n))] : [];
  }
  if (!reqs.length) out.why = window.webpackChunkdiscord_app ? "no webpack runtime answered the probe chunk" : "no webpackChunkdiscord_app";
  out.modules = modules;
  out.diag = {
    runtimes,
    modules,
    exportObjects: objects,
    candidates: candidates.size,
    named: byName.size,
    namedViaWebpack: viaWebpack,
    registry,
    dispatcher,
    fiberRoot,
    fiberNodes,
    names: [...byName.keys()].slice(0, 60),
  };
  // Kept only once the main stores are there: an early, failed search is not remembered.
  if (S.guilds && (S.channels || S.guildChannels)) {
    Object.defineProperty(window, "__xsdStores", { value: out, enumerable: false, configurable: true });
  }
  return out;
};
