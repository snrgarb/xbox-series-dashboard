// A made-up Discord, in the shape of the real web client's in-page world (#198), for tests only: a chunk global
// `webpackChunkdiscord_app` whose runtimes answer a probe chunk with a `require` holding fake Flux stores (made-up names
// and shapes of the same kind: ChannelStore, PrivateChannelSortStore, UserStore, ReadStateStore, MessageStore,
// CallStore, and an action with `fetchMessages`), driven through a `world` the test or the stand-in page changes.
// Nothing here is a saved copy of a real page. Used by crates/discord_web/tests (node, in a vm) and by
// `standin serve-discord-web`'s app page (in a browser).
//
// installFakeDiscord(window, opts) -> world. opts: { seed: true, hide: [store names left out], noMethod: ["Store.method"],
// markReadOnOpen: true }.
function installFakeDiscord(window, opts) {
  opts = Object.assign({ seed: true, hide: [], noMethod: [], markReadOnOpen: true }, opts || {});
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
  // Stores the page has that this code does not read: only there to make the registry realistic.
  mk("GuildStore", { getGuilds: () => ({}), getGuild: () => undefined });
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

  // ---- webpack: two runtimes share the global, each answers a probe chunk with its own require.
  const mod = (exports) => ({ exports });
  const big = { c: {}, m: {} };
  const small = { c: { 1: mod({ nothing: 1 }) }, m: {} };
  let n = 0;
  for (const s of Object.values(world.stores)) {
    if (opts.hide.includes(s.constructor.displayName)) continue;
    big.c[++n] = mod({ Z: s });
    Flux.registry.push(s);
  }
  big.c[++n] = mod({ Ay: actions });
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
  world.signOut = (gone) => { world.signedOut = gone !== false; emit("UserStore"); };
  world.apply = (cmd) => {
    switch (cmd.do) {
      case "say": world.say(cmd.channel, cmd.author || world.channels[cmd.channel].recipients[0], cmd.text); break;
      case "unread": world.setUnread(cmd.channel, cmd.count); break;
      case "ring": world.ring(cmd.channel, !!cmd.on); break;
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
  }
  return world;
}
if (typeof module !== "undefined") module.exports = { installFakeDiscord };
