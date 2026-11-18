// --voice-probe: an AudioContext of the spike's own in the hidden page, playing a silent tone (gain 0: nothing is
// heard) with the microphone's stream (js/probe_devices.js) into an analyser. STAGE (declared before this):
// "start" makes it and tries resume(); "resume" tries resume() again (run with a user gesture); "sample" says its
// state, its clock, the 1 s timer of js/voice_hooks.js and the microphone's level now; "end" stops it all. Numbers
// and states only. Every wait is capped.
(async () => {
  const within = (ms, p) => Promise.race([p, new Promise((r) => setTimeout(() => r("timeout"), ms))]);
  const P = window.__xsdProbe || (window.__xsdProbe = {});
  const R = window.__xsdVoice || { ticks: 0, maxGap: 0 };
  if (STAGE === "start") {
    if (P.ctx) return { state: P.ctx.state, again: true };
    const ctx = new AudioContext();
    P.ctx = ctx;
    const born = ctx.state;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    gain.gain.value = 0;
    osc.connect(gain).connect(ctx.destination);
    osc.start();
    if (P.stream) {
      P.analyser = ctx.createAnalyser();
      ctx.createMediaStreamSource(P.stream).connect(P.analyser);
    }
    const resumed = await within(2000, ctx.resume().then(() => "ok", (e) => "error " + e.name));
    return { born, resumed, state: ctx.state, sampleRate: ctx.sampleRate, baseLatency: ctx.baseLatency || 0, mic: !!P.analyser };
  }
  if (!P.ctx) return { state: "none" };
  if (STAGE === "resume") {
    const resumed = await within(2000, P.ctx.resume().then(() => "ok", (e) => "error " + e.name));
    return { resumed, state: P.ctx.state };
  }
  if (STAGE === "sample") {
    let level = 0;
    if (P.analyser) {
      const buf = new Float32Array(P.analyser.fftSize);
      P.analyser.getFloatTimeDomainData(buf);
      level = Math.sqrt(buf.reduce((s, v) => s + v * v, 0) / buf.length);
    }
    const track = P.stream && P.stream.getAudioTracks()[0];
    const timer = { ticks: R.ticks, maxGap: Math.round(R.maxGap) };
    R.ticks = 0;
    R.maxGap = 0;
    return {
      state: P.ctx.state,
      time: P.ctx.currentTime,
      wall: performance.now() / 1000,
      level,
      track: track ? track.readyState : "none",
      muted: track ? track.muted : null,
      visibility: document.visibilityState,
      focus: document.hasFocus(),
      timer,
    };
  }
  if (STAGE === "end") {
    if (P.stream) for (const t of P.stream.getTracks()) t.stop();
    const closed = await within(2000, P.ctx.close().then(() => "ok", (e) => "error " + e.name));
    const state = P.ctx.state;
    window.__xsdProbe = {};
    return { closed, state };
  }
  return { state: "unknown stage" };
})()
