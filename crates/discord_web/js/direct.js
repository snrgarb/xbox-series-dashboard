// Discord's Direct Messages, read from its in-page stores (#198; docs/discord-web.md, "Reading") and PUSHED to the host:
// installed with AddScriptToExecuteOnDocumentCreated, after stores.js (the host wraps both in one function). It waits
// for webpack to settle, finds the stores (retrying a bounded number of times, then slowly), subscribes to each
// store's change listener, and posts JSON with window.chrome.webview.postMessage, at most one snapshot a coalescing
// window (250 ms, trailing). Four kinds, each sent only when it differs from the last sent:
//   {k:"health", ...}  which stores and methods were found (our keys, Discord's method names)
//   {k:"dms", dms:[...]}  the DM list in Discord's order
//   {k:"conv", ...}  the conversation the host opened
//   {k:"rings", rings:[...]}  DM calls ringing the user
//   {k:"reset"}  the user is gone (signed out in the page)
// Reading is read-only: only store getters are called, plus the page's own route (history) and Discord's own
// older-messages action by the host's ask. Acting (send, react, mark read; act.js, the next file of the same function)
// goes by id: the route, the page's own message box by trusted input from the host, and Discord's own actions only
// where the page has no control to press. It reads no secret, stores nothing and makes no request of its own.
// Idempotent: a second injection finds window.__xsdDirect and returns.
if (window.__xsdDirect || window.top !== window) return;
const O = Object.assign(
  { poll: 500, settle: 2, cap: 15000, coalesce: 250, voiceWindow: 150, joinCap: 20000, buttonCap: 5000, signCap: 6000, tries: 5, gap: 2000, slow: 30000, actPoll: 40, readyCap: 8000, actCap: 20000, readCap: 3000, doneCap: 10000 },
  window.__xsdDirectOptions || {}
);
const A = { S: null, me: "", open: null, anchor: null, older: false, timer: null, sent: {}, errors: new Set(), dm: new Map(), tries: 0, health: "", composer: false, voiceMissing: [], onInstall: [] };
const post = (o) => { try { window.chrome.webview.postMessage(o); } catch (e) {} };
const str = (v) => (v === undefined || v === null ? "" : String(v));
// The methods whose failure to answer is a break (the same ones stores.js requires).
const REQUIRED = { channels: ["getChannel"], privateSort: ["getPrivateChannelIds", "getSortedPrivateChannels"], users: ["getUser", "getCurrentUser"], readState: ["getUnreadCount"], messages: ["getMessages"] };
const call = (key, name, ...args) => {
  const s = A.S && A.S[key];
  try {
    if (s && typeof s[name] === "function") return s[name](...args);
  } catch (e) {
    if ((REQUIRED[key] || []).includes(name)) A.errors.add(key + "." + name);
  }
  return undefined;
};
const has = (key, name) => { try { return !!(A.S && A.S[key] && typeof A.S[key][name] === "function"); } catch (e) { return false; } };
// A snowflake's time: (id >> 22) + Discord's epoch.
const flake = (id) => { try { return Number((BigInt(id) >> 22n) + 1420070400000n); } catch (e) { return 0; } };
const tsOf = (m) => {
  try {
    const t = m.timestamp;
    if (typeof t === "number") return t;
    if (t && typeof t.valueOf === "function") { const v = t.valueOf(); if (typeof v === "number" && v > 0) return v; }
    if (typeof t === "string") { const v = Date.parse(t); if (v > 0) return v; }
  } catch (e) {}
  return flake(m.id);
};
const nameOf = (u) => (u ? str(u.globalName || u.global_name || u.username) : "");
const userOf = (r) => (r && typeof r === "object" ? r : call("users", "getUser", r));
const pic = (u) => (u && u.id && u.avatar ? "avatars/" + u.id + "/" + u.avatar + ".png?size=128" : "");
const num = (v) => (typeof v === "number" && isFinite(v) ? v : 0);

// Names for the mentions in a text: users by id, channels as "#id".
const namesIn = (text) => {
  const names = {};
  const t = str(text);
  for (const m of t.matchAll(/<@!?(\d+)>/g)) names[m[1]] = nameOf(userOf(m[1]));
  for (const m of t.matchAll(/<#(\d+)>/g)) { const c = call("channels", "getChannel", m[1]); names["#" + m[1]] = c ? str(c.name) : ""; }
  return names;
};

const lastOf = (c) => {
  const lid = str(c.lastMessageId || c.last_message_id);
  let m = lid ? call("messages", "getMessage", c.id, lid) : undefined;
  if (!m) m = call("messages", "getLastMessage", c.id);
  const at = m ? tsOf(m) : lid ? flake(lid) : 0;
  if (!at) return null;
  if (!m) return { at };
  const a = m.author || {};
  return { at, text: str(m.content), who: nameOf(userOf(a.id) || a), names: namesIn(m.content), type: Number(m.type) || 0 };
};

const privateIds = () => {
  let ids = call("privateSort", "getPrivateChannelIds");
  if (!Array.isArray(ids)) {
    const sorted = call("privateSort", "getSortedPrivateChannels");
    ids = Array.isArray(sorted) ? sorted.map((c) => c && (c.channelId || c.id)).filter(Boolean) : null;
  }
  return Array.isArray(ids) ? ids.map(String) : null;
};

const dmOf = (id) => {
  const c = call("channels", "getChannel", id);
  if (!c) return null;
  const rec = (c.recipients || c.rawRecipients || []).map(userOf);
  const group = Number(c.type) === 3 || rec.length > 1;
  const names = rec.map(nameOf).filter(Boolean);
  return {
    id: String(id),
    name: str(c.name) || names.join(", "),
    group,
    members: rec.length + 1,
    pic: group ? (c.icon ? "channel-icons/" + c.id + "/" + c.icon + ".png?size=128" : "") : pic(rec[0]),
    unread: num(call("readState", "getUnreadCount", id)),
    mentions: num(call("readState", "getMentionCount", id)),
    last: lastOf(c),
  };
};

const replyOf = (m) => {
  const ref = m.messageReference || m.message_reference;
  if (!ref || (ref.type !== undefined && ref.type !== null && Number(ref.type) !== 0)) return null;
  let rm = m.referencedMessage;
  if (rm === null || (rm && (rm.state === "DELETED" || rm.state === 2))) return { s: "deleted" };
  if (rm && rm.message) rm = rm.message;
  if (!rm || typeof rm !== "object" || (rm.content === undefined && !rm.author)) {
    rm = call("messages", "getMessage", str(ref.channel_id || ref.channelId || m.channel_id), str(ref.message_id || ref.messageId));
  }
  if (!rm) return { s: "none" };
  const a = rm.author || {};
  return { s: "to", who: nameOf(userOf(a.id) || a), tx: str(rm.content), names: namesIn(rm.content) };
};

const list = (v) => (Array.isArray(v) ? v : []);
// Discord's message state, as one of three words.
const stateOf = (m) => { const s = str(m && m.state); return s === "SENDING" || s === "SEND_FAILED" ? s : "SENT"; };
const msgOf = (m) => {
  const a = m.author || {};
  const u = userOf(a.id) || a;
  return {
    id: str(m.id),
    author: { id: str(a.id), name: nameOf(u), pic: pic(u) },
    own: !!A.me && str(a.id) === A.me,
    at: tsOf(m),
    edited: !!m.editedTimestamp,
    type: Number(m.type) || 0,
    text: str(m.content),
    names: namesIn(m.content),
    state: stateOf(m),
    reply: replyOf(m),
    mentioned: list(m.mentions).map((x) => nameOf(userOf(x))).filter(Boolean),
    files: list(m.attachments).map((x) => ({ ct: str(x.content_type || x.contentType), name: str(x.filename), voice: ((Number(x.flags) || 0) & 8192) !== 0 || (x.waveform !== undefined && x.waveform !== null) })),
    stickers: list(m.stickerItems || m.sticker_items).map((x) => str(x.name)),
    embeds: list(m.embeds).map((x) => ({ type: str(x.type), title: str(x.rawTitle || x.title), site: str(x.provider && x.provider.name) })),
    poll: m.poll ? str(m.poll.question && m.poll.question.text) : null,
    reactions: list(m.reactions).map((x) => ({ id: str(x.emoji && x.emoji.id), name: str(x.emoji && x.emoji.name), count: num(x.count), me: !!x.me })),
  };
};

// The first message from `id` on (or after it, when exclusive): the unread line's place.
const firstFrom = (arr, id, inclusive) => {
  try {
    const at = BigInt(id);
    for (const m of arr) { const v = BigInt(m.id); if (inclusive ? v >= at : v > at) return str(m.id); }
  } catch (e) {}
  return "";
};

const convOf = () => {
  const id = A.open;
  if (!id) return null;
  const coll = call("messages", "getMessages", id);
  let arr = null;
  try { arr = coll ? (typeof coll.toArray === "function" ? coll.toArray() : coll._array) : null; } catch (e) { arr = coll && coll._array; }
  const loaded = !!arr && coll.ready !== false;
  const msgs = loaded ? list(arr).filter(Boolean) : [];
  let from = "";
  const an = A.anchor;
  if (loaded && an) {
    if (an.oldest) from = firstFrom(msgs, an.oldest, true);
    else if (an.unread > 0 && an.ack) from = firstFrom(msgs, an.ack, false);
  }
  return { k: "conv", id, loaded, more: loaded ? coll.hasMoreBefore !== false : true, older: A.older, unread_from: from, msgs: msgs.map(msgOf) };
};

const ringsOf = (dms) => {
  if (!A.S.calls || !A.me) return [];
  const out = [];
  const seen = new Set();
  const take = (c) => {
    if (!c) return;
    const ch = str(c.channelId || c.channel_id);
    if (!ch || seen.has(ch) || !list(c.ringing).map(str).includes(A.me)) return;
    seen.add(ch);
    const d = dms.find((x) => x.id === ch);
    out.push({ channel: ch, caller: d ? d.name : "", pic: d ? d.pic : "" });
  };
  if (has("calls", "getCall")) for (const d of dms) take(call("calls", "getCall", d.id));
  else {
    const all = call("calls", "getCalls");
    for (const c of Array.isArray(all) ? all : all && typeof all === "object" ? Object.values(all) : []) take(c);
  }
  return out;
};

const sendIfChanged = (key, obj) => {
  const j = JSON.stringify(obj);
  if (A.sent[key] === j) return;
  A.sent[key] = j;
  post(obj);
};

const sendHealth = (extra) => {
  const missing = [...A.missing, ...[...A.errors].filter((e) => !A.missing.includes(e))];
  // `composer` is the message box as last looked for by a send: yes or no, never a reason for a break.
  const found = Object.assign({}, A.found, { composer: A.composer ? ["textbox"] : [] });
  sendIfChanged("health", Object.assign({ k: "health", ok: missing.length === 0, found, missing, voice_missing: A.voiceMissing, older: !!(A.S && A.S.older), tries: A.tries }, extra || {}));
};

const flush = () => {
  A.timer = null;
  if (!A.S) return;
  try {
    A.errors.clear();
    const me = call("users", "getCurrentUser");
    const meId = me && me.id ? str(me.id) : "";
    if (!meId) {
      if (A.me) { A.me = ""; A.sent = {}; post({ k: "reset" }); }
      sendHealth();
      return;
    }
    A.me = meId;
    const ids = privateIds();
    if (ids) {
      const dms = ids.map(dmOf).filter(Boolean);
      sendIfChanged("dms", { k: "dms", dms });
      sendIfChanged("rings", { k: "rings", rings: ringsOf(dms) });
    }
    const conv = convOf();
    if (conv) sendIfChanged("conv", conv);
    sendHealth();
  } catch (e) {
    A.errors.add("read");
    sendHealth();
  }
};
const schedule = () => { if (!A.timer) A.timer = setTimeout(flush, O.coalesce); };

const route = (path) => {
  try {
    history.pushState(history.state, "", path);
    window.dispatchEvent(new PopStateEvent("popstate", { state: history.state }));
  } catch (e) {}
};

const api = {
  // The host asked to open a conversation: its unread line is fixed first, before the page marks it read.
  open(id) {
    id = str(id);
    if (!/^\d+$/.test(id)) return false;
    A.open = id;
    A.older = false;
    A.anchor = {
      oldest: str(call("readState", "getOldestUnreadMessageId", id)),
      ack: str(call("readState", "ackMessageId", id)),
      unread: num(call("readState", "getUnreadCount", id)),
    };
    delete A.sent.conv;
    route("/channels/@me/" + id);
    schedule();
    return true;
  },
  close() {
    A.open = null;
    A.anchor = null;
    A.older = false;
    delete A.sent.conv;
    route("/channels/@me");
    return true;
  },
  // Older messages by Discord's own action (the page has no button for it). False if it is not there.
  older() {
    const a = A.S && A.S.older;
    if (!A.open || !a || A.older) return false;
    const coll = call("messages", "getMessages", A.open);
    let arr = [];
    try { arr = coll ? (typeof coll.toArray === "function" ? coll.toArray() : coll._array) : []; } catch (e) {}
    const first = list(arr).find(Boolean);
    if (!first) return false;
    A.older = true;
    const done = () => { A.older = false; schedule(); };
    try { Promise.resolve(a.fetchMessages({ channelId: A.open, before: str(first.id), limit: 50 })).then(done, done); } catch (e) { done(); }
    schedule();
    return true;
  },
  // Sends everything again (the host lost it, or a new session began).
  resend() { A.sent = {}; schedule(); if (A.onResend) A.onResend(); return true; },
};
Object.defineProperty(window, "__xsdDirect", { value: api, enumerable: false, configurable: true });

const install = (found) => {
  A.S = found.S;
  A.found = found.found;
  A.missing = found.missing;
  for (const k of ["channels", "privateSort", "users", "readState", "messages", "calls"]) {
    const s = A.S[k];
    try { if (s && typeof s.addChangeListener === "function") s.addChangeListener(schedule); } catch (e) {}
  }
  for (const f of A.onInstall) { try { f(); } catch (e) {} }
  flush();
};

const onLogin = () => /^\/(login|register)/.test(location.pathname);

// Wait for webpack to settle (the module cache stops growing, capped), then search, retrying a bounded number of times
// before saying so, and slowly after that: the page may still be loading, or become the app later.
const search = () => {
  A.tries++;
  const found = xsdFindStores();
  const broken = !found.S.channels || found.missing.length > 0;
  if (!broken) { install(found); return; }
  A.found = found.found;
  A.missing = found.missing.length ? found.missing : [found.why || "stores"];
  if (A.tries < O.tries) { setTimeout(search, O.gap); return; }
  if (!onLogin()) { A.S = null; sendHealth(); }
  setTimeout(search, O.slow);
};
let last = -1, stable = 0, waited = 0;
const tick = () => {
  waited += O.poll;
  const size = window.webpackChunkdiscord_app ? xsdCacheSize() : -1;
  stable = size === last && size > 0 ? stable + 1 : 0;
  last = size;
  if (stable >= O.settle || waited >= O.cap) search();
  else setTimeout(tick, O.poll);
};
setTimeout(tick, O.poll);
