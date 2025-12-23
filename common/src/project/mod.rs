mod map;
mod revision;

pub use map::{CompassProject, Project, SpeleoDb};
pub use revision::SpeleoDbProjectRevision;

pub struct ProjectData {
    pub project: Project,
    pub compass_project: CompassProject,
    pub speleodb_revision: Option<SpeleoDbProjectRevision>,
}
