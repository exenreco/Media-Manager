const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const script = fs.readFileSync(path.join(__dirname, '../../src/native-ad.js'), 'utf8');

function harness({bridge = true, ios = false, viewportHeight = 800} = {}) {
  const messages = [], listeners = new Map();
  const rect = {x:10, y:10, top:10, left:10, right:310, bottom:450, width:300, height:440};
  const parentRect = {top:0, left:0, right:400, bottom:800};
  const parent = {parentElement:null, getBoundingClientRect:() => parentRect};
  const slot = {id:'library',style:{},hidden:false, parentElement:parent, getBoundingClientRect:() => rect};
  let modal = false, frame;
  const window = {
    addEventListener:(name, callback) => listeners.set(name, callback),
    removeEventListener:name => listeners.delete(name),
  };
  if (bridge) {
    const handler = {postMessage:value => messages.push(JSON.parse(value))};
    if (ios) window.webkit = {messageHandlers:{MediaManagerAds:handler}};
    else window.MediaManagerAds = handler;
  }
  const status = {hidden:false,textContent:''}, retry = {hidden:true};
  const posterRect = {...rect, y:900, top:900, bottom:1340};
  const poster = {id:'posters', style:{}, hidden:false, parentElement:parent, getBoundingClientRect:() => posterRect};
  const document = {
    querySelectorAll:selector => selector === '.native-ad-slot' ? [slot, poster] : selector === '.native-ad-status' ? [status] : selector === '.ad-retry' ? [retry] : [],
    hidden:false,
    getElementById:id => id === 'native-ad-slot' ? slot : id === 'native-ad-status' ? status : id === 'ad-retry' ? retry : {hidden:true},
    querySelector:() => modal ? {} : null,
    addEventListener:(name, callback) => listeners.set(name, callback),
    removeEventListener:name => listeners.delete(name),
  };
  const context = {window, document, innerWidth:400, innerHeight:viewportHeight,
    getComputedStyle:() => ({overflow:'auto', overflowX:'auto', overflowY:'auto'}),
    requestAnimationFrame:callback => {frame = callback; return 1;},
    cancelAnimationFrame:() => {frame = null;},
  };
  vm.runInNewContext(script, context);
  return {messages, rect, slot, poster, posterRect, parentRect, window, document, listeners, status, retry,
    installBridge:() => { window.MediaManagerAds = {postMessage:value => messages.push(JSON.parse(value))}; },
    tick:() => frame?.(), modal:value => {modal = value;}};
}

test('shows an entirely visible slot, deduplicates unchanged geometry and cleans up', () => {
  const h = harness();
  assert.equal(h.messages.at(-1).visible, true);
  h.tick(); assert.equal(h.messages.length, 1);
  h.window.mediaManagerAdCleanup();
  assert.equal(h.messages.at(-1).type, 'hide');
  assert.equal(h.listeners.size, 0);
});

test('hides the native overlay under dialogs, scrolling clips, and backgrounding', () => {
  const h = harness();
  h.modal(true); h.tick(); assert.equal(h.messages.at(-1).visible, false);
  h.modal(false); h.tick(); assert.equal(h.messages.at(-1).visible, true);
  h.parentRect.bottom = 400; h.tick(); assert.equal(h.messages.at(-1).visible, false);
  h.parentRect.bottom = 800; h.tick(); assert.equal(h.messages.at(-1).visible, true);
  h.document.hidden = true; h.tick(); assert.equal(h.messages.at(-1).visible, false);
});

test('fails closed without an Android bridge and never forwards ad click events', () => {
  const missing = harness({bridge:false});
  for(let i=0;i<120;i++) missing.tick();
  assert.equal(missing.slot.hidden, true);
  assert.equal(missing.messages.length, 0);
  const h = harness();
  h.listeners.get('click')({target:{closest:() => null}});
  assert.equal(h.messages.length, 1);
  h.listeners.get('click')({target:{closest:selector => selector === '.ad-privacy' ? {} : null}});
  assert.equal(h.messages.at(-1).type, 'privacy');
});

test('uses the iOS WebKit handler for geometry, privacy and teardown', () => {
  const h = harness({ios:true});
  assert.equal(h.messages.at(-1).type, 'bounds');
  assert.equal(h.messages.at(-1).visible, true);
  h.listeners.get('click')({target:{closest:selector => selector === '.ad-privacy' ? {} : null}});
  assert.equal(h.messages.at(-1).type, 'privacy');
  h.window.mediaManagerAdCleanup();
  assert.equal(h.messages.at(-1).type, 'hide');
});

test('loads an eligible clipped slot but keeps its native overlay hidden', () => {
  const h=harness(); h.rect.bottom=850; h.rect.top=410; h.rect.y=410; h.tick();
  assert.equal(h.messages.at(-1).eligible,true);
  assert.equal(h.messages.at(-1).visible,false);
});
test('waits for a late native bridge instead of permanently hiding the slot', () => {
  const h=harness({bridge:false}); h.tick(); h.installBridge(); h.tick();
  assert.equal(h.slot.hidden,false); assert.equal(h.messages.at(-1).eligible,true);
});
test('hides ads in a panel switched out of the single-column flow', () => {
  const h=harness(); h.parentRect.bottom=0; h.tick();
  assert.equal(h.messages.at(-1).eligible,false); assert.equal(h.messages.at(-1).visible,false);
});

test('uses a compact slot on a short phone viewport', () => {
  const h = harness({viewportHeight:600});
  assert.equal(h.slot.style.height,'380px');
});
test('failed requests expose status and retry only when the user asks', () => {
  const h = harness(); h.slot.hidden=true;
  h.window.mediaManagerAdStatus('Ad unavailable (code 3).',true);
  assert.equal(h.status.textContent,'Ad unavailable (code 3).'); assert.equal(h.retry.hidden,false);
  h.tick(); assert.equal(h.messages.at(-1).eligible,false);
  assert.equal(h.messages.some(m => m.type==='retry'),false);
  h.listeners.get('click')({target:{closest:selector => selector === '.ad-retry' ? {} : null}});
  assert.equal(h.slot.hidden,false); assert.equal(h.messages.at(-1).type,'retry');
});

test('moves the native view to the poster placement when the library is hidden', () => {
  const h = harness();
  assert.equal(h.messages.at(-1).placement, 'library');
  h.rect.top = -500; h.rect.bottom = -60;
  Object.assign(h.posterRect, {top:10,y:10,bottom:450});
  h.tick();
  assert.equal(h.messages.at(-1).placement, 'posters');
  assert.equal(h.messages.at(-1).visible, true);
  h.window.mediaManagerAdSetHidden(true);
  assert.equal(h.slot.hidden, true); assert.equal(h.poster.hidden, true);
  h.window.mediaManagerAdSetHidden(false);
  h.tick(); assert.equal(h.messages.at(-1).visible, true);
});


test('floating navigation and expanded tools cannot be covered by a native ad', () => {
  const h = harness();
  const dock = {getBoundingClientRect:() => ({top:400,bottom:780,left:20,right:380})};
  let expanded = false;
  h.document.querySelector = selector => selector === '.mobile-dock' ? dock : selector === '.mobile-dock.expanded' && expanded ? {} : null;
  h.tick(); assert.equal(h.messages.at(-1).visible, false);
  h.rect.bottom=390; h.tick(); assert.equal(h.messages.at(-1).visible, true);
  expanded=true; h.tick(); assert.equal(h.messages.at(-1).eligible, false);
  assert.equal(h.messages.at(-1).visible, false);
});


test('connection confirmation keeps native advertisements behind the notice', () => {
  const h=harness();
  h.document.querySelector=selector=>selector === '.connection-island[data-state=warning]' ? {} : null;
  h.tick();assert.equal(h.messages.at(-1).eligible,false);assert.equal(h.messages.at(-1).visible,false);
});

test('ordering popup keeps native advertisements behind the dialog', () => {
  const h = harness();
  h.document.querySelector = selector => selector === '.ordering-dialog[open]' ? {} : null;
  h.tick();
  assert.equal(h.messages.at(-1).eligible, false);
});
test('matched show confirmation keeps advertisements behind the notice', () => {
  const h = harness();
  h.document.querySelector = selector => selector === '.connection-island[data-state=match]' ? {} : null;
  h.tick();
  assert.equal(h.messages.at(-1).eligible, false);
});

test('preview sheets keep native advertisements hidden until fully dismissed', () => {
  const h = harness();
  h.document.querySelector = selector => selector === 'main[data-preview-sheet]' ? {} : null;
  h.tick();
  assert.equal(h.messages.at(-1).eligible, false);
  assert.equal(h.messages.at(-1).visible, false);
  h.document.querySelector = () => null;
  h.tick();
  assert.equal(h.messages.at(-1).visible, true);
});
