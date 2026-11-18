// The index's source (#292): Discord's own in-page stores, found READ-ONLY through its webpack chunk global and
// kept on window.__xsdStores once the main ones are found. A probe chunk is pushed (and popped) to get webpack's
// require; its module cache (require.c) is searched. Candidates are each module's exports and every own property of
// them (minified keys), each access guarded. Flux stores are class instances: found first by their own name
// (getName() or constructor.displayName), then by the methods on their prototype; Flux's registry of every store
// (a getAll() on a store class's parent) is added if reachable. Only the methods named below are ever called, and
// only to read, except selectVoiceChannel by store_join.js. Nothing credential-like is read: a unit test asserts
// this file and the scripts that use it hold none of the words for one. Defines xsdRequire() and xsdStores().
const xsdRequire = () => {
  const chunk = window.webpackChunkdiscord_app;
  if (!chunk || typeof chunk.push !== "function") return null;
  let req = null;
  try {
    chunk.push([[Symbol("xsd")], {}, (r) => { req = r; }]);
    chunk.pop();
  } catch (e) {
    return null;
  }
  return req;
};
const xsdStores = () => {
  if (window.__xsdStores) return window.__xsdStores;
  const out = { found: {}, why: "", diag: {} };
  const req = xsdRequire();
  if (!req || !req.c) {
    out.why = window.webpackChunkdiscord_app ? "no require or module cache from the probe chunk" : "no webpackChunkdiscord_app";
    return out;
  }
  const get = (o, k) => { try { return o[k]; } catch (e) { return undefined; } };
  const isDom = (o) => { try { return o === window || o === document || (typeof Node !== "undefined" && o instanceof Node); } catch (e) { return true; } };
  const candidates = new Set();
  let objects = 0;
  const modules = Object.keys(req.c);
  for (const id of modules) {
    const e = get(get(req.c, id), "exports");
    if (!e || (typeof e !== "object" && typeof e !== "function") || isDom(e)) continue;
    objects++;
    candidates.add(e);
    let keys = [];
    try { keys = Object.getOwnPropertyNames(e); } catch (x) {}
    for (const k of keys) {
      const v = get(e, k);
      if (v && (typeof v === "object" || typeof v === "function") && !isDom(v)) candidates.add(v);
    }
  }
  const fn = (o, n) => { try { return typeof o[n] === "function"; } catch (x) { return false; } };
  const nameOf = (o) => {
    try { if (fn(o, "getName")) { const n = o.getName(); if (typeof n === "string") return n; } } catch (x) {}
    try { const c = o.constructor; if (c && typeof c.displayName === "string") return c.displayName; } catch (x) {}
    return "";
  };
  const byName = new Map();
  const add = (o) => { const n = nameOf(o); if (n && !byName.has(n) && typeof o === "object") byName.set(n, o); };
  for (const c of candidates) add(c);
  // Flux's own registry: a getAll() on a store class's parent.
  let registry = -1;
  for (const s of byName.values()) {
    let k = s && s.constructor;
    for (let i = 0; i < 4 && k && registry < 0; i++, k = Object.getPrototypeOf(k)) {
      if (fn(k, "getAll")) {
        try {
          const list = k.getAll();
          if (Array.isArray(list)) { registry = list.length; for (const x of list) add(x); }
        } catch (x) {}
      }
    }
    if (registry >= 0) break;
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
  out.modules = modules.length;
  out.diag = {
    modules: modules.length,
    factories: req.m ? Object.keys(req.m).length : -1,
    exportObjects: objects,
    candidates: candidates.size,
    named: byName.size,
    registry,
    names: [...byName.keys()].slice(0, 60),
  };
  // Kept only once the main stores are there: an early, failed search is not remembered.
  if (S.guilds && (S.channels || S.guildChannels)) {
    Object.defineProperty(window, "__xsdStores", { value: out, enumerable: false, configurable: true });
  }
  return out;
};
