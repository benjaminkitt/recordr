use crate::models::Sentence;
use csv::ReaderBuilder;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

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

    Ok(sentences) // Add this line to return the sentences
}

#[tauri::command]
pub fn create_new_project(parent_dir: &str, project_name: &str) -> Result<bool, String> {
    let project_path = Path::new(parent_dir).join(project_name);
    fs::create_dir_all(&project_path).map_err(|e| e.to_string())?;

    let json_path = project_path.join(format!("{}.json", project_name));
    let initial_data = serde_json::json!({ "sentences": [] });
    fs::write(
        json_path,
        serde_json::to_string_pretty(&initial_data).unwrap(),
    )
    .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
pub fn open_project(file_path: &str) -> Result<Vec<Sentence>, String> {
    let content = fs::read_to_string(file_path).map_err(|e| e.to_string())?;
    let data: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let sentences: Vec<Sentence> =
        serde_json::from_value(data["sentences"].clone()).map_err(|e| e.to_string())?;
    Ok(sentences)
}

#[tauri::command]
pub fn save_project(file_path: &str, sentences: Vec<Sentence>) -> Result<bool, String> {
    let data = serde_json::json!({ "sentences": sentences });
    fs::write(file_path, serde_json::to_string_pretty(&data).unwrap())
        .map_err(|e| e.to_string())?;
    Ok(true)
}
