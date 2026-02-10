mod dashboard;
mod rules;
mod settings;
mod sidebar;
mod traffic;
mod whitelist;

pub use dashboard::DashboardPanel;
pub use rules::RulesPanel;
pub use settings::SettingsPanel;
pub use sidebar::Sidebar;
pub use traffic::TrafficPanel;
pub use whitelist::WhitelistPanel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    #[default]
    Dashboard,
    Traffic,
    Rules,
    Whitelist,
    Settings,
}

impl Panel {
    pub fn label(&self) -> &'static str {
        match self {
            Panel::Dashboard => "Dashboard",
            Panel::Traffic => "Traffic",
            Panel::Rules => "Rules",
            Panel::Whitelist => "Whitelist",
            Panel::Settings => "Settings",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Panel::Dashboard => "📊",
            Panel::Traffic => "📡",
            Panel::Rules => "📋",
            Panel::Whitelist => "🔒",
            Panel::Settings => "⚙️",
        }
    }
}
