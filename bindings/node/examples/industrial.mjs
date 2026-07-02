// Industrial scraping: one fresh tab per page, capped concurrency.
// Mirrors examples/industrial.rs (hand-rolled semaphore instead of JoinSet).
//
//   google-chrome --headless=new --remote-debugging-port=9222 --no-first-run
//   node examples/industrial.mjs
import { mkdir } from 'node:fs/promises'
import { CdpClient } from '../index.js'

const HOST = '127.0.0.1'
const PORT = 9222
const SHOTS = 'screenshots'
const MAX_CONCURRENT = 5

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
  'https://nodejs.org',
  'https://deno.com',
  'https://bun.sh',
  'https://go.dev',
  'https://www.python.org',
]

await mkdir(SHOTS, { recursive: true })

console.log('=== Industrial Scraping Demo ===')
console.log('Pages to process:', URLS.length)
console.log('Max concurrent:  ', MAX_CONCURRENT, '\n')

async function processPage(id, url) {
  const start = performance.now()

  const target = await CdpClient.createTab(HOST, PORT, null)
  const ws = target.webSocketDebuggerUrl
  if (!ws) throw new Error(`no WS URL for tab ${id}`)

  const client = await CdpClient.connect(ws)
  await client.enableDomain('Page')
  await client.enableDomain('Runtime')
  await client.setViewport(1920, 1200, false)
  await client.navigateAndWait(url, 15_000)

  let title = 'Unknown'
  try {
    title = await client.eval('document.title')
  } catch {}

  await client.fullPageScreenshotToFile(`${SHOTS}/page_${String(id).padStart(3, '0')}.png`)
  await client.close()

  return { title, elapsed: (performance.now() - start) / 1000 }
}

// Bounded-concurrency worker pool over an index queue.
let next = 0
let success = 0
let failed = 0
const start = performance.now()

async function worker() {
  while (next < URLS.length) {
    const id = next++
    try {
      const { title, elapsed } = await processPage(id, URLS[id])
      console.log(`[${String(id).padStart(3)}] ✓ ${title} (${elapsed.toFixed(1)}s)`)
      success++
    } catch (e) {
      console.log(`[${String(id).padStart(3)}] ✗ Error: ${e.message}`)
      failed++
    }
  }
}

await Promise.all(Array.from({ length: MAX_CONCURRENT }, worker))

const total = (performance.now() - start) / 1000
console.log(
  `\nTotal: ${total.toFixed(2)}s | Success: ${success} | Failed: ${failed} | ${(URLS.length / total).toFixed(2)} pages/sec`,
)
