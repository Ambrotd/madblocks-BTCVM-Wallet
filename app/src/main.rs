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
mod clip;
mod journal;
mod store;
mod wallet;

use btcvm_wallet_core::about;
use serde::Serialize;
use std::io::BufRead;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{
    DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
};
use tauri_plugin_opener::OpenerExt;
use wallet::{BackupText, Failure, Review, Sent, View, Wallet};
use zeroize::Zeroizing;

type Shared = Arc<Wallet>;

/// Runs blocking work (the network, Windows Hello) off the main thread. A
/// panic there comes back as a failure, logged, instead of a call that never
/// answers.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, Failure> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| failure(INTERNAL))
}

const INTERNAL: &str = "something went wrong inside the wallet; nothing was signed or sent. The details are in the log (Settings)";

/// Logs a failure on its way to the window. Canceling Windows Hello isn't one.
fn logged<T>(what: &str, result: Result<T, Failure>) -> Result<T, Failure> {
    if let Err(e) = &result {
        if !e.canceled {
            journal::warn(&format!("{what}: {}", e.message));
        }
    }
    result
}

/// Runs one pass of a background loop; a panic in it is logged (by the
/// hook) and the loop goes on.
fn guarded(f: impl FnOnce()) {
    let _ = catch_unwind(AssertUnwindSafe(f));
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
    phrase_body: &'static str,
    copy: &'static str,
    copied: &'static str,
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
            backup_title: "Tu copia de seguridad",
            phrase_body: "Estas son las {n} palabras de tu cartera {wallet}:\n\n{words}\n\nApúntalas en papel, en orden, y guárdalas en un sitio seguro. Con ellas recuperas esta cartera aquí, o su dirección de Bitcoin en cualquier wallet compatible con BIP39 (Sparrow, Electrum, BlueWallet…). Quien las vea puede llevarse tus BTC: nunca las compartas ni les hagas fotos. Después te pediremos tres de ellas para comprobar la copia.\n\nLa misma clave en formato WIF, para la wallet web de BTCVM: {key}",
            backup_body: "Esta es la clave de la cartera {wallet} (WIF):\n\n{key}\n\nGuárdala en un gestor de contraseñas o escríbela en papel. Quien la vea puede llevarse tus BTC; nunca la compartas. Sin ella, si pierdes este PC o se restablece Windows Hello, perderás tus fondos.",
            copy: "Copiar la clave",
            copied: "Copiada. Se borrará del portapapeles en un minuto, y Windows no la guarda en el historial del portapapeles (Win+V) ni la sincroniza. Pégala ya en tu gestor de contraseñas.",
            saved: "La he guardado",
            not_yet: "Todavía no",
            remove_title: "Quitar la cartera de este PC",
            remove_body: "Se borrará de este PC la clave de la cartera {wallet} (después de Windows Hello). Si no tienes copia de seguridad, perderás sus BTC para siempre. ¿Quitarla?",
            remove: "Quitar",
            cancel: "Cancelar",
            change_title: "Han cambiado los firmantes del puente",
            change_body: "El puente informa de un conjunto de firmantes distinto del que esta wallet tiene fijado.\n\nAntes: {old}\nAhora: {new}\n\nAsí se ve una rotación planificada por los operadores de BTCVM, pero también un servidor secuestrado. Los depósitos y retiradas quedan en pausa hasta que la wallet encuentre el traslado de los fondos firmado por los firmantes antiguos y compruebe sus firmas; lo busca sola cada diez minutos. Los envíos siguen funcionando.\n\nMientras tanto, puedes comprobar el cambio en un explorador de Bitcoin, en metalbtc.com y con madblocks. Tienes los enlaces en la ventana.",
        }
    } else {
        Texts {
            backup_title: "Your backup",
            phrase_body: "These are the {n} words of your wallet {wallet}:\n\n{words}\n\nWrite them on paper, in order, and keep them somewhere safe. They restore this wallet here, or its Bitcoin address in any BIP39 wallet (Sparrow, Electrum, BlueWallet…). Anyone who sees them can take your BTC: never share them or take photos of them. Next you'll be asked for three of them, to check your backup.\n\nThe same key as a WIF, for BTCVM's web wallet: {key}",
            backup_body: "This is the key of wallet {wallet} (WIF):\n\n{key}\n\nKeep it in a password manager or write it on paper. Anyone who sees it can take your BTC; never share it. Without it, if you lose this PC or Windows Hello is reset, your funds are gone.",
            copy: "Copy the key",
            copied: "Copied. It will be cleared from the clipboard in a minute, and Windows keeps it out of clipboard history (Win+V) and cloud sync. Paste it into your password manager now.",
            saved: "I've saved it",
            not_yet: "Not yet",
            remove_title: "Remove the wallet from this PC",
            remove_body: "The key of wallet {wallet} will be deleted from this PC (after Windows Hello). Without a backup, its BTC are gone for good. Remove it?",
            remove: "Remove",
            cancel: "Cancel",
            change_title: "The bridge's signers have changed",
            change_body: "The bridge reports another signer set than the one pinned in this wallet.\n\nBefore: {old}\nNow: {new}\n\nThat is what a planned rotation by BTCVM's operators looks like, and also what a hijacked server would show. Deposits and withdrawals are paused until the wallet finds the old signers' signed move of the funds and checks their signatures; it looks for it every ten minutes. Sends still work.\n\nMeanwhile, you can check the change on a Bitcoin explorer, at metalbtc.com and with madblocks. The links are in the window.",
        }
    }
}

/// The words numbered, four to a line.
fn numbered(words: &str) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::new());
    for (i, word) in words.split(' ').enumerate() {
        if i > 0 {
            out.push_str(if i % 4 == 0 { "\n" } else { "     " });
        }
        out.push_str(&format!("{}. {word}", i + 1));
    }
    out
}

/// Shows the backup of `wallet` (its name and address) in a native dialog,
/// outside the web view, and says whether the user saved it: the recovery
/// phrase when it has one, else the key. "Copy" puts it on the clipboard the
/// way password managers do (see `clip`) and shows the dialog again.
fn show_backup(app: &AppHandle, language: &str, wallet: &str, backup: &BackupText) -> bool {
    let t = texts(language);
    let mut note = "";
    let text = match &backup.words {
        Some(words) => Zeroizing::new(
            t.phrase_body
                .replace("{n}", &words.split(' ').count().to_string())
                .replace("{wallet}", wallet)
                .replace("{words}", &numbered(words))
                .replace("{key}", &backup.wif),
        ),
        None => Zeroizing::new(
            t.backup_body
                .replace("{wallet}", wallet)
                .replace("{key}", &backup.wif),
        ),
    };
    let copy: &str = backup.words.as_deref().map_or(&backup.wif, |w| w.as_str());
    loop {
        let body = Zeroizing::new(format!(
            "{}{}{note}",
            text.as_str(),
            if note.is_empty() {
                ""
            } else {
                "

"
            }
        ));
        let choice = app
            .dialog()
            .message(body.as_str())
            .title(t.backup_title)
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::YesNoCancelCustom(
                t.copy.into(),
                t.saved.into(),
                t.not_yet.into(),
            ))
            .blocking_show_with_result();
        match choice {
            MessageDialogResult::Yes => {}
            MessageDialogResult::Custom(label) if label == t.copy => {}
            MessageDialogResult::No => return true,
            MessageDialogResult::Custom(label) if label == t.saved => return true,
            _ => return false,
        }
        note = match clip::copy_secret(copy) {
            Ok(()) => t.copied,
            Err(_) => "",
        };
    }
}

/// Pushes the latest view to the window, announces payments received
/// (flashing the taskbar button when the window isn't in front), and warns
/// once about a new signer change.
fn publish(app: &AppHandle, w: &Wallet) {
    let _ = app.emit("view", w.view());
    let notices = w.take_notices();
    if !notices.is_empty() {
        for n in &notices {
            let _ = app.emit("notice", n);
        }
        if let Some(window) = app.get_webview_window("main") {
            if !window.is_focused().unwrap_or(true) {
                let _ =
                    window.request_user_attention(Some(tauri::UserAttentionType::Informational));
            }
        }
    }
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
    blocking(move || {
        w.refresh();
        publish(&app, &w);
        w.view()
    })
    .await
}

#[tauri::command]
async fn create_wallet(
    name: String,
    app: AppHandle,
    w: State<'_, Shared>,
) -> Result<View, Failure> {
    let w = w.inner().clone();
    logged(
        "creating a wallet",
        blocking(move || {
            let language = w.language();
            w.create(&name, |backup| {
                show_backup(&app, &language, &w.active_label(), backup)
            })
        })
        .await?,
    )
}

#[tauri::command]
async fn backup(app: AppHandle, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    logged(
        "backing up",
        blocking(move || {
            let language = w.language();
            w.backup(|backup| show_backup(&app, &language, &w.active_label(), backup))
        })
        .await?,
    )
}

/// Reads the key from the clipboard here, then clears the clipboard.
fn take_key_from_clipboard(app: &AppHandle) -> Result<Zeroizing<String>, Failure> {
    let text = Zeroizing::new(app.clipboard().read_text().map_err(|_| {
        failure("copy your recovery phrase or your key (WIF) first, then press the button")
    })?);
    let _ = app.clipboard().clear();
    Ok(text)
}

#[tauri::command]
async fn import_from_clipboard(
    name: String,
    app: AppHandle,
    w: State<'_, Shared>,
) -> Result<View, Failure> {
    let text = take_key_from_clipboard(&app)?;
    let w = w.inner().clone();
    logged(
        "importing a key",
        blocking(move || w.import(&name, text)).await?,
    )
}

#[tauri::command]
async fn restore_from_clipboard(app: AppHandle, w: State<'_, Shared>) -> Result<View, Failure> {
    let text = take_key_from_clipboard(&app)?;
    let w = w.inner().clone();
    logged("restoring", blocking(move || w.restore(text)).await?)
}

#[tauri::command]
async fn remove_wallet(app: AppHandle, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    let removed = blocking(move || {
        let t = texts(&w.language());
        let wallet = w.active_label();
        w.remove(|| {
            app.dialog()
                .message(t.remove_body.replace("{wallet}", &wallet))
                .title(t.remove_title)
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    t.remove.into(),
                    t.cancel.into(),
                ))
                .blocking_show()
        })
    })
    .await?;
    logged("removing a wallet", removed)
}

#[tauri::command]
async fn select_wallet(id: String, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    logged("switching wallets", blocking(move || w.select(&id)).await?)
}

#[tauri::command]
async fn rename_wallet(id: String, name: String, w: State<'_, Shared>) -> Result<View, Failure> {
    w.rename(&id, &name)
}

#[tauri::command]
async fn add_contact(
    name: String,
    address: String,
    chain: String,
    w: State<'_, Shared>,
) -> Result<View, Failure> {
    w.add_contact(&name, &address, &chain)
}

#[tauri::command]
async fn rename_contact(
    address: String,
    chain: String,
    name: String,
    w: State<'_, Shared>,
) -> Result<View, Failure> {
    w.rename_contact(&address, &chain, &name)
}

#[tauri::command]
async fn remove_contact(
    address: String,
    chain: String,
    w: State<'_, Shared>,
) -> Result<View, Failure> {
    w.remove_contact(&address, &chain)
}

#[tauri::command]
async fn fee_options(w: State<'_, Shared>) -> Result<wallet::FeeOptions, Failure> {
    let w = w.inner().clone();
    blocking(move || w.fee_options()).await
}

#[tauri::command]
async fn prepare_send(
    chain: String,
    to: String,
    amount: String,
    fee_rate: Option<u64>,
    w: State<'_, Shared>,
) -> Result<Review, Failure> {
    let w = w.inner().clone();
    logged(
        "preparing a payment",
        blocking(move || w.prepare_send(&chain, &to, &amount, fee_rate)).await?,
    )
}

#[tauri::command]
async fn prepare_deposit(
    amount: String,
    fee_rate: Option<u64>,
    w: State<'_, Shared>,
) -> Result<Review, Failure> {
    let w = w.inner().clone();
    logged(
        "preparing a deposit",
        blocking(move || w.prepare_deposit(&amount, fee_rate)).await?,
    )
}

#[tauri::command]
async fn prepare_withdrawal(
    to: String,
    amount: String,
    w: State<'_, Shared>,
) -> Result<Review, Failure> {
    let w = w.inner().clone();
    logged(
        "preparing a withdrawal",
        blocking(move || w.prepare_withdrawal(&to, &amount)).await?,
    )
}

#[tauri::command]
async fn prepare_bump(
    txid: String,
    fee_rate: u64,
    w: State<'_, Shared>,
) -> Result<Review, Failure> {
    let w = w.inner().clone();
    logged(
        "preparing a higher fee",
        blocking(move || w.prepare_bump(&txid, fee_rate)).await?,
    )
}

#[tauri::command]
async fn max_amount(
    action: String,
    chain: String,
    to: String,
    fee_rate: Option<u64>,
    w: State<'_, Shared>,
) -> Result<String, Failure> {
    let w = w.inner().clone();
    blocking(move || w.max_amount(&action, &chain, &to, fee_rate)).await?
}

#[tauri::command]
async fn confirm(id: u64, app: AppHandle, w: State<'_, Shared>) -> Result<Sent, Failure> {
    let w = w.inner().clone();
    logged(
        "signing",
        blocking(move || {
            let sent = w.confirm(id);
            std::thread::sleep(Duration::from_millis(1500));
            w.refresh();
            publish(&app, &w);
            sent
        })
        .await?,
    )
}

#[tauri::command]
async fn cancel(id: u64, w: State<'_, Shared>) -> Result<(), Failure> {
    w.cancel(id);
    Ok(())
}

#[tauri::command]
async fn set_server(url: String, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    logged(
        "changing the bridge",
        blocking(move || w.set_server(&url)).await?,
    )
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

#[tauri::command]
async fn check_rotation(w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    blocking(move || w.check_rotation_now()).await
}

#[tauri::command]
async fn rotation_seen(w: State<'_, Shared>) -> Result<View, Failure> {
    Ok(w.rotation_seen())
}

#[tauri::command]
async fn verify_backup(answers: Vec<String>, w: State<'_, Shared>) -> Result<View, Failure> {
    logged("checking a backup", w.verify_backup(&answers))
}

#[tauri::command]
async fn cancel_backup_check(w: State<'_, Shared>) -> Result<View, Failure> {
    Ok(w.cancel_backup_check())
}

#[tauri::command]
async fn receive_qr(w: State<'_, Shared>) -> Result<wallet::Qr, Failure> {
    w.receive_qr()
}

#[tauri::command]
async fn set_fiat(currency: String, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    blocking(move || w.set_fiat(&currency)).await?
}

#[tauri::command]
async fn software_key_seen(w: State<'_, Shared>) -> Result<View, Failure> {
    Ok(w.software_key_seen())
}

/// Saves the active wallet's history as CSV where the user picks. False
/// when they cancel.
#[tauri::command]
async fn export_history(app: AppHandle, w: State<'_, Shared>) -> Result<bool, Failure> {
    let w = w.inner().clone();
    let saved = blocking(move || {
        let (name, csv) = w.history_csv()?;
        let Some(path) = app
            .dialog()
            .file()
            .set_file_name(name)
            .add_filter("CSV", &["csv"])
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let path = path.into_path().map_err(|e| failure(e.to_string()))?;
        std::fs::write(&path, csv).map_err(|e| failure(e.to_string()))?;
        journal::info(&format!("exported the history to {}", path.display()));
        Ok(true)
    })
    .await?;
    logged("exporting the history", saved)
}

/// Opens the folder with the log, for sending it to support.
#[tauri::command]
async fn open_logs(app: AppHandle) -> Result<(), Failure> {
    let dir = journal::dir().ok_or_else(|| failure("there is no log yet"))?;
    std::fs::create_dir_all(dir).map_err(|e| failure(e.to_string()))?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
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
    if let Ok(dir) = btcvm_wallet_vault::default_dir() {
        journal::init(&dir);
    }
    journal::info(&format!("starting version {}", env!("CARGO_PKG_VERSION")));
    let wallet: Shared = match Wallet::open("en") {
        Ok(w) => Arc::new(w),
        Err(e) => {
            journal::error(&format!("can't start: {e}"));
            eprintln!("madblocks BTCVM Wallet can't start: {e}");
            std::process::exit(1);
        }
    };
    tauri::Builder::default()
        // First: a second copy only brings this window forward. Two copies
        // would each save over the other's records, and could spend the
        // same coins twice.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
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
                    guarded(|| {
                        w.refresh();
                        publish(&handle, &w);
                    });
                    std::thread::sleep(Duration::from_secs(30));
                }
            });
            // And at once when a block lands on either chain.
            std::thread::spawn(move || {
                loop {
                    if let Ok(stream) = w.events() {
                        for line in stream.lines() {
                            match line {
                                Ok(l) if l.starts_with("data:") => guarded(|| {
                                    w.refresh();
                                    publish(&handle, &w);
                                }),
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
            select_wallet,
            rename_wallet,
            add_contact,
            rename_contact,
            remove_contact,
            fee_options,
            backup,
            import_from_clipboard,
            restore_from_clipboard,
            remove_wallet,
            prepare_send,
            prepare_deposit,
            prepare_withdrawal,
            prepare_bump,
            max_amount,
            confirm,
            cancel,
            set_server,
            set_language,
            copy_address,
            open_link,
            open_logs,
            receive_qr,
            check_rotation,
            rotation_seen,
            verify_backup,
            cancel_backup_check,
            set_fiat,
            software_key_seen,
            export_history,
            about_info,
        ])
        .run(tauri::generate_context!())
        .expect("the wallet runs");
}
