pub fn print_startup_help(port: u16) {
    println!(
        r#"
╭────────────────────────────────────────────────────────────────────────╮
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
