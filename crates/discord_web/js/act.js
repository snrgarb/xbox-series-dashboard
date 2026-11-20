// Acting through Discord's own page (#201; docs/discord-web.md, "Acting"): the last file of the page script, in the same
// function as stores.js and direct.js (their names are in scope; a second injection returned in direct.js before this).
// The rule: an action goes by id. The route first, then the page's own control by TRUSTED input from the host (DevTools'
// `Input.*`: this file focuses the message box and says so, the host types and presses Enter); one of Discord's own
// functions only where the page has no control that can be reached without hovering (a reply, a reaction, marking
// read). It never calls Discord's web interface itself and holds nothing that identifies the session.
//
// The host asks, one at a time, through window.__xsdDirect:
//   send({seq, channel, text, reply})   route, wait for the messages and the box, set or clear the reply, focus the box,
//                                       say {k:"act", stage:"typing"}; the host types; then watch the store: "sending"
//                                       (with the optimistic message's id, again if it changes), "sent" (with the real
//                                       id), or "failed" / "captcha"
//   react({seq, channel, message, emoji, on})   Discord's own reaction action; "done" when the store shows it
//   ack({seq, channel})                 Discord's own read action; "done" when the unread count is 0
//   forget(channel, id)                 drops Discord's own failed message of ours from its store, locally
//   cancel()                            the host gave up waiting
// Every answer is {k:"act", seq, stage, ...} with `why` one of a fixed set of words on a failure. What was typed is
// never posted back. The text of a message is looked at only to tell which message is ours, never kept.
const D = { cur: null, reads: 0 };
const nap = (ms) => new Promise((r) => setTimeout(r, ms));
const told = (o) => post(Object.assign({ k: "act" }, o));
// Discord loads some of its actions only once the chat is first used (the TV, 2026-11-19: reactions, replies and ack
// were not in the cache at sign-in). An action asked for and not yet found is searched for again, at most once a second;
// what is found then is kept.
let refoundAt = 0;
const actionOf = (k) => {
  if (!A.S) return null;
  if (A.S[k] || Date.now() - refoundAt < 1000) return A.S[k] || null;
  refoundAt = Date.now();
  try {
    const f = xsdFindStores();
    for (const [key, v] of Object.entries(f.S)) if (v && !A.S[key]) A.S[key] = v;
    if (f.copies) A.copies = f.copies;
    for (const [key, v] of Object.entries(f.found)) if (v.length && !(A.found[key] || []).length) A.found[key] = v;
    sendHealth();
  } catch (e) {}
  return A.S[k] || null;
};
const isId = (v) => typeof v === "string" && /^\d+$/.test(v);
const fnOf = (o, n) => { try { return !!o && typeof o[n] === "function"; } catch (e) { return false; } };

const messagesOf = (id) => {
  const coll = call("messages", "getMessages", id);
  try { return coll ? list(typeof coll.toArray === "function" ? coll.toArray() : coll._array).filter(Boolean) : []; } catch (e) { return []; }
};
const messagesReady = (id) => { const c = call("messages", "getMessages", id); return !!c && c.ready !== false; };
const messageOf = (channel, id) => messagesOf(channel).find((m) => str(m.id) === id) || null;
const ownMessage = (m) => !!A.me && !!m.author && str(m.author.id) === A.me;
const unreadOf = (id) => num(call("readState", "getUnreadCount", id));

// The message box: the spike found it on the real page as a contenteditable textbox in the chat form.
const BOX = '[role="textbox"][contenteditable="true"]';
const boxEl = () => {
  try { return document.querySelector("form " + BOX) || document.querySelector(BOX); } catch (e) { return null; }
};
const boxFocused = (el) => {
  try {
    const a = document.activeElement;
    return !!a && (a === el || (typeof el.contains === "function" && el.contains(a)));
  } catch (e) { return false; }
};
const boxEmpty = (el) => { try { return str(el.textContent).replace(/[​﻿]/g, "").trim().length === 0; } catch (e) { return true; } };
// A check Discord asks for shows as a frame of a captcha service in the page.
const captchaUp = () => {
  try { return !!document.querySelector('iframe[src*="captcha" i], iframe[src*="hcaptcha" i], iframe[src*="recaptcha" i]'); } catch (e) { return false; }
};
// What Discord's own dialog says, if one is up (a refusal's reason, as the page shows it).
const dialogText = () => {
  try {
    const d = document.querySelector('[role="dialog"], [role="alertdialog"]');
    return d ? str(d.textContent).replace(/\s+/g, " ").trim().slice(0, 300) : "";
  } catch (e) { return ""; }
};

const until = async (my, test, cap) => {
  const end = Date.now() + cap;
  for (;;) {
    let ok = false;
    try { ok = !!test(); } catch (e) {}
    if (ok) return true;
    if (my.dead || Date.now() >= end) return false;
    await nap(O.actPoll);
  }
};
const finish = (my, o) => {
  if (my.dead) return;
  my.dead = true;
  if (D.cur === my) D.cur = null;
  told(Object.assign({ seq: my.seq }, o));
};
const fail = (my, why, more) => finish(my, Object.assign({ stage: "failed", why }, more || {}));
// One action at a time: false when one is running (the host never asks two).
const begin = (op, body) => {
  if (!op || D.cur || !Number.isInteger(op.seq)) return false;
  const my = { seq: op.seq, dead: false };
  D.cur = my;
  Promise.resolve().then(() => body(my)).catch(() => fail(my, "page error"));
  return true;
};
const pushNow = () => { try { if (A.timer) { clearTimeout(A.timer); A.timer = null; } flush(); } catch (e) {} };

// Discord's reaction action takes its arguments by position in the builds seen so far, and a single object in a newer
// one: the function's own `length` decides (1 is the object form).
const callReaction = (a, on, channel, message, emoji) => {
  const f = on ? a.addReaction : a.removeReaction;
  const e = { id: null, name: emoji, animated: false };
  return typeof f.length === "number" && f.length === 1 ? f.call(a, { channelId: channel, messageId: message, emoji: e }) : f.call(a, channel, message, e);
};
const iReacted = (m, emoji) => !!m && list(m.reactions).some((r) => r && r.emoji && !r.emoji.id && r.emoji.name === emoji && !!r.me);

api.send = (op) => begin(op, async (my) => {
  const channel = op.channel, text = typeof op.text === "string" ? op.text : "";
  if (!isId(channel) || !text.trim() || (op.reply && !isId(op.reply))) return fail(my, "page error");
  const path = "/channels/@me/" + channel;
  if (location.pathname !== path) route(path);
  const ready = await until(my, () => messagesReady(channel) && boxEl(), O.readyCap);
  if (my.dead) return;
  const box = boxEl();
  A.composer = !!box;
  schedule();
  if (!ready || !box) return fail(my, "composer not found");
  const d = A.S.dispatcher;
  if (op.reply) {
    const a = actionOf("reply");
    const ch = call("channels", "getChannel", channel), m = messageOf(channel, op.reply);
    if (!ch || !m) return fail(my, "reply target not found");
    // Discord's action by name, else what it does: the pending reply dispatched.
    try {
      if (a && fnOf(a, "createPendingReply")) a.createPendingReply({ channel: ch, message: m, shouldMention: true });
      else if (d && fnOf(d, "dispatch")) d.dispatch({ type: "CREATE_PENDING_REPLY", channel: ch, message: m, shouldMention: true, showMentionToggle: true });
      else return fail(my, "reply action not found");
    } catch (e) { return fail(my, "reply action failed"); }
  } else {
    // A reply the Dashboard did not ask for (set in the page earlier) is cleared before a plain send.
    try {
      const store = A.S.pending;
      const set = !store || !fnOf(store, "getPendingReply") || store.getPendingReply(channel);
      if (set && actionOf("replyClear") && fnOf(A.S.replyClear, "deletePendingReply")) A.S.replyClear.deletePendingReply(channel);
      else if (set && store && d && fnOf(d, "dispatch")) d.dispatch({ type: "DELETE_PENDING_REPLY", channelId: channel });
    } catch (e) {}
  }
  try { box.focus(); } catch (e) {}
  if (!boxFocused(box)) return fail(my, "composer not focused");
  const before = new Set(messagesOf(channel).map((m) => str(m.id)));
  const frame = captchaUp();
  const draft = !boxEmpty(box);
  told({ seq: my.seq, stage: "typing", draft });
  // Ours: new since the box was focused, our own, and the words typed. The first is Discord's optimistic message
  // (SENDING), which the store replaces by the real one (SENT) under another id.
  const mine = () => messagesOf(channel).filter((m) => !before.has(str(m.id)) && ownMessage(m) && str(m.content).trim() === text.trim());
  let tied = "";
  const end = Date.now() + O.actCap;
  while (!my.dead && Date.now() < end) {
    await nap(O.actPoll);
    if (my.dead) return;
    const found = mine();
    const m = found.find((x) => stateOf(x) === "SENT") || found[0];
    if (m) {
      const state = stateOf(m), id = str(m.id);
      if (state === "SEND_FAILED") { pushNow(); return fail(my, "send refused", { mid: id, reason: dialogText() }); }
      if (state === "SENT") { pushNow(); return finish(my, { stage: "sent", mid: id }); }
      if (id !== tied) { tied = id; told({ seq: my.seq, stage: "sending", mid: id }); }
    } else if (!frame && captchaUp()) {
      return finish(my, { stage: "captcha" });
    }
  }
});

api.react = (op) => begin(op, async (my) => {
  const { channel, message, emoji, on } = op;
  if (!isId(channel) || !isId(message) || typeof emoji !== "string" || !emoji || emoji.length > 32 || typeof on !== "boolean") return fail(my, "page error");
  const a = actionOf("react");
  // Removing without a found removing function goes through Discord's own HTTP utility (it signs its own requests).
  const viaHttp = !on && !(a && fnOf(a, "removeReaction")) && !!actionOf("http") && fnOf(A.S.http, "del");
  if (!viaHttp && (!a || !fnOf(a, on ? "addReaction" : "removeReaction"))) return fail(my, on ? "react action not found" : "unreact action not found");
  if (!messageOf(channel, message)) return fail(my, "message not loaded");
  const shown = () => iReacted(messageOf(channel, message), emoji) === on;
  if (shown()) return finish(my, { stage: "done" });
  let rejected = false, status = 0;
  try {
    const sent = viaHttp
      ? A.S.http.del({ url: "/channels/" + channel + "/messages/" + message + "/reactions/" + encodeURIComponent(emoji) + "/@me", oldFormErrors: true })
      : callReaction(a, on, channel, message, emoji);
    // What Discord answered a refused request with: its HTTP status, a number only.
    Promise.resolve(sent).catch((e) => { rejected = true; try { status = Number(e && e.status) || 0; } catch (x) {} });
  } catch (e) { return fail(my, "react action failed"); }
  const ok = await until(my, () => shown() || rejected, O.doneCap);
  if (my.dead) return;
  if (shown()) return finish(my, { stage: "done" });
  fail(my, ok && rejected ? "react action failed" + (viaHttp ? " (http" + (status ? " " + status : "") + ")" : "") : "not shown");
});

api.ack = (op) => begin(op, async (my) => {
  const channel = op.channel;
  if (!isId(channel)) return fail(my, "page error");
  if (unreadOf(channel) === 0) return finish(my, { stage: "done" });
  const read = () => unreadOf(channel) === 0;
  // Discord's own shortcut first, Escape in the conversation (on the TV Discord's own ack function did nothing,
  // 2026-11-19): the conversation's page, its message box focused, the host's trusted Escape.
  let way = "no box";
  const here = () => { try { return location.pathname.endsWith("/" + channel); } catch (e) { return false; } };
  // Another conversation open in the Dashboard: the page goes back to it after.
  const back = here() ? "" : location.pathname;
  if (back) route("/channels/@me/" + channel);
  let box = null;
  await until(my, () => here() && (box = boxEl()), O.buttonCap);
  if (my.dead) return;
  if (box) {
    try { box.focus(); } catch (e) {}
    way = boxFocused(box) ? "escape" : "box not focused";
    if (way === "escape") {
      told({ seq: my.seq, stage: "press" });
      await until(my, read, O.signCap);
      if (my.dead) return;
    }
  }
  if (back) route(back);
  if (read()) return finish(my, { stage: "done" });
  // Then Discord's action by name, else what it does, CHANNEL_ACK dispatched; briefly: a failure here holds the
  // one-at-a-time queue (a call waited behind it).
  const a = actionOf("ack"), d = A.S.dispatcher;
  let rejected = false;
  try {
    if (a && fnOf(a, "ack")) Promise.resolve(a.ack(channel)).catch(() => { rejected = true; });
    else if (d && fnOf(d, "dispatch")) {
      const c = call("channels", "getChannel", channel);
      d.dispatch({ type: "CHANNEL_ACK", channelId: channel, messageId: c ? str(c.lastMessageId || c.last_message_id) : undefined, immediate: true, force: true, context: "APP" });
    } else return fail(my, "ack action not found (way " + way + ")");
  } catch (e) { return fail(my, "ack action failed"); }
  await until(my, () => read() || rejected, O.ackCap);
  if (my.dead) return;
  if (read()) return finish(my, { stage: "done" });
  fail(my, rejected ? "ack action failed" : "not shown (way " + way + ", action)");
});

// A failed message of ours dropped from Discord's own store, locally: the store's `deleteMessage(channel, id, true)`
// removes what was never sent and asks Discord for nothing. False unless it is ours and failed.
api.forget = (channel, id) => {
  if (!isId(channel) || !isId(id)) return false;
  const a = actionOf("forget");
  const m = messageOf(channel, id);
  if (!a || !fnOf(a, "deleteMessage") || !m || !ownMessage(m) || stateOf(m) !== "SEND_FAILED") return false;
  try { a.deleteMessage(channel, id, true); } catch (e) { return false; }
  schedule();
  return true;
};

api.cancel = () => {
  if (D.cur) { D.cur.dead = true; D.cur = null; }
  return true;
};

// A conversation opened by the host is expected to be marked read by the page itself (it believes it is visible and
// focused). Said once per open that had unread messages: whether the count went to 0 within a few seconds. If it did
// not, the user opening it in the Guide has read it: the page's own read action is called once.
const opened = api.open;
api.open = (id) => {
  const ok = opened(id);
  if (!ok) return ok;
  const mine = ++D.reads;
  const was = A.anchor ? A.anchor.unread : 0;
  if (was > 0) {
    const my = { dead: false };
    const stale = () => mine !== D.reads || A.open !== id;
    (async () => {
      const read = await until(my, () => stale() || unreadOf(id) === 0, O.readCap);
      if (stale()) return;
      let acked = false;
      if (!read || unreadOf(id) !== 0) {
        const a = actionOf("ack");
        const d = A.S && A.S.dispatcher;
        if (a && fnOf(a, "ack")) { try { a.ack(id); acked = true; } catch (e) {} }
        else if (d && fnOf(d, "dispatch")) {
          const c = call("channels", "getChannel", id);
          try { d.dispatch({ type: "CHANNEL_ACK", channelId: id, messageId: c ? str(c.lastMessageId || c.last_message_id) : undefined, immediate: true, force: true, context: "APP" }); acked = true; } catch (e) {}
        }
        post({ k: "opened", read: false, acked });
      } else post({ k: "opened", read: true, acked });
    })().catch(() => {});
  }
  return ok;
};
