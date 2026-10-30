const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const script = fs.readFileSync(path.join(__dirname, '../../src/billing.js'), 'utf8');
function harness() {
  const messages=[], listeners=new Map(), buy={}, restore={}, label={}; let tick, modal=false, refreshes=0;
  const window={MediaManagerAds:{postMessage:data=>messages.push(JSON.parse(data))},mediaManagerFullscreenRefresh:()=>refreshes++};
  const document={hidden:false,querySelector:()=>modal?{}:null,
    querySelectorAll:selector=>selector==='.billing-buy'?[buy]:selector==='.billing-restore'?[restore]:selector==='.billing-status'?[label]:[],
    addEventListener:(name,fn)=>listeners.set(name,fn),removeEventListener:name=>listeners.delete(name)};
  vm.runInNewContext(script,{window,document,setTimeout:fn=>{tick=fn;return 1},clearTimeout:()=>{tick=null}});
  return {window,document,messages,buy,restore,label,listeners,tick:()=>tick?.(),modal:value=>modal=value,
    click:selector=>listeners.get('click')({target:{closest:s=>s===selector?{}:null}})};
}
const available={owned:false,busy:false,pending:false,available:true,price:'$3.99',message:''};
test('buy requires an available native offer and explicit tap; it never grants ownership',()=>{
  const h=harness();assert.equal(h.messages[0].type,'billing-state');
  h.click('.billing-buy');assert.equal(h.messages.length,1);
  h.window.mediaManagerBillingState(available);assert.equal(h.buy.textContent,'Remove ads — $3.99');
  h.click('.billing-buy');assert.equal(h.messages.at(-1).type,'billing-buy');
  assert.equal(h.window.mediaManagerPermanentAdFree,false);
  h.window.mediaManagerBillingState({...available,pending:true});h.click('.billing-buy');
  assert.equal(h.messages.length,2);assert.equal(h.buy.textContent,'Payment pending');
});
test('native ownership hides ads across late renders; restore is available to existing owners',()=>{
  const h=harness();h.window.mediaManagerBillingState({...available,owned:true});
  assert.equal(h.window.mediaManagerPermanentAdFree,true);assert.equal(h.buy.disabled,true);
  h.click('.billing-buy');assert.equal(h.messages.length,1);
  h.click('.billing-restore');assert.equal(h.messages.at(-1).type,'billing-restore');
  h.buy.textContent='late mounted';h.tick();assert.equal(h.buy.textContent,'Ads removed');
  h.window.mediaManagerBillingState(available);assert.equal(h.window.mediaManagerPermanentAdFree,false);
});
test('checkout suppresses ads; busy, modal and playback prevent duplicate purchase actions',()=>{
  const h=harness();h.window.mediaManagerBillingState({...available,busy:true,presenting:true});
  assert.equal(h.window.mediaManagerBillingPresenting,true);h.click('.billing-buy');h.click('.billing-restore');assert.equal(h.messages.length,1);
  h.window.mediaManagerBillingState(available);h.modal(true);h.click('.billing-buy');assert.equal(h.messages.length,1);
  h.modal(false);h.window.mediaManagerNativePlaybackActive=true;h.click('.billing-buy');assert.equal(h.messages.length,1);
});
test('resume queries native ownership; cleanup releases callbacks and listeners',()=>{
  const h=harness();h.listeners.get('visibilitychange')();assert.equal(h.messages.at(-1).type,'billing-state');
  h.window.mediaManagerBillingCleanup();assert.equal(h.listeners.size,0);assert.equal(h.window.mediaManagerBillingState,undefined);
  const count=h.messages.length;h.tick();assert.equal(h.messages.length,count);
});
test('restore remains visible but is disabled after the payment window without removing ownership',()=>{
  const h=harness();
  h.window.mediaManagerBillingState({...available,owned:true,restoreUntil:Date.now()+86400000});
  assert.equal(h.restore.disabled,false);
  h.window.mediaManagerBillingState({...available,owned:true,restoreUntil:Date.now()-1});
  assert.equal(h.restore.disabled,true);
  assert.equal(h.window.mediaManagerPermanentAdFree,true);
  const count=h.messages.length;h.click('.billing-restore');assert.equal(h.messages.length,count);
});
