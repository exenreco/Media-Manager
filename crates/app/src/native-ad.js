// Only geometry, lifecycle and privacy commands cross the bridge.
// Google's native view owns every ad asset, click and impression.
(() => {
  window.mediaManagerAdCleanup?.();
  let bridge, slot, previous = '', stopped = false, frame, waiting = false, attempts = 0;
  const status = (message, retry = false) => {
    document.querySelectorAll('.native-ad-status').forEach(label => { label.textContent = message; label.hidden = !message; });
    document.querySelectorAll('.ad-retry').forEach(button => { button.hidden = !retry; });
  };
  window.mediaManagerAdStatus = status;
  const slots = () => [...document.querySelectorAll('.native-ad-slot')];
  window.mediaManagerAdSetHidden = hidden => slots().forEach(slot => { slot.hidden = hidden; });
  const geometry = slot => {
    const r = slot.getBoundingClientRect();
    let eligible = !document.hidden && !slot.hidden && !document.querySelector('.overlay') && !document.querySelector('main[data-preview-sheet]') && !document.querySelector('.ordering-dialog[open]') && !document.querySelector('.mobile-dock.expanded') && !document.querySelector('.connection-island[data-state=warning]') && !document.querySelector('.connection-island[data-state=match]') &&
      r.width >= 240 && r.height >= 379 && r.bottom > 0 && r.top < innerHeight;
    let visible = eligible && r.top >= -1 && r.left >= -1 && r.bottom <= innerHeight + 1 && r.right <= innerWidth + 1;
    const dock = document.querySelector('.mobile-dock');
    if (dock && visible && getComputedStyle(dock).display !== 'none') {
      const bounds = dock.getBoundingClientRect();
      visible = r.bottom <= bounds.top || r.top >= bounds.bottom || r.right <= bounds.left || r.left >= bounds.right;
    }
    for (let p = slot.parentElement; p; p = p.parentElement) {
      const style = getComputedStyle(p);
      if (style.display === 'none' || style.visibility === 'hidden') { eligible = false; visible = false; }
      if (/(auto|scroll|hidden|clip)/.test(style.overflow + style.overflowY + style.overflowX)) {
        const b = p.getBoundingClientRect();
        eligible = eligible && r.bottom > b.top && r.top < b.bottom && b.bottom - b.top >= 379;
        visible = visible && r.top >= b.top - 1 && r.bottom <= b.bottom + 1 && r.left >= b.left - 1 && r.right <= b.right + 1;
      }
    }
    return {r, eligible, visible};
  };
  const send = data => bridge?.postMessage(JSON.stringify(data));
  function update() {
    if (stopped) return;
    bridge = window.MediaManagerAds || window.webkit?.messageHandlers?.MediaManagerAds;
    const candidates = slots();
    const height = Math.min(440, Math.max(380, innerHeight - 220));
    candidates.forEach(item => { item.style.height = `${height}px`; });
    // A single native SDK view follows the visible placement; never duplicate an ad impression.
    slot = candidates.find(item => geometry(item).visible) || candidates.find(item => geometry(item).eligible) || candidates[0];
    if (!bridge || !slot) {
      waiting = true;
      if (++attempts >= 120) {
        window.mediaManagerAdSetHidden(true);
        status('Advertising is unavailable in this build or WebView.', true);
        return;
      }
      frame = requestAnimationFrame(update); return;
    }
    if (waiting) { window.mediaManagerAdSetHidden(false); waiting = false; status('Waiting for an advertisement…'); }
    const {r, eligible, visible} = geometry(slot);
    const message = JSON.stringify({type:'bounds', placement:slot.id, eligible, visible, x:r.x, y:r.y, width:r.width, height:r.height, viewportWidth:innerWidth});
    if (message !== previous) { bridge.postMessage(message); previous = message; }
    frame = requestAnimationFrame(update);
  }
  const action = e => {
    if (e.target.closest('.ad-privacy')) send({type:'privacy'});
    if (e.target.closest('.ad-retry')) {
      attempts = 0; previous = ''; window.mediaManagerAdSetHidden(false);
      status('Retrying advertisement…');
      cancelAnimationFrame(frame);
      if (bridge) { update(); send({type:'retry'}); }
      else { cancelAnimationFrame(frame); update(); }
    }
  };
  const hide = () => send({type:'hide'});
  document.addEventListener('click', action);
  window.addEventListener('pagehide', hide);
  window.mediaManagerAdCleanup = () => {
    stopped = true; cancelAnimationFrame(frame); hide();
    document.removeEventListener('click', action);
    window.removeEventListener('pagehide', hide);
    delete window.mediaManagerAdStatus;
    delete window.mediaManagerAdSetHidden;
  };
  update();
})();
