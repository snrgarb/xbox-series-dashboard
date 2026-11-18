// The voice steps, before a press: Discord's button WHAT (declared before this, with CID, a voice channel's id or
// null) found, marked with data-xsd-target (so DevTools' DOM.querySelector finds that one element to focus),
// scrolled into view, and the point to click. Kinds, states and booleans only.
(() => {
  const all = (s, root) => { try { return [...(root || document).querySelectorAll(s)]; } catch (e) { return []; } };
  for (const old of all("[data-xsd-target]")) old.removeAttribute("data-xsd-target");
  const panels = all('section[aria-label="User area"]')[0] || all('[class*="panels_"]')[0] || null;
  const label = (b) => b.getAttribute("aria-label") || "";
  const byLabel = (re, root) => all('button, [role="button"], [role="switch"]', root).find((b) => re.test(label(b)));
  const ring = all('[class*="incomingCall" i]')[0] || all('[class*="ringing" i]')[0] || all('[aria-label*="incoming call" i]')[0] || null;
  const inRing = (re) => ring ? all('button, [role="button"]', ring).find((b) => re.test(label(b) || b.textContent || "")) : null;
  const header = all('section[class*="title_"]')[0] || all('[class*="toolbar_"]')[0] || null;
  let el = null;
  switch (WHAT) {
    case "mute": el = byLabel(/^(mute|unmute)$/i, panels) || byLabel(/^(mute|unmute)$/i); break;
    case "deafen": el = byLabel(/^(deafen|undeafen)$/i, panels) || byLabel(/^(deafen|undeafen)$/i); break;
    case "disconnect": el = byLabel(/^disconnect$/i, panels) || byLabel(/^disconnect$/i); break;
    case "accept": el = inRing(/accept|answer|join call/i); break;
    case "decline": el = inRing(/decline|ignore|reject/i); break;
    case "call": el = byLabel(/^start (a )?voice call$/i, header) || byLabel(/^start (a )?voice call$/i); break;
    case "join_voice": el = all("button").find((b) => /^join voice$/i.test((b.textContent || "").trim()) || /^join voice$/i.test(label(b))) || null; break;
    case "channel": el = CID ? all(`[data-list-item-id="channels___${CID}"]`)[0] || null : null; break;
  }
  if (!el) return { found: false, ring: !!ring };
  el.setAttribute("data-xsd-target", "1");
  el.scrollIntoView({ block: "center" });
  const r = el.getBoundingClientRect();
  const x = r.left + r.width / 2, y = r.top + r.height / 2;
  const top = document.elementFromPoint(x, y);
  return {
    found: true,
    x, y,
    w: Math.round(r.width), h: Math.round(r.height),
    covered: !(top && (el === top || el.contains(top))),
    checked: el.getAttribute("aria-checked"),
    tag: el.tagName.toLowerCase() + (el.getAttribute("role") ? `[role=${el.getAttribute("role")}]` : ""),
  };
})()
