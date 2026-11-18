// --voice-join and --call when the page's own button cannot be found: Discord's own voice action
// selectVoiceChannel(CID) (declared before this), found like the stores (js/store_find.js before this). Called once.
(() => {
  const S = xsdStores().S || {};
  if (!S.voiceActions) return { called: false, why: "selectVoiceChannel not found" };
  try {
    S.voiceActions.selectVoiceChannel(CID);
    return { called: true };
  } catch (e) {
    return { called: false, why: "it threw " + e.name };
  }
})()
