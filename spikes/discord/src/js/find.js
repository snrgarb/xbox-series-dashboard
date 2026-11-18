// --send, first: the DM row whose name is TARGET (exactly, else ignoring case; refused if more than one), scrolled
// into view, and the point to click. TARGET is declared before this by the Rust side, as a JSON string.
(async () => {
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const DM = 'a[href^="/channels/@me/"]';
  const rowsNow = () =>
    [...document.querySelectorAll(DM)]
      .filter((a) => !a.closest('[data-list-id="guildsnav"]'))
      .map((a) => {
        const nameEl = a.querySelector('[class*="name_"]');
        return { a, name: (nameEl ? nameEl.textContent : a.getAttribute("aria-label") || a.textContent || "").trim() };
      });
  const match = (rows) => {
    let hits = rows.filter((r) => r.name === TARGET);
    if (!hits.length) hits = rows.filter((r) => r.name.toLowerCase() === TARGET.toLowerCase());
    return hits;
  };
  const first = rowsNow()[0];
  let scroller = first && first.a;
  while (scroller && !(scroller.scrollHeight > scroller.clientHeight + 4 && /(auto|scroll)/.test(getComputedStyle(scroller).overflowY))) {
    scroller = scroller.parentElement;
  }
  const seen = new Map();
  let pages = 1;
  const look = () => { for (const r of match(rowsNow())) seen.set(r.a.getAttribute("href"), r); };
  if (scroller) { scroller.scrollTop = 0; await sleep(300); }
  look();
  while (!seen.size && scroller && scroller.scrollTop + scroller.clientHeight < scroller.scrollHeight - 2 && pages < 60) {
    scroller.scrollTop += Math.max(100, scroller.clientHeight * 0.8);
    pages++;
    await sleep(350);
    look();
  }
  if (seen.size !== 1) return { found: seen.size, pages, rows: rowsNow().length };
  const [href, hit] = [...seen.entries()][0];
  hit.a.scrollIntoView({ block: "center" });
  await sleep(300);
  const link = [...document.querySelectorAll(`a[href="${href}"]`)].find((a) => !a.closest('[data-list-id="guildsnav"]'));
  const r = link.getBoundingClientRect();
  const x = r.left + Math.min(r.width / 2, 120), y = r.top + r.height / 2;
  const top = document.elementFromPoint(x, y);
  return { found: 1, pages, href, x, y, covered: !(top && link.contains(top)), exact: hit.name === TARGET };
})()
