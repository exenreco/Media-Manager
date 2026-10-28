// One controller for both artwork and video. Selection stays in Rust; dismissal
// reaches Rust only after the sheet has finished leaving the screen.
(() => {
  window.mediaManagerPreviewSheet?.cleanup();
  const main = document.querySelector('main');
  const panel = document.getElementById('media-inspector-sidebar');
  if (!main || !panel) return;
  const compactQuery = matchMedia('(max-width:1050px), (max-height:650px)');
  const reduced = matchMedia('(prefers-reduced-motion:reduce)');
  const grip = panel.querySelector('.preview-sheet-grip');
  const toolbar = panel.querySelector('.preview-sheet-toolbar');
  let compact = false, visible = false, closing = false, stopped = false;
  let frame = 0, timer = 0, drag = null, restoreFocus = null, trigger = null;
  const inert = new Map();
  const listeners = [];
  const listen = (node, type, callback, options) => {
    node.addEventListener(type, callback, options);
    listeners.push(() => node.removeEventListener(type, callback, options));
  };
  const lock = () => {
    for (const node of [...main.children, ...panel.parentElement.children]) {
      if (node === panel || node.contains(panel) || node.classList.contains('preview-sheet-backdrop')) continue;
      if (!inert.has(node)) inert.set(node, node.hasAttribute('inert'));
      node.setAttribute('inert', '');
    }
  };
  const unlock = () => {
    for (const [node, previous] of inert) if (!previous) node.removeAttribute('inert');
    inert.clear();
  };
  const focusable = () => [...panel.querySelectorAll('button,input,select,textarea,a[href],video[controls],[tabindex="0"]')]
    .filter(node => !node.disabled && node.getClientRects().length && !node.closest('[inert]'));
  const reset = (restore = true) => {
    clearTimeout(timer); cancelAnimationFrame(frame); drag = null; closing = false;
    panel.style.removeProperty('--sheet-drag');
    main.removeAttribute('data-preview-sheet');
    panel.setAttribute('role', 'region'); panel.removeAttribute('aria-modal');
    unlock();
    if (restore && restoreFocus?.isConnected) restoreFocus.focus({preventScroll:true});
    restoreFocus = null;
  };
  const dismiss = () => {
    if (!visible || closing || stopped) return;
    if (!compact) { dioxus.send('dismiss'); return; }
    closing = true; drag = null;
    cancelAnimationFrame(frame);
    panel.style.removeProperty('--sheet-drag');
    main.setAttribute('data-preview-sheet', 'closing');
    timer = setTimeout(() => {
      if (stopped || !closing) return;
      reset(); visible = false;
      dioxus.send('dismiss');
    }, reduced.matches ? 0 : 300);
  };
  const viewport = () => main.style.setProperty('--preview-viewport', `${window.visualViewport?.height || innerHeight}px`);
  const reconcile = () => {
    const nextCompact = main.classList.contains('mobile-app') || compactQuery.matches;
    const wanted = main.dataset.mediaSelected === 'true' && ['preview','inspector','posters'].includes(main.dataset.view);
    viewport();
    const changedMode = compact !== nextCompact;
    compact = nextCompact;
    main.dataset.sheetMode = String(compact);
    if (!wanted || !compact) {
      if (main.hasAttribute('data-preview-sheet')) reset();
      visible = wanted; return;
    }
    if (visible && !changedMode) return;
    reset(false); visible = true;
    const focused = document.activeElement;
    restoreFocus = focused?.closest?.('.media-row,.poster-grid-item') || (trigger?.isConnected ? trigger : focused);
    lock(); panel.setAttribute('role', 'dialog'); panel.setAttribute('aria-modal', 'true');
    main.setAttribute('data-preview-sheet', 'entering');
    frame = requestAnimationFrame(() => { frame = requestAnimationFrame(() => {
      if (stopped || closing || !visible) return;
      main.setAttribute('data-preview-sheet', 'open');
      panel.querySelector('.preview-sheet-close')?.focus({preventScroll:true});
    }); });
  };
  listen(document, 'click', event => {
    const target = event.target.closest?.('.media-row,.poster-grid-item');
    if (target && !panel.contains(target)) trigger = target;
  }, true);
  listen(document, 'keydown', event => {
    if (!visible) return;
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); dismiss(); }
    if (!compact || event.key !== 'Tab') return;
    const nodes = focusable();
    if (!nodes.length) { event.preventDefault(); panel.focus(); return; }
    const first = nodes[0], last = nodes[nodes.length - 1];
    if (event.shiftKey && (document.activeElement === first || !panel.contains(document.activeElement))) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && (document.activeElement === last || !panel.contains(document.activeElement))) { event.preventDefault(); first.focus(); }
  }, true);
  const start = event => {
    if (!compact || !visible || closing || event.target.closest('button') || (event.pointerType === 'mouse' && event.button !== 0)) return;
    drag = {id:event.pointerId, start:event.clientY, time:performance.now(), distance:0};
    event.currentTarget.setPointerCapture(event.pointerId); event.preventDefault();
  };
  const move = event => {
    if (!drag || drag.id !== event.pointerId) return;
    drag.distance = Math.max(0, event.clientY - drag.start);
    main.setAttribute('data-preview-sheet', 'dragging');
    panel.style.setProperty('--sheet-drag', `${drag.distance}px`);
  };
  const finish = event => {
    if (!drag || drag.id !== event.pointerId) return;
    const distance = drag.distance, speed = distance / Math.max(1, performance.now() - drag.time);
    drag = null; panel.style.removeProperty('--sheet-drag');
    if (event.type === 'pointerup' && (distance > Math.min(140, panel.getBoundingClientRect().height * .22) || (distance > 35 && speed > .65))) dismiss();
    else main.setAttribute('data-preview-sheet', 'open');
  };
  for (const target of [grip, toolbar]) {
    listen(target, 'pointerdown', start); listen(target, 'pointermove', move);
    listen(target, 'pointerup', finish); listen(target, 'pointercancel', finish);
    listen(target, 'lostpointercapture', finish);
  }
  const observer = new MutationObserver(reconcile);
  observer.observe(main, {attributes:true, attributeFilter:['data-media-selected','data-view','class']});
  listen(compactQuery, 'change', reconcile); listen(window, 'resize', reconcile);
  if (window.visualViewport) listen(window.visualViewport, 'resize', viewport);
  window.mediaManagerPreviewSheet = {dismiss, cleanup:() => {
    stopped = true; observer.disconnect(); listeners.forEach(remove => remove());
    reset(false); main.removeAttribute('data-sheet-mode');
    main.style.removeProperty('--preview-viewport');
    window.mediaManagerPreviewSheet?.finish?.();
    delete window.mediaManagerPreviewSheet;
  }};
  reconcile();
})();
// Keep the document-eval channel alive for later gesture/keyboard callbacks.
if (window.mediaManagerPreviewSheet) {
  await new Promise(resolve => { window.mediaManagerPreviewSheet.finish = resolve; });
}
