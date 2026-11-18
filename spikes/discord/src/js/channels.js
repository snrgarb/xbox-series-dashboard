// --voice-list, --voice-join: once a server is open, its channel list read whole (it is virtualised: scrolled a page
// at a time, 40 pages at most), its voice channels told by their accessible label ("(voice channel)", English page)
// and kept with their names and how many are in each; TARGET (declared before this: a string, or null) the one to
// join. Names and ids come back for the Rust side: names to voice-names.txt, ids to the route, neither to the log.
(async () => {
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const all = (s, root) => { try { return [...(root || document).querySelectorAll(s)]; } catch (e) { return []; } };
  const ROW = '[data-list-item-id^="channels___"]';
  const rows = () => all(ROW);
  const seen = new Map();
  const counts = { rows: 0, voice_by_label: 0, stage_by_label: 0, name_class: 0, voice_users: 0 };
  const look = () => {
    for (const el of rows()) {
      const id = el.getAttribute("data-list-item-id").slice("channels___".length);
      if (seen.has(id)) continue;
      const aria = el.getAttribute("aria-label") || "";
      const voice = /\(voice channel\)|voice channel/i.test(aria);
      const stage = /\(stage channel\)|stage channel/i.test(aria);
      const nameEl = el.querySelector('[class*="name_"]');
      const name = (nameEl ? nameEl.textContent : aria.replace(/\s*\(.*$/, "").replace(/,.*$/, "")).trim();
      const box = el.closest("li") || el.parentElement;
      const users = box ? all('[class*="voiceUser_"]', box).length : 0;
      seen.set(id, { id, name, voice, stage, users, nameClass: !!nameEl });
    }
  };
  // The list may come a moment after the route changes: 10 s at most.
  for (let i = 0; i < 40 && !rows().length; i++) await sleep(250);
  const first = rows()[0];
  let scroller = first;
  while (scroller && !(scroller.scrollHeight > scroller.clientHeight + 4 && /(auto|scroll)/.test(getComputedStyle(scroller).overflowY))) {
    scroller = scroller.parentElement;
  }
  let pages = 1;
  if (scroller) { scroller.scrollTop = 0; await sleep(300); }
  look();
  while (scroller && scroller.scrollTop + scroller.clientHeight < scroller.scrollHeight - 2 && pages < 40) {
    scroller.scrollTop += Math.max(100, scroller.clientHeight * 0.8);
    pages++;
    await sleep(350);
    look();
  }
  const list = [...seen.values()];
  counts.rows = list.length;
  counts.voice_by_label = list.filter((c) => c.voice).length;
  counts.stage_by_label = list.filter((c) => c.stage).length;
  counts.name_class = list.filter((c) => c.nameClass).length;
  counts.voice_users = list.reduce((s, c) => s + c.users, 0);
  const voice = list.filter((c) => c.voice || c.stage);
  const out = { counts, pages, scroller: !!scroller, voice: voice.map((c) => ({ name: c.name, users: c.users, stage: c.stage })) };
  if (TARGET === null) return out;
  let hits = voice.filter((c) => c.name === TARGET);
  const exact = hits.length > 0;
  if (!exact) hits = voice.filter((c) => c.name.toLowerCase() === TARGET.toLowerCase());
  out.found = hits.length;
  if (hits.length === 1) {
    out.cid = hits[0].id;
    out.exact = exact;
    out.stage = hits[0].stage;
  }
  return out;
})()
