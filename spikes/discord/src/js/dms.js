// The DM list as the page shows it, read from the DOM and its accessible attributes: never Discord's API, never its
// token or storage. Run once by --dms through DevTools' Runtime.evaluate (awaitPromise), since Discord's list is
// virtualised: only the rows on screen exist, so the list is scrolled a page at a time and read at each.
// Returns the counts of every selector tried (for the log) and the rows (names for the separate file only).
(async () => {
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const counts = {};
  const count = (name, selector) => {
    let n;
    try { n = document.querySelectorAll(selector).length; } catch (e) { n = -1; }
    counts[name] = n;
    return n;
  };
  const DM = 'a[href^="/channels/@me/"]';
  const GUILDS = '[data-list-id="guildsnav"]';
  count("dm_links", DM);
  count("dm_links_in_guild_bar", `${GUILDS} ${DM}`);
  count("guild_bar_items", `${GUILDS} [data-list-item-id^="guildsnav___"]`);
  count("guild_bar_badges", `${GUILDS} [class*="numberBadge"]`);
  count("listitem_role_dm", `[role="listitem"] ${DM}, li ${DM}`);
  count("dm_name_class", `${DM} [class*="name_"]`);
  count("dm_aria_label", `${DM}[aria-label]`);
  count("dm_number_badge", `li [class*="numberBadge"]`);
  count("unread_class", '[class*="modeUnread"], [class*="unread_"]');
  count("private_channels_list", '[data-list-id^="private-channels"], ul[aria-label]');

  // DMs with unread messages are also shown in the server bar: their ids, and badge counts there.
  const bar = new Map();
  for (const item of document.querySelectorAll(`${GUILDS} [data-list-item-id^="guildsnav___"]`)) {
    const id = item.getAttribute("data-list-item-id").slice("guildsnav___".length);
    const link = item.matches(DM) ? item : item.querySelector(DM);
    const href = link ? link.getAttribute("href") : "";
    const badge = item.querySelector('[class*="numberBadge"]');
    bar.set(id, { href, badge: badge ? parseInt(badge.textContent, 10) || 0 : 0 });
  }

  const inBar = (a) => !!a.closest(GUILDS);
  const first = [...document.querySelectorAll(DM)].find((a) => !inBar(a));
  let scroller = first;
  while (scroller && !(scroller.scrollHeight > scroller.clientHeight + 4 && /(auto|scroll)/.test(getComputedStyle(scroller).overflowY))) {
    scroller = scroller.parentElement;
  }

  const rows = new Map();
  const read = () => {
    for (const a of document.querySelectorAll(DM)) {
      if (inBar(a)) continue;
      const href = a.getAttribute("href");
      if (rows.has(href)) continue;
      const li = a.closest("li") || a.parentElement;
      const nameEl = a.querySelector('[class*="name_"]');
      const label = a.getAttribute("aria-label") || "";
      let name = nameEl ? nameEl.textContent.trim() : "";
      let nameFrom = name ? "name_class" : "";
      if (!name && label) { name = label; nameFrom = "aria-label"; }
      if (!name) { name = (a.textContent || "").trim(); nameFrom = "text"; }
      const badge = (li || a).querySelector('[class*="numberBadge"]');
      const labelCount = (label.match(/^(\d+)/) || [])[1];
      const classes = [li && li.className, a.className, a.parentElement && a.parentElement.className].join(" ");
      const id = href.split("/").pop();
      const barEntry = bar.get(id) || [...bar.values()].find((b) => b.href === href);
      rows.set(href, {
        name,
        nameFrom,
        mentions: badge ? parseInt(badge.textContent, 10) || 0 : labelCount ? +labelCount : barEntry ? barEntry.badge : 0,
        mentionsFrom: badge ? "row_badge" : labelCount ? "aria-label" : barEntry && barEntry.badge ? "guild_bar_badge" : "",
        unreadClass: /unread/i.test(classes),
        unreadBar: !!barEntry,
        weight: nameEl ? parseInt(getComputedStyle(nameEl).fontWeight, 10) || 0 : 0,
      });
    }
  };

  read();
  let pages = 1;
  if (scroller) {
    scroller.scrollTop = 0;
    await sleep(300);
    read();
    while (scroller.scrollTop + scroller.clientHeight < scroller.scrollHeight - 2 && pages < 60) {
      scroller.scrollTop += Math.max(100, scroller.clientHeight * 0.8);
      pages++;
      await sleep(350);
      read();
    }
    scroller.scrollTop = 0;
  }

  // Unread by weight: most rows share one font weight; a heavier name is an unread one.
  const list = [...rows.values()];
  const weights = list.map((r) => r.weight).filter(Boolean).sort((a, b) => a - b);
  const usual = weights.length ? weights[Math.floor(weights.length / 2)] : 0;
  for (const r of list) r.unreadWeight = !!usual && r.weight > usual;
  return { counts, scroller: !!scroller, pages, bar: bar.size, rows: list };
})()
