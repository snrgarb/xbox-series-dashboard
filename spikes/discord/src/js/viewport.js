// --send: the page's viewport as a hidden web view gives it, and the row of HREF (declared before this): its rect,
// and whether elementFromPoint at its centre finds it. Numbers and booleans only.
(() => {
  const row = [...document.querySelectorAll(`a[href="${HREF}"]`)].find((a) => !a.closest('[data-list-id="guildsnav"]'));
  const r = row ? row.getBoundingClientRect() : { width: 0, height: 0, left: 0, top: 0 };
  const top = row ? document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2) : null;
  return {
    w: innerWidth,
    h: innerHeight,
    visibility: document.visibilityState,
    rw: Math.round(r.width),
    rh: Math.round(r.height),
    rx: Math.round(r.left),
    ry: Math.round(r.top),
    hit: !!(row && top && row.contains(top)),
  };
})()
