// Transactions stay native. Web content requests store actions, never ownership.
(() => {
  window.mediaManagerBillingCleanup?.();
  let stopped = false, initialized = false, timer, bridge;
  const ios = () => !!window.webkit?.messageHandlers?.MediaManagerAds && !window.MediaManagerAds;
  let state = {owned:false, busy:true, pending:false, available:false, price:'', message:ios() ? 'Checking the App Store…' : 'Checking Google Play…'};
  const restoreExpired = () => state.restoreUntil > 0 && Date.now() >= state.restoreUntil;
  const restoreAllowed = () => !restoreExpired() && (!ios() || (state.owned && state.canRestore && state.restoreUntil > Date.now()));
  const send = type => bridge?.postMessage(JSON.stringify({type}));
  const render = () => {
    window.mediaManagerPermanentAdFree = !!state.owned;
    window.mediaManagerBillingPresenting = !!state.presenting;
    window.mediaManagerFullscreenRefresh?.();
    document.querySelectorAll('.billing-buy').forEach(button => {
      button.disabled = state.owned || state.busy || state.pending || !state.available;
      button.textContent = state.owned ? 'Ads removed' : state.pending ? 'Payment pending' : state.price ? `Remove ads — ${state.price}` : 'Remove ads';
    });
    document.querySelectorAll('.billing-restore').forEach(button => { button.disabled = !bridge || state.busy || !restoreAllowed(); button.title = restoreExpired() ? 'The 10-minute restore window has ended. Purchase status still updates automatically.' : ios() && !state.owned ? 'No verified purchase found.' : ''; });
    document.querySelectorAll('.billing-status').forEach(label => { label.textContent = state.message || (state.owned ? 'All ads removed.' : 'One-time purchase. No subscription.'); });
  };
  window.mediaManagerBillingState = value => { state = value; render(); };
  const click = event => {
    if (document.hidden || state.busy || document.querySelector('dialog[open], .overlay') || window.mediaManagerNativePlaybackActive) return;
    if (event.target.closest('.billing-buy') && !state.owned && !state.pending && state.available) send('billing-buy');
    if (event.target.closest('.billing-restore') && restoreAllowed()) send('billing-restore');
  };
  const resume = () => { if (!document.hidden) send('billing-state'); };
  const tick = () => {
    if (stopped) return;
    bridge = window.MediaManagerAds || window.webkit?.messageHandlers?.MediaManagerAds;
    if (bridge && !initialized) { initialized = true; send('billing-state'); }
    render(); timer = setTimeout(tick, 1000);
  };
  document.addEventListener('click', click);
  document.addEventListener('visibilitychange', resume);
  window.mediaManagerBillingCleanup = () => {
    stopped = true; clearTimeout(timer);
    document.removeEventListener('click', click);
    document.removeEventListener('visibilitychange', resume);
    delete window.mediaManagerBillingState;
    delete window.mediaManagerPermanentAdFree;
    delete window.mediaManagerBillingPresenting;
    window.mediaManagerFullscreenRefresh?.();
  };
  tick();
})();
