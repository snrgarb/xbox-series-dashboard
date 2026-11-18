// How the page stands, as flags (#197, docs/discord-web.md): run through ExecuteScript about once a second, and a
// few times a second while the sign-in page is up. Returns a route's kind and booleans only: never a value typed, a
// name, an address, a cookie or the page's text. The signs are the ones the spike (#196) found on discord.com.
(() => {
  const q = (s) => { try { return document.querySelector(s); } catch (e) { return null; } };
  const path = location.pathname;
  const active = document.activeElement;
  const textInput = (el) =>
    el && el.tagName === "INPUT" &&
    !["checkbox", "radio", "button", "submit", "reset", "file", "image", "range", "color", "hidden"].includes(el.type);
  const editable = !!active && (active.isContentEditable || textInput(active) || active.tagName === "TEXTAREA");
  return {
    login: /^\/(login|register)/.test(path) || (!!q('input[type="password"]') && !q('[data-list-id="guildsnav"]')),
    // Signed in on any of the app's routes (Discord went /login -> /app -> /quest-home on the TV): no login or
    // register route, no password field, and the app's shell, its server bar, shown.
    app:
      !/^\/(login|register)/.test(path) &&
      !q('input[type="password"]') &&
      !!q('[data-list-id="guildsnav"], nav[class*="guilds" i]'),
    field: editable ? (active.type === "password" ? "password" : "text") : "",
    ready: document.readyState === "complete",
  };
})()
