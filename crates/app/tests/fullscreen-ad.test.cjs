const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const script = fs.readFileSync(path.join(__dirname, '../src/fullscreen-ad.js'), 'utf8');

function harness({ios = true} = {}) {
  const messages = [], listeners = new Map(), button = {}, status = {};
  let modal = false, tick, now = 1_000_000, frame;
  const handler = {postMessage: value => messages.push(JSON.parse(value))};
  const window = ios ? {webkit: {messageHandlers: {MediaManagerAds: handler}}} : {MediaManagerAds: handler};
  const document = {
    hidden: false, documentElement: {dataset: {}},
    querySelector: () => modal ? {} : null,
    querySelectorAll: selector => ({'.ad-rewarded': [button], '.fullscreen-ad-status': [status]}[selector] || []),
    addEventListener: (name, callback) => listeners.set(name, callback),
    removeEventListener: name => listeners.delete(name),
  };
  vm.runInNewContext(script, {window, document, Date: {now: () => now},
    setTimeout: callback => {tick = callback; return 1;}, clearTimeout: () => {tick = null;},
    requestAnimationFrame: callback => {frame = callback;},
  });
  return {window, document, messages, button, status, listeners,
    state: value => window.mediaManagerFullscreenState({adFreeUntil: 0, presenting: false, rewardedReady: false, ...value}),
    click: () => listeners.get('click')({target: {closest: selector => selector === '.ad-rewarded' ? {} : null}}),
    tick: () => tick?.(), frame: () => frame?.(),
    advance: ms => {now += ms;}, modal: value => {modal = value;},
  };
}

for (const ios of [true, false]) {
  test(`${ios ? 'iOS' : 'Android'} requests a rewarded ad without granting ad-free time`, () => {
    const h = harness({ios});
    assert.equal(h.messages[0].type, 'fullscreen-state');
    h.click();
    assert.equal(h.messages.at(-1).type, 'rewarded');
    assert.equal(h.window.mediaManagerAdsSuppressed, false);
    assert.equal(h.document.documentElement.dataset.adsPaused, 'false');
  });
}

test('consent and network failures appear beside the rewarded button and allow retry', () => {
  const h = harness();
  const message = 'Advertising privacy check failed. Check your DNS/content blocker.';
  h.state({message}); h.tick();
  assert.equal(h.status.textContent, message);
  assert.equal(h.button.disabled, false);
  h.click();
  assert.equal(h.messages.at(-1).type, 'rewarded');
  assert.equal(h.window.mediaManagerAdsSuppressed, false);
});

test('loading disables repeat requests and readiness re-enables watching', () => {
  const h = harness();
  h.state({rewardedLoading: true, message: 'Loading a rewarded ad…'});
  assert.equal(h.button.disabled, true);
  assert.equal(h.button.textContent, 'Loading rewarded ad…');
  const count = h.messages.length;
  h.click();
  assert.equal(h.messages.length, count);
  h.state({rewardedLoading: false, rewardedReady: true});
  assert.equal(h.button.disabled, false);
  h.click();
  assert.equal(h.messages.at(-1).type, 'rewarded');
});

test('only a native earned deadline activates 30 minutes, then expires', () => {
  const h = harness();
  h.state({presenting: true});
  assert.equal(h.document.documentElement.dataset.adsPaused, 'false');
  h.state({adFreeUntil: 1_000_000 + 30 * 60 * 1000});
  assert.equal(h.button.disabled, true);
  assert.equal(h.document.documentElement.dataset.adsPaused, 'true');
  assert.match(h.status.textContent, /30 more minutes/);
  h.advance(30 * 60 * 1000); h.tick();
  assert.equal(h.button.disabled, false);
  assert.equal(h.window.mediaManagerAdsSuppressed, false);
});

test('modals, backgrounding, playback and payment UI block rewarded presentation', () => {
  const h = harness();
  const count = h.messages.length;
  h.modal(true); h.click(); h.modal(false);
  h.document.hidden = true; h.click(); h.document.hidden = false;
  h.window.mediaManagerNativePlaybackActive = true; h.click();
  h.window.mediaManagerNativePlaybackActive = false;
  h.window.mediaManagerBillingPresenting = true; h.tick(); h.click();
  assert.equal(h.messages.length, count);
  assert.equal(h.button.disabled, true);
});

test('rename ads are skipped if a dialog opens before the scheduled frame', () => {
  const h = harness();
  h.window.mediaManagerRenameAd(); h.modal(true); h.frame();
  assert.equal(h.messages.length, 1);
  h.window.mediaManagerFullscreenCleanup();
  assert.equal(h.listeners.size, 0);
  assert.equal(h.window.mediaManagerAdsSuppressed, undefined);
});
