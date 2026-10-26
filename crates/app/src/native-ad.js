// Only geometry, lifecycle and privacy commands cross the bridge.
// Google's native view owns every ad asset, click and impression.
(() => {
  window.mediaManagerAdCleanup?.();
  let bridge, slot, previous = '', stopped = false, frame, waiting = false, attempts = 0;
  const status = (message, retry = false) => {
    const label = document.getElementById('native-ad-status');
    if (label) { label.textContent = message; label.hidden = !message; }
    const button = document.getElementById('ad-retry');
    if (button) button.hidden = !retry;
  };
  window.mediaManagerAdStatus = status;
  const send = data => bridge?.postMessage(JSON.stringify(data));
  function update() {
    if (stopped) return;
    bridge = window.MediaManagerAds || window.webkit?.messageHandlers?.MediaManagerAds;
    slot = document.getElementById('native-ad-slot');
    if (!bridge || !slot) {
      waiting = true;
      if (++attempts >= 120) {
        if (slot) slot.hidden = true;
        status('Advertising is unavailable in this build or WebView.', true);
        return;
      }
      frame = requestAnimationFrame(update); return;
    }
    if (waiting) { slot.hidden = false; waiting = false; status('Waiting for an advertisement…'); }
    // A compact native layout fits smaller phones without clipping its media or AdChoices.
    const height = Math.min(440, Math.max(380, innerHeight - 220));
    slot.style.height = `${height}px`;
    const r = slot.getBoundingClientRect();
    let eligible = !document.hidden && !slot.hidden && !document.querySelector('.overlay') &&
      r.width >= 240 && r.height >= 379 && r.bottom > 0 && r.top < innerHeight;
    let visible = eligible && r.top >= -1 && r.left >= -1 && r.bottom <= innerHeight + 1 && r.right <= innerWidth + 1;
    for (let p = slot.parentElement; p; p = p.parentElement) {
      const style = getComputedStyle(p);
      if (style.display === 'none' || style.visibility === 'hidden') { eligible = false; visible = false; }
      if (/(auto|scroll|hidden|clip)/.test(style.overflow + style.overflowY + style.overflowX)) {
        const b = p.getBoundingClientRect();
        eligible = eligible && r.bottom > b.top && r.top < b.bottom && b.bottom - b.top >= 379;
        visible = visible && r.top >= b.top - 1 && r.bottom <= b.bottom + 1 && r.left >= b.left - 1 && r.right <= b.right + 1;
      }
    }
    const message = JSON.stringify({type:'bounds', eligible, visible, x:r.x, y:r.y, width:r.width, height:r.height, viewportWidth:innerWidth});
    if (message !== previous) { bridge.postMessage(message); previous = message; }
    frame = requestAnimationFrame(update);
  }
  const action = e => {
    if (e.target.closest('#ad-privacy')) send({type:'privacy'});
    if (e.target.closest('#ad-retry')) {
      attempts = 0; previous = ''; if (slot) slot.hidden = false;
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
  };
  update();
})();
