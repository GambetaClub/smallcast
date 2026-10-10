mod actions;
mod agents;
mod analytics;
mod apps;
mod backup;
mod clipboard;
mod commands;
mod docker_ps;
mod files;
mod git;
mod github;
mod hotkey;
mod icons;
#[cfg(target_os = "macos")]
mod macos_perf;
mod proc;
mod recommend;
mod skills;
mod stats;
mod store;
mod system;
mod themes;
mod types;
mod vite_ports;

use parking_lot::RwLock;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Held for the whole process; the OS releases it on exit or crash.
    let _single_instance = instance_lock();
    if matches!(
        _single_instance.as_ref().map(|f| f.try_lock()),
        Some(Err(std::fs::TryLockError::WouldBlock))
    ) {
        eprintln!("smallcast is already running; quit it first");
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_drag::init())
        .setup(|app| {
            // On macOS, run as an accessory app — no dock icon, menu bar only.
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                // Opt out of App Nap: keeps the WebView warm + responsive
                // so the first ⌃Space after long idle doesn't pay a
                // wake-up tax (CPU clock-up + page faults). Tiny constant
                // energy hit; the app is otherwise idle most of the time.
                macos_perf::opt_out_of_app_nap();
                // Suppress the slide-in animation on the palette window.
                // Default Cocoa window-show takes ~250ms — the dominant
                // remaining cost in our paint_ms metric. With it off the
                // window appears instantly.
                if let Some(w) = app.get_webview_window("main") {
                    if let Ok(ns_window) = w.ns_window() {
                        unsafe {
                            macos_perf::disable_window_animation(ns_window);
                            // Convert the window to an NSPanel with the
                            // NonactivatingPanel style. Must come before
                            // make_visible_over_fullscreen — the level
                            // + collection-behavior story only actually
                            // floats over fullscreen Spaces when the
                            // backing window is an NSPanel.
                            macos_perf::make_panel(ns_window);
                            macos_perf::make_visible_over_fullscreen(ns_window);
                        }
                    }
                }
            }

            // Load the store.
            let store = store::Store::load().expect("failed to load smallcast store");
            app.manage(RwLock::new(store));

            // Register global hotkeys: palette + clipboard.
            let handle = app.handle().clone();
            let palette_sc = hotkey::default_shortcut();
            let clipboard_sc = hotkey::clipboard_shortcut();
            app.global_shortcut().on_shortcut(palette_sc, {
                let handle = handle.clone();
                move |_app, sc, event| {
                    hotkey::on_palette_shortcut(&handle, sc, event.state);
                }
            })?;
            app.global_shortcut().on_shortcut(clipboard_sc, {
                let handle = handle.clone();
                move |_app, sc, event| {
                    hotkey::on_clipboard_shortcut(&handle, sc, event.state);
                }
            })?;

            // Background clipboard watcher.
            clipboard::start_watcher(app.handle().clone());

            // Warm the /Applications scan cache off the open critical path.
            // `apps::list_apps` was the dominant cost in `list_palette`;
            // this thread runs the scan now and every 60s thereafter so
            // every palette open hits an in-memory cache.
            apps::start_background_refresher();

            // Drain analytics writes on a dedicated thread so each
            // `analytics_record` Tauri command returns instantly (push to
            // queue, no file I/O on the call path). Per-keystroke logging
            // never blocks the UI even if the disk is contended.
            analytics::start_writer_thread();

            // Background backup auto-sync.
            //
            // Wakes every 60s and pushes when:
            //   - backup is enabled AND initialized AND has a remote
            //   - more than `auto_interval_min` minutes since the last
            //     successful sync
            //   - the working tree has uncommitted changes
            //
            // Records last_synced_ms / last_error back into the store so
            // the Preferences UI can render fresh status without polling.
            let backup_handle = app.handle().clone();
            std::thread::spawn(move || {
                use std::time::Duration;
                loop {
                    std::thread::sleep(Duration::from_secs(60));
                    let store = match backup_handle.try_state::<RwLock<store::Store>>() {
                        Some(s) => s,
                        None => continue,
                    };
                    let cfg = {
                        let s = store.read();
                        s.config.backup.clone()
                    };
                    if !cfg.enabled || cfg.remote.trim().is_empty() {
                        continue;
                    }
                    if !backup::is_initialized() {
                        continue;
                    }
                    // Clamp to ≥1 min: a synced/edited config with 0 would make
                    // the dirty-tree guard always true and push every 60s.
                    let interval_ms = (cfg.auto_interval_min.max(1) as u64) * 60 * 1000;
                    let elapsed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .and_then(|d| {
                            cfg.last_synced_ms
                                .map(|t| (d.as_millis() as u64).saturating_sub(t))
                        });
                    if let Some(ms) = elapsed {
                        if ms < interval_ms {
                            continue;
                        }
                    }
                    let s_now = backup::status(
                        &cfg.branch,
                        cfg.last_synced_ms,
                        cfg.last_error.clone(),
                    );
                    if s_now.dirty_count == 0 {
                        continue;
                    }
                    let result = backup::sync(&cfg.branch, cfg.include_analytics);
                    let mut s = store.write();
                    match result {
                        Ok(()) => {
                            s.config.backup.last_synced_ms = Some(
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as u64)
                                    .unwrap_or(0),
                            );
                            s.config.backup.last_error = None;
                        }
                        Err(e) => {
                            s.config.backup.last_error = Some(e);
                        }
                    }
                    let _ = s.save_config();
                }
            });

            // Background recommender retrain.
            //
            // Wakes every 5 minutes. When the user has the recommender
            // enabled and there are at least RETRAIN_MIN_NEW_EVENTS new
            // execute events since the last training pass, we replay
            // the entire analytics.jsonl. Training is fast (well under
            // a second for tens of thousands of events) and runs off
            // the UI thread, so the user never sees a hitch.
            //
            // We deliberately retrain from scratch instead of doing
            // online updates: replaying the log is cheap, and a full
            // pass guarantees that aged-out events (deleted from the
            // log via "Clear Analytics") stop influencing the model.
            let recommend_handle = app.handle().clone();
            std::thread::spawn(move || {
                use std::time::Duration;
                const RETRAIN_TICK_SECS: u64 = 5 * 60;
                const RETRAIN_MIN_NEW_EVENTS: usize = 30;
                loop {
                    std::thread::sleep(Duration::from_secs(RETRAIN_TICK_SECS));
                    let store = match recommend_handle
                        .try_state::<RwLock<store::Store>>()
                    {
                        Some(s) => s,
                        None => continue,
                    };
                    let enabled = store.read().config.enable_recommendations;
                    if !enabled {
                        continue;
                    }
                    let prev = recommend::load_state();
                    let Some(log_path) = analytics::log_path() else {
                        continue;
                    };
                    let Ok(content) = std::fs::read_to_string(&log_path) else {
                        continue;
                    };
                    // Cheap line count is good enough — execute events
                    // are a strict subset, so this is an upper bound on
                    // "events the model hasn't seen."
                    let lines = content.lines().count();
                    let already = prev.train_examples;
                    if lines < already + RETRAIN_MIN_NEW_EVENTS {
                        continue;
                    }
                    if let Ok(next) = recommend::train_from_jsonl(&content) {
                        let _ = recommend::save_state(&next);
                    }
                }
            });

            // Build the menu-bar tray.
            let show_i = MenuItem::with_id(app, "show", "Open smallcast", true, Some("Ctrl+Space"))?;
            let prefs_i = MenuItem::with_id(app, "prefs", "Preferences…", true, Some("Cmd+,"))?;
            let sep_i = tauri::menu::PredefinedMenuItem::separator(app)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit smallcast", true, Some("Cmd+Q"))?;
            let menu = Menu::with_items(app, &[&show_i, &prefs_i, &sep_i, &quit_i])?;

            let tray_icon = tauri::image::Image::from_bytes(include_bytes!(
                "../icons/tray.png"
            ))?;
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .menu(&menu)
                // Left-click opens the palette; the menu surfaces on right-click.
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        hotkey::toggle_palette(tray.app_handle());
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => hotkey::toggle_palette(app),
                    "prefs" => {
                        if let Err(e) = commands::show_preferences(app.clone()) {
                            eprintln!("failed to open preferences: {e}");
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::Focused(false) = event {
                    // If the user just pressed the hotkey, the blur is a
                    // side-effect of the press — let toggle_palette decide
                    // whether to hide. Otherwise (clicked elsewhere etc.),
                    // auto-hide as normal.
                    if hotkey::blur_within_hotkey_grace() {
                        return;
                    }
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::set_show_vite_inline,
            commands::set_show_docker_inline,
            commands::set_show_snippets_inline,
            commands::set_show_quicklinks_inline,
            commands::set_screenshot_dirs,
            commands::set_check_updates_on_launch,
            commands::set_github_repos,
            commands::github_list_prs,
            commands::github_list_issues,
            commands::github_list_assigned,
            commands::open_url,
            commands::get_backup_settings,
            commands::set_backup_remote,
            commands::set_backup_branch,
            commands::set_backup_enabled,
            commands::set_backup_include_analytics,
            commands::backup_status,
            commands::backup_init,
            commands::backup_sync,
            commands::backup_pull,
            commands::backup_force_push,
            commands::backup_git_dir,
            commands::search_screenshots,
            commands::analytics_record,
            commands::analytics_tail,
            commands::analytics_summary,
            commands::analytics_clear,
            commands::analytics_log_path,
            commands::set_enable_recommendations,
            commands::recommend_train,
            commands::recommend_status,
            commands::recommend_score,
            commands::recommend_clear,
            commands::search_files,
            commands::open_file,
            commands::reveal_file,
            commands::copy_file_path,
            commands::copy_file_image,
            commands::file_thumbnail,
            commands::list_skills,
            commands::read_skill,
            commands::system_stats,
            commands::system_lock_screen,
            commands::system_sleep,
            commands::system_empty_trash,
            commands::system_restart,
            commands::system_shut_down,
            commands::system_log_out,
            commands::get_active_theme,
            commands::list_items,
            commands::list_palette,
            commands::list_agents,
            commands::list_vite_ports,
            commands::list_docker_containers,
            commands::execute_app,
            commands::execute_agent,
            commands::execute_vite,
            commands::execute_docker_shell,
            commands::execute_docker_logs,
            commands::get_app_icon,
            commands::list_clipboard,
            commands::execute_clipboard,
            commands::delete_clipboard,
            commands::clear_clipboard,
            commands::create_snippet,
            commands::update_snippet,
            commands::delete_snippet,
            commands::create_quicklink,
            commands::update_quicklink,
            commands::delete_quicklink,
            commands::execute_snippet,
            commands::execute_quicklink,
            commands::hide_palette,
            commands::hide_and_paste,
            commands::show_preferences,
            commands::import_from_file,
            commands::detect_raycast,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Two copies both answer the hotkey and steal focus from each other.
fn instance_lock() -> Option<std::fs::File> {
    let dir = dirs::data_dir()?.join("smallcast");
    std::fs::create_dir_all(&dir).ok()?;
    std::fs::File::create(dir.join("instance.lock")).ok()
}
