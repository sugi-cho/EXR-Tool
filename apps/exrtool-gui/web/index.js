(() => {
  let invoke = null; // 解決済みの invoke（nullなら未解決）
  let imgW = 0, imgH = 0;

  function getEl(id) {
    const el = document.getElementById(id);
    if (!el) console.error(`element #${id} not found`);
    return el;
  }

  function appendLog(msg) {
    const logbox = document.getElementById('logbox');
    const ts = new Date().toISOString();
    const line = `[${ts}] ${msg}\n`;
    if (logbox) {
      logbox.textContent += line;
      logbox.scrollTop = logbox.scrollHeight;
    }
    console.log(line);
  }

  async function ensureTauriReady(timeoutMs = 5000) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      const t = window.__TAURI__;
      if (t) {
        const inv = t.invoke || (t.tauri && t.tauri.invoke) || (t.core && t.core.invoke);
        if (inv) { invoke = inv; return true; }
      }
      await new Promise(r => setTimeout(r, 50));
    }
    appendLog('Tauri API 解決に失敗');
    return false;
  }

  async function openExr() {
    const pathEl = getEl('path');
    const lutEl = getEl('lut');
    const maxEl = getEl('max');
    const expEl = getEl('exp');
    const gammaEl = getEl('gamma');
    const cv = getEl('cv');
    const info = getEl('info');
    if (!pathEl || !cv || !info) return;
    const ctx = cv.getContext('2d');

    const path = pathEl.value.trim();
    const lutPath = lutEl ? (lutEl.value.trim() || null) : null;
    try {
      if (!(await ensureTauriReady())) throw new Error('Tauri API が利用できません');
      const [w, h, b64] = await invoke('open_exr', {
        path,
        maxSize: parseInt(maxEl?.value ?? '2048', 10) || 2048,
        exposure: parseFloat(expEl?.value ?? '0'),
        gamma: parseFloat(gammaEl?.value ?? '2.2'),
        lutPath
      });
      const img = new Image();
      img.onload = () => {
        imgW = w; imgH = h;
        cv.width = w; cv.height = h;
        ctx.clearRect(0,0,w,h);
        ctx.drawImage(img, 0, 0);
        info.textContent = `preview: ${w}x${h}`;
        appendLog(`open ok: ${w}x${h}`);
      };
      img.src = 'data:image/png;base64,' + b64;
    } catch (e) {
      appendLog('読み込み失敗: ' + e);
      alert('読み込み失敗: ' + e);
    }
  }

  document.addEventListener('DOMContentLoaded', () => {
    const openBtn = getEl('open');
    const browseBtn = getEl('browse');
    const saveBtn = getEl('save');
    const cv = getEl('cv');
    const pathEl = getEl('path');

    if (openBtn) openBtn.addEventListener('click', openExr);

    if (browseBtn) browseBtn.addEventListener('click', async () => {
      try {
        const t = window.__TAURI__;
        const dialogOpen = t && (t.dialog && t.dialog.open) ? t.dialog.open : (t && t.tauri && t.tauri.dialog && t.tauri.dialog.open ? t.tauri.dialog.open : null);
        if (dialogOpen) {
          const selected = await dialogOpen({ multiple: false, filters: [{ name: 'EXR', extensions: ['exr'] }] });
          if (selected) { pathEl.value = selected; }
        } else {
          const p = prompt('EXRファイルのパスを入力');
          if (p) pathEl.value = p;
        }
      } catch (e) { appendLog('ファイルダイアログ失敗: ' + e); }
    });

    if (saveBtn) saveBtn.addEventListener('click', async () => {
      const out = prompt('保存するPNGパスを入力:', 'preview.png');
      if (!out) return;
      try {
        if (!(await ensureTauriReady())) throw new Error('Tauri API が利用できません');
        await invoke('export_preview_png', { outPath: out });
        appendLog('PNG保存: ' + out);
        alert('保存しました: ' + out);
      } catch (e) { alert('保存に失敗: ' + e); }
    });

    if (cv) cv.addEventListener('mousemove', async (ev) => {
      if (imgW === 0) return;
      const rect = cv.getBoundingClientRect();
      const x = Math.floor((ev.clientX - rect.left));
      const y = Math.floor((ev.clientY - rect.top));
      try {
        if (!(invoke || await ensureTauriReady())) return;
        const [r,g,b,a] = await invoke('probe_pixel', { px: x, py: y });
        const readout = getEl('readout');
        if (readout) readout.textContent = `x:${x}, y:${y}  linear: R ${r.toFixed(6)}  G ${g.toFixed(6)}  B ${b.toFixed(6)}  A ${a.toFixed(6)}`;
      } catch (_) { /* ignore */ }
    });

    // 早期にTauri注入が完了するケース向け
    ensureTauriReady(2000);
    appendLog('UI ready');
  });
})();

