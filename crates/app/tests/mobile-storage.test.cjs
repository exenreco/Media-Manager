const { test } = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const script = fs.readFileSync(path.join(__dirname, '../src/mobile-storage.js'), 'utf8');

async function bridge(native) {
  let listener, timeout, reply, sent;
  const window = {
    addEventListener: (_, callback) => { listener = callback; },
    removeEventListener: () => { listener = undefined; },
  };
  if (native) window.MediaManagerStorage = { postMessage: text => { sent = JSON.parse(text); } };
  await vm.runInNewContext(`(async () => { ${script} })()`, {
    window, Date, Math,
    setTimeout: callback => { timeout = callback; return 1; }, clearTimeout: () => {},
    dioxus: { recv: async () => ({ op: 'pick' }), send: value => { reply = value; } },
  });
  return { get reply() { return reply; }, get listener() { return listener; }, sent,
    respond: detail => listener({ detail }), expire: () => timeout() };
}

test('folder response is correlated and listener is removed', async () => {
  const request = await bridge(true);
  request.respond({ id: 'another-request', root: 'wrong' });
  assert.equal(request.reply, undefined);
  request.respond({ id: request.sent.id, root: 'granted-folder' });
  assert.equal(request.reply.root, 'granted-folder');
  assert.equal(request.listener, undefined);
});
test('native cancellation and revoked-permission errors reach Rust', async () => {
  for (const error of ['Folder selection cancelled', 'Folder permission expired']) {
    const request = await bridge(true);
    request.respond({ id: request.sent.id, error });
    assert.equal(request.reply.error, error);
  }
});
test('missing bridge fails immediately and cleans up', async () => {
  const request = await bridge(false);
  assert.match(request.reply.error, /bridge unavailable/);
  assert.equal(request.listener, undefined);
});
test('timeout warns about possible mutations and cleans up', async () => {
  const request = await bridge(true); request.expire();
  assert.match(request.reply.error, /Inspect your library/);
  assert.equal(request.listener, undefined);
});
