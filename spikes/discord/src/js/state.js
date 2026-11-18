// What the page shows, as flags: run every two seconds by --sign-in (and once by the others) through ExecuteScript.
// Returns booleans and kinds only: never a value typed, a name, an address or the page's text.
(() => {
  const q = (s) => { try { return document.querySelector(s); } catch (e) { return null; } };
  const text = ((document.body && document.body.innerText) || "").slice(0, 20000);
  const path = location.pathname;
  const frameHosts = [...document.querySelectorAll("iframe")].map((f) => {
    try { return new URL(f.src, location.href).hostname; } catch (e) { return ""; }
  });
  const active = document.activeElement;
  const textInput = (el) =>
    el && el.tagName === "INPUT" && !["checkbox", "radio", "button", "submit", "reset", "file", "image", "range", "color"].includes(el.type);
  const editable = !!active && (active.isContentEditable || textInput(active) || active.tagName === "TEXTAREA");
  return {
    route: path.startsWith("/channels/") ? "app" : path.startsWith("/login") ? "login" : path.startsWith("/register") ? "register" : "other",
    login_form: !!q('input[name="email"], input[name="login"], input[type="email"]') && !!q('input[type="password"]'),
    password_field: !!q('input[type="password"]'),
    captcha: frameHosts.some((h) => h.includes("hcaptcha")) || !!q('[class*="captcha" i]'),
    captcha_frames: frameHosts.filter((h) => h.includes("hcaptcha")).length,
    two_factor: !!q('input[autocomplete="one-time-code"]') || /two-factor|authentication code|2fa|backup code/i.test(text),
    qr: !!q('[class*="qrCode" i], [class*="qr-code" i], [aria-label*="QR" i]'),
    new_location: /new login location|new location|verify (your )?(login|e-?mail)|check your e-?mail/i.test(text),
    app: path.startsWith("/channels/") && !!q('[data-list-id="guildsnav"], nav[class*="guilds" i]'),
    editable: editable,
    editable_kind: editable ? (active.isContentEditable ? "contenteditable" : (active.type || active.tagName).toLowerCase()) : "",
    ready: document.readyState,
    frames: frameHosts.length,
  };
})()
