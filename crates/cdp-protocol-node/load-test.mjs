// Load / concurrency test for the napi async boundary.
// Hammers the shared-&self + Arc-clone-before-await pattern:
//   1. huge concurrent eval fan-out on ONE client (pending map + broadcast)
//   2. high-concurrency Cluster with per-task correctness checks
//   3. concurrent screenshots (Buffer marshaling across worker threads)
//
//   google-chrome --headless=new --remote-debugging-port=9222 --no-first-run
//   node load-test.mjs
import { CdpClient, Cluster } from './index.js'

const HOST = '127.0.0.1'
const PORT = 9222

const rss = () => `${(process.memoryUsage().rss / 1024 / 1024).toFixed(0)}MB`
let failures = 0
const fail = (msg) => {
  failures++
  console.error('  FAIL:', msg)
}

// --- 1. Fan-out on a single shared client -------------------------------
{
  const N = 3000
  console.log(`\n[1] ${N} concurrent eval() on one shared client  rss=${rss()}`)
  const client = await CdpClient.connectToPage(HOST, PORT)
  await client.enableDomain('Runtime')

  const t0 = performance.now()
  const out = await Promise.all(
    Array.from({ length: N }, (_, i) => client.eval(`${i} * 2`)),
  )
  const ms = performance.now() - t0

  let bad = 0
  out.forEach((v, i) => {
    if (Number(v) !== i * 2) bad++
  })
  if (bad) fail(`${bad}/${N} eval results mismatched (race in pending map?)`)
  console.log(
    `    ${N} resolved in ${ms.toFixed(0)}ms  (${((N / ms) * 1000).toFixed(0)}/s)  mismatches=${bad}  rss=${rss()}`,
  )
  // client left open  reused below
}

// --- 2. High-concurrency cluster with correctness -----------------------
{
  const CONCURRENCY = 12
  const TASKS = 600
  console.log(`\n[2] cluster concurrency=${CONCURRENCY}, ${TASKS} tasks  rss=${rss()}`)
  const cluster = await Cluster.create({ host: HOST, port: PORT, concurrency: CONCURRENCY, retries: 1 })

  const t0 = performance.now()
  const results = await Promise.all(
    Array.from({ length: TASKS }, (_, i) =>
      cluster.execute([
        { action: 'navigate', url: `data:text/html,<title>T${i}</title><h1>${i}</h1>` },
        { action: 'wait_for_selector', selector: 'h1', timeout_ms: 5000 },
        { action: 'get_title' },
      ]),
    ),
  )
  const ms = performance.now() - t0

  let ok = 0
  let wrong = 0
  results.forEach((r, i) => {
    if (!r.success) {
      fail(`task ${i}: ${r.error}`)
      return
    }
    ok++
    const title = r.results.at(-1)?.value
    if (title !== `T${i}`) {
      wrong++
      fail(`task ${i}: title "${title}" != "T${i}" (worker cross-talk?)`)
    }
  })
  console.log(
    `    ${ok}/${TASKS} ok, ${wrong} wrong-title in ${(ms / 1000).toFixed(1)}s  (${((TASKS / ms) * 1000).toFixed(0)}/s)  rss=${rss()}`,
  )
  await cluster.close()
}

// --- 3. Concurrent screenshots (Buffer marshaling) ----------------------
{
  const CONCURRENCY = 8
  const SHOTS = 200
  console.log(`\n[3] ${SHOTS} concurrent screenshots, concurrency=${CONCURRENCY}  rss=${rss()}`)
  const cluster = await Cluster.create({ host: HOST, port: PORT, concurrency: CONCURRENCY, retries: 0 })

  // Prime each worker with content once.
  const jobs = Array.from({ length: SHOTS }, (_, i) =>
    cluster.execute([
      { action: 'navigate', url: `data:text/html,<body style="background:%23${(i % 900) + 100}"><h1>${i}</h1>` },
      { action: 'wait_for_selector', selector: 'h1', timeout_ms: 5000 },
      { action: 'screenshot' }, // no path -> returns byte count in value
    ]),
  )
  const t0 = performance.now()
  const results = await Promise.all(jobs)
  const ms = performance.now() - t0

  let ok = 0
  let empty = 0
  results.forEach((r, i) => {
    if (!r.success) {
      fail(`shot ${i}: ${r.error}`)
      return
    }
    ok++
    const bytes = r.results.at(-1)?.value
    if (!bytes || bytes <= 0) {
      empty++
      fail(`shot ${i}: empty screenshot (${bytes})`)
    }
  })
  console.log(
    `    ${ok}/${SHOTS} ok, ${empty} empty in ${(ms / 1000).toFixed(1)}s  (${((SHOTS / ms) * 1000).toFixed(0)}/s)  rss=${rss()}`,
  )
  await cluster.close()
}

console.log(`\n${failures === 0 ? 'PASS' : `FAIL (${failures} problems)`}  final rss=${rss()}`)
process.exit(failures === 0 ? 0 : 1)
