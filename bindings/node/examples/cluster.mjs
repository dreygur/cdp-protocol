// Cluster: fixed-size worker pool with retries.
// Mirrors examples/cluster.rs.
//
//   google-chrome --headless=new --remote-debugging-port=9222 --no-first-run
//   node examples/cluster.mjs
import { mkdir } from 'node:fs/promises'
import { Cluster } from '../index.js'

const SHOTS = 'screenshots'
await mkdir(SHOTS, { recursive: true })

const URLS = [
  'https://slishee.com',
  'https://www.rust-lang.org',
  'https://www.google.com',
  'https://github.com',
  'https://stackoverflow.com',
  'https://news.ycombinator.com',
  'https://www.wikipedia.org',
  'https://docs.rs',
  'https://crates.io',
  'https://www.mozilla.org',
]

const CONCURRENCY = 3

console.log(`Starting cluster (${CONCURRENCY} workers)...`)
const cluster = await Cluster.create({
  host: '127.0.0.1',
  port: 9222,
  concurrency: CONCURRENCY,
  retries: 1,
})

const tasks = await Promise.all(
  URLS.map((url, i) =>
    cluster.execute([
      { action: 'navigate', url },
      { action: 'wait_for_selector', selector: 'body', timeout_ms: 15_000 },
      { action: 'screenshot', path: `${SHOTS}/cluster_${String(i).padStart(3, '0')}.png` },
      { action: 'get_title' },
    ]),
  ),
)

let ok = 0
tasks.forEach((t, i) => {
  if (t.success) {
    ok++
    console.log(`[${String(i).padStart(2)}] ${t.results.at(-1)?.value} (${(t.elapsedMs / 1000).toFixed(1)}s)`)
  } else {
    console.log(`[${String(i).padStart(2)}] error: ${t.error}`)
  }
})

console.log(`\nDone. ${ok} ok, ${URLS.length - ok} failed.`)
await cluster.close()
