// --send, last: whether the newest message is TEXT (declared before this), and whether Discord still marks it as
// sending or as failed. Booleans and counts only.
(() => {
  const norm = (s) => (s || "").replace(/\s+/g, " ").trim();
  const items = [...document.querySelectorAll('li[id^="chat-messages-"]')];
  const last = items[items.length - 1];
  const content = last && last.querySelector('[id^="message-content-"]');
  const classes = last ? [last.className, ...[...last.querySelectorAll("[class]")].slice(0, 12).map((e) => e.className)].join(" ") : "";
  const box = document.querySelector('[role="textbox"][data-slate-editor="true"]') || document.querySelector('div[role="textbox"][contenteditable="true"]');
  const time = last && last.querySelector("time[datetime]");
  return {
    messages: items.length,
    last_age_ms: time ? Date.now() - Date.parse(time.getAttribute("datetime")) : null,
    last_is_text: !!content && norm(content.textContent) === norm(TEXT),
    sending: /isSending|sending_/i.test(classes),
    failed: /isFailed|failed_/i.test(classes),
    box_empty: !!box && (box.textContent || "").replace(/​|﻿/g, "").trim().length === 0,
  };
})()
