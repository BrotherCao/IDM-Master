// IDM Master — Content Script
// 页面内链接捕获：Alt+Click 或检测下载链接

(function () {
  'use strict';

  const SERVER_URL = 'http://127.0.0.1:16888';

  // ── Alt+Click 快捷下载 ──
  document.addEventListener('click', async (e) => {
    if (!e.altKey) return;

    const link = findClosestLink(e.target);
    if (!link) return;

    const url = link.href;
    if (!url || !/^https?:\/\//i.test(url)) return;

    e.preventDefault();
    e.stopPropagation();

    try {
      const resp = await fetch(`${SERVER_URL}/api/download`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          url: url,
          filename: url.split('/').pop()?.split('?')[0] || '',
          referer: window.location.href,
        }),
        signal: AbortSignal.timeout(3000),
      });

      if (resp.ok) {
        showToast('✓ 已发送到 IDM Master');
      } else {
        showToast('✗ 发送失败，IDM Master 是否已启动？');
      }
    } catch (_) {
      showToast('✗ 无法连接 IDM Master，请先启动客户端');
    }
  }, true);

  function findClosestLink(el) {
    while (el && el !== document.body) {
      if (el.tagName === 'A' && el.href) return el;
      el = el.parentElement;
    }
    return null;
  }

  function showToast(msg) {
    const toast = document.createElement('div');
    toast.textContent = msg;
    toast.style.cssText = `
      position: fixed; bottom: 24px; left: 50%; transform: translateX(-50%);
      background: #1e88e5; color: #fff; padding: 10px 24px; border-radius: 8px;
      font-size: 14px; z-index: 2147483647; pointer-events: none;
      box-shadow: 0 4px 12px rgba(0,0,0,0.3); transition: opacity 0.3s;
    `;
    document.body.appendChild(toast);
    setTimeout(() => { toast.style.opacity = '0'; }, 2000);
    setTimeout(() => toast.remove(), 2500);
  }
})();
