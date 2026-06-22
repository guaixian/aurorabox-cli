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
            config_dir,
            web_server,
            port,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let proxy_mode = proxy::ProxyMode::from(mode.clone());

                // Generate config
                log::info!("Generating config for mode: {:?}", mode);
                let config_path = core::generate_and_write_config(
                    &mode.to_mode_str(),
                    false, // tun flag
                    subscription.as_deref(),
                    config_dir.as_deref(),
                )?;
                log::info!("Config written to: {}", config_path);

                // Start sing-box
                log::info!("Starting sing-box...");
                proxy::process::start_singbox(&config_path, proxy_mode)?;

                // Wait for readiness
                log::info!("Waiting for engine to be ready...");
                if proxy::readiness::wait_ready(std::time::Duration::from_secs(20)).await {
                    proxy::manager::transition(proxy::manager::Intent::MarkRunning)?;
                    log::info!("Engine is running!");

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
                        // Block and wait for sing-box to exit
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
                        "[{}] {} - {}://{}:{}{}",
                        s.id, s.name, s.proxy_type, s.server_address, s.server_port, active
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
