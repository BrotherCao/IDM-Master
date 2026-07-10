// IDM Master — Service Worker (Manifest V3)
// 拦截 Chrome 下载事件，取消浏览器原生下载，转交桌面客户端

const SERVER_URL = 'http://127.0.0.1:16888';

// ── 初始化：检查服务端健康状态 ──
async function checkHealth() {
  try {
    const resp = await fetch(`${SERVER_URL}/api/health`, { signal: AbortSignal.timeout(3000) });
    return resp.ok;
  } catch {
    return false;
  }
}

// 上线通知
checkHealth().then(ok => {
  if (ok) {
    console.log('[IDM Master] 客户端已连接 ✓');
  } else {
    console.warn('[IDM Master] 客户端未运行，下载拦截已暂停');
  }
});

// ── 下载拦截 ──
chrome.downloads.onCreated.addListener(async (item) => {
  // 跳过空 URL 和非 HTTP(S)
  if (!item.url || !/^https?:\/\//i.test(item.url)) return;

  const alive = await checkHealth();
  if (!alive) {
    // 客户端未运行 → 不拦截，让 Chrome 原生下载
    console.log('[IDM Master] 客户端离线，回退到 Chrome 原生下载');
    return;
  }

  try {
    // 取消 Chrome 原生下载
    await chrome.downloads.cancel(item.id);
    console.log('[IDM Master] 拦截下载:', item.filename || item.url);

    // 提取 cookies
    let cookies = '';
    try {
      const url = new URL(item.url);
      const jar = await chrome.cookies.getAll({ url: url.origin });
      cookies = jar.map(c => `${c.name}=${c.value}`).join('; ');
    } catch (_) { /* 无 cookies */ }

    // 发给桌面客户端
    const resp = await fetch(`${SERVER_URL}/api/download`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        url: item.url,
        filename: item.filename || '',
        referer: item.referrer || '',
        cookies: cookies,
        user_agent: navigator.userAgent,
        content_length: item.fileSize || 0
      }),
      signal: AbortSignal.timeout(5000),
    });

    if (!resp.ok) {
      console.error('[IDM Master] 提交失败:', await resp.text());
      chrome.notifications?.create?.(`err-${item.id}`, {
        type: 'basic',
        iconUrl: 'icons/icon48.png',
        title: 'IDM Master',
        message: `提交下载失败: ${item.filename || item.url}`
      });
      return;
    }

    const data = await resp.json();
    console.log('[IDM Master] 下载已提交:', data);

    // 可选：弹出通知
    chrome.notifications?.create?.(`ok-${item.id}`, {
      type: 'basic',
      iconUrl: 'icons/icon48.png',
      title: 'IDM Master',
      message: `已添加: ${item.filename || item.url.split('/').pop()}`
    });
  } catch (err) {
    console.error('[IDM Master] 拦截出错:', err);
  }
});

// ── 上下文菜单（右键下载） ──
chrome.contextMenus.create({
  id: 'idm-download-link',
  title: '使用 IDM Master 下载',
  contexts: ['link', 'image', 'video', 'audio']
});

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId !== 'idm-download-link') return;

  const url = info.srcUrl || info.linkUrl;
  if (!url) return;

  const alive = await checkHealth();
  if (!alive) {
    chrome.notifications?.create?.('offline', {
      type: 'basic',
      iconUrl: 'icons/icon48.png',
      title: 'IDM Master',
      message: '客户端未运行，请先启动 IDM Master'
    });
    return;
  }

  try {
    const resp = await fetch(`${SERVER_URL}/api/download`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        url: url,
        filename: url.split('/').pop().split('?')[0] || '',
        referer: tab?.url || '',
        cookies: '',
        user_agent: navigator.userAgent,
      }),
      signal: AbortSignal.timeout(5000),
    });

    if (resp.ok) {
      chrome.notifications?.create?.('right-click-ok', {
        type: 'basic',
        iconUrl: 'icons/icon48.png',
        title: 'IDM Master',
        message: `已添加下载: ${url.split('/').pop()?.split('?')[0]}`
      });
    }
  } catch (err) {
    console.error('[IDM Master] 右键下载失败:', err);
  }
});

// ── 点击扩展图标时向 popup 汇报客户端状态 ──
chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg.type === 'CHECK_STATUS') {
    checkHealth().then(alive => sendResponse({ alive }));
    return true; // 异步响应
  }
});
