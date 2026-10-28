// Native SDK callbacks are the only source of reward entitlement.
(() => {
  window.mediaManagerFullscreenCleanup?.();
  let bridge, timer, stopped = false, initialized = false;
  let state = {adFreeUntil:0, presenting:false, rewardedReady:false, message:''};
  const blocked = () => window.mediaManagerNativePlaybackActive || document.hidden || !!document.querySelector('.overlay, dialog[open], main[data-preview-sheet], main[data-menu-sheet], .mobile-dock.expanded, [aria-busy="true"]') ||
    [...document.querySelectorAll('video')].some(video => !video.paused && !video.ended);
  const send = type => bridge?.postMessage(JSON.stringify({type}));
  const render = () => {
    const free = state.adFreeUntil > Date.now();
    window.mediaManagerAdsSuppressed = free || state.presenting;
    document.documentElement.dataset.adsPaused = String(free);
    document.querySelectorAll('.ad-privacy').forEach(button => { button.hidden = !state.privacyRequired; });
    document.querySelectorAll('.ad-rewarded').forEach(button => {
      button.disabled = free || state.presenting;
      button.textContent = free ? 'Ad-free time is active' : 'Watch an ad — 30 minutes without ads';
    });
    const message = free ? `Ads paused for ${Math.ceil((state.adFreeUntil-Date.now())/60000)} more minutes on this device.` :
      state.message || (state.rewardedReady ? 'Rewarded ad ready. Watching is optional.' : 'A rewarded ad is not ready yet. Tap to check availability.');
    document.querySelectorAll('.fullscreen-ad-status').forEach(label => { label.textContent = message; });
  };
  window.mediaManagerFullscreenState = value => { state = value; render(); };
  window.mediaManagerRenameAd = () => {
    // Never defer this opportunity: a late ad would interrupt a different task.
    requestAnimationFrame(() => { if (!blocked() && !window.mediaManagerAdsSuppressed) send('interstitial'); });
  };
  const action = event => {
    if (event.target.closest('.ad-rewarded') && !blocked() && !window.mediaManagerAdsSuppressed) send('rewarded');
  };
  const tick = () => {
    if (stopped) return;
    bridge = window.MediaManagerAds || window.webkit?.messageHandlers?.MediaManagerAds;
    if (bridge && !initialized) { initialized = true; send('fullscreen-state'); }
    render();
    timer = setTimeout(tick, 1000);
  };
  const resume = () => { if (!document.hidden) send('fullscreen-state'); };
  document.addEventListener('click',action);
  document.addEventListener('visibilitychange',resume);
  window.mediaManagerFullscreenCleanup = () => {
    stopped = true; clearTimeout(timer);
    document.removeEventListener('click',action);
    document.removeEventListener('visibilitychange',resume);
    delete window.mediaManagerFullscreenState;
    delete window.mediaManagerRenameAd;
    delete window.mediaManagerAdsSuppressed;
    delete document.documentElement.dataset.adsPaused;
  };
  tick();
})();
