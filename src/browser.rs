use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use chromiumoxide::browser::Browser;
use chromiumoxide::cdp::browser_protocol::input::InsertTextParams;
use chromiumoxide::cdp::browser_protocol::network::{
    CookieParam, CookieSameSite, SetCookiesParams, TimeSinceEpoch,
};
use chromiumoxide::cdp::browser_protocol::page::NavigateParams;
use chromiumoxide::Page;
use futures::StreamExt;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tracing::{debug, info, warn};

const CONVERSATION_SELECTOR: &str =
    "[data-e2e=\"chat-list-item\"], [data-e2e=\"dm-new-conversation-item\"]";
const MESSAGE_INPUT_SELECTOR: &str = "[data-e2e=\"message-input-area\"] [contenteditable=\"true\"]";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
    pub expires: Option<f64>,
}

pub async fn load_cookies(path: &Path) -> Result<Vec<SavedCookie>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read cookies: {}", path.display()))?;
    let cookies = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse cookies: {}", path.display()))?;
    Ok(cookies)
}

pub async fn save_cookies(page: &Page, path: &Path) -> Result<()> {
    let cookies = page
        .get_cookies()
        .await
        .context("failed to read browser cookies")?;
    let saved: Vec<SavedCookie> = cookies
        .into_iter()
        .map(|c| SavedCookie {
            name: c.name,
            value: c.value,
            domain: c.domain,
            path: c.path,
            secure: c.secure,
            http_only: c.http_only,
            same_site: c.same_site.map(|s| format!("{s:?}")),
            expires: Some(c.expires),
        })
        .collect();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let raw = serde_json::to_string_pretty(&saved).context("failed to serialize cookies")?;
    tokio::fs::write(path, raw)
        .await
        .with_context(|| format!("failed to write cookies: {}", path.display()))?;
    Ok(())
}

pub async fn launch(
    cfg: &Config,
    headless: bool,
    browser_path: Option<&str>,
) -> Result<(Browser, tokio::task::JoinHandle<()>)> {
    let path = browser_path
        .map(str::to_owned)
        .or_else(|| cfg.browser_path.clone());
    if let Some(p) = path {
        if !p.trim().is_empty() {
            std::env::set_var("CLOAKBROWSER_BINARY_PATH", p);
        }
    }

    let mut extra_args = cfg.browser_args.clone();
    extra_args.push("--disable-blink-features=AutomationControlled".to_string());
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        extra_args.push("--ozone-platform=wayland".to_string());
    } else if cfg!(target_os = "linux") {
        extra_args.push("--ozone-platform=x11".to_string());
    }

    let opts = cloak_cdp::LaunchOptions {
        headless,
        extra_args,
        ..Default::default()
    };
    let (browser, mut handler) = cloak_cdp::launch(opts)
        .await
        .map_err(|e| anyhow!(e))
        .context("failed to launch stealth browser via cloak-cdp")?;
    let handle = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if let Err(e) = event {
                debug!("browser event error: {e}");
            }
        }
    });
    Ok((browser, handle))
}

pub async fn apply_cookies(page: &Page, cookies: &[SavedCookie]) -> Result<()> {
    if cookies.is_empty() {
        return Ok(());
    }
    let params: Vec<CookieParam> = cookies
        .iter()
        .map(|c| {
            let mut p = CookieParam::new(c.name.clone(), c.value.clone());
            p.domain = Some(c.domain.clone());
            p.path = Some(c.path.clone());
            p.secure = Some(c.secure);
            p.http_only = Some(c.http_only);
            if let Some(exp) = c.expires {
                p.expires = Some(TimeSinceEpoch::new(exp));
            }
            if let Some(ss) = &c.same_site {
                p.same_site = match ss.as_str() {
                    "Strict" => Some(CookieSameSite::Strict),
                    "Lax" => Some(CookieSameSite::Lax),
                    "None" => Some(CookieSameSite::None),
                    _ => None,
                };
            }
            p
        })
        .collect();
    page.execute(SetCookiesParams::new(params))
        .await
        .context("failed to set cookies")?;
    Ok(())
}

pub async fn goto(page: &Page, url: &str) -> Result<()> {
    page.execute(NavigateParams::new(url))
        .await
        .with_context(|| format!("failed to navigate to {url}"))?;
    page.wait_for_navigation().await.ok();
    Ok(())
}

pub async fn wait_for_login(page: &Page, timeout: Duration) -> Result<()> {
    info!("waiting for login...");
    let start = tokio::time::Instant::now();
    loop {
        if start.elapsed() > timeout {
            anyhow::bail!("timed out waiting for login after {}s", timeout.as_secs());
        }
        let url = page.url().await.ok().flatten().unwrap_or_default();
        let url_ok = !url.contains("/login")
            && url.contains("tiktok.com")
            && url != "https://www.tiktok.com/"
            && !url.is_empty();
        let dom_ok = element_exists(page, CONVERSATION_SELECTOR).await?
            || element_exists(page, "[data-e2e=\"recommend-list-item-container\"]").await?
            || element_exists(page, "[data-e2e=\"browse-video\"]").await?;
        if url_ok || dom_ok {
            info!("login detected (url={url})");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn element_exists(page: &Page, selector: &str) -> Result<bool> {
    let js = format!(
        "document.querySelector({}) !== null",
        serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string())
    );
    let value = page.evaluate(js).await;
    Ok(match value {
        Ok(v) => v.into_value::<bool>().unwrap_or(false),
        Err(_) => false,
    })
}

async fn wait_for_selector(page: &Page, selector: &str, timeout: Duration) -> Result<()> {
    let start = tokio::time::Instant::now();
    loop {
        if element_exists(page, selector).await? {
            return Ok(());
        }
        if start.elapsed() > timeout {
            anyhow::bail!("timed out waiting for selector: {selector}");
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

async fn click_nth(page: &Page, selector: &str, index: usize) -> Result<()> {
    let js = format!(
        "(() => {{ const els = document.querySelectorAll({}); if (els[{index}]) {{ els[{index}].scrollIntoView({{block:'center'}}); els[{index}].click(); return true; }} return false; }})()",
        serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string())
    );
    let ok = page
        .evaluate(js)
        .await
        .context("failed to evaluate click script")?
        .into_value::<bool>()
        .unwrap_or(false);
    if !ok {
        anyhow::bail!("could not click element {selector} at index {index}");
    }
    Ok(())
}

async fn type_text(page: &Page, text: &str, delay_ms: u64) -> Result<()> {
    let js = format!(
        "(() => {{ const el = document.querySelector({}); if (!el) return false; el.focus(); return true; }})()",
        serde_json::to_string(MESSAGE_INPUT_SELECTOR).unwrap_or_else(|_| "\"\"".to_string())
    );
    let focused = page
        .evaluate(js)
        .await
        .context("failed to focus message input")?
        .into_value::<bool>()
        .unwrap_or(false);
    if !focused {
        anyhow::bail!("message input not found");
    }
    let mut rng = rand::thread_rng();
    for ch in text.chars() {
        page.execute(InsertTextParams::new(ch.to_string()))
            .await
            .context("failed to insert text")?;
        let jitter = if delay_ms > 20 {
            rng.gen_range(0..=(delay_ms / 3))
        } else {
            0
        };
        tokio::time::sleep(Duration::from_millis(delay_ms.saturating_add(jitter))).await;
    }
    Ok(())
}

async fn press_enter(page: &Page) -> Result<()> {
    let js = format!(
        r#"(() => {{
        const selectors = ['[data-e2e="dm-new-send-btn"]','[data-e2e="message-send-button"]','[data-e2e="send-message-button"]','button[type="submit"]'];
        for (const sel of selectors) {{
            const btn = document.querySelector(sel);
            if (btn && !btn.disabled && btn.getAttribute('aria-disabled') !== 'true') {{ btn.click(); return 'clicked'; }}
        }}
        const el = document.querySelector({});
        if (el) {{
            el.dispatchEvent(new KeyboardEvent('keydown', {{key:'Enter',code:'Enter',keyCode:13,which:13,bubbles:true}}));
            el.dispatchEvent(new KeyboardEvent('keyup', {{key:'Enter',code:'Enter',keyCode:13,which:13,bubbles:true}}));
            return 'key';
        }}
        return 'none';
    }})()"#,
        serde_json::to_string(MESSAGE_INPUT_SELECTOR).unwrap_or_else(|_| "\"\"".to_string())
    );
    page.evaluate(js).await.context("failed to send message")?;
    Ok(())
}

pub async fn send_streak_messages(
    page: &Page,
    cfg: &Config,
    dry_run: bool,
    limit: Option<usize>,
) -> Result<usize> {
    wait_for_selector(page, CONVERSATION_SELECTOR, Duration::from_secs(30)).await?;
    let count_js = format!(
        "document.querySelectorAll({}).length",
        serde_json::to_string(CONVERSATION_SELECTOR).unwrap_or_else(|_| "\"\"".to_string())
    );
    let total = page
        .evaluate(count_js)
        .await
        .context("failed to count conversations")?
        .into_value::<usize>()
        .unwrap_or(0);
    if total == 0 {
        warn!("no conversations found");
        return Ok(0);
    }
    let target = limit.map(|n| n.min(total)).unwrap_or(total);
    info!("found {total} conversations; processing {target}");
    let mut sent = 0usize;
    let mut rng = rand::thread_rng();
    for i in 0..target {
        let result: Result<()> = async {
            click_nth(page, CONVERSATION_SELECTOR, i).await?;
            wait_for_selector(page, MESSAGE_INPUT_SELECTOR, Duration::from_secs(15)).await?;
            tokio::time::sleep(Duration::from_millis(500)).await;
            let msg = cfg
                .messages
                .get(rng.gen_range(0..cfg.messages.len()))
                .cloned()
                .unwrap_or_else(|| "hey".to_string());
            if dry_run {
                info!("[{}/{}] dry-run: would send {:?}", i + 1, target, msg);
            } else {
                type_text(page, &msg, cfg.typing_delay_ms).await?;
                tokio::time::sleep(Duration::from_millis(350)).await;
                press_enter(page).await?;
                info!("[{}/{}] sent {:?}", i + 1, target, msg);
                sent += 1;
            }
            let (min_d, max_d) = cfg.delay_between_ms;
            let delay = if max_d > min_d {
                rng.gen_range(min_d..=max_d)
            } else {
                min_d
            };
            tokio::time::sleep(Duration::from_millis(delay)).await;
            Ok(())
        }
        .await;
        if let Err(e) = result {
            warn!("[{}/{}] failed: {e:#}", i + 1, target);
        }
    }
    Ok(if dry_run { target } else { sent })
}
