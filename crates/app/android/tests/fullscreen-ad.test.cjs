const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const script = fs.readFileSync(path.join(__dirname, '../../src/fullscreen-ad.js'), 'utf8');
function harness({ios=false}={}) {
  const messages=[],listeners=new Map(),dataset={}; let modal=false,playing=false,now=1000,tick,frame;
  const button={},label={},privacy={hidden:true};
  const handler={postMessage:data=>messages.push(JSON.parse(data))};
  const window={};if(ios)window.webkit={messageHandlers:{MediaManagerAds:handler}};else window.MediaManagerAds=handler;
  const document={hidden:false,documentElement:{dataset},querySelector:()=>modal?{}:null,
    querySelectorAll:selector=>selector==='video'?[{paused:!playing,ended:false}]:selector==='.ad-rewarded'?[button]:selector==='.fullscreen-ad-status'?[label]:selector==='.ad-privacy'?[privacy]:[],
    addEventListener:(name,fn)=>listeners.set(name,fn),removeEventListener:name=>listeners.delete(name)};
  vm.runInNewContext(script,{window,document,Date:{now:()=>now},setTimeout:fn=>{tick=fn;return 1},clearTimeout:()=>{tick=null},requestAnimationFrame:fn=>{frame=fn}});
  return {window,document,dataset,messages,button,label,privacy,listeners,modal:value=>modal=value,playing:value=>playing=value,now:value=>now=value,
    tick:()=>tick?.(),frame:()=>{const fn=frame;frame=null;fn?.()},click:()=>listeners.get('click')({target:{closest:s=>s==='.ad-rewarded'?{}:null}})};
}
test('requests native state on both platforms and requires an explicit rewarded click',()=>{
  for(const ios of [false,true]) {
    const h=harness({ios});assert.equal(h.messages.at(-1).type,'fullscreen-state');h.tick();assert.equal(h.messages.length,1);
    h.click();assert.equal(h.messages.at(-1).type,'rewarded');assert.equal(h.window.mediaManagerAdsSuppressed,false);
  }
});
test('only native earned state activates reward; expiry restores ad eligibility',()=>{
  const h=harness();h.click();assert.equal(h.dataset.adsPaused,'false');
  h.window.mediaManagerFullscreenState({adFreeUntil:61000,presenting:false,rewardedReady:false,message:''});
  assert.equal(h.dataset.adsPaused,'true');assert.equal(h.button.disabled,true);assert.match(h.label.textContent,/1 more minutes/);
  h.window.mediaManagerRenameAd();h.frame();assert.equal(h.messages.at(-1).type,'rewarded');
  h.now(62000);h.tick();assert.equal(h.dataset.adsPaused,'false');assert.equal(h.button.disabled,false);
});
test('rename opportunity is skipped under modal, background, work or playing video; never deferred',()=>{
  const h=harness();h.modal(true);h.window.mediaManagerRenameAd();h.frame();assert.equal(h.messages.length,1);
  h.modal(false);h.tick();assert.equal(h.messages.length,1);
  h.playing(true);h.window.mediaManagerRenameAd();h.frame();assert.equal(h.messages.length,1);
  h.playing(false);h.document.hidden=true;h.click();assert.equal(h.messages.length,1);
  h.document.hidden=false;h.window.mediaManagerRenameAd();h.frame();assert.equal(h.messages.at(-1).type,'interstitial');
});
test('presentation suppresses native ads and duplicate requests; cleanup releases listeners',()=>{
  const h=harness();h.window.mediaManagerFullscreenState({adFreeUntil:0,presenting:true,rewardedReady:false,message:''});
  assert.equal(h.window.mediaManagerAdsSuppressed,true);h.click();h.window.mediaManagerRenameAd();h.frame();assert.equal(h.messages.length,1);
  h.window.mediaManagerFullscreenCleanup();assert.equal(h.listeners.size,0);assert.equal(h.window.mediaManagerRenameAd,undefined);assert.equal(h.dataset.adsPaused,undefined);
});

test('native inline playback blocks interstitial opportunities without a DOM video element',()=>{
  const h=harness();h.window.mediaManagerNativePlaybackActive=true;
  h.window.mediaManagerRenameAd();h.frame();assert.equal(h.messages.length,1);
  h.window.mediaManagerNativePlaybackActive=false;
  h.window.mediaManagerRenameAd();h.frame();assert.equal(h.messages.at(-1).type,'interstitial');
});

test('privacy options stay available when Settings controls mount after consent is checked',()=>{
  const h=harness();h.window.mediaManagerFullscreenState({adFreeUntil:0,presenting:false,rewardedReady:true,privacyRequired:true,message:''});
  h.privacy.hidden=true;h.tick();assert.equal(h.privacy.hidden,false);
});
