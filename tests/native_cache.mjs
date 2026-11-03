import assert from 'node:assert/strict';
import { connect, closeConnections } from './native_cdp.mjs';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const [chromePort, contentPort, origin, artifacts] = process.argv.slice(2);
async function until(probe, label) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    try {
      if (await probe()) return;
    } catch (error) {
      if (!/Execution context was destroyed|Cannot find context with specified id/.test(error.message)) throw error;
    }
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw Error('Timed out: ' + label);
}

try {
  const sidebar = await connect(page => page.title === 'Veluno', chromePort);
  const command = (action, payload = {}) => sidebar.evaluate(`window.ipc.postMessage(${JSON.stringify(JSON.stringify({ action, ...payload }))});true`);
  const url = origin + '/cache-recovery/';
  await command('new_tab', { url });
  const page = await connect(target => target.url === url, contentPort);
  await until(() => page.evaluate('window.fixtureReady === true'), 'initial working bootstrap');
  const tabId = await sidebar.evaluate('currentState.active_tab_id');
  const permission = await connect(target => target.title.includes('Site permission'), chromePort);
  await page.call('Runtime.evaluate', { expression: 'window.permissionResult="pending";Notification.requestPermission().then(result=>window.permissionResult=result);true', userGesture: true });
  await until(() => permission.evaluate('current?.kind === "notifications"'), 'saved permission fixture');
  await permission.evaluate('document.getElementById("allow").click()');
  await until(() => page.evaluate('window.permissionResult === "granted"'), 'permission stored');
  const otherUrl = origin.replace('127.0.0.1', 'localhost') + '/cache-recovery/';
  await command('new_tab', { url: otherUrl });
  const other = await connect(target => target.url === otherUrl, contentPort);
  await until(() => other.evaluate('window.fixtureReady === true'), 'other origin ready');
  await other.evaluate('caches.open("unrelated-origin").then(cache=>cache.put("/keep",new Response("kept")))');
  await command('switch_tab', { id: tabId });
  await page.evaluate(`(async () => {
    document.cookie = 'cache_fixture_session=kept; path=/; SameSite=Lax';
    localStorage.setItem('cache_fixture_auth', 'kept');
    const db = await new Promise((resolve, reject) => {
      const request = indexedDB.open('cache-fixture', 1);
      request.onupgradeneeded = () => request.result.createObjectStore('auth');
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    await new Promise((resolve, reject) => {
      const tx = db.transaction('auth', 'readwrite');
      tx.objectStore('auth').put('kept', 'session');
      tx.oncomplete = resolve; tx.onerror = () => reject(tx.error);
    });
    db.close();
    const cache = await caches.open('veluno-poisoned');
    await cache.put('/cache-recovery/bootstrap.js', new Response('stale bootstrap', { status: 503 }));
  })()`);
  await page.call('Page.reload');
  await until(() => page.evaluate('document.body?.dataset.state === "stalled"'), 'cached bootstrap reproduces endless startup');
  assert.equal(await page.evaluate('window.fixtureReady === true'), false);
  await until(() => sidebar.evaluate('currentState.update_status?.includes("Some page scripts failed")'), 'browser reports failed startup script');
  await command('reload');
  await until(() => page.evaluate('window.fixtureReady === true'), 'normal Reload recovers failed cached scripts');
  assert.equal(await page.evaluate('caches.has("veluno-poisoned").then(async exists => !exists || !(await (await caches.open("veluno-poisoned")).match("/cache-recovery/bootstrap.js")))'), true);
  assert.equal(await page.evaluate('document.cookie.includes("cache_fixture_session=kept")'), true);
  assert.equal(await page.evaluate('localStorage.getItem("cache_fixture_auth")'), 'kept');
  assert.equal(await page.evaluate('Notification.permission'), 'granted');
  assert.equal(await other.evaluate('caches.open("unrelated-origin").then(cache=>cache.match("/keep")).then(response=>response.text())'), 'kept');
  assert.equal(await page.evaluate(`new Promise((resolve, reject) => {
    const request = indexedDB.open('cache-fixture', 1);
    request.onsuccess = () => {
      const db = request.result, read = db.transaction('auth').objectStore('auth').get('session');
      read.onsuccess = () => { resolve(read.result); db.close(); };
      read.onerror = () => reject(read.error);
    };
    request.onerror = () => reject(request.error);
  })`), 'kept');
  await page.evaluate('window.reloadMarker="kept";window.ipc.postMessage(JSON.stringify({action:"reload_fresh"}));true');
  await new Promise(resolve => setTimeout(resolve, 200));
  assert.equal(await page.evaluate('window.reloadMarker'), 'kept', 'page IPC cannot trigger a fresh reload');
  await page.evaluate('caches.open("healthy-cache").then(cache=>cache.put("/keep",new Response("kept")))');
  await command('reload');
  await until(() => page.evaluate('!window.reloadMarker && window.fixtureReady === true'), 'ordinary reload on a healthy page');
  assert.equal(await page.evaluate('caches.has("healthy-cache")'), true, 'healthy reloads retain resource caches');
  await page.evaluate('window.reloadMarker="keyboard";true');
  await page.call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'R', modifiers: 10, windowsVirtualKeyCode: 82 });
  await page.call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'R', modifiers: 10, windowsVirtualKeyCode: 82 });
  await until(() => page.evaluate('!window.reloadMarker && window.fixtureReady === true'), 'native Ctrl+Shift+R');
  assert.equal(await page.evaluate('caches.has("healthy-cache")'), false, 'explicit fresh reload clears cached resources');
  await until(() => sidebar.evaluate('!document.getElementById("app-reload-fresh").disabled'), 'fresh reload menu enabled');
  await sidebar.evaluate('document.getElementById("app-menu").open=true;true');
  const menuLayout = await sidebar.evaluate(`(() => {
    const menu=document.querySelector('#app-menu .menu'), button=document.getElementById('app-reload-fresh');
    const m=menu.getBoundingClientRect(), b=button.getBoundingClientRect();
    return { fits: menu.scrollWidth <= menu.clientWidth && b.left >= m.left && b.right <= m.right, label: button.innerText };
  })()`);
  assert.equal(menuLayout.fits, true, 'recovery action fits the existing sidebar menu');
  if (artifacts) {
    const screenshot = await sidebar.call('Page.captureScreenshot', { format: 'png' });
    await writeFile(join(artifacts, 'fresh-reload-menu.png'), Buffer.from(screenshot.data, 'base64'));
  }
  await page.evaluate('window.reloadMarker="menu";true');
  await sidebar.evaluate('document.getElementById("app-reload-fresh").click();true');
  await until(() => page.evaluate('!window.reloadMarker && window.fixtureReady === true'), 'fresh reload menu action');
  const watchUrl = origin + '/truncated-watch';
  await command('new_tab', { url: watchUrl });
  const watchPage = await connect(target => target.url === watchUrl, contentPort);
  await until(() => watchPage.evaluate('window.watchReady === true && !!document.getElementById("movie_player")'), 'truncated HTTP 200 watch skeleton recovers automatically');
  await until(() => sidebar.evaluate('!currentState.tabs.find(tab => tab.id === currentState.active_tab_id)?.error && !currentState.is_loading'), 'recovered watch tab clears loading and error state');
  console.log('Cached-script recovery, truncated watch-stream recovery, ordinary Reload, Ctrl+Shift+R, and the app menu passed; site data, permissions, and other origins survive.');
} finally {
  closeConnections();
}
