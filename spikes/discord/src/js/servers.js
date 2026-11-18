// --voice-list, --voice-join: the server bar's servers, and the one named TARGET (declared before this: a string,
// or null to only count). The names come back for the Rust side to write to voice-names.txt, never to the log; the
// log gets the counts.
(() => {
  const all = (s, root) => { try { return [...(root || document).querySelectorAll(s)]; } catch (e) { return []; } };
  const bar = all('[data-list-id="guildsnav"]')[0] || document;
  // A server's item: guildsnav___<id>, a number (home, add a server and discover are words).
  const items = all('[data-list-item-id^="guildsnav___"]', bar).filter((el) => /^guildsnav___\d+$/.test(el.getAttribute("data-list-item-id")));
  const servers = items.map((el) => {
    const dnd = el.closest("[data-dnd-name]");
    const aria = (el.getAttribute("aria-label") || "").trim();
    const name = dnd ? dnd.getAttribute("data-dnd-name") : aria;
    return { id: el.getAttribute("data-list-item-id").slice("guildsnav___".length), name: (name || "").trim(), from: dnd ? "dnd" : aria ? "aria" : "none" };
  });
  const counts = {
    guildsnav: all('[data-list-id="guildsnav"]').length,
    items_all: all('[data-list-item-id^="guildsnav___"]', bar).length,
    servers: servers.length,
    name_dnd: servers.filter((s) => s.from === "dnd").length,
    name_aria: servers.filter((s) => s.from === "aria").length,
    folders: all('[class*="folder" i][aria-expanded]', bar).length,
    dm_rows: all('a[href^="/channels/@me/"]').filter((a) => !a.closest('[data-list-id="guildsnav"]')).length,
  };
  const out = { counts, names: servers.map((s) => s.name) };
  if (TARGET === null) return out;
  let hits = servers.filter((s) => s.name === TARGET);
  const exact = hits.length > 0;
  if (!exact) hits = servers.filter((s) => s.name.toLowerCase() === TARGET.toLowerCase());
  out.found = new Set(hits.map((h) => h.id)).size;
  if (out.found === 1) {
    out.gid = hits[0].id;
    out.exact = exact;
  }
  return out;
})()
