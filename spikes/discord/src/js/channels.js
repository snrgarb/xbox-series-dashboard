// --voice-list, --voice-join, --dump-shape: once a server is open, its channel list read a page at a time, the
// paging driven from the Rust side (a trusted mouse wheel over the list, else scrollTop) so the virtual list really
// moves. OP (declared before this, with TARGET): "start" (from the top), "look" (the rows shown now added), "nudge"
// (scrollTop moved by most of a page), "result" (everything seen; TARGET, a string or null, the one to join).
// Voice channels are told by any label in or around the row saying "voice", or users shown in or after it. Names
// and ids come back for the Rust side: names to voice-names.txt, ids to the route, neither to the log.
(async () => {
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const all = (s, root) => { try { return [...(root || document).querySelectorAll(s)]; } catch (e) { return []; } };
  const ROW = '[data-list-item-id^="channels___"]';
  const rows = () => all(ROW);
  if (OP === "start" || !window.__xsdCh) window.__xsdCh = { seen: new Map() };
  const S = window.__xsdCh;
  const findScroller = () => {
    const first = rows()[0];
    let s = first;
    while (s && !(s.scrollHeight > s.clientHeight + 4 && /(auto|scroll)/.test(getComputedStyle(s).overflowY))) s = s.parentElement;
    return s;
  };
  const look = () => {
    for (const el of rows()) {
      const id = el.getAttribute("data-list-item-id").slice("channels___".length);
      const box = el.closest("li") || el.parentElement;
      const labels = [el, ...all("[aria-label]", el), ...(box ? all("[aria-label]", box) : []), ...(box && box.hasAttribute("aria-label") ? [box] : [])]
        .map((x) => x.getAttribute("aria-label") || "");
      const aria = labels.join(" | ");
      const next = box && box.nextElementSibling;
      const follow = next && !next.querySelector(ROW) ? next : null;
      const users = (box ? all('[class*="voiceUser"]', box).length : 0) + (follow ? all('[class*="voiceUser"]', follow).length : 0);
      const nameEl = el.querySelector('[class*="name_"]');
      const old = S.seen.get(id);
      S.seen.set(id, {
        id,
        name: (nameEl ? nameEl.textContent : (el.getAttribute("aria-label") || "").replace(/\s*\(.*$/, "").replace(/,.*$/, "")).trim(),
        voice: /voice/i.test(aria),
        stage: /stage/i.test(aria),
        users: Math.max(users, old ? old.users : 0),
        nameClass: !!nameEl,
        labelled: labels.some((l) => l),
        link: el.tagName === "A" || !!el.querySelector('a[href^="/channels/"]'),
      });
    }
  };
  const metrics = () => {
    const s = findScroller();
    const r = s ? s.getBoundingClientRect() : { left: 0, top: 0, width: 0, height: 0 };
    return {
      scroller: !!s,
      top: s ? Math.round(s.scrollTop) : 0,
      client: s ? s.clientHeight : 0,
      height: s ? s.scrollHeight : 0,
      atEnd: !s || s.scrollTop + s.clientHeight >= s.scrollHeight - 2,
      x: r.left + r.width / 2,
      y: r.top + r.height / 2,
      seen: S.seen.size,
      shown: rows().length,
      collapsed: all('[aria-expanded="false"]', s || document).filter((e) => e.closest(ROW) || e.querySelector(ROW) || /categor/i.test(e.className || "")).length,
      scrollerClass: s ? String(s.className || "").split(/\s+/)[0] : "",
    };
  };
  if (OP === "start") {
    // The list may come a moment after the route changes: 10 s at most.
    for (let i = 0; i < 40 && !rows().length; i++) await sleep(250);
    const s = findScroller();
    if (s) { s.scrollTop = 0; await sleep(300); }
    look();
    return metrics();
  }
  if (OP === "look") { look(); return metrics(); }
  if (OP === "nudge") {
    const s = findScroller();
    if (s) s.scrollTop += Math.max(100, s.clientHeight * 0.8);
    return metrics();
  }
  // "result"
  const list = [...S.seen.values()];
  const isVoice = (c) => c.voice || c.stage || c.users > 0;
  const voice = list.filter(isVoice);
  const counts = {
    rows: list.length,
    voice_by_label: list.filter((c) => c.voice).length,
    stage_by_label: list.filter((c) => c.stage).length,
    name_class: list.filter((c) => c.nameClass).length,
    voice_users: list.reduce((s, c) => s + c.users, 0),
    labelled: list.filter((c) => c.labelled).length,
    links: list.filter((c) => c.link).length,
    with_users: list.filter((c) => c.users > 0).length,
  };
  const out = {
    counts,
    voice: voice.map((c) => ({ name: c.name, users: c.users, stage: c.stage })),
    rows: list.map((c) => ({ name: c.name, users: c.users, voice: isVoice(c), link: c.link })),
    // For --dump-shape to page to one (an id: never logged).
    firstVoice: voice.length ? voice[0].id : null,
  };
  if (TARGET === null) return out;
  const pick = (pool) => {
    let hits = pool.filter((c) => c.name === TARGET);
    const exact = hits.length > 0;
    if (!exact) hits = pool.filter((c) => c.name.toLowerCase() === TARGET.toLowerCase());
    return { hits, exact };
  };
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
