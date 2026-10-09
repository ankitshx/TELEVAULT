//! Phase 20: MTProto Authentication, Channel Setup & Centralized Auth Gate Tests.
//!
//! Validates:
//! 1. Centralized auth gate blocks sensitive operations with `UNAUTHORIZED` when not logged in.
//! 2. Multi-step MTProto login flow (phone, code, channel setup).
//! 3. Auth gate unlocks once `Ready` state is achieved.
//! 4. Logout purges session and relocks the auth gate.

use std::fs;
use std::path::PathBuf;
use tauri::Manager;

use televault_desktop::commands::backup::*;
use televault_desktop::commands::repair::*;
use televault_desktop::commands::restore::*;
use televault_desktop::commands::scheduler::*;
use televault_desktop::commands::telegram_auth::*;
use televault_desktop::commands::verification::*;
use televault_desktop::dto::*;
use televault_desktop::state::DesktopAppState;

fn setup_unauthenticated_test_env(name: &str) -> (tauri::App<tauri::test::MockRuntime>, PathBuf) {
    std::env::set_var("TELEVAULT_MOCK_TELEGRAM", "1");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("televault_auth_test_{name}_{nanos}"));
    fs::create_dir_all(&root).unwrap();

    let state = DesktopAppState::new(root.clone(), None)
        .expect("Initialize unauthenticated DesktopAppState");
    let app = tauri::test::mock_app();
    app.manage(state);
    (app, root)
}

#[tokio::test]
async fn test_auth_gate_lifecycle_and_enforcement() {
    let (app, root) = setup_unauthenticated_test_env("lifecycle");
    let state = app.state::<DesktopAppState>();

    // 1. Initial State must be AuthenticationRequired
    let initial_status = get_telegram_auth_status(state.clone())
        .await
        .expect("get initial status");
    assert_eq!(initial_status.state, AuthStateDto::AuthenticationRequired);
    assert!(initial_status.account.is_none());
    assert!(initial_status.channel.is_none());

    // 2. Unauthenticated operations are rejected with UNAUTHORIZED
    let backup_res = start_backup(
        state.clone(),
        StartBackupRequest {
            profile_id: "test-prof".into(),
            passphrase: None,
        },
    )
    .await;
    assert!(backup_res.is_err());
    assert_eq!(backup_res.unwrap_err().code, "UNAUTHORIZED");

    let restore_res = restore_file(
        state.clone(),
        RestoreFileRequest {
            file_id: "test-file".into(),
            destination_path: root.join("restored.bin").to_string_lossy().to_string(),
            collision_policy: CollisionPolicyDto::Overwrite,
            passphrase: None,
        },
    )
    .await;
    assert!(restore_res.is_err());
    assert_eq!(restore_res.unwrap_err().code, "UNAUTHORIZED");

    let verify_res = verify_snapshot(
        state.clone(),
        VerifyTargetRequest {
            profile_id: "test-prof".into(),
            target_id: Some("snap-1".into()),
            level: Some(2),
            full_hash_check: None,
            decrypt_check: None,
            operation_id: None,
        },
    )
    .await;
    assert!(verify_res.is_err());
    assert_eq!(verify_res.unwrap_err().code, "UNAUTHORIZED");

    let repair_res = repair_file(
        state.clone(),
        RepairFileRequest {
            profile_id: "test-prof".into(),
            file_id: "file-1".into(),
            passphrase: None,
            dry_run: Some(true),
            operation_id: None,
        },
    )
    .await;
    assert!(repair_res.is_err());
    assert_eq!(repair_res.unwrap_err().code, "UNAUTHORIZED");

    let sched_res = run_schedule_now(state.clone(), "sched-1".into()).await;
    assert!(sched_res.is_err());
    assert_eq!(sched_res.unwrap_err().code, "UNAUTHORIZED");

    // 3. Initiate MTProto Login Flow
    let start_dto = StartTelegramAuthDto {
        phone_number: "+1 555 123 4567".into(),
        api_id: Some(12345),
        api_hash: Some("mockhash123".into()),
    };
    let step1_status = start_telegram_auth(state.clone(), start_dto)
        .await
        .expect("start_telegram_auth");
    assert_eq!(step1_status.state, AuthStateDto::Authenticating);

    // 4. Submit Mock Verification Code
    let code_dto = SubmitAuthCodeDto {
        code: "12345".into(),
    };
    let step2_status = submit_telegram_auth_code(state.clone(), code_dto)
        .await
        .expect("submit_telegram_auth_code");
    assert_eq!(step2_status.state, AuthStateDto::ChannelSetupRequired);
    assert!(step2_status.account.is_some());
    let account = step2_status.account.unwrap();
    assert_eq!(account.user_id, 987654321);
    assert_eq!(account.first_name, "MockUser");

    // Auth gate should still block because channel is not yet set up
    let premature_backup = start_backup(
        state.clone(),
        StartBackupRequest {
            profile_id: "test-prof".into(),
            passphrase: None,
        },
    )
    .await;
    assert!(premature_backup.is_err());
    assert_eq!(premature_backup.unwrap_err().code, "UNAUTHORIZED");

    // 5. Setup Dedicated Backup Channel
    let channel_dto = SetupChannelDto { channel_id: None };
    let step3_status = setup_backup_channel(state.clone(), channel_dto)
        .await
        .expect("setup_backup_channel");
    assert_eq!(step3_status.state, AuthStateDto::Ready);
    assert!(step3_status.channel.is_some());
    let channel = step3_status.channel.unwrap();
    assert!(channel.verified);
    assert!(channel.is_private);

    // 6. Auth Gate is now UNLOCKED
    assert!(state.check_auth_gate().await.is_ok());

    // 7. Verify Account & Channel Persistence in DB
    let db_acc = state
        .db
        .get_telegram_account(987654321)
        .expect("db account query");
    assert!(db_acc.is_some());
    assert_eq!(db_acc.unwrap().first_name, "MockUser");

    let db_ch = state
        .db
        .get_telegram_channel(channel.channel_id)
        .expect("db channel query");
    assert!(db_ch.is_some());
    assert!(db_ch.unwrap().verified);

    // 8. Logout
    let logout_status = logout_telegram(state.clone())
        .await
        .expect("logout_telegram");
    assert_eq!(logout_status.state, AuthStateDto::AuthenticationRequired);
    assert!(logout_status.account.is_none());
    assert!(logout_status.channel.is_none());

    // 9. Auth Gate is RELOCKED
    assert!(state.check_auth_gate().await.is_err());
    let post_logout_backup = start_backup(
        state.clone(),
        StartBackupRequest {
            profile_id: "test-prof".into(),
            passphrase: None,
        },
    )
    .await;
    assert!(post_logout_backup.is_err());
    assert_eq!(post_logout_backup.unwrap_err().code, "UNAUTHORIZED");

    let _ = fs::remove_dir_all(root);
}
