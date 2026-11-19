// Discord voice through the web session (#297; docs/discord-web.md, "Voice"): the last file of the page script, in the
// same function as stores.js, direct.js and act.js (their names are in scope).
//
// READING, from Discord's in-page stores only, pushed like the rest, at most once a coalescing window (150 ms: the
// speaking store changes many times a second):
//   {k:"voice", me, channel, title, guild:{id,name,icon}|null, dm, people:[{id,name,avatar,muted,deafened,me}],
//    speaking:[ids], mute, deaf, rtc:bool|null, in:bool}   the user's Call (channel "" out of one) and own mute/deafen
//   {k:"guilds", ordered, servers:[{id,name,icon}]}        the user's servers, in the user's own order if a store gives it
//   {k:"server", id, channels:[{id,name,type,users:[{id,avatar}]}]}   the voice and stage channels of the server the
//                                                                      host is browsing (`browse(id)`), with who is in each
// A person's `avatar` is the hash only ("" for the default one); the host builds the CDN path.
//
// ACTING goes by id, through the host's paced queue (one at a time, with act.js).
//   join({seq, channel, ring})  The way the spike proved on the TV first: the channel's own page opened by route
//                               (`/channels/<server>/<channel>`, a DM's `/channels/@me/<id>`), the page's own Join
//                               Voice (a server's) or Start Voice Call (a DM's, which also rings) button focused and
//                               pressed by the host's trusted Enter (`{k:"act", stage:"press"}`). Only if there is no
//                               such button, or pressing it shows no sign within 6 s, Discord's own
//                               `selectVoiceChannel(channel)` (on the TV it alone never connected, 2026-11-19). Done
//                               when the user's selected voice channel (or own voice state) is that channel and RTC
//                               does not say otherwise, within 20 s, else the page leaves and fails, naming in fixed
//                               words which way was tried and what was seen. With `ring` and joined by the action, once
//                               in and alone, Discord's own call action rings the DM.
// The buttons' labels are English (as the spike's were): another language finds none and falls to the action.
//   leave({seq})                selectVoiceChannel(null); done when the user has no voice channel
//   mute({seq}) / deafen({seq}) toggleSelfMute() / toggleSelfDeaf(); done when MediaEngineStore shows the flip
//   decline({seq, channel})     stopRinging(channel); done when CallStore no longer rings the user there
//   browse(id|null)             the server whose voice channels are pushed as they change
// Nothing here reads a credential or the page's text, and nothing makes a request.
const V = { timer: null, last: 0, errors: new Set(), missing: [], browse: null, attached: new Set(), refind: 0 };
const REQUIRED_VOICE = { channels: ["getChannel"], voice: ["getVoiceStatesForChannel"], media: ["isSelfMute", "isSelfDeaf"], guilds: ["getGuilds"], voiceActions: ["selectVoiceChannel"] };
const CONNECT = typeof BigInt === "function" ? BigInt(1) << BigInt(20) : 0;

const vcall = (key, name, ...args) => {
  const s = A.S && A.S[key];
  try {
    if (s && typeof s[name] === "function") return s[name](...args);
  } catch (e) {
    if ((REQUIRED_VOICE[key] || []).includes(name)) V.errors.add(key + "." + name);
  }
  return undefined;
};
const vhas = (key, name) => has(key, name);
const optPic = (u) => (u && u.avatar ? str(u.avatar) : "");

// What voice lacks: a required key not found, a required method missing, or one that threw. Empty is ready.
const voiceMissing = () => {
  const out = [];
  for (const [key, methods] of Object.entries(REQUIRED_VOICE)) {
    if (!A.S || !A.S[key]) { out.push(key); continue; }
    for (const m of methods) if (!vhas(key, m)) out.push(key + "." + m);
  }
  for (const e of V.errors) if (!out.includes(e)) out.push(e);
  return out;
};

const statesOf = (channel) => {
  const v = channel ? vcall("voice", "getVoiceStatesForChannel", channel) : undefined;
  return v ? (Array.isArray(v) ? v : Object.values(v)).filter(Boolean) : [];
};
const uidOf = (s) => str(s.userId || s.user_id);
// The channel the user is in: the selected voice channel, else the channel their own voice state names.
const myChannel = () => {
  let id = vcall("selected", "getVoiceChannelId");
  if (!id && A.me && vhas("voice", "getVoiceStateForUser")) {
    const s = vcall("voice", "getVoiceStateForUser", A.me);
    id = s && (s.channelId || s.channel_id);
  }
  return id ? str(id) : "";
};
const inChannel = (channel) => statesOf(channel).some((s) => uidOf(s) === A.me);
const rtcConnected = () => {
  if (!A.S || !A.S.rtc) return null;
  if (vhas("rtc", "isConnected")) { const r = vcall("rtc", "isConnected"); return typeof r === "boolean" ? r : null; }
  const st = vcall("rtc", "getState");
  return typeof st === "string" ? st === "RTC_CONNECTED" : null;
};
const guildMap = () => { const g = vcall("guilds", "getGuilds"); return g && typeof g === "object" ? g : {}; };
const guildOf = (id) => { const g = guildMap()[id]; return g ? { id: str(g.id), name: str(g.name), icon: str(g.icon) } : null; };
const mediaFlag = (name) => !!vcall("media", name);

const titleOf = (ch, guildId) => {
  if (!ch) return "";
  if (guildId) return str(ch.name);
  if (str(ch.name)) return str(ch.name);
  return list(ch.recipients || ch.rawRecipients).map((r) => nameOf(userOf(r))).filter(Boolean).join(", ");
};

const voiceMsg = () => {
  const meUser = call("users", "getCurrentUser");
  const channel = myChannel();
  const ch = channel ? call("channels", "getChannel", channel) : null;
  const guildId = ch ? str(ch.guild_id || ch.guildId) : "";
  const mute = mediaFlag("isSelfMute"), deaf = mediaFlag("isSelfDeaf");
  const people = [];
  let joined = false;
  for (const s of statesOf(channel)) {
    const id = uidOf(s);
    if (!id) continue;
    const u = userOf(id);
    if (id === A.me) joined = true;
    const nick = guildId && vhas("members", "getNick") ? str(vcall("members", "getNick", guildId, id)) : "";
    people.push({
      id,
      name: nick || nameOf(u),
      avatar: optPic(u),
      muted: !!(s.selfMute || s.mute),
      deafened: !!(s.selfDeaf || s.deaf),
      me: id === A.me,
    });
  }
  // Selected but not yet shown in the channel (connecting): the user is still shown in it.
  if (channel && !joined) people.unshift({ id: A.me, name: nameOf(meUser), avatar: optPic(meUser), muted: mute, deafened: deaf, me: true });
  const speaking = A.S.speaking ? people.filter((p) => vcall("speaking", "isSpeaking", p.id)).map((p) => p.id) : [];
  return {
    k: "voice",
    me: nameOf(meUser),
    channel,
    title: titleOf(ch, guildId),
    guild: guildId ? guildOf(guildId) : null,
    dm: !!channel && !guildId,
    people,
    speaking,
    mute,
    deaf,
    rtc: channel ? rtcConnected() : null,
    in: joined,
  };
};

const guildsMsg = () => {
  const map = guildMap();
  const all = Object.values(map).filter(Boolean).map((g) => ({ id: str(g.id), name: str(g.name), icon: str(g.icon) }));
  let order = vhas("guildOrder", "getFlattenedGuildIds") ? vcall("guildOrder", "getFlattenedGuildIds") : null;
  order = Array.isArray(order) ? order.map(str) : [];
  const by = new Map(all.map((g) => [g.id, g]));
  const mine = order.filter((id) => by.has(id)).map((id) => by.get(id));
  const rest = all.filter((g) => !order.includes(g.id)).sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  return { k: "guilds", ordered: mine.length > 0, servers: mine.concat(rest) };
};

// Whether the user may connect to a channel: true or false where the permission store answers, null where it cannot be
// told. Discord's documented CONNECT bit (1 << 20) is the only one asked.
const canConnect = (ch) => {
  const p = A.S && A.S.permissions;
  if (!p || typeof p.can !== "function" || !CONNECT) return null;
  try { const r = p.can(CONNECT, ch); return typeof r === "boolean" ? r : null; } catch (e) { return null; }
};

const serverMsg = (id) => {
  let chans = [];
  const mutable = vcall("channels", "getMutableGuildChannelsForGuild", id);
  if (mutable && typeof mutable === "object") chans = Object.values(mutable);
  else if (A.S.guildChannels) {
    const groups = vcall("guildChannels", "getChannels", id) || {};
    for (const arr of Object.values(groups)) if (Array.isArray(arr)) for (const x of arr) if (x && x.channel) chans.push(x.channel);
  }
  const seen = new Set();
  chans = chans.filter((c) => c && !seen.has(c.id) && seen.add(c.id) && (Number(c.type) === 2 || Number(c.type) === 13));
  chans.sort((a, b) => num(a.position) - num(b.position) || str(a.name).localeCompare(str(b.name)));
  // Only those the user may connect to, if the permission store says; when it says no to every one it is not to be
  // trusted, and all are listed.
  const can = chans.map(canConnect);
  const all = can.every((r) => r === false);
  const shown = chans.filter((c, i) => all || can[i] !== false);
  return {
    k: "server",
    id,
    channels: shown.map((c) => ({
      id: str(c.id),
      name: str(c.name),
      type: Number(c.type),
      users: statesOf(c.id).map((s) => ({ id: uidOf(s), avatar: optPic(userOf(uidOf(s))) })).filter((u) => u.id),
    })),
  };
};

const flushVoice = () => {
  V.timer = null;
  V.last = Date.now();
  if (!A.S || !A.me) return;
  try {
    V.errors.clear();
    const was = A.voiceMissing.join(",");
    A.voiceMissing = voiceMissing();
    if (A.voiceMissing.length) { if (A.voiceMissing.join(",") !== was) sendHealth(); return; }
    sendIfChanged("voice", voiceMsg());
    sendIfChanged("guilds", guildsMsg());
    if (V.browse) sendIfChanged("server", serverMsg(V.browse));
    if (V.errors.size) { A.voiceMissing = voiceMissing(); sendHealth(); }
    else if (was) sendHealth();
  } catch (e) {
    V.errors.add("voice.read");
    A.voiceMissing = voiceMissing();
    sendHealth();
  }
};
// At most one push per window: the first change waits out what is left of it.
const scheduleVoice = () => {
  if (V.timer) return;
  const wait = Math.max(0, O.voiceWindow - (Date.now() - V.last));
  V.timer = setTimeout(flushVoice, wait);
};

const VOICE_LISTENED = ["channels", "users", "guilds", "guildChannels", "voice", "speaking", "media", "selected", "rtc", "members", "guildOrder", "permissions"];
const attach = () => {
  for (const k of VOICE_LISTENED) {
    const s = A.S && A.S[k];
    if (!s || V.attached.has(s)) continue;
    V.attached.add(s);
    try { if (typeof s.addChangeListener === "function") s.addChangeListener(scheduleVoice); } catch (e) {}
  }
};

// A store the first search did not find may load later: the search is run again, a bounded number of times, then
// slowly, until voice has all it requires.
const refind = () => {
  if (!A.S) return;
  if (!voiceMissing().length) return;
  V.refind++;
  try {
    const f = xsdFindStores();
    for (const k of Object.keys(f.S)) {
      if (!A.S[k] && f.S[k]) { A.S[k] = f.S[k]; A.found[k] = f.found[k]; }
    }
  } catch (e) {}
  attach();
  A.voiceMissing = voiceMissing();
  sendHealth();
  scheduleVoice();
  if (A.voiceMissing.length) setTimeout(refind, V.refind < O.tries ? O.gap : O.slow);
};

A.onInstall.push(() => {
  attach();
  A.voiceMissing = voiceMissing();
  scheduleVoice();
  if (A.voiceMissing.length) setTimeout(refind, O.gap);
});
A.onResend = () => scheduleVoice();

api.browse = (id) => {
  if (id === null) { V.browse = null; return true; }
  id = str(id);
  if (!/^\d+$/.test(id)) return false;
  V.browse = id;
  delete A.sent.server;
  scheduleVoice();
  return true;
};

// ---- acting
const rungMe = (channel) => {
  const c = has("calls", "getCall") ? call("calls", "getCall", channel) : null;
  return !!c && list(c.ringing).map(str).includes(A.me);
};
const needVoice = (my) => {
  if (!A.S || !A.S.voiceActions || !fnOf(A.S.voiceActions, "selectVoiceChannel")) { fail(my, "voice action not found"); return false; }
  return true;
};

// The page's own button for joining `channel`: a server voice channel's Join Voice, a DM's Start Voice Call.
const joinButton = (dm) => {
  const all = (q) => { try { return [...document.querySelectorAll(q)]; } catch (e) { return []; } };
  const label = (b) => (b.getAttribute("aria-label") || "").trim();
  const text = (b) => (b.textContent || "").trim();
  const buttons = all('button, [role="button"]');
  if (dm) return buttons.find((b) => /^start (a )?voice call$/i.test(label(b))) || null;
  return buttons.find((b) => /^join voice$/i.test(text(b)) || /^join voice$/i.test(label(b))) || null;
};

api.join = (op) => begin(op, async (my) => {
  const channel = op.channel;
  if (!isId(channel)) return fail(my, "page error");
  if (!needVoice(my)) return;
  // The sign: the user's selected voice channel, or their own voice state, is this channel.
  const sign = () => myChannel() === channel || inChannel(channel);
  const connected = () => sign() && rtcConnected() !== false;
  let way = "none";
  let rejected = false;
  if (!connected()) {
    const ch = call("channels", "getChannel", channel);
    const guild = ch ? str(ch.guild_id || ch.guildId) : "";
    const dm = !guild;
    // Way 1: the channel's page and its own button, pressed by the host.
    route(dm ? "/channels/@me/" + channel : "/channels/" + guild + "/" + channel);
    let button = null;
    await until(my, () => (button = joinButton(dm)), O.buttonCap);
    if (my.dead) return;
    if (button) {
      try { button.scrollIntoView({ block: "center" }); button.focus(); } catch (e) {}
      if (document.activeElement === button) {
        way = "button";
        told({ seq: my.seq, stage: "press" });
        await until(my, sign, O.signCap);
        if (my.dead) return;
      } else way = "button not focused";
    } else way = "no button";
    // Way 2: Discord's own action, only if the button showed no sign.
    if (!sign()) {
      way = way === "button" ? "button, then action" : way + ", action";
      try { Promise.resolve(A.S.voiceActions.selectVoiceChannel(channel)).catch(() => { rejected = true; }); } catch (e) { return fail(my, "join action failed"); }
    }
    await until(my, () => connected() || rejected, O.joinCap);
    if (my.dead) return;
    if (!connected()) {
      const rtc = rtcConnected();
      const seen = `way ${way}; selected ${myChannel() === channel ? "yes" : myChannel() ? "another" : "none"}, ` +
        `voice state ${inChannel(channel) ? "yes" : "no"}, rtc ${rtc === null ? "unknown" : rtc ? "connected" : "not connected"}`;
      // Not joined in time: the page leaves, so nothing half-joined is left behind.
      try { A.S.voiceActions.selectVoiceChannel(null); } catch (e) {}
      return fail(my, (rejected ? "join action failed" : "not connected") + " (" + seen + ")");
    }
  }
  // A DM's Start Voice Call rings by itself.
  if (way === "button") op = Object.assign({}, op, { ring: false });
  let ring = "";
  if (op.ring) {
    const others = statesOf(channel).filter((s) => uidOf(s) !== A.me).length;
    const a = A.S.callActions;
    const f = a && (fnOf(a, "ring") ? "ring" : fnOf(a, "call") ? "call" : "");
    if (others > 0) ring = "";
    else if (!f) ring = "no";
    else { try { a[f](channel); ring = "yes"; } catch (e) { ring = "no"; } }
  }
  finish(my, { stage: "done", ring });
});

api.leave = (op) => begin(op, async (my) => {
  if (!needVoice(my)) return;
  const gone = () => !myChannel();
  if (!gone()) {
    try { Promise.resolve(A.S.voiceActions.selectVoiceChannel(null)).catch(() => {}); } catch (e) { return fail(my, "join action failed"); }
    await until(my, gone, O.doneCap);
    if (my.dead) return;
  }
  if (!gone()) return fail(my, "not shown");
  finish(my, { stage: "done" });
});

const toggle = (op, name, flag) => begin(op, async (my) => {
  const a = A.S && A.S.mediaActions;
  if (!a || !fnOf(a, name) || !A.S.media) return fail(my, "media action not found");
  const before = mediaFlag(flag);
  try { Promise.resolve(a[name]()).catch(() => {}); } catch (e) { return fail(my, "media action failed"); }
  await until(my, () => mediaFlag(flag) !== before, O.doneCap);
  if (my.dead) return;
  if (mediaFlag(flag) === before) return fail(my, "not shown");
  scheduleVoice();
  finish(my, { stage: "done", on: mediaFlag(flag) });
});
api.mute = (op) => toggle(op, "toggleSelfMute", "isSelfMute");
api.deafen = (op) => toggle(op, "toggleSelfDeaf", "isSelfDeaf");

api.decline = (op) => begin(op, async (my) => {
  const channel = op.channel;
  if (!isId(channel)) return fail(my, "page error");
  const a = A.S && A.S.callActions;
  if (!a || !fnOf(a, "stopRinging")) return fail(my, "call action not found");
  if (!rungMe(channel)) return finish(my, { stage: "done" });
  try { Promise.resolve(a.stopRinging(channel)).catch(() => {}); } catch (e) { return fail(my, "call action failed"); }
  await until(my, () => !rungMe(channel), O.doneCap);
  if (my.dead) return;
  if (rungMe(channel)) return fail(my, "not shown");
  finish(my, { stage: "done" });
});
