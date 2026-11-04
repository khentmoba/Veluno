(() => {
  if (window.Veluno) return;
  function send(action, payload = {}) {
    window.ipc?.postMessage(JSON.stringify({ action, ...payload }));
  }
  // Search keywords: '@yt cats' searches YouTube; a bare '@yt' opens the site.
  const keywords = {
    yt: ['YouTube', 'https://www.youtube.com/results?search_query=', 'https://www.youtube.com/'],
    gh: ['GitHub', 'https://github.com/search?q=', 'https://github.com/'],
    w: ['Wikipedia', 'https://en.wikipedia.org/w/index.php?search=', 'https://en.wikipedia.org/'],
    r: ['Reddit', 'https://www.reddit.com/search/?q=', 'https://www.reddit.com/'],
    maps: ['Maps', 'https://www.openstreetmap.org/search?query=', 'https://www.openstreetmap.org/'],
    g: ['Google', 'https://www.google.com/search?q=', 'https://www.google.com/'],
    b: ['Bing', 'https://www.bing.com/search?q=', 'https://www.bing.com/'],
    ddg: ['DuckDuckGo', 'https://duckduckgo.com/?q=', 'https://duckduckgo.com/'],
    brave: ['Brave Search', 'https://search.brave.com/search?q=', 'https://search.brave.com/'],
  };
  function keyword(value) {
    const match = /^@([a-z]+)(?:\s+([\s\S]*))?$/i.exec(value.trim());
    const entry = match && Object.hasOwn(keywords, match[1].toLowerCase()) && keywords[match[1].toLowerCase()];
    if (!entry) return null;
    const query = (match[2] || '').trim();
    return { name: entry[0], query, url: query ? entry[1] + encodeURIComponent(query) : entry[2] };
  }
  function resolveAddress(value) {
    const text = value.trim();
    if (!text) return null;
    const shortcut = keyword(text);
    if (shortcut) return shortcut.url;
    const internal=text.startsWith('felaris://')?'veluno://'+text.slice(10):text;
    if (['veluno://newtab','veluno://blank','veluno://settings','veluno://history'].includes(internal)) return internal;
    if (/^(https?:\/\/|file:\/\/)/i.test(text)) return text;
    if (!/\s/.test(text) && /^(localhost(?=[:/]|$)|\[[0-9a-f:]+\](?=[:/]|$)|[^/:]+\.[^/:]+)(:\d+)?([/?#].*)?$/i.test(text)) {
      return (/^(localhost|127\.0\.0\.1|\[::1\])(?=[:/]|$)/i.test(text) ? 'http://' : 'https://') + text;
    }
    return window.Veluno.searchUrl + encodeURIComponent(text);
  }
  const suggestionMetadata=new Map();
  function metadata(entry) {
    const title=entry.title||entry.url;
    let data=suggestionMetadata.get(entry.url);
    if(!data||data.title!==title) {
     const url=entry.url.toLowerCase();
     const lowerTitle=title.toLowerCase();
     data={title,key:url.replace(/\/+$/,''),lowerTitle,text:lowerTitle+' '+url};
     if(suggestionMetadata.size>=4096) suggestionMetadata.clear();
     suggestionMetadata.set(entry.url,data);
    }
    return data;
  }
  function suggestions(value,state={},historyEntries=[]) {
    const query=value.trim();
    const needle=query.toLowerCase();
    const target=Veluno.resolveAddress(query);
    const options=[];
    const seen=new Set();
    const norm=u=>(u||'').replace(/\/+$/,'').toLowerCase();
    const words=needle.split(/\s+/);
    const limit=target?7:8;
    const maxScore=needle?100:0;
    const complete=()=>options.length===limit&&options[limit-1].score===maxScore;
    function relevance(data,url) {
     if(!needle) return 0;
     if(!words.every(word=>data.text.includes(word))) return -1;
     if(data.host===undefined) {
      data.host='';
      try { data.host=new URL(url).hostname.replace(/^www\./,'').toLowerCase(); } catch {}
      data.site=data.host.split('.')[0];
     }
     if(needle===data.host||needle===data.site) return 100;
     if(needle===data.lowerTitle) return 95;
     if(needle.length>=2&&data.host.startsWith(needle)) return 90;
     if(needle.length>=2&&data.lowerTitle.startsWith(needle)) return 80;
     return 40;
    }
    function add(entry,kind) {
     if(!entry?.url) return;
     const data=metadata(entry);
     if(seen.has(data.key)) return;
     const score=relevance(data,entry.url);
     if(score<0) return;
     seen.add(data.key);
     let index=options.findIndex(item=>item.score<score);
     if(index<0) index=options.length;
     if(index>=limit) return;
     let option;
     if(kind==='tab') {
      const workspace=(state.workspaces||[]).find(space=>space.id===entry.workspace_id);
      option={title:data.title,detail:`Switch to tab${workspace?' · '+workspace.name:''} · ${entry.url}`,id:entry.id,url:entry.url,favicon:entry.favicon};
     } else {
      const label=kind==='bookmark'?'Bookmark':kind==='history'?'History':'Shortcut';
      option={title:data.title,detail:`${label} · ${entry.url}`,url:entry.url};
      if(kind==='shortcut') option.favicon=entry.favicon;
      option[kind]=true;
     }
     options.splice(index,0,{option,score});
     if(options.length>limit) options.pop();
     return score===maxScore&&complete();
    }
    let fallback;
    const shortcut=keyword(query);
    if(shortcut) {
     fallback={option:{title:shortcut.query||shortcut.name,detail:shortcut.query?'Search '+shortcut.name:'Open '+shortcut.name,url:target,search:!!shortcut.query},score:120};
     seen.add(norm(target));
    } else if(target) {
     const search=target===Veluno.searchUrl+encodeURIComponent(query);
     fallback={option:{title:query,detail:search?'Search with '+Veluno.searchName.replace(/ Search$/,''):'Open in a new tab',url:target,search},score:search?60:110};
     seen.add(norm(target));
    }
    let tabCount=0;
    for(const tab of state.tabs||[]) {
     if(['veluno://newtab','veluno://blank'].includes(tab.url)) continue;
     if(!needle&&tabCount++>=6) break;
     if(add(tab,'tab')) break;
    }
    for(const [entries,kind] of [[state.bookmarks,'bookmark'],[historyEntries,'history'],[state.shortcuts,'shortcut']]) {
     if(complete()) break;
     for(const entry of entries||[]) {
      if(add(entry,kind)) break;
     }
    }
    if(fallback) options.push(fallback);
    return options.sort((a,b)=>b.score-a.score).map(item=>item.option);
  }
  window.Veluno = { send, resolveAddress, suggestions, keywords, navigate: url => send('navigate', { url }), searchUrl: 'https://search.brave.com/search?q=', searchName: 'Brave Search' };
  const postShortcut = window.ipc?.postMessage.bind(window.ipc);
  const stringify = JSON.stringify;
  window.addEventListener('keydown', event => {
    if (!event.isTrusted || event.defaultPrevented || event.isComposing || event.repeat) return;
    const key = event.key.toLowerCase();
    let action;
    let payload = {};
    if (event.ctrlKey && !event.altKey && !event.metaKey) {
      if (key === 't') action = event.shiftKey ? 'restore_closed_tab' : 'new_tab';
      else if (key === 's' && event.shiftKey) action = 'sleep_inactive_tabs';
      else if (key === 'b' && event.shiftKey) action = 'toggle_sidebar';
      else if (key === 'n' && event.shiftKey) action = 'new_private_tab';
      else if (key === 'd' && !event.shiftKey) action = 'toggle_bookmark';
      else if (key === 'w' && !event.shiftKey) action = 'close_active_tab';
      else if (key === 'l' && !event.shiftKey) action = 'focus_address';
      else if (key === 'j' && !event.shiftKey) action = 'open_downloads';
      else if (key === 'h' && !event.shiftKey) action = 'open_history';
      else if (key === 'r') action = event.shiftKey ? 'reload_fresh' : 'reload';
      else if (key === 'f5') action = 'reload_fresh';
      else if (key === 'delete' && event.shiftKey) { action = 'open_settings'; payload = { section: 'privacy' }; }
      else if (key === 'tab') { action = 'cycle_tab'; payload = { backwards: event.shiftKey }; }
      else if (key === '=' || key === '+') { action = 'zoom'; payload = { delta: 10 }; }
      else if (key === '-' || key === '_') { action = 'zoom'; payload = { delta: -10 }; }
      else if (key === '0' && !event.shiftKey) { action = 'zoom'; payload = { delta: 0 }; }
    } else if (event.altKey && !event.ctrlKey && !event.shiftKey && !event.metaKey) {
      if (key === 'arrowleft') action = 'back';
      else if (key === 'arrowright') action = 'forward';
      else if (key === 'd') action = 'focus_address';
      else if (/^[1-9]$/.test(key)) { action = 'switch_profile'; payload = { index: Number(key) }; }
    } else if (!event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey) {
      if (key === 'f5') action = 'reload';
      else if (key === 'f9') action = 'toggle_reader';
      else if (key === 'f11') action = 'toggle_fullscreen';
    }
    if (action) {
      event.preventDefault();
      postShortcut?.(stringify({ __proto__: null, action, ...payload, shortcut_token: '__SHORTCUT_TOKEN__' }));
    }
  }, true);
  // Glance: Alt+Click previews a link in a floating window instead of navigating or downloading.
  window.addEventListener('click', event => {
    if (!event.isTrusted || event.defaultPrevented || event.button !== 0 || !event.altKey
      || event.ctrlKey || event.shiftKey || event.metaKey) return;
    const link = event.target?.closest?.('a[href]');
    if (!link) return;
    let url;
    try { url = new URL(link.href, location.href); } catch { return; }
    if (!['http:', 'https:'].includes(url.protocol)) return;
    event.preventDefault();
    event.stopPropagation();
    postShortcut?.(stringify({ __proto__: null, action: 'glance', url: url.href, shortcut_token: '__SHORTCUT_TOKEN__' }));
  }, true);
  let resourceFailureReported = false;
  window.addEventListener('error', event => {
    if (resourceFailureReported || !event.isTrusted || window.top !== window
      || event.target?.tagName !== 'SCRIPT' || !event.target.src
      || !navigator.serviceWorker?.controller) return;
    const resource = new URL(event.target.src, location.href);
    if (!['http:', 'https:'].includes(resource.protocol) || resource.origin !== location.origin) return;
    resourceFailureReported = true;
    postShortcut?.(stringify({ __proto__: null, action: 'resource_load_failed', document_url: location.href,
      resource_url: resource.href, shortcut_token: '__SHORTCUT_TOKEN__' }));
  }, true);
})();
