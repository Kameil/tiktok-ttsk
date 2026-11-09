mod browser;
mod config;
mod notify;
mod state;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::Config;
use state::State;
use std::path::PathBuf;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "tiktok-ttsk", version, about = "TikTok Streak Keeper")]
struct Cli {
    #[arg(long, global = true, env = "TIKTOK_TTSK_DATA_DIR")]
    data_dir: Option<PathBuf>,

    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Run {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        headless: bool,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        force: bool,
    },
    Login {
        #[arg(long, default_value_t = 300)]
        timeout: u64,
    },
    Status,
    InitConfig,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let data_dir = match cli.data_dir {
        Some(p) => {
            std::fs::create_dir_all(&p)?;
            p
        }
        None => config::data_dir()?,
    };
    init_logging(&data_dir)?;

    let config_path = cli.config.unwrap_or_else(|| data_dir.join("config.toml"));
    let cfg = Config::load(&config_path)?;
    let state_path = data_dir.join("state.json");
    let cookies_path = data_dir.join("cookies.json");
    info!("data dir: {}", data_dir.display());

    match cli.command.unwrap_or(Command::Run {
        dry_run: false,
        headless: false,
        limit: None,
        force: false,
    }) {
        Command::InitConfig => {
            if config_path.exists() {
                println!("config already exists: {}", config_path.display());
            } else {
                Config::save_default(&config_path)?;
                println!("wrote default config: {}", config_path.display());
            }
        }
        Command::Status => {
            let st = State::load(&state_path)?;
            println!("data_dir:    {}", data_dir.display());
            println!(
                "config:      {} ({})",
                config_path.display(),
                if config_path.exists() {
                    "found"
                } else {
                    "missing, using defaults"
                }
            );
            println!(
                "cookies:     {} ({})",
                cookies_path.display(),
                if cookies_path.exists() {
                    "found"
                } else {
                    "missing"
                }
            );
            println!("state:       {}", state_path.display());
            println!(
                "last_sent:   {}",
                st.last_sent_date.as_deref().unwrap_or("never")
            );
            println!("sent_today:  {}", st.sent_today());
            println!("messages:    {}", cfg.messages.len());
        }
        Command::Login { timeout } => {
            do_login(&cfg, &cookies_path, Duration::from_secs(timeout)).await?;
        }
        Command::Run {
            dry_run,
            headless,
            limit,
            force,
        } => {
            let mut st = State::load(&state_path)?;
            if !force && !dry_run && st.sent_today() {
                info!("already sent today");
                notify::send(
                    "Already sent today",
                    "Messages already sent today. Reopen the app tomorrow or use --force.",
                );
                println!("already sent today (use --force to override)");
                return Ok(());
            }
            do_run(
                &cfg,
                &cookies_path,
                &state_path,
                &mut st,
                dry_run,
                headless,
                limit,
            )
            .await?;
        }
    }
    Ok(())
}

fn init_logging(data_dir: &std::path::Path) -> Result<()> {
    let file_appender = tracing_appender::rolling::never(data_dir, "app.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    std::mem::forget(guard);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();
    Ok(())
}

async fn do_login(cfg: &Config, cookies_path: &std::path::Path, timeout: Duration) -> Result<()> {
    notify::send(
        "Login required",
        "Log in on the browser. Cookies will be saved automatically.",
    );
    let browser_cfg = browser::build_browser_config(cfg, false)?;
    let (browser, handle) = browser::launch(&browser_cfg).await?;
    let page = browser
        .new_page("https://www.tiktok.com/login")
        .await
        .context("failed to open login page")?;
    browser::wait_for_login(&page, timeout).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    browser::save_cookies(&page, cookies_path).await?;
    info!("cookies saved to {}", cookies_path.display());
    notify::send(
        "Login successful",
        "Cookies saved. Run `tiktok-ttsk run` to send messages.",
    );
    println!("cookies saved: {}", cookies_path.display());
    drop(browser);
    handle.abort();
    Ok(())
}

async fn do_run(
    cfg: &Config,
    cookies_path: &std::path::Path,
    state_path: &std::path::Path,
    st: &mut State,
    dry_run: bool,
    headless: bool,
    limit: Option<usize>,
) -> Result<()> {
    let cookies = browser::load_cookies(cookies_path).await?;
    if cookies.is_empty() {
        anyhow::bail!(
            "no cookies found at {}. Run `tiktok-ttsk login` first.",
            cookies_path.display()
        );
    }
    let browser_cfg = browser::build_browser_config(cfg, headless)?;
    let (browser, handle) = browser::launch(&browser_cfg).await?;
    let page = browser
        .new_page("about:blank")
        .await
        .context("failed to create browser page")?;
    browser::apply_cookies(&page, &cookies).await?;
    browser::goto(&page, &cfg.messages_url).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let sent = browser::send_streak_messages(&page, cfg, dry_run, limit).await?;
    if !dry_run {
        st.mark_sent_today();
        st.save(state_path)?;
        browser::save_cookies(&page, cookies_path).await.ok();
        notify::send(
            "Streak messages sent",
            &format!("Processed {sent} conversation(s)."),
        );
    } else {
        println!("dry-run complete: {sent} conversation(s) would be processed");
    }
    info!("done (sent={sent}, dry_run={dry_run})");
    drop(browser);
    handle.abort();
    Ok(())
}
