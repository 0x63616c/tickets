//! Browser decoding and canvas processing keep originals out of storage.
use crate::{
    components::Avatar,
    pages::settings::{Account, AccountState},
    ui::api,
};
use leptos::prelude::*;
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};

async fn process(file: web_sys::File) -> Result<web_sys::Blob, String> {
    if !matches!(
        file.type_().as_str(),
        "image/jpeg" | "image/png" | "image/webp"
    ) {
        return Err("Choose a JPEG, PNG or WebP image.".into());
    }
    if file.size() > 5.0 * 1024.0 * 1024.0 {
        return Err("Choose a photo smaller than 5 MB.".into());
    }
    let fail = || "Could not read this image. Choose another JPEG, PNG or WebP photo.".to_string();
    let bitmap: web_sys::ImageBitmap = JsFuture::from(
        window()
            .create_image_bitmap_with_blob(&file)
            .map_err(|_| fail())?,
    )
    .await
    .map_err(|_| fail())?
    .dyn_into()
    .map_err(|_| fail())?;
    let result = async {
        let canvas: web_sys::HtmlCanvasElement = document()
            .create_element("canvas")
            .map_err(|_| fail())?
            .dyn_into()
            .map_err(|_| fail())?;
        canvas.set_width(256);
        canvas.set_height(256);
        let ctx: web_sys::CanvasRenderingContext2d = canvas
            .get_context("2d")
            .map_err(|_| fail())?
            .ok_or_else(|| "Image processing is unavailable.".to_string())?
            .dyn_into()
            .map_err(|_| fail())?;
        let side = bitmap.width().min(bitmap.height()) as f64;
        if side == 0.0 {
            return Err("This image is empty. Choose another photo.".into());
        }
        ctx.draw_image_with_image_bitmap_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
            &bitmap,
            (bitmap.width() as f64 - side) / 2.0,
            (bitmap.height() as f64 - side) / 2.0,
            side,
            side,
            0.0,
            0.0,
            256.0,
            256.0,
        )
        .map_err(|_| fail())?;
        let mut blob = encode(&canvas, "image/webp", 0.82).await?;
        if blob.type_() != "image/webp" {
            blob = encode(&canvas, "image/jpeg", 0.82).await?;
        }
        if blob.size() > 100.0 * 1024.0 {
            blob = encode(&canvas, &blob.type_(), 0.65).await?;
        }
        if blob.size() > 512.0 * 1024.0 {
            return Err("Could not compress this photo. Choose a simpler image.".into());
        }
        Ok(blob)
    }
    .await;
    bitmap.close();
    result
}
async fn encode(
    canvas: &web_sys::HtmlCanvasElement,
    mime: &str,
    quality: f64,
) -> Result<web_sys::Blob, String> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        if let Err(e) = canvas.to_blob_with_type_and_encoder_options(
            &resolve,
            mime,
            &JsValue::from_f64(quality),
        ) {
            let _ = reject.call1(&JsValue::NULL, &e);
        }
    });
    JsFuture::from(promise)
        .await
        .ok()
        .and_then(|v| v.dyn_into().ok())
        .ok_or_else(|| "Could not encode this photo. Try another image.".into())
}
async fn upload(
    file: web_sys::File,
    fixture: bool,
    state: AccountState,
) -> Result<Account, String> {
    let blob = process(file).await?;
    if fixture {
        if state.account.is_disposed() {
            return Err("Account page closed.".into());
        }
        let mut account = state.account.get_untracked();
        account.avatar_url = Some(
            web_sys::Url::create_object_url_with_blob(&blob)
                .map_err(|_| "Could not preview photo.")?,
        );
        return Ok(account);
    }
    let response = gloo_net::http::Request::put("/api/v1/me/avatar")
        .header("Content-Type", &blob.type_())
        .body(blob)
        .map_err(|_| "Could not upload photo.")?
        .send()
        .await
        .map_err(|_| "Could not upload photo. Check your connection and try again.")?;
    let value: Value = response
        .json()
        .await
        .map_err(|_| "Could not read upload response.")?;
    if !response.ok() {
        return Err(value["error"]["message"]
            .as_str()
            .unwrap_or("Could not save photo. Try again.")
            .into());
    }
    serde_json::from_value(value).map_err(|_| "Could not read saved photo.".into())
}
#[component]
pub fn PhotoSettings(fixture: bool) -> impl IntoView {
    let state = expect_context::<AccountState>();
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let message = RwSignal::new(String::new());
    let input = NodeRef::<leptos::html::Input>::new();
    let finish = move |result: Result<Account, String>, success: &str| {
        if busy.is_disposed() {
            return;
        }
        match result {
            Ok(account) => {
                if fixture && let Some(url) = state.account.get_untracked().avatar_url {
                    let _ = web_sys::Url::revoke_object_url(&url);
                }
                state.account.set(account);
                if !fixture {
                    state.load_users();
                }
                message.set(success.into());
            }
            Err(e) => error.set(e),
        }
        busy.set(false);
    };
    view! {<div class="settings-group profile-photo" aria-busy=move||busy.get().to_string()><h2>"Profile photo"</h2>
        <div class="profile-photo-row"><div class="profile-photo-preview">{move||view!{<Avatar label=state.account.get().label() initials={state.account.get().label().chars().take(1).collect::<String>().to_uppercase()} url=Signal::derive(move||state.account.get().avatar_url) lookup=false/>}}</div>
        <div class="profile-photo-controls"><div class="profile-photo-actions">
        <input autocomplete="off" data-1p-ignore="true" data-lpignore="true" data-form-type="other" node_ref=input id="profile-photo-file" tabindex="-1" class="sr-only" type="file" accept="image/jpeg,image/png,image/webp" aria-label="Choose profile photo" aria-describedby="photo-hint photo-error" disabled=move||busy.get() || !state.loaded.get() on:change=move|e|{
            let target=event_target::<web_sys::HtmlInputElement>(&e);
            let file=target.files().and_then(|files|files.get(0));target.set_value("");
            let Some(file)=file else{return;};
            busy.set(true);error.set(String::new());message.set(String::new());
            spawn_local(async move {let result=upload(file,fixture,state).await;finish(result,"Photo saved.");});
        }/>
        <button type="button" class="button button-secondary" disabled=move||busy.get() || !state.loaded.get() on:click=move|_|{if let Some(input)=input.get(){input.click();}}>{move||if busy.get(){"Saving…"}else if state.account.get().avatar_url.is_some(){"Replace"}else{"Upload photo"}}</button>
        <Show when=move||state.account.get().avatar_url.is_some()><button type="button" class="button button-ghost" disabled=move||busy.get() on:click=move|_|{
            busy.set(true);error.set(String::new());message.set(String::new());
            spawn_local(async move {
                let result=if fixture {let mut account=state.account.get_untracked();if let Some(url)=account.avatar_url.take(){let _=web_sys::Url::revoke_object_url(&url);}Ok(account)}else{api("DELETE","/api/v1/me/avatar",Value::Null).await.and_then(|v|serde_json::from_value(v).map_err(|_|"Could not read account.".into()))};
                finish(result,"Photo removed.");
            });
        }>"Remove"</button></Show></div><p id="photo-hint" class="field-hint">"JPEG, PNG or WebP, up to 5 MB. Automatically cropped to a square."</p></div></div>
        <p id="photo-error" class="settings-error" role="alert">{error}</p><p class="field-hint" role="status">{message}</p>
    </div>}
}
