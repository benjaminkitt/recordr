use std::fs;
use std::path::Path;
use std::ffi::OsStr;
use csv::ReaderBuilder; // Assuming you're using the `csv` crate

#[derive(Clone, serde::Serialize)]
pub struct Sentence {
    pub text: String,
    pub recorded: bool,
    pub audio_file_path: Option<String>,
}

#[tauri::command]
pub async fn import_sentences(file_path: &str, project_dir: &str) -> Result<Vec<Sentence>, String> {
    // 1. Read the file contents
    let file_contents = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

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
        .map(|sentence| {
            let audio_file_name = format!("{}.wav", sentence.text); 
            let audio_file_path = Path::new(project_dir)
                .join(audio_file_name)
                .to_string_lossy()
                .to_string();
            Sentence {
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
        .map(|line| Sentence {
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

    for result in rdr.records() {
        let record = result.map_err(|e| format!("Failed to parse: {}", e))?;
        if let Some(text) = record.get(0) {
            sentences.push(Sentence {
                text: text.to_string(),
                recorded: false,
                audio_file_path: None,
            });
        }
    }

    Ok(sentences) // Add this line to return the sentences
}
