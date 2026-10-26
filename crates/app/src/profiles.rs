use crate::Form;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tv_manager_core::Numbering;
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub movie: bool,
    pub plex: bool,
    pub jellyfin: bool,
    pub numbering: Numbering,
    pub absolute_start: u32,
    pub companions: bool,
    pub template: String,
}
impl Profile {
    pub fn from_form(name: String, form: &Form) -> Result<Self> {
        let name = name.trim().to_string();
        if name.is_empty() || name.len() > 80 {
            bail!("Enter a profile name of 1–80 characters");
        }
        Ok(Self {
            name,
            movie: form.movie,
            plex: form.plex,
            jellyfin: form.jellyfin,
            numbering: form.options.numbering,
            absolute_start: form.options.absolute_start,
            companions: form.options.companions,
            template: form.template.clone(),
        })
    }
    pub fn apply(&self, form: &mut Form) {
        if form.movie != self.movie {
            form.options.selected_id.clear();
            form.options.poster_override = None;
            form.options.season_overrides.clear();
        }
        form.movie = self.movie;
        form.plex = self.plex;
        form.jellyfin = self.jellyfin;
        form.options.numbering = self.numbering;
        form.options.absolute_start = self.absolute_start;
        form.options.companions = self.companions;
        form.template = self.template.clone();
    }
}
pub fn load() -> Result<Vec<Profile>> {
    let path = crate::session::path()?.with_file_name("naming-profiles.json");
    if !path.exists() {
        return Ok(vec![]);
    }
    if std::fs::metadata(&path)?.len() > 1024 * 1024 {
        bail!("Profiles file is too large");
    }
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}
pub fn save(profiles: &[Profile]) -> Result<()> {
    use std::io::Write;
    if profiles.len() > 50 {
        bail!("Maximum 50 naming profiles");
    }
    let path = crate::session::path()?.with_file_name("naming-profiles.json");
    let directory = path.parent().unwrap();
    std::fs::create_dir_all(directory)?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    file.write_all(&serde_json::to_vec_pretty(profiles)?)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_do_not_store_credentials_or_library_paths() {
        let mut form = Form {
            key: "secret".into(),
            ..Default::default()
        };
        form.options.tmdb_key = "secret".into();
        form.folder = "private".into();
        form.options.numbering = Numbering::Absolute;
        let profile = Profile::from_form("Anime".into(), &form).unwrap();
        let json = serde_json::to_string(&profile).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("private"));
        let mut restored = Form::default();
        profile.apply(&mut restored);
        assert_eq!(restored.options.numbering, Numbering::Absolute);
    }
}
