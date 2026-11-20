// A made-up Discord, in the shape of the real web client's in-page world (#198), for tests only: a chunk global
// `webpackChunkdiscord_app` whose runtimes answer a probe chunk with a `require` holding fake Flux stores (made-up names
// and shapes of the same kind: ChannelStore, PrivateChannelSortStore, UserStore, ReadStateStore, MessageStore,
// CallStore, and an action with `fetchMessages`), driven through a `world` the test or the stand-in page changes.
// Nothing here is a saved copy of a real page. Used by crates/discord_web/tests (node, in a vm) and by
// `standin serve-discord-web`'s app page (in a browser).
//
// installFakeDiscord(window, opts) -> world. opts: { seed: true, hide: [store names left out], noMethod: ["Store.method"],
// markReadOnOpen: true, noActions: ["reply", "react", "ack", "forget"] (action objects left out), reactForm:
// "positional" | "object" | "code" | "code-add-only" (how the reaction action takes its arguments) }.
//
// Acting (#201): the page "changes as the real one does". A message box exists while a conversation is the page's route:
// `world.type(text)` and `world.enter()` play the host's trusted input in node (the stand-in page has a real box,
// which a real Enter sends). Sending adds Discord's own optimistic message (state SENDING, an id of its own) to the
// MessageStore and, `world.slowMs` later (300), makes it SENT under a real id, or SEND_FAILED when `world.sendFails`;
// with `world.captcha` nothing is sent and a captcha frame appears. Fake `createPendingReply`/`deletePendingReply`,
// `addReaction`/`removeReaction`, `ack`, `deleteMessage` are exported by the same shapes as the real ones. What the fake
// received is in `world.received` (and `world.onReceived(entry)`, for the stand-in to report).
//
// Voice (#297): fake GuildStore (3 made-up servers), ChannelStore's `getMutableGuildChannelsForGuild` and GuildChannelStore
// with voice channels (some occupied), VoiceStateStore, SpeakingStore, MediaEngineStore, SelectedChannelStore,
// RTCConnectionStore, GuildMemberStore, SortedGuildStore, PermissionStore, and the actions `selectVoiceChannel` (connects
// `world.connectMs` later, 300), `toggleSelfMute`/`toggleSelfDeaf`, `ring`/`stopRinging`, exported by the same shapes.
// `opts.noActions` also takes "voice", "media", "call". What the fake received of voice is in `world.received` as
// {kind: "join"|"leave"|"mute"|"deafen"|"ring"|"stop-ringing", channel?}. Controls: world.voiceSpeak(user, on),
// world.voiceJoin(channel, user), world.voiceLeave(user) (the signed-in user when none is named).
function installFakeDiscord(window, opts) {
  opts = Object.assign({ seed: true, hide: [], noMethod: [], markReadOnOpen: true, noActions: [], reactForm: "positional" }, opts || {});
  const ME = "300000000000000099";
  const T0 = Date.UTC(2026, 10, 18, 20, 0, 0);
  let seq = 0;
  const flake = (ms) => String((BigInt(ms - 1420070400000) << 22n) | BigInt(++seq));

  class Flux {
    constructor() { this._l = new Set(); }
    static getAll() { return Flux.registry; }
    getName() { return this.constructor.displayName; }
    addChangeListener(f) { this._l.add(f); }
    removeChangeListener(f) { this._l.delete(f); }
    emitChange() { for (const f of [...this._l]) f(); }
  }
  Flux.registry = [];

  const world = {
    me: ME,
    users: {},
    channels: {},
    order: [],
    pool: {},
    loaded: {},
    read: {},
    calls: {},
    selected: null,
    fetchLog: [],
    stores: {},
    notes: [],
    pending: {},
    received: [],
    slowMs: 300,
    sendFails: false,
    failReason: "",
    captcha: false,
    captchaShown: false,
    dialogText: "",
    composer: { text: "", focused: false },
    onReceived: null,
    onCaptcha: null,
    onDialog: null,
    guilds: {},
    guildOrder: [],
    voiceStates: {},
    speaking: new Set(),
    selfMute: false,
    selfDeaf: false,
    voiceChannel: null,
    rtcConnected: false,
    nicks: {},
    noConnect: [],
    connectMs: 300,
  };
  const emit = (...names) => { for (const n of names) if (world.stores[n]) world.stores[n].emitChange(); };
  const cnt = (id) => world.read[id] || (world.read[id] = { unread: 0, mentions: 0, oldest: null, ack: null });

  // ---- stores
  const mk = (name, methods) => {
    const C = class extends Flux {};
    C.displayName = name;
    const s = new C();
    for (const [k, f] of Object.entries(methods)) if (!opts.noMethod.includes(name + "." + k)) s[k] = f;
    world.stores[name] = s;
    return s;
  };
  mk("UserStore", {
    getUser: (id) => world.users[id],
    getCurrentUser: () => (world.signedOut ? undefined : world.users[ME]),
  });
  mk("ChannelStore", {
    getChannel: (id) => world.channels[id],
    hasChannel: (id) => !!world.channels[id],
    getMutableGuildChannelsForGuild: (gid) => {
      const out = {};
      for (const c of Object.values(world.channels)) if (c.guild_id === gid) out[c.id] = c;
      return out;
    },
  });
  mk("PrivateChannelSortStore", {
    getPrivateChannelIds: () => world.order.slice(),
  });
  mk("ReadStateStore", {
    getUnreadCount: (id) => cnt(id).unread,
    getMentionCount: (id) => cnt(id).mentions,
    hasUnread: (id) => cnt(id).unread > 0,
    getOldestUnreadMessageId: (id) => cnt(id).oldest,
    ackMessageId: (id) => cnt(id).ack,
  });
  const collection = (id) => {
    const l = world.loaded[id];
    return {
      ready: !!l,
      hasMoreBefore: l ? l.more : true,
      loadingMore: false,
      _array: l ? l.arr : [],
      toArray() { return this._array; },
    };
  };
  mk("MessageStore", {
    getMessages: (id) => collection(id),
    getMessage: (id, mid) => ((world.loaded[id] && world.loaded[id].arr.find((m) => m.id === mid)) || undefined),
  });
  mk("CallStore", {
    getCall: (id) => world.calls[id],
    getCalls: () => Object.values(world.calls),
  });
  mk("PendingReplyStore", {
    getPendingReply: (id) => (world.pending[id] ? { channelId: id, message: world.pending[id] } : undefined),
  });
  // Stores the page has that this code does not read: only there to make the registry realistic.
  mk("GuildStore", { getGuilds: () => world.guilds, getGuild: (id) => world.guilds[id] });
  mk("GuildChannelStore", {
    getChannels: (gid) => {
      const vocal = Object.values(world.channels).filter((c) => c.guild_id === gid && (c.type === 2 || c.type === 13));
      return { SELECTABLE: [], VOCAL: vocal.sort((a, b) => a.position - b.position).map((channel) => ({ channel, comparator: channel.position })) };
    },
    getDefaultChannel: () => undefined,
  });
  mk("VoiceStateStore", {
    getVoiceStatesForChannel: (id) => {
      const out = {};
      for (const s of world.voiceStates[id] || []) out[s.userId] = Object.assign({}, s);
      return out;
    },
    getVoiceStateForUser: (uid) => {
      for (const arr of Object.values(world.voiceStates)) { const s = arr.find((x) => x.userId === uid); if (s) return Object.assign({}, s); }
      return undefined;
    },
  });
  mk("SpeakingStore", { isSpeaking: (uid) => world.speaking.has(uid) });
  mk("MediaEngineStore", { isSelfMute: () => world.selfMute, isSelfDeaf: () => world.selfDeaf });
  mk("SelectedChannelStore", { getVoiceChannelId: () => world.voiceChannel });
  mk("RTCConnectionStore", { isConnected: () => world.rtcConnected, getState: () => (world.rtcConnected ? "RTC_CONNECTED" : "RTC_DISCONNECTED") });
  mk("GuildMemberStore", { getNick: (gid, uid) => (world.nicks[gid] && world.nicks[gid][uid]) || null });
  mk("SortedGuildStore", { getFlattenedGuildIds: () => world.guildOrder.slice() });
  mk("PermissionStore", { can: (perm, channel) => { if (typeof perm !== "bigint") throw new Error("bad permission"); return !world.noConnect.includes(channel.id); } });
  const actions = {
    fetchMessages(args) {
      world.fetchLog.push(args);
      const l = world.loaded[args.channelId];
      const pool = world.pool[args.channelId] || [];
      if (l) {
        const at = pool.findIndex((m) => m.id === args.before);
        const older = pool.slice(Math.max(0, at - (args.limit || 50)), Math.max(0, at));
        l.arr = older.concat(l.arr);
        l.more = pool.indexOf(l.arr[0]) > 0;
        emit("MessageStore");
      }
      return Promise.resolve({ ok: true });
    },
  };
  world.actions = actions;
  const note = (entry) => { world.received.push(entry); if (world.onReceived) world.onReceived(entry); };
  const findMessage = (channel, id) => (world.pool[channel] || []).find((m) => m.id === id);
  const replyActions = {
    createPendingReply(arg) {
      if (!arg || !arg.channel || !arg.message) throw new Error("bad reply");
      world.pending[arg.channel.id] = arg.message;
      emit("PendingReplyStore");
    },
    deletePendingReply(channelId) { delete world.pending[channelId]; emit("PendingReplyStore"); },
  };
  const setReaction = (channel, id, emoji, on) => {
    const m = findMessage(channel, id);
    if (!m) return Promise.reject(new Error("unknown message"));
    const name = emoji && emoji.name;
    let list = (m.reactions || []).map((r) => Object.assign({}, r));
    const at = list.findIndex((r) => !r.emoji.id && r.emoji.name === name);
    if (on) {
      if (at < 0) list.push({ emoji: { id: null, name }, count: 1, me: true });
      else if (!list[at].me) { list[at].count++; list[at].me = true; }
    } else if (at >= 0 && list[at].me) {
      list[at].me = false;
      if (--list[at].count <= 0) list.splice(at, 1);
    }
    m.reactions = list;
    note({ kind: "react", channel, message: id, emoji: name, on });
    emit("MessageStore");
    return Promise.resolve();
  };
  // The positional builds take (channel, message, emoji); a newer one takes one object. `length` tells them apart.
  const reactActions = {
    addReaction: opts.reactForm === "object" ? function (a) { return setReaction(a.channelId, a.messageId, a.emoji, true); } : function (c, m, e) { return setReaction(c, m, e, true); },
    removeReaction: opts.reactForm === "object" ? function (a) { return setReaction(a.channelId, a.messageId, a.emoji, false); } : function (c, m, e, l, u) { return setReaction(c, m, e, false); },
  };
  const ackActions = {
    ack(channelId) {
      const r = cnt(channelId);
      r.unread = 0; r.mentions = 0; r.oldest = null;
      const pool = world.pool[channelId] || [];
      r.ack = pool.length ? pool[pool.length - 1].id : null;
      note({ kind: "read", channel: channelId });
      emit("ReadStateStore");
      return Promise.resolve();
    },
    bulkAck() {},
    localAck() {},
  };
  // Another object with an `ack` and nothing else: not the read-state action.
  const decoyAck = { ack() { throw new Error("not the read action"); } };
  const messageActions = {
    sendMessage() {},
    deleteMessage(channelId, id, local) {
      const l = world.loaded[channelId];
      const pool = world.pool[channelId] || [];
      for (const arr of [pool, l && l.arr]) {
        if (!arr) continue;
        const at = arr.findIndex((m) => m.id === id);
        if (at >= 0) arr.splice(at, 1);
      }
      world.deleted = (world.deleted || []).concat([{ channelId, id, local }]);
      emit("MessageStore");
    },
  };

  // ---- voice: Discord's own functions, by the shapes the page script looks for
  const setVoiceState = (channel, userId, flags) => {
    for (const k of Object.keys(world.voiceStates)) {
      world.voiceStates[k] = world.voiceStates[k].filter((s) => s.userId !== userId);
      if (!world.voiceStates[k].length) delete world.voiceStates[k];
    }
    if (channel) (world.voiceStates[channel] = world.voiceStates[channel] || []).push(Object.assign({ userId, channelId: channel, selfMute: false, selfDeaf: false, mute: false, deaf: false }, flags || {}));
  };
  const myState = () => { for (const arr of Object.values(world.voiceStates)) { const s = arr.find((x) => x.userId === ME); if (s) return s; } return null; };
  let connecting = 0;
  const voiceActions = {
    selectVoiceChannel(channelId) {
      const my = ++connecting;
      if (!channelId) {
        note({ kind: "leave" });
        world.voiceChannel = null;
        world.rtcConnected = false;
        setVoiceState(null, ME);
        emit("SelectedChannelStore", "RTCConnectionStore", "VoiceStateStore");
        return Promise.resolve();
      }
      if (!world.channels[channelId]) return Promise.reject(new Error("unknown channel"));
      note({ kind: "join", channel: channelId });
      world.voiceChannel = channelId;
      world.rtcConnected = false;
      emit("SelectedChannelStore", "RTCConnectionStore");
      setTimeout(() => {
        if (my !== connecting) return;
        setVoiceState(channelId, ME, { selfMute: world.selfMute, selfDeaf: world.selfDeaf });
        world.rtcConnected = true;
        // Joining a DM that rings the user answers it: the ring ends.
        if (world.calls[channelId]) delete world.calls[channelId];
        emit("VoiceStateStore", "RTCConnectionStore", "CallStore");
      }, world.connectMs);
      return Promise.resolve();
    },
  };
  const mediaActions = {
    toggleSelfMute() {
      world.selfMute = !world.selfMute;
      const s = myState(); if (s) s.selfMute = world.selfMute;
      note({ kind: "mute" });
      emit("MediaEngineStore", "VoiceStateStore");
    },
    toggleSelfDeaf() {
      world.selfDeaf = !world.selfDeaf;
      const s = myState(); if (s) s.selfDeaf = world.selfDeaf;
      note({ kind: "deafen" });
      emit("MediaEngineStore", "VoiceStateStore");
    },
  };
  const callActions = {
    ring(channelId) { note({ kind: "ring", channel: channelId }); },
    stopRinging(channelId) {
      note({ kind: "stop-ringing", channel: channelId });
      delete world.calls[channelId];
      emit("CallStore");
    },
  };

  // ---- webpack: two runtimes share the global, each answers a probe chunk with its own require.
  const mod = (exports) => ({ exports });
  const big = { c: {}, m: {} };
  const small = { c: { 1: mod({ nothing: 1 }) }, m: {} };
  let n = 0;
  // As on the real discord.com (the TV, 2026-11-19: about a hundred of them, ahead of the real actions): objects that
  // answer every name with a function that does nothing. Searching by shape must pass them over.
  const catchAll = () => new Proxy({}, { get: (_, k) => (typeof k === "string" ? function n() {} : undefined) });
  big.c[++n] = mod({ Q: catchAll() });
  big.c[++n] = mod(catchAll());
  for (const s of Object.values(world.stores)) {
    if (opts.hide.includes(s.constructor.displayName)) continue;
    big.c[++n] = mod({ Z: s });
    Flux.registry.push(s);
  }
  big.c[++n] = mod({ Ay: actions });
  if (!opts.noActions.includes("ack")) big.c[++n] = mod({ Dk: decoyAck });
  if (!opts.noActions.includes("reply")) big.c[++n] = mod({ Rp: replyActions });
  if (!opts.noActions.includes("react") && (opts.reactForm === "code" || opts.reactForm === "code-add-only")) {
    // As on the real discord.com (the TV, 2026-11-19): no object with addReaction, only module functions under
    // minified names whose source names the action type, beside a small helper that names it too.
    const BB = function (c, m, e) { const type = "MESSAGE_REACTION_ADD"; void type; /* the request, then the optimistic update */ return setReaction(c, m, e, true); };
    const Qr = function (c, m, e, l) { const type = "MESSAGE_REACTION_REMOVE"; void type; /* the request, then the optimistic update */ return setReaction(c, m, e, false); };
    const y4 = function (e, t) { return { type: "MESSAGE_REACTION_ADD", e, t }; };
    // "code-add-only": as on the TV (2026-11-20), the removing function is not found by its source.
    big.c[++n] = mod(opts.reactForm === "code" ? { BB, Qr, y4 } : { BB, y4 });
  } else if (!opts.noActions.includes("react")) big.c[++n] = mod({ Rx: reactActions });
  if (!opts.noActions.includes("ack")) big.c[++n] = mod({ Ak: ackActions });
  if (!opts.noActions.includes("forget")) big.c[++n] = mod({ Ms: messageActions });
  if (!opts.noActions.includes("voice")) big.c[++n] = mod({ Vx: voiceActions });
  if (!opts.noActions.includes("media")) big.c[++n] = mod({ Me: mediaActions });
  if (!opts.noActions.includes("call")) big.c[++n] = mod({ Ca: callActions });
  const chunk = [];
  const runtimes = [big, small];
  chunk.push = function (item) {
    if (item && typeof item[2] === "function") for (const r of runtimes) item[2](r);
    return Array.prototype.push.call(this, item);
  };
  window.webpackChunkdiscord_app = chunk;
  world.late = (name) => { big.c[++n] = mod({ Z: world.stores[name] }); };

  // ---- the page's router: a channel route selects it, as Discord's does.
  const select = (id) => {
    world.selected = id;
    if (id && world.channels[id]) {
      const pool = world.pool[id] || [];
      world.loaded[id] = { arr: pool.slice(-50), more: pool.length > 50 };
      if (opts.markReadOnOpen) {
        const r = cnt(id);
        r.unread = 0;
        r.mentions = 0;
        r.oldest = null;
        r.ack = pool.length ? pool[pool.length - 1].id : null;
      }
      emit("MessageStore", "ReadStateStore");
    }
  };
  if (window.addEventListener) {
    window.addEventListener("popstate", () => {
      const m = /^\/channels\/@me\/(\d+)$/.exec(window.location.pathname);
      select(m ? m[1] : null);
    });
  }

  // ---- the message box and sending
  world.type = (text) => { world.composer.text += text; };
  world.clearBox = () => { world.composer.text = ""; };
  world.enter = () => {
    const text = world.composer.text;
    world.composer.text = "";
    world.submit(text);
  };
  // What the box's Enter does, with the box's text.
  world.submit = (text) => {
    const channel = world.selected;
    if (!channel || !world.channels[channel] || !String(text).trim()) return null;
    if (world.captcha) {
      world.captchaShown = true;
      if (world.onCaptcha) world.onCaptcha(true);
      return null;
    }
    const ref = world.pending[channel] || null;
    delete world.pending[channel];
    const u = world.users[ME];
    const id = flake(Date.now());
    const m = {
      id, channel_id: channel, type: ref ? 19 : 0, content: String(text).trim(), timestamp: new Date(), editedTimestamp: null,
      state: "SENDING", author: { id: ME, username: u && u.username, avatar: u && u.avatar },
      attachments: [], embeds: [], stickerItems: [], reactions: [], mentions: [],
    };
    if (ref) { m.messageReference = { message_id: ref.id, channel_id: channel }; m.referencedMessage = ref; }
    world.pool[channel].push(m);
    const l = world.loaded[channel];
    if (l) l.arr = l.arr.concat([m]);
    emit("MessageStore");
    setTimeout(() => {
      if (world.sendFails) {
        m.state = "SEND_FAILED";
        if (world.failReason) { world.dialogText = world.failReason; if (world.onDialog) world.onDialog(world.failReason); }
      } else {
        m.id = flake(Date.now());
        m.state = "SENT";
        world.channels[channel].lastMessageId = m.id;
        note({ kind: "message", channel, text: m.content, reply: ref ? ref.id : null });
      }
      emit("MessageStore", "ChannelStore", "PrivateChannelSortStore");
    }, world.slowMs);
    return m;
  };
  world.setCaptcha = (on) => { world.captcha = !!on; if (!on) { world.captchaShown = false; if (world.onCaptcha) world.onCaptcha(false); } };
  // A document for node, where the page has none: the box exists while a conversation is the route.
  world.fakeDocument = () => {
    const box = {
      isContentEditable: true,
      focus() { world.composer.focused = true; },
      contains: (x) => x === box,
      get textContent() { return world.composer.text; },
    };
    return {
      get activeElement() { return world.composer.focused ? box : null; },
      querySelector(sel) {
        if (sel.includes("textbox")) return world.selected && world.channels[world.selected] ? box : null;
        if (sel.includes("captcha")) return world.captchaShown ? { tagName: "IFRAME" } : null;
        if (sel.includes("dialog")) return world.dialogText ? { textContent: world.dialogText } : null;
        return null;
      },
      querySelectorAll: () => [],
    };
  };

  // ---- what a test or the stand-in page changes
  world.addUser = (id, username, globalName, avatar) => { world.users[id] = { id, username, globalName, avatar: avatar || null }; };
  world.addDm = (id, recipients, extra) => {
    world.channels[id] = Object.assign({ id, type: recipients.length > 1 ? 3 : 1, recipients, lastMessageId: null }, extra || {});
    world.order.push(id);
    world.pool[id] = world.pool[id] || [];
    emit("ChannelStore", "PrivateChannelSortStore");
  };
  // A message record in the shape of the client's own, as far as this code reads it.
  world.say = (channel, author, content, extra) => {
    const ms = T0 + 60000 * (++world.tick || (world.tick = 1));
    const id = flake(ms);
    const u = world.users[author];
    const m = Object.assign(
      {
        id, channel_id: channel, type: 0, content, timestamp: new Date(ms), editedTimestamp: null, state: "SENT",
        author: { id: author, username: u && u.username, avatar: u && u.avatar },
        attachments: [], embeds: [], stickerItems: [], reactions: [], mentions: [],
      },
      extra || {}
    );
    world.pool[channel].push(m);
    const ch = world.channels[channel];
    ch.lastMessageId = id;
    const l = world.loaded[channel];
    if (l) l.arr = l.arr.concat([m]);
    if (author !== ME && world.selected !== channel) {
      const r = cnt(channel);
      if (!r.unread) { r.oldest = id; }
      r.unread++;
    }
    // Moves to the top of the list, as Discord's order does.
    world.order = [channel].concat(world.order.filter((x) => x !== channel));
    emit("ChannelStore", "PrivateChannelSortStore", "MessageStore", "ReadStateStore");
    return m;
  };
  world.setUnread = (channel, count) => {
    const r = cnt(channel);
    r.unread = count;
    if (!count) { r.oldest = null; r.mentions = 0; }
    emit("ReadStateStore");
  };
  world.ring = (channel, on) => {
    if (on) world.calls[channel] = { channelId: channel, ringing: [ME] };
    else delete world.calls[channel];
    emit("CallStore");
  };
  world.voiceSpeak = (user, on) => { if (on) world.speaking.add(user); else world.speaking.delete(user); emit("SpeakingStore"); };
  world.voiceJoin = (channel, user) => {
    if (user === ME) { voiceActions.selectVoiceChannel(channel); return; }
    setVoiceState(channel, user);
    emit("VoiceStateStore");
  };
  world.voiceLeave = (user) => {
    if (!user || user === ME) {
      world.voiceChannel = null; world.rtcConnected = false; setVoiceState(null, ME);
      emit("SelectedChannelStore", "RTCConnectionStore", "VoiceStateStore");
    } else { world.speaking.delete(user); setVoiceState(null, user); emit("VoiceStateStore", "SpeakingStore"); }
  };
  world.addGuild = (id, name, icon, channels) => {
    world.guilds[id] = { id, name, icon: icon || null };
    world.guildOrder.push(id);
    for (const c of channels || []) world.channels[c.id] = Object.assign({ guild_id: id, position: 0, parent_id: null }, c);
    emit("GuildStore", "ChannelStore", "SortedGuildStore");
  };
  world.signOut = (gone) => { world.signedOut = gone !== false; emit("UserStore"); };
  world.apply = (cmd) => {
    switch (cmd.do) {
      case "say": world.say(cmd.channel, cmd.author || world.channels[cmd.channel].recipients[0], cmd.text); break;
      case "unread": world.setUnread(cmd.channel, cmd.count); break;
      case "ring": world.ring(cmd.channel, !!cmd.on); break;
      case "voice-speak": world.voiceSpeak(cmd.user, !!cmd.on); break;
      case "voice-join": world.voiceJoin(cmd.channel, cmd.user); break;
      case "voice-leave": world.voiceLeave(cmd.user); break;
      case "send-fails": world.sendFails = !!cmd.on; world.failReason = cmd.reason || ""; break;
      case "captcha": world.setCaptcha(!!cmd.on); break;
      case "slow": world.slowMs = Math.max(0, Number(cmd.ms) || 0); break;
      default: break;
    }
  };

  if (opts.seed) {
    world.addUser(ME, "me_user", "Me Myself", "hmea");
    world.addUser("300000000000000001", "stand_in_friend", "Stand-in Friend", "ha1");
    world.addUser("300000000000000002", "test_pal", "Test Pal", null);
    world.addUser("300000000000000003", "made_up_mate", "Made-up Mate", "ha3");
    const [a, b, c] = ["300000000000000001", "300000000000000002", "300000000000000003"];
    world.addDm("100000000000000001", [a]);
    world.addDm("100000000000000002", [b]);
    world.addDm("100000000000000003", [c]);
    world.addDm("100000000000000010", [a, b], { name: "", icon: "gi1" });
    world.addDm("100000000000000011", [b, c], { name: "Made-up Group" });
    // A conversation with varied messages (the first DM).
    const ch = "100000000000000001";
    world.say(ch, a, "hello there <@" + ME + "> and welcome");
    world.say(ch, ME, "hi! see https://example.test/page, ok?");
    const hello = world.say(ch, a, "a custom <:wave:123456> emoji and <a:dance:654321> another");
    world.say(ch, a, "look", { attachments: [{ content_type: "image/png", filename: "one.png" }, { content_type: "image/jpeg", filename: "two.jpg" }] });
    world.say(ch, a, "", { attachments: [{ content_type: "video/mp4", filename: "clip.mp4" }] });
    world.say(ch, a, "", { attachments: [{ content_type: "audio/ogg", filename: "voice-message.ogg", flags: 8192, waveform: "AAAA" }] });
    world.say(ch, a, "", { attachments: [{ content_type: "application/pdf", filename: "notes.pdf" }], stickerItems: [{ name: "Wave Sticker" }] });
    world.say(ch, ME, "answering that", { type: 19, messageReference: { message_id: hello.id, channel_id: ch }, referencedMessage: hello, editedTimestamp: new Date(T0 + 99999999) });
    world.say(ch, a, "gone one", { type: 19, messageReference: { message_id: "999", channel_id: ch }, referencedMessage: null });
    world.say(ch, a, "", { embeds: [{ type: "article", rawTitle: "A Page Title", provider: { name: "Example" } }, { type: "link", provider: { name: "Site Only" } }, { type: "gifv" }] });
    world.say(ch, a, "", { poll: { question: { text: "Which game tonight?" } } });
    world.say(ch, a, "reacted", { reactions: [{ emoji: { id: null, name: "\u{1F44D}" }, count: 2, me: true }, { emoji: { id: "777", name: "party" }, count: 1, me: false }] });
    world.say(ch, a, "", { type: 3 });
    world.say(ch, a, "", { type: 6 });
    world.say(ch, b, "", { type: 1, mentions: [c] });
    // Unread: the second DM has two, the group one.
    for (const [id, who, n] of [["100000000000000002", b, 2], ["100000000000000010", a, 1]]) {
      for (let i = 0; i < n; i++) world.say(id, who, "unread message " + (i + 1));
    }
    // The first DM's messages are read.
    world.setUnread(ch, 0);
    world.read[ch].ack = world.pool[ch][world.pool[ch].length - 1].id;
    world.selected = null;
    // Older messages for the group, behind the 50 the store loads.
    for (let i = 0; i < 60; i++) world.say("100000000000000011", c, "old " + i);
    world.setUnread("100000000000000011", 0);
    world.order = ["100000000000000002", "100000000000000010", "100000000000000001", "100000000000000003", "100000000000000011"];
    emit("PrivateChannelSortStore");
    // Voice: three made-up servers (in the user's own order, not by name), voice channels, some occupied.
    world.addGuild("400000000000000001", "Stand-in Server", "gha", [
      { id: "500000000000000001", name: "Lounge", type: 2, position: 0 },
      { id: "500000000000000002", name: "Games", type: 2, position: 1 },
      { id: "500000000000000003", name: "chat", type: 0, position: 0 },
      { id: "500000000000000004", name: "Stage", type: 13, position: 2 },
    ]);
    world.addGuild("400000000000000003", "Made-up Place", "ghc", [{ id: "500000000000000021", name: "Quiet", type: 2, position: 0 }, { id: "500000000000000022", name: "Private Vault", type: 2, position: 1 }]);
    world.addGuild("400000000000000002", "Test Guild", null, [{ id: "500000000000000011", name: "Voice 1", type: 2, position: 0 }]);
    world.noConnect = ["500000000000000022"];
    world.nicks["400000000000000001"] = { [a]: "Lounge Regular" };
    world.voiceStates["500000000000000001"] = [{ userId: a, channelId: "500000000000000001", selfMute: false, selfDeaf: false, mute: false, deaf: false }];
    world.voiceStates["500000000000000011"] = [{ userId: b, channelId: "500000000000000011", selfMute: true, selfDeaf: false, mute: false, deaf: false }];
  }
  return world;
}
if (typeof module !== "undefined") module.exports = { installFakeDiscord };
