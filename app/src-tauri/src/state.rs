use common::api_types::ProjectRevisionInfo;
use std::{collections::HashMap, sync::Mutex};

pub struct ProjectInfoManager {
    project_info: Mutex<HashMap<uuid::Uuid, ProjectRevisionInfo>>,
}

impl ProjectInfoManager {
    pub fn new() -> Self {
        Self {
            project_info: Mutex::new(HashMap::new()),
        }
    }
    pub fn update_project(&self, project_info: &ProjectRevisionInfo) {
        let mut project_lock = self.project_info.lock().unwrap();
        project_lock.insert(project_info.project.id, project_info.clone());
    }

    pub fn get_project(&self, project_id: uuid::Uuid) -> Option<ProjectRevisionInfo> {
        let project_lock = self.project_info.lock().unwrap();
        project_lock.get(&project_id).cloned()
    }

    pub fn clear_projects(&self) {
        let mut project_lock = self.project_info.lock().unwrap();
        project_lock.clear();
    }
}
