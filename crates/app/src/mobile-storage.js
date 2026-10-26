const command = await dioxus.recv();
const id = (globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random()}`);
command.id = id;
const listener = event => {
    if (event.detail?.id !== id) return;
    clearTimeout(timer); window.removeEventListener('media-manager-storage', listener);
    dioxus.send(event.detail);
};
const timer = setTimeout(() => { window.removeEventListener('media-manager-storage', listener); dioxus.send({error:'Storage request timed out. Inspect your library before retrying.'}); }, 300000);
window.addEventListener('media-manager-storage', listener);
const bridge = window.MediaManagerStorage || window.webkit?.messageHandlers?.MediaManagerStorage;
if (bridge) {
    try { bridge.postMessage(JSON.stringify(command)); }
    catch (_) { clearTimeout(timer); window.removeEventListener('media-manager-storage', listener); dioxus.send({error:'Could not contact native storage. Select the folder again.'}); }
}
else { clearTimeout(timer); window.removeEventListener('media-manager-storage', listener); dioxus.send({error:'Native storage bridge unavailable. Rebuild the mobile app.'}); }
