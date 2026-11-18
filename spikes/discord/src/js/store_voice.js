// The hold, once a second, from Discord's stores (js/store_find.js before this), read-only: the call's channel CID
// (declared before this), who is in it by its voice states, how many of them the speaking store says are speaking,
// how many are muted or deafened, whether the user is one of them, and the user's own mute and deafen. Counts and
// booleans only; -1 where a store was not found.
(() => {
  const S = xsdStores().S || {};
  const call = (o, n, ...a) => { try { return o && typeof o[n] === "function" ? o[n](...a) : undefined; } catch (e) { return undefined; } };
  const v = CID ? call(S.voice, "getVoiceStatesForChannel", CID) : undefined;
  const list = v ? (Array.isArray(v) ? v : Object.values(v)) : [];
  const me = call(S.users, "getCurrentUser");
  const ids = list.map((s) => s && (s.userId || s.user_id)).filter(Boolean);
  const speaking = S.speaking ? ids.filter((id) => call(S.speaking, "isSpeaking", id)).length : -1;
  const flag = (n) => { const r = call(S.media, n); return r === undefined ? null : !!r; };
  return {
    users: S.voice ? list.length : -1,
    speaking,
    muted: list.filter((s) => s && (s.selfMute || s.mute)).length,
    deafened: list.filter((s) => s && (s.selfDeaf || s.deaf)).length,
    meIn: !!(me && ids.includes(me.id)),
    selfMute: flag("isSelfMute"),
    selfDeaf: flag("isSelfDeaf"),
  };
})()
