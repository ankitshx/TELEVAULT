# BUILD RULES

1. No hardcoded developer paths
2. All filesystem access via PathManager
3. Static CRT mandatory (RUSTFLAGS="-C target-feature=+crt-static")
4. WebView2 bundling mandatory (embedBootstrapper)
5. Typed IPC only (tauri-specta)
6. No secrets in logs
7. Every bug = root cause + fix + regression test + prevention rule
8. Local DB is cache; Telegram is source of truth
9. Backup complete only when hash verified
10. Feature done only when test + error handling + logging + docs
