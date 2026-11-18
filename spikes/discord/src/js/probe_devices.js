// --voice-probe, first: the microphone as this hidden page gets it. Devices counted (labels show only once the
// microphone is allowed), the permission's state, getUserMedia({audio:true}) with what the track reports, devices
// again. The stream is kept (window.__xsdProbe) for js/probe_audio.js and stopped by it. Counts, states and
// booleans only: never a device's name or id. Every wait is capped.
(async () => {
  const within = (ms, p) => Promise.race([p, new Promise((r) => setTimeout(() => r("timeout"), ms))]);
  const devices = async () => {
    const list = await within(5000, navigator.mediaDevices.enumerateDevices().catch((e) => "error " + e.name));
    if (!Array.isArray(list)) return { error: String(list) };
    const kind = (k) => list.filter((d) => d.kind === k);
    return {
      inputs: kind("audioinput").length,
      outputs: kind("audiooutput").length,
      cameras: kind("videoinput").length,
      labels: list.some((d) => d.label.length > 0),
      defaultInput: kind("audioinput").some((d) => d.deviceId === "default"),
      communicationsInput: kind("audioinput").some((d) => d.deviceId === "communications"),
    };
  };
  const out = { visibility: document.visibilityState, focus: document.hasFocus() };
  out.before = await devices();
  try {
    const p = await within(3000, navigator.permissions.query({ name: "microphone" }));
    out.permission = p === "timeout" ? "timeout" : p.state;
  } catch (e) { out.permission = "error " + e.name; }
  const started = performance.now();
  try {
    const stream = await within(10000, navigator.mediaDevices.getUserMedia({ audio: true }));
    out.gumMs = Math.round(performance.now() - started);
    if (stream === "timeout") {
      out.gum = "timeout";
    } else {
      const track = stream.getAudioTracks()[0];
      const s = track ? track.getSettings() : {};
      out.gum = "ok";
      out.track = {
        count: stream.getAudioTracks().length,
        readyState: track && track.readyState,
        enabled: track && track.enabled,
        muted: track && track.muted,
        sampleRate: s.sampleRate || null,
        channelCount: s.channelCount || null,
        echoCancellation: s.echoCancellation,
        noiseSuppression: s.noiseSuppression,
        autoGainControl: s.autoGainControl,
      };
      window.__xsdProbe = { stream };
    }
  } catch (e) {
    out.gumMs = Math.round(performance.now() - started);
    out.gum = "error " + e.name;
  }
  out.after = await devices();
  out.sinkMedia = "setSinkId" in HTMLMediaElement.prototype;
  out.sinkContext = typeof AudioContext !== "undefined" && "setSinkId" in AudioContext.prototype;
  return out;
})()
