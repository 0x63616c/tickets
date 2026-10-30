//! HMAC-authenticated GitHub App events and WebCrypto installation authentication.
use crate::github_app_events as storage;
use js_sys::{Array, Object, Reflect, Uint8Array};
use serde_json::{Value, json};
use tickets_core::{github_app::AppCredentials, github_app_checks as checks};
use wasm_bindgen_futures::JsFuture;
use worker::{wasm_bindgen::JsCast, *};

fn unavailable() -> Error {
    Error::RustError("GitHub App operation failed".into())
}

async fn jwt(credentials: &AppCredentials) -> Result<String> {
    let der = checks::pem_pkcs8(&credentials.secrets.pem).ok_or_else(unavailable)?;
    let input = checks::jwt_signing_input(
        credentials.app.app_id,
        (Date::now().as_millis() / 1000) as i64,
    );
    let crypto: web_sys::Crypto =
        Reflect::get(&js_sys::global(), &"crypto".into())?.unchecked_into();
    let algorithm = Object::new();
    Reflect::set(&algorithm, &"name".into(), &"RSASSA-PKCS1-v1_5".into())?;
    Reflect::set(&algorithm, &"hash".into(), &"SHA-256".into())?;
    let key = JsFuture::from(crypto.subtle().import_key_with_object(
        "pkcs8",
        &Uint8Array::from(der.as_slice()),
        &algorithm,
        false,
        &Array::of1(&"sign".into()),
    )?)
    .await?;
    let signature = JsFuture::from(crypto.subtle().sign_with_str_and_u8_array(
        "RSASSA-PKCS1-v1_5",
        &key.unchecked_into(),
        input.as_bytes(),
    )?)
    .await?;
    Ok(format!(
        "{input}.{}",
        checks::base64url(&Uint8Array::new(&signature).to_vec())
    ))
}

/// Configurable for local mock servers. Redirects never receive authorization.
async fn api(
    env: &Env,
    token: &str,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value> {
    let base = env
        .var("GITHUB_API_BASE_URL")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "https://api.github.com".into());
    let mut init = RequestInit::new();
    init.with_method(method)
        .with_redirect(RequestRedirect::Manual);
    init.headers
        .set("Authorization", &format!("Bearer {token}"))?;
    init.headers.set("Accept", "application/vnd.github+json")?;
    init.headers.set("X-GitHub-Api-Version", "2022-11-28")?;
    init.headers.set("User-Agent", "tickets-worldwidewebb")?;
    if let Some(body) = body {
        init.headers.set("Content-Type", "application/json")?;
        init.with_body(Some(body.to_string().into()));
    }
    let request = Request::new_with_init(&format!("{}{path}", base.trim_end_matches('/')), &init)?;
    let mut response = Fetch::Request(request).send().await?;
    if !(200..300).contains(&response.status_code()) {
        return Err(unavailable());
    }
    response.json().await
}

async fn token(env: &Env, credentials: &AppCredentials, installation: i64) -> Result<String> {
    if installation <= 0 {
        return Err(unavailable());
    }
    let response = api(
        env,
        &jwt(credentials).await?,
        Method::Post,
        &format!("/app/installations/{installation}/access_tokens"),
        Some(json!({})),
    )
    .await?;
    response["token"]
        .as_str()
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .ok_or_else(unavailable)
}

fn repository(env: &Env, payload: &Value) -> Option<String> {
    let repo = payload["repository"]["full_name"]
        .as_str()?
        .to_ascii_lowercase();
    let (owner, name) = repo.split_once('/')?;
    if !payload["repository"]["owner"]["login"]
        .as_str()?
        .eq_ignore_ascii_case(owner)
    {
        return None;
    }
    if !tickets_core::github::valid_owner(owner)
        || !tickets_core::github::valid_repo(name)
        || !crate::github_prs::owner_allowed(env, owner)
    {
        return None;
    }
    Some(repo)
}

async fn post_check(env: &Env, token: &str, repo: &str, pr: &Value) -> Result<()> {
    let number = pr["number"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(unavailable)?;
    let sha = pr["head"]["sha"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(unavailable)?;
    let title = pr["title"].as_str().ok_or_else(unavailable)?;
    let links = tickets_core::pull_request::links(title, pr["body"].as_str().unwrap_or_default());
    let db = env.d1("DB")?;
    let tickets = storage::tickets(&db, &links)
        .await
        .map_err(|_| unavailable())?;
    let output = checks::check_output(sha, &links, &tickets);
    let result = api(
        env,
        token,
        Method::Post,
        &format!("/repos/{repo}/check-runs"),
        Some(output.clone()),
    )
    .await?;
    let id = result["id"]
        .as_i64()
        .filter(|n| *n > 0)
        .ok_or_else(unavailable)?;
    storage::save_check(
        &db,
        repo,
        number,
        sha,
        output["conclusion"].as_str().unwrap(),
        id,
    )
    .await
    .map_err(|_| unavailable())?;
    notify(env, &db).await;
    Ok(())
}

async fn notify(env: &Env, db: &D1Database) {
    let _ = crate::live::notify(
        env,
        db,
        crate::api::CommittedChange {
            entity: "tickets".into(),
            id: 0,
            action: "update".into(),
            projects: vec![],
        },
    )
    .await;
}

async fn recompute(
    env: &Env,
    credentials: &AppCredentials,
    payload: &Value,
    prs: &[Value],
) -> Result<()> {
    let Some(repo) = repository(env, payload) else {
        return Ok(());
    };
    if prs.is_empty() {
        return Ok(());
    }
    let installation = payload["installation"]["id"]
        .as_i64()
        .ok_or_else(unavailable)?;
    let access = token(env, credentials, installation).await?;
    // Continue remaining PRs when one lookup/check fails.
    let mut failed = false;
    for pr in prs {
        let result = async {
            let number = pr["number"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or_else(unavailable)?;
            let fetched;
            let pr = if pr["title"].is_string() {
                pr
            } else {
                fetched = api(
                    env,
                    &access,
                    Method::Get,
                    &format!("/repos/{repo}/pulls/{number}"),
                    None,
                )
                .await?;
                &fetched
            };
            post_check(env, &access, &repo, pr).await
        }
        .await;
        failed |= result.is_err();
    }
    if failed { Err(unavailable()) } else { Ok(()) }
}

pub async fn webhook(mut req: Request, env: &Env) -> Result<Response> {
    if req.method() != Method::Post {
        return Response::error("Method not allowed", 405);
    }
    let credentials = match crate::github_app::load_credentials(env).await {
        Ok(Some(credentials)) => credentials,
        Ok(None) => return Response::error("GitHub App unavailable", 503),
        Err(_) => return Response::error("GitHub App unavailable", 503),
    };
    let bytes =
        match crate::github_mirror::signed_body(&mut req, &credentials.secrets.webhook_secret)
            .await?
        {
            Ok(bytes) => bytes,
            Err(response) => return Ok(response),
        };
    let payload: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return Response::error("Malformed payload", 400),
    };
    let event = req.headers().get("X-GitHub-Event")?.unwrap_or_default();
    let action = payload["action"].as_str().unwrap_or_default();
    match event.as_str() {
        "ping" => Response::empty().map(|r| r.with_status(204)),
        "installation" | "installation_repositories" => {
            let supported = if event == "installation" {
                [
                    "created",
                    "deleted",
                    "suspend",
                    "unsuspend",
                    "new_permissions_accepted",
                ]
                .contains(&action)
            } else {
                ["added", "removed"].contains(&action)
            };
            if !supported {
                return Response::empty().map(|r| r.with_status(202));
            }
            if let Err(error) = storage::installation(&env.d1("DB")?, &event, &payload).await {
                return match error {
                    crate::repository::Error::Validation(_) => {
                        Response::error("Invalid installation payload", 400)
                    }
                    _ => Response::error("Installation storage failed", 500),
                };
            }
            Response::empty().map(|r| r.with_status(204))
        }
        "pull_request" => {
            // This remains the single ingestion path, including allowlist/replay rules.
            let response = crate::github_prs::handle(env, &event, &bytes).await?;
            if response.status_code() < 300
                && [
                    "opened",
                    "reopened",
                    "synchronize",
                    "edited",
                    "ready_for_review",
                ]
                .contains(&action)
                && recompute(
                    env,
                    &credentials,
                    &payload,
                    &[payload["pull_request"].clone()],
                )
                .await
                .is_err()
            {
                console_error!("Tickets check could not be posted");
            }
            Ok(response)
        }
        "check_suite" | "check_run" => {
            let check = &payload[event.as_str()];
            let own = check["app"]["id"].as_i64() == Some(credentials.app.app_id);
            if event == "check_suite" && action == "completed" && !own {
                if let Some(repo) = repository(env, &payload) {
                    let db = env.d1("DB")?;
                    storage::save_ci(&db, &repo, check)
                        .await
                        .map_err(|_| unavailable())?;
                    // CI completion used to broadcast a global ticket id=0,
                    // including suites with no Tickets links. Send point invalidations.
                    if let Ok(tickets) = storage::ci_tickets(
                        &db,
                        &repo,
                        check["head_sha"].as_str().unwrap_or_default(),
                    )
                    .await
                    {
                        for ticket in tickets {
                            let _ = crate::live::notify(
                                env,
                                &db,
                                crate::api::CommittedChange {
                                    entity: "tickets".into(),
                                    id: ticket["id"].as_i64().unwrap(),
                                    action: "update".into(),
                                    projects: vec![ticket["project_id"].as_i64().unwrap()],
                                },
                            )
                            .await;
                        }
                    } else {
                        console_error!("CI notification lookup failed");
                    }
                }
            } else if (event == "check_suite" && ["requested", "rerequested"].contains(&action))
                || (event == "check_run"
                    && action == "rerequested"
                    && own
                    && check["name"] == "Tickets")
            {
                if recompute(
                    env,
                    &credentials,
                    &payload,
                    check["pull_requests"]
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or_default(),
                )
                .await
                .is_err()
                {
                    // Request retry for standalone check requests; PR status writes above succeed.
                    return Response::error("Tickets check unavailable", 502);
                }
            } else {
                return Response::empty().map(|r| r.with_status(202));
            }
            Response::empty().map(|r| r.with_status(204))
        }
        _ => Response::empty().map(|r| r.with_status(202)),
    }
}
