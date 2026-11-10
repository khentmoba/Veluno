const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { execFileSync } = require('node:child_process');
const { performance } = require('node:perf_hooks');

const baseline = process.argv[process.argv.indexOf('--baseline') + 1];
if (!process.argv.includes('--baseline') || !baseline) {
  throw Error('Usage: node tests/suggestions.bench.cjs --baseline <git-ref>');
}
function load(source) {
  const window = { addEventListener() {} };
  const context = vm.createContext({ window, URL });
  vm.runInContext(source, context);
  context.Veluno = window.Veluno;
  return window.Veluno.suggestions;
}
const baselineSource = execFileSync('git', ['show', `${baseline}:src/browser.js`], { encoding: 'utf8' });
const candidateSource = fs.readFileSync('src/browser.js', 'utf8');
function run(before, after, assert, performance, iterations = 150) {
  const plain = value => JSON.parse(JSON.stringify(value));
  const queries = ['', 'g', 'git', 'github', 'guide', 'rust async', 'no match', 'https://github.com/new', '東京'];
  function fixture(size) {
    const history = Array.from({ length: size }, (_, i) => ({
      title: ['GitHub guide', 'Async Rust guide', '東京 notes', 'Reference'][i % 4] + ' ' + i,
      url: `https://${['github.com', 'docs.rs', 'news.example', 'www.github.com'][i % 4]}/guide/${i}`,
    }));
    return {
      state: {
        tabs: history.slice(0, 24).map((entry, i) => ({ ...entry, id: i, workspace_id: i % 3 })),
        workspaces: [{ id: 0, name: 'Work' }, { id: 1, name: 'Personal' }],
        bookmarks: history.slice(20, 120).map(entry => ({ ...entry, url: entry.url + '/' })),
        shortcuts: history.slice(0, 8),
      },
      history,
    };
  }
  let comparisons = 0;
  for (const size of [0, 8, 100, 2000]) {
    const { state, history } = fixture(size);
    const compare = () => {
      for (const query of queries) {
        assert.deepEqual(plain(after(query, state, history)), plain(before(query, state, history)), `${size}: ${query}`);
        comparisons++;
      }
    };
    compare();
    for (const entry of [...state.tabs, ...state.bookmarks, ...history]) {
      entry.title = 'Updated GitHub guide';
      entry.url = entry.url.replace('github.com', 'gitlab.com');
      entry.favicon = 'https://icons.example/new.png';
      if ('id' in entry) entry.workspace_id = 1;
    }
    compare();
    state.tabs.reverse();
    state.bookmarks.reverse();
    history.reverse();
    compare();
  }
  // Duplicate URLs can carry different titles and scores; source precedence applies only after matching.
  let seed = 12345;
  function random(n) {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed % n;
  }
  for (let run = 0; run < 100; run++) {
    const entries = Array.from({ length: 80 }, (_, id) => ({
      id, title: ['', 'GitHub', 'GitHub guide', 'Rust async', '東京'][random(5)],
      url: ['https://github.com/', 'https://www.github.com/guide', 'https://docs.rs/', 'invalid url', 'veluno://newtab'][random(5)] + (random(3) ? '' : '/'),
    }));
    const state = { tabs: entries.slice(0, 12), bookmarks: entries.slice(12, 40), shortcuts: entries.slice(60) };
    for (const query of queries) {
      assert.deepEqual(plain(after(query, state, entries.slice(40, 60))), plain(before(query, state, entries.slice(40, 60))));
      comparisons++;
    }
  }
  const equivalence = `Identical suggestions in ${comparisons} baseline comparisons (including mutations and duplicate URLs).`;

  const { state, history } = fixture(2000);
  function batch(fn, query, fresh, iterations) {
    const start = performance.now();
    for (let i = 0; i < iterations; i++) {
      const entries = fresh ? history.map(entry => ({ ...entry })) : history;
      fn(query, state, entries);
    }
    return (performance.now() - start) / iterations;
  }
  const results = [];
  for (const [name, query, fresh] of [
    ['Empty palette', '', false], ['Broad match', 'guide', false],
    ['Site match', 'github', false], ['Multiple words', 'rust async', false],
    ['No match', 'no match', false], ['Replaced history objects', 'github', true],
  ]) {
    batch(before, query, fresh, Math.min(iterations, 100));
    batch(after, query, fresh, Math.min(iterations, 100));
    const samples = [[], []];
    for (let round = 0; round < 7; round++) {
      for (const index of round % 2 ? [1, 0] : [0, 1]) {
        samples[index].push(batch(index ? after : before, query, fresh, iterations));
      }
    }
    const median = values => values.sort((a, b) => a - b)[Math.floor(values.length / 2)];
    const oldMs = median(samples[0]), newMs = median(samples[1]);
    results.push({ workload: name, beforeMs: +oldMs.toFixed(3), afterMs: +newMs.toFixed(3), speedup: +(oldMs / newMs).toFixed(2) });
  }
  return { equivalence, results, method: 'Median of seven alternating samples; 2,000 history entries, 24 tabs, 100 bookmarks. Times include cloning for replaced-history workload.' };
}
function main() {
  const report = run(load(baselineSource), load(candidateSource), assert, performance);
  console.log(report.equivalence);
  console.table(report.results);
  console.log(report.method);
}
main();
