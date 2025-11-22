use speleodb_compass_common::{CompassProject, Project};

pub fn main() {
    let sample_project = CompassProject {
        speleodb: Default::default(),
        project: Project::new(
            "Sample Project",
            "This is a sample SpeleoDB Compass project.",
            "project.mak".to_string(),
            vec![
                "data/file1.dat".to_string(),
                "data/file2.dat".to_string(),
                "data/file3.dat".to_string(),
            ],
            vec![],
        ),
    };
    let sample = toml::to_string_pretty(&sample_project).unwrap();
    std::fs::write("compass.toml", sample).unwrap();
}
