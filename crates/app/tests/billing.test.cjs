const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const script = fs.readFileSync(path.join(__dirname, '../src/billing.js'), 'utf8');

function harness({ios = true, bridge = true} = {}) {
  const messages = [], listeners = new Map();
  const buy = {}, restore = {}, status = {};
  let tick, modal = false, refreshes = 0;
  const handler = {postMessage: value => messages.push(JSON.parse(value))};
  const window = {mediaManagerFullscreenRefresh: () => refreshes++};
  if (bridge) {
    if (ios) window.webkit = {messageHandlers: {MediaManagerAds: handler}};
    else window.MediaManagerAds = handler;
  }
  const document = {
    hidden: false,
    querySelectorAll: selector => ({'.billing-buy': [buy], '.billing-restore': [restore], '.billing-status': [status]}[selector] || []),
    querySelector: () => modal ? {} : null,
    addEventListener: (name, callback) => listeners.set(name, callback),
    removeEventListener: name => listeners.delete(name),
  };
  vm.runInNewContext(script, {window, document, Date, setTimeout: callback => {tick = callback; return 1;}, clearTimeout: () => {tick = null;}});
  return {window, document, buy, restore, status, messages, listeners,
    tick: () => tick?.(), modal: value => {modal = value;},
    refreshes: () => refreshes,
    installBridge: () => {window.webkit = {messageHandlers: {MediaManagerAds: handler}};},
    state: value => window.mediaManagerBillingState({owned: false, busy: false, pending: false, available: true, price: '$2.99', ...value}),
    click: selector => listeners.get('click')({target: {closest: candidate => candidate === selector ? {} : null}}),
  };
}

for (const ios of [true, false]) {
  test(`${ios ? 'iOS' : 'Android'} requests native billing and displays the store price`, () => {
    const h = harness({ios});
    assert.equal(h.messages[0].type, 'billing-state');
    assert.equal(h.buy.disabled, true);
    h.state({});
    assert.equal(h.buy.textContent, 'Remove ads — $2.99');
    assert.equal(h.buy.disabled, false);
    h.click('.billing-buy');
    assert.deepEqual(h.messages.at(-1), {type: 'billing-buy'});
    h.state({owned: true, canRestore: true, restoreUntil: Date.now() + 600_000});
    h.click('.billing-restore');
    assert.deepEqual(h.messages.at(-1), {type: 'billing-restore'});
    assert.equal(h.window.mediaManagerPermanentAdFree, true);
  });
}

test('verified native ownership suppresses ads and disables repeat purchases', () => {
  const h = harness();
  h.state({owned: true, canRestore: true, restoreUntil: Date.now() + 600_000, message: 'All ads removed on iOS.'});
  assert.equal(h.window.mediaManagerPermanentAdFree, true);
  assert.equal(h.buy.textContent, 'Ads removed');
  assert.equal(h.buy.disabled, true);
  assert.equal(h.restore.disabled, false);
  const count = h.messages.length;
  h.click('.billing-buy');
  assert.equal(h.messages.length, count);
  h.state({owned: false});
  assert.equal(h.window.mediaManagerPermanentAdFree, false);
  assert.ok(h.refreshes() >= 3);
});

test('pending and unavailable products never unlock ad removal', () => {
  const h = harness();
  h.state({pending: true});
  assert.equal(h.buy.textContent, 'Payment pending');
  assert.equal(h.buy.disabled, true);
  const count = h.messages.length;
  h.click('.billing-buy');
  h.state({available: false});
  h.click('.billing-buy');
  assert.equal(h.buy.disabled, true);
  assert.equal(h.window.mediaManagerPermanentAdFree, false);
  assert.equal(h.messages.length, count);
});

test('background, modal, playback and busy states block payment actions', () => {
  const h = harness();
  h.state({});
  const count = h.messages.length;
  h.document.hidden = true; h.click('.billing-buy');
  h.document.hidden = false;
  h.modal(true); h.click('.billing-restore'); h.modal(false);
  h.window.mediaManagerNativePlaybackActive = true; h.click('.billing-buy');
  h.window.mediaManagerNativePlaybackActive = false;
  h.state({busy: true, presenting: true}); h.click('.billing-restore');
  assert.equal(h.messages.length, count);
  assert.equal(h.window.mediaManagerBillingPresenting, true);
});

test('iOS restore requires a verified purchase and an unexpired 10-minute window', () => {
  const h = harness();
  h.state({});
  assert.equal(h.restore.disabled, true);
  const count = h.messages.length;
  h.click('.billing-restore');
  assert.equal(h.messages.length, count);
  h.state({owned: true, canRestore: true, restoreUntil: Date.now() + 600_000});
  assert.equal(h.restore.disabled, false);
  h.click('.billing-restore');
  assert.equal(h.messages.at(-1).type, 'billing-restore');
  h.state({owned: true, canRestore: true, restoreUntil: Date.now() - 1});
  assert.equal(h.restore.disabled, true);
  const afterRestore = h.messages.length;
  h.click('.billing-restore');
  assert.equal(h.messages.length, afterRestore);
  assert.equal(h.window.mediaManagerPermanentAdFree, true);
  h.listeners.get('visibilitychange')();
  assert.equal(h.messages.at(-1).type, 'billing-state');
});

test('late iOS bridge connects once and cleanup removes listeners and timers', () => {
  const h = harness({bridge: false});
  assert.equal(h.messages.length, 0);
  assert.equal(h.restore.disabled, true);
  h.installBridge(); h.tick(); h.tick();
  assert.equal(h.messages.length, 1);
  h.state({});
  assert.equal(h.restore.disabled, true);
  h.window.mediaManagerBillingCleanup(); h.tick();
  assert.equal(h.listeners.size, 0);
  assert.equal(h.window.mediaManagerPermanentAdFree, undefined);
  assert.equal(h.window.mediaManagerBillingPresenting, undefined);
  assert.equal(h.messages.length, 1);
});
