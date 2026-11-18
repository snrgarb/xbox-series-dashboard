// The index's way A (#292): Discord's own in-page stores, found READ-ONLY through its webpack chunk global, kept on
// window.__xsdStores. A probe chunk is pushed once to get webpack's require; its module cache is searched for objects
// of the right shape. Only the methods named below are ever called, and only to read guilds, channels, private
// channels, voice states and users' display names. Nothing credential-like is read: a unit test asserts this file and
// the scripts that use it hold none of the words for one. Defines xsdStores(): { found: {...}, why }.
const xsdStores = () => {
  if (window.__xsdStores) return window.__xsdStores;
  const out = { found: {}, why: "" };
  const chunk = window.webpackChunkdiscord_app;
  if (!chunk || typeof chunk.push !== "function") {
    out.why = "no webpackChunkdiscord_app";
    return out;
  }
  let req = null;
  try {
    chunk.push([[Symbol("xsd")], {}, (r) => { req = r; }]);
    chunk.pop();
  } catch (e) {
    out.why = "the probe chunk was refused: " + e.name;
    return out;
  }
  if (!req || !req.c) {
    out.why = "no module cache";
    return out;
  }
  const objects = [];
  for (const m of Object.values(req.c)) {
    const e = m && m.exports;
    if (!e || (typeof e !== "object" && typeof e !== "function")) continue;
    objects.push(e);
    for (const k of ["default", "Z", "ZP", "A", "Ay"]) {
      try { if (e[k] && typeof e[k] === "object") objects.push(e[k]); } catch (x) {}
    }
    try {
      for (const v of Object.values(e)) if (v && typeof v === "object") objects.push(v);
    } catch (x) {}
  }
  const fn = (o, n) => { try { return typeof o[n] === "function"; } catch (x) { return false; } };
  const find = (all) => objects.find((o) => all.every((n) => fn(o, n))) || null;
  const S = {
    guilds: find(["getGuilds", "getGuild"]),
    channels: find(["getChannel", "getMutableGuildChannelsForGuild"]) || find(["getChannel", "hasChannel"]),
    guildChannels: find(["getChannels", "getDefaultChannel"]),
    privateSort: find(["getPrivateChannelIds"]) || find(["getSortedPrivateChannels"]),
    voice: find(["getVoiceStatesForChannel"]),
    users: find(["getUser", "getCurrentUser"]),
    voiceActions: find(["selectVoiceChannel"]),
    speaking: find(["isSpeaking"]),
    media: find(["isSelfMute", "isSelfDeaf"]),
  };
  out.S = S;
  // The method names each store was found by, for the log (names only: how brittle this is).
  const names = {
    guilds: ["getGuilds", "getGuild"],
    channels: ["getChannel", "getMutableGuildChannelsForGuild", "hasChannel", "getSortedPrivateChannels"],
    guildChannels: ["getChannels", "getDefaultChannel"],
    privateSort: ["getPrivateChannelIds", "getSortedPrivateChannels"],
    voice: ["getVoiceStatesForChannel", "getAllVoiceStates"],
    users: ["getUser", "getCurrentUser"],
    voiceActions: ["selectVoiceChannel"],
    speaking: ["isSpeaking", "isAnyoneElseSpeaking"],
    media: ["isSelfMute", "isSelfDeaf", "isMute", "isDeaf"],
  };
  for (const [k, list] of Object.entries(names)) out.found[k] = S[k] ? list.filter((n) => fn(S[k], n)) : [];
  out.modules = Object.keys(req.c).length;
  Object.defineProperty(window, "__xsdStores", { value: out, enumerable: false, configurable: true });
  return out;
};
