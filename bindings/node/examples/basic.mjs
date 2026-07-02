// Low-level CdpClient: discovery, navigation, eval, DOM, screenshot, cookies.
// Mirrors examples/basic.rs.
//
//   google-chrome --headless=new --remote-debugging-port=9222 --no-first-run
//   node examples/basic.mjs
import { mkdir } from 'node:fs/promises'
import { CdpClient } from '../index.js'

const HOST = '127.0.0.1'
const PORT = 9222
const SHOTS = 'screenshots'

await mkdir(SHOTS, { recursive: true })

// Discovery
const version = await CdpClient.getVersion(HOST, PORT)
console.log('Browser:', version.Browser)
console.log('Protocol:', version['Protocol-Version'])

const targets = await CdpClient.listTargets(HOST, PORT)
console.log(`\nTargets (${targets.length}):`)
for (const t of targets) console.log(`  - ${t.type} [${t.id}]: ${t.title}`)

// Connect to first page target
const client = await CdpClient.connectToPage(HOST, PORT)
for (const d of ['Page', 'Runtime', 'DOM', 'Network']) await client.enableDomain(d)
await client.setViewport(1920, 1200, false)

// Navigate
const frameId = await client.navigateAndWait('https://example.com', 10_000)
console.log('\nNavigated, frameId:', frameId)

// JavaScript
console.log('Title:', await client.eval('document.title'))
console.log('Math:', await client.evaluate('1 + 2 * 3'))
console.log('Viewport:', await client.evaluate('({ width: innerWidth, height: innerHeight })'))

// DOM
const h1 = await client.querySelector('h1')
if (h1 > 0) console.log('H1:', await client.getOuterHtml(h1))

// Screenshot
const path = `${SHOTS}/example.png`
await client.fullPageScreenshotToFile(path)
console.log('\nScreenshot saved:', path)

// Cookies
console.log('Cookies:', (await client.getCookies()).length)

await client.close()
