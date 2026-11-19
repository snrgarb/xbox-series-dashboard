// Finds Discord's own in-page Flux stores, READ-ONLY (#198; docs/discord-web.md, "Reading"). Ported from the voice
// spike's search (#292, spikes/discord/src/js/store_find.js), which found them on the real discord.com: Discord's web
// client is a webpack bundle whose chunk global `webpackChunkdiscord_app` takes one probe chunk, which hands back that
// runtime's `require` and its module cache. Several runtimes share the global, each answers, so every cache is
// searched, the largest first. No factory is ever run: only modules already loaded are looked at. Every module's
// exports and each of their own properties are candidates (each access guarded). A Flux store is found by its own name
// (`getName()` or `constructor.displayName`), else by the methods it has; Flux's registry (`getAll()` on a store's
// class) adds the ones no module exported. There is no DOM route and no fiber route: stores only.
//
// What this file never touches: anything that identifies the session, anything the browser stores, a request or the
// network. A test (tests/scripts.rs) fails the build if it, or direct.js, holds the words for one.
//
// Defines xsdFindStores(): { S, found, missing, why, modules }.
// Every webpack runtime's answer to a probe chunk (a fresh id each time, popped after), the largest cache first.
const xsdRuntimes = () => {
  const chunk = window.webpackChunkdiscord_app;
  const reqs = [];
  if (!chunk || typeof chunk.push !== "function") return reqs;
  window.__xsdProbe = (window.__xsdProbe || 0) + 1;
  try {
    chunk.push([[Symbol("xsd-probe-" + window.__xsdProbe)], {}, (r) => { if (r && !reqs.includes(r)) reqs.push(r); }]);
    chunk.pop();
  } catch (e) {}
  const size = (r) => { try { return r.c ? Object.keys(r.c).length : 0; } catch (e) { return 0; } };
  return reqs.sort((a, b) => size(b) - size(a));
};
// How many modules the page has loaded: the search waits until it stops growing.
const xsdCacheSize = () => {
  let n = 0;
  for (const r of xsdRuntimes()) { try { n += r.c ? Object.keys(r.c).length : 0; } catch (e) {} }
  return n;
};
const xsdFindStores = () => {
  const out = { S: {}, found: {}, missing: [], why: "", modules: 0, copies: "" };
  if (!window.webpackChunkdiscord_app || typeof window.webpackChunkdiscord_app.push !== "function") {
    out.why = "no chunk global";
    return out;
  }
  const reqs = xsdRuntimes();
  if (!reqs.length) {
    out.why = "no runtime answered";
    return out;
  }
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
  // Which runtime (by its place, largest first) each candidate was first seen in: for the copies line.
  const runtimeOf = new Map();
  let ri = -1;
  const add = (o) => {
    if (!isObj(o) || isDom(o) || candidates.has(o)) return;
    candidates.add(o);
    runtimeOf.set(o, ri);
    if (typeof o !== "object") return;
    const n = nameOf(o);
    if (n && !byName.has(n)) byName.set(n, o);
  };
  for (const req of reqs) {
    ri++;
    const c = get(req, "c");
    let ids = [];
    try { ids = c ? Object.keys(c) : []; } catch (x) {}
    out.modules += ids.length;
    for (const id of ids) {
      const e = get(get(c, id), "exports");
      if (!isObj(e) || isDom(e)) continue;
      add(e);
      let keys = [];
      try { keys = Object.getOwnPropertyNames(e); } catch (x) {}
      for (const k of keys) add(get(e, k));
    }
  }
  // Flux's own registry: getAll() on a store's class or its parents.
  let registered = false;
  for (const s of [...byName.values()]) {
    let k = get(s, "constructor");
    for (let i = 0; i < 4 && k && !registered; i++, k = Object.getPrototypeOf(k)) {
      if (!fn(k, "getAll")) continue;
      try {
        const list = k.getAll();
        if (Array.isArray(list)) { registered = true; for (const x of list) add(x); }
      } catch (x) {}
    }
    if (registered) break;
  }
  // An object that answers a name nobody would give with a function answers every name: a catch-all whose functions
  // do nothing (the TV, 2026-11-19: about a hundred matched every action's shape ahead of the real one, so every
  // action did nothing). Never picked by shape.
  const catchAll = (o) => fn(o, "xsdNoSuchName" + Date.now().toString(36));
  const fits = (o, all) => typeof o === "object" && all.every((n) => fn(o, n)) && !catchAll(o);
  const shape = (all) => [...byName.values(), ...candidates].find((o) => fits(o, all)) || null;
  const shapeBest = (all, prefer) => {
    const all_ = [...byName.values(), ...candidates].filter((o) => fits(o, all));
    return all_.find((o) => prefer.some((n) => fn(o, n))) || all_[0] || null;
  };
  const pick = (names, methods) => {
    for (const n of names) if (byName.has(n)) return byName.get(n);
    return shape(methods);
  };
  // Our key -> the store, by Flux name then by shape.
  const S = {
    channels: pick(["ChannelStore"], ["getChannel", "hasChannel"]),
    privateSort: pick(["PrivateChannelSortStore", "ChannelListStore"], ["getPrivateChannelIds"]) || shape(["getSortedPrivateChannels"]),
    users: pick(["UserStore"], ["getUser", "getCurrentUser"]),
    readState: pick(["ReadStateStore"], ["getUnreadCount", "getMentionCount"]),
    messages: pick(["MessageStore"], ["getMessages", "getMessage"]),
    calls: pick(["CallStore"], ["getCall"]),
    // Actions, not stores: found by shape on the exports (#201, docs/discord-web.md, "Acting"). Each is optional: the ask
    // it serves fails plainly without it, and the reading goes on.
    older: shape(["fetchMessages"]),
    reply: shape(["createPendingReply"]),
    replyClear: shape(["deletePendingReply"]),
    pending: pick(["PendingReplyStore"], ["getPendingReply"]),
    react: shape(["addReaction", "removeReaction"]),
    // `ack` alone is on more than one object: the one that also has its siblings is the channel read-state action.
    ack: shapeBest(["ack"], ["bulkAck", "ackCategory", "localAck", "ackChannel"]),
    forget: shape(["deleteMessage", "sendMessage"]) || shape(["deleteMessage"]),
    // Voice (#297; docs/discord-web.md, "Voice"). Required for voice: voice, media, guilds, voiceActions (js/voice.js
    // decides); the rest is optional.
    guilds: pick(["GuildStore"], ["getGuilds"]),
    guildChannels: pick(["GuildChannelStore"], ["getChannels", "getDefaultChannel"]),
    voice: pick(["VoiceStateStore"], ["getVoiceStatesForChannel"]),
    speaking: pick(["SpeakingStore"], ["isSpeaking"]),
    media: pick(["MediaEngineStore"], ["isSelfMute", "isSelfDeaf"]),
    selected: pick(["SelectedChannelStore"], ["getVoiceChannelId"]),
    rtc: pick(["RTCConnectionStore"], ["isConnected", "getState"]),
    members: pick(["GuildMemberStore"], ["getNick"]),
    guildOrder: pick(["SortedGuildStore"], ["getFlattenedGuildIds"]),
    permissions: pick(["PermissionStore"], ["can", "computePermissions"]),
    voiceActions: shape(["selectVoiceChannel"]),
    mediaActions: shape(["toggleSelfMute", "toggleSelfDeaf"]),
    callActions: shape(["ring", "stopRinging"]) || shape(["stopRinging"]),
  };
  out.S = S;
  // The copies line (TV, 2026-11-19: every action called by shape did nothing, every store read worked): how many
  // runtimes and modules, and for each action how many objects have its shape, in which runtimes, which was picked, and
  // whether the picked function's own source looks like a forwarder (shorter than 120 characters). Numbers and our
  // keys only.
  try {
    const sizes = reqs.map((r) => { try { return r.c ? Object.keys(r.c).length : 0; } catch (e) { return 0; } });
    const rt = (o) => (o && runtimeOf.has(o) ? runtimeOf.get(o) : "?");
    const parts = ["runtimes " + reqs.length + " (" + sizes.join("/") + ")", "stores in " + rt(S.channels) + "/" + rt(S.messages) + "/" + rt(S.voice)];
    const acts = { voiceActions: "selectVoiceChannel", mediaActions: "toggleSelfMute", ack: "ack", react: "addReaction", reply: "createPendingReply", callActions: "stopRinging", older: "fetchMessages" };
    for (const [k, m] of Object.entries(acts)) {
      const all = [...candidates].filter((o) => fits(o, [m]));
      const len = (() => { try { return String(S[k] && S[k][m]).length; } catch (e) { return -1; } })();
      parts.push(k + " " + all.length + " (catch-alls passed over: " + [...candidates].filter((o) => typeof o === "object" && catchAll(o)).length + ") in [" + [...new Set(all.map(rt))].join(",") + "] picked " + rt(S[k]) + " src " + len);
    }
    out.copies = parts.join("; ");
  } catch (e) { out.copies = "page error"; }
  // Discord's method names we call: [name, required]. A key is required when its first group is; `either` lists
  // alternatives of which one must be there.
  const wants = {
    channels: { need: ["getChannel"], opt: [] },
    privateSort: { either: ["getPrivateChannelIds", "getSortedPrivateChannels"], opt: [] },
    users: { need: ["getUser", "getCurrentUser"], opt: [] },
    readState: { need: ["getUnreadCount"], opt: ["getMentionCount", "hasUnread", "getOldestUnreadMessageId", "ackMessageId"] },
    messages: { need: ["getMessages"], opt: ["getMessage", "getLastMessage"] },
    calls: { either: ["getCall", "getCalls"], opt: [], optional: true },
    older: { need: ["fetchMessages"], opt: [], optional: true },
    reply: { need: ["createPendingReply"], opt: ["deletePendingReply"], optional: true },
    react: { need: ["addReaction", "removeReaction"], opt: [], optional: true },
    ack: { need: ["ack"], opt: [], optional: true },
    forget: { need: ["deleteMessage"], opt: [], optional: true },
    // Voice keys never make `missing` (the DM reading goes on without them); js/voice.js says what voice lacks.
    guilds: { need: ["getGuilds"], opt: ["getGuild"], optional: true },
    guildChannels: { need: ["getChannels"], opt: [], optional: true },
    voice: { need: ["getVoiceStatesForChannel"], opt: ["getVoiceStateForUser"], optional: true },
    speaking: { need: ["isSpeaking"], opt: [], optional: true },
    media: { need: ["isSelfMute", "isSelfDeaf"], opt: [], optional: true },
    selected: { need: ["getVoiceChannelId"], opt: [], optional: true },
    rtc: { either: ["isConnected", "getState"], opt: [], optional: true },
    members: { need: ["getNick"], opt: [], optional: true },
    guildOrder: { need: ["getFlattenedGuildIds"], opt: [], optional: true },
    permissions: { need: ["can"], opt: [], optional: true },
    voiceActions: { need: ["selectVoiceChannel"], opt: [], optional: true },
    mediaActions: { need: ["toggleSelfMute", "toggleSelfDeaf"], opt: [], optional: true },
    callActions: { need: ["stopRinging"], opt: ["ring", "call"], optional: true },
  };
  for (const [k, w] of Object.entries(wants)) {
    const s = S[k];
    const methods = [...(w.need || []), ...(w.either || []), ...w.opt];
    out.found[k] = s ? [nameOf(s) || "(by shape)", ...methods.filter((n) => fn(s, n))] : [];
    if (w.optional && !s) continue;
    if (!s) { out.missing.push(k); continue; }
    for (const n of w.need || []) if (!fn(s, n)) out.missing.push(k + "." + n);
    if (w.either && !w.either.some((n) => fn(s, n))) out.missing.push(k + "." + w.either[0]);
  }
  return out;
};
