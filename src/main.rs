mod cli;
mod core;
mod db;
mod proxy;
mod utils;

#[cfg(feature = "web-server")]
mod service;

use clap::Parser;
use cli::Commands;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = cli::Cli::parse();

    match cli.command {
        Commands::Web { port, host } => {
            #[cfg(feature = "web-server")]
            {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async {
                    service::start_web_server(&host, port).await
                })?;
            }
            #[cfg(not(feature = "web-server"))]
            {
                log::error!("Web server feature is not enabled. Rebuild with --features web-server");
                std::process::exit(1);
            }
        }

        Commands::Start {
            mode,
            subscription,
            proxy_ids,
            config_dir,
            web_server,
            port,
            daemon,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let proxy_mode = proxy::ProxyMode::from(mode.clone());

                if !proxy_ids.is_empty() {
                    set_active_proxies(&proxy_ids)?;
                }

                log::info!("Generating config for mode: {:?}", mode);
                let tun = mode.is_tun();
                let config_path = core::generate_and_write_config(
                    &mode.to_mode_str(),
                    tun,
                    subscription.as_deref(),
                    config_dir.as_deref(),
                )?;
                log::info!("Config written to: {}", config_path);

                proxy::manager::transition(proxy::manager::Intent::Start {
                    mode: mode.to_mode_str().to_string(),
                })?;

                log::info!("Starting sing-box...");
                proxy::process::start_singbox(&config_path, proxy_mode)?;

                log::info!("Waiting for engine to be ready...");
                if proxy::readiness::wait_ready(std::time::Duration::from_secs(20)).await {
                    proxy::manager::transition(proxy::manager::Intent::MarkRunning)?;
                    log::info!("Engine is running!");

                    // Save state for restart
                    save_start_state(&mode, &subscription, &proxy_ids);

                    if daemon {
                        #[cfg(unix)]
                        {
                            let pid = unsafe { libc::fork() };
                            if pid < 0 {
                                anyhow::bail!("fork failed");
                            }
                            if pid > 0 {
                                // Parent: exit immediately
                                let child_pid = proxy::process::get_pid().unwrap_or(0);
                                println!("Daemon started (sing-box PID: {})", child_pid);
                                std::process::exit(0);
                            }
                            // Child: continue monitoring
                            log::info!("Daemonized (PID: {})", std::process::id());
                        }
                        #[cfg(not(unix))]
                        {
                            log::warn!("--daemon not supported on this platform, running in foreground");
                        }
                    }

                    if web_server {
                        #[cfg(feature = "web-server")]
                        {
                            log::info!("Starting web server on port {}...", port);
                            let host = "127.0.0.1".to_string();
                            service::start_web_server(&host, port).await?;
                        }
                        #[cfg(not(feature = "web-server"))]
                        {
                            log::error!("Web server feature not enabled");
                        }
                    } else {
                        proxy::monitor::wait_for_exit().await;
                    }
                } else {
                    log::error!("Engine failed to become ready within timeout");
                    proxy::manager::transition(proxy::manager::Intent::Fail {
                        reason: "Readiness timeout".to_string(),
                    })?;
                }
                Ok::<_, anyhow::Error>(())
            })?;
        }

        Commands::Stop => {
            log::info!("Stopping sing-box...");
            proxy::process::stop_singbox()?;
            proxy::manager::transition(proxy::manager::Intent::MarkIdle)?;
            log::info!("Engine stopped");
        }

        Commands::Restart => {
            if let Some(state) = load_start_state() {
                log::info!("Restarting with mode={:?} proxy={:?}", state.mode, state.proxy_ids);
                // Stop first
                let _ = proxy::process::stop_singbox();
                let _ = proxy::manager::transition(proxy::manager::Intent::MarkIdle);
                std::thread::sleep(std::time::Duration::from_millis(500));

                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async {
                    let proxy_mode = proxy::ProxyMode::from(state.mode.clone());
                    if !state.proxy_ids.is_empty() {
                        set_active_proxies(&state.proxy_ids)?;
                    }
                    let tun = state.mode.is_tun();
                    let mode_str = state.mode.to_mode_str().to_string();
                    let sub = state.subscription.as_deref();
                    let config_path = core::generate_and_write_config(&mode_str, tun, sub, None)?;
                    proxy::manager::transition(proxy::manager::Intent::Start { mode: mode_str })?;
                    proxy::process::start_singbox(&config_path, proxy_mode)?;
                    if proxy::readiness::wait_ready(std::time::Duration::from_secs(20)).await {
                        proxy::manager::transition(proxy::manager::Intent::MarkRunning)?;
                        log::info!("Engine restarted");
                    } else {
                        log::error!("Engine failed to become ready");
                    }
                    Ok::<_, anyhow::Error>(())
                })?;
            } else {
                log::error!("No previous state found. Use `start` first.");
            }
        }

        Commands::Reload => {
            log::info!("Reloading sing-box configuration...");
            proxy::process::reload_singbox()?;
            log::info!("Config reloaded");
        }

        Commands::Status => {
            let state = proxy::manager::snapshot();
            println!("{}", serde_json::to_string_pretty(&state)?);
        }

        Commands::Add { source } => {
            handle_add(source)?;
        }

        Commands::Import { links, file } => {
            handle_import(&links, file.as_deref())?;
        }

        Commands::List { target } => {
            handle_list(target)?;
        }

        Commands::Remove { target } => {
            handle_remove(target)?;
        }

        Commands::Generate {
            mode,
            subscription,
            output,
        } => {
            let config = core::generate_config(
                &mode.to_mode_str(),
                false,
                subscription.as_deref(),
            )?;
            let json = serde_json::to_string_pretty(&config)?;

            if let Some(path) = output {
                std::fs::write(&path, json)?;
                log::info!("Config written to: {}", path);
            } else {
                println!("{}", json);
            }
        }

        Commands::Install { version, dir } => {
            let target_dir = dir.unwrap_or_else(|| {
                let home = dirs_fallback();
                format!("{}/.local/share/aurorabox/bin", home)
            });
            let ver = version.unwrap_or_else(|| "1.13.13".to_string());
            log::info!("Downloading sing-box v{} to {}", ver, target_dir);
            utils::sing_box::download_singbox(&ver, &target_dir)?;
            log::info!("sing-box installed successfully!");
        }
    }

    Ok(())
}

fn dirs_fallback() -> String {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string())
}

fn handle_add(source: cli::AddSource) -> anyhow::Result<()> {
    let db_path = db::db_path();
    let conn = db::open(&db_path)?;

    match source {
        cli::AddSource::Subscription { url, name } => {
            let name = name.unwrap_or_else(|| {
                url::Url::parse(&url)
                    .map(|u| u.host_str().unwrap_or("unknown").to_string())
                    .unwrap_or_else(|_| "unknown".to_string())
            });
            let identifier = uuid::Uuid::new_v4().to_string();
            db::queries::insert_subscription(&conn, &identifier, &url, &name)?;
            log::info!("Subscription added: {} ({})", name, identifier);
        }
        cli::AddSource::Proxy { proto } => {
            let server = db::models::ProxyServer::from_protocol(proto)?;
            db::queries::insert_proxy_server(&conn, &server)?;
            log::info!("Proxy server added: {}", server.name);
        }
    }

    Ok(())
}

fn handle_list(target: cli::ListTarget) -> anyhow::Result<()> {
    let db_path = db::db_path();
    let conn = db::open(&db_path)?;

    match target {
        cli::ListTarget::Subscriptions => {
            let subs = db::queries::get_all_subscriptions(&conn)?;
            if subs.is_empty() {
                println!("No subscriptions found.");
            } else {
                for sub in &subs {
                    println!(
                        "[{}] {} - {} (traffic: {}/{})",
                        sub.id, sub.name.as_deref().unwrap_or("N/A"),
                        sub.subscription_url.as_deref().unwrap_or("N/A"),
                        sub.used_traffic, sub.total_traffic
                    );
                }
            }
        }
        cli::ListTarget::Proxies => {
            let servers = db::queries::get_all_proxy_servers(&conn)?;
            if servers.is_empty() {
                println!("No proxy servers found.");
            } else {
                for s in &servers {
                    let active = if s.is_active != 0 { " [ACTIVE]" } else { "" };
                    println!(
                        "[{}] {} - {}://{}:{}{}  id={}",
                        s.id, s.name, s.proxy_type, s.server_address, s.server_port, active,
                        &s.identifier[..s.identifier.len().min(12)]
                    );
                }
            }
        }
        cli::ListTarget::Groups => {
            let groups = db::queries::get_all_proxy_groups(&conn)?;
            if groups.is_empty() {
                println!("No proxy groups found.");
            } else {
                for g in &groups {
                    let active = if g.is_active != 0 { " [ACTIVE]" } else { "" };
                    println!(
                        "[{}] {} - type: {}{}",
                        g.id, g.name, g.group_type, active
                    );
                }
            }
        }
    }

    Ok(())
}

fn handle_import(links: &[String], file: Option<&str>) -> anyhow::Result<()> {
    let text = if let Some(path) = file {
        std::fs::read_to_string(path)?
    } else if !links.is_empty() {
        links.join("\n")
    } else {
        // Read from stdin
        let mut input = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)?;
        input
    };

    if text.trim().is_empty() {
        anyhow::bail!("No share links provided. Use --file, pass links as arguments, or pipe from stdin.");
    }

    let parsed = utils::proxy_parser::parse_share_links(&text);
    if parsed.is_empty() {
        anyhow::bail!("No valid proxy links found in input.");
    }

    let db_path = db::db_path();
    let conn = db::open(&db_path)?;
    let now = chrono::Utc::now().timestamp();
    let mut count = 0;

    for p in &parsed {
        let identifier = uuid::Uuid::new_v4().to_string();
        let server = db::models::ProxyServer {
            id: 0,
            identifier: identifier.clone(),
            name: p.name.clone(),
            server_address: p.server.clone(),
            server_port: p.port as i64,
            password: p.password.clone(),
            encryption_method: if p.method.is_empty() {
                "aes-256-gcm".to_string()
            } else {
                p.method.clone()
            },
            plugin: p.plugin.clone(),
            plugin_opts: p.plugin_opts.clone(),
            is_active: 0,
            created_at: now,
            updated_at: now,
            proxy_type: p.proxy_type.clone(),
            username: p.username.clone(),
            vless_uuid: p.vless_uuid.clone(),
            vless_opts: p.vless_opts.clone(),
        };

        match db::queries::insert_proxy_server(&conn, &server) {
            Ok(_) => {
                log::info!(
                    "Imported: [{}] {} - {}://{}:{}",
                    p.proxy_type,
                    p.name,
                    p.proxy_type,
                    p.server,
                    p.port
                );
                count += 1;
            }
            Err(e) => {
                log::warn!("Failed to import {}: {}", p.name, e);
            }
        }
    }

    log::info!("Successfully imported {} proxy servers", count);
    println!("Imported {} proxy servers", count);

    Ok(())
}

fn set_active_proxies(ids: &[String]) -> anyhow::Result<()> {
    let db_path = db::db_path();
    let conn = db::open(&db_path)?;

    for id in ids {
        // Try as a proxy server first
        if let Ok(Some(_server)) = db::queries::get_proxy_server_by_identifier(&conn, id) {
            db::queries::set_active_proxy_server(&conn, id)?;
            log::info!("Activated proxy server: {}", id);
        } else if let Ok(Some(_group)) = db::queries::get_proxy_group_by_identifier(&conn, id) {
            db::queries::set_active_proxy_group(&conn, id)?;
            log::info!("Activated proxy group: {}", id);
        } else {
            log::warn!("No proxy server or group found with identifier: {}", id);
        }
    }

    Ok(())
}

fn handle_remove(target: cli::RemoveTarget) -> anyhow::Result<()> {
    let db_path = db::db_path();
    let conn = db::open(&db_path)?;

    match target {
        cli::RemoveTarget::Subscription { id } => {
            db::queries::delete_subscription(&conn, id)?;
            log::info!("Subscription {} removed", id);
        }
        cli::RemoveTarget::Proxy { id } => {
            db::queries::delete_proxy_server(&conn, id)?;
            log::info!("Proxy server {} removed", id);
        }
        cli::RemoveTarget::Group { id } => {
            db::queries::delete_proxy_group(&conn, id)?;
            log::info!("Proxy group {} removed", id);
        }
    }

    Ok(())
}

// ── daemon state persist for restart ──

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct StartState {
    mode: cli::ProxyModeArg,
    subscription: Option<String>,
    proxy_ids: Vec<String>,
}

fn state_path() -> String {
    format!("{}/start-state.json", core::config_dir())
}

fn save_start_state(mode: &cli::ProxyModeArg, subscription: &Option<String>, proxy_ids: &[String]) {
    let state = StartState {
        mode: mode.clone(),
        subscription: subscription.clone(),
        proxy_ids: proxy_ids.to_vec(),
    };
    if let Ok(json) = serde_json::to_string(&state) {
        let _ = std::fs::write(state_path(), json);
    }
}

fn load_start_state() -> Option<StartState> {
    let json = std::fs::read_to_string(state_path()).ok()?;
    serde_json::from_str(&json).ok()
}
