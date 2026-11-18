// The voice steps (#292), before Discord's own scripts run (DevTools' Page.addScriptToEvaluateOnNewDocument): a
// note of the page's peer connections, audio contexts, microphone tracks and media elements as Discord makes them,
// so the spike can say whether audio flows while the page is hidden. Each is Discord's own object, made as before
// and returned unchanged; only a reference is kept. And a 1 s timer, to see whether timers are slowed.
// Read by js/voice_sample.js. Nothing here reads a name, a message, a token or the page's storage.
(() => {
  if (window.__xsdVoice) return;
  const R = { pcs: [], contexts: [], mics: [], media: new Set(), ticks: 0, maxGap: 0, last: performance.now() };
  Object.defineProperty(window, "__xsdVoice", { value: R, enumerable: false });
  const wrap = (name, keep) => {
    const Orig = window[name];
    if (typeof Orig !== "function") return;
    window[name] = new Proxy(Orig, {
      construct(target, args, newTarget) {
        const made = Reflect.construct(target, args, newTarget);
        try { keep(made); } catch (e) {}
        return made;
      },
    });
  };
  wrap("RTCPeerConnection", (pc) => R.pcs.push(pc));
  wrap("AudioContext", (ctx) => R.contexts.push(ctx));
  try {
    const md = navigator.mediaDevices;
    const gum = md && md.getUserMedia && md.getUserMedia.bind(md);
    if (gum) {
      md.getUserMedia = (constraints) =>
        gum(constraints).then((stream) => {
          try { for (const t of stream.getAudioTracks()) R.mics.push(t); } catch (e) {}
          return stream;
        });
    }
  } catch (e) {}
  try {
    const play = HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play = function () {
      try { R.media.add(this); } catch (e) {}
      return play.apply(this, arguments);
    };
  } catch (e) {}
  setInterval(() => {
    const now = performance.now();
    R.maxGap = Math.max(R.maxGap, now - R.last);
    R.last = now;
    R.ticks++;
  }, 1000);
})();
