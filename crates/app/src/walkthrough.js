// Kept pure so phone/tablet/desktop positioning can be checked without a WebView.
function placeTour(rect, card, viewport) {
  const gap = 14, margin = 12;
  const width = Math.min(360, viewport.width - margin * 2);
  const height = Math.min(card.height, viewport.height * .52);
  let top = rect ? rect.bottom + gap : (viewport.height - height) / 2;
  if (top + height > viewport.height - margin && rect) top = rect.top - height - gap;
  top = Math.max(margin, Math.min(top, viewport.height - height - margin));
  const left = Math.max(margin, Math.min(rect ? rect.left : (viewport.width - width) / 2, viewport.width - width - margin));
  return {top, left, width};
}
if (typeof module !== 'undefined') module.exports = {placeTour};
if (typeof document !== 'undefined') (() => {
  window.mediaManagerTour?.cleanup();
  const card = document.getElementById('app-walkthrough');
  if (!card) return;
  let target, frame = 0, stopped = false, described, sent = false, missingFrames = 0;
  const previousFocus = document.activeElement;
  const ring = document.createElement('div');
  ring.className = 'walkthrough-spotlight';
  ring.setAttribute('aria-hidden', 'true');
  document.body.append(ring);
  const listeners = [];
  const listen = (node, type, handler, options) => {
    node.addEventListener(type, handler, options);
    listeners.push(() => node.removeEventListener(type, handler, options));
  };
  const visible = node => node.getClientRects().length && !node.closest('[inert],[hidden]');
  const restoreDescription = () => {
    if (!target) return;
    if (described === null) target.removeAttribute('aria-describedby');
    else if (described !== undefined) target.setAttribute('aria-describedby', described);
  };
  const update = () => {
    if (stopped) return;
    const next = [...document.querySelectorAll(config.selector)].find(visible);
    if (next !== target) {
      restoreDescription();
      target = next;
      if (target) {
        described = target.getAttribute('aria-describedby');
        target.setAttribute('aria-describedby', [described, 'walkthrough-description'].filter(Boolean).join(' '));
        target.scrollIntoView({block:'center', inline:'nearest', behavior:'instant'});
      }
    }
    const rect = target?.getBoundingClientRect();
    if (!rect && ++missingFrames < 3) { schedule(); return; }
    if (!rect) { send('skip'); return; }
    missingFrames = 0;
    document.getElementById('walkthrough-unavailable').hidden = !!rect;
    ring.hidden = !rect;
    if (rect) Object.assign(ring.style, {left:`${rect.left - 5}px`, top:`${rect.top - 5}px`, width:`${rect.width + 10}px`, height:`${rect.height + 10}px`});
    const view = {width:window.innerWidth, height:window.innerHeight};
    const position = placeTour(rect, card.getBoundingClientRect(), view);
    Object.assign(card.style, {left:`${position.left}px`, top:`${position.top}px`, width:`${position.width}px`});
  };
  const schedule = () => { cancelAnimationFrame(frame); frame = requestAnimationFrame(update); };
  const send = action => { if (!stopped && !sent) { sent = true; dioxus.send(action); } };
  // Capture before app handlers: highlighted actions advance the tour without
  // triggering purchases, file operations, dialogs, or network requests.
  listen(document, 'click', event => {
    if (card.contains(event.target)) return;
    event.preventDefault(); event.stopImmediatePropagation();
    if (target?.contains(event.target) && !event.target.closest('[disabled]')) send('next');
  }, true);
  listen(document, 'pointerdown', event => {
    if (!card.contains(event.target)) { event.preventDefault(); event.stopImmediatePropagation(); }
  }, true);
  listen(document, 'keydown', event => {
    if (event.key === 'Escape') {event.preventDefault(); event.stopImmediatePropagation(); send('close'); return;}
    if (event.key !== 'Tab' && !card.contains(event.target)) {
      event.preventDefault(); event.stopImmediatePropagation();
      if (event.key === 'Enter' || event.key === ' ') send('next');
      return;
    }
    if (event.key !== 'Tab') return;
    const nodes = [...card.querySelectorAll('button:not(:disabled)')];
    const first = nodes[0], last = nodes[nodes.length - 1];
    if (event.shiftKey && (document.activeElement === first || !card.contains(document.activeElement))) {event.preventDefault();last?.focus();}
    else if (!event.shiftKey && (document.activeElement === last || !card.contains(document.activeElement))) {event.preventDefault();first?.focus();}
  }, true);
  listen(window, 'resize', schedule);
  listen(document, 'scroll', schedule, true);
  const observer = new MutationObserver(schedule);
  observer.observe(document.querySelector('main'), {subtree:true, childList:true, attributes:true, attributeFilter:['class','hidden','inert']});
  const sizeObserver = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(schedule) : null;
  sizeObserver?.observe(card);
  window.mediaManagerTour = {cleanup() {
    if (stopped) return;
    dioxus.send('closed'); stopped = true;
    cancelAnimationFrame(frame); observer.disconnect(); sizeObserver?.disconnect(); listeners.forEach(remove => remove());
    restoreDescription(); ring.remove();
    if (previousFocus?.isConnected) previousFocus.focus({preventScroll:true});
  }};
  schedule();
  document.getElementById('walkthrough-heading')?.focus({preventScroll:true});
})();
