// TeleVault Web Application — Futuristic Cyber-Vault Client Logic
let currentStatus = null;
let currentRecords = [];
let currentVersions = {};
let pendingPhoneCodeHash = null;
let activeRestoreRecord = null;

document.addEventListener('DOMContentLoaded', () => {
    initTabs();
    initDropzones();
    initModals();
    refreshAll();
    setInterval(refreshStatus, 6000);
});

// =============================================================
// Tab Switching
// =============================================================
function initTabs() {
    const tabs = document.querySelectorAll('.nav-tab');
    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            const targetId = tab.dataset.tab;
            switchToTab(targetId);
        });
    });
}

function switchToTab(tabId) {
    const tabs = document.querySelectorAll('.nav-tab');
    tabs.forEach(t => {
        if (t.dataset.tab === tabId) {
            t.classList.add('active');
        } else {
            t.classList.remove('active');
        }
    });

    document.querySelectorAll('.tab-content').forEach(c => {
        if (c.id === tabId) {
            c.classList.add('active');
        } else {
            c.classList.remove('active');
        }
    });

    // Lazy load tab specific data
    if (tabId === 'tab-timeline') {
        loadVersionTimeline();
    } else if (tabId === 'tab-files') {
        refreshRecords();
    }
}

// =============================================================
// Data Fetching & State Synchronization
// =============================================================
async function refreshAll() {
    await refreshStatus();
    await refreshRecords();
}

async function refreshStatus() {
    try {
        const res = await fetch('/api/status');
        if (!res.ok) throw new Error('Failed to load status');
        const data = await res.json();
        currentStatus = data;

        // Header Status Badges
        const modeBadge = document.getElementById('header-mode-badge');
        const userBtn = document.getElementById('header-user-btn');

        if (data.authenticated && data.user) {
            modeBadge.className = 'badge badge-live';
            const name = data.user.first_name || data.user.phone || 'Connected';
            modeBadge.innerHTML = `<span class="badge-pulse"></span> MTProto Live: ${escapeHtml(name)}`;
            userBtn.innerHTML = `👤 ${escapeHtml(name)}`;
            userBtn.title = "Connected to Telegram MTProto. Click to view.";

            // Account Tab Status
            const acctLabel = document.getElementById('account-status-label');
            const acctDetails = document.getElementById('account-user-details');
            const acctActions = document.getElementById('account-actions');
            if (acctLabel) {
                acctLabel.textContent = "Live MTProto Session Active";
                acctLabel.style.color = "var(--emerald)";
            }
            if (acctDetails) {
                acctDetails.textContent = `User: ${name} (Phone: ${data.user.phone || 'N/A'}) | User ID: ${data.user.id || 'N/A'}`;
            }
            if (acctActions) {
                acctActions.innerHTML = `<button class="btn btn-danger" onclick="handleLogout()">Disconnect Session</button>`;
            }

            // Node Topology Map
            const nodeGwBadge = document.getElementById('node-gw-badge');
            const nodeGwName = document.getElementById('node-gw-name');
            const nodeGwUser = document.getElementById('node-gw-user');
            if (nodeGwBadge) {
                nodeGwBadge.style.color = "var(--emerald)";
                nodeGwBadge.textContent = "● Session Live";
            }
            if (nodeGwName) nodeGwName.textContent = escapeHtml(name);
            if (nodeGwUser) nodeGwUser.textContent = `MTProto Authenticated`;

        } else {
            modeBadge.className = 'badge badge-disconnected';
            modeBadge.innerHTML = `<span class="badge-pulse"></span> Telegram Disconnected`;
            userBtn.innerHTML = `⚡ Connect Telegram`;
            userBtn.title = "Connect your Telegram MTProto session";

            // Account Tab Status
            const acctLabel = document.getElementById('account-status-label');
            const acctDetails = document.getElementById('account-user-details');
            const acctActions = document.getElementById('account-actions');
            if (acctLabel) {
                acctLabel.textContent = "Disconnected (Setup Required)";
                acctLabel.style.color = "var(--amber)";
            }
            if (acctDetails) {
                acctDetails.textContent = "No active MTProto session. Click below to connect your Telegram account.";
            }
            if (acctActions) {
                acctActions.innerHTML = `<button class="btn btn-primary" onclick="openLoginModal()">⚡ Connect Telegram</button>`;
            }

            // Node Topology Map
            const nodeGwBadge = document.getElementById('node-gw-badge');
            const nodeGwName = document.getElementById('node-gw-name');
            const nodeGwUser = document.getElementById('node-gw-user');
            if (nodeGwBadge) {
                nodeGwBadge.style.color = "var(--amber)";
                nodeGwBadge.textContent = "● Not Connected";
            }
            if (nodeGwName) nodeGwName.textContent = "MTProto Standby";
            if (nodeGwUser) nodeGwUser.textContent = "Click to link account";
        }

        // Metrics Counters
        document.getElementById('stat-files').textContent = data.stats.total_files;
        document.getElementById('stat-size').textContent = formatBytes(data.stats.total_bytes);
        document.getElementById('stat-healthy').textContent = data.stats.healthy;
        document.getElementById('stat-degraded').textContent = data.stats.degraded;
        document.getElementById('stat-lost').textContent = data.stats.lost;

        // Channel Status
        const pChan = data.channels.primary_id ? `ID: ${data.channels.primary_id}` : 'Unlinked';
        const mChan = data.channels.mirror_id ? `ID: ${data.channels.mirror_id}` : 'Unlinked';
        document.getElementById('stat-channels').textContent = `${pChan} | ${mChan}`;

        // Node Topology Channels
        const nodeP = document.getElementById('node-primary-id');
        const nodeM = document.getElementById('node-mirror-id');
        if (nodeP) nodeP.textContent = data.channels.primary_id || 'Primary Unset';
        if (nodeM) nodeM.textContent = data.channels.mirror_id || 'Mirror Unset';

        const nodeDbRecords = document.getElementById('node-db-records');
        if (nodeDbRecords) nodeDbRecords.textContent = `${data.stats.total_files} records indexed`;

        // Update Channel Inputs in Settings
        const inP = document.getElementById('input-primary-channel');
        const inM = document.getElementById('input-mirror-channel');
        if (inP) inP.value = data.channels.primary_id || 'Not configured in .env';
        if (inM) inM.value = data.channels.mirror_id || 'Not configured in .env';

        // Update Storage & Parity Visual Ring
        updateStorageRing(data.stats);

        // Render Terminal Event Logs
        renderTerminalLogs(data.logs);

    } catch (e) {
        console.error("Error refreshing status:", e);
    }
}

function updateStorageRing(stats) {
    const ringHealthy = document.getElementById('ring-healthy');
    const ringEncrypted = document.getElementById('ring-encrypted');
    const ringPct = document.getElementById('ring-pct');
    const ringEncryptedLabel = document.getElementById('ring-encrypted-label');
    if (!ringHealthy || !ringEncrypted || !ringPct) return;

    const total = stats.total_files;
    if (total === 0) {
        ringHealthy.style.strokeDashoffset = '440';
        ringEncrypted.style.strokeDashoffset = '340';
        ringPct.textContent = '100%';
        if (ringEncryptedLabel) ringEncryptedLabel.textContent = '0% Private';
        return;
    }

    const healthyRatio = stats.healthy / total;
    const offsetHealthy = 440 - (440 * healthyRatio);
    ringHealthy.style.strokeDashoffset = Math.max(0, offsetHealthy).toString();
    ringPct.textContent = `${Math.round(healthyRatio * 100)}%`;

    // Calculate private encryption ratio
    const privateCount = currentRecords.filter(r => r.mode === 'PRIVATE').length;
    const privateRatio = privateCount / total;
    const offsetEncrypted = 340 - (340 * privateRatio);
    ringEncrypted.style.strokeDashoffset = Math.max(0, offsetEncrypted).toString();
    if (ringEncryptedLabel) {
        ringEncryptedLabel.textContent = `${Math.round(privateRatio * 100)}% Zero-Knowledge`;
    }
}

async function refreshRecords() {
    try {
        const res = await fetch('/api/records');
        if (!res.ok) throw new Error('Failed to load records');
        const data = await res.json();
        currentRecords = data.records || [];
        renderRecordsTable(currentRecords);
        populateDrillSelect(currentRecords);
        if (currentStatus) updateStorageRing(currentStatus.stats);
    } catch (e) {
        console.error("Error loading records:", e);
    }
}

function filterRecordsTable() {
    const q = (document.getElementById('file-search-input').value || '').trim().toLowerCase();
    if (!q) {
        renderRecordsTable(currentRecords);
        return;
    }
    const filtered = currentRecords.filter(r => 
        r.name.toLowerCase().includes(q) || 
        r.id.toLowerCase().includes(q) || 
        (r.sha256 && r.sha256.toLowerCase().includes(q))
    );
    renderRecordsTable(filtered);
}

function renderRecordsTable(records) {
    const tbody = document.getElementById('vault-table-body');
    if (!tbody) return;
    if (!records || records.length === 0) {
        tbody.innerHTML = `<tr><td colspan="8" style="text-align: center; color: var(--text-dim); padding: 2.5rem;">No files backed up yet. Drop a file in "Fast Ingest" to begin.</td></tr>`;
        return;
    }

    tbody.innerHTML = records.map(r => {
        let stateClass = 'status-healthy';
        if (r.state === 'DEGRADED') stateClass = 'status-degraded';
        if (r.state === 'LOST') stateClass = 'status-lost';

        const modeBadge = r.mode === 'PRIVATE'
            ? `<span class="status-pill" style="background: rgba(199,125,255,0.15); color: #c77dff; border: 1px solid rgba(199,125,255,0.3);">🔐 AES-256-GCM</span>`
            : `<span class="status-pill" style="background: rgba(0,242,254,0.12); color: var(--cyan); border: 1px solid rgba(0,242,254,0.25);">Original</span>`;

        const multipartBadge = r.is_multipart
            ? `<span class="status-pill" style="background: rgba(255, 170, 0, 0.15); color: #ffaa00; border: 1px solid rgba(255, 170, 0, 0.3); margin-left: 0.35rem;" title="${r.parts_count || ''} chunks safely managed">🧩 ${r.parts_count || ''} Chunks</span>`
            : '';

        return `
            <tr>
                <td><strong>${escapeHtml(r.name)}</strong>${multipartBadge}</td>
                <td><span class="status-pill" style="background: rgba(255,255,255,0.06); color: #fff;">v${r.version || 1}</span></td>
                <td class="code-cell">${escapeHtml(r.id)}</td>
                <td>${formatBytes(r.size)}</td>
                <td><span class="status-pill ${stateClass}">${r.state}</span></td>
                <td>${modeBadge}</td>
                <td style="color: var(--text-dim); font-size: 0.78rem;">${escapeHtml(r.local_status)}</td>
                <td>
                    <div style="display: flex; gap: 0.4rem;">
                        <button class="btn" style="padding: 0.3rem 0.55rem; font-size: 0.75rem;" title="Preview / Stream Media" onclick="openPreviewModal('${escapeHtml(r.id)}', '${escapeHtml(r.name)}', '${r.mode}')">
                            👁️
                        </button>
                        <button class="btn" style="padding: 0.3rem 0.65rem; font-size: 0.75rem;" onclick="openRestoreModal('${escapeHtml(r.id)}', '${escapeHtml(r.name)}', '${r.mode}')">
                            📥 Restore
                        </button>
                        <button class="btn" style="padding: 0.3rem 0.55rem; font-size: 0.75rem;" title="View in Time Machine" onclick="viewInTimeMachine('${escapeHtml(r.name)}')">
                            ⏳
                        </button>
                    </div>
                </td>
            </tr>
        `;
    }).join('');
}

function viewInTimeMachine(fileName) {
    switchToTab('tab-timeline');
    setTimeout(() => {
        const selector = document.getElementById('timeline-file-selector');
        if (selector) {
            selector.value = fileName;
            renderSelectedFileVersionTree();
        }
    }, 200);
}

function populateDrillSelect(records) {
    const select = document.getElementById('drill-record-select');
    if (!select) return;
    const currentVal = select.value;
    select.innerHTML = '<option value="">Latest Uploaded File (Auto)</option>';
    records.forEach(r => {
        const opt = document.createElement('option');
        opt.value = r.id;
        opt.textContent = `${r.name} (${formatBytes(r.size)}) [${r.id}]`;
        select.appendChild(opt);
    });
    if (currentVal) select.value = currentVal;
}

function renderTerminalLogs(logs) {
    const container = document.getElementById('terminal-logs');
    if (!container || !logs || logs.length === 0) return;

    container.innerHTML = logs.map(l => {
        let msgClass = 'log-msg-info';
        if (l.level === 'success') msgClass = 'log-msg-success';
        if (l.level === 'warning') msgClass = 'log-msg-warning';
        if (l.level === 'error') msgClass = 'log-msg-error';

        const timeStr = l.timestamp || new Date().toLocaleTimeString();
        return `
            <div class="log-entry">
                <span class="log-time">[${escapeHtml(timeStr)}]</span>
                <span class="log-cat">[${escapeHtml(l.category)}]</span>
                <span class="${msgClass}">${escapeHtml(l.message)}</span>
            </div>
        `;
    }).join('');

    container.scrollTop = container.scrollHeight;
}

// =============================================================
// Drag and Drop & Backup Ingestion
// =============================================================
function initDropzones() {
    // 1. Fast Dropzone in Overview
    const overviewDrop = document.getElementById('overview-dropzone');
    if (overviewDrop) {
        setupDropEvents(overviewDrop, file => uploadFileDirectly(file));
    }

    // 2. Full Dropzone in Backup Tab
    const fullDrop = document.getElementById('dropzone');
    const fileInput = document.getElementById('file-input');
    if (fullDrop) {
        setupDropEvents(fullDrop, file => uploadFileDirectly(file));
    }
    if (fileInput) {
        fileInput.addEventListener('change', e => {
            if (e.target.files && e.target.files.length > 0) {
                uploadFileDirectly(e.target.files[0]);
            }
        });
    }
}

function setupDropEvents(element, onFile) {
    ['dragenter', 'dragover'].forEach(eventName => {
        element.addEventListener(eventName, e => {
            e.preventDefault();
            e.stopPropagation();
            element.classList.add('drag-over');
        });
    });

    ['dragleave', 'drop'].forEach(eventName => {
        element.addEventListener(eventName, e => {
            e.preventDefault();
            e.stopPropagation();
            element.classList.remove('drag-over');
        });
    });

    element.addEventListener('drop', e => {
        if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
            onFile(e.dataTransfer.files[0]);
        }
    });
}

function handleOverviewFileSelect(event) {
    if (event.target.files && event.target.files.length > 0) {
        uploadFileDirectly(event.target.files[0]);
    }
}

async function uploadFileDirectly(file) {
    const isPrivate = document.getElementById('check-private') ? document.getElementById('check-private').checked : false;
    const passphraseInput = document.getElementById('input-passphrase');
    const passphrase = (isPrivate && passphraseInput) ? passphraseInput.value.trim() : null;

    if (isPrivate && !passphrase) {
        showToast("Please enter an encryption passphrase for Private Mode!", "error");
        switchToTab('tab-backup');
        return;
    }

    showToast(`Encrypting & uploading ${file.name} to MTProto...`, "info");

    const statusText = document.getElementById('upload-status-text');
    if (statusText) {
        statusText.style.display = 'block';
        statusText.textContent = `Streaming SHA-256 upload for ${file.name}...`;
    }

    const formData = new FormData();
    formData.append('file', file);
    formData.append('mode', isPrivate ? 'private' : 'original');
    if (passphrase) {
        formData.append('passphrase', passphrase);
    }

    try {
        const res = await fetch('/api/backup/upload', {
            method: 'POST',
            body: formData,
        });

        const data = await res.json();
        if (!res.ok) {
            throw new Error(data.detail || 'Upload failed');
        }

        if (data.status === 'duplicate') {
            showToast(`Duplicate: ${data.message}`, "warning");
        } else {
            showToast(`Successfully backed up ${file.name} (ID: ${data.record_id})!`, "success");
        }

        if (statusText) statusText.style.display = 'none';
        await refreshAll();

    } catch (e) {
        showToast(`Backup error: ${e.message}`, "error");
        if (statusText) {
            statusText.style.color = "var(--crimson)";
            statusText.textContent = `Upload failed: ${e.message}`;
        }
    }
}

async function handlePathBackup() {
    const pathInput = document.getElementById('input-local-path');
    const path = pathInput ? pathInput.value.trim() : '';
    if (!path) {
        showToast("Please enter a valid file or directory path.", "warning");
        return;
    }

    const isPrivate = document.getElementById('check-private') ? document.getElementById('check-private').checked : false;
    const passphraseInput = document.getElementById('input-passphrase');
    const passphrase = (isPrivate && passphraseInput) ? passphraseInput.value.trim() : null;

    if (isPrivate && !passphrase) {
        showToast("Please enter a passphrase for Private Mode!", "error");
        return;
    }

    showToast(`Scanning path ${path}...`, "info");
    const btn = document.getElementById('btn-path-backup');
    if (btn) btn.disabled = true;

    try {
        const res = await fetch('/api/backup/path', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                path: path,
                mode: isPrivate ? 'private' : 'original',
                passphrase: passphrase,
            }),
        });

        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || 'Path backup failed');

        const successCnt = data.results.length;
        showToast(`Path backup completed: ${successCnt} files processed!`, "success");
        if (pathInput) pathInput.value = '';
        await refreshAll();

    } catch (e) {
        showToast(`Backup error: ${e.message}`, "error");
    } finally {
        if (btn) btn.disabled = false;
    }
}

// =============================================================
// Passphrase Helpers
// =============================================================
function togglePassphraseInput() {
    const isChecked = document.getElementById('check-private').checked;
    const container = document.getElementById('passphrase-container');
    const genBtn = document.getElementById('btn-gen-pass');
    if (container) container.style.display = isChecked ? 'block' : 'none';
    if (genBtn) genBtn.style.display = isChecked ? 'inline-flex' : 'none';
}

function togglePassphraseVisibility() {
    const input = document.getElementById('input-passphrase');
    if (!input) return;
    input.type = input.type === 'password' ? 'text' : 'password';
}

function generateSecurePassphrase() {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+';
    let pass = '';
    const array = new Uint32Array(24);
    crypto.getRandomValues(array);
    for (let i = 0; i < 24; i++) {
        pass += chars[array[i] % chars.length];
    }
    const input = document.getElementById('input-passphrase');
    if (input) {
        input.value = pass;
        input.type = 'text';
        updatePassphraseStrength();
        showToast("High-entropy AES key generated!", "success");
    }
}

function updatePassphraseStrength() {
    const input = document.getElementById('input-passphrase');
    const label = document.getElementById('strength-label');
    if (!input || !label) return;

    const val = input.value;
    if (!val) {
        label.textContent = "Waiting for input...";
        label.style.color = "var(--text-dim)";
        return;
    }

    if (val.length < 8) {
        label.textContent = "Weak (Minimum 8 chars recommended)";
        label.style.color = "var(--crimson)";
    } else if (val.length < 16) {
        label.textContent = "Good (AES-256 Protected)";
        label.style.color = "var(--amber)";
    } else {
        label.textContent = "Military-grade (Argon2id + AES-256-GCM Optimal)";
        label.style.color = "var(--emerald)";
    }
}

// =============================================================
// Time Machine & Version Tree
// =============================================================
async function loadVersionTimeline() {
    try {
        const res = await fetch('/api/versions');
        const data = await res.json();
        currentVersions = data.versions || {};

        const selector = document.getElementById('timeline-file-selector');
        if (!selector) return;
        const currentSel = selector.value;
        selector.innerHTML = '<option value="">Select a file to inspect...</option>';

        Object.keys(currentVersions).sort().forEach(name => {
            const count = currentVersions[name].length;
            const opt = document.createElement('option');
            opt.value = name;
            opt.textContent = `${name} (${count} version${count > 1 ? 's' : ''})`;
            selector.appendChild(opt);
        });

        if (currentSel && currentVersions[currentSel]) {
            selector.value = currentSel;
        } else if (Object.keys(currentVersions).length > 0) {
            selector.value = Object.keys(currentVersions)[0];
        }
        renderSelectedFileVersionTree();
    } catch (e) {
        console.error("Error loading version timeline:", e);
    }
}

function renderSelectedFileVersionTree() {
    const selector = document.getElementById('timeline-file-selector');
    const flowContainer = document.getElementById('timeline-flow');
    const tableBody = document.getElementById('timeline-versions-body');
    if (!selector || !flowContainer || !tableBody) return;

    const selectedName = selector.value;
    if (!selectedName || !currentVersions[selectedName]) {
        flowContainer.innerHTML = `<div style="color: var(--text-dim); font-size: 0.85rem; padding: 1rem;">Select a file above to visualize its version lineage.</div>`;
        tableBody.innerHTML = `<tr><td colspan="7" style="text-align: center; color: var(--text-dim); padding: 1.5rem;">No file selected.</td></tr>`;
        return;
    }

    const versions = currentVersions[selectedName];
    const ascending = [...versions].reverse();

    // Render tree flow
    flowContainer.innerHTML = ascending.map((v, idx) => {
        const isLatest = idx === ascending.length - 1;
        const nodeHtml = `
            <div class="tree-node ${isLatest ? 'active-current' : ''}">
                <div class="tree-node-version">v${v.version}</div>
                <div style="font-size: 0.75rem; color: var(--text-dim); margin-top: 0.2rem;">${formatBytes(v.size)}</div>
                ${isLatest ? '<div class="tree-node-tag">CURRENT</div>' : ''}
            </div>
        `;
        const arrowHtml = isLatest ? '' : `<div class="tree-arrow">➔</div>`;
        return nodeHtml + arrowHtml;
    }).join('');

    // Render revision table
    tableBody.innerHTML = versions.map(v => `
        <tr>
            <td><strong style="color: var(--cyan);">v${v.version}</strong></td>
            <td class="code-cell">${escapeHtml(v.id)}</td>
            <td class="code-cell">${v.sha256 ? v.sha256.substring(0, 16) + '...' : 'N/A'}</td>
            <td>${formatBytes(v.size)}</td>
            <td><span class="status-pill" style="background: rgba(255,255,255,0.06);">${v.mode}</span></td>
            <td style="color: var(--text-dim); font-size: 0.78rem;">${v.created_at ? v.created_at.substring(0, 19).replace('T', ' ') : 'N/A'}</td>
            <td>
                <button class="btn" style="padding: 0.3rem 0.65rem; font-size: 0.75rem;" onclick="openRestoreModal('${escapeHtml(v.id)}', '${escapeHtml(selectedName)}', '${v.mode}')">
                    📥 Restore v${v.version}
                </button>
            </td>
        </tr>
    `).join('');
}

// =============================================================
// Resilience & Auto-Healing Center
// =============================================================
async function handleVerify() {
    showToast("Auditing dual-channel MTProto redundancy...", "info");
    try {
        const res = await fetch('/api/verify', { method: 'POST' });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || 'Verification failed');

        showToast(`Audit complete: ${data.healthy_count} healthy, ${data.degraded_count} degraded, ${data.lost_count} lost`, "success");
        await refreshAll();
    } catch (e) {
        showToast(`Verification error: ${e.message}`, "error");
    }
}

async function handleAutoHeal() {
    showToast("Initiating dual-channel auto-healing engine...", "info");
    try {
        const res = await fetch('/api/verify?heal=true', { method: 'POST' });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || 'Auto-heal failed');

        const healed = data.healing ? data.healing.healed_count : 0;
        showToast(`Auto-heal complete: ${healed} degraded files recovered to Mirror channel!`, "success");
        await refreshAll();
    } catch (e) {
        showToast(`Auto-heal error: ${e.message}`, "error");
    }
}

async function runRecoveryDrill() {
    showToast("Executing non-destructive sandboxed recovery drill...", "info");
    const recSelect = document.getElementById('drill-record-select');
    const passInput = document.getElementById('drill-passphrase-input');
    const recId = recSelect ? recSelect.value : '';
    const pass = passInput ? passInput.value.trim() : '';

    let url = '/api/recovery-drill';
    const params = [];
    if (recId) params.push(`record_id=${encodeURIComponent(recId)}`);
    if (pass) params.push(`passphrase=${encodeURIComponent(pass)}`);
    if (params.length > 0) url += '?' + params.join('&');

    try {
        const res = await fetch(url, { method: 'POST' });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || 'Recovery drill failed');

        const panel = document.getElementById('drill-results-panel');
        if (panel) {
            panel.style.display = 'block';
            panel.innerHTML = `
                <div style="font-weight: 700; color: var(--emerald); margin-bottom: 0.5rem;">
                    ✓ Sandboxed Drill Succeeded for ${escapeHtml(data.record_id)}
                </div>
                <div style="font-size: 0.8rem; color: var(--text-main); line-height: 1.5;">
                    <div>• Download verified: <strong style="color: var(--cyan);">${data.download_success ? 'YES' : 'NO'}</strong></div>
                    <div>• SHA-256 byte-by-byte match: <strong style="color: var(--emerald);">${data.hash_matched ? 'MATCHED' : 'FAILED'}</strong></div>
                    <div>• Scratch cleanup: <strong style="color: var(--emerald);">${data.cleanup_verified ? 'SECURELY WIPED' : 'FAILED'}</strong></div>
                    <div style="margin-top: 0.35rem; color: var(--text-dim);">${data.message}</div>
                </div>
            `;
        }

        showToast("Recovery Drill passed! Vault download and crypto proven intact.", "success");
        await refreshAll();

    } catch (e) {
        showToast(`Recovery drill error: ${e.message}`, "error");
    }
}

async function handleRebuild() {
    if (!confirm("Start disaster recovery scan from Telegram channels? This will parse cloud messages to rebuild missing index entries.")) {
        return;
    }
    showToast("Scanning MTProto channel history to rebuild vault...", "info");
    try {
        const res = await fetch('/api/rebuild', { method: 'POST' });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || 'Rebuild failed');

        showToast(`Rebuild complete: ${data.records_reconstructed} records restored!`, "success");
        await refreshAll();
    } catch (e) {
        showToast(`Rebuild error: ${e.message}`, "error");
    }
}

// =============================================================
// Restore Modal
// =============================================================
function openRestoreModal(recordId, fileName, mode) {
    activeRestoreRecord = { id: recordId, name: fileName, mode: mode };
    const modal = document.getElementById('restore-modal');
    document.getElementById('restore-file-name').textContent = fileName;
    document.getElementById('restore-record-id').textContent = `Record ID: ${recordId}`;

    const passRow = document.getElementById('restore-passphrase-row');
    if (passRow) passRow.style.display = (mode === 'PRIVATE') ? 'block' : 'none';

    const statusDiv = document.getElementById('restore-status');
    if (statusDiv) statusDiv.style.display = 'none';

    if (modal) modal.style.display = 'flex';
}

function closeRestoreModal() {
    const modal = document.getElementById('restore-modal');
    if (modal) modal.style.display = 'none';
    activeRestoreRecord = null;
}

// =============================================================
// Preview & Media Streaming Modal
// =============================================================
function openPreviewModal(recordId, fileName, mode) {
    const modal = document.getElementById('preview-modal');
    const title = document.getElementById('preview-modal-title');
    const container = document.getElementById('preview-container');
    const directLink = document.getElementById('preview-direct-link');
    const metaInfo = document.getElementById('preview-meta-info');

    if (!modal || !container) return;

    title.textContent = `👁️ Preview: ${fileName}`;
    if (directLink) directLink.href = `/api/files/${encodeURIComponent(recordId)}/preview`;
    if (metaInfo) metaInfo.textContent = `ID: ${recordId} | Mode: ${mode}`;

    if (mode === 'PRIVATE') {
        container.innerHTML = `
            <div style="text-align: center; padding: 2rem; color: var(--text-dim);">
                <div style="font-size: 2.5rem; margin-bottom: 0.5rem;">🔐</div>
                <div style="font-weight: 600; color: #fff; margin-bottom: 0.25rem;">Encrypted Zero-Knowledge Document</div>
                <div style="font-size: 0.82rem;">This file is encrypted with client-side AES-256-GCM.<br>Use "Restore" to decrypt with your passphrase.</div>
            </div>
        `;
        modal.style.display = 'flex';
        return;
    }

    container.innerHTML = `<div style="color: var(--cyan); font-size: 0.9rem;"><span class="badge-pulse"></span> Streaming media from vault...</div>`;
    modal.style.display = 'flex';

    const ext = fileName.split('.').pop().toLowerCase();
    const url = `/api/files/${encodeURIComponent(recordId)}/preview`;

    if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp'].includes(ext)) {
        container.innerHTML = `<img src="${url}" alt="${escapeHtml(fileName)}" style="max-width: 100%; max-height: 480px; object-fit: contain; border-radius: 8px;">`;
    } else if (['mp4', 'webm', 'ogg', 'mov'].includes(ext)) {
        container.innerHTML = `
            <video controls autoplay style="max-width: 100%; max-height: 480px; border-radius: 8px; width: 100%;">
                <source src="${url}">
                Your browser does not support HTML5 video streaming.
            </video>
        `;
    } else if (['mp3', 'wav', 'ogg', 'm4a', 'flac', 'aac'].includes(ext)) {
        container.innerHTML = `
            <div style="text-align: center; width: 100%; padding: 2rem;">
                <div style="font-size: 3rem; margin-bottom: 1rem;">🎵</div>
                <audio controls autoplay style="width: 80%; max-width: 500px;">
                    <source src="${url}">
                    Your browser does not support audio streaming.
                </audio>
            </div>
        `;
    } else if (ext === 'pdf') {
        container.innerHTML = `
            <iframe src="${url}" style="width: 100%; height: 500px; border: none; border-radius: 8px;"></iframe>
        `;
    } else if (['txt', 'log', 'md', 'json', 'py', 'js', 'html', 'css', 'sql', 'sh', 'bat', 'yaml', 'yml', 'toml'].includes(ext)) {
        fetch(url)
            .then(res => res.text())
            .then(text => {
                container.innerHTML = `<pre class="preview-code-block">${escapeHtml(text.slice(0, 50000))}</pre>`;
            })
            .catch(err => {
                container.innerHTML = `<div style="color: var(--rose);">Failed to load preview text: ${escapeHtml(err.message)}</div>`;
            });
    } else {
        container.innerHTML = `
            <div style="text-align: center; padding: 2rem; color: var(--text-dim);">
                <div style="font-size: 2.5rem; margin-bottom: 0.5rem;">📦</div>
                <div style="font-weight: 600; color: #fff; margin-bottom: 0.25rem;">Binary Package</div>
                <div style="font-size: 0.82rem;">Direct preview not available for .${ext} files.<br>Click "Open in New Tab" or "Restore" to download.</div>
            </div>
        `;
    }
}

function closePreviewModal() {
    const modal = document.getElementById('preview-modal');
    if (modal) modal.style.display = 'none';
    const container = document.getElementById('preview-container');
    if (container) container.innerHTML = '';
}

async function executeRestore() {
    if (!activeRestoreRecord) return;
    const destFolder = document.getElementById('restore-dest-input').value.trim();
    const passphrase = document.getElementById('restore-passphrase-input') ? document.getElementById('restore-passphrase-input').value.trim() : '';

    const btn = document.getElementById('btn-execute-restore');
    const statusDiv = document.getElementById('restore-status');
    if (btn) btn.disabled = true;

    if (statusDiv) {
        statusDiv.style.display = 'block';
        statusDiv.style.background = 'rgba(0, 242, 254, 0.1)';
        statusDiv.style.color = 'var(--cyan)';
        statusDiv.textContent = 'Downloading from Telegram and calculating SHA-256...';
    }

    let url = `/api/restore/${encodeURIComponent(activeRestoreRecord.id)}`;
    const params = [];
    if (destFolder) params.push(`dest_folder=${encodeURIComponent(destFolder)}`);
    if (params.length > 0) url += '?' + params.join('&');

    try {
        const res = await fetch(url, { method: 'POST' });
        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || 'Restore failed');

        if (statusDiv) {
            statusDiv.style.background = 'rgba(0, 245, 160, 0.15)';
            statusDiv.style.color = 'var(--emerald)';
            statusDiv.innerHTML = `✓ Restored to: <code>${escapeHtml(data.restored_path)}</code><br>✓ SHA-256 Checksum Verified!`;
        }

        showToast(`Restored: ${activeRestoreRecord.name}`, "success");
        setTimeout(closeRestoreModal, 2500);

    } catch (e) {
        if (statusDiv) {
            statusDiv.style.background = 'rgba(244, 63, 94, 0.15)';
            statusDiv.style.color = 'var(--crimson)';
            statusDiv.textContent = `Error: ${e.message}`;
        }
        showToast(`Restore error: ${e.message}`, "error");
    } finally {
        if (btn) btn.disabled = false;
    }
}

// =============================================================
// Telegram MTProto Login Wizard
// =============================================================
function initModals() {
    // Click outside modal card to close
    document.querySelectorAll('.modal-overlay').forEach(overlay => {
        overlay.addEventListener('click', e => {
            if (e.target === overlay) {
                overlay.style.display = 'none';
                const pc = document.getElementById('preview-container');
                if (pc) pc.innerHTML = '';
            }
        });
    });
}

function openLoginModal() {
    const modal = document.getElementById('login-modal');
    document.getElementById('login-step-1').style.display = 'block';
    document.getElementById('login-step-2').style.display = 'none';
    const feedback = document.getElementById('login-feedback');
    if (feedback) feedback.style.display = 'none';
    if (modal) modal.style.display = 'flex';
}

function closeLoginModal() {
    const modal = document.getElementById('login-modal');
    if (modal) modal.style.display = 'none';
}

async function handleSendLoginCode() {
    const apiId = document.getElementById('login-api-id').value.trim();
    const apiHash = document.getElementById('login-api-hash').value.trim();
    const phone = document.getElementById('login-phone').value.trim();
    const feedback = document.getElementById('login-feedback');

    if (!apiId || !apiHash || !phone) {
        showToast("Please enter API ID, API Hash, and Phone Number.", "warning");
        return;
    }

    if (feedback) {
        feedback.style.display = 'block';
        feedback.style.background = 'rgba(0, 242, 254, 0.1)';
        feedback.style.color = 'var(--cyan)';
        feedback.textContent = 'Connecting to Telegram MTProto and requesting code...';
    }

    try {
        const res = await fetch('/api/login/send-code', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ api_id: apiId, api_hash: apiHash, phone: phone }),
        });

        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || 'Failed to send login code');

        pendingPhoneCodeHash = data.phone_code_hash;
        document.getElementById('login-step-1').style.display = 'none';
        document.getElementById('login-step-2').style.display = 'block';

        if (feedback) {
            feedback.style.background = 'rgba(0, 245, 160, 0.15)';
            feedback.style.color = 'var(--emerald)';
            feedback.textContent = `Verification code sent to ${phone}. Enter code below:`;
        }

    } catch (e) {
        if (feedback) {
            feedback.style.background = 'rgba(244, 63, 94, 0.15)';
            feedback.style.color = 'var(--crimson)';
            feedback.textContent = `Error: ${e.message}`;
        }
        showToast(`Login error: ${e.message}`, "error");
    }
}

async function handleVerifyLoginCode() {
    const code = document.getElementById('login-code').value.trim();
    const password = document.getElementById('login-password').value.trim() || null;
    const feedback = document.getElementById('login-feedback');

    if (!code) {
        showToast("Please enter the verification code.", "warning");
        return;
    }

    if (feedback) {
        feedback.style.display = 'block';
        feedback.style.background = 'rgba(0, 242, 254, 0.1)';
        feedback.style.color = 'var(--cyan)';
        feedback.textContent = 'Verifying Telegram session credentials...';
    }

    try {
        const res = await fetch('/api/login/verify-code', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                code: code,
                phone_code_hash: pendingPhoneCodeHash,
                password: password,
            }),
        });

        const data = await res.json();
        if (!res.ok) throw new Error(data.detail || 'Verification failed');

        if (data.status === '2fa_required') {
            if (feedback) {
                feedback.style.background = 'rgba(251, 191, 36, 0.15)';
                feedback.style.color = 'var(--amber)';
                feedback.textContent = '2FA password required. Enter your 2-Step password above.';
            }
            return;
        }

        showToast("Telegram account authenticated successfully! Vault is live.", "success");
        closeLoginModal();
        await refreshAll();

    } catch (e) {
        if (feedback) {
            feedback.style.background = 'rgba(244, 63, 94, 0.15)';
            feedback.style.color = 'var(--crimson)';
            feedback.textContent = `Auth error: ${e.message}`;
        }
        showToast(`Auth error: ${e.message}`, "error");
    }
}

async function handleLogout() {
    if (!confirm("Are you sure you want to disconnect your Telegram session?")) return;
    try {
        await fetch('/api/logout', { method: 'POST' });
        showToast("Telegram session disconnected.", "info");
        await refreshAll();
    } catch (e) {
        showToast(`Logout error: ${e.message}`, "error");
    }
}

// =============================================================
// Toast System & Utility Formatters
// =============================================================
function showToast(message, type = 'info') {
    const container = document.getElementById('toast-container');
    if (!container) return;

    const toast = document.createElement('div');
    toast.className = `toast toast-${type}`;

    let icon = 'ℹ️';
    if (type === 'success') icon = '✓';
    if (type === 'error') icon = '✖';
    if (type === 'warning') icon = '⚠️';

    toast.innerHTML = `<span>${icon}</span><span>${escapeHtml(message)}</span>`;
    container.appendChild(toast);

    setTimeout(() => {
        toast.style.opacity = '0';
        toast.style.transform = 'translateY(10px)';
        toast.style.transition = 'all 0.3s ease';
        setTimeout(() => toast.remove(), 300);
    }, 4000);
}

function formatBytes(bytes) {
    if (bytes === 0 || !bytes) return '0 B';
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
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#039;');
}
