#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod adblock;
mod app;
mod desktop;
mod extensions;
mod history;
mod import;
mod instance;
mod permission;
mod profile;
mod recovery;
mod session;
mod settings;
mod smoke;
mod start_page;
mod state;
mod ui;
mod updater;

fn has_option(args: &[String], option: &str) -> bool {
    args.iter()
        .skip(1)
        .take_while(|argument| argument.as_str() != "--")
        .any(|argument| argument == option)
}

fn main() -> wry::Result<()> {
    #[cfg(windows)]
    {
        use windows::{Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID, core::w};
        // SAFETY: `w!` is a static, NUL-terminated wide string, and Windows copies the ID during the call.
        // Only taskbar grouping depends on it, so a failure is ignored.
        let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(w!("Veluno.Browser")) };
    }
    let args: Vec<String> = std::env::args().collect();
    if has_option(&args, "--check-update") {
        println!("Current version: {}", updater::CURRENT_VERSION);
        match updater::check_for_updates() {
            Ok(Some(update)) => {
                println!(
                    "Update available: v{} ({})",
                    update.version, update.release_url
                );
                if let Some(url) = update.download_url {
                    println!("Download URL: {url}");
                }
            }
            Ok(None) => println!("Veluno is up to date."),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        std::process::exit(0);
    }

    if let Err(error) = app::run(&args) {
        desktop::message(
            "Veluno could not start",
            &format!(
                "{error}\n\nMake sure Microsoft Edge WebView2 Runtime is installed, then try again."
            ),
            false,
        );
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn external_link_arguments_cannot_trigger_command_options() {
        let arguments = |values: &[&str]| {
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        };
        assert!(super::has_option(
            &arguments(&["veluno.exe", "--check-update"]),
            "--check-update"
        ));
        assert!(!super::has_option(
            &arguments(&["veluno.exe", "--", "--check-update"]),
            "--check-update"
        ));
        assert!(!super::has_option(
            &arguments(&["veluno.exe", "--", "https://example.com/?x=--check-update"]),
            "--check-update"
        ));
    }
}
