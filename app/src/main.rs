//! madblocks Metal Wallet: Bitcoin and BTCVM, Dogecoin and DogecoinVM on
//! Windows, and the bridges between them.
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
#[cfg(test)]
mod tests;
mod update;
mod wallet;
mod winapi;

use btcvm_wallet_core::{Coin, about};
use serde::Serialize;
use std::io::BufRead;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};
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

/// A newer release found, waiting for the user to install it.
#[derive(Default)]
struct Updates(Mutex<Option<update::Manifest>>);

/// A release for the window: its version and notes.
#[derive(Serialize, Clone)]
struct UpdateView {
    version: String,
    notes: update::Notes,
}

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
    if let Err(e) = &result
        && !e.canceled
    {
        journal::warn(&format!("{what}: {}", e.message));
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
    change_title_doge: &'static str,
    change_body_doge: &'static str,
}

fn texts(language: &str) -> Texts {
    if language == "es" {
        Texts {
            backup_title: "Tu copia de seguridad",
            phrase_body: "Estas son las {n} palabras de tu cartera {wallet}:\n\n{words}\n\nApúntalas en papel, en orden, y guárdalas en un sitio seguro. Con ellas recuperas esta cartera aquí, o sus direcciones en otras carteras: la de Bitcoin en cualquiera compatible con BIP39 (Sparrow, Electrum, BlueWallet…) y la de Dogecoin en las que siguen BIP44. Quien las vea puede llevarse tus BTC y tus DOGE: nunca las compartas ni les hagas fotos. Después te pediremos tres de ellas para comprobar la copia.\n\nLa clave de Bitcoin en formato WIF, para la cartera web de BTCVM: {key}",
            backup_body: "Esta es la clave de la cartera {wallet} (WIF):\n\n{key}\n\nEs la clave de sus dos direcciones, la de BTC y la de DOGE. Guárdala en un gestor de contraseñas o escríbela en papel. Quien la vea puede llevarse tus BTC y tus DOGE; nunca la compartas. Sin ella, si pierdes este PC o se restablece Windows Hello, perderás tus fondos.",
            copy: "Copiar la clave",
            copied: "Copiada. Se borrará del portapapeles en un minuto, y Windows no la guarda en el historial del portapapeles (Win+V) ni la sincroniza. Pégala ya en tu gestor de contraseñas.",
            saved: "La he guardado",
            not_yet: "Todavía no",
            remove_title: "Quitar la cartera de este PC",
            remove_body: "Se borrará de este PC la clave de la cartera {wallet} (después de Windows Hello). Si no tienes copia de seguridad, perderás sus BTC y sus DOGE para siempre. ¿Quitarla?",
            remove: "Quitar",
            cancel: "Cancelar",
            change_title: "Han cambiado los firmantes del puente de BTCVM",
            change_body: "El puente de BTCVM informa de un conjunto de firmantes distinto del que esta cartera tiene fijado.\n\nAntes: {old}\nAhora: {new}\n\nAsí se ve una rotación planificada por los operadores de BTCVM, pero también un servidor secuestrado. Los depósitos y retiradas de BTC quedan en pausa hasta que la cartera encuentre el traslado de los fondos firmado por los firmantes antiguos y compruebe sus firmas; lo busca sola cada diez minutos. Los envíos siguen funcionando.\n\nMientras tanto, puedes comprobar el cambio en un explorador de Bitcoin, en metalbtc.com y con madblocks. Tienes los enlaces en la ventana.",
            change_title_doge: "Han cambiado los firmantes del puente de DogecoinVM",
            change_body_doge: "El puente de DogecoinVM informa de un conjunto de firmantes distinto del que esta cartera tiene fijado.\n\nAntes: {old}\nAhora: {new}\n\nAsí se vería un cambio de firmantes hecho por los operadores de DogecoinVM, pero también un servidor secuestrado. Los depósitos y retiradas de DOGE quedan en pausa hasta que una versión nueva de la cartera, firmada por madblocks, incluya los firmantes nuevos. Los envíos siguen funcionando.\n\nMientras tanto, puedes comprobar el cambio en un explorador de Dogecoin, en metaldoge.com y con madblocks. Tienes los enlaces en la ventana.",
        }
    } else {
        Texts {
            backup_title: "Your backup",
            phrase_body: "These are the {n} words of your wallet {wallet}:\n\n{words}\n\nWrite them on paper, in order, and keep them somewhere safe. They restore this wallet here, or its addresses in other wallets: its Bitcoin address in any BIP39 wallet (Sparrow, Electrum, BlueWallet…), and its Dogecoin address in those that follow BIP44. Anyone who sees them can take your BTC and your DOGE: never share them or take photos of them. Next you'll be asked for three of them, to check your backup.\n\nThe Bitcoin key as a WIF, for BTCVM's web wallet: {key}",
            backup_body: "This is the key of wallet {wallet} (WIF):\n\n{key}\n\nIt is the key of both its addresses, BTC's and DOGE's. Keep it in a password manager or write it on paper. Anyone who sees it can take your BTC and your DOGE; never share it. Without it, if you lose this PC or Windows Hello is reset, your funds are gone.",
            copy: "Copy the key",
            copied: "Copied. It will be cleared from the clipboard in a minute, and Windows keeps it out of clipboard history (Win+V) and cloud sync. Paste it into your password manager now.",
            saved: "I've saved it",
            not_yet: "Not yet",
            remove_title: "Remove the wallet from this PC",
            remove_body: "The key of wallet {wallet} will be deleted from this PC (after Windows Hello). Without a backup, its BTC and DOGE are gone for good. Remove it?",
            remove: "Remove",
            cancel: "Cancel",
            change_title: "The signers of BTCVM's bridge have changed",
            change_body: "BTCVM's bridge reports another signer set than the one pinned in this wallet.\n\nBefore: {old}\nNow: {new}\n\nThat is what a planned rotation by BTCVM's operators looks like, and also what a hijacked server would show. BTC deposits and withdrawals are paused until the wallet finds the old signers' signed move of the funds and checks their signatures; it looks for it every ten minutes. Sends still work.\n\nMeanwhile, you can check the change on a Bitcoin explorer, at metalbtc.com and with madblocks. The links are in the window.",
            change_title_doge: "The signers of DogecoinVM's bridge have changed",
            change_body_doge: "DogecoinVM's bridge reports another signer set than the one pinned in this wallet.\n\nBefore: {old}\nNow: {new}\n\nThat is what a change of signers by DogecoinVM's operators would look like, and also what a hijacked server would show. DOGE deposits and withdrawals are paused until a new version of the wallet, signed by madblocks, carries the new signers. Sends still work.\n\nMeanwhile, you can check the change on a Dogecoin explorer, at metaldoge.com and with madblocks. The links are in the window.",
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
        if let Some(window) = app.get_webview_window("main")
            && !window.is_focused().unwrap_or(true)
        {
            let _ = window.request_user_attention(Some(tauri::UserAttentionType::Informational));
        }
    }
    if let Some(change) = w.new_signer_change() {
        let t = texts(&w.language());
        let (title, body) = if change.coin == "doge" {
            (t.change_title_doge, t.change_body_doge)
        } else {
            (t.change_title, t.change_body)
        };
        app.dialog()
            .message(
                body.replace("{old}", &change.trusted_peg)
                    .replace("{new}", &change.reported_peg),
            )
            .title(title)
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
async fn set_coin(coin: String, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    blocking(move || w.set_coin(&coin)).await?
}

/// Unlocks a phrase wallet's DOGE address with Windows Hello, once.
#[tauri::command]
async fn learn_doge_address(w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    logged(
        "showing the DOGE address",
        blocking(move || w.learn_doge_address()).await?,
    )
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
async fn set_check_balances(on: bool, w: State<'_, Shared>) -> Result<View, Failure> {
    let w = w.inner().clone();
    blocking(move || w.set_check_balances(on)).await
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

/// Looks for a newer release now; the one found, if any.
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<UpdateView>, Failure> {
    let found = blocking(update::latest).await?.map_err(failure)?;
    Ok(remember(&app, found))
}

/// The newer release already found, if any.
#[tauri::command]
async fn update_info(updates: State<'_, Updates>) -> Result<Option<UpdateView>, Failure> {
    Ok(updates
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(update_view))
}

#[tauri::command]
async fn set_updates(on: bool, w: State<'_, Shared>) -> Result<View, Failure> {
    Ok(w.set_updates(on))
}

/// Installs the newer release found, then starts it and closes this one.
#[tauri::command]
async fn install_update(app: AppHandle, updates: State<'_, Updates>) -> Result<(), Failure> {
    let manifest = updates
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(|| failure("there is no update to install"))?;
    let exe = logged(
        "installing an update",
        blocking(move || update::install(&manifest))
            .await?
            .map_err(failure),
    )?;
    std::process::Command::new(&exe)
        .arg("--after-update")
        .arg(std::process::id().to_string())
        .spawn()
        .map_err(|e| {
            failure(format!(
                "the new version is in place but didn't start ({e}); open it again"
            ))
        })?;
    app.exit(0);
    Ok(())
}

fn update_view(m: &update::Manifest) -> UpdateView {
    UpdateView {
        version: m.version.clone(),
        notes: m.notes.clone(),
    }
}

/// Keeps a release found and tells the window.
fn remember(app: &AppHandle, found: Option<update::Manifest>) -> Option<UpdateView> {
    let view = found.as_ref().map(update_view);
    if let Some(m) = &found {
        journal::info(&format!("version {} is available", m.version));
    }
    *app.state::<Updates>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = found;
    if let Some(v) = &view {
        let _ = app.emit("update", v);
    }
    view
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

/// Says why the app can't start, in a native box, and exits.
fn fatal(language: &str, why: &str) -> ! {
    journal::error(&format!("can't start: {why}"));
    let (title, text) = if language == "es" {
        (
            "madblocks Metal Wallet no puede arrancar",
            format!(
                "{why}\n\nSi falta Microsoft Edge WebView2 Runtime, instálalo desde https://go.microsoft.com/fwlink/p/?LinkId=2124703 y vuelve a abrir la wallet. Tus carteras no se han tocado."
            ),
        )
    } else {
        (
            "madblocks Metal Wallet can't start",
            format!(
                "{why}\n\nIf Microsoft Edge WebView2 Runtime is missing, install it from https://go.microsoft.com/fwlink/p/?LinkId=2124703 and open the wallet again. Your wallets weren't touched."
            ),
        )
    };
    winapi::error_box(title, &text);
    std::process::exit(1);
}

fn main() {
    // Started by an update: the previous copy closes first, so the single
    // instance check below doesn't take this one for a second copy.
    let args: Vec<String> = std::env::args().collect();
    if let Some(pid) = args
        .iter()
        .position(|a| a == "--after-update")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u32>().ok())
    {
        winapi::wait_for_exit(pid, 15_000);
    }
    if let Ok(dir) = btcvm_wallet_vault::default_dir() {
        journal::init(&dir);
    }
    journal::info(&format!("starting version {}", env!("CARGO_PKG_VERSION")));
    let wallet: Shared = match Wallet::open("en") {
        Ok(w) => Arc::new(w),
        Err(e) => fatal("en", &e),
    };
    let language = wallet.language();
    let app = tauri::Builder::default()
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
        .manage(Updates::default())
        .setup(|app| {
            let w: Shared = app.state::<Shared>().inner().clone();
            let handle = app.handle().clone();
            // Running: the previous version, if an update left it, can go.
            update::clean_up();
            // Newer releases, signed by madblocks: at start, then twice a day.
            if update::configured() {
                let (w, handle) = (w.clone(), handle.clone());
                std::thread::spawn(move || {
                    loop {
                        if w.updates_on() {
                            match update::latest() {
                                Ok(found) => {
                                    remember(&handle, found);
                                }
                                Err(e) => journal::warn(&format!("looking for updates: {e}")),
                            }
                        }
                        std::thread::sleep(Duration::from_secs(12 * 3600));
                    }
                });
            }
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
            // And at once when a block lands on any chain: each coin's bridge
            // streams its own two.
            for coin in Coin::ALL {
                let (w, handle) = (w.clone(), handle.clone());
                std::thread::spawn(move || {
                    loop {
                        if let Ok(stream) = w.events(coin) {
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
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hello,
            view,
            refresh,
            create_wallet,
            select_wallet,
            set_coin,
            learn_doge_address,
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
            set_check_balances,
            check_rotation,
            rotation_seen,
            verify_backup,
            cancel_backup_check,
            set_fiat,
            software_key_seen,
            export_history,
            check_update,
            update_info,
            set_updates,
            install_update,
            about_info,
        ])
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|_, _| {}),
        Err(e) => fatal(&language, &e.to_string()),
    }
}
