pub mod ca;
pub mod cache;
pub mod config;
pub mod dynamic;
pub mod install;
pub mod sni;

pub use ca::{
    generate_root_ca, load_root_ca, parse_cert_info, save_root_ca, CertInfo, CertificateAuthority,
};
pub use cache::CertCache;
pub use config::TlsConfig;
pub use dynamic::DynamicCertGenerator;
pub use install::{get_platform_name, CertInstaller, CertStatus, CertSystemInfo};
pub use sni::{build_sni_server_config, SniResolver};

pub use rustls;
pub use rustls::pki_types;
pub use rustls::sign::CertifiedKey;

pub fn init_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}
