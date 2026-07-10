// IDM Master — Content Script
// Alt+Click 下载 + 视频/音频嗅探 + 浮动下载按钮

(function () {
  'use strict';

  const SERVER_URL = 'http://127.0.0.1:16888';

  // ═══════════════════════════════════════════
  // 1. Alt+Click 快捷下载（链接）
  // ═══════════════════════════════════════════

  document.addEventListener('click', async (e) => {
    if (!e.altKey) return;
    const link = findClosestLink(e.target);
    if (!link) return;
    const url = link.href;
    if (!url || !/^https?:\/\//i.test(url)) return;
    e.preventDefault();
    e.stopPropagation();
    await sendToClient(url, url.split('/').pop()?.split('?')[0] || '');
  }, true);

  function findClosestLink(el) {
    while (el && el !== document.body) {
      if (el.tagName === 'A' && el.href) return el;
      el = el.parentElement;
    }
    return null;
  }

  // ═══════════════════════════════════════════
  // 2. 视频/音频元素嗅探 — 浮动下载按钮
  // ═══════════════════════════════════════════

  const MEDIA_FILE_PATTERNS = /\.(mp4|mkv|webm|flv|avi|mov|wmv|m4v|3gp|ts|m3u8|mpd|mp3|wav|flac|aac|ogg|wma|m4a|opus)(\?.*)?$/i;

  function findMediaElements() {
    const media = [];
    // <video> / <audio> 直接元素
    document.querySelectorAll('video, audio').forEach(el => {
      const src = el.currentSrc || el.src;
      if (src && /^https?:\/\//i.test(src)) media.push({ el, url: src });

      el.querySelectorAll('source').forEach(s => {
        if (s.src && /^https?:\/\//i.test(s.src)) media.push({ el, url: s.src });
      });
    });
    return media;
  }

  function isMediaUrl(url) {
    try {
      const u = new URL(url, location.href);
      return MEDIA_FILE_PATTERNS.test(u.pathname);
    } catch { return false; }
  }

  // 为检测到的媒体元素添加浮动下载按钮
  function injectMediaButtons() {
    const media = findMediaElements();
    media.forEach(({ el, url }) => {
      // 防止重复注入
      if (el.dataset.idmInjected) return;
      el.dataset.idmInjected = '1';

      const wrapper = document.createElement('div');
      wrapper.className = 'idm-master-media-wrapper';
      wrapper.style.cssText = 'position:relative;display:inline-block;';

      el.parentNode?.insertBefore(wrapper, el);
      wrapper.appendChild(el);

      const btn = document.createElement('button');
      btn.className = 'idm-master-download-btn';
      btn.innerHTML = '⬇';
      btn.title = '使用 IDM Master 下载此媒体';
      btn.style.cssText = `
        position:absolute; top:8px; right:8px; z-index:9999;
        width:36px; height:36px; border-radius:50%; border:none;
        background:#1e88e5; color:#fff; font-size:18px; cursor:pointer;
        box-shadow:0 2px 8px rgba(0,0,0,0.4); opacity:0;
        transition:opacity 0.2s; display:flex; align-items:center; justify-content:center;
      `;

      btn.addEventListener('mouseenter', () => { btn.style.opacity = '1'; });
      btn.addEventListener('mouseleave', () => { btn.style.opacity = '0'; });
      wrapper.addEventListener('mouseenter', () => { btn.style.opacity = '1'; });
      wrapper.addEventListener('mouseleave', () => { btn.style.opacity = '0'; });

      btn.addEventListener('click', async (e) => {
        e.preventDefault();
        e.stopPropagation();
        const filename = url.split('/').pop()?.split('?')[0] || 'media';
        await sendToClient(url, filename);
      });

      wrapper.appendChild(btn);
    });
  }

  // 初始扫描 + DOM 变化监听
  injectMediaButtons();
  const observer = new MutationObserver(() => injectMediaButtons());
  observer.observe(document.body, { childList: true, subtree: true });

  // 定期重扫描（应对动态加载）
  setInterval(injectMediaButtons, 3000);

  // ═══════════════════════════════════════════
  // 3. 网络请求拦截 — 检测媒体流
  // ═══════════════════════════════════════════

  // 拦截 XMLHttpRequest
  const origOpen = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (method, url) {
    this._idmUrl = url;
    return origOpen.apply(this, arguments);
  };
  const origSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.send = function () {
    if (this._idmUrl && isMediaUrl(this._idmUrl)) {
      notifyMediaDetected(this._idmUrl);
    }
    return origSend.apply(this, arguments);
  };

  // 拦截 fetch
  const origFetch = window.fetch;
  window.fetch = function (input) {
    const url = typeof input === 'string' ? input : (input.url || '');
    if (isMediaUrl(url)) {
      notifyMediaDetected(url);
    }
    return origFetch.apply(this, arguments);
  };

  // 已通知的 URL（避免重复提示）
  const notifiedUrls = new Set();

  function notifyMediaDetected(url) {
    if (notifiedUrls.has(url)) return;
    notifiedUrls.add(url);
    const filename = url.split('/').pop()?.split('?')[0] || 'media';
    showToast(`🎬 检测到媒体: ${filename} — Alt+Click 下载`);
  }

  // ═══════════════════════════════════════════
  // 4. 向客户端发送下载请求
  // ═══════════════════════════════════════════

  async function sendToClient(url, filename) {
    try {
      const resp = await fetch(`${SERVER_URL}/api/download`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          url: url,
          filename: filename,
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
  }

  function showToast(msg) {
    const toast = document.createElement('div');
    toast.textContent = msg;
    toast.style.cssText = `
      position:fixed; bottom:24px; left:50%; transform:translateX(-50%);
      background:#1e88e5; color:#fff; padding:10px 24px; border-radius:8px;
      font-size:14px; z-index:2147483647; pointer-events:none;
      box-shadow:0 4px 12px rgba(0,0,0,0.3); transition:opacity 0.3s;
    `;
    document.body.appendChild(toast);
    setTimeout(() => { toast.style.opacity = '0'; }, 2500);
    setTimeout(() => toast.remove(), 3000);
  }
})();
