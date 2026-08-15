pub trait GitProvider {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
}

pub struct GitHubProvider;
pub struct GitLabProvider;
pub struct CodebergProvider;

impl GitProvider for GitHubProvider {
    fn id(&self) -> &'static str { "github" }
    fn display_name(&self) -> &'static str { "GitHub" }
}

impl GitProvider for GitLabProvider {
    fn id(&self) -> &'static str { "gitlab" }
    fn display_name(&self) -> &'static str { "GitLab" }
}

impl GitProvider for CodebergProvider {
    fn id(&self) -> &'static str { "codeberg" }
    fn display_name(&self) -> &'static str { "Codeberg" }
}
