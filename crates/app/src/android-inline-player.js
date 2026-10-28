// Geometry only. Android owns decoding, controls, and bounded media streaming.
(() => {
  window.mediaManagerInlineCleanup?.();
  let previous = '', frame, stopped = false;
  const send = value => window.MediaManagerStorage?.postMessage(JSON.stringify({op:'inlineMediaBounds', ...value}));
  const refresh = () => { previous = ''; };
  function update() {
    if (stopped) return;
    const slot = document.querySelector('.android-inline-preview[data-native-source]');
    const r = slot?.getBoundingClientRect();
    let visible = !!r && !document.hidden && r.width >= 160 && r.height >= 90 &&
      r.left >= 0 && r.top >= 0 && r.right <= innerWidth + 1 && r.bottom <= innerHeight + 1 &&
      !document.querySelector('.overlay,dialog[open],.mobile-dock.expanded,.connection-island[data-state=warning],.connection-island[data-state=match]');
    for (let p = slot; p; p = p.parentElement) {
      const style = getComputedStyle(p);
      if (style.display === 'none' || style.visibility === 'hidden') visible = false;
      if (/(auto|scroll|hidden|clip)/.test(style.overflow + style.overflowY + style.overflowX)) {
        const b = p.getBoundingClientRect();
        visible = visible && r.top >= b.top - 1 && r.bottom <= b.bottom + 1 && r.left >= b.left - 1 && r.right <= b.right + 1;
      }
    }
    const dock = document.querySelector('.mobile-dock');
    if (visible && dock && getComputedStyle(dock).display !== 'none') {
      const b = dock.getBoundingClientRect();
      visible = r.bottom <= b.top || r.top >= b.bottom || r.right <= b.left || r.left >= b.right;
    }
    // Also suppress interstitials while a native preview is visible.
    window.mediaManagerNativePlaybackActive = visible;
    const value = {source:slot?.getAttribute('data-native-source') || '', visible,
      x:r?.x || 0, y:r?.y || 0, width:r?.width || 0, height:r?.height || 0, viewportWidth:innerWidth};
    const key = JSON.stringify(value);
    if (key !== previous && window.MediaManagerStorage) { send(value); previous = key; }
    frame = requestAnimationFrame(update);
  }
  window.mediaManagerInlineRefresh = refresh;
  document.addEventListener('visibilitychange', refresh);
  window.addEventListener('resize', refresh);
  window.mediaManagerInlineCleanup = () => {
    stopped = true; cancelAnimationFrame(frame); send({source:'',visible:false});
    document.removeEventListener('visibilitychange', refresh);
    window.removeEventListener('resize', refresh);
    delete window.mediaManagerInlineRefresh;
    window.mediaManagerNativePlaybackActive = false;
  };
  update();
})();
