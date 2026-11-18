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
      const box = el.closest("li") || el.parentElement;
      // The label may be on the row, a link inside it, or its list item (the first TV run: none on the row).
      const labels = [el, ...all("[aria-label]", el), ...(box ? all("[aria-label]", box) : []), ...(box && box.hasAttribute("aria-label") ? [box] : [])]
        .map((x) => x.getAttribute("aria-label") || "");
      const aria = labels.join(" | ");
      const voice = /voice/i.test(aria);
      const stage = /stage/i.test(aria);
      const linkToChannel = el.tagName === "A" || !!el.querySelector('a[href^="/channels/"]');
      const nameEl = el.querySelector('[class*="name_"]');
      const name = (nameEl ? nameEl.textContent : (el.getAttribute("aria-label") || "").replace(/\s*\(.*$/, "").replace(/,.*$/, "")).trim();
      // Its users: inside its list item, or in the element right after it when that is not another channel.
      const next = box && box.nextElementSibling;
      const follow = next && !next.querySelector(ROW) ? next : null;
      const users = (box ? all('[class*="voiceUser"]', box).length : 0) + (follow ? all('[class*="voiceUser"]', follow).length : 0);
      seen.set(id, { id, name, voice, stage, users, nameClass: !!nameEl, labelled: labels.some((l) => l), link: linkToChannel });
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
  counts.labelled = list.filter((c) => c.labelled).length;
  counts.links = list.filter((c) => c.link).length;
  counts.with_users = list.filter((c) => c.users > 0).length;
  // Voice: labelled so, or showing users in it.
  const isVoice = (c) => c.voice || c.stage || c.users > 0;
  const voice = list.filter(isVoice);
  const out = {
    counts,
    pages,
    scroller: !!scroller,
    voice: voice.map((c) => ({ name: c.name, users: c.users, stage: c.stage })),
    // Every row, for the names file when no voice channel is told apart.
    rows: list.map((c) => ({ name: c.name, users: c.users, voice: isVoice(c), link: c.link })),
  };
  if (TARGET === null) return out;
  const pick = (pool) => {
    let hits = pool.filter((c) => c.name === TARGET);
    const exact = hits.length > 0;
    if (!exact) hits = pool.filter((c) => c.name.toLowerCase() === TARGET.toLowerCase());
    return { hits, exact };
  };
  // A voice channel of that name; else any one row of that name (its kind unknown: the press shows what it is).
  let { hits, exact } = pick(voice);
  out.kind = "voice";
  if (!hits.length) {
    ({ hits, exact } = pick(list));
    out.kind = "unknown";
  }
  out.found = hits.length;
  if (hits.length === 1) {
    out.cid = hits[0].id;
    out.exact = exact;
    out.stage = hits[0].stage;
  }
  return out;
})()
