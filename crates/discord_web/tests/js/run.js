// Runs the page script (argv[2]: the script's text file, written by tests/scripts.rs from discord_web::direct_script())
// in a vm context against the made-up Discord of crates/standin/js/fake_discord.js (argv[3]), as a page would run it:
// a minimal window, history and chrome.webview.postMessage, no network. Drives store changes and asserts what was
// posted. Exit code 0 when every check holds; each check says ok or FAIL.
"use strict";
const fs = require("fs");
const vm = require("vm");
const assert = require("assert");

const script = fs.readFileSync(process.argv[2], "utf8");
const { installFakeDiscord } = require(process.argv[3]);

const FAST = { poll: 5, settle: 2, cap: 400, coalesce: 20, tries: 3, gap: 10, slow: 40, actPoll: 5, readyCap: 300, actCap: 900, readCap: 150, doneCap: 200 };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function page(opts, options) {
  const posts = [];
  const routes = [];
  const listeners = {};
  const win = {
    location: { pathname: "/channels/@me" },
    history: {
      state: null,
      pushState(state, _t, path) { win.location.pathname = path; routes.push(path); },
    },
    chrome: { webview: { postMessage: (o) => posts.push(JSON.parse(JSON.stringify(o))) } },
    addEventListener(type, f) { (listeners[type] = listeners[type] || []).push(f); },
    dispatchEvent(e) { for (const f of listeners[e.type] || []) f(e); },
    PopStateEvent: class { constructor(type, init) { this.type = type; Object.assign(this, init); } },
    __xsdDirectOptions: Object.assign({}, FAST, options || {}),
    setTimeout, clearTimeout, setInterval, clearInterval, console, Promise, BigInt, JSON, Date, Symbol, Object, Array, Set, Map,
  };
  win.window = win;
  win.top = win;
  // The page script has no network: nothing to call.
  const ctx = vm.createContext(win);
  const world = installFakeDiscord(win, opts);
  world.slowMs = 120;
  win.document = world.fakeDocument();
  const run = () => vm.runInContext(script, ctx);
  return { win, ctx, posts, routes, world, run, last: (k) => [...posts].reverse().find((p) => p.k === k), all: (k) => posts.filter((p) => p.k === k) };
}

let failed = 0;
async function check(name, fn) {
  try {
    await fn();
    console.log("ok " + name);
  } catch (e) {
    failed++;
    console.log("FAIL " + name + ": " + (e && e.stack || e));
  }
}

const ME = "300000000000000099";
// The host's half of a send: waits for the page to say it is typing, then types and presses Enter as trusted input would.
async function sendText(p, op, text) {
  const taken = p.win.__xsdDirect.send(Object.assign({ seq: 1 }, op, { text }));
  if (!taken) return false;
  const seq = op.seq || 1;
  const typed = () => p.posts.find((x) => x.k === "act" && x.stage === "typing" && x.seq === seq);
  for (let i = 0; i < 200 && !typed(); i++) await sleep(5);
  const typing = typed();
  if (typing) { p.world.type(text); p.world.enter(); }
  return typing || null;
}
const acts = (p, seq) => p.posts.filter((x) => x.k === "act" && (seq === undefined || x.seq === seq));
const stages = (p, seq) => acts(p, seq).map((x) => x.stage);
const F1 = "100000000000000001", F2 = "100000000000000002", GROUP = "100000000000000010", NAMED = "100000000000000011";

(async () => {
  await check("the stores are found, the DM list is read in Discord's order", async () => {
    const p = page();
    p.run();
    await sleep(150);
    const health = p.last("health");
    assert.strictEqual(health.ok, true, JSON.stringify(health));
    assert.deepStrictEqual(health.missing, []);
    for (const k of ["channels", "privateSort", "users", "readState", "messages", "calls", "older"]) assert.ok(health.found[k].length > 0, k);
    assert.strictEqual(health.found.channels[0], "ChannelStore");
    assert.strictEqual(health.older, true);
    const dms = p.last("dms").dms;
    assert.deepStrictEqual(dms.map((d) => d.id), [F2, GROUP, F1, "100000000000000003", NAMED]);
    const by = Object.fromEntries(dms.map((d) => [d.id, d]));
    assert.strictEqual(by[F2].name, "Test Pal");
    assert.strictEqual(by[F2].unread, 2);
    assert.strictEqual(by[F2].group, false);
    assert.strictEqual(by[F2].members, 2);
    assert.strictEqual(by[F2].pic, "", "no avatar hash: no picture");
    assert.strictEqual(by[F1].pic, "avatars/300000000000000001/ha1.png?size=128");
    assert.strictEqual(by[GROUP].name, "Stand-in Friend, Test Pal", "a group's members joined");
    assert.strictEqual(by[GROUP].group, true);
    assert.strictEqual(by[GROUP].members, 3);
    assert.strictEqual(by[GROUP].pic, "channel-icons/" + GROUP + "/gi1.png?size=128");
    assert.strictEqual(by[NAMED].name, "Made-up Group");
    assert.ok(by[F2].last.at > 1.7e12, "a snowflake's time");
    assert.strictEqual(by[F2].last.text, undefined, "the store holds no message of a channel nobody opened");
    assert.strictEqual(p.last("rings").rings.length, 0);
  });

  await check("changes are pushed, coalesced and not repeated", async () => {
    const p = page();
    p.run();
    await sleep(150);
    const before = p.all("dms").length;
    for (let i = 0; i < 6; i++) p.world.say("100000000000000003", "300000000000000003", "burst " + i);
    await sleep(120);
    assert.strictEqual(p.all("dms").length, before + 1, "six changes in one window, one snapshot");
    const dms = p.last("dms").dms;
    assert.strictEqual(dms[0].id, "100000000000000003", "it moved to the top");
    assert.strictEqual(dms[0].unread, 6);
    const n = p.posts.length;
    p.world.stores.ReadStateStore.emitChange();
    p.world.stores.UserStore.emitChange();
    await sleep(120);
    assert.strictEqual(p.posts.length, n, "nothing changed: nothing sent");
  });

  await check("opening a conversation routes the page, and its messages are read in every kind", async () => {
    const p = page();
    p.run();
    await sleep(150);
    assert.strictEqual(p.win.__xsdDirect.open(F1), true);
    assert.deepStrictEqual(p.routes, ["/channels/@me/" + F1]);
    await sleep(120);
    const c = p.last("conv");
    assert.strictEqual(c.id, F1);
    assert.strictEqual(c.loaded, true);
    assert.strictEqual(c.more, false);
    assert.strictEqual(c.msgs.length, 15);
    const listed = p.last("dms").dms.find((d) => d.id === F1);
    assert.strictEqual(listed.last.text, "", "an opened conversation's last message is held");
    assert.strictEqual(listed.last.who, "Test Pal");
    const m = c.msgs;
    assert.strictEqual(m[0].author.name, "Stand-in Friend");
    assert.strictEqual(m[0].author.pic, "avatars/300000000000000001/ha1.png?size=128");
    assert.deepStrictEqual(m[0].names, { "300000000000000099": "Me Myself" });
    assert.ok(m[0].text.includes("<@300000000000000099>"));
    assert.strictEqual(m[1].author.id, "300000000000000099");
    assert.ok(m[0].at < m[1].at && m[0].at > 1.7e12);
    assert.strictEqual(m[3].files.length, 2);
    assert.strictEqual(m[3].files[0].ct, "image/png");
    assert.strictEqual(m[4].files[0].ct, "video/mp4");
    assert.strictEqual(m[5].files[0].voice, true);
    assert.strictEqual(m[6].files[0].name, "notes.pdf");
    assert.deepStrictEqual(m[6].stickers, ["Wave Sticker"]);
    assert.strictEqual(m[7].reply.s, "to");
    assert.strictEqual(m[7].reply.who, "Stand-in Friend");
    assert.ok(m[7].reply.tx.startsWith("a custom"));
    assert.strictEqual(m[7].edited, true);
    assert.strictEqual(m[7].type, 19);
    assert.strictEqual(m[8].reply.s, "deleted");
    assert.deepStrictEqual(m[9].embeds.map((e) => [e.type, e.title, e.site]), [["article", "A Page Title", "Example"], ["link", "", "Site Only"], ["gifv", "", ""]]);
    assert.strictEqual(m[10].poll, "Which game tonight?");
    assert.deepStrictEqual(m[11].reactions, [{ id: "", name: "\u{1F44D}", count: 2, me: true }, { id: "777", name: "party", count: 1, me: false }]);
    assert.strictEqual(m[12].type, 3);
    assert.strictEqual(m[13].type, 6);
    assert.strictEqual(m[14].type, 1);
    assert.deepStrictEqual(m[14].mentioned, ["Made-up Mate"]);
    assert.strictEqual(m[0].poll, null);
    assert.strictEqual(m[0].reply, null);
    assert.strictEqual(m[0].state, "SENT");
  });

  await check("the unread line is fixed from the read state at the moment of opening", async () => {
    const p = page();
    p.run();
    await sleep(150);
    const first = p.world.pool[F2][0].id;
    p.win.__xsdDirect.open(F2);
    await sleep(120);
    const c = p.last("conv");
    assert.strictEqual(c.unread_from, first, "the page marked it read on opening; the line is where it was");
    assert.strictEqual(p.last("dms").dms.find((d) => d.id === F2).unread, 0, "and the list follows the page");
    p.win.__xsdDirect.open(F1);
    await sleep(120);
    assert.strictEqual(p.last("conv").unread_from, "", "nothing was unread");
  });

  await check("older messages by Discord's own action, found by shape", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.win.__xsdDirect.open(NAMED);
    await sleep(120);
    let c = p.last("conv");
    assert.strictEqual(c.msgs.length, 50);
    assert.strictEqual(c.more, true);
    const oldest = c.msgs[0].id;
    assert.strictEqual(p.win.__xsdDirect.older(), true);
    await sleep(120);
    assert.strictEqual(JSON.stringify(p.world.fetchLog), JSON.stringify([{ channelId: NAMED, before: oldest, limit: 50 }]));
    c = p.last("conv");
    assert.strictEqual(c.msgs.length, 60);
    assert.strictEqual(c.more, false, "the start of the conversation");
    assert.strictEqual(c.older, false);
  });

  await check("closing forgets the conversation and routes back", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.win.__xsdDirect.open(F1);
    await sleep(80);
    const n = p.all("conv").length;
    p.win.__xsdDirect.close();
    assert.strictEqual(p.routes[p.routes.length - 1], "/channels/@me");
    p.world.say(F1, "300000000000000001", "after");
    await sleep(80);
    assert.strictEqual(p.all("conv").length, n, "no conversation is read while none is open");
    assert.strictEqual(p.win.__xsdDirect.older(), false);
  });

  await check("an id that is not digits is refused, and nothing is routed", async () => {
    const p = page();
    p.run();
    await sleep(80);
    assert.strictEqual(p.win.__xsdDirect.open("1; alert(1)"), false);
    assert.deepStrictEqual(p.routes, []);
  });

  await check("a ring is a call the user is ringing in", async () => {
    const p = page();
    p.run();
    await sleep(120);
    p.world.ring(F1, true);
    await sleep(80);
    assert.deepStrictEqual(p.last("rings").rings, [{ channel: F1, caller: "Stand-in Friend", pic: "avatars/300000000000000001/ha1.png?size=128" }]);
    p.world.ring(F1, false);
    await sleep(80);
    assert.deepStrictEqual(p.last("rings").rings, []);
    // Ringing someone else only is not the user's.
    p.world.calls[F2] = { channelId: F2, ringing: ["300000000000000001"] };
    p.world.stores.CallStore.emitChange();
    await sleep(80);
    assert.deepStrictEqual(p.last("rings").rings, []);
  });

  await check("a store that is missing is PageChanged's health, and the script tries again slowly", async () => {
    const p = page({ hide: ["ReadStateStore"] });
    p.run();
    await sleep(250);
    const h = p.last("health");
    assert.strictEqual(h.ok, false);
    assert.ok(h.missing.includes("readState"), JSON.stringify(h));
    assert.strictEqual(p.all("dms").length, 0, "nothing is read without it");
    p.world.late("ReadStateStore");
    await sleep(250);
    assert.strictEqual(p.last("health").ok, true, "it came back: " + JSON.stringify(p.last("health")));
    assert.strictEqual(p.all("dms").length, 1);
  });

  await check("a method that is missing names our key and Discord's method", async () => {
    const p = page({ noMethod: ["MessageStore.getMessages"] });
    p.run();
    await sleep(250);
    const h = p.last("health");
    assert.strictEqual(h.ok, false);
    assert.deepStrictEqual(h.missing, ["messages.getMessages"]);
  });

  await check("the older-messages action and the call store are optional", async () => {
    const p = page({ hide: ["CallStore"], noMethod: [] });
    p.world.actions.fetchMessages = undefined;
    p.run();
    await sleep(150);
    const h = p.last("health");
    assert.strictEqual(h.ok, true, JSON.stringify(h));
    assert.strictEqual(h.older, false);
    assert.deepStrictEqual(h.found.calls, []);
    p.win.__xsdDirect.open(NAMED);
    await sleep(80);
    assert.strictEqual(p.win.__xsdDirect.older(), false);
    assert.deepStrictEqual(p.last("rings").rings, []);
  });

  await check("a store that throws is health, not a crash", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.world.stores.MessageStore.getMessages = () => { throw new Error("changed"); };
    p.win.__xsdDirect.open(F1);
    await sleep(100);
    const h = p.last("health");
    assert.strictEqual(h.ok, false);
    assert.ok(h.missing.includes("messages.getMessages"));
  });

  await check("injected twice, it reads once", async () => {
    const p = page();
    p.run();
    p.run();
    await sleep(150);
    assert.strictEqual(p.all("dms").length, 1);
    assert.strictEqual(p.all("health").length, 1);
    assert.strictEqual(p.world.stores.ChannelStore._l.size, 1);
  });

  await check("the page signing out resets, and resend sends it all again", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.world.signOut();
    await sleep(80);
    assert.ok(p.last("reset"));
    p.world.signOut(false);
    await sleep(80);
    assert.strictEqual(p.all("dms").length, 2, "read again once the user is back");
    p.win.__xsdDirect.resend();
    await sleep(80);
    assert.strictEqual(p.all("dms").length, 3);
    assert.strictEqual(p.all("health").length, 3, "health is sent again with each fresh start");
  });

  await check("the login route stays quiet when nothing is found", async () => {
    const p = page({ hide: ["ChannelStore"] }, { tries: 2 });
    p.win.location.pathname = "/login";
    p.run();
    await sleep(200);
    assert.strictEqual(p.posts.length, 0);
  });

  await check("a page with no chunk global at all is a miss, not an error", async () => {
    const p = page({}, { cap: 60 });
    delete p.win.webpackChunkdiscord_app;
    p.run();
    await sleep(350);
    const h = p.last("health");
    assert.strictEqual(h.ok, false);
  });

  await check("a send routes the page, focuses the box, and is done only when the store shows it sent", async () => {
    const p = page();
    p.run();
    await sleep(150);
    const typing = await sendText(p, { channel: F1 }, "hello from a test");
    assert.ok(typing, "the page asked for typing: " + JSON.stringify(p.posts.filter((x) => x.k === "act")));
    assert.strictEqual(typing.draft, false);
    assert.deepStrictEqual(p.routes, ["/channels/@me/" + F1]);
    await sleep(300);
    assert.deepStrictEqual(stages(p, 1), ["typing", "sending", "sent"]);
    const [sending, sent] = acts(p).filter((x) => x.stage === "sending" || x.stage === "sent");
    assert.ok(sending.mid && sent.mid && sending.mid !== sent.mid, "the optimistic id, then the real one");
    assert.deepStrictEqual(p.world.received, [{ kind: "message", channel: F1, text: "hello from a test", reply: null }]);
    const h = p.last("health");
    assert.deepStrictEqual(h.found.composer, ["textbox"], "the box was seen");
    assert.strictEqual(h.ok, true);
    for (const k of ["reply", "react", "ack", "forget"]) assert.ok(h.found[k].length > 0, k);
    assert.strictEqual(h.found.ack[0], "(by shape)");
  });

  await check("the conversation shows the message as SENDING, then SENT, with the optimistic id and then the real one", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.win.__xsdDirect.open(F1);
    await sleep(80);
    await sendText(p, { channel: F1 }, "watch me go");
    await sleep(60);
    let c = p.last("conv");
    const out = c.msgs[c.msgs.length - 1];
    assert.strictEqual(out.text, "watch me go");
    assert.strictEqual(out.state, "SENDING");
    assert.strictEqual(out.own, true);
    assert.strictEqual(out.author.id, ME);
    const optimistic = out.id;
    await sleep(250);
    c = p.last("conv");
    const done = c.msgs[c.msgs.length - 1];
    assert.strictEqual(done.state, "SENT");
    assert.notStrictEqual(done.id, optimistic);
    assert.strictEqual(c.msgs.filter((m) => m.text === "watch me go").length, 1, "never both");
    // The conversation was posted before the act said sent: the host never sees the real message after the word.
    let lastConv = -1;
    p.posts.forEach((x, i) => { if (x.k === "conv") lastConv = i; });
    const sentAt = p.posts.findIndex((x) => x.k === "act" && x.stage === "sent");
    assert.ok(sentAt > 0 && lastConv < sentAt, "the fresh conversation comes before 'sent'");
  });

  await check("the same words said before are not taken for the send", async () => {
    const p = page();
    p.run();
    await sleep(150);
    await sendText(p, { channel: F1, seq: 1 }, "twice");
    await sleep(300);
    assert.strictEqual(stages(p, 1).pop(), "sent");
    p.posts.length = 0;
    p.world.slowMs = 200;
    await sendText(p, { channel: F1, seq: 2 }, "twice");
    await sleep(80);
    assert.deepStrictEqual(stages(p, 2), ["typing", "sending"], "the earlier one is not this send's sent");
    await sleep(300);
    assert.strictEqual(stages(p, 2).pop(), "sent");
    assert.strictEqual(p.world.received.length, 2);
  });

  await check("a reply is set by Discord's own action, with its channel and message, and goes as a reply", async () => {
    const p = page();
    p.run();
    await sleep(150);
    const target = p.world.pool[F1][2].id;
    await sendText(p, { channel: F1, reply: target }, "answering you");
    await sleep(300);
    assert.strictEqual(stages(p, 1).pop(), "sent");
    assert.deepStrictEqual(p.world.received, [{ kind: "message", channel: F1, text: "answering you", reply: target }]);
    assert.deepStrictEqual(p.world.pending, {}, "the pending reply was used up");
  });

  await check("a plain send clears a reply the Dashboard did not ask for", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.world.pending[F1] = p.world.pool[F1][0];
    await sendText(p, { channel: F1 }, "no reply here");
    await sleep(300);
    assert.deepStrictEqual(p.world.received, [{ kind: "message", channel: F1, text: "no reply here", reply: null }]);
  });

  await check("without Discord's reply action nothing is sent and the key is optional health", async () => {
    const p = page({ noActions: ["reply"] });
    p.run();
    await sleep(150);
    assert.strictEqual(p.last("health").ok, true);
    assert.deepStrictEqual(p.last("health").found.reply, []);
    p.win.__xsdDirect.send({ seq: 3, channel: F1, text: "never", reply: p.world.pool[F1][0].id });
    await sleep(100);
    const a = acts(p, 3);
    assert.deepStrictEqual(a.map((x) => [x.stage, x.why]), [["failed", "reply action not found"]]);
    assert.deepStrictEqual(p.world.received, []);
    assert.ok(!p.posts.some((x) => x.k === "act" && x.stage === "typing"));
  });

  await check("a reply to a message that is not loaded sends nothing", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.win.__xsdDirect.send({ seq: 4, channel: F1, text: "never", reply: "999" });
    await sleep(100);
    assert.deepStrictEqual(acts(p, 4).map((x) => x.why), ["reply target not found"]);
  });

  await check("a failed send is the store's SEND_FAILED, with the reason the page's own dialog gives", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.world.sendFails = true;
    p.world.failReason = "You are sending messages too fast. Slow down.";
    await sendText(p, { channel: F1 }, "refused text");
    await sleep(300);
    const failed = acts(p, 1).pop();
    assert.strictEqual(failed.stage, "failed");
    assert.strictEqual(failed.why, "send refused");
    assert.strictEqual(failed.reason, "You are sending messages too fast. Slow down.");
    assert.ok(failed.mid);
    assert.deepStrictEqual(p.world.received, []);
  });

  await check("a captcha frame after the send is a captcha, and nothing is sent", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.world.setCaptcha(true);
    await sendText(p, { channel: F1 }, "checked text");
    await sleep(120);
    assert.deepStrictEqual(stages(p, 1), ["typing", "captcha"]);
    assert.deepStrictEqual(p.world.received, []);
  });

  await check("no message box in time is a failure naming the box, and the health says no", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.win.__xsdDirect.send({ seq: 5, channel: "100000000000000099", text: "lost" });
    await sleep(500);
    assert.deepStrictEqual(acts(p, 5).map((x) => [x.stage, x.why]), [["failed", "composer not found"]]);
    assert.deepStrictEqual(p.last("health").found.composer, []);
  });

  await check("one action at a time, and cancel lets the page go", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.world.slowMs = 5000;
    assert.strictEqual(p.win.__xsdDirect.send({ seq: 1, channel: F1, text: "first" }), true);
    assert.strictEqual(p.win.__xsdDirect.send({ seq: 2, channel: F1, text: "second" }), false, "busy");
    assert.strictEqual(p.win.__xsdDirect.ack({ seq: 3, channel: F2 }), false);
    p.win.__xsdDirect.cancel();
    assert.strictEqual(p.win.__xsdDirect.ack({ seq: 3, channel: F2 }), true, "free again");
    await sleep(60);
    assert.deepStrictEqual(stages(p, 1), [], "the cancelled send says nothing more");
  });

  await check("bad asks are refused in the page: ids that are not digits, empty text", async () => {
    const p = page();
    p.run();
    await sleep(120);
    p.win.__xsdDirect.send({ seq: 1, channel: "1; alert(1)", text: "x" });
    await sleep(30);
    p.win.__xsdDirect.send({ seq: 2, channel: F1, text: "  " });
    await sleep(30);
    p.win.__xsdDirect.react({ seq: 3, channel: F1, message: "abc", emoji: "x", on: true });
    await sleep(30);
    assert.deepStrictEqual([1, 2, 3].map((n) => acts(p, n).map((x) => x.why)), [["page error"], ["page error"], ["page error"]]);
    assert.deepStrictEqual(p.routes, []);
  });

  for (const form of ["positional", "object"]) {
    await check("a reaction by Discord's own action (" + form + " arguments) is done when the store shows it, and again when it is gone", async () => {
      const p = page({ reactForm: form });
      p.run();
      await sleep(150);
      p.win.__xsdDirect.open(F1);
      await sleep(60);
      const target = p.world.pool[F1][0].id;
      p.win.__xsdDirect.react({ seq: 1, channel: F1, message: target, emoji: "\u{1F525}", on: true });
      await sleep(80);
      assert.deepStrictEqual(stages(p, 1), ["done"]);
      assert.deepStrictEqual(p.world.received, [{ kind: "react", channel: F1, message: target, emoji: "\u{1F525}", on: true }]);
      assert.strictEqual(p.world.pool[F1][0].reactions[0].me, true);
      // Already so: done without asking Discord again.
      p.win.__xsdDirect.react({ seq: 2, channel: F1, message: target, emoji: "\u{1F525}", on: true });
      await sleep(40);
      assert.deepStrictEqual(stages(p, 2), ["done"]);
      assert.strictEqual(p.world.received.length, 1);
      p.win.__xsdDirect.react({ seq: 3, channel: F1, message: target, emoji: "\u{1F525}", on: false });
      await sleep(80);
      assert.deepStrictEqual(stages(p, 3), ["done"]);
      assert.deepStrictEqual(p.world.pool[F1][0].reactions, []);
    });
  }

  await check("a reaction fails plainly without the action, or on a message not loaded", async () => {
    const p = page({ noActions: ["react"] });
    p.run();
    await sleep(150);
    assert.strictEqual(p.last("health").ok, true);
    p.win.__xsdDirect.react({ seq: 1, channel: F1, message: p.world.pool[F1][0].id, emoji: "x", on: true });
    await sleep(40);
    assert.deepStrictEqual(acts(p, 1).map((x) => x.why), ["react action not found"]);
    const q = page();
    q.run();
    await sleep(150);
    q.win.__xsdDirect.react({ seq: 1, channel: F1, message: "12345", emoji: "x", on: true });
    await sleep(40);
    assert.deepStrictEqual(acts(q, 1).map((x) => x.why), ["message not loaded"]);
  });

  await check("a reaction that never shows is not done", async () => {
    const p = page();
    p.run();
    await sleep(150);
    // Discord's action answers but the store never shows the reaction.
    p.win.__xsdDirect.open(F1);
    await sleep(60);
    const target = p.world.pool[F1][0];
    Object.defineProperty(target, "reactions", { get: () => [], set: () => {}, configurable: true });
    p.win.__xsdDirect.react({ seq: 1, channel: F1, message: target.id, emoji: "\u{1F44D}", on: true });
    await sleep(320);
    assert.deepStrictEqual(acts(p, 1).map((x) => [x.stage, x.why]), [["failed", "not shown"]]);
  });

  await check("marking read is Discord's own action, done when the unread count is 0", async () => {
    const p = page({ markReadOnOpen: false });
    p.run();
    await sleep(150);
    assert.strictEqual(p.last("dms").dms.find((d) => d.id === F2).unread, 2);
    p.win.__xsdDirect.ack({ seq: 1, channel: F2 });
    await sleep(80);
    assert.deepStrictEqual(stages(p, 1), ["done"]);
    assert.deepStrictEqual(p.world.received, [{ kind: "read", channel: F2 }]);
    assert.strictEqual(p.last("dms").dms.find((d) => d.id === F2).unread, 0);
    p.win.__xsdDirect.ack({ seq: 2, channel: F2 });
    await sleep(30);
    assert.deepStrictEqual(stages(p, 2), ["done"], "nothing unread: done");
    assert.strictEqual(p.world.received.length, 1);
    const q = page({ noActions: ["ack"] });
    q.run();
    await sleep(150);
    assert.strictEqual(q.last("health").ok, true);
    assert.deepStrictEqual(q.last("health").found.ack, []);
    q.win.__xsdDirect.ack({ seq: 1, channel: F2 });
    await sleep(30);
    assert.deepStrictEqual(acts(q, 1).map((x) => x.why), ["ack action not found"]);
  });

  await check("a conversation opened with unread messages says whether the page read it, and reads it itself if not", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.win.__xsdDirect.open(F2);
    await sleep(100);
    assert.deepStrictEqual(p.all("opened"), [{ k: "opened", read: true, acked: false }]);
    p.win.__xsdDirect.open(F1);
    await sleep(100);
    assert.strictEqual(p.all("opened").length, 1, "nothing was unread: nothing to say");
    const q = page({ markReadOnOpen: false });
    q.run();
    await sleep(150);
    q.win.__xsdDirect.open(F2);
    await sleep(300);
    assert.deepStrictEqual(q.all("opened"), [{ k: "opened", read: false, acked: true }]);
    assert.deepStrictEqual(q.world.received, [{ kind: "read", channel: F2 }]);
    assert.strictEqual(q.last("dms").dms.find((d) => d.id === F2).unread, 0);
  });

  await check("a failed message of ours is dropped from Discord's store locally, and only that", async () => {
    const p = page();
    p.run();
    await sleep(150);
    p.world.sendFails = true;
    await sendText(p, { channel: F1 }, "will fail");
    await sleep(300);
    const failed = acts(p, 1).pop();
    assert.strictEqual(failed.stage, "failed");
    assert.strictEqual(p.win.__xsdDirect.forget(F1, p.world.pool[F1][0].id), false, "not ours and failed");
    assert.strictEqual(p.win.__xsdDirect.forget(F1, failed.mid), true);
    assert.deepStrictEqual(p.world.deleted, [{ channelId: F1, id: failed.mid, local: true }]);
    assert.strictEqual(p.win.__xsdDirect.forget(F1, failed.mid), false, "gone");
  });

  await check("every message of ours maps to one of three states", async () => {
    const p = page();
    p.run();
    await sleep(120);
    p.win.__xsdDirect.open(F1);
    p.world.pool[F1][1].state = "SOMETHING_NEW";
    p.world.stores.MessageStore.emitChange();
    await sleep(80);
    assert.strictEqual(p.last("conv").msgs[1].state, "SENT");
  });

  process.exit(failed ? 1 : 0);
})();
