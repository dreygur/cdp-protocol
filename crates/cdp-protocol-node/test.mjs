// Smoke test for all three classes.
// Requires Chrome running with:
//   google-chrome --headless=new --remote-debugging-port=9222 --no-first-run
// Run after `npm run build`: node test.mjs
import { CdpClient, BrowserAgent, Cluster } from './index.js'

const HOST = '127.0.0.1'
const PORT = 9222

// 1. Low-level client
{
  const client = await CdpClient.connectToPage(HOST, PORT)
  await client.enableDomain('Page')
  await client.enableDomain('Runtime')
  const frameId = await client.navigateAndWait('https://example.com', 10000)
  console.log('[client] frame:', frameId)
  console.log('[client] title:', await client.eval('document.title'))
  console.log('[client] screenshot bytes:', (await client.screenshot()).length)
  // note: not closing  the sole page target is reused by the agent block below
}

// 2. High-level agent
{
  const agent = await BrowserAgent.connect(HOST, PORT)
  await agent.navigate('https://example.com')
  console.log('[agent] title:', (await agent.getTitle()).value)
  console.log('[agent] links:', (await agent.getLinks()).value?.length ?? 0)
  const batch = await agent.executeMany([
    { action: 'evaluate', expression: '1 + 1' },
    { action: 'exists', selector: 'h1' },
  ])
  console.log('[agent] batch:', batch.map((r) => r.value))
  // note: not closing  keep the page target alive for reruns
}

// 3. Cluster
{
  const cluster = await Cluster.create({ host: HOST, port: PORT, concurrency: 2, retries: 1 })
  const urls = ['https://example.com', 'https://example.org', 'https://example.net']
  const tasks = await Promise.all(
    urls.map((url) =>
      cluster.execute([
        { action: 'navigate', url },
        { action: 'get_title' },
      ]),
    ),
  )
  for (const [i, t] of tasks.entries()) {
    console.log(`[cluster] ${urls[i]} ok=${t.success} title=${t.results.at(-1)?.value} ${t.elapsedMs.toFixed(0)}ms`)
  }
  await cluster.close()
}

console.log('ok')
