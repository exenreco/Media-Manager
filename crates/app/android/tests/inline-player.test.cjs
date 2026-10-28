const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const script = fs.readFileSync(path.join(__dirname, '../../src/android-inline-player.js'), 'utf8');

function harness() {
  const messages = [], listeners = new Map();
  const rect = {x:10,y:100,left:10,top:100,right:330,bottom:300,width:320,height:200};
  let source = '/native-media-preview/one', modal = false, frame;
  const parent = {parentElement:null,getBoundingClientRect:()=>({left:0,top:0,right:400,bottom:750})};
  const slot = {parentElement:parent,getBoundingClientRect:()=>rect,getAttribute:()=>source};
  const window = {MediaManagerStorage:{postMessage:s=>messages.push(JSON.parse(s))},
    addEventListener:(k,v)=>listeners.set(k,v),removeEventListener:k=>listeners.delete(k)};
  const document = {hidden:false,querySelector:selector=>selector.startsWith('.android-inline')?slot:modal?{}:null,
    addEventListener:(k,v)=>listeners.set(k,v),removeEventListener:k=>listeners.delete(k)};
  vm.runInNewContext(script,{window,document,innerWidth:400,innerHeight:800,
    getComputedStyle:()=>({display:'block',visibility:'visible',overflow:'hidden',overflowX:'hidden',overflowY:'hidden'}),
    requestAnimationFrame:f=>{frame=f;return 1;},cancelAnimationFrame:()=>{frame=null;}});
  return {messages,window,document,rect,listeners,tick:()=>frame?.(),modal:v=>modal=v,source:v=>source=v};
}

test('native inline preview follows source and geometry without retransmitting unchanged bounds',()=>{
  const h=harness();assert.equal(h.messages.at(-1).visible,true);assert.equal(h.window.mediaManagerNativePlaybackActive,true);
  h.tick();assert.equal(h.messages.length,1);
  h.source('/native-media-preview/two');h.tick();assert.equal(h.messages.at(-1).source,'/native-media-preview/two');
  h.rect.x=20;h.rect.left=20;h.tick();assert.equal(h.messages.at(-1).x,20);
});
test('native overlay hides under dialogs, outside the viewport, and when backgrounded',()=>{
  const h=harness();h.modal(true);h.tick();assert.equal(h.messages.at(-1).visible,false);
  h.modal(false);h.tick();assert.equal(h.messages.at(-1).visible,true);
  h.rect.bottom=900;h.tick();assert.equal(h.messages.at(-1).visible,false);
  h.rect.bottom=300;h.document.hidden=true;h.tick();assert.equal(h.messages.at(-1).visible,false);
  assert.equal(h.window.mediaManagerNativePlaybackActive,false);
});
test('cleanup releases native playback and listeners; fullscreen dismissal can refresh identical bounds',()=>{
  const h=harness();h.window.mediaManagerInlineRefresh();h.tick();assert.equal(h.messages.length,2);
  h.window.mediaManagerInlineCleanup();assert.equal(h.messages.at(-1).source,'');
  assert.equal(h.messages.at(-1).visible,false);assert.equal(h.listeners.size,0);
  assert.equal(h.window.mediaManagerNativePlaybackActive,false);
});
