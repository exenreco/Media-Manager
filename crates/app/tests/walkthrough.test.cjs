const {test} = require('node:test');
const assert = require('node:assert/strict');
const {placeTour} = require('../src/walkthrough.js');

test('phone, landscape, tablet and desktop cards stay within the viewport', () => {
  for (const [width,height] of [[320,568],[390,844],[844,390],[1024,1366],[1280,800]]) {
    for (const rect of [null,{left:0,top:0,bottom:40},{left:width-40,top:height-50,bottom:height-10}]) {
      const p = placeTour(rect,{height:300},{width,height});
      assert.ok(p.left >= 12);
      assert.ok(p.top >= 12);
      assert.ok(p.left+p.width <= width-12);
      assert.ok(p.top+Math.min(300,height*.52) <= height-12);
    }
  }
});
test('card goes below a titlebar control and above a footer control', () => {
  assert.equal(placeTour({left:300,top:0,bottom:46},{height:250},{width:1280,height:800}).top,60);
  assert.equal(placeTour({left:30,top:730,bottom:790},{height:250},{width:1280,height:800}).top,466);
});

test('highlighted actions advance once without reaching app handlers; cleanup restores target', () => {
  const vm = require('node:vm');
  const fs = require('node:fs');
  const listeners = new Map(), frames = [], messages = [];
  const attrs = new Map();
  const target = {getClientRects:()=>[{}],closest:()=>null,contains:n=>n===target,
    getAttribute:k=>attrs.get(k)??null,setAttribute:(k,v)=>attrs.set(k,v),removeAttribute:k=>attrs.delete(k),
    scrollIntoView(){},getBoundingClientRect:()=>({left:10,top:10,bottom:50,width:100,height:40})};
  const card = {contains:()=>false,style:{},getBoundingClientRect:()=>({height:200}),querySelectorAll:()=>[]};
  const ring = {style:{},setAttribute(){},remove(){this.removed=true;}};
  const document = {activeElement:null,body:{append(){}},getElementById:id=>id==='app-walkthrough'?card:id==='walkthrough-unavailable'?{}:{focus(){}},
    querySelectorAll:()=>[target],querySelector:()=>({}),createElement:()=>ring,
    addEventListener:(k,v)=>listeners.set(k,v),removeEventListener:k=>listeners.delete(k)};
  const window = {innerWidth:390,innerHeight:844,addEventListener(){},removeEventListener(){}};
  vm.runInNewContext(fs.readFileSync(require.resolve('../src/walkthrough.js'),'utf8'),{document,window,config:{selector:'button'},dioxus:{send:x=>messages.push(x)},requestAnimationFrame:fn=>(frames.push(fn),frames.length),cancelAnimationFrame(){},MutationObserver:class{observe(){} disconnect(){}}});
  frames.shift()();
  let blocked=0;
  const click={target,preventDefault(){blocked++},stopImmediatePropagation(){blocked++}};
  listeners.get('click')(click);
  listeners.get('click')(click);
  assert.deepEqual(messages,['next']);
  assert.equal(blocked,4);
  assert.equal(attrs.get('aria-describedby'),'walkthrough-description');
  window.mediaManagerTour.cleanup();
  assert.equal(attrs.has('aria-describedby'),false);
  assert.equal(listeners.size,0);
  assert.equal(ring.removed,true);
});
