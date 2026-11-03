import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { connect, closeConnections, writeDiagnostics } from './native_cdp.mjs';

const [chromePort, contentPort, origin, artifacts] = process.argv.slice(2);
async function until(probe, label) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    if (await probe()) return;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw Error('Timed out: ' + label);
}

try {
  const sidebar = await connect(page => page.title === 'Veluno', chromePort);
  if (process.argv.includes('--restored')) {
    await until(()=>sidebar.evaluate('currentState?.dismissed_update_version==="999.0.0"'),'dismissal restored from disk');
    await sidebar.evaluate('updateState({...currentState,update_available:{version:"999.0.0"}});true');
    assert.equal(await sidebar.evaluate('document.getElementById("update-banner").hidden'),true,'dismissed release stays hidden after restart');
    console.log('Update dismissal survives a real app restart.');
  } else {
    await until(()=>sidebar.evaluate('!!currentState?.tabs?.length&&!currentState.is_loading'),'sidebar ready');
    assert.equal(await sidebar.evaluate('updateState({...currentState,update_available:{version:"999.0.0"}});document.getElementById("update-banner").hidden'),false);
    if (artifacts) {
      const shot = await sidebar.call('Page.captureScreenshot', { format: 'png' });
      await writeFile(join(artifacts, 'update-notice.png'), Buffer.from(shot.data, 'base64'));
    }
    assert.equal(await sidebar.evaluate('!!document.getElementById("dismiss-update")'),true,'update notice can be dismissed');
    await sidebar.evaluate('updateState({...currentState,update_available:{version:"999.0.0"}});document.getElementById("dismiss-update").click();true');
    await until(()=>sidebar.evaluate('currentState.dismissed_update_version==="999.0.0"'),'dismissal acknowledged');
    assert.equal(await sidebar.evaluate('updateState({...currentState,update_available:{version:"999.0.0"}});document.getElementById("update-banner").hidden'),true,'same release does not reappear on state changes');
    assert.equal(await sidebar.evaluate('updateState({...currentState,update_available:{version:"999.0.1"}});document.getElementById("update-banner").hidden'),false,'newer releases can notify');
    await sidebar.evaluate('updateState({...currentState,update_available:null});true');
    const command = (action, payload = {}) => sidebar.evaluate(`Veluno.send(${JSON.stringify(action)},${JSON.stringify(payload)});true`);
    await command('new_tab', { url: origin + '/download-source' });
    const source = await connect(page => page.url === origin + '/download-source', contentPort);
    await until(() => source.evaluate(`location.href===${JSON.stringify(origin + '/download-source')}&&document.readyState==="complete"`), 'source page');
    await until(() => sidebar.evaluate(`currentState.tabs.some(t=>t.url===${JSON.stringify(origin + '/download-source')}&&!t.is_loading)`), 'source committed');
    const sourceId = await sidebar.evaluate('currentState.active_tab_id');
    await source.evaluate(`document.getElementById('note').value='Keep my draft';true`);
    await command('new_tab', { url: origin + '/other-page' });
    await until(() => sidebar.evaluate(`currentState.tabs.some(t=>t.url==='${origin}/other-page'&&!t.is_loading)`), 'other page committed');
    await command('switch_tab', { id: sourceId });
    await until(() => sidebar.evaluate(`currentState.active_tab_id===${sourceId}`), 'return to source');
    await fetch(origin + '/hold-downloads');
    const before = await sidebar.evaluate('currentState.tabs.length');
    await sidebar.evaluate('document.getElementById("toast").hidden=true;true');
    await source.evaluate(`window.open('${origin}/slow-download','_blank');true`);
    await until(() => sidebar.evaluate('currentState.active_downloads===1'), 'real download started');
    console.log('Download-only tab state:', await sidebar.evaluate('({tabs:currentState.tabs.length,active:currentState.tabs.find(t=>t.id===currentState.active_tab_id)?.url,downloads:currentState.active_downloads})'));
    await until(() => sidebar.evaluate(`currentState.tabs.length===${before}&&currentState.active_tab_id===${sourceId}`), 'empty download tab removed while transfer is active');
    assert.equal(await source.evaluate('document.getElementById("note").value'), 'Keep my draft');
    assert.equal(await sidebar.evaluate('document.getElementById("toast").hidden'),true,'automatic download cleanup must not offer Undo');
    assert.equal(await sidebar.evaluate(`currentState.closed_tabs.some(t=>t.url==='${origin}/slow-download')`), false, 'download should not be reopened by Undo');
    await command('open_downloads');
    await until(()=>sidebar.evaluate('currentState.download_items?.some(item=>item.status==="downloading"&&item.received>0&&item.received<item.total)'), 'partial byte progress delivered');
    if (artifacts) {
      const shot = await sidebar.call('Page.captureScreenshot', { format: 'png' });
      await writeFile(join(artifacts, 'download-progress.png'), Buffer.from(shot.data, 'base64'));
    }
    assert.equal(await sidebar.evaluate('currentState.download_items?.some(item=>item.status==="downloading"&&item.total===65536)'), true, 'active download exposes file and byte progress');
    assert.equal(await sidebar.evaluate('document.getElementById("downloads-panel")?.hidden'), false, 'Downloads panel is visible even when its owner tab is hidden');
    await fetch(origin + '/release-downloads');
    await until(() => sidebar.evaluate('currentState.active_downloads===0'), 'detached download completes');
    assert.equal((await readFile(join(artifacts, 'profile', 'smoke-download.txt'))).length, 65536, 'complete download contents');
    await command('open_downloads');
    await until(()=>sidebar.evaluate('currentState.download_items?.some(item=>item.status==="complete"&&item.received===65536)'), 'completed file retained with byte count');
    assert.match(await sidebar.evaluate('document.getElementById("download-items").textContent'), /Complete.*64.0 KB.*Show in folder/);
    await command('close_downloads');
    await until(()=>sidebar.evaluate('document.getElementById("downloads-panel").hidden'),'panel closed');
    await command('open_downloads');
    await until(()=>sidebar.evaluate('!document.getElementById("downloads-panel").hidden'),'completed downloads reopened');
    assert.equal(await sidebar.evaluate('currentState.update_status?.startsWith("Download complete")'), true, 'completed download remains accessible');

    await fetch(origin + '/hold-downloads');
    await source.evaluate(`document.getElementById('download').href='${origin}/slow-download';document.getElementById('download').click();true`);
    await until(() => sidebar.evaluate('currentState.active_downloads===1'), 'same-tab download starts');
    await until(() => sidebar.evaluate(`currentState.tabs.length===${before}&&currentState.tabs.find(t=>t.id===${sourceId}).url==='${origin}/download-source'&&!currentState.tabs.find(t=>t.id===${sourceId}).is_loading`), 'existing document remains with correct address');
    assert.equal(await source.evaluate('document.getElementById("note").value'), 'Keep my draft');
    await fetch(origin + '/release-downloads');
    await until(() => sidebar.evaluate('currentState.active_downloads===0'), 'same-tab download completes');

    await command('new_tab', { url: origin + '/download' });
    await until(async () => {
      const contents = await readFile(join(artifacts, 'profile', 'smoke-download.txt')).catch(() => Buffer.alloc(0));
      return contents.toString() === 'Veluno download fixture\n' && await sidebar.evaluate(`currentState.tabs.length===${before}&&currentState.active_tab_id===${sourceId}`);
    }, 'fast direct download completed and tab removed');
    assert.equal(await source.evaluate('document.getElementById("note").value'), 'Keep my draft');
    await until(() => sidebar.evaluate('currentState.active_downloads===0'), 'fast download completes');
    await fetch(origin + '/hold-downloads');
    await command('new_tab', { url: origin + '/redirect-download' });
    await until(() => sidebar.evaluate(`currentState.active_downloads===1&&currentState.tabs.length===${before}&&currentState.active_tab_id===${sourceId}`), 'redirected download tab removed');
    await fetch(origin + '/release-downloads');
    await until(() => sidebar.evaluate('currentState.active_downloads===0'), 'redirected download completes');

    await command('new_tab', { url: 'veluno://newtab' });
    await until(() => sidebar.evaluate('currentState.tabs.find(t=>t.id===currentState.active_tab_id)?.url==="veluno://newtab"&&!currentState.is_loading'), 'Home document committed');
    const homeId = await sidebar.evaluate('currentState.active_tab_id');
    await fetch(origin + '/hold-downloads');
    await command('navigate', { url: origin + '/slow-download' });
    await until(() => sidebar.evaluate(`currentState.active_downloads===1&&currentState.tabs.find(t=>t.id===${homeId})?.url==='veluno://newtab'&&!currentState.is_loading`), 'same-tab download keeps Home');
    await fetch(origin + '/release-downloads');
    await until(() => sidebar.evaluate('currentState.active_downloads===0'), 'Home download completes');
    await command('switch_tab', { id: sourceId });
    await until(()=>sidebar.evaluate(`currentState.active_tab_id===${sourceId}`),'source selected after Home download');
    const targets=await (await fetch(`http://127.0.0.1:${contentPort}/json/list`)).json();
    const awake=await sidebar.evaluate('currentState.tabs.filter(t=>!t.is_sleeping).length');
    assert.ok(targets.filter(t=>t.type==='page').length<=awake+1,'completed downloads retain at most one extra view');
    await until(async()=>{
      try {
        const session=JSON.parse(await readFile(join(artifacts,'profile','session.json'),'utf8'));
        return !session.tabs.some(tab=>['/download','/slow-download','/redirect-download'].some(path=>tab.url===origin+path));
      } catch(error) { if(error.code==='ENOENT')return false;throw error; }
    },'download-only URLs removed from saved session');
    await fetch(origin + '/hold-downloads');
    await command('new_tab', { url: origin + '/slow-download' });
    await until(()=>sidebar.evaluate('currentState.active_downloads===1'), 'cancellable download starts');
    await sidebar.evaluate('document.querySelector("#download-items .download-item button").click();true');
    await until(()=>sidebar.evaluate('currentState.download_items?.at(-1)?.status==="cancelled"&&currentState.active_downloads===0'), 'native cancellation stops the transfer');
    await fetch(origin + '/release-downloads');
    assert.match(await sidebar.evaluate('document.getElementById("download-items").textContent'), /Cancelled/);
    if (artifacts) {
      const shot = await sidebar.call('Page.captureScreenshot', { format: 'png' });
      await writeFile(join(artifacts, 'download-tab-after.png'), Buffer.from(shot.data, 'base64'));
    }
    console.log('Download-only tabs disappear without cancelling transfers; the source draft, address, and completed download access survive.');
  }
} catch (error) {
  if (artifacts) {
    try {
      const sidebar = await connect(page => page.title === 'Veluno', chromePort);
      await writeFile(join(artifacts, 'download-state.json'), JSON.stringify(await sidebar.evaluate('({items:currentState.download_items,active:currentState.active_downloads})'), null, 2));
    } catch {}
    await writeDiagnostics([chromePort, contentPort], artifacts, error, true);
  }
  throw error;
} finally {
  await fetch(origin + '/release-downloads').catch(() => {});
  closeConnections();
}