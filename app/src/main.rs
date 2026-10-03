//! madblocks BTCVM Wallet: Bitcoin and BTCVM on Windows, and the bridge
//! between them.
//!
//! The window is a web view with no network of its own and no plugins: it
//! calls the commands below and nothing else. Keys stay in Rust. A key shown
//! for backup appears in a native dialog, and an imported one is read from
//! the clipboard here and the clipboard cleared, so neither passes through
//! the web view.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod store;
mod wallet;

use btcvm_wallet_core::about;
use serde::Serialize;
use std::io::BufRead;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;
use wallet::{Failure, Review, Sent, View, Wallet};
use zeroize::Zeroizing;

type Shared = Arc<Wallet>;

/// Runs blocking work (the network, Windows Hello) off the main thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .expect("the task ran")
}

fn failure(message: impl Into<String>) -> Failure {
    Failure {
        message: message.into(),
        untrusted: false,
        canceled: false,
    }
}

// --- native dialogs, in the user's language ----------------------------------

struct Texts {
    backup_title: &'static str,
    backup_body: &'static str,
    saved: &'static str,
    not_yet: &'static str,
    remove_title: &'static str,
    remove_body: &'static str,
    remove: &'static str,
    cancel: &'static str,
    change_title: &'static str,
    change_body: &'static str,
}

fn texts(language: &str) -> Texts {
    if language == "es" {
        Texts {
            backup_title: "Tu clave privada",
            backup_body: "Esta es la clave de tu wallet (WIF):\n\n{key}\n\nGuárdala en un gestor de contraseñas o escríbela en papel. Quien la vea puede llevarse tus BTC; nunca la compartas. Sin ella, si pierdes este PC o se restablece Windows Hello, perderás tus fondos.",
            saved: "La he guardado",
            not_yet: "Todavía no",
            remove_title: "Quitar la wallet de este PC",
            remove_body: "Se borrará la clave de este PC (después de Windows Hello). Si no tienes copia de seguridad, perderás tus BTC para siempre. ¿Quitarla?",
            remove: "Quitar",
            cancel: "Cancelar",
            change_title: "Han cambiado los firmantes del puente",
            change_body: "El puente informa de un conjunto de firmantes distinto del que esta wallet tiene fijado.\n\nAntes: {old}\nAhora: {new}\n\nAsí se ve una rotación planificada por los operadores de BTCVM, pero también un servidor secuestrado. Hasta actualizar la wallet con el nuevo conjunto, los depósitos y retiradas quedan en pausa; los envíos siguen funcionando.\n\nComprueba el cambio en un explorador de Bitcoin, en metalbtc.com y con madblocks. Tienes los enlaces en la ventana.",
        }
    } else {
        Texts {
            backup_title: "Your private key",
            backup_body: "This is your wallet's key (WIF):\n\n{key}\n\nKeep it in a password manager or write it on paper. Anyone who sees it can take your BTC; never share it. Without it, if you lose this PC or Windows Hello is reset, your funds are gone.",
            saved: "I've saved it",
            not_yet: "Not yet",
            remove_title: "Remove the wallet from this PC",
            remove_body: "The key will be deleted from this PC (after Windows Hello). Without a backup, your BTC are gone for good. Remove it?",
            remove: "Remove",
            cancel: "Cancel",
            change_title: "The bridge's signers have changed",
            change_body: "The bridge reports another signer set than the one pinned in this wallet.\n\nBefore: {old}\nNow: {new}\n\nThat is what a planned rotation by BTCVM's operators looks like, and also what a hijacked server would show. Until the wallet is updated with the new set, deposits and withdrawals are paused; sends still work.\n\nCheck the change on a Bitcoin explorer, at metalbtc.com and with madblocks. The links are in the window.",
        }
    }
}

/// Shows the key in a native dialog, outside the web view, and says whether
/// the user saved it.
fn show_backup(app: &AppHandle, language: &str, wif: &str) -> bool {
    let t = texts(language);
    let body = Zeroizing::new(t.backup_body.replace("{key}", wif));
    app.dialog()
        .message(body.as_str())
        .title(t.backup_title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            t.saved.into(),
            t.not_yet.into(),
        ))
        .blocking_show()
}

/// Pushes the latest view to the window, and warns once about a new signer
/// change.
fn publish(app: &AppHandle, w: &Wallet) {
    let _ = app.emit("view", w.view());
    if let Some(change) = w.new_signer_change() {
        let t = texts(&w.language());
        app.dialog()
            .message(
                t.change_body
                    .replace("{old}", &change.trusted_peg)
                    .replace("{new}", &change.reported_peg),
            )
            .title(t.change_title)
            .kind(MessageDialogKind::Warning)
            .show(|_| {});
    }
}

// --- commands ------------------------------------------------------------------

#[tauri::command]
async fn hello(language: String, w: State<'_, Shared>) -> Result<View, Failure> {
    w.adopt_system_language(&language);
    Ok(w.view())
}

#[tauri::command]
async fn view(w: State<'_, Shared>) -> Result<View, Failure> {
    Ok(w.view())
}

#[tauri::command]
async fn refresh(app: AppHandle, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    Ok(blocking(move || {
        w.refresh();
        publish(&app, &w);
        w.view()
    })
    .await)
}

#[tauri::command]
async fn create_wallet(app: AppHandle, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    blocking(move || {
        let language = w.language();
        w.create(|wif| show_backup(&app, &language, wif))
    })
    .await
}

#[tauri::command]
async fn backup(app: AppHandle, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    blocking(move || {
        let language = w.language();
        w.backup(|wif| show_backup(&app, &language, wif))
    })
    .await
}

/// Reads the key from the clipboard here, then clears the clipboard.
fn take_key_from_clipboard(app: &AppHandle) -> Result<Zeroizing<String>, Failure> {
    let text = Zeroizing::new(
        app.clipboard()
            .read_text()
            .map_err(|_| failure("copy your key (WIF) first, then press the button"))?,
    );
    let _ = app.clipboard().clear();
    Ok(text)
}

#[tauri::command]
async fn import_from_clipboard(app: AppHandle, w: State<'_, Shared>) -> Result<View, Failure> {
    let text = take_key_from_clipboard(&app)?;
    let w = w.inner().clone();
    blocking(move || w.import(text)).await
}

#[tauri::command]
async fn restore_from_clipboard(app: AppHandle, w: State<'_, Shared>) -> Result<View, Failure> {
    let text = take_key_from_clipboard(&app)?;
    let w = w.inner().clone();
    blocking(move || w.restore(text)).await
}

#[tauri::command]
async fn remove_wallet(app: AppHandle, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    blocking(move || {
        let t = texts(&w.language());
        w.remove(|| {
            app.dialog()
                .message(t.remove_body)
                .title(t.remove_title)
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    t.remove.into(),
                    t.cancel.into(),
                ))
                .blocking_show()
        })
    })
    .await
}

#[tauri::command]
async fn prepare_send(
    chain: String,
    to: String,
    amount: String,
    w: State<'_, Shared>,
) -> Result<Review, Failure> {
    let w = w.inner().clone();
    blocking(move || w.prepare_send(&chain, &to, &amount)).await
}

#[tauri::command]
async fn prepare_deposit(amount: String, w: State<'_, Shared>) -> Result<Review, Failure> {
    let w = w.inner().clone();
    blocking(move || w.prepare_deposit(&amount)).await
}

#[tauri::command]
async fn prepare_withdrawal(
    to: String,
    amount: String,
    w: State<'_, Shared>,
) -> Result<Review, Failure> {
    let w = w.inner().clone();
    blocking(move || w.prepare_withdrawal(&to, &amount)).await
}

#[tauri::command]
async fn confirm(id: u64, app: AppHandle, w: State<'_, Shared>) -> Result<Sent, Failure> {
    let w = w.inner().clone();
    blocking(move || {
        let sent = w.confirm(id);
        std::thread::sleep(Duration::from_millis(1500));
        w.refresh();
        publish(&app, &w);
        sent
    })
    .await
}

#[tauri::command]
async fn cancel(id: u64, w: State<'_, Shared>) -> Result<(), Failure> {
    w.cancel(id);
    Ok(())
}

#[tauri::command]
async fn set_server(url: String, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    blocking(move || w.set_server(&url)).await
}

#[tauri::command]
async fn set_language(language: String, w: State<'_, Shared>) -> Result<View, Failure> {
    Ok(w.set_language(&language))
}

#[tauri::command]
async fn copy_address(app: AppHandle, w: State<'_, Shared>) -> Result<(), Failure> {
    let address = w
        .view()
        .address
        .ok_or_else(|| failure("back up your key before receiving"))?;
    app.clipboard()
        .write_text(address)
        .map_err(|e| failure(e.to_string()))
}

#[tauri::command]
async fn open_link(
    kind: String,
    arg: String,
    app: AppHandle,
    w: State<'_, Shared>,
) -> Result<(), Failure> {
    let url = w.link(&kind, &arg)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| failure(e.to_string()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct About {
    creator: &'static str,
    role: &'static str,
    producer: &'static str,
    metal_node_id: &'static str,
    version: &'static str,
}

#[tauri::command]
async fn about_info() -> Result<About, Failure> {
    Ok(About {
        creator: about::CREATOR,
        role: about::CREATOR_ROLE,
        producer: about::XPR_PRODUCER,
        metal_node_id: about::METAL_NODE_ID,
        version: env!("CARGO_PKG_VERSION"),
    })
}

fn main() {
    let wallet: Shared = match Wallet::open("en") {
        Ok(w) => Arc::new(w),
        Err(e) => {
            eprintln!("madblocks BTCVM Wallet can't start: {e}");
            std::process::exit(1);
        }
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(wallet)
        .setup(|app| {
            let w: Shared = app.state::<Shared>().inner().clone();
            let handle = app.handle().clone();
            // Refreshes on a timer, whatever the event stream does.
            std::thread::spawn({
                let (w, handle) = (w.clone(), handle.clone());
                move || loop {
                    w.refresh();
                    publish(&handle, &w);
                    std::thread::sleep(Duration::from_secs(30));
                }
            });
            // And at once when a block lands on either chain.
            std::thread::spawn(move || {
                loop {
                    if let Ok(stream) = w.events() {
                        for line in stream.lines() {
                            match line {
                                Ok(l) if l.starts_with("data:") => {
                                    w.refresh();
                                    publish(&handle, &w);
                                }
                                Ok(_) => {}
                                Err(_) => break,
                            }
                        }
                    }
                    std::thread::sleep(Duration::from_secs(3));
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hello,
            view,
            refresh,
            create_wallet,
            backup,
            import_from_clipboard,
            restore_from_clipboard,
            remove_wallet,
            prepare_send,
            prepare_deposit,
            prepare_withdrawal,
            confirm,
            cancel,
            set_server,
            set_language,
            copy_address,
            open_link,
            about_info,
        ])
        .run(tauri::generate_context!())
        .expect("the wallet runs");
}
