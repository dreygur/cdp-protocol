# Chrome DevTools Protocol (CDP) - Complete Technical Reference

## Overview

CDP is a **JSON-RPC 2.0-based protocol** providing programmatic access to Chromium-based browsers. It's the underlying protocol that powers Chrome DevTools - every DevTools action sends CDP commands.

## Architecture

```
┌─────────────────┐     WebSocket      ┌──────────────────┐
│  Client         │ ←────────────────→ │  Chrome Browser  │
│  - Puppeteer    │   JSON Messages    │  - Page targets  │
│  - Playwright   │                    │  - Worker targets│
│  - Custom code  │                    │  - Browser target│
└─────────────────┘                    └──────────────────┘
```

**Connection**: `ws://localhost:{port}/devtools/page/{targetId}`

Launch Chrome with: `chrome --remote-debugging-port=9222`

## Message Format

### Command (Client → Browser)
```json
{
  "id": 1,
  "method": "Page.navigate",
  "params": { "url": "https://example.com" }
}
```

### Response (Browser → Client)
```json
{
  "id": 1,
  "result": { "frameId": "ABC123", "loaderId": "XYZ789" }
}
```

### Event (Browser → Client, no id)
```json
{
  "method": "Network.requestWillBeSent",
  "params": { "requestId": "...", "request": {...} }
}
```

## Protocol Structure

### 61 Domains

| Category | Domains |
|----------|---------|
| **Core** | Page, Runtime, Target, Browser, Inspector |
| **Debugging** | Debugger, DOMDebugger, EventBreakpoints |
| **DOM/CSS** | DOM, CSS, DOMSnapshot, Overlay, Accessibility |
| **Network** | Network, Fetch, WebSocket events |
| **Performance** | Performance, PerformanceTimeline, Profiler, HeapProfiler, Tracing, Memory |
| **Storage** | IndexedDB, CacheStorage, DOMStorage, Storage |
| **Media** | Media, WebAudio |
| **Emulation** | Emulation, DeviceOrientation, Input, BluetoothEmulation |
| **Security** | Security, WebAuthn |
| **Service** | ServiceWorker, BackgroundService, PWA |
| **Other** | Console, Log, LayerTree, Animation, Autofill, Cast, SystemInfo |

### Domain Components
- **Methods**: Commands you invoke (request/response)
- **Events**: Async notifications from browser
- **Types**: Data structures for params/results

---

## Key Domains Reference

### Target Domain
Manages browser targets (pages, workers, service workers).

```
Methods:
- createTarget(url, width?, height?, browserContextId?)
- attachToTarget(targetId, flatten?)
- detachFromTarget(sessionId?, targetId?)
- setAutoAttach(autoAttach, waitForDebuggerOnStart, flatten?)
- getTargets()
- createBrowserContext()
- disposeBrowserContext(browserContextId)
- closeTarget(targetId)

Events:
- targetCreated, targetDestroyed, targetCrashed
- attachedToTarget, detachedFromTarget
- targetInfoChanged
```

### Page Domain
Navigation, screenshots, lifecycle management.

```
Methods:
- navigate(url, referrer?, transitionType?, frameId?)
- reload(ignoreCache?, scriptToEvaluateOnLoad?)
- stopLoading()
- getNavigationHistory()
- navigateToHistoryEntry(entryId)
- captureScreenshot(format?, quality?, clip?, fromSurface?, captureBeyondViewport?)
- printToPDF(landscape?, displayHeaderFooter?, printBackground?, scale?, paperWidth?, paperHeight?, marginTop?, marginRight?, marginBottom?, marginLeft?, pageRanges?, headerTemplate?, footerTemplate?)
- setDocumentContent(frameId, html)
- addScriptToEvaluateOnNewDocument(source, worldName?, includeCommandLineAPI?)
- bringToFront()
- close()
- handleJavaScriptDialog(accept, promptText?)
- setInterceptFileChooserDialog(enabled)

Events:
- loadEventFired, domContentEventFired
- frameNavigated, frameStartedLoading, frameStoppedLoading
- frameAttached, frameDetached
- javascriptDialogOpening, javascriptDialogClosed
- windowOpen
- lifecycleEvent
- fileChooserOpened
```

### Runtime Domain
JavaScript execution and object inspection.

```
Methods:
- enable()
- disable()
- evaluate(expression, objectGroup?, includeCommandLineAPI?, silent?, contextId?, returnByValue?, generatePreview?, userGesture?, awaitPromise?, throwOnSideEffect?, timeout?, disableBreaks?, replMode?, allowUnsafeEvalBlockedByCSP?)
- callFunctionOn(functionDeclaration, objectId?, arguments?, silent?, returnByValue?, generatePreview?, userGesture?, awaitPromise?)
- getProperties(objectId, ownProperties?, accessorPropertiesOnly?, generatePreview?, nonIndexedPropertiesOnly?)
- releaseObject(objectId)
- releaseObjectGroup(objectGroup)
- runScript(scriptId, executionContextId?, objectGroup?, silent?, includeCommandLineAPI?, returnByValue?, generatePreview?, awaitPromise?)
- compileScript(expression, sourceURL, persistScript, executionContextId?)
- queryObjects(prototypeObjectId, objectGroup?)
- globalLexicalScopeNames(executionContextId?)
- addBinding(name, executionContextId?, executionContextName?)
- removeBinding(name)

Events:
- executionContextCreated, executionContextDestroyed, executionContextsCleared
- exceptionThrown
- consoleAPICalled
- inspectRequested
- bindingCalled
```

### Network Domain
HTTP traffic monitoring and interception.

```
Methods:
- enable(maxTotalBufferSize?, maxResourceBufferSize?, maxPostDataSize?)
- disable()
- setExtraHTTPHeaders(headers)
- setUserAgentOverride(userAgent, acceptLanguage?, platform?, userAgentMetadata?)
- getResponseBody(requestId)
- getRequestPostData(requestId)
- getCookies(urls?)
- setCookie(name, value, url?, domain?, path?, secure?, httpOnly?, sameSite?, expires?, priority?, sameParty?, sourceScheme?, sourcePort?, partitionKey?)
- setCookies(cookies)
- deleteCookies(name, url?, domain?, path?)
- clearBrowserCache()
- clearBrowserCookies()
- setCacheDisabled(cacheDisabled)
- setBypassServiceWorker(bypass)
- emulateNetworkConditions(offline, latency, downloadThroughput, uploadThroughput, connectionType?)
- loadNetworkResource(frameId?, url, options)

Events:
- requestWillBeSent
- requestWillBeSentExtraInfo
- responseReceived
- responseReceivedExtraInfo
- dataReceived
- loadingFinished
- loadingFailed
- requestServedFromCache
- webSocketCreated, webSocketWillSendHandshakeRequest, webSocketHandshakeResponseReceived, webSocketFrameSent, webSocketFrameReceived, webSocketClosed, webSocketFrameError
- eventSourceMessageReceived
- requestIntercepted (deprecated, use Fetch domain)
```

### Fetch Domain
Request interception and modification.

```
Methods:
- enable(patterns?, handleAuthRequests?)
- disable()
- failRequest(requestId, errorReason)
- fulfillRequest(requestId, responseCode, responseHeaders?, binaryResponseHeaders?, body?, responsePhrase?)
- continueRequest(requestId, url?, method?, postData?, headers?, interceptResponse?)
- continueWithAuth(requestId, authChallengeResponse)
- continueResponse(requestId, responseCode?, responsePhrase?, responseHeaders?, binaryResponseHeaders?)
- getResponseBody(requestId)
- takeResponseBodyAsStream(requestId)

Events:
- requestPaused
- authRequired
```

### Debugger Domain
JavaScript debugging capabilities.

```
Methods:
- enable(maxScriptsCacheSize?)
- disable()
- setBreakpointsActive(active)
- setSkipAllPauses(skip)
- setBreakpointByUrl(lineNumber, url?, urlRegex?, scriptHash?, columnNumber?, condition?)
- setBreakpoint(location, condition?)
- removeBreakpoint(breakpointId)
- getPossibleBreakpoints(start, end?, restrictToFunction?)
- continueToLocation(location, targetCallFrames?)
- pause()
- resume(terminateOnResume?)
- stepOver(skipList?)
- stepInto(breakOnAsyncCall?, skipList?)
- stepOut()
- setScriptSource(scriptId, scriptSource, dryRun?, allowTopFrameEditing?)
- getScriptSource(scriptId)
- evaluateOnCallFrame(callFrameId, expression, objectGroup?, includeCommandLineAPI?, silent?, returnByValue?, generatePreview?, throwOnSideEffect?, timeout?)
- setVariableValue(scopeNumber, variableName, newValue, callFrameId)
- setReturnValue(newValue)
- setAsyncCallStackDepth(maxDepth)
- setBlackboxPatterns(patterns)
- searchInContent(scriptId, query, caseSensitive?, isRegex?)

Events:
- scriptParsed
- scriptFailedToParse
- breakpointResolved
- paused
- resumed
```

### DOM Domain
Document Object Model inspection and manipulation.

```
Methods:
- enable(includeWhitespace?)
- disable()
- getDocument(depth?, pierce?)
- getOuterHTML(nodeId?, backendNodeId?, objectId?)
- setOuterHTML(nodeId, outerHTML)
- querySelector(nodeId, selector)
- querySelectorAll(nodeId, selector)
- requestNode(objectId)
- resolveNode(nodeId?, backendNodeId?, objectGroup?, executionContextId?)
- setAttributeValue(nodeId, name, value)
- setAttributesAsText(nodeId, text, name?)
- removeAttribute(nodeId, name)
- removeNode(nodeId)
- moveTo(nodeId, targetNodeId, insertBeforeNodeId?)
- copyTo(nodeId, targetNodeId, insertBeforeNodeId?)
- focus(nodeId?, backendNodeId?, objectId?)
- setFileInputFiles(files, nodeId?, backendNodeId?, objectId?)
- getBoxModel(nodeId?, backendNodeId?, objectId?)
- getNodeForLocation(x, y, includeUserAgentShadowDOM?, ignorePointerEventsNone?)
- performSearch(query, includeUserAgentShadowDOM?)
- getSearchResults(searchId, fromIndex, toIndex)
- discardSearchResults(searchId)
- highlightNode()
- hideHighlight()

Events:
- documentUpdated
- setChildNodes
- attributeModified, attributeRemoved
- childNodeCountUpdated, childNodeInserted, childNodeRemoved
```

### CSS Domain
Stylesheet inspection and manipulation.

```
Methods:
- enable()
- disable()
- getMatchedStylesForNode(nodeId)
- getInlineStylesForNode(nodeId)
- getComputedStyleForNode(nodeId)
- getStyleSheetText(styleSheetId)
- setStyleSheetText(styleSheetId, text)
- setStyleTexts(edits)
- createStyleSheet(frameId)
- addRule(styleSheetId, ruleText, location)
- forcePseudoState(nodeId, forcedPseudoClasses)
- getMediaQueries()
- setMediaText(styleSheetId, range, text)
- getBackgroundColors(nodeId)
- getPlatformFontsForNode(nodeId)

Events:
- styleSheetAdded, styleSheetChanged, styleSheetRemoved
- fontsUpdated
- mediaQueryResultChanged
```

### Emulation Domain
Device and environment emulation.

```
Methods:
- setDeviceMetricsOverride(width, height, deviceScaleFactor, mobile, scale?, screenWidth?, screenHeight?, positionX?, positionY?, dontSetVisibleSize?, screenOrientation?, viewport?, displayFeature?)
- clearDeviceMetricsOverride()
- setTouchEmulationEnabled(enabled, maxTouchPoints?)
- setEmulatedMedia(media?, features?)
- setEmulatedVisionDeficiency(type)
- setCPUThrottlingRate(rate)
- setGeolocationOverride(latitude?, longitude?, accuracy?)
- clearGeolocationOverride()
- setTimezoneOverride(timezoneId)
- setLocaleOverride(locale?)
- setUserAgentOverride(userAgent, acceptLanguage?, platform?, userAgentMetadata?)
- setScrollbarsHidden(hidden)
- setDocumentCookieDisabled(disabled)
- setFocusEmulationEnabled(enabled)
- setAutoDarkModeOverride(enabled?)
- setAutomationOverride(enabled)

Events:
- virtualTimeBudgetExpired
```

### Input Domain
User input simulation.

```
Methods:
- dispatchKeyEvent(type, modifiers?, timestamp?, text?, unmodifiedText?, keyIdentifier?, code?, key?, windowsVirtualKeyCode?, nativeVirtualKeyCode?, autoRepeat?, isKeypad?, isSystemKey?, location?, commands?)
- dispatchMouseEvent(type, x, y, modifiers?, timestamp?, button?, buttons?, clickCount?, force?, tangentialPressure?, tiltX?, tiltY?, twist?, deltaX?, deltaY?, pointerType?)
- dispatchTouchEvent(type, touchPoints, modifiers?, timestamp?)
- dispatchDragEvent(type, x, y, data, modifiers?)
- emulateTouchFromMouseEvent(type, x, y, button, timestamp?, deltaX?, deltaY?, modifiers?, clickCount?)
- setIgnoreInputEvents(ignore)
- setInterceptDrags(enabled)
- synthesizePinchGesture(x, y, scaleFactor, relativeSpeed?, gestureSourceType?)
- synthesizeScrollGesture(x, y, xDistance?, yDistance?, xOverscroll?, yOverscroll?, preventFling?, speed?, gestureSourceType?, repeatCount?, repeatDelayMs?, interactionMarkerName?)
- synthesizeTapGesture(x, y, duration?, tapCount?, gestureSourceType?)
- insertText(text)
- imeSetComposition(text, selectionStart, selectionEnd, replacementStart?, replacementEnd?)
```

### Performance Domain
Performance metrics collection.

```
Methods:
- enable(timeDomain?)
- disable()
- getMetrics()
- setTimeDomain(timeDomain)

Events:
- metrics
```

### Profiler Domain
JavaScript CPU profiling.

```
Methods:
- enable()
- disable()
- start()
- stop()
- setSamplingInterval(interval)
- startPreciseCoverage(callCount?, detailed?, allowTriggeredUpdates?)
- stopPreciseCoverage()
- takePreciseCoverage()
- getBestEffortCoverage()

Events:
- consoleProfileStarted
- consoleProfileFinished
- preciseCoverageDeltaUpdate
```

### HeapProfiler Domain
Memory/heap profiling.

```
Methods:
- enable()
- disable()
- startSampling(samplingInterval?, includeObjectsCollectedByMajorGC?, includeObjectsCollectedByMinorGC?)
- stopSampling()
- getSamplingProfile()
- takeHeapSnapshot(reportProgress?, treatGlobalObjectsAsRoots?, captureNumericValue?, exposeInternals?)
- getObjectByHeapObjectId(objectId, objectGroup?)
- addInspectedHeapObject(heapObjectId)
- getHeapObjectId(objectId)
- startTrackingHeapObjects(trackAllocations?)
- stopTrackingHeapObjects(reportProgress?, treatGlobalObjectsAsRoots?, captureNumericValue?)
- collectGarbage()

Events:
- addHeapSnapshotChunk
- reportHeapSnapshotProgress
- resetProfiles
- lastSeenObjectId
- heapStatsUpdate
```

### Tracing Domain
Performance tracing (Timeline).

```
Methods:
- start(categories?, options?, bufferUsageReportingInterval?, transferMode?, streamFormat?, streamCompression?, traceConfig?, perfettoConfig?, tracingBackend?)
- end()
- getCategories()
- recordClockSyncMarker(syncId)
- requestMemoryDump(deterministic?, levelOfDetail?)

Events:
- dataCollected
- tracingComplete
- bufferUsage
```

---

## HTTP Endpoints

When Chrome is launched with `--remote-debugging-port=9222`:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/json/version` | GET | Browser version info and WebSocket URL |
| `/json/list` | GET | List all available targets |
| `/json/protocol` | GET | Full protocol schema as JSON |
| `/json/new?{url}` | PUT | Create new tab with optional URL |
| `/json/activate/{targetId}` | GET | Bring target to foreground |
| `/json/close/{targetId}` | GET | Close target |

### Example Response: /json/version
```json
{
  "Browser": "Chrome/120.0.0.0",
  "Protocol-Version": "1.3",
  "User-Agent": "Mozilla/5.0...",
  "V8-Version": "12.0.0.0",
  "WebKit-Version": "537.36",
  "webSocketDebuggerUrl": "ws://localhost:9222/devtools/browser/guid"
}
```

### Example Response: /json/list
```json
[
  {
    "id": "ABC123",
    "type": "page",
    "title": "Example Page",
    "url": "https://example.com",
    "webSocketDebuggerUrl": "ws://localhost:9222/devtools/page/ABC123"
  }
]
```

---

## Protocol Monitor (DevTools Feature)

### Enabling
1. Open DevTools (F12)
2. Settings (F1) → Experiments
3. Check "Protocol Monitor"
4. Restart DevTools
5. Open via More tools → Protocol Monitor

### Features
- **View all CDP traffic** in real-time
- **Command editor** with auto-completion
- **Parameter validation** (red=required, blue=optional)
- **Type-aware inputs** (dropdowns for enums/booleans)
- **Tooltips** with documentation links
- **Save/export** as JSON
- **Edit and resend** previous commands
- **Copy as JSON** for external use

---

## Versioning

| Version | Description | Stability |
|---------|-------------|-----------|
| **tip-of-tree** | Latest from Chromium HEAD | Unstable, may change |
| **Stable 1.3** | Frozen at Chrome 64 | Stable, subset of features |
| **v8-inspector** | Node.js debugging | Stable for Node.js |

---

## npm Package: devtools-protocol

### Installation
```bash
npm install devtools-protocol
```

### Files Provided
- `types/protocol.d.ts` - All CDP type definitions
- `types/protocol-proxy-api.d.ts` - Domain API style mappings
- `types/protocol-mapping.d.ts` - Command/event name-to-type mappings

### Usage with TypeScript
```typescript
import Protocol from 'devtools-protocol';

// Type for Network.Request
type Request = Protocol.Network.Request;

// Type for Page.navigate params
type NavigateParams = Protocol.Page.NavigateRequest;

// Type for Page.navigate result
type NavigateResult = Protocol.Page.NavigateResponse;
```

---

## Common Usage Patterns

### 1. Connect and Navigate
```javascript
const ws = new WebSocket('ws://localhost:9222/devtools/page/TARGET_ID');

ws.onopen = () => {
  // Enable necessary domains
  ws.send(JSON.stringify({ id: 1, method: 'Page.enable' }));
  ws.send(JSON.stringify({ id: 2, method: 'Network.enable' }));

  // Navigate
  ws.send(JSON.stringify({
    id: 3,
    method: 'Page.navigate',
    params: { url: 'https://example.com' }
  }));
};

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  if (message.id) {
    console.log('Response:', message);
  } else {
    console.log('Event:', message.method, message.params);
  }
};
```

### 2. Capture Screenshot
```javascript
ws.send(JSON.stringify({
  id: 10,
  method: 'Page.captureScreenshot',
  params: {
    format: 'png',
    quality: 100,
    captureBeyondViewport: true
  }
}));
// Response contains base64-encoded image in result.data
```

### 3. Execute JavaScript
```javascript
ws.send(JSON.stringify({
  id: 20,
  method: 'Runtime.evaluate',
  params: {
    expression: 'document.title',
    returnByValue: true
  }
}));
```

### 4. Intercept Network Requests
```javascript
// Enable Fetch domain for interception
ws.send(JSON.stringify({
  id: 30,
  method: 'Fetch.enable',
  params: {
    patterns: [{ urlPattern: '*', requestStage: 'Request' }]
  }
}));

// Handle requestPaused events
ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.method === 'Fetch.requestPaused') {
    // Continue, modify, or fail the request
    ws.send(JSON.stringify({
      id: 31,
      method: 'Fetch.continueRequest',
      params: { requestId: msg.params.requestId }
    }));
  }
};
```

### 5. Set Breakpoint and Debug
```javascript
ws.send(JSON.stringify({ id: 40, method: 'Debugger.enable' }));
ws.send(JSON.stringify({
  id: 41,
  method: 'Debugger.setBreakpointByUrl',
  params: {
    lineNumber: 10,
    url: 'https://example.com/app.js'
  }
}));

// When paused event fires, inspect or step
ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.method === 'Debugger.paused') {
    // Evaluate in current scope
    ws.send(JSON.stringify({
      id: 42,
      method: 'Debugger.evaluateOnCallFrame',
      params: {
        callFrameId: msg.params.callFrames[0].callFrameId,
        expression: 'myVariable'
      }
    }));
  }
};
```

---

## Multi-Client Support

Chrome 63+ supports multiple simultaneous CDP clients. When a client disconnects:
- `detached` event fired with reason
- Applications can pause state and offer reconnection

---

## Tools Built on CDP

- **Puppeteer** - Node.js browser automation
- **Playwright** - Cross-browser automation
- **Chrome DevTools** - The official debugger
- **Lighthouse** - Performance auditing
- **chrome-remote-interface** - Low-level Node.js client
- **Selenium 4** - Uses CDP for Chrome
- **Cypress** - Testing framework

---

## Security Considerations

- CDP provides **full browser control** - treat access carefully
- Only expose debugging port on localhost in production
- Use `--remote-debugging-address=127.0.0.1` explicitly
- Consider using `Target.setAutoAttach` with `flatten: true` for session isolation

---

## Resources

- **Protocol Viewer**: https://chromedevtools.github.io/devtools-protocol/
- **GitHub**: https://github.com/ChromeDevTools/devtools-protocol
- **npm**: https://www.npmjs.com/package/devtools-protocol
- **Chromium Source**: https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/public/devtools_protocol/
