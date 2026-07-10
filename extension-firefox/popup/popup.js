// IDM Master — Popup Script

const SERVER_URL = 'http://127.0.0.1:16888';

const statusDot = document.getElementById('statusDot');
const statusText = document.getElementById('statusText');
const taskList = document.getElementById('taskList');
const openClient = document.getElementById('openClient');

// ── 检查客户端状态 ──
async function checkStatus() {
  try {
    const resp = await fetch(`${SERVER_URL}/api/health`, { signal: AbortSignal.timeout(2000) });
    const data = await resp.json();
    if (data.ok) {
      statusDot.classList.add('online');
      statusText.textContent = '客户端已连接';
      loadTasks();
    } else {
      setOffline();
    }
  } catch {
    setOffline();
  }
}

function setOffline() {
  statusDot.classList.remove('online');
  statusText.textContent = '客户端未运行 — 下载将使用 Chrome 原生';
}

// ── 加载任务列表 ──
async function loadTasks() {
  try {
    const resp = await fetch(`${SERVER_URL}/api/tasks`, { signal: AbortSignal.timeout(3000) });
    const data = await resp.json();
    if (!data.ok || !data.data || data.data.length === 0) {
      taskList.innerHTML = '<div class="empty"><div class="icon">📂</div><div>暂无活动下载</div></div>';
      return;
    }

    const active = data.data.filter(t => t.state === 'running' || t.state === 'pending');
    if (active.length === 0) {
      taskList.innerHTML = '<div class="empty"><div class="icon">✅</div><div>暂无活动下载</div></div>';
      return;
    }

    taskList.innerHTML = active.map(t => `
      <div class="task-item">
        <div class="name" title="${esc(t.filename)}">${esc(t.filename)}</div>
        <div class="progress-bar">
          <div class="fill" style="width:${(t.progress * 100).toFixed(0)}%"></div>
        </div>
        <div class="meta">
          <span>${(t.progress * 100).toFixed(0)}%</span>
          <span>${formatSpeed(t.speed_bps)}</span>
        </div>
      </div>
    `).join('');
  } catch {
    // 忽略加载错误
  }
}

function esc(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function formatSpeed(bps) {
  if (bps >= 1_048_576) return (bps / 1_048_576).toFixed(1) + ' MB/s';
  if (bps >= 1024) return (bps / 1024).toFixed(0) + ' KB/s';
  if (bps > 0) return bps.toFixed(0) + ' B/s';
  return '等待中';
}

// ── 打开客户端 ──
openClient.addEventListener('click', () => {
  // 尝试通过自定义协议打开，或提示用户手动启动
  chrome.runtime.sendMessage({ type: 'CHECK_STATUS' }, (resp) => {
    if (!resp || !resp.alive) {
      chrome.notifications?.create?.('launch-hint', {
        type: 'basic',
        iconUrl: 'icons/icon48.png',
        title: 'IDM Master',
        message: '请手动启动 IDM Master 桌面客户端'
      });
    }
  });
});

// ── 初始加载 ──
checkStatus();

// 每 3 秒自动刷新
setInterval(checkStatus, 3000);
