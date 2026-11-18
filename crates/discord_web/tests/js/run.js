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

const FAST = { poll: 5, settle: 2, cap: 400, coalesce: 20, tries: 3, gap: 10, slow: 40 };
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
  win.document = { querySelector: () => null, querySelectorAll: () => [] };
  // The page script has no network: nothing to call.
  const ctx = vm.createContext(win);
  const world = installFakeDiscord(win, opts);
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

  process.exit(failed ? 1 : 0);
})();
