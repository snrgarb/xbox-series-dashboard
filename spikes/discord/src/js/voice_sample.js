// The voice steps, once a second: how the call stands as the page shows it, and whether audio flows. CID (declared
// before this: a voice channel's id, or null for a DM call) scopes who is counted. Counts, states, booleans and
// Discord's own status words from a fixed list only: never a name, a message or an id.
(async () => {
  const R = window.__xsdVoice || { pcs: [], contexts: [], mics: [], media: new Set(), ticks: 0, maxGap: 0 };
  const all = (s, root) => { try { return [...(root || document).querySelectorAll(s)]; } catch (e) { return []; } };
  const one = (s, root) => all(s, root)[0] || null;
  const within = (ms, p) => Promise.race([p, new Promise((r) => setTimeout(() => r(null), ms))]);

  // The voice panel above the user area, and its status.
  const panels = one('section[aria-label="User area"]') || one('[class*="panels_"]');
  const status = one('[class*="rtcConnectionStatus_"]', panels) || one('[class*="rtcConnectionStatus_"]');
  const statusClass = one('[class*="rtcConnectionStatusConnected_"]') ? "connected"
    : one('[class*="rtcConnectionStatusConnecting_"]') ? "connecting"
    : one('[class*="rtcConnectionStatusError_"]') ? "error" : status ? "other" : "none";
  const KNOWN = /^(voice connected|video connected|stream connected|rtc connecting|connecting|awaiting endpoint|ice checking|dtls connecting|no route|rtc disconnected|voice disconnected)$/i;
  // Discord's own status words, from a text node of the status that is one of the known ones; else only a length.
  const leaves = status ? [status, ...all("*", status)].filter((e) => e.childNodes.length === 1 && e.firstChild.nodeType === 3) : [];
  const known = leaves.map((e) => e.textContent.trim()).find((t) => KNOWN.test(t)) || "";
  const words = known || (status ? status.textContent.trim() : "");
  const disconnect = one('button[aria-label="Disconnect"]', panels) || one('button[aria-label="Disconnect"]');
  const sw = (re) => {
    const inPanel = all('button[role="switch"], button[aria-label]', panels).find((b) => re.test(b.getAttribute("aria-label") || ""));
    const any = inPanel || all('button[role="switch"], button[aria-label]').find((b) => re.test(b.getAttribute("aria-label") || ""));
    if (!any) return { present: false };
    return { present: true, where: inPanel ? "user panel" : "elsewhere", checked: any.getAttribute("aria-checked"), label: (any.getAttribute("aria-label") || "").toLowerCase() };
  };

  // Who is in the call, and who is speaking: under the voice channel's row, or the DM call's tiles.
  const row = CID ? one(`[data-list-item-id="channels___${CID}"]`) : null;
  const channelBox = row ? (row.closest("li") || row.parentElement) : null;
  const call = one('[class*="callContainer_"]') || one('[class*="voiceCallWrapper_"]');
  const tiles = all('[data-selenium-video-tile], [class*="tile_"]', call || document.createElement("div"));
  const scope = channelBox || call;

  // Audio, from the objects Discord made (js/voice_hooks.js).
  R.pcs = R.pcs.filter((pc) => pc.connectionState !== "closed");
  const audio = { pcs: R.pcs.length, pcsConnected: 0, inBytes: 0, inPackets: 0, inEnergy: 0, inLevel: 0, inStreams: 0, outBytes: 0, micLevel: 0, outStreams: 0 };
  for (const pc of R.pcs) {
    if (pc.connectionState === "connected") audio.pcsConnected++;
    const stats = await within(2000, pc.getStats().catch(() => null));
    if (!stats) continue;
    stats.forEach((s) => {
      if (s.type === "inbound-rtp" && s.kind === "audio") {
        audio.inStreams++;
        audio.inBytes += s.bytesReceived || 0;
        audio.inPackets += s.packetsReceived || 0;
        audio.inEnergy += s.totalAudioEnergy || 0;
        audio.inLevel = Math.max(audio.inLevel, s.audioLevel || 0);
      } else if (s.type === "outbound-rtp" && s.kind === "audio") {
        audio.outStreams++;
        audio.outBytes += s.bytesSent || 0;
      } else if (s.type === "media-source" && s.kind === "audio") {
        audio.micLevel = Math.max(audio.micLevel, s.audioLevel || 0);
      }
    });
  }
  R.contexts = R.contexts.filter((c) => c.state !== "closed");
  const media = new Set([...R.media, ...all("audio, video")]);
  const mediaList = [...media];
  const mics = R.mics.filter((t) => t.readyState === "live");
  const timer = { ticks: R.ticks, maxGap: Math.round(R.maxGap) };
  R.ticks = 0;
  R.maxGap = 0;

  // An incoming call's ring, as the page may show it.
  const ringBox = one('[class*="incomingCall" i]') || one('[class*="ringing" i]') || one('[aria-label*="incoming call" i]');
  // Only the ring's own buttons: an "Accept" elsewhere (a friend request) is never counted or pressed.
  const buttons = ringBox ? all('button, [role="button"]', ringBox) : [];
  const accept = buttons.filter((b) => /accept|answer|join call/i.test(b.getAttribute("aria-label") || b.textContent || ""));
  const decline = buttons.filter((b) => /decline|ignore|reject/i.test(b.getAttribute("aria-label") || b.textContent || ""));

  return {
    visibility: document.visibilityState,
    focus: document.hasFocus(),
    panel: !!status,
    statusClass,
    status: KNOWN.test(words) ? words : words ? `(other, ${words.length} chars)` : "",
    disconnect: !!disconnect,
    mute: sw(/^(mute|unmute)$/i),
    deafen: sw(/^(deafen|undeafen)$/i),
    channelRow: !!row,
    voiceUsers: scope ? all('[class*="voiceUser_"]', scope).length : 0,
    voiceUsersPage: all('[class*="voiceUser_"]').length,
    speakingNames: scope ? all('[class*="usernameSpeaking_"]', scope).length : 0,
    speakingAvatars: scope ? all('[class*="avatarSpeaking_"]', scope).length : 0,
    speakingTiles: call ? all('[class*="speaking_"]', call).length : 0,
    callView: !!call,
    tiles: tiles.length,
    audio,
    contexts: R.contexts.length,
    contextsRunning: R.contexts.filter((c) => c.state === "running").length,
    contextTime: R.contexts.reduce((m, c) => Math.max(m, c.currentTime), 0),
    mics: mics.length,
    micEnabled: mics.filter((t) => t.enabled).length,
    micMuted: mics.filter((t) => t.muted).length,
    media: mediaList.length,
    mediaWithStream: mediaList.filter((m) => !!m.srcObject).length,
    mediaPlaying: mediaList.filter((m) => !m.paused).length,
    mediaTime: mediaList.reduce((s, m) => s + (m.currentTime || 0), 0),
    timer,
    ring: !!ringBox,
    ringAccept: accept.length,
    ringDecline: decline.length,
  };
})()
