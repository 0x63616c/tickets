//! Internal DO traffic is reachable only through the authenticated Worker.
use tickets_core::live::ChangeEvent;
use worker::*;

#[durable_object]
pub struct BoardRoom {
    state: State,
}
impl DurableObject for BoardRoom {
    fn new(state: State, _env: Env) -> Self {
        Self { state }
    }
    async fn fetch(&self, mut req: Request) -> Result<Response> {
        if req.method() == Method::Post {
            let event: ChangeEvent = req.json().await?;
            let text = serde_json::to_string(&event)?;
            for socket in self.state.get_websockets() {
                if socket.send_with_str(&text).is_err() {
                    let _ = socket.close(Some(1011), Some("Reconnect"));
                }
            }
            return Response::empty();
        }
        if req.headers().get("Upgrade")?.as_deref() != Some("websocket") {
            return Response::error("WebSocket upgrade required", 426);
        }
        let pair = WebSocketPair::new()?;
        self.state.accept_web_socket(&pair.server);
        Response::from_websocket(pair.client)
    }
    async fn websocket_message(
        &self,
        socket: WebSocket,
        _message: WebSocketIncomingMessage,
    ) -> Result<()> {
        socket.close(Some(1008), Some("Receive-only connection"))
    }
    async fn websocket_close(
        &self,
        socket: WebSocket,
        code: usize,
        _reason: String,
        _clean: bool,
    ) -> Result<()> {
        socket.close::<&str>(Some(if code == 1005 { 1000 } else { code as u16 }), None)
    }
    async fn websocket_error(&self, socket: WebSocket, _error: Error) -> Result<()> {
        socket.close(Some(1011), Some("Reconnect"))
    }
}

pub async fn upgrade(req: Request, env: &Env, db: &D1Database, project: i64) -> Result<Response> {
    use crate::repository::Store;
    let exists = Store::query::<serde_json::Value>(
        db,
        "SELECT id FROM projects WHERE id=?",
        vec![project.into()],
    )
    .await
    .map_err(|_| Error::RustError("Project lookup failed".into()))?;
    if exists.is_empty() {
        return Response::error("Project not found", 404);
    }
    // Day one has one owner: all authenticated credentials see all projects.
    env.durable_object("BOARD_ROOMS")?
        .id_from_name(&project.to_string())?
        .get_stub()?
        .fetch_with_request(req)
        .await
}

pub async fn notify(env: &Env, db: &D1Database, change: crate::api::CommittedChange) -> Result<()> {
    use crate::repository::Store;
    let crate::api::CommittedChange {
        entity,
        id,
        action,
        mut projects,
    } = change;
    // Labels are global. Invalidate every project, including currently empty boards.
    if projects.is_empty() {
        projects = Store::query::<serde_json::Value>(db, "SELECT id FROM projects", vec![])
            .await
            .map_err(|_| Error::RustError("Project lookup failed".into()))?
            .into_iter()
            .filter_map(|p| p["id"].as_i64())
            .collect();
    }
    let namespace = env.durable_object("BOARD_ROOMS")?;
    for project_id in projects {
        let event = ChangeEvent {
            project_id,
            entity: entity.clone(),
            id,
            action: action.clone(),
        };
        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_body(Some(serde_json::to_string(&event)?.into()));
        let request = Request::new_with_init("https://board-room/notify", &init)?;
        // Keep notifying other rooms if one fails (e.g. a cross-project move).
        let result = async {
            namespace
                .id_from_name(&project_id.to_string())?
                .get_stub()?
                .fetch_with_request(request)
                .await
        }
        .await;
        if !matches!(result, Ok(response) if response.status_code() < 400) {
            console_error!("Live notification failed; clients reconcile on reconnect");
        }
    }
    Ok(())
}
