// --dump-shape, and the `shape` control file while holding: the page's SHAPE where the voice selectors look, for
// the lead to read, as text. Per element: tag, role, data-list-item-id with its digits as #, its classes (Discord's
// hashed class names: not personal), aria-label and title with every word outside a fixed list of Discord's UI words
// written x and digits #, aria-checked/expanded/hidden, an svg path's `d` as a hash, and a text node's length only.
// Never a name, a message or an id.
(() => {
  const WORDS = new Set(("voice channel channels stage text users user limit mute muted unmute deafen deafened undeafen " +
    "connected connecting disconnect disconnected speaking call calls video camera screen share stream streaming live " +
    "start join leave end accept decline ignore answer incoming ringing ring settings server servers direct message " +
    "messages unread mention mentions category announcement announcements forum thread threads rules event events " +
    "activity activities soundboard noise suppression invite create open close collapse expand folder mic microphone " +
    "headset output input of in the a an and with to for on off new private locked full member members online idle do " +
    "not disturb inbox help friends nitro home add more options menu panel area account status rtc ping connection " +
    "quality packet loss avatar username profile game playing user's area channel's is are no none here now enter " +
    "pip popout focus show hide hidden list button toggle sound effects quit you your me tap click").split(/\s+/));
  const all = (s, root) => { try { return [...(root || document).querySelectorAll(s)]; } catch (e) { return []; } };
  const words = (s) =>
    (s || "").replace(/[0-9]+/g, "#").replace(/[\p{L}']+/gu, (w) => (WORDS.has(w.toLowerCase()) ? w : "x"));
  const hash = (s) => {
    let h = 5381;
    for (let i = 0; i < s.length; i++) h = ((h * 33) ^ s.charCodeAt(i)) >>> 0;
    return h.toString(16);
  };
  const lines = [];
  const node = (el, depth, budget) => {
    if (budget.n <= 0) return;
    budget.n--;
    const pad = "  ".repeat(depth);
    const a = [el.tagName.toLowerCase()];
    const role = el.getAttribute("role");
    if (role) a.push(`role=${role}`);
    const lid = el.getAttribute("data-list-item-id");
    if (lid) a.push(`list-item-id=${lid.replace(/[0-9]+/g, "#")}`);
    const listId = el.getAttribute("data-list-id");
    if (listId) a.push(`list-id=${listId.replace(/[0-9]+/g, "#")}`);
    if (el.hasAttribute("data-dnd-name")) a.push("data-dnd-name=(a name)");
    const href = el.getAttribute("href");
    if (href) a.push(`href=${href.replace(/[0-9]+/g, "#")}`);
    const cls = typeof el.className === "string" ? el.className : el.getAttribute("class") || "";
    if (cls.trim()) a.push(`class="${cls.trim()}"`);
    for (const k of ["aria-label", "title", "aria-checked", "aria-expanded", "aria-hidden", "aria-selected", "aria-describedby"]) {
      const v = el.getAttribute(k);
      if (v !== null) a.push(`${k}="${k.startsWith("aria-label") || k === "title" ? words(v) : v.replace(/[0-9]+/g, "#")}"`);
    }
    if (el.tagName.toLowerCase() === "path" && el.getAttribute("d")) a.push(`d#${hash(el.getAttribute("d"))}`);
    const texts = [...el.childNodes].filter((c) => c.nodeType === 3 && c.textContent.trim()).map((c) => c.textContent.trim());
    if (texts.length) a.push(`text(${texts.map((t) => WORDS.has(t.toLowerCase()) || /^[\p{L} ]+$/u.test(t) && t.split(/\s+/).every((w) => WORDS.has(w.toLowerCase())) ? JSON.stringify(t) : `${t.length} chars`).join(", ")})`);
    lines.push(pad + a.join(" "));
    for (const c of el.children) node(c, depth + 1, budget);
  };
  const section = (title, els, perEl, total) => {
    lines.push(`== ${title}: ${els.length} found`);
    let left = total;
    els.forEach((el, i) => {
      if (left <= 0) return;
      lines.push(`-- ${title} ${i + 1}`);
      const budget = { n: Math.min(perEl, left) };
      const start = budget.n;
      node(el, 1, budget);
      left -= start - budget.n;
    });
  };
  lines.push(`route kind: ${location.pathname.replace(/[0-9]+/g, "#")}; visibility ${document.visibilityState}; focus ${document.hasFocus()}`);
  // The channel list: each row's own item and its list item (the users in a voice channel may be in it, or after it).
  const rows = all('[data-list-item-id^="channels___"]');
  const boxes = [...new Set(rows.map((r) => r.closest("li") || r.parentElement))];
  section("channel rows (their list items)", boxes, 40, 1500);
  const after = boxes.map((b) => b.nextElementSibling).filter((n) => n && !n.querySelector('[data-list-item-id^="channels___"]'));
  section("what follows a channel's list item, when not another channel", after, 30, 400);
  section("user area", all('section[aria-label="User area"]').concat(all('[class*="panels_"]')).slice(0, 2), 250, 400);
  section("voice status", all('[class*="rtcConnectionStatus"]').slice(0, 2), 30, 60);
  section("voice users anywhere", all('[class*="voiceUser"]').slice(0, 5), 30, 150);
  section("speaking anywhere", all('[class*="peaking"]').slice(0, 10), 10, 100);
  section("call area", all('[class*="callContainer"], [class*="voiceCallWrapper"], [class*="callWrapper"], [class*="videoGrid"]').slice(0, 2), 250, 400);
  section("tiles", all('[class*="tile"]').slice(0, 6), 25, 150);
  section("ring", all('[class*="incoming" i], [class*="ringing" i], [aria-label*="incoming" i]').slice(0, 3), 60, 150);
  section("main area's top", all("main, [class*='chatContent'], [class*='chat_']").slice(0, 1), 60, 60);
  return lines.join("\n") + "\n";
})()
