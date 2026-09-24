// TeleVault Web Application Client Logic
let currentStatus = null;
let currentRecords = [];
let pendingPhoneCodeHash = null;

document.addEventListener('DOMContentLoaded', () => {
    initTabs();
    initDropzone();
    initModals();
    refreshAll();
    setInterval(refreshStatus, 8000);
});

// Tab Switching
function initTabs() {
    const tabs = document.querySelectorAll('.nav-tab');
    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            tabs.forEach(t => t.classList.remove('active'));
            document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));

            tab.classList.add('active');
            const targetId = tab.dataset.tab;
            const targetContent = document.getElementById(targetId);
            if (targetContent) targetContent.classList.add('active');
        });
    });
}

// Data Fetching
async function refreshAll() {
    await refreshStatus();
    await refreshRecords();
}

async function refreshStatus() {
    try {
        const res = await fetch('/api/status');
        const data = await res.json();
        currentStatus = data;

        // Update header badges
        const modeBadge = document.getElementById('header-mode-badge');
        const userBtn = document.getElementById('header-user-btn');

        if (data.authenticated && data.user) {
            modeBadge.className = 'badge badge-live';
            modeBadge.innerHTML = `<span class="badge-pulse"></span> MTProto Live: ${data.user.first_name || data.user.phone}`;
            userBtn.innerHTML = `👤 ${data.user.first_name || data.user.phone}`;
            userBtn.title = "Connected to Telegram. Click for details.";
        } else {
            modeBadge.className = 'badge badge-simulated';
            modeBadge.innerHTML = `<span class="badge-pulse"></span> Simulation Mode`;
            userBtn.innerHTML = `⚡ Connect Telegram`;
        }

        // Update Stats
        document.getElementById('stat-files').textContent = data.stats.total_files;
        document.getElementById('stat-size').textContent = formatBytes(data.stats.total_bytes);
        document.getElementById('stat-healthy').textContent = data.stats.healthy;
        document.getElementById('stat-degraded').textContent = data.stats.degraded;
        document.getElementById('stat-lost').textContent = data.stats.lost;

        // Channel Status
        const pChan = data.channels.primary_id ? `ID: ${data.channels.primary_id}` : 'Not Linked';
        const mChan = data.channels.mirror_id ? `ID: ${data.channels.mirror_id}` : 'Not Linked';
        document.getElementById('stat-channels').textContent = `${pChan} | ${mChan}`;

        // Account tab info
        renderAccountInfo(data);

        // Logs
        renderLogs(data.logs);
    } catch (e) {
        console.error("Error refreshing status:", e);
    }
}

async function refreshRecords() {
    try {
        const res = await fetch('/api/records');
        const data = await res.json();
        currentRecords = data.records || [];
        renderRecordsTable(currentRecords);
    } catch (e) {
        console.error("Error loading records:", e);
    }
}

function renderRecordsTable(records) {
    const tbody = document.getElementById('vault-table-body');
    if (!records || records.length === 0) {
        tbody.innerHTML = `<tr><td colspan="7" style="text-align: center; color: var(--text-dim); padding: 2rem;">No files backed up yet. Drop a file above to start!</td></tr>`;
        return;
    }

    tbody.innerHTML = records.map(r => {
        let stateClass = 'status-healthy';
        if (r.state === 'DEGRADED') stateClass = 'status-degraded';
        if (r.state === 'LOST') stateClass = 'status-lost';

        const sizeStr = formatBytes(r.size);
        const modeBadge = r.mode === 'PRIVATE' ? '<span class="status-pill" style="background: rgba(157,78,221,0.2); color:#c77dff;">🔒 Private</span>' : '<span style="color:var(--text-dim)">Original</span>';

        return `
            <tr>
                <td style="font-weight: 600; color: #fff;">${escapeHtml(r.name)}</td>
                <td class="code-cell">${r.id.substring(0, 8)}...</td>
                <td>${sizeStr}</td>
                <td><span class="status-pill ${stateClass}">${r.state}</span></td>
                <td>${modeBadge}</td>
                <td><span style="font-size:0.8rem; color:var(--text-muted);">${r.local_status}</span></td>
                <td>
                    <button class="btn" style="padding: 0.25rem 0.6rem; font-size: 0.75rem;" onclick="handleRestore('${r.id}')">📥 Restore</button>
                </td>
            </tr>
        `;
    }).join('');
}

function renderAccountInfo(status) {
    const container = document.getElementById('account-info-container');
    if (!container) return;

    if (status.authenticated && status.user) {
        const u = status.user;
        const premiumBadge = u.is_premium ? '<span class="badge badge-live">★ Telegram Premium</span>' : '';
        container.innerHTML = `
            <div style="background: rgba(0,0,0,0.2); padding: 1.5rem; border-radius: 12px; border: 1px solid var(--border-subtle);">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
                    <h3 style="font-size: 1.1rem; color: #fff;">Authenticated Telegram Account</h3>
                    ${premiumBadge}
                </div>
                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 1rem; margin-bottom: 1.5rem;">
                    <div><span style="color:var(--text-dim); font-size:0.75rem;">NAME</span><div style="font-weight:600;">${u.first_name} ${u.last_name || ''}</div></div>
                    <div><span style="color:var(--text-dim); font-size:0.75rem;">USERNAME</span><div style="font-weight:600;">@${u.username || 'N/A'}</div></div>
                    <div><span style="color:var(--text-dim); font-size:0.75rem;">PHONE</span><div style="font-weight:600;">${u.phone || 'N/A'}</div></div>
                    <div><span style="color:var(--text-dim); font-size:0.75rem;">USER ID</span><div class="code-cell">${u.id}</div></div>
                </div>
                <div style="margin-bottom: 1.5rem; padding: 1rem; background: rgba(0,210,255,0.05); border-radius: 8px; border: 1px solid rgba(0,210,255,0.2);">
                    <div style="font-size:0.85rem; font-weight:600; color:var(--cyan); margin-bottom:0.25rem;">Active Vault Storage Channels:</div>
                    <div style="font-size:0.8rem; color:var(--text-muted);">Primary Vault: <code>${status.channels.primary_id || 'Auto-created on backup'}</code></div>
                    <div style="font-size:0.8rem; color:var(--text-muted);">Mirror Vault: <code>${status.channels.mirror_id || 'Auto-created on backup'}</code></div>
                </div>
                <button class="btn btn-danger" onclick="handleLogout()">Disconnect Telegram Account</button>
            </div>
        `;
    } else {
        container.innerHTML = `
            <div style="background: rgba(0,0,0,0.2); padding: 2rem; border-radius: 12px; text-align: center; border: 1px solid var(--border-subtle);">
                <div style="font-size: 2.5rem; margin-bottom: 1rem;">✈️</div>
                <h3 style="font-size: 1.2rem; margin-bottom: 0.5rem;">No Telegram Account Connected</h3>
                <p style="color: var(--text-muted); font-size: 0.85rem; max-width: 480px; margin: 0 auto 1.5rem auto;">
                    TeleVault is currently running in local simulation mode. Connect your real Telegram account (MTProto user session) to back up your files directly to your private Telegram storage channels.
                </p>
                <button class="btn btn-primary" onclick="openLoginModal()">Login with Telegram</button>
            </div>
        `;
    }
}

function renderLogs(logs) {
    const term = document.getElementById('terminal-logs');
    if (!term) return;
    if (!logs || logs.length === 0) {
        term.innerHTML = '<div style="color: var(--text-dim);">Awaiting actions...</div>';
        return;
    }

    term.innerHTML = logs.map(l => {
        let msgClass = '';
        if (l.level === 'success') msgClass = 'log-msg-success';
        if (l.level === 'error') msgClass = 'log-msg-error';
        if (l.level === 'warning') msgClass = 'log-msg-warning';

        return `<div class="log-entry">
            <span class="log-cat">[${escapeHtml(l.category)}]</span>
            <span class="${msgClass}">${escapeHtml(l.message)}</span>
        </div>`;
    }).join('');
    term.scrollTop = term.scrollHeight;
}

// Dropzone Implementation
function initDropzone() {
    const dropzone = document.getElementById('dropzone');
    const fileInput = document.getElementById('file-input');
    if (!dropzone || !fileInput) return;

    ['dragenter', 'dragover'].forEach(eventName => {
        dropzone.addEventListener(eventName, (e) => {
            e.preventDefault();
            dropzone.classList.add('drag-over');
        });
    });

    ['dragleave', 'drop'].forEach(eventName => {
        dropzone.addEventListener(eventName, (e) => {
            e.preventDefault();
            dropzone.classList.remove('drag-over');
        });
    });

    dropzone.addEventListener('drop', (e) => {
        const files = e.dataTransfer.files;
        if (files.length > 0) {
            uploadFile(files[0]);
        }
    });

    dropzone.addEventListener('click', () => {
        fileInput.click();
    });

    fileInput.addEventListener('change', () => {
        if (fileInput.files.length > 0) {
            uploadFile(fileInput.files[0]);
        }
    });
}

async function uploadFile(file) {
    const isPrivate = document.getElementById('check-private').checked;
    const statusText = document.getElementById('upload-status-text');
    statusText.style.display = 'block';
    statusText.textContent = `Uploading & backing up: ${file.name}...`;

    const formData = new FormData();
    formData.append('file', file);
    formData.append('mode', isPrivate ? 'private' : 'original');

    try {
        const res = await fetch('/api/backup/upload', {
            method: 'POST',
            body: formData,
        });
        const data = await res.json();
        if (res.ok) {
            statusText.textContent = `✓ Backed up ${file.name} successfully! (Dual-write complete)`;
            statusText.style.color = 'var(--emerald)';
            setTimeout(() => { statusText.style.display = 'none'; }, 4000);
            refreshAll();
        } else {
            statusText.textContent = `✗ Backup failed: ${data.detail || 'Error'}`;
            statusText.style.color = 'var(--crimson)';
        }
    } catch (e) {
        statusText.textContent = `✗ Network error during backup: ${e.message}`;
        statusText.style.color = 'var(--crimson)';
    }
}

// Backup from Local Path
async function handlePathBackup() {
    const pathInput = document.getElementById('input-local-path');
    const path = pathInput.value.trim();
    if (!path) {
        alert("Please enter a valid file or folder path.");
        return;
    }

    const isPrivate = document.getElementById('check-private').checked;
    const btn = document.getElementById('btn-path-backup');
    btn.disabled = true;
    btn.textContent = "Processing...";

    try {
        const res = await fetch('/api/backup/path', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ path, mode: isPrivate ? 'private' : 'original' }),
        });
        const data = await res.json();
        if (res.ok) {
            alert(`Backup complete! Processed ${data.results.length} file(s).`);
            pathInput.value = '';
            refreshAll();
        } else {
            alert(`Error: ${data.detail}`);
        }
    } catch (e) {
        alert(`Request failed: ${e.message}`);
    } finally {
        btn.disabled = false;
        btn.textContent = "Back Up Local Path";
    }
}

// Restore
async function handleRestore(recordId) {
    const dest = prompt("Enter destination folder to restore to (leave blank for default):", "");
    if (dest === null) return; // user cancelled

    try {
        const url = `/api/restore/${recordId}` + (dest ? `?dest_folder=${encodeURIComponent(dest)}` : '');
        const res = await fetch(url, { method: 'POST' });
        const data = await res.json();
        if (res.ok) {
            alert(`✓ File successfully restored to:\n${data.restored_path}\nSHA-256 byte integrity verified!`);
            refreshAll();
        } else {
            alert(`Restore error: ${data.detail}`);
        }
    } catch (e) {
        alert(`Restore request failed: ${e.message}`);
    }
}

// Vault Actions
async function handleVerify() {
    try {
        const res = await fetch('/api/verify', { method: 'POST' });
        const data = await res.json();
        alert(`Verification Complete:\n• Checked: ${data.total_checked}\n• Healthy: ${data.healthy_count}\n• Degraded: ${data.degraded_count}\n• Lost: ${data.lost_count}`);
        refreshAll();
    } catch (e) {
        alert(`Verification failed: ${e.message}`);
    }
}

async function handleAutoHeal() {
    try {
        const res = await fetch('/api/verify?heal=true', { method: 'POST' });
        const data = await res.json();
        const healed = data.healing ? data.healing.healed_count : 0;
        alert(`Auto-healing Complete:\n• Degraded items healed: ${healed}`);
        refreshAll();
    } catch (e) {
        alert(`Healing failed: ${e.message}`);
    }
}

async function handleRebuild() {
    if (!confirm("Rebuild will scan your Telegram channels to reconstruct any missing index entries. Continue?")) return;
    try {
        const res = await fetch('/api/rebuild', { method: 'POST' });
        const data = await res.json();
        alert(`Rebuild Complete:\n• Reconstructed: ${data.records_reconstructed} records`);
        refreshAll();
    } catch (e) {
        alert(`Rebuild failed: ${e.message}`);
    }
}

async function handleSnapshot() {
    try {
        const res = await fetch('/api/snapshot', { method: 'POST' });
        const data = await res.json();
        alert(`✓ SQLite index snapshot created and uploaded: ${data.snapshot_file}`);
        refreshAll();
    } catch (e) {
        alert(`Snapshot failed: ${e.message}`);
    }
}

async function handleLogout() {
    if (!confirm("Are you sure you want to disconnect your Telegram account?")) return;
    try {
        await fetch('/api/logout', { method: 'POST' });
        refreshAll();
    } catch (e) {
        alert(`Logout error: ${e.message}`);
    }
}

// Telegram Login Modal
function initModals() {
    const modal = document.getElementById('login-modal');
    modal.addEventListener('click', (e) => {
        if (e.target === modal) closeLoginModal();
    });
}

function openLoginModal() {
    document.getElementById('login-modal').classList.add('active');
    document.getElementById('login-step-1').style.display = 'block';
    document.getElementById('login-step-2').style.display = 'none';
    document.getElementById('login-error-msg').style.display = 'none';
}

function closeLoginModal() {
    document.getElementById('login-modal').classList.remove('active');
}

async function submitSendCode() {
    const apiId = document.getElementById('login-api-id').value.trim();
    const apiHash = document.getElementById('login-api-hash').value.trim();
    const phone = document.getElementById('login-phone').value.trim();
    const errorEl = document.getElementById('login-error-msg');

    if (!apiId || !apiHash || !phone) {
        errorEl.textContent = "Please fill in API ID, API Hash, and Phone Number.";
        errorEl.style.display = 'block';
        return;
    }

    const btn = document.getElementById('btn-send-code');
    btn.disabled = true;
    btn.textContent = "Sending code...";
    errorEl.style.display = 'none';

    try {
        const res = await fetch('/api/login/send-code', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ api_id: apiId, api_hash: apiHash, phone }),
        });
        const data = await res.json();
        if (res.ok) {
            pendingPhoneCodeHash = data.phone_code_hash;
            document.getElementById('login-step-1').style.display = 'none';
            document.getElementById('login-step-2').style.display = 'block';
            document.getElementById('step-2-phone-label').textContent = phone;
        } else {
            errorEl.textContent = data.detail || "Failed to send code.";
            errorEl.style.display = 'block';
        }
    } catch (e) {
        errorEl.textContent = `Error: ${e.message}`;
        errorEl.style.display = 'block';
    } finally {
        btn.disabled = false;
        btn.textContent = "Send Verification Code";
    }
}

async function submitVerifyCode() {
    const code = document.getElementById('login-code').value.trim();
    const password = document.getElementById('login-password').value.trim();
    const errorEl = document.getElementById('login-error-msg');

    if (!code) {
        errorEl.textContent = "Please enter the verification code.";
        errorEl.style.display = 'block';
        return;
    }

    const btn = document.getElementById('btn-verify-code');
    btn.disabled = true;
    btn.textContent = "Verifying...";
    errorEl.style.display = 'none';

    try {
        const res = await fetch('/api/login/verify-code', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({
                code,
                phone_code_hash: pendingPhoneCodeHash,
                password: password || null,
            }),
        });
        const data = await res.json();

        if (res.ok) {
            if (data.status === '2fa_required') {
                errorEl.textContent = "Two-Factor Cloud Password is required. Please enter it below.";
                errorEl.style.display = 'block';
                document.getElementById('group-2fa').style.display = 'block';
                return;
            }

            closeLoginModal();
            alert(`🎉 Success! Logged in as ${data.user.first_name}.\nPrivate vault storage channels verified!`);
            refreshAll();
        } else {
            errorEl.textContent = data.detail || "Verification failed.";
            errorEl.style.display = 'block';
        }
    } catch (e) {
        errorEl.textContent = `Error: ${e.message}`;
        errorEl.style.display = 'block';
    } finally {
        btn.disabled = false;
        btn.textContent = "Verify & Connect";
    }
}

// Helpers
function formatBytes(bytes) {
    if (!bytes || bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function escapeHtml(str) {
    if (!str) return '';
    return String(str)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}
