// Reader view: toggled on demand, never injected into every page. It copies the main article
// into an isolated overlay above the page, so closing it returns the page exactly as it was.
(() => {
  const open = document.getElementById('__veluno-reader');
  if (open) { open.__velunoClose?.(); return 'closed'; }
  if (!document.body || !/^https?:$/.test(location.protocol)) return 'unavailable';

  const SKIP = 'nav,aside,footer,form,[role=navigation],[role=complementary],[role=banner],[aria-hidden=true],[hidden]';
  const words = node => (node.textContent || '').replace(/\s+/g, ' ').trim();
  const linkDensity = node => {
    const total = words(node).length || 1;
    let linked = 0;
    for (const link of node.querySelectorAll('a')) linked += words(link).length;
    return linked / total;
  };

  // Score each paragraph's ancestors, like most reader modes: long, comma-rich text wins.
  const scores = new Map();
  for (const block of document.body.querySelectorAll('p,pre,blockquote,li')) {
    if (block.closest(SKIP)) continue;
    const text = words(block);
    if (text.length < 40) continue;
    const score = 1 + Math.min(text.length / 100, 3) + text.split(',').length * 0.25;
    let parent = block.parentElement;
    for (const factor of [1, 0.5, 0.25]) {
      if (!parent || parent === document.documentElement) break;
      scores.set(parent, (scores.get(parent) || 0) + score * factor);
      parent = parent.parentElement;
    }
  }
  let root = null;
  let best = 0;
  for (const [node, score] of scores) {
    const adjusted = score * (1 - linkDensity(node));
    if (adjusted > best) { best = adjusted; root = node; }
  }
  const articles = [...document.querySelectorAll('article')].filter(node => !node.closest(SKIP) && words(node).length > 500);
  if (articles.length === 1 && (!root || articles[0].contains(root) || root.contains(articles[0]))) root = articles[0];
  if (!root || words(root).length < 400) return 'unavailable';

  const meta = selector => document.querySelector(selector)?.getAttribute('content')?.trim() || '';
  const title = meta('meta[property="og:title"]') || words(document.querySelector('h1') || document.createElement('i')) || document.title || location.hostname;
  const site = meta('meta[property="og:site_name"]') || location.hostname.replace(/^www\./, '');
  const author = meta('meta[name="author"]');

  // Rebuild from the live page so resolved links and lazy images are kept; nothing else is copied.
  const KEEP = new Set(['DIV','SECTION','ARTICLE','MAIN','P','H1','H2','H3','H4','H5','H6','UL','OL','LI','BLOCKQUOTE','PRE','CODE','EM','STRONG','B','I','U','S','A','IMG','FIGURE','FIGCAPTION','TABLE','THEAD','TBODY','TFOOT','TR','TH','TD','CAPTION','BR','HR','SUP','SUB','DL','DT','DD','MARK','SMALL','Q','CITE','ABBR','TIME','KBD','SAMP','VAR','DEL','INS']);
  const DROP = new Set(['SCRIPT','STYLE','NOSCRIPT','TEMPLATE','IFRAME','FRAME','OBJECT','EMBED','FORM','BUTTON','INPUT','SELECT','TEXTAREA','OPTION','LABEL','NAV','ASIDE','FOOTER','HEADER','SVG','CANVAS','VIDEO','AUDIO','SOURCE','TRACK','MAP','DIALOG','LINK','META']);
  const safeUrl = (value, images) => {
    try {
      const url = new URL(value, location.href);
      if (url.protocol === 'http:' || url.protocol === 'https:') return url.href;
      if (images && url.protocol === 'data:' && /^data:image\/(png|jpe?g|gif|webp|avif);/i.test(value)) return value;
    } catch {}
    return '';
  };
  let nodes = 0;
  const build = (node, into) => {
    for (const child of node.childNodes) {
      if (++nodes > 20000) return;
      if (child.nodeType === Node.TEXT_NODE) { into.append(child.textContent); continue; }
      if (child.nodeType !== Node.ELEMENT_NODE) continue;
      const tag = child.tagName.toUpperCase();
      if (DROP.has(tag) || child.hidden || child.getAttribute('aria-hidden') === 'true') continue;
      if (tag === 'IMG') {
        const src = safeUrl(child.currentSrc || child.getAttribute('data-src') || child.getAttribute('data-lazy-src') || child.src, true);
        const width = child.naturalWidth || child.width;
        if (!src || (width && width < 48)) continue;
        const image = document.createElement('img');
        image.src = src; image.alt = child.alt || ''; image.loading = 'lazy'; image.referrerPolicy = 'no-referrer';
        into.append(image);
        continue;
      }
      if (!KEEP.has(tag)) { build(child, into); continue; }
      const copy = document.createElement(tag);
      if (tag === 'A') {
        const href = safeUrl(child.getAttribute('href') ? child.href : '', false);
        if (href) copy.href = href;
      }
      for (const name of ['colspan', 'rowspan']) if (child.hasAttribute(name)) copy.setAttribute(name, child.getAttribute(name));
      build(child, copy);
      if (tag !== 'BR' && tag !== 'HR' && !copy.textContent.trim() && !copy.querySelector('img')) continue;
      into.append(copy);
    }
  };
  const article = document.createElement('article');
  build(root, article);
  const heading = article.querySelector('h1,h2');
  if (heading && words(heading) === title) heading.remove();
  const count = words(article).split(' ').length;

  const host = document.createElement('div');
  host.id = '__veluno-reader';
  host.style.cssText = 'all:initial;position:fixed;inset:0;z-index:2147483647;display:block';
  const shadow = host.attachShadow({ mode: 'closed' });
  shadow.innerHTML = `<style>
:host{color-scheme:dark}
*{box-sizing:border-box}
.page{position:absolute;inset:0;overflow:auto;scrollbar-width:thin;scrollbar-color:#3f3f49 transparent;background:#0c0c0e;color:#f0f0f4;outline:0;font:var(--size,19px)/1.72 Georgia,Cambria,"Times New Roman",serif}
.bar{position:sticky;top:0;display:flex;align-items:center;gap:6px;padding:10px 16px;background:#101011;border-bottom:1px solid #3b3b44;font:13px "Segoe UI",system-ui,sans-serif;color:#c8c8d2;z-index:1}
.bar .label{flex:1;font-weight:600;color:#f8f8fa}
button{font:500 13px "Segoe UI",system-ui,sans-serif;color:#f8f8fa;background:#232329;border:1px solid #3f3f49;border-radius:8px;height:30px;min-width:34px;padding:0 10px;cursor:pointer}
button:hover{background:#2e2e36}
button:focus-visible{outline:2px solid #dedee3;outline-offset:2px}
main{max-width:700px;margin:0 auto;padding:40px 24px 120px}
.site{font:600 12px "Segoe UI",system-ui,sans-serif;color:#c8c8d2;margin:0 0 10px;text-transform:uppercase;letter-spacing:.06em}
h1.title{font:600 2em/1.2 "Segoe UI",system-ui,sans-serif;letter-spacing:-.02em;margin:0 0 12px;color:#f8f8fa}
.meta{font:13px "Segoe UI",system-ui,sans-serif;color:#c8c8d2;margin:0 0 32px;padding-bottom:20px;border-bottom:1px solid #3b3b44}
article h1,article h2,article h3,article h4{font-family:"Segoe UI",system-ui,sans-serif;line-height:1.3;color:#f8f8fa;margin:1.6em 0 .5em}
article p,article ul,article ol,article blockquote,article pre,article figure,article table{margin:0 0 1.1em}
article a{color:#f8f8fa;text-decoration-color:#8a8a96;text-underline-offset:3px}
article img{display:block;max-width:100%;height:auto;margin:1.2em auto;border-radius:8px}
article figcaption{font:13px/1.5 "Segoe UI",system-ui,sans-serif;color:#c8c8d2;text-align:center}
article blockquote{border-left:2px solid #484854;padding-left:16px;color:#d8d8e0}
article pre{overflow:auto;padding:14px;background:#1c1c21;border:1px solid #3b3b44;border-radius:8px;font:14px/1.5 Consolas,monospace}
article code{font-family:Consolas,monospace;font-size:.88em}
article table{border-collapse:collapse;display:block;overflow:auto;font-size:.85em}
article th,article td{border:1px solid #3f3f49;padding:6px 10px}
::selection{background:#45454b;color:#fff}
</style><div class="page" tabindex="-1"><div class="bar" role="toolbar" aria-label="Reader view"><span class="label">Reader view</span><button data-size="-1" aria-label="Smaller text" title="Smaller text">A−</button><button data-size="1" aria-label="Larger text" title="Larger text">A+</button><button data-close title="Close reader view (Esc or F9)">Close</button></div><main><p class="site"></p><h1 class="title"></h1><p class="meta"></p></main></div>`;
  const page = shadow.querySelector('.page');
  shadow.querySelector('.site').textContent = site;
  shadow.querySelector('.title').textContent = title;
  shadow.querySelector('.meta').textContent = [author, `${Math.max(1, Math.round(count / 230))} min read`].filter(Boolean).join(' · ');
  shadow.querySelector('main').append(article);

  let size = Number(sessionStorage.getItem('__velunoReaderSize')) || 19;
  const resize = delta => {
    size = Math.min(28, Math.max(14, size + delta));
    page.style.setProperty('--size', size + 'px');
    try { sessionStorage.setItem('__velunoReaderSize', String(size)); } catch {}
  };
  resize(0);
  for (const button of shadow.querySelectorAll('[data-size]')) button.onclick = () => resize(Number(button.dataset.size));

  const html = document.documentElement;
  const overflow = html.style.overflow;
  const focused = document.activeElement;
  const onKey = event => {
    if (event.key === 'Escape' && !event.defaultPrevented) { event.preventDefault(); event.stopPropagation(); close(); }
  };
  const close = () => {
    window.removeEventListener('keydown', onKey, true);
    host.remove();
    html.style.overflow = overflow;
    focused?.focus?.({ preventScroll: true });
  };
  host.__velunoClose = close;
  shadow.querySelector('[data-close]').onclick = close;
  window.addEventListener('keydown', onKey, true);
  html.style.overflow = 'hidden';
  html.append(host);
  page.focus({ preventScroll: true });
  return 'opened';
})()
