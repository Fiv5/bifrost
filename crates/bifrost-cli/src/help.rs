const BIFROST_BANNER: &str = r#"
    ____  _ ____                __
   / __ )(_) __/_________  ____/ /_
  / __  / / /_/ ___/ __ \/ ___/ __/
 / /_/ / / __/ /  / /_/ (__  ) /_
/_____/_/_/ /_/   \____/____/\__/

"#;

pub fn print_banner() {
    if supports_color() {
        print_rainbow_banner();
    } else {
        print!("{}", BIFROST_BANNER);
    }
}

fn print_rainbow_banner() {
    const ESC: &str = "\x1b[";
    const RESET: &str = "\x1b[0m";

    println!();
    println!(
        "{}38;5;196m    ____  _ ____                __{}",
        ESC, RESET
    );
    println!(
        "{}38;5;208m   / __ )(_) __/_________  ____/ /_{}",
        ESC, RESET
    );
    println!(
        "{}38;5;226m  / __  / / /_/ ___/ __ \\/ ___/ __/{}",
        ESC, RESET
    );
    println!("{}38;5;46m / /_/ / / __/ /  / /_/ (__  ) /_{}", ESC, RESET);
    println!(
        "{}38;5;21m/_____/_/_/ /_/   \\____/____/\\__/{}",
        ESC, RESET
    );
    println!();
}

fn supports_color() -> bool {
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    if std::env::var("FORCE_COLOR").is_ok() {
        return true;
    }
    #[cfg(unix)]
    {
        if let Ok(term) = std::env::var("TERM") {
            if term == "dumb" {
                return false;
            }
        }
        unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        let handle = std::io::stdout().as_raw_handle();
        let mut mode: u32 = 0;
        unsafe {
            let result = windows_sys::Win32::System::Console::GetConsoleMode(
                handle as windows_sys::Win32::Foundation::HANDLE,
                &mut mode,
            );
            result != 0
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

pub fn print_startup_help(port: u16) {
    print_banner();
    println!(
        r#"╭────────────────────────────────────────────────────────────────────────╮
│                       BIFROST PROXY COMMANDS                           │
╰────────────────────────────────────────────────────────────────────────╯

PROXY CONTROL
  bifrost status                    Show proxy status
  bifrost stop                      Stop the running proxy

RULE MANAGEMENT
  bifrost rule list                 List all rules
  bifrost rule add <name>           Add a new rule
    -c, --content <CONTENT>           Rule content (inline)
    -f, --file <FILE>                 Rule file path
  bifrost rule enable <name>        Enable a rule
  bifrost rule disable <name>       Disable a rule
  bifrost rule show <name>          Show rule content
  bifrost rule delete <name>        Delete a rule

CA CERTIFICATE
  bifrost ca info                   Show CA certificate info
  bifrost ca export                 Export CA certificate
    -o, --output <PATH>               Output file path
  bifrost ca generate               (Re)generate CA certificate
    -f, --force                       Force regenerate

SYSTEM PROXY
  bifrost system-proxy status       Show system proxy status
  bifrost system-proxy enable       Enable system proxy
    --host <HOST>                     Proxy host (default: 127.0.0.1)
    --port <PORT>                     Proxy port
    --bypass <LIST>                   Bypass list (comma-separated)
  bifrost system-proxy disable      Disable system proxy

VALUES (Variable Expansion)
  bifrost value list                List all values
  bifrost value get <name>          Get a value
  bifrost value set <name> <value>  Set a value
  bifrost value delete <name>       Delete a value
  bifrost value import <file>       Import from file (.txt/.kv/.json)

ACCESS CONTROL
  bifrost whitelist list            List whitelist entries
  bifrost whitelist add <ip>        Add IP/CIDR to whitelist
  bifrost whitelist remove <ip>     Remove IP/CIDR from whitelist
  bifrost whitelist allow-lan <bool> Enable/disable LAN access
  bifrost whitelist status          Show access control settings

TLS INTERCEPTION CONTROL
  Start options:
    --no-intercept                    Disable TLS/HTTPS interception completely
    --intercept-exclude <DOMAINS>     Domains to skip interception (comma-separated)
    --intercept-include <DOMAINS>     Force intercept domains (highest priority, comma-separated)
    --unsafe-ssl                      Skip upstream TLS cert verification (dangerous)
    --no-disconnect-on-config-change  Disable auto-disconnect when TLS config changes

  Rule-based TLS control (highest priority):
    example.com tlsIntercept://       Force TLS interception for matching domain
    example.com tlsPassthrough://     Force TLS passthrough for matching domain

  TLS Interception Priority (highest to lowest):
    1. Rule-based (tlsIntercept://, tlsPassthrough://)
    2. --intercept-include: Always intercept matched domains
    3. --intercept-exclude: Never intercept matched domains
    4. --no-intercept flag: Global switch (default: enabled)

ADMIN UI
  http://127.0.0.1:{port}/          Web-based admin interface

Use 'bifrost <command> --help' for more details."#,
        port = port
    );
    println!();
}
