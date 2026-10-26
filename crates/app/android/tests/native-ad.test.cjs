const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const script = fs.readFileSync(path.join(__dirname, '../../src/native-ad.js'), 'utf8');

function harness({bridge = true, ios = false} = {}) {
  const messages = [], listeners = new Map();
  const rect = {x:10, y:10, top:10, left:10, right:310, bottom:450, width:300, height:440};
  const parentRect = {top:0, left:0, right:400, bottom:800};
  const parent = {parentElement:null, getBoundingClientRect:() => parentRect};
  const slot = {hidden:false, parentElement:parent, getBoundingClientRect:() => rect};
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
  const document = {
    hidden:false,
    getElementById:() => slot,
    querySelector:() => modal ? {} : null,
    addEventListener:(name, callback) => listeners.set(name, callback),
    removeEventListener:name => listeners.delete(name),
  };
  const context = {window, document, innerWidth:400, innerHeight:800,
    getComputedStyle:() => ({overflow:'auto', overflowX:'auto', overflowY:'auto'}),
    requestAnimationFrame:callback => {frame = callback; return 1;},
    cancelAnimationFrame:() => {frame = null;},
  };
  vm.runInNewContext(script, context);
  return {messages, rect, slot, parentRect, window, document, listeners,
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
  assert.equal(missing.slot.hidden, true);
  assert.equal(missing.messages.length, 0);
  const h = harness();
  h.listeners.get('click')({target:{closest:() => null}});
  assert.equal(h.messages.length, 1);
  h.listeners.get('click')({target:{closest:() => ({})}});
  assert.equal(h.messages.at(-1).type, 'privacy');
});

test('uses the iOS WebKit handler for geometry, privacy and teardown', () => {
  const h = harness({ios:true});
  assert.equal(h.messages.at(-1).type, 'bounds');
  assert.equal(h.messages.at(-1).visible, true);
  h.listeners.get('click')({target:{closest:() => ({})}});
  assert.equal(h.messages.at(-1).type, 'privacy');
  h.window.mediaManagerAdCleanup();
  assert.equal(h.messages.at(-1).type, 'hide');
});
