// --send, second: once HREF (declared before this) is the page's route, the message box and the point to click to
// put the caret in it, and how many messages the conversation shows now. Lengths and counts only.
(() => {
  const box = document.querySelector('[role="textbox"][data-slate-editor="true"]') || document.querySelector('div[role="textbox"][contenteditable="true"]');
  const messages = document.querySelectorAll('li[id^="chat-messages-"]').length;
  if (location.pathname !== HREF || !box) return { opened: location.pathname === HREF, box: !!box, messages };
  const r = box.getBoundingClientRect();
  return {
    opened: true,
    box: true,
    messages,
    x: r.left + Math.min(40, r.width / 2),
    y: r.top + r.height / 2,
    focused: document.activeElement === box || box.contains(document.activeElement),
    empty: (box.textContent || "").replace(/​|﻿/g, "").trim().length === 0,
  };
})()
