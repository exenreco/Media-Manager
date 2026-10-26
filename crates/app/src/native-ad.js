// Only geometry and privacy commands cross the bridge. Ad assets and clicks
// stay entirely in Google's platform NativeAdView.
(() => {
  window.mediaManagerAdCleanup?.();
  const bridge = window.MediaManagerAds || window.webkit?.messageHandlers?.MediaManagerAds;
  const slot = document.getElementById('native-ad-slot');
  if (!bridge || !slot) { if (slot) slot.hidden = true; return; }
  let previous = '', stopped = false, frame;
  const send = data => bridge.postMessage(JSON.stringify(data));
  function update() {
    if (stopped) return;
    const r = slot.getBoundingClientRect();
    let visible = !document.hidden && !slot.hidden && !document.querySelector('.overlay, .panel-menu') &&
      r.width >= 240 && r.height >= 440 && r.top >= 0 && r.left >= 0 &&
      r.bottom <= innerHeight && r.right <= innerWidth;
    // Hide the native view rather than covering controls or clipping AdChoices
    // when the settings panel scrolls or a modal opens.
    for (let p = slot.parentElement; visible && p; p = p.parentElement) {
      const style = getComputedStyle(p);
      if (/(auto|scroll|hidden|clip)/.test(style.overflow + style.overflowY + style.overflowX)) {
        const b = p.getBoundingClientRect();
        visible = r.top >= b.top && r.bottom <= b.bottom && r.left >= b.left && r.right <= b.right;
      }
    }
    const message = JSON.stringify({type:'bounds', visible, x:r.x, y:r.y, width:r.width, height:r.height,
      viewportWidth:innerWidth});
    if (message !== previous) { bridge.postMessage(message); previous = message; }
    frame = requestAnimationFrame(update);
  }
  const privacy = e => { if (e.target.closest('#ad-privacy')) send({type:'privacy'}); };
  const hide = () => send({type:'hide'});
  document.addEventListener('click', privacy);
  window.addEventListener('pagehide', hide);
  window.mediaManagerAdCleanup = () => {
    stopped = true; cancelAnimationFrame(frame); hide();
    document.removeEventListener('click', privacy);
    window.removeEventListener('pagehide', hide);
  };
  update();
})();
