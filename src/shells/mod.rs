use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl Shell {
    pub fn keys() -> &'static [Shell] {
        return &[Shell::Bash, Shell::Zsh, Shell::Fish];
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
        }
    }
}
