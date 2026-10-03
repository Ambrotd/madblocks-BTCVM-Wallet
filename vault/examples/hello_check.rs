//! Checks Windows Hello works for the vault on this PC. It makes a
//! throwaway Windows Hello key, has it sign the same challenge twice, checks
//! both signatures verify and are identical, and deletes the key. Windows
//! Hello asks three times: to make the key, and for each signature.
//!
//!     cargo run -p btcvm-wallet-vault --example hello_check

#[cfg(windows)]
fn main() {
    use btcvm_wallet_vault::{Gate, WindowsHello};

    let step = |what: &str| println!("· {what}");
    match WindowsHello::is_supported() {
        Ok(true) => step("Windows Hello is set up"),
        Ok(false) => {
            return println!(
                "✗ Windows Hello isn't set up: add a PIN in Settings > Accounts > Sign-in options"
            );
        }
        Err(e) => return println!("✗ {e}"),
    }
    let name = "madblocks-btcvm-wallet/hello-check";
    let hello = WindowsHello;
    let _ = hello.delete(name); // from an earlier, interrupted run
    let check = || -> Result<(), btcvm_wallet_vault::VaultError> {
        step("making a throwaway Windows Hello key (Windows Hello asks)");
        let public_key = hello.create(name)?;
        step(&format!("its public key is {} bytes", public_key.len()));
        let challenge = b"madblocks BTCVM Wallet: hello check";
        step("signing a challenge (Windows Hello asks)");
        let first = hello.sign(name, challenge, &public_key)?;
        step("signing it again (Windows Hello asks)");
        let second = hello.sign(name, challenge, &public_key)?;
        if first != second {
            println!("✗ the two signatures differ: the vault can't use Windows Hello on this PC");
        } else {
            println!(
                "✓ both signatures verify and are identical ({} bytes): the vault can use Windows Hello on this PC",
                first.len()
            );
        }
        Ok(())
    };
    let result = check();
    match hello.delete(name) {
        Ok(()) => step("deleted the throwaway key"),
        Err(e) => println!("! couldn't delete the throwaway key: {e}"),
    }
    if let Err(e) = result {
        println!("✗ {e}");
    }
}

#[cfg(not(windows))]
fn main() {
    println!("Windows Hello exists only on Windows.");
}
