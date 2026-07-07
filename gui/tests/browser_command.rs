use operon_gui::window::menu::{browser_command_parts, BrowserCommand};

#[test]
fn browser_command_matches_the_current_platform() {
    let command = browser_command_parts("https://example.com/docs");

    #[cfg(target_os = "windows")]
    {
        assert_eq!(
            command,
            BrowserCommand {
                program: "cmd".to_string(),
                args: vec![
                    "/C".to_string(),
                    "start".to_string(),
                    String::new(),
                    "https://example.com/docs".to_string(),
                ],
            }
        );
    }

    #[cfg(target_os = "macos")]
    {
        assert_eq!(
            command,
            BrowserCommand {
                program: "open".to_string(),
                args: vec!["https://example.com/docs".to_string()],
            }
        );
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        assert_eq!(
            command,
            BrowserCommand {
                program: "xdg-open".to_string(),
                args: vec!["https://example.com/docs".to_string()],
            }
        );
    }
}
