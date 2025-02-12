use crate::models::{Project, Sentence};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::api::path::app_local_data_dir;

#[derive(Serialize, Deserialize)]
pub struct RecentProject {
    pub path: String,
    pub name: String,
    pub last_accessed: String, // ISO timestamp
}

#[derive(Serialize, Deserialize)]
pub struct RecentProjectsData {
    pub app_version: String,
    pub recent_projects: Vec<RecentProject>,
}

#[tauri::command]
pub async fn import_sentences(file_path: &str, project_dir: &str) -> Result<Vec<Sentence>, String> {
    // 1. Read the file contents
    let file_contents =
        fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;

    // 2. Parse the sentences based on file extension
    let sentences = match Path::new(file_path).extension().and_then(OsStr::to_str) {
        Some("txt") => parse_txt(&file_contents),
        Some("csv") => parse_delimited(&file_contents, b',')?,
        Some("tsv") => parse_delimited(&file_contents, b'\t')?,
        _ => return Err("Unsupported file format".into()),
    };

    // 3. Construct the full audio file path for each sentence
    let sentences_with_paths: Vec<Sentence> = sentences
        .into_iter()
        .enumerate()
        .map(|(index, sentence)| {
            let audio_file_name = format!("{}.wav", sentence.text);
            let audio_file_path = Path::new(project_dir)
                .join(audio_file_name)
                .to_string_lossy()
                .to_string();
            Sentence {
                id: (index + 1),
                text: sentence.text,
                recorded: false,
                audio_file_path: Some(audio_file_path),
            }
        })
        .collect();

    Ok(sentences_with_paths)
}

fn parse_txt(file_contents: &str) -> Vec<Sentence> {
    file_contents
        .lines()
        .enumerate()
        .map(|(index, line)| Sentence {
            id: (index + 1),
            text: line.trim().to_string(),
            recorded: false,
            audio_file_path: None,
        })
        .collect()
}

// Function to parse both CSV and TSV with a configurable delimiter
fn parse_delimited(file_contents: &str, delimiter: u8) -> Result<Vec<Sentence>, String> {
    let mut rdr = ReaderBuilder::new()
        .delimiter(delimiter)
        .from_reader(file_contents.as_bytes());
    let mut sentences = Vec::new();

    for (index, result) in rdr.records().enumerate() {
        let record = result.map_err(|e| format!("Failed to parse: {}", e))?;
        if let Some(text) = record.get(0) {
            sentences.push(Sentence {
                id: (index + 1),
                text: text.to_string(),
                recorded: false,
                audio_file_path: None,
            });
        }
    }

    Ok(sentences)
}

#[tauri::command]
pub fn create_new_project(parent_dir: &str, mut project: Project) -> Result<Project, String> {
    let project_path = Path::new(parent_dir).join(&project.metadata.name);
    fs::create_dir_all(&project_path).map_err(|e| e.to_string())?;

    project.metadata.directory = project_path.to_string_lossy().to_string();

    let json_path = project_path.join(format!("{}.json", project.metadata.name));
    let project_data = serde_json::to_string_pretty(&project).unwrap();
    fs::write(json_path, project_data).map_err(|e| e.to_string())?;

    Ok(project)
}

#[tauri::command]
pub fn open_project(file_path: &str) -> Result<Project, String> {
    let content = fs::read_to_string(file_path).map_err(|e| e.to_string())?;
    let project: Project = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(project)
}

#[tauri::command]
pub fn save_project(project: Project) -> Result<Project, String> {
    let file_path =
        Path::new(&project.metadata.directory).join(format!("{}.json", project.metadata.name));
    let project_data = serde_json::to_string_pretty(&project).unwrap();
    fs::write(file_path, project_data).map_err(|e| e.to_string())?;
    Ok(project)
}

fn get_recent_projects_path() -> PathBuf {
    let mut path =
        app_local_data_dir(&tauri::Config::default()).expect("Failed to get app local data dir");
    path.push("recordr");
    fs::create_dir_all(&path).expect("Failed to create recordr directory");
    path.push("recent_projects.json");
    path
}

fn save_recent_projects_data(data: &RecentProjectsData) {
    let json = serde_json::to_string(data).unwrap();
    let path = get_recent_projects_path();
    // The directory has already been created in get_recent_projects_path.
    fs::write(path, json).unwrap();
}

#[tauri::command]
pub fn get_recent_projects() -> RecentProjectsData {
    let path = get_recent_projects_path();
    if path.exists() {
        let contents = fs::read_to_string(&path).unwrap();
        let mut data: RecentProjectsData =
            serde_json::from_str(&contents).unwrap_or_else(|_| RecentProjectsData {
                app_version: "unknown".into(),
                recent_projects: vec![],
            });
        data.recent_projects
            .sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));
        data.recent_projects.truncate(10);
        data
    } else {
        RecentProjectsData {
            app_version: "unknown".into(),
            recent_projects: vec![],
        }
    }
}

#[tauri::command]
pub fn add_recent_project(new_project: RecentProject, app_version: String) -> RecentProjectsData {
    let mut data = get_recent_projects();
    // Update the top-level app version.
    data.app_version = app_version;
    if let Some(existing) = data
        .recent_projects
        .iter_mut()
        .find(|proj| proj.path == new_project.path)
    {
        existing.last_accessed = new_project.last_accessed.clone();
        existing.name = new_project.name.clone();
    } else {
        data.recent_projects.push(new_project);
    }
    data.recent_projects
        .sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));
    data.recent_projects.truncate(10);
    save_recent_projects_data(&data);
    data
}
