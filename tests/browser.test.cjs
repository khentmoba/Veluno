const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const messages = [];
let keydown;
let click;
let resourceError;
const window = { ipc: { postMessage: json => messages.push(JSON.parse(json)) }, addEventListener: (type, handler) => { if (type === 'keydown') keydown = handler; else if (type === 'click') click = handler; else if (type === 'error') resourceError = handler; } };
window.top = window;
const navigator = { serviceWorker: { controller: {} } };
const location = { href: 'https://example.com/meeting', origin: 'https://example.com' };
const context = vm.createContext({ window, navigator, location, URL });
vm.runInContext(fs.readFileSync('src/browser.js', 'utf8'), context);
const resolve = window.Veluno.resolveAddress;
for (const [input, expected] of [
  ['', null], ['  ', null], ['veluno://newtab', 'veluno://newtab'],
  ['felaris://newtab', 'veluno://newtab'], ['veluno://settings', 'veluno://settings'],
  ['felaris://history', 'veluno://history'],
  ['example.com/docs', 'https://example.com/docs'], ['HTTPS://example.com', 'HTTPS://example.com'],
  ['localhost:5173', 'http://localhost:5173'], ['127.0.0.1:8080/a', 'http://127.0.0.1:8080/a'],
  ['[::1]:8080', 'http://[::1]:8080'], ['file:///C:/test.html', 'file:///C:/test.html'],
  ['a.b search', 'https://search.brave.com/search?q=a.b%20search'],
  ['rust & café #東京', 'https://search.brave.com/search?q=rust%20%26%20caf%C3%A9%20%23%E6%9D%B1%E4%BA%AC'],
  ['javascript:alert(1)', 'https://search.brave.com/search?q=javascript%3Aalert(1)'],
]) assert.equal(resolve(input), expected, input);
for (const [input, expected] of [
  ['@yt lofi beats', 'https://www.youtube.com/results?search_query=lofi%20beats'],
  ['@GH  veluno ', 'https://github.com/search?q=veluno'],
  ['@w', 'https://en.wikipedia.org/'],
  ['@nope cats', 'https://search.brave.com/search?q=%40nope%20cats'],
  ['@constructor x', 'https://search.brave.com/search?q=%40constructor%20x'],
  ['email@yt.com', 'https://email@yt.com'],
]) assert.equal(resolve(input), expected, input);
context.Veluno=window.Veluno;
const suggest=window.Veluno.suggestions;
const sources={tabs:[{id:1,title:'YouTube tab',url:'https://youtube.com'}],bookmarks:[{title:'Duplicate',url:'https://youtube.com/'}],shortcuts:[{title:'YouTube',url:'https://youtube.com'},{title:'GitHub',url:'https://github.com'}]};
const suggestions=suggest('you',sources,[{title:'YouTube Studio',url:'https://studio.youtube.com'}]);
assert.equal(suggestions[0].id,1,'An obvious open-tab match leads the search fallback');
assert.equal(suggestions[1].title,'YouTube Studio');
assert.equal(suggestions[2].url,'https://search.brave.com/search?q=you');
assert.equal(suggestions.length,3,'Deduplicate suggestions');
const rankedSources={tabs:[{id:2,title:'Reading about GitHub',url:'https://news.example/article'}],bookmarks:[{title:'GitHub',url:'https://github.com'}]};
assert.equal(suggest('github',rankedSources)[0].url,'https://github.com','Strong bookmarks outrank incidental tab matches');
assert.equal(suggest('github actions tutorial',rankedSources)[0].search,true,'General queries still search');
assert.equal(suggest('git',{},[{title:'GitHub',url:'https://github.com'}])[0].history,true);
assert.equal(suggest('youtube',{shortcuts:sources.shortcuts})[0].shortcut,true);
assert.equal(suggest('rust async',{},[{title:'Async Rust guide',url:'https://example.com/async-rust'}])[1].history,true,'Match all words in any order');
assert.equal(suggest('hub',rankedSources)[0].search,true,'Weak substring matches do not hijack Enter');
assert.equal(suggest('g',rankedSources)[0].search,true,'One letter is ambiguous');
const keywordOption=suggest('@yt cats',sources)[0];
assert.equal(keywordOption.url,'https://www.youtube.com/results?search_query=cats','Keyword searches lead suggestions');
assert.equal(keywordOption.detail,'Search YouTube');
assert.equal(suggest('@gh',sources)[0].detail,'Open GitHub');
assert.equal(suggest('x',{shortcuts:[{title:'X',url:'https://x.com'}]})[0].shortcut,true);
assert.equal(suggest('https://github.com/new',rankedSources)[0].url,'https://github.com/new','Explicit addresses stay first');
const crowded={tabs:Array.from({length:12},(_,i)=>({id:i+10,title:'GitHub article '+i,url:'https://example.com/'+i})),bookmarks:[{title:'GitHub',url:'https://github.com'}]};
assert.equal(suggest('',crowded)[6].bookmark,true,'Empty input still leaves room after six tabs for saved sites');
assert.equal(suggest('github',crowded)[0].bookmark,true,'Rank before truncating');
assert.equal(suggest('github',crowded).length,8);
assert.ok(suggest('github',crowded).some(option=>option.search),'Keep an explicit web search choice');
assert.equal(suggest('',{shortcuts:sources.shortcuts})[0].title,'YouTube');
assert.equal(suggest('no match',sources).length,1);
assert.equal(suggest('github.com',{})[0].url,'https://github.com');
assert.equal(suggest('',{shortcuts:Array.from({length:12},(_,i)=>({title:String(i),url:'https://example.com/'+i}))}).length,8);
const changing={tabs:[{id:70,title:'GitHub',url:'https://github.com',workspace_id:1,favicon:'old'}],workspaces:[{id:1,name:'Work'},{id:2,name:'Personal'}]};
assert.equal(suggest('git',changing)[0].id,70);
Object.assign(changing.tabs[0],{title:'GitLab',url:'https://gitlab.com',workspace_id:2,favicon:'new'});
const changed=suggest('gitlab',changing)[0];
assert.equal(changed.title,'GitLab','URL and title edits invalidate cached search metadata');
assert.equal(changed.favicon,'new','Suggestion display fields stay live');
assert.ok(changed.detail.includes('Personal'),'Workspace changes stay live');
assert.equal(suggest('github',changing).length,1,'Changed URLs do not leave stale results');
changing.tabs[0].title='Updated project';
assert.equal(suggest('updated',changing)[0].title,'Updated project','Title-only edits invalidate metadata');
const sameUrl=[{title:'Unrelated',url:'https://same.example'},{title:'GitHub',url:'https://same.example/'}];
assert.equal(suggest('github',{},sameUrl)[0].history,true,'A nonmatching duplicate must not hide a matching title');
const historyWithFavicon=[{title:'Experiential',url:'https://www.experientiallabs.ai/',favicon:'https://www.experientiallabs.ai/icon.svg'}];
const historyOption=suggest('experiential',{},historyWithFavicon)[0];
assert.equal(historyOption.history,true);
assert.equal(historyOption.favicon,'https://www.experientiallabs.ai/icon.svg','History suggestions preserve saved favicons');
const ties=Array.from({length:40},(_,id)=>({id,title:'Guide '+id,url:'https://example.com/'+id}));
assert.deepEqual(Array.from(suggest('guide',{tabs:ties}).filter(option=>!option.search),option=>option.id),[0,1,2,3,4,5,6],'Equal scores preserve source order at the result limit');
const saturated=Array.from({length:7},(_,id)=>({id,title:'GitHub '+id,url:'https://github.com/'+id}));
const unread={get url(){throw Error('A full list of maximum-score matches must not inspect later entries');}};
assert.deepEqual(Array.from(suggest('github',{tabs:[...saturated,unread]},[unread]).filter(option=>!option.search),option=>option.id),[0,1,2,3,4,5,6]);
assert.equal(suggest('',{bookmarks:[...ties.slice(0,8),unread]},[unread]).length,8,'Empty input stops once eight unique entries fill the list');
const lateBest={tabs:ties.map(entry=>({...entry,title:'GitHub guide '+entry.id})),bookmarks:saturated};
assert.deepEqual(Array.from(suggest('github',lateBest).filter(option=>!option.search),option=>option.url),saturated.map(entry=>entry.url),'Only maximum-score results permit early termination');
const duplicateTop=[...saturated.map(entry=>({...entry,url:saturated[0].url})),...saturated.slice(1)];
assert.equal(suggest('github',{bookmarks:duplicateTop},[unread]).length,8,'Duplicates do not count toward a full result list');
suggest('overflow',{},Array.from({length:5000},(_,i)=>({title:'Overflow '+i,url:'https://overflow.test/'+i})));
assert.equal(suggest('gitlab',changing)[0].id,70,'Eviction changes speed only, never suggestion results');
assert.equal(suggest('gitlab',{tabs:JSON.parse(JSON.stringify(changing.tabs))})[0].title,'Updated project','Replaced IPC objects retain current values');
function press(key, modifiers = {}) {
  let prevented = false;
  keydown({ key, isTrusted: true, ...modifiers, preventDefault() { prevented = true; } });
  assert.ok(prevented);
  const { shortcut_token, ...message } = messages.pop();
  assert.equal(shortcut_token, '__SHORTCUT_TOKEN__');
  return message;
}
assert.deepEqual(press('T', { ctrlKey: true, shiftKey: true }), { action: 'restore_closed_tab' });
assert.deepEqual(press('S', { ctrlKey: true, shiftKey: true }), { action: 'sleep_inactive_tabs' });
assert.deepEqual(press('t', { ctrlKey: true }), { action: 'new_tab' });
assert.deepEqual(press('B', { ctrlKey: true, shiftKey: true }), { action: 'toggle_sidebar' });
assert.deepEqual(press('N', { ctrlKey: true, shiftKey: true }), { action: 'new_private_tab' });
assert.deepEqual(press('d', { ctrlKey: true }), { action: 'toggle_bookmark' });
assert.deepEqual(press('l', { ctrlKey: true }), { action: 'focus_address' });
assert.deepEqual(press('j', { ctrlKey: true }), { action: 'open_downloads' });
assert.deepEqual(press('w', { ctrlKey: true }), { action: 'close_active_tab' });
assert.deepEqual(press('Tab', { ctrlKey: true, shiftKey: true }), { action: 'cycle_tab', backwards: true });
assert.deepEqual(press('Tab', { ctrlKey: true, shiftKey: false }), { action: 'cycle_tab', backwards: false });
assert.deepEqual(press('PageDown', { ctrlKey: true }), { action: 'cycle_tab', backwards: false });
assert.deepEqual(press('PageUp', { ctrlKey: true }), { action: 'cycle_tab', backwards: true });
assert.deepEqual(press('=', { ctrlKey: true }), { action: 'zoom', delta: 10 });
assert.deepEqual(press('-', { ctrlKey: true }), { action: 'zoom', delta: -10 });
assert.deepEqual(press('0', { ctrlKey: true }), { action: 'zoom', delta: 0 });
assert.deepEqual(press('2', { altKey: true }), { action: 'switch_profile', index: 2 });
assert.deepEqual(press('F5'), { action: 'reload' });
assert.deepEqual(press('R', { ctrlKey: true, shiftKey: true }), { action: 'reload_fresh' });
assert.deepEqual(press('F5', { ctrlKey: true }), { action: 'reload_fresh' });
assert.deepEqual(press('F11'), { action: 'toggle_fullscreen' });
assert.deepEqual(press('F9'), { action: 'toggle_reader' });
assert.deepEqual(press('Delete', { ctrlKey: true, shiftKey: true }), { action: 'open_settings', section: 'privacy' });
keydown({ key: 'Delete', ctrlKey: true, isTrusted: true, preventDefault() { throw Error('Ctrl+Delete deletes a word in the page'); } });
const link = href => ({ closest: selector => selector === 'a[href]' ? { href } : null });
function altClick(href, modifiers = {}) {
  let prevented = false;
  click({ isTrusted: true, button: 0, altKey: true, target: link(href), ...modifiers, preventDefault() { prevented = true; }, stopPropagation() {} });
  return prevented;
}
assert.ok(altClick('/docs?a=1'), 'Alt+Click on a link opens Glance instead of downloading it');
assert.deepEqual(messages.pop(), { action: 'glance', url: 'https://example.com/docs?a=1', shortcut_token: '__SHORTCUT_TOKEN__' });
assert.equal(altClick('mailto:a@example.com'), false, 'Only web links open in Glance');
assert.equal(altClick('/docs', { altKey: false }), false, 'Plain clicks belong to the page');
assert.equal(altClick('/docs', { ctrlKey: true }), false, 'Ctrl+Alt+Click is left to the page');
assert.equal(altClick('/docs', { isTrusted: false }), false, 'Synthetic clicks cannot open Glance');
assert.equal(messages.length, 0);
new vm.Script(fs.readFileSync('src/reader.js', 'utf8'), { filename: 'src/reader.js' });
vm.runInContext("Object.prototype.toJSON = function () { throw Error('page serialization hook'); }", context);
assert.deepEqual(press('t', { ctrlKey: true }), { action: 'new_tab' });
keydown({ key: 't', ctrlKey: true, isTrusted: true, isComposing: true });
keydown({ key: 't', ctrlKey: true, isTrusted: false });
keydown({ key: 's', ctrlKey: true, isTrusted: true, preventDefault() { throw Error('Ctrl+S belongs to the page'); } });
assert.equal(messages.length, 0);
resourceError({ isTrusted: false, target: { tagName: 'SCRIPT', src: 'https://example.com/app.js' } });
resourceError({ isTrusted: true, target: { tagName: 'IMG', src: 'https://example.com/pixel' } });
resourceError({ isTrusted: true, target: { tagName: 'SCRIPT', src: 'https://tracker.test/ad.js' } });
assert.equal(messages.length, 0, 'synthetic errors, images, and third-party failures do not trigger cache recovery');
navigator.serviceWorker.controller = null;
resourceError({ isTrusted: true, target: { tagName: 'SCRIPT', src: 'https://example.com/app.js' } });
assert.equal(messages.length, 0, 'ordinary pages without a service worker keep normal reload behavior');
navigator.serviceWorker.controller = {};
resourceError({ isTrusted: true, target: { tagName: 'SCRIPT', src: 'https://example.com/app.js' } });
assert.deepEqual(messages.pop(), { action: 'resource_load_failed', document_url: location.href, resource_url: 'https://example.com/app.js', shortcut_token: '__SHORTCUT_TOKEN__' });
resourceError({ isTrusted: true, target: { tagName: 'SCRIPT', src: 'https://example.com/another.js' } });
assert.equal(messages.length, 0, 'script failures report once per document');
for (const file of ['src/ui.html', 'src/start_page.html', 'src/new_tab.html', 'src/topbar.html', 'src/popup.html', 'src/permission.html', 'src/history.html', 'src/settings.html']) {
  const html = fs.readFileSync(file, 'utf8').replace('__COMMON_SCRIPT__', fs.readFileSync('src/browser.js', 'utf8')).replace('__SHORTCUTS__', '[]');
  for (const script of html.matchAll(/<script>([\s\S]*?)<\/script>/g)) new vm.Script(script[1], { filename: file });
  assert.ok(!html.includes('__COMMON_SCRIPT__'));
}
const uiHtml = fs.readFileSync('src/ui.html', 'utf8');
assert.ok(/submitAddress\(true\)/.test(uiHtml), 'Ctrl+Enter opens the address in a new tab');
assert.ok(uiHtml.includes('submitAddress(newTab=false)'), 'the address submit knows about new tabs');
const startPage = fs.readFileSync('src/start_page.html', 'utf8');
assert.ok(/ctrlKey\|\|event\.altKey\) Veluno\.send\('new_tab'/.test(startPage), 'Ctrl+Enter on the start page opens a new tab');
assert.ok(uiHtml.includes('MB'), 'src/ui.html must format download size in MB');
assert.ok(!uiHtml.includes("toFixed(1)} KB"), 'src/ui.html must not use KB for download progress');
assert.ok(uiHtml.includes('<symbol id="veluno"'), 'src/ui.html must define #veluno logo symbol');
assert.ok(uiHtml.includes("icon('veluno')"), 'src/ui.html must render #veluno icon for veluno://newtab');
const expectedAssets = [
  'favicon.svg',
  'icon.ico',
  'icon_64.rgba',
  'logo-monochrome.svg',
  'logo-wordmark.svg',
  'logo.svg',
  'veluno-icon.png',
  'veluno-logo.png',
  'veluno-logo.svg',
];
assert.deepEqual(fs.readdirSync('assets').sort(), expectedAssets, 'assets/ must contain only canonical branding files');
for (const required of ['scripts/build.py', 'scripts/generate-icons.py', 'scripts/notify_discord.py', '.agents/skills/discord-release/SKILL.md']) {
  assert.ok(fs.existsSync(required), `Missing tracked release file: ${required}`);
}
assert.ok(
  fs.readFileSync('scripts/notify_discord.py', 'utf8').includes('LOCALAPPDATA'),
  'scripts/notify_discord.py must support LOCALAPPDATA/Veluno/discord_webhook.txt fallback across worktrees',
);
console.log('Address resolution, shortcut routing, asset hygiene, and embedded script syntax passed.');
require('./sleep.test.cjs');
