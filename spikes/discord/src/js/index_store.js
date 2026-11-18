// --index, way A: the whole index from Discord's stores (js/store_find.js before this), read-only: every server
// (id, name), every channel of each (id, name, type, parent, position) and how many are in each voice channel, every
// DM (id, type, the recipients' display names). Names and ids go to the Rust side's index.txt, never the log.
(() => {
  const started = performance.now();
  const st = xsdStores();
  const S = st.S || {};
  const out = { ok: false, why: st.why, found: st.found, modules: st.modules || 0, diag: st.diag, guilds: [], channels: [], dms: [] };
  if (!S.guilds || !(S.channels || S.guildChannels)) {
    out.why = out.why || "the guild or channel store was not found";
    return out;
  }
  const call = (o, n, ...a) => { try { return o && typeof o[n] === "function" ? o[n](...a) : undefined; } catch (e) { return undefined; } };
  const voiceCount = (cid) => {
    const v = call(S.voice, "getVoiceStatesForChannel", cid);
    return v ? (Array.isArray(v) ? v.length : Object.keys(v).length) : 0;
  };
  const guilds = call(S.guilds, "getGuilds") || {};
  for (const g of Object.values(guilds)) {
    out.guilds.push({ id: String(g.id), name: String(g.name || "") });
    let list = [];
    const mutable = call(S.channels, "getMutableGuildChannelsForGuild", g.id);
    if (mutable) list = Object.values(mutable);
    else {
      const groups = call(S.guildChannels, "getChannels", g.id) || {};
      for (const arr of Object.values(groups)) if (Array.isArray(arr)) for (const x of arr) if (x && x.channel) list.push(x.channel);
    }
    const seen = new Set();
    for (const c of list) {
      if (!c || seen.has(c.id)) continue;
      seen.add(c.id);
      const type = Number(c.type);
      out.channels.push({
        guild: String(g.id),
        id: String(c.id),
        name: String(c.name || ""),
        type,
        parent: c.parent_id ? String(c.parent_id) : (c.parentId ? String(c.parentId) : ""),
        position: Number(c.position || 0),
        users: type === 2 || type === 13 ? voiceCount(c.id) : 0,
      });
    }
  }
  let dmIds = call(S.privateSort, "getPrivateChannelIds");
  if (!Array.isArray(dmIds)) {
    const sorted = call(S.privateSort, "getSortedPrivateChannels") || call(S.channels, "getSortedPrivateChannels") || [];
    dmIds = sorted.map((c) => c && (c.channelId || c.id)).filter(Boolean);
  }
  for (const id of dmIds) {
    const c = call(S.channels, "getChannel", id);
    if (!c) continue;
    const ids = c.recipients || c.rawRecipients || [];
    const names = ids.map((r) => {
      const u = typeof r === "object" ? r : call(S.users, "getUser", r);
      return u ? String(u.globalName || u.global_name || u.username || "") : "";
    });
    out.dms.push({ id: String(id), type: Number(c.type), name: c.name ? String(c.name) : names.join(", "), users: voiceCount(id) });
  }
  out.ok = true;
  out.ms = Math.round(performance.now() - started);
  return out;
})()
