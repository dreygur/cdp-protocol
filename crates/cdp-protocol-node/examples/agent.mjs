// BrowserAgent: object actions, JSON dispatch, action batches, data extraction.
// Mirrors examples/agent.rs.
//
//   google-chrome --headless=new --remote-debugging-port=9222 --no-first-run
//   node examples/agent.mjs
import { mkdir } from 'node:fs/promises'
import { BrowserAgent } from '../index.js'

const SHOTS = 'screenshots'
await mkdir(SHOTS, { recursive: true })

const agent = await BrowserAgent.connectWithConfig({
  host: '127.0.0.1',
  port: 9222,
  viewportWidth: 1920,
  viewportHeight: 1200,
})

const show = (label, r) => console.log(`${label}:`, r.success ? (r.value ?? 'ok') : `ERR ${r.error}`)

// --- Object actions ---
console.log('=== Object actions ===')
show('Navigate', await agent.execute({ action: 'navigate', url: 'https://example.com' }))
show('Wait', await agent.execute({ action: 'wait', ms: 1500 }))
show('Title', await agent.getTitle())
show('URL', await agent.execute({ action: 'get_url' }))

// --- JSON dispatch (LLM tool calls) ---
console.log('\n=== JSON dispatch ===')
show('Navigate', await agent.executeJson('{"action":"navigate","url":"https://www.rust-lang.org"}'))
show('Wait', await agent.executeJson('{"action":"wait","ms":2000}'))
show('Screenshot', await agent.executeJson(`{"action":"screenshot","path":"${SHOTS}/rust-lang.png"}`))
show('Title', await agent.executeJson('{"action":"get_title"}'))
show('UserAgent', await agent.executeJson('{"action":"evaluate","expression":"navigator.userAgent"}'))

// --- Action batch (fluent chaining -> array) ---
console.log('\n=== Action batch ===')
const results = await agent.executeMany([
  { action: 'navigate', url: 'https://www.google.com' },
  { action: 'wait', ms: 1500 },
  { action: 'fill', selector: "textarea[name='q'],input[name='q']", value: 'Rust programming' },
  { action: 'press_key', key: 'Enter' },
  { action: 'wait', ms: 2000 },
  { action: 'screenshot', path: `${SHOTS}/google-search.png` },
  { action: 'get_title' },
])
results.forEach((r, i) => show(`[${i}]`, r))

// --- Data extraction ---
console.log('\n=== Data extraction ===')
await agent.navigate('https://example.com')
await agent.execute({ action: 'wait', ms: 1000 })
show(
  'Page info',
  await agent.evaluate(
    '({ viewport: { width: innerWidth, height: innerHeight }, userAgent: navigator.userAgent, language: navigator.language, platform: navigator.platform })',
  ),
)
show('Links', await agent.getLinks())

await agent.close()
