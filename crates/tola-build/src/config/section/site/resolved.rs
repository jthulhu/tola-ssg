//! Site values after configuration validation, with a canonical deployment mount.

use serde::{Serialize, Serializer};
use tola_address::{SiteOrigin, SiteUrlMount};

use super::{LanguageDeclaration, SiteAuthor, SiteLanguage, SiteSectionConfig};

/// Validated site settings available to builds and configuration readers.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedSiteSection {
    /// Canonical HTTP(S) origin without a deployment path.
    pub origin: Option<String>,
    /// The deployment mount shared by URL rendering and configuration comparisons.
    #[serde(rename = "base-path", serialize_with = "serialize_base_path")]
    pub base_path: SiteUrlMount,
    pub title: String,
    pub authors: Vec<SiteAuthor>,
    pub description: String,
    pub languages: Vec<LanguageDeclaration>,
    pub language: LanguageDeclaration,
}

impl SiteSectionConfig {
    /// Consume the effective settings after all configuration diagnostics have passed.
    pub(in crate::config) fn into_resolved(self) -> ResolvedSiteSection {
        ResolvedSiteSection {
            origin: self.origin.map(|origin| {
                SiteOrigin::parse(&origin)
                    .expect("site origin was validated")
                    .origin()
                    .to_owned()
            }),
            base_path: SiteUrlMount::from_base_path(&self.base_path)
                .expect("site base path was validated"),
            title: self.title,
            authors: self.authors,
            description: self.description,
            languages: self.languages,
            language: self.language,
        }
    }
}

impl ResolvedSiteSection {
    pub(crate) fn language(&self) -> SiteLanguage {
        SiteLanguage::parse(&self.language).expect("the site language was validated")
    }

    pub(crate) fn languages(&self) -> impl Iterator<Item = SiteLanguage> + '_ {
        self.languages
            .iter()
            .map(|language| SiteLanguage::parse(language).expect("languages were validated"))
    }

    pub(crate) fn url(&self) -> Option<String> {
        self.origin
            .as_ref()
            .map(|origin| format!("{origin}{}", self.base_path.base_path()))
    }
}

fn serialize_base_path<S: Serializer>(
    mount: &SiteUrlMount,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&mount.base_path())
}
