// Native <dialog> retains focus trapping and background inertness. Compact
// menus share the media inspector's sheet geometry, gestures and spring timing.
const id = await dioxus.recv();
window.mediaManagerMenuSheets ||= new Map();
window.mediaManagerMenuSheets.get(id)?.cleanup();
const dialog = document.getElementById(id);
if (dialog) {
  const main = document.querySelector('main');
  const compact = matchMedia('(max-width:1050px), (max-height:650px)');
  const reduced = matchMedia('(prefers-reduced-motion:reduce)');
  const listeners = [];
  let closing = false, drag = null, timer = 0, frame = 0, finish;
  const listen = (node,type,fn) => {node.addEventListener(type,fn);listeners.push(()=>node.removeEventListener(type,fn));};
  const sheetMode = () => main?.classList.contains('mobile-app') || compact.matches;
  const sync = () => {
    dialog.dataset.sheet = String(sheetMode());
    dialog.style.setProperty('--preview-viewport', `${window.visualViewport?.height || innerHeight}px`);
    if (sheetMode()) main?.setAttribute('data-menu-sheet',id);
    else if (main?.dataset.menuSheet === id) main.removeAttribute('data-menu-sheet');
  };
  const dismiss = () => {
    if (closing) return;
    closing = true; drag = null; cancelAnimationFrame(frame);
    dialog.style.removeProperty('--sheet-drag');
    dialog.dataset.state = 'closing';
    timer = setTimeout(()=>{
      dialog.close();
      if (main?.dataset.menuSheet === id) main.removeAttribute('data-menu-sheet');
      dioxus.send('dismiss');
    }, sheetMode() && !reduced.matches ? 300 : 0);
  };
  listen(dialog,'cancel',event=>{event.preventDefault();dismiss();});
  listen(dialog,'click',event=>{
    if (event.target !== dialog) return;
    const r=dialog.getBoundingClientRect();
    if (event.clientX<r.left || event.clientX>r.right || event.clientY<r.top || event.clientY>r.bottom) dismiss();
  });
  const chrome=dialog.querySelector('.menu-sheet-chrome');
  listen(chrome,'pointerdown',event=>{
    if (!sheetMode() || closing || event.target.closest('button') || (event.pointerType==='mouse' && event.button!==0)) return;
    drag={id:event.pointerId,y:event.clientY,time:performance.now(),distance:0};
    chrome.setPointerCapture(event.pointerId);event.preventDefault();
  });
  listen(chrome,'pointermove',event=>{
    if (!drag || drag.id!==event.pointerId) return;
    drag.distance=Math.max(0,event.clientY-drag.y);
    dialog.dataset.state='dragging';dialog.style.setProperty('--sheet-drag',`${drag.distance}px`);
  });
  const end = event=>{
    if (!drag || drag.id!==event.pointerId) return;
    const distance=drag.distance, speed=distance/Math.max(1,performance.now()-drag.time);
    drag=null;dialog.style.removeProperty('--sheet-drag');
    if (event.type==='pointerup' && (distance>Math.min(140,dialog.getBoundingClientRect().height*.22) || (distance>35 && speed>.65))) dismiss();
    else dialog.dataset.state='open';
  };
  for (const type of ['pointerup','pointercancel','lostpointercapture']) listen(chrome,type,end);
  listen(compact,'change',sync);listen(window,'resize',sync);
  if (window.visualViewport) listen(window.visualViewport,'resize',sync);
  window.mediaManagerMenuSheets.set(id,{dismiss,cleanup:()=>{
    clearTimeout(timer);cancelAnimationFrame(frame);listeners.forEach(remove=>remove());
    if (dialog.open) dialog.close();
    if (main?.dataset.menuSheet===id) main.removeAttribute('data-menu-sheet');
    window.mediaManagerMenuSheets.delete(id);finish?.();
  }});
  sync();dialog.dataset.state='entering';dialog.showModal();
  frame=requestAnimationFrame(()=>{frame=requestAnimationFrame(()=>{if(!closing)dialog.dataset.state='open';});});
  await new Promise(resolve=>{finish=resolve;});
}
