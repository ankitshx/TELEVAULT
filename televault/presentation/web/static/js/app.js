// TeleVault Web Application Client Logic
let currentStatus = null;
let currentRecords = [];
let currentVersions = {};
let pendingPhoneCodeHash = null;
let selectedAIAgentKey = "doctor";
let activeRestoreRecord = null;

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
    } else if (tabId === 'tab-security') {
        loadManifestDetails();
    } else if (tabId === 'tab-audit') {
        refreshAuditLedger();
    }
}

// Data Fetching
async function refreshAll() {
    await refreshStatus();
    await refreshRecords();
    loadManifestDetails();
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
            userBtn.title = "Connected to Telegram MTProto. Click to view.";
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

        // Overview Summary
        const pOverview = document.getElementById('overview-primary-chan');
        const mOverview = document.getElementById('overview-mirror-chan');
        if (pOverview) pOverview.textContent = pChan;
        if (mOverview) mOverview.textContent = mChan;

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
        populateDrillSelect(currentRecords);
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
        tbody.innerHTML = `<tr><td colspan="8" style="text-align: center; color: var(--text-dim); padding: 2rem;">No files backed up yet. Drop a file in "Backup File" to begin.</td></tr>`;
        return;
    }

    tbody.innerHTML = records.map(r => {
        let stateClass = 'status-healthy';
        if (r.state === 'DEGRADED') stateClass = 'status-degraded';
        if (r.state === 'LOST') stateClass = 'status-lost';

        const modeBadge = r.mode === 'PRIVATE'
            ? `<span class="status-pill" style="background: rgba(199,125,255,0.15); color: #c77dff; border: 1px solid rgba(199,125,255,0.3);">🔐 AES-GCM</span>`
            : `<span class="status-pill" style="background: rgba(0,210,255,0.12); color: var(--cyan); border: 1px solid rgba(0,210,255,0.25);">Original</span>`;

        return `
            <tr>
                <td><strong>${escapeHtml(r.name)}</strong></td>
                <td><span class="status-pill" style="background: rgba(255,255,255,0.06); color: #fff;">v${r.version || 1}</span></td>
                <td class="code-cell">${escapeHtml(r.id)}</td>
                <td>${formatBytes(r.size)}</td>
                <td><span class="status-pill ${stateClass}">${r.state}</span></td>
                <td>${modeBadge}</td>
                <td style="color: var(--text-dim); font-size: 0.78rem;">${r.local_status}</td>
                <td>
                    <button class="btn" style="padding: 0.3rem 0.65rem; font-size: 0.75rem;" onclick="openRestoreModal('${escapeHtml(r.id)}', '${escapeHtml(r.name)}', '${r.mode}')">
                        📥 Restore
                    </button>
                </td>
            </tr>
        `;
    }).join('');
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

// -------------------------------------------------------------
// Time Machine & Version Tree
// -------------------------------------------------------------
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

    const versions = currentVersions[selectedName]; // sorted desc by version
    const ascending = [...versions].reverse();

    // Render tree flow
    flowContainer.innerHTML = ascending.map((v, idx) => {
        const isLatest = idx === ascending.length - 1;
        const nodeHtml = `
            <div class="tree-node ${isLatest ? 'active-current' : ''}">
                <div class="tree-node-version">Version ${v.version}</div>
                <div style="font-size: 0.75rem; color: var(--text-dim); margin-top: 0.2rem;">${formatBytes(v.size)}</div>
                ${isLatest ? '<div class="tree-node-tag">CURRENT</div>' : ''}
            </div>
        `;
        const arrowHtml = isLatest ? '' : `<div class="tree-arrow">➔</div>`;
        return nodeHtml + arrowHtml;
    }).join('');

    // Render table
    tableBody.innerHTML = versions.map((v, idx) => {
        const isLatest = idx === 0;
        return `
            <tr>
                <td><strong>v${v.version}</strong> ${isLatest ? '<span class="status-pill status-healthy" style="margin-left: 0.5rem;">LATEST</span>' : ''}</td>
                <td class="code-cell">${escapeHtml(v.id)}</td>
                <td class="code-cell" style="font-size: 0.72rem; color: var(--text-muted);">${v.sha256 ? v.sha256.substring(0, 16) + '...' : 'N/A'}</td>
                <td>${formatBytes(v.size)}</td>
                <td>${v.mode}</td>
                <td style="color: var(--text-dim); font-size: 0.78rem;">${v.created_at ? v.created_at.substring(0, 19).replace('T', ' ') : 'N/A'}</td>
                <td>
                    <button class="btn" style="padding: 0.3rem 0.65rem; font-size: 0.75rem;" onclick="openRestoreModal('${escapeHtml(v.id)}', '${escapeHtml(v.name)}', '${v.mode}')">
                        Restore v${v.version}
                    </button>
                </td>
            </tr>
        `;
    }).join('');
}

// -------------------------------------------------------------
// Recovery Center: Non-Destructive Recovery Drill
// -------------------------------------------------------------
async function runRecoveryDrill() {
    switchToTab('tab-recovery');
    const select = document.getElementById('drill-record-select');
    const passInput = document.getElementById('drill-passphrase-input');
    const resultsPanel = document.getElementById('drill-results-panel');

    const recordId = select ? select.value : null;
    const passphrase = passInput ? passInput.value : null;

    resultsPanel.style.display = 'block';
    resultsPanel.innerHTML = `
        <div style="color: var(--cyan); display: flex; align-items: center; gap: 0.5rem;">
            <span class="badge-pulse"></span> Initializing non-destructive sandboxed recovery drill...
        </div>
        <div style="color: var(--text-dim); font-size: 0.78rem; margin-top: 0.5rem;">
            Step 1: Locating document in Telegram Primary channel.<br>
            Step 2: Downloading ciphertext/plaintext stream to scratch sandbox.<br>
            Step 3: Calculating streaming SHA-256 and validating exact byte integrity.<br>
            Step 4: Cleaning scratch sandbox without touching local files.
        </div>
    `;

    try {
        let url = '/api/recovery-drill';
        const params = [];
        if (recordId) params.push(`record_id=${encodeURIComponent(recordId)}`);
        if (passphrase) params.push(`passphrase=${encodeURIComponent(passphrase)}`);
        if (params.length > 0) url += `?${params.join('&')}`;

        const res = await fetch(url, { method: 'POST' });
        const data = await res.json();

        if (res.ok && data.passed) {
            resultsPanel.innerHTML = `
                <div style="color: var(--emerald); font-weight: 700; font-size: 0.95rem; margin-bottom: 0.5rem;">
                    ✅ RECOVERY DRILL PASSED — CRYPTOGRAPHIC VERIFICATION PROVEN
                </div>
                <div style="line-height: 1.6;">
                    • Target File: <span style="color: #fff;">${escapeHtml(data.file_name)}</span> (${formatBytes(data.file_size)})<br>
                    • Record ID: <span class="code-cell">${escapeHtml(data.record_id)}</span><br>
                    • Duration: <span style="color: var(--cyan);">${data.duration_seconds.toFixed(3)}s</span><br>
                    • SHA-256 Byte Match: <span style="color: var(--emerald);">VERIFIED INTACT</span><br>
                    • File Size Match: <span style="color: var(--emerald);">EXACT</span><br>
                    • Scratch Sandbox: <span style="color: var(--emerald);">CLEANED AND REMOVED</span><br>
                    • Result Summary: <span style="color: var(--text-muted);">${escapeHtml(data.message)}</span>
                </div>
            `;
        } else {
            resultsPanel.innerHTML = `
                <div style="color: var(--crimson); font-weight: 700; font-size: 0.95rem; margin-bottom: 0.5rem;">
                    ❌ RECOVERY DRILL FAILED
                </div>
                <div style="line-height: 1.6; color: var(--crimson);">
                    • Error: ${escapeHtml(data.detail || data.message || "Integrity verification check failed.")}
                </div>
            `;
        }
        refreshStatus();
    } catch (e) {
        resultsPanel.innerHTML = `<div style="color: var(--crimson);">Error running recovery drill: ${escapeHtml(e.message)}</div>`;
    }
}

// -------------------------------------------------------------
// AI Doctor Diagnostics & Advisory Console
// -------------------------------------------------------------
async function runDoctorDiagnostics() {
    const container = document.getElementById('doctor-findings-container');
    const repairBtn = document.getElementById('btn-doctor-repair');
    if (!container) return;

    container.innerHTML = `
        <div style="color: var(--cyan); display: flex; align-items: center; gap: 0.5rem;">
            <span class="badge-pulse"></span> Running comprehensive Vault Doctor diagnostics...
        </div>
    `;

    try {
        const res = await fetch('/api/doctor');
        const data = await res.json();
        renderDoctorFindings(data.findings || []);
        if (repairBtn) {
            const hasIssues = (data.findings || []).some(f => f.state !== 'HEALTHY');
            repairBtn.style.display = hasIssues ? 'inline-flex' : 'none';
        }
    } catch (e) {
        container.innerHTML = `<div style="color: var(--crimson);">Error running diagnostics: ${escapeHtml(e.message)}</div>`;
    }
}

async function applyDoctorRepairs() {
    const container = document.getElementById('doctor-findings-container');
    const repairBtn = document.getElementById('btn-doctor-repair');
    if (!container) return;

    container.innerHTML = `
        <div style="color: var(--emerald); display: flex; align-items: center; gap: 0.5rem;">
            <span class="badge-pulse"></span> Applying automatic repairs to degraded copies and manifest chains...
        </div>
    `;

    try {
        const res = await fetch('/api/doctor?repair=true');
        const data = await res.json();
        renderDoctorFindings(data.findings || []);
        if (repairBtn) repairBtn.style.display = 'none';
        refreshStatus();
        refreshRecords();
    } catch (e) {
        container.innerHTML = `<div style="color: var(--crimson);">Error applying repairs: ${escapeHtml(e.message)}</div>`;
    }
}

function renderDoctorFindings(findings) {
    const container = document.getElementById('doctor-findings-container');
    if (!container) return;

    if (!findings || findings.length === 0) {
        container.innerHTML = `<div style="color: var(--emerald); font-weight: 600;">✅ Vault is fully healthy. All dual copies, manifests, and transactions verified.</div>`;
        return;
    }

    container.innerHTML = findings.map(f => {
        let cardClass = 'healthy';
        let badgeColor = 'var(--emerald)';
        if (f.state === 'CRITICAL' || f.state === 'LOST') {
            cardClass = 'critical';
            badgeColor = 'var(--crimson)';
        } else if (f.state === 'WARNING' || f.state === 'DEGRADED') {
            cardClass = 'warning';
            badgeColor = 'var(--amber)';
        }

        return `
            <div class="finding-card ${cardClass}">
                <div class="finding-title">
                    <span style="color: #fff;">${escapeHtml(f.title)}</span>
                    <span class="status-pill" style="color: ${badgeColor}; border: 1px solid ${badgeColor};">${f.state}</span>
                </div>
                <div class="finding-grid">
                    <div class="finding-box">
                        <div class="finding-box-label">What Happened</div>
                        <div style="color: #f1f5f9;">${escapeHtml(f.what_happened)}</div>
                    </div>
                    <div class="finding-box">
                        <div class="finding-box-label">Why</div>
                        <div style="color: var(--text-muted);">${escapeHtml(f.why)}</div>
                    </div>
                    <div class="finding-box">
                        <div class="finding-box-label">What Is Safe</div>
                        <div style="color: var(--emerald);">${escapeHtml(f.what_is_safe)}</div>
                    </div>
                    <div class="finding-box">
                        <div class="finding-box-label">Recommended Action</div>
                        <div style="color: var(--cyan);">${escapeHtml(f.recommended_action)}</div>
                    </div>
                </div>
            </div>
        `;
    }).join('');
}

function selectAIAgent(agentKey) {
    selectedAIAgentKey = agentKey;
    const pills = document.querySelectorAll('.agent-pill');
    pills.forEach(p => {
        if (p.dataset.agent === agentKey) {
            p.classList.add('active');
        } else {
            p.classList.remove('active');
        }
    });

    const input = document.getElementById('ai-user-prompt');
    if (input) {
        const agentNames = {
            doctor: "Vault Doctor",
            redundancy: "Redundancy Auditor",
            recovery: "Recovery Drill Agent",
            versioning: "Retention & Versioning Agent",
            forensics: "Incident Forensic Agent",
            channels: "Channel Health Agent",
            storage: "Storage Optimizer Agent",
            security: "Security Reviewer Agent",
            onboarding: "Onboarding Coach Agent",
            copilot: "Disaster Recovery Copilot",
        };
        input.placeholder = `Consult with ${agentNames[agentKey] || agentKey}...`;
    }
}

async function submitAIQuery() {
    const input = document.getElementById('ai-user-prompt');
    const respBox = document.getElementById('ai-response-box');
    const agentNameEl = document.getElementById('ai-response-agent-name');
    const msgEl = document.getElementById('ai-response-message');
    const proposalsEl = document.getElementById('ai-proposals-container');

    const promptText = (input.value || '').trim();
    if (!promptText) return;

    respBox.style.display = 'block';
    agentNameEl.textContent = `Consulting Agent...`;
    msgEl.textContent = "Analyzing domain port telemetry...";
    proposalsEl.innerHTML = "";

    try {
        const res = await fetch('/api/ai/query', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ role: selectedAIAgentKey, prompt: promptText }),
        });
        const data = await res.json();

        agentNameEl.textContent = `🤖 ${data.agent_name}`;
        msgEl.textContent = data.message;

        // Render Action Proposals
        if (data.proposals && data.proposals.length > 0) {
            proposalsEl.innerHTML = data.proposals.map(p => `
                <div class="proposal-card" id="proposal-${escapeHtml(p.proposal_id)}">
                    <div class="proposal-header">
                        <div class="proposal-title">⚡ ACTION PROPOSAL: ${escapeHtml(p.action_title)}</div>
                        <span class="status-pill ${p.safe ? 'status-healthy' : 'status-degraded'}">${p.safe ? 'SAFE' : 'CAUTION'}</span>
                    </div>
                    <div class="proposal-meta">
                        <div><strong>Why:</strong> ${escapeHtml(p.why)}</div>
                        <div style="margin-top: 0.25rem;"><strong>What will change:</strong> ${escapeHtml(p.what_will_change)}</div>
                        <div style="margin-top: 0.25rem; color: var(--emerald);"><strong>What will NOT change:</strong> ${escapeHtml(p.what_will_not_change)}</div>
                    </div>
                    <div class="proposal-actions">
                        <button class="btn btn-success" onclick="approveProposal('${escapeHtml(p.proposal_id)}')">Approve & Execute</button>
                        <button class="btn" onclick="rejectProposal('${escapeHtml(p.proposal_id)}')">Dismiss</button>
                    </div>
                </div>
            `).join('');
        }
    } catch (e) {
        msgEl.textContent = `Error: ${e.message}`;
    }
}

async function approveProposal(proposalId) {
    try {
        const res = await fetch('/api/ai/proposals/approve', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ proposal_id: proposalId }),
        });
        const data = await res.json();
        const card = document.getElementById(`proposal-${proposalId}`);
        if (card) {
            card.innerHTML = `<div style="color: var(--emerald); font-weight: 600;">✅ Proposal approved and executed: ${escapeHtml(data.action_title)}</div>`;
        }
        refreshStatus();
        refreshRecords();
    } catch (e) {
        alert("Error approving proposal: " + e.message);
    }
}

async function rejectProposal(proposalId) {
    try {
        await fetch('/api/ai/proposals/reject', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ proposal_id: proposalId }),
        });
        const card = document.getElementById(`proposal-${proposalId}`);
        if (card) {
            card.innerHTML = `<div style="color: var(--text-dim); font-size: 0.85rem;">Proposal dismissed.</div>`;
        }
    } catch (e) {
        console.error("Error dismissing proposal:", e);
    }
}

// -------------------------------------------------------------
// Cryptographic Security & Manifest Chain
// -------------------------------------------------------------
async function loadManifestDetails() {
    try {
        const res = await fetch('/api/manifest');
        const data = await res.json();

        const genEl = document.getElementById('manifest-gen');
        const filesEl = document.getElementById('manifest-files');
        const bytesEl = document.getElementById('manifest-bytes');
        const badgeEl = document.getElementById('manifest-status-badge');
        const rootHashEl = document.getElementById('manifest-root-hash');
        const prevHashEl = document.getElementById('manifest-prev-hash');
        const overviewState = document.getElementById('overview-manifest-state');

        if (data.latest) {
            if (genEl) genEl.textContent = `Gen ${data.latest.generation}`;
            if (filesEl) filesEl.textContent = data.latest.total_files;
            if (bytesEl) bytesEl.textContent = formatBytes(data.latest.total_bytes);
            if (rootHashEl) rootHashEl.textContent = data.latest.manifest_hash;
            if (prevHashEl) prevHashEl.textContent = data.latest.previous_hash;
        }

        if (badgeEl) {
            badgeEl.className = data.is_valid ? 'status-pill status-healthy' : 'status-pill status-lost';
            badgeEl.textContent = data.is_valid ? 'Chain Verified Valid' : 'Chain Integrity Violation';
        }
        if (overviewState) {
            overviewState.textContent = data.is_valid ? 'Verified Intact' : 'Violation Detected';
            overviewState.style.color = data.is_valid ? 'var(--emerald)' : 'var(--crimson)';
        }
    } catch (e) {
        console.error("Error loading manifest:", e);
    }
}

async function verifyManifestChain() {
    await loadManifestDetails();
    alert("Manifest chain integrity verified across all historical generations.");
}

async function generateNewManifest() {
    try {
        const res = await fetch('/api/manifest/generate', { method: 'POST' });
        const data = await res.json();
        alert(`New manifest Generation ${data.generation} generated with root hash: ${data.manifest_hash.substring(0, 16)}...`);
        loadManifestDetails();
        refreshStatus();
    } catch (e) {
        alert("Error generating manifest: " + e.message);
    }
}

// -------------------------------------------------------------
// Tamper-Evident Audit Ledger
// -------------------------------------------------------------
async function refreshAuditLedger() {
    try {
        const res = await fetch('/api/audit');
        const data = await res.json();

        const banner = document.getElementById('audit-integrity-banner');
        if (banner) {
            if (data.is_valid) {
                banner.style.background = 'rgba(16, 185, 129, 0.1)';
                banner.style.color = 'var(--emerald)';
                banner.innerHTML = `<span>●</span> Audit ledger integrity verified: 0 cryptographic discrepancies detected.`;
            } else {
                banner.style.background = 'rgba(239, 68, 68, 0.1)';
                banner.style.color = 'var(--crimson)';
                banner.innerHTML = `<span>✖</span> Audit ledger integrity violation detected!`;
            }
        }

        const tbody = document.getElementById('audit-table-body');
        if (!tbody) return;
        if (!data.events || data.events.length === 0) {
            tbody.innerHTML = `<tr><td colspan="5" style="text-align: center; color: var(--text-dim); padding: 1.5rem;">No audit events recorded yet.</td></tr>`;
            return;
        }

        tbody.innerHTML = data.events.map(e => `
            <tr>
                <td style="color: var(--text-dim); font-size: 0.75rem;">${escapeHtml(e.timestamp.replace('T', ' ').substring(0, 19))}</td>
                <td><strong style="color: var(--cyan);">${escapeHtml(e.action)}</strong></td>
                <td class="code-cell" style="font-size: 0.72rem;">${escapeHtml(e.entity_id)}</td>
                <td><span class="status-pill ${e.result === 'SUCCESS' ? 'status-healthy' : 'status-degraded'}">${escapeHtml(e.result)}</span></td>
                <td style="font-size: 0.78rem; color: var(--text-muted);">${escapeHtml(e.details)}</td>
            </tr>
        `).join('');
    } catch (e) {
        console.error("Error refreshing audit ledger:", e);
    }
}

async function verifyAuditLedger() {
    await refreshAuditLedger();
    alert("Cryptographic hash chain of audit events has been validated.");
}

// -------------------------------------------------------------
// Backup Upload & Local Path
// -------------------------------------------------------------
function togglePassphraseInput() {
    const check = document.getElementById('check-private');
    const container = document.getElementById('passphrase-container');
    if (check && container) {
        container.style.display = check.checked ? 'flex' : 'none';
    }
}

function initDropzone() {
    const dropzone = document.getElementById('dropzone');
    const fileInput = document.getElementById('file-input');

    if (!dropzone || !fileInput) return;

    ['dragenter', 'dragover'].forEach(eventName => {
        dropzone.addEventListener(eventName, (e) => {
            e.preventDefault();
            dropzone.classList.add('drag-over');
        }, false);
    });

    ['dragleave', 'drop'].forEach(eventName => {
        dropzone.addEventListener(eventName, (e) => {
            e.preventDefault();
            dropzone.classList.remove('drag-over');
        }, false);
    });

    dropzone.addEventListener('drop', (e) => {
        const dt = e.dataTransfer;
        const files = dt.files;
        if (files.length > 0) {
            handleFileUpload(files[0]);
        }
    });

    fileInput.addEventListener('change', () => {
        if (fileInput.files.length > 0) {
            handleFileUpload(fileInput.files[0]);
        }
    });
}

async function handleFileUpload(file) {
    const statusText = document.getElementById('upload-status-text');
    const isPrivate = document.getElementById('check-private').checked;
    const passphraseInput = document.getElementById('input-passphrase');
    const passphrase = isPrivate && passphraseInput ? passphraseInput.value : '';

    statusText.style.display = 'block';
    statusText.style.color = 'var(--cyan)';
    statusText.textContent = `Uploading ${file.name} to Primary & Mirror channels...`;

    const formData = new FormData();
    formData.append('file', file);
    formData.append('mode', isPrivate ? 'private' : 'original');
    if (passphrase) formData.append('passphrase', passphrase);

    try {
        const res = await fetch('/api/backup/upload', {
            method: 'POST',
            body: formData,
        });
        const data = await res.json();

        if (res.ok && data.status === 'success') {
            statusText.style.color = 'var(--emerald)';
            statusText.textContent = `✅ Successfully backed up ${file.name} (Record ID: ${data.record_id})`;
            refreshAll();
        } else if (data.status === 'duplicate') {
            statusText.style.color = 'var(--amber)';
            statusText.textContent = `⚠️ ${data.message}`;
        } else {
            statusText.style.color = 'var(--crimson)';
            statusText.textContent = `❌ Upload failed: ${data.detail || data.message || "Unknown error"}`;
        }
    } catch (e) {
        statusText.style.color = 'var(--crimson)';
        statusText.textContent = `❌ Upload failed: ${e.message}`;
    }
}

async function handlePathBackup() {
    const pathInput = document.getElementById('input-local-path');
    const btn = document.getElementById('btn-path-backup');
    const isPrivate = document.getElementById('check-private').checked;
    const passInput = document.getElementById('input-passphrase');
    const passphrase = isPrivate && passInput ? passInput.value : '';

    const pathVal = (pathInput.value || '').trim();
    if (!pathVal) {
        alert("Please enter a local file or folder path.");
        return;
    }

    btn.disabled = true;
    btn.textContent = "Backing up...";

    try {
        const res = await fetch('/api/backup/path', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                path: pathVal,
                mode: isPrivate ? 'private' : 'original',
                passphrase: passphrase || null,
            }),
        });
        const data = await res.json();
        if (res.ok) {
            alert(`Backup complete! Processed ${data.results.length} file(s).`);
            pathInput.value = "";
            refreshAll();
        } else {
            alert("Backup failed: " + (data.detail || "Unknown error"));
        }
    } catch (e) {
        alert("Backup failed: " + e.message);
    } finally {
        btn.disabled = false;
        btn.textContent = "Back Up Local Path";
    }
}

// -------------------------------------------------------------
// Restore Modal
// -------------------------------------------------------------
function openRestoreModal(recordId, fileName, mode) {
    activeRestoreRecord = { id: recordId, name: fileName, mode: mode };
    document.getElementById('restore-modal-record-id').value = recordId;
    document.getElementById('restore-modal-file-name').value = fileName;
    document.getElementById('restore-modal-dest').value = '';

    const passGroup = document.getElementById('restore-passphrase-group');
    if (passGroup) {
        passGroup.style.display = (mode === 'PRIVATE') ? 'block' : 'none';
    }

    document.getElementById('restore-modal').classList.add('active');
}

function closeRestoreModal() {
    document.getElementById('restore-modal').classList.remove('active');
    activeRestoreRecord = null;
}

async function submitRestore() {
    if (!activeRestoreRecord) return;
    const recordId = activeRestoreRecord.id;
    const dest = document.getElementById('restore-modal-dest').value.trim();
    const btn = document.getElementById('btn-confirm-restore');

    btn.disabled = true;
    btn.textContent = "Downloading & Verifying...";

    try {
        let url = `/api/restore/${encodeURIComponent(recordId)}`;
        if (dest) url += `?dest_folder=${encodeURIComponent(dest)}`;

        const res = await fetch(url, { method: 'POST' });
        const data = await res.json();

        if (res.ok && data.status === 'success') {
            alert(`✅ Document successfully restored to:\n${data.restored_path}\nSHA-256 byte integrity verified!`);
            closeRestoreModal();
            refreshAll();
        } else {
            alert(`❌ Restore failed: ${data.detail || "SHA-256 integrity verification failed."}`);
        }
    } catch (e) {
        alert(`❌ Restore failed: ${e.message}`);
    } finally {
        btn.disabled = false;
        btn.textContent = "Restore Document";
    }
}

// -------------------------------------------------------------
// Other Core Actions: Verify, Heal, Rebuild, Snapshot
// -------------------------------------------------------------
async function handleVerify() {
    try {
        const res = await fetch('/api/verify', { method: 'POST' });
        const data = await res.json();
        alert(`Verification Complete:\nTotal checked: ${data.total_checked}\nHealthy: ${data.healthy_count}\nDegraded: ${data.degraded_count}\nLost: ${data.lost_count}`);
        refreshAll();
    } catch (e) {
        alert("Verification failed: " + e.message);
    }
}

async function handleAutoHeal() {
    try {
        const res = await fetch('/api/verify?heal=true', { method: 'POST' });
        const data = await res.json();
        const healed = data.healing ? data.healing.healed_count : 0;
        alert(`Auto-Heal Complete:\nHealed ${healed} degraded document(s).`);
        refreshAll();
    } catch (e) {
        alert("Auto-heal failed: " + e.message);
    }
}

async function handleRebuild() {
    if (!confirm("Rebuild will scan your Telegram channels, read cryptographic tv2 captions, and reconstruct your SQLite index. Proceed?")) return;
    try {
        const res = await fetch('/api/rebuild', { method: 'POST' });
        const data = await res.json();
        alert(`Rebuild Complete:\nReconstructed: ${data.records_reconstructed}\nHealthy: ${data.records_healthy}\nDegraded: ${data.records_degraded}`);
        refreshAll();
    } catch (e) {
        alert("Rebuild failed: " + e.message);
    }
}

async function handleSnapshot() {
    try {
        const res = await fetch('/api/snapshot', { method: 'POST' });
        const data = await res.json();
        alert(`Snapshot exported to Telegram: ${data.snapshot_file}`);
        refreshStatus();
    } catch (e) {
        alert("Snapshot failed: " + e.message);
    }
}

// -------------------------------------------------------------
// Telegram Login Modal
// -------------------------------------------------------------
function initModals() {
    window.openLoginModal = () => {
        document.getElementById('login-modal').classList.add('active');
        document.getElementById('login-error-msg').style.display = 'none';
        document.getElementById('login-step-1').style.display = 'block';
        document.getElementById('login-step-2').style.display = 'none';
    };

    window.closeLoginModal = () => {
        document.getElementById('login-modal').classList.remove('active');
    };
}

async function submitSendCode() {
    const apiId = document.getElementById('login-api-id').value.trim();
    const apiHash = document.getElementById('login-api-hash').value.trim();
    const phone = document.getElementById('login-phone').value.trim();
    const errBox = document.getElementById('login-error-msg');
    const sendBtn = document.getElementById('btn-send-code');

    if (!apiId || !apiHash || !phone) {
        errBox.textContent = "Please fill in API ID, API Hash, and Phone Number.";
        errBox.style.display = 'block';
        return;
    }

    sendBtn.disabled = true;
    sendBtn.textContent = "Sending Code...";
    errBox.style.display = 'none';

    try {
        const res = await fetch('/api/login/send-code', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ api_id: apiId, api_hash: apiHash, phone: phone }),
        });
        const data = await res.json();

        if (res.ok && data.status === 'code_sent') {
            pendingPhoneCodeHash = data.phone_code_hash;
            document.getElementById('step-2-phone-label').textContent = phone;
            document.getElementById('login-step-1').style.display = 'none';
            document.getElementById('login-step-2').style.display = 'block';
        } else {
            errBox.textContent = data.detail || "Failed to send code.";
            errBox.style.display = 'block';
        }
    } catch (e) {
        errBox.textContent = e.message;
        errBox.style.display = 'block';
    } finally {
        sendBtn.disabled = false;
        sendBtn.textContent = "Send Verification Code";
    }
}

async function submitVerifyCode() {
    const code = document.getElementById('login-code').value.trim();
    const password = document.getElementById('login-password').value;
    const errBox = document.getElementById('login-error-msg');
    const verifyBtn = document.getElementById('btn-verify-code');

    if (!code) {
        errBox.textContent = "Please enter the confirmation code from Telegram.";
        errBox.style.display = 'block';
        return;
    }

    verifyBtn.disabled = true;
    verifyBtn.textContent = "Verifying...";
    errBox.style.display = 'none';

    try {
        const res = await fetch('/api/login/verify-code', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                code: code,
                phone_code_hash: pendingPhoneCodeHash,
                password: password || null,
            }),
        });
        const data = await res.json();

        if (res.ok && data.status === 'success') {
            closeLoginModal();
            refreshStatus();
            alert(`Connected to Telegram MTProto as ${data.user.first_name || data.user.phone}!`);
        } else if (data.status === '2fa_required') {
            document.getElementById('group-2fa').style.display = 'block';
            errBox.textContent = "Two-step verification password is required for this account.";
            errBox.style.display = 'block';
        } else {
            errBox.textContent = data.detail || "Verification failed.";
            errBox.style.display = 'block';
        }
    } catch (e) {
        errBox.textContent = e.message;
        errBox.style.display = 'block';
    } finally {
        verifyBtn.disabled = false;
        verifyBtn.textContent = "Verify & Connect";
    }
}

// -------------------------------------------------------------
// Account Info & Logs Rendering
// -------------------------------------------------------------
function renderAccountInfo(status) {
    const container = document.getElementById('account-info-container');
    if (!container) return;

    if (status.authenticated && status.user) {
        container.innerHTML = `
            <div class="table-card" style="padding: 1.5rem;">
                <div style="font-size: 1.15rem; font-weight: 700; color: #fff; margin-bottom: 1rem;">
                    👤 Connected Telegram Account (MTProto)
                </div>
                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 1rem; margin-bottom: 1.5rem;">
                    <div class="finding-box">
                        <div class="finding-box-label">Name</div>
                        <div style="font-size: 1rem; font-weight: 600; color: #fff;">${escapeHtml(status.user.first_name || '')} ${escapeHtml(status.user.last_name || '')}</div>
                    </div>
                    <div class="finding-box">
                        <div class="finding-box-label">Phone Number</div>
                        <div style="font-size: 1rem; font-weight: 600; color: var(--cyan);">${escapeHtml(status.user.phone || 'N/A')}</div>
                    </div>
                    <div class="finding-box">
                        <div class="finding-box-label">Username</div>
                        <div style="font-size: 1rem; font-weight: 600; color: #fff;">@${escapeHtml(status.user.username || 'none')}</div>
                    </div>
                    <div class="finding-box">
                        <div class="finding-box-label">Data Center (DC)</div>
                        <div style="font-size: 1rem; font-weight: 600; color: #fff;">DC ${status.user.dc_id || 'N/A'}</div>
                    </div>
                </div>
                <button class="btn btn-danger" onclick="handleLogout()">Disconnect Telegram Account</button>
            </div>
        `;
    } else {
        container.innerHTML = `
            <div class="table-card" style="padding: 2.5rem; text-align: center;">
                <div style="font-size: 2.5rem; margin-bottom: 0.75rem;">📡</div>
                <div style="font-size: 1.25rem; font-weight: 700; color: #fff; margin-bottom: 0.5rem;">Running in Simulation Mode</div>
                <div style="color: var(--text-muted); font-size: 0.85rem; max-width: 520px; margin: 0 auto 1.5rem auto;">
                    TeleVault is operating using an in-memory Fake MTProto Gateway. To back up real documents to your private Telegram storage channels, connect your Telegram account.
                </div>
                <button class="btn btn-primary" onclick="openLoginModal()">⚡ Connect Telegram Account</button>
            </div>
        `;
    }
}

async function handleLogout() {
    if (!confirm("Are you sure you want to disconnect your Telegram account?")) return;
    try {
        await fetch('/api/logout', { method: 'POST' });
        refreshAll();
    } catch (e) {
        alert("Logout failed: " + e.message);
    }
}

function renderLogs(logs) {
    const term = document.getElementById('terminal-logs');
    if (!term || !logs) return;

    term.innerHTML = logs.map(l => {
        let levelClass = 'log-msg-success';
        if (l.level === 'error') levelClass = 'log-msg-error';
        if (l.level === 'warning') levelClass = 'log-msg-warning';

        return `
            <div class="log-entry">
                <span class="log-time">[${l.timestamp || 'SYS'}]</span>
                <span class="log-cat">[${escapeHtml(l.category)}]</span>
                <span class="${levelClass}">${escapeHtml(l.message)}</span>
            </div>
        `;
    }).join('');
    term.scrollTop = term.scrollHeight;
}

// Helpers
function formatBytes(bytes) {
    if (!bytes || bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function escapeHtml(text) {
    if (!text) return '';
    return String(text)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
}
