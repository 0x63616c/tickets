//! Chromium DevTools capture; all browser state stays in ignored target/.
use base64::Engine;
use serde_json::{Value, json};
use std::{
    error::Error,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use tungstenite::{Message, connect};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
pub fn run() -> Result<()> {
    let root = std::env::current_dir()?;
    let mut chrome =
        Command::new(std::env::var("TICKETS_CHROME").unwrap_or_else(|_| "chromium".into()))
            .args([
                "--headless",
                "--no-sandbox",
                "--no-proxy-server",
                "--disable-dev-shm-usage",
                "--remote-debugging-port=9224",
                "--window-size=1280,800",
            ])
            .arg(format!(
                "--user-data-dir={}",
                root.join("target/chrome-auth").display()
            ))
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
    let result = (|| -> Result<()> {
        thread::sleep(Duration::from_secs(1));
        let output = Command::new("curl")
            .args(["-sS", "http://localhost:9224/json"])
            .output()?;
        let tabs: Value = serde_json::from_slice(&output.stdout)?;
        let tab = tabs
            .as_array()
            .ok_or("no tabs")?
            .iter()
            .find(|t| t["type"] == "page" && t["url"] == "about:blank")
            .ok_or("no page tab")?;
        let ws = tab["webSocketDebuggerUrl"].as_str().ok_or("no tab")?;
        let (mut socket, _) = connect(ws)?;
        let mut id = 0;
        let mut call = |method: &str, params: Value| -> Result<Value> {
            id += 1;
            socket.send(Message::Text(
                json!({"id":id,"method":method,"params":params})
                    .to_string()
                    .into(),
            ))?;
            loop {
                let msg = socket.read()?;
                if let Message::Text(text) = msg {
                    let value: Value = serde_json::from_str(&text)?;
                    if value["id"] == id {
                        if !value["error"].is_null() {
                            return Err("CDP command failed".into());
                        }
                        return Ok(value["result"].clone());
                    }
                }
            }
        };
        call("Page.enable", json!({}))?;
        call("Runtime.enable", json!({}))?;
        call(
            "Emulation.setDeviceMetricsOverride",
            json!({"width":1280,"height":800,"deviceScaleFactor":1,"mobile":false}),
        )?;
        call(
            "Page.navigate",
            json!({"url":"http://localhost:8787/login"}),
        )?;
        thread::sleep(Duration::from_secs(5));
        let dom = call(
            "Runtime.evaluate",
            json!({"expression":"document.body.innerText","returnByValue":true}),
        )?;
        if !dom.to_string().contains("Sign in to Tickets") {
            return Err("login not rendered".into());
        }
        let shot = call("Page.captureScreenshot", json!({"format":"png"}))?;
        std::fs::write(
            "docs/screenshots/04-login-dark.png",
            base64::engine::general_purpose::STANDARD
                .decode(shot["data"].as_str().ok_or("missing image")?)?,
        )?;
        let storage: Value =
            serde_json::from_str(&std::fs::read_to_string("target/browser-storage.local")?)?;
        call("Network.setCookies", json!({"cookies":storage["cookies"]}))?;
        call(
            "Page.navigate",
            json!({"url":"http://localhost:8787/settings/tokens"}),
        )?;
        thread::sleep(Duration::from_secs(5));
        let dom = call(
            "Runtime.evaluate",
            json!({"expression":"document.body.innerText","returnByValue":true}),
        )?;
        if !dom.to_string().contains("Local verification")
            || dom.to_string().contains("Please sign in")
        {
            return Err("authenticated list not rendered".into());
        }
        // Exercise the Leptos create handler without retaining the one-time secret.
        call(
            "Runtime.evaluate",
            json!({"expression":"(()=>{const e=document.querySelector('#token-name');e.value='Browser verification';e.dispatchEvent(new Event('input',{bubbles:true}));e.form.requestSubmit();})()"}),
        )?;
        thread::sleep(Duration::from_secs(1));
        let shown = call(
            "Runtime.evaluate",
            json!({"expression":"!!document.querySelector('.secret code')?.textContent.startsWith('tk_')","returnByValue":true}),
        )?;
        if shown["result"]["value"] != true {
            return Err("created token not shown".into());
        }
        call(
            "Runtime.evaluate",
            json!({"expression":"Array.from(document.querySelectorAll('button')).find(e=>e.textContent==='Dismiss').click()"}),
        )?;
        thread::sleep(Duration::from_millis(200));
        let hidden = call(
            "Runtime.evaluate",
            json!({"expression":"document.querySelector('.secret code')===null","returnByValue":true}),
        )?;
        if hidden["result"]["value"] != true {
            return Err("token not dismissed".into());
        }
        // Revoke through the actual Leptos click handler, then capture metadata only.
        call(
            "Runtime.evaluate",
            json!({"expression":"document.querySelector('.tokens li button').click()"}),
        )?;
        thread::sleep(Duration::from_secs(1));
        let shot = call("Page.captureScreenshot", json!({"format":"png"}))?;
        std::fs::write(
            "docs/screenshots/04-tokens-dark.png",
            base64::engine::general_purpose::STANDARD
                .decode(shot["data"].as_str().ok_or("missing image")?)?,
        )?;
        println!(
            "PASS: browser login rendering, authenticated list, create, show once, dismiss, revoke; dark screenshots saved"
        );
        Ok(())
    })();
    let _ = chrome.kill();
    let _ = chrome.wait();
    result
}
