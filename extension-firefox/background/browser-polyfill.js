// Minimal browser-polyfill for Firefox → Chrome API compatibility
// Firefox uses `browser.*` with Promises; Chrome uses `chrome.*` with callbacks.
// This shim maps `chrome.*` calls to `browser.*` where available.

(function () {
  if (typeof browser === 'undefined') return; // Chrome already has chrome.*
  if (typeof chrome === 'undefined') self.chrome = {};

  // Copy browser APIs to chrome namespace if they exist
  const APIS = ['downloads', 'cookies', 'contextMenus', 'storage', 'notifications', 'runtime'];

  APIS.forEach(api => {
    if (browser[api] && !chrome[api]) {
      chrome[api] = browser[api];
    }
  });

  // Ensure chrome.action exists (Firefox uses browser.browserAction or browser.action)
  if (browser.action && !chrome.action) {
    chrome.action = browser.action;
  }
})();
